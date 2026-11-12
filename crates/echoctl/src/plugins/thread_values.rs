//! A plugin's values per thread (`ctx.threads.get / set / list`): `<data>/.echo/plugins/<id>/threads.json`,
//! `{thread id: {born, values: {key: value}}}` — in the plugin's own folder, written only through here
//! (`ctx.store.write` and `remove` refuse the file). They follow the thread: dropped when it is deleted, moved to the
//! thread it is merged into. `born` is the thread's first stage time: a thread id used again (`projects::create`)
//! never inherits the values of the thread that had it before.
//!
//! Every change is read-change-write under the plugin's lock across processes (`.locks/<id>.threads.lock` beside the
//! stores): the window, a terminal's `echoctl plugin call` and `echoctl project delete` never undo each other.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};

use super::data_dir;
use crate::paths;

/// The file in a plugin's store.
pub const FILE: &str = "threads.json";
/// One value, as JSON, at most this long.
const MAX_VALUE: usize = 16 * 1024;
/// Keys a thread may have, per plugin.
const MAX_KEYS: usize = 64;
/// The whole file, at most this long.
const MAX_FILE: usize = 4 * 1024 * 1024;
/// Events for the app waiting in a file (a terminal's change), at most this long.
const MAX_QUEUE: u64 = 4 * 1024 * 1024;

fn stores() -> PathBuf {
    paths::state_dir().join("plugins")
}

fn file(plugin: &str) -> PathBuf {
    data_dir(plugin).join(FILE)
}

/// The plugin's values file, locked across processes for one read-change-write.
fn lock(plugin: &str) -> Result<crate::app::FileLock> {
    let dir = stores().join(".locks");
    std::fs::create_dir_all(&dir)?;
    super::wait_lock(&dir.join(format!("{plugin}.threads.lock")))
}

/// One thread's values and whose they are (`born`: that thread's first stage time; none in a file from before it).
struct Entry {
    born: Option<String>,
    values: Map<String, Value>,
}

impl Entry {
    fn from_json(v: Value) -> Entry {
        match v {
            Value::Object(mut o) if o.get("values").is_some_and(Value::is_object) && o.keys().all(|k| k == "values" || k == "born") => {
                let born = o.get("born").and_then(Value::as_str).map(str::to_owned);
                let Some(Value::Object(values)) = o.remove("values") else { unreachable!() };
                Entry { born, values }
            }
            // As #465 wrote it: the values alone.
            Value::Object(values) => Entry { born: None, values },
            _ => Entry { born: None, values: Map::new() },
        }
    }

    fn to_json(&self) -> Value {
        let mut o = json!({"values": self.values});
        if let Some(b) = &self.born {
            o["born"] = json!(b);
        }
        o
    }

    /// Its values belong to the thread born then (an entry or a thread without a time: yes).
    fn of(&self, born: &Option<String>) -> bool {
        self.born.is_none() || born.is_none() || self.born == *born
    }
}

/// The thread's identity, or none when there is no such thread.
fn born(thread: &str) -> Option<Option<String>> {
    crate::projects::load(thread).ok().map(|p| p.stages.first().map(|s| s.at.clone()))
}

/// Its values: a missing file is none; one that can't be read is an error.
fn read(plugin: &str) -> Result<Map<String, Value>> {
    let p = file(plugin);
    match std::fs::read_to_string(&p) {
        Ok(t) => serde_json::from_str(&t).with_context(|| format!("{} isn't readable JSON", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(anyhow!("reading {}: {e}", p.display())),
    }
}

/// The same, to change it (under its lock): a file that isn't JSON is moved aside (`threads.json.bad-<time>`, logged)
/// and its values start empty — never written over; one that can't be read stops the change.
fn read_to_change(plugin: &str) -> Result<Map<String, Value>> {
    let p = file(plugin);
    match read(plugin) {
        Err(e) if p.is_file() && std::fs::read_to_string(&p).is_ok() => {
            let aside = paths::unused(&data_dir(plugin), &format!("{FILE}.bad-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")), "");
            std::fs::rename(&p, &aside).with_context(|| format!("moving {} aside", p.display()))?;
            eprintln!("echoctl: plugin {plugin}: {e:#} — moved aside to {}", aside.display());
            Ok(Map::new())
        }
        r => r,
    }
}

fn write(plugin: &str, all: &Map<String, Value>) -> Result<()> {
    let text = serde_json::to_string_pretty(all)?;
    if text.len() > MAX_FILE {
        bail!("a plugin's thread values are {} MB at most altogether", MAX_FILE / (1024 * 1024));
    }
    std::fs::create_dir_all(data_dir(plugin))?;
    paths::write_atomic(&file(plugin), text.as_bytes())?;
    Ok(())
}

fn check_key(key: &str) -> Result<()> {
    if key.is_empty() || key.chars().count() > 80 {
        bail!("a thread value needs a key of 1–80 characters");
    }
    Ok(())
}

/// Its value `key` for `thread` (null: none, or the thread is gone).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn get(plugin: &str, thread: &str, key: &str) -> Result<Value> {
    let Some(born) = born(thread) else { return Ok(Value::Null) };
    let mut all = read(plugin)?;
    Ok(all.remove(thread).map(Entry::from_json).filter(|e| e.of(&born)).and_then(|mut e| e.values.remove(key)).unwrap_or(Value::Null))
}

/// Keep `value` as its `key` for `thread` (null removes it): a thread there is, a value of 16 KB at most, 64 keys a
/// thread at most, the whole file 4 MB at most.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn set(plugin: &str, thread: &str, key: &str, value: &Value) -> Result<()> {
    check_key(key)?;
    if value.to_string().len() > MAX_VALUE {
        bail!("a thread value is 16 KB at most");
    }
    let _l = lock(plugin)?;
    // Inside the lock: a thread deleted meanwhile has dropped its values already, and gets none now.
    let Some(born) = born(thread) else { bail!("there is no thread “{thread}”") };
    let mut all = read_to_change(plugin)?;
    let mut e = all.remove(thread).map(Entry::from_json).filter(|e| e.of(&born)).unwrap_or(Entry { born: None, values: Map::new() });
    e.born = born;
    if value.is_null() {
        e.values.remove(key);
    } else {
        if !e.values.contains_key(key) && e.values.len() >= MAX_KEYS {
            bail!("a thread keeps {MAX_KEYS} values per plugin at most");
        }
        e.values.insert(key.to_string(), value.clone());
    }
    if !e.values.is_empty() {
        all.insert(thread.to_string(), e.to_json());
    }
    write(plugin, &all)
}

/// The threads that have a value `key`, with it: `[{thread, value}]` — a thread that is gone left out.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn list(plugin: &str, key: &str) -> Result<Value> {
    check_key(key)?;
    Ok(Value::Array(
        read(plugin)?
            .into_iter()
            .map(|(thread, e)| (thread, Entry::from_json(e)))
            .filter(|(thread, e)| born(thread).is_some_and(|b| e.of(&b)))
            .filter_map(|(thread, mut e)| e.values.remove(key).map(|v| json!({"thread": thread, "value": v})))
            .collect(),
    ))
}

/// What happened to a thread.
pub enum Change<'a> {
    /// Its name changed (`from` the old one).
    Renamed { name: &'a str, from: &'a str },
    /// Merged into the thread `into`.
    #[allow(dead_code)] // Echo merges no threads yet: the plugins' side is ready for it.
    Merged { into: &'a str },
    /// Deleted (its folder is in the trash).
    Deleted,
}

/// A thread was renamed, merged or deleted: the one place that follows it. Every plugin's values follow it (deleted:
/// gone; merged: each key `into` lacks moved to it), then the plugins that asked hear it (`threads_changed`:
/// `{thread, change, name, from}` renamed, `{thread, change, into}` merged, `{thread, change, values}` deleted — each
/// plugin its own removed values, to clean what it keeps elsewhere). A merge into a thread that isn't there is refused.
pub fn changed(thread: &str, change: Change) -> Result<()> {
    let into = match change {
        Change::Merged { into } => Some((into, born(into).ok_or_else(|| anyhow!("there is no thread “{into}” to merge into"))?)),
        _ => None,
    };
    let mut each: Map<String, Value> = Map::new();
    if !matches!(change, Change::Renamed { .. }) {
        let mut names: Vec<String> = std::fs::read_dir(stores())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !n.starts_with('.'))
            .collect();
        names.sort();
        for plugin in names {
            match follow(&plugin, thread, into.as_ref()) {
                Ok(Some(values)) => drop(each.insert(plugin, json!({"values": values}))),
                Ok(None) => {}
                Err(e) => eprintln!("echoctl: plugin {plugin}: its values for thread {thread}: {e:#}"),
            }
        }
    }
    let payload = match change {
        Change::Renamed { name, from } => json!({"thread": thread, "change": "renamed", "name": name, "from": from}),
        Change::Merged { into } => json!({"thread": thread, "change": "merged", "into": into}),
        Change::Deleted => json!({"thread": thread, "change": "deleted", "values": {}}),
    };
    // Each plugin hears its own values only (deleted).
    let each = if matches!(change, Change::Deleted) { each } else { Map::new() };
    tell(&payload, &each);
    Ok(())
}

/// One plugin's values for `thread`, taken out under its lock (and moved key by key into `into`): what it had.
fn follow(plugin: &str, thread: &str, into: Option<&(&str, Option<String>)>) -> Result<Option<Map<String, Value>>> {
    if !file(plugin).exists() {
        return Ok(None);
    }
    let _l = lock(plugin)?;
    let mut all = read_to_change(plugin)?;
    let Some(gone) = all.remove(thread).map(Entry::from_json) else { return Ok(None) };
    if let Some((into, born)) = into {
        let mut e = all.remove(*into).map(Entry::from_json).filter(|e| e.of(born)).unwrap_or(Entry { born: None, values: Map::new() });
        e.born = born.clone();
        for (k, v) in &gone.values {
            if !e.values.contains_key(k) && e.values.len() < MAX_KEYS {
                e.values.insert(k.clone(), v.clone());
            }
        }
        if !e.values.is_empty() {
            all.insert(into.to_string(), e.to_json());
        }
    }
    write(plugin, &all)?;
    Ok(Some(gone.values))
}

// ── telling the plugins: here when this process hosts them (the app), else through a file the app takes ──

fn queue_file() -> PathBuf {
    paths::state_dir().join("plugin-events.jsonl")
}

fn queue_lock() -> Result<crate::app::FileLock> {
    std::fs::create_dir_all(paths::state_dir())?;
    super::wait_lock(&paths::state_dir().join("plugin-events.lock"))
}

/// `threads_changed` to the plugins: the app's own host hears it now; a terminal's change (`echoctl project delete`,
/// no host here, or `echoctl plugin call`'s host without the app's timers) waits in a file for the running app, which
/// takes it within a second (`take_queued`) — or at its next start when it isn't running.
fn tell(payload: &Value, each: &Map<String, Value>) {
    #[cfg(feature = "js")]
    if crate::jshost::hosts_here() {
        crate::jshost::event_each("threads_changed", payload, each);
        return;
    }
    if let Err(e) = queue(payload, each) {
        eprintln!("echoctl: the running app won't hear that thread {}: {e:#}", payload["thread"].as_str().unwrap_or(""));
    }
}

fn queue(payload: &Value, each: &Map<String, Value>) -> Result<()> {
    use std::io::Write;
    let line = json!({"event": "threads_changed", "payload": payload, "each": each}).to_string();
    let _l = queue_lock()?;
    let p = queue_file();
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) + line.len() as u64 > MAX_QUEUE {
        bail!("{} is full (the app hasn't taken it)", p.display());
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// The events a terminal left for the app, taken (the file emptied): `(event, payload, each plugin's part)`.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn take_queued() -> Vec<(String, Value, Map<String, Value>)> {
    let p = queue_file();
    if !p.exists() {
        return vec![];
    }
    let Ok(_l) = queue_lock() else { return vec![] };
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    let _ = std::fs::remove_file(&p);
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|mut v| {
            let each = v["each"].as_object().cloned().unwrap_or_default();
            Some((v["event"].as_str()?.to_string(), v["payload"].take(), each))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugins_values_per_thread_are_kept_listed_and_follow_the_thread() {
        let (_env, _root) = paths::testenv::folders("plugin-thread-values");
        let alpha = crate::projects::create("Alpha", None, None).unwrap().id;
        let beta = crate::projects::create("Beta", None, None).unwrap().id;
        set("gh", &alpha, "repos", &json!(["acme-test/api"])).unwrap();
        set("gh", &alpha, "board", &json!(7)).unwrap();
        assert_eq!(get("gh", &alpha, "repos").unwrap(), json!(["acme-test/api"]));
        assert_eq!(get("gh", &beta, "repos").unwrap(), Value::Null, "Alpha's only");
        assert_eq!(get("jira", &alpha, "repos").unwrap(), Value::Null, "the plugin's own");
        assert_eq!(list("gh", "repos").unwrap(), json!([{"thread": alpha, "value": ["acme-test/api"]}]));
        // Refused: no such thread, too big, no key, a key over 80 characters; exactly 16 KB is kept.
        assert!(set("gh", "nope", "repos", &json!(1)).unwrap_err().to_string().contains("no thread"));
        assert!(set("gh", &alpha, "big", &json!("x".repeat(17 * 1024))).unwrap_err().to_string().contains("16 KB"));
        assert!(set("gh", &alpha, "", &json!(1)).is_err());
        assert!(set("gh", &alpha, &"k".repeat(81), &json!(1)).unwrap_err().to_string().contains("1–80"));
        set("gh", &alpha, &"k".repeat(80), &json!(1)).unwrap();
        set("gh", &alpha, &"k".repeat(80), &Value::Null).unwrap();
        let exact = json!("x".repeat(MAX_VALUE - 2)); // its quotes make it 16 KB
        assert_eq!(exact.to_string().len(), MAX_VALUE);
        set("gh", &alpha, "edge", &exact).unwrap();
        assert!(set("gh", &alpha, "edge", &json!("x".repeat(MAX_VALUE - 1))).unwrap_err().to_string().contains("16 KB"));
        set("gh", &alpha, "edge", &Value::Null).unwrap();
        // null removes it.
        set("gh", &alpha, "board", &Value::Null).unwrap();
        assert_eq!(get("gh", &alpha, "board").unwrap(), Value::Null);
        // Merged into Beta, which has none: moved; into a thread that has its own key: Beta's kept, the others added.
        changed(&alpha, Change::Merged { into: &beta }).unwrap();
        assert_eq!(get("gh", &beta, "repos").unwrap(), json!(["acme-test/api"]));
        assert_eq!(get("gh", &alpha, "repos").unwrap(), Value::Null);
        set("gh", &alpha, "repos", &json!(["acme-test/web"])).unwrap();
        set("gh", &alpha, "board", &json!(9)).unwrap();
        changed(&alpha, Change::Merged { into: &beta }).unwrap();
        assert_eq!(get("gh", &beta, "repos").unwrap(), json!(["acme-test/api"]), "Beta's own kept");
        assert_eq!(get("gh", &beta, "board").unwrap(), json!(9), "the key Beta lacked added");
        assert!(changed(&alpha, Change::Merged { into: "nope" }).unwrap_err().to_string().contains("no thread"));
        // Renamed: kept. Deleted: gone, and a thread gone is left out of the list.
        changed(&beta, Change::Renamed { name: "Beta 2", from: "Beta" }).unwrap();
        assert_eq!(get("gh", &beta, "repos").unwrap(), json!(["acme-test/api"]));
        set("gh", &alpha, "repos", &json!(["acme-test/web"])).unwrap();
        crate::projects::delete(&alpha).unwrap();
        assert_eq!(get("gh", &alpha, "repos").unwrap(), Value::Null, "deleting a thread drops its values");
        assert!(!read("gh").unwrap().contains_key(&alpha));
        std::fs::write(file("gh"), json!({"gone": {"repos": 1}, beta.clone(): {"repos": 2}}).to_string()).unwrap();
        assert_eq!(list("gh", "repos").unwrap(), json!([{"thread": beta, "value": 2}]), "#465's file read as it was");
    }

    #[test]
    fn a_thread_id_used_again_inherits_nothing() {
        let (_env, _root) = paths::testenv::folders("plugin-thread-values-reuse");
        let a = crate::projects::create("Alpha", None, None).unwrap().id;
        set("gh", &a, "repos", &json!(["acme-test/api"])).unwrap();
        // Its values left behind (a delete that never reached them), then a new thread under the same id.
        let kept = std::fs::read_to_string(file("gh")).unwrap();
        crate::projects::delete(&a).unwrap();
        std::fs::write(file("gh"), &kept).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5)); // another first stage time
        let again = crate::projects::create("Alpha", None, None).unwrap().id;
        assert_eq!(again, a, "the id is used again");
        assert_eq!(get("gh", &a, "repos").unwrap(), Value::Null, "not the old thread's");
        assert_eq!(list("gh", "repos").unwrap(), json!([]));
        set("gh", &a, "board", &json!(1)).unwrap();
        assert_eq!(read("gh").unwrap()[&a]["values"], json!({"board": 1}), "the old values dropped when it is written");
    }

    #[test]
    fn caps_and_a_file_that_isnt_json() {
        let (_env, _root) = paths::testenv::folders("plugin-thread-values-caps");
        let a = crate::projects::create("Alpha", None, None).unwrap().id;
        for i in 0..MAX_KEYS {
            set("gh", &a, &format!("k{i}"), &json!(i)).unwrap();
        }
        assert!(set("gh", &a, "one-more", &json!(1)).unwrap_err().to_string().contains("64 values"));
        set("gh", &a, "k0", &json!("changed")).unwrap(); // a key it has: changed
        // The whole file: 4 MB at most (a file near it, then one more value).
        let b = crate::projects::create("Beta", None, None).unwrap().id;
        let mut all = read("gh").unwrap();
        all.insert(b.clone(), json!({"values": {"filler": "x".repeat(MAX_FILE - 8 * 1024)}}));
        std::fs::write(file("gh"), serde_json::to_string_pretty(&all).unwrap()).unwrap();
        let err = set("gh", &b, "big", &json!("x".repeat(MAX_VALUE - 2))).unwrap_err();
        assert!(err.to_string().contains("4 MB"), "{err}");
        set("gh", &b, "filler", &Value::Null).unwrap();
        // A file that isn't JSON: get and list say so; a change moves it aside and starts again, never over it.
        std::fs::write(file("gh"), "{\"half").unwrap();
        assert!(get("gh", &a, "k0").is_err());
        assert!(list("gh", "k0").is_err());
        set("gh", &a, "k0", &json!(1)).unwrap();
        let aside: Vec<_> = std::fs::read_dir(data_dir("gh")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("threads.json.bad-")).collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read_to_string(aside[0].path()).unwrap(), "{\"half");
        assert_eq!(get("gh", &a, "k0").unwrap(), json!(1));
    }

    #[test]
    fn a_terminals_change_waits_for_the_app() {
        let (_env, _root) = paths::testenv::folders("plugin-thread-values-queue");
        queue(&json!({"thread": "alpha", "change": "deleted", "values": {}}), &Map::from_iter([("gh".to_string(), json!({"values": {"repos": 1}}))])).unwrap();
        queue(&json!({"thread": "beta", "change": "renamed", "name": "B", "from": "Beta"}), &Map::new()).unwrap();
        let got = take_queued();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "threads_changed");
        assert_eq!(got[0].1["thread"], "alpha");
        assert_eq!(got[0].2["gh"], json!({"values": {"repos": 1}}));
        assert_eq!(got[1].1["from"], "Beta");
        assert!(take_queued().is_empty(), "taken once");
    }
}

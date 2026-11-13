//! A plugin's values per thread (`ctx.threads.get / set / list`): `<data>/.echo/plugins/<id>/threads.json`,
//! `{"v": 2, "threads": {thread id: {born, values: {key: value}}}}` — in the plugin's own folder, written only through
//! here (`ctx.store.write` and `remove` refuse the file). They follow the thread: dropped when it is deleted, moved to
//! the thread it is merged into. `born` is the thread's first stage time: a thread id used again (`projects::create`)
//! never inherits the values of the thread that had it before. A file without `"v": 2` is #465's, `{thread id: its
//! values}`, read as that and written in the new shape at its next change.
//!
//! Every change is read-change-write under the plugin's lock across processes (`.locks/<id>.threads.lock` beside the
//! stores): the window, a terminal's `echoctl plugin call` and `echoctl project delete` never undo each other.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};

use super::data_dir;
use crate::paths;

/// The file in a plugin's store.
pub const FILE: &str = "threads.json";
/// The file's shape (`"v"`): anything else is #465's.
const FORMAT: u64 = 2;
/// One value, as JSON, at most this long.
const MAX_VALUE: usize = 16 * 1024;
/// Keys a thread may have, per plugin.
const MAX_KEYS: usize = 64;
/// The whole file, at most this long.
const MAX_FILE: usize = 4 * 1024 * 1024;
/// Events for the app waiting in a file (a terminal's change), at most this long.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
const MAX_QUEUE: u64 = 4 * 1024 * 1024;
/// How long a change waits for a plugin's lock: less than the projects' lock (10 s), held around it by a delete.
const LOCK_WAIT: Duration = Duration::from_secs(5);

fn stores() -> PathBuf {
    paths::state_dir().join("plugins")
}

fn file(plugin: &str) -> PathBuf {
    data_dir(plugin).join(FILE)
}

fn lock_file(plugin: &str) -> PathBuf {
    stores().join(".locks").join(format!("{plugin}.threads.lock"))
}

/// The plugin's values file, locked across processes for one read-change-write.
fn lock(plugin: &str) -> Result<crate::app::FileLock> {
    let p = lock_file(plugin);
    std::fs::create_dir_all(p.parent().unwrap_or(&stores()))?;
    super::wait_lock_for(&p, &format!("{plugin}'s thread values"), LOCK_WAIT)
}

/// A plugin removed with its data: its lock goes too (its `threads.json` and any `threads.json.bad-*` go with its
/// folder).
pub fn forget(plugin: &str) {
    let p = lock_file(plugin);
    // Only when nobody holds it: a holder would keep a lock no other process sees.
    if let Ok(Some(l)) = crate::app::FileLock::try_take(&p) {
        drop(l);
        let _ = std::fs::remove_file(&p);
    }
}

/// One thread's values and whose they are (`born`: that thread's first stage time; none in a file from before it).
#[derive(Default)]
struct Entry {
    born: Option<String>,
    values: Map<String, Value>,
}

impl Entry {
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

type Threads = BTreeMap<String, Entry>;

/// The file's text read: `{"v": 2, "threads": {…}}`, or #465's `{thread: values}` (whatever keys its values have).
fn parse(text: &str) -> Result<Threads> {
    let Value::Object(mut top) = serde_json::from_str::<Value>(text)? else { bail!("it isn't a JSON object") };
    if top.get("v").and_then(Value::as_u64) == Some(FORMAT) {
        let Some(Value::Object(threads)) = top.remove("threads") else { bail!("it has no “threads”") };
        return Ok(threads
            .into_iter()
            .map(|(id, mut e)| {
                let born = e["born"].as_str().map(str::to_owned);
                (id, Entry { born, values: e.get_mut("values").and_then(Value::as_object_mut).map(std::mem::take).unwrap_or_default() })
            })
            .collect());
    }
    // #465's: a thread's values alone (a thread id is never "v" with a number: its values are an object).
    Ok(top.into_iter().map(|(id, v)| (id, Entry { born: None, values: if let Value::Object(o) = v { o } else { Map::new() } })).collect())
}

fn text(all: &Threads) -> Result<String> {
    let threads: Map<String, Value> = all.iter().map(|(id, e)| (id.clone(), e.to_json())).collect();
    Ok(serde_json::to_string_pretty(&json!({"v": FORMAT, "threads": threads}))?)
}

/// The thread's identity, or none when there is no such thread.
fn born(thread: &str) -> Option<Option<String>> {
    crate::projects::load(thread).ok().map(|p| p.stages.first().map(|s| s.at.clone()))
}

/// Its values: a missing file is none; one that can't be read is an error.
fn read(plugin: &str) -> Result<Threads> {
    let p = file(plugin);
    match std::fs::read_to_string(&p) {
        Ok(t) => parse(&t).with_context(|| format!("{} isn't readable", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Threads::new()),
        Err(e) => Err(anyhow!("reading {}: {e}", p.display())),
    }
}

/// The same, to change it (under its lock): a file that isn't JSON is moved aside (`threads.json.bad-<time>`, logged)
/// and its values start empty — never written over; one that can't be read stops the change.
fn read_to_change(plugin: &str) -> Result<Threads> {
    let p = file(plugin);
    match read(plugin) {
        Err(e) if p.is_file() && std::fs::read_to_string(&p).is_ok() => {
            let aside = paths::unused(&data_dir(plugin), &format!("{FILE}.bad-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")), "");
            std::fs::rename(&p, &aside).with_context(|| format!("moving {} aside", p.display()))?;
            eprintln!("echoctl: plugin {plugin}: {e:#} — moved aside to {}", aside.display());
            Ok(Threads::new())
        }
        r => r,
    }
}

/// Written (under its lock). Over 4 MB only when it was already over and gets no bigger: a file from before the cap
/// still loses values (a key set to null, a thread deleted).
fn write(plugin: &str, all: &Threads) -> Result<()> {
    let text = text(all)?;
    if text.len() > MAX_FILE {
        // What is there now, in the same shape (a #465 file grows a little when it is rewritten).
        let before = read(plugin).ok().and_then(|t| self::text(&t).ok()).map_or(0, |t| t.len());
        if text.len() > before {
            bail!("a plugin's thread values are {} MB at most altogether", MAX_FILE / (1024 * 1024));
        }
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
    Ok(all.remove(thread).filter(|e| e.of(&born)).and_then(|mut e| e.values.remove(key)).unwrap_or(Value::Null))
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
    let mut e = all.remove(thread).filter(|e| e.of(&born)).unwrap_or_default();
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
        all.insert(thread.to_string(), e);
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
    /// Deleted (its folder is in the trash): `born`, its first stage time, read before it went.
    Deleted { born: Option<String> },
}

/// A thread was renamed, merged or deleted: the one place that follows it. Every plugin's values follow it (deleted:
/// gone; merged: each key `into` lacks moved to it while it has fewer than 64), then the plugins that asked hear it
/// (`threads_changed`: `{thread, born, change, name, from}` renamed, `{thread, born, change, into, dropped}` merged —
/// each plugin the keys of its own that didn't fit — `{thread, born, change, values}` deleted — each plugin its own
/// removed values, to clean what it keeps elsewhere). `born` tells this thread from a later one under the same id. A
/// merge into a thread that isn't there is refused.
pub fn changed(thread: &str, change: Change) -> Result<()> {
    let into = match change {
        Change::Merged { into } => Some((into, born(into).ok_or_else(|| anyhow!("there is no thread “{into}” to merge into"))?)),
        _ => None,
    };
    let was = match &change {
        Change::Deleted { born } => born.clone(),
        _ => born(thread).flatten(),
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
                Ok(Some((values, dropped))) => match change {
                    Change::Deleted { .. } => drop(each.insert(plugin, json!({"values": values}))),
                    _ if !dropped.is_empty() => drop(each.insert(plugin, json!({"dropped": dropped}))),
                    _ => {}
                },
                Ok(None) => {}
                Err(e) => eprintln!("echoctl: plugin {plugin}: its values for thread {thread}: {e:#}"),
            }
        }
        // The plugins' work rows asked for it (`ctx.work.provide`, its Work tab) go with it — in whichever process
        // deletes or merges the thread (each plugin's work.json under its own lock); `into` asks for its own again.
        #[cfg(feature = "js")]
        crate::jshost::work::forget_thread(thread);
        // Its dashboard and the plugins' answers for it too (`into` keeps its own).
        crate::today_layout::forget(&crate::today_layout::Page::Thread(thread.to_string()));
    }
    let payload = match change {
        Change::Renamed { name, from } => json!({"thread": thread, "born": was, "change": "renamed", "name": name, "from": from}),
        Change::Merged { into } => json!({"thread": thread, "born": was, "change": "merged", "into": into, "dropped": []}),
        Change::Deleted { .. } => json!({"thread": thread, "born": was, "change": "deleted", "values": {}}),
    };
    tell(&payload, &each);
    Ok(())
}

/// What a thread had (its values) and the keys that didn't fit in the thread it was merged into.
type Followed = (Map<String, Value>, Vec<String>);

/// One plugin's values for `thread`, taken out under its lock (and moved key by key into `into`): what it had, and the
/// keys that didn't fit in `into` (64 at most).
fn follow(plugin: &str, thread: &str, into: Option<&(&str, Option<String>)>) -> Result<Option<Followed>> {
    if !file(plugin).exists() {
        return Ok(None);
    }
    let _l = lock(plugin)?;
    let mut all = read_to_change(plugin)?;
    let Some(gone) = all.remove(thread) else { return Ok(None) };
    let mut dropped = vec![];
    if let Some((into, born)) = into {
        let mut e = all.remove(*into).filter(|e| e.of(born)).unwrap_or_default();
        e.born = born.clone();
        for (k, v) in &gone.values {
            if e.values.contains_key(k) {
                continue;
            }
            if e.values.len() < MAX_KEYS {
                e.values.insert(k.clone(), v.clone());
            } else {
                dropped.push(k.clone());
            }
        }
        if !e.values.is_empty() {
            all.insert(into.to_string(), e);
        }
    }
    write(plugin, &all)?;
    Ok(Some((gone.values, dropped)))
}

// ── telling the plugins: here when this process is the app, else through a file the app takes ──

#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn queue_file() -> PathBuf {
    paths::state_dir().join("plugin-events.jsonl")
}

/// The events being taken: kept until they have been handed on (a crash before that leaves them for the next start).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn taking_file() -> PathBuf {
    paths::state_dir().join("plugin-events.jsonl.taking")
}

#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn queue_lock_path() -> Result<PathBuf> {
    std::fs::create_dir_all(paths::state_dir())?;
    Ok(paths::state_dir().join("plugin-events.lock"))
}

/// `threads_changed` to the plugins: the app's own host hears it now; another process's change (`echoctl project
/// delete`, `echoctl plugin call`, `echoctl widget show`) waits in a file for the running app, which takes it within a
/// second (`take_queued`) — or at its next start when it isn't running. A build without plugin hosts queues nothing:
/// no app of its would take it.
fn tell(payload: &Value, each: &Map<String, Value>) {
    #[cfg(feature = "js")]
    {
        if crate::jshost::hosts_here() {
            crate::jshost::event_each("threads_changed", payload, each);
            return;
        }
        if let Err(e) = queue(payload, each) {
            eprintln!("echoctl: the running app won't hear that thread {}: {e:#}", payload["thread"].as_str().unwrap_or(""));
        }
    }
    #[cfg(not(feature = "js"))]
    let _ = (payload, each);
}

#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn queue(payload: &Value, each: &Map<String, Value>) -> Result<()> {
    use std::io::Write;
    let line = json!({"event": "threads_changed", "payload": payload, "each": each}).to_string();
    let _l = super::wait_lock_for(&queue_lock_path()?, "the plugins' events for the app", LOCK_WAIT)?;
    let p = queue_file();
    if std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0) + line.len() as u64 > MAX_QUEUE {
        bail!("{} is full (the app hasn't taken it)", p.display());
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&p)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// A queued `deleted` or `renamed` of a thread whose id now has another thread (another first stage time): not that
/// thread's news.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn stale(payload: &Value) -> bool {
    let (Some(thread), Some(was)) = (payload["thread"].as_str(), payload["born"].as_str()) else { return false };
    matches!(payload["change"].as_str(), Some("deleted" | "renamed")) && born(thread).is_some_and(|now| now.is_some_and(|n| n != was))
}

/// The events another process left for the app, each handed to `deliver(event, payload, each plugin's part)` — at
/// least once: the file is renamed aside (`.taking`) first and removed only after all were handed on; one left aside
/// (a crash, a remove that failed) is handed on first, next time. A file that can't be read stays. Never waits: a busy
/// lock is tried again at the next tick.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn take_queued(mut deliver: impl FnMut(&str, &Value, &Map<String, Value>)) {
    let (p, taking) = (queue_file(), taking_file());
    if !p.exists() && !taking.exists() {
        return;
    }
    let Ok(Some(_l)) = queue_lock_path().and_then(|l| Ok(crate::app::FileLock::try_take(&l)?)) else { return };
    if !taking.exists() {
        if let Err(e) = std::fs::rename(&p, &taking) {
            if p.exists() {
                eprintln!("echoctl: taking {}: {e}", p.display());
            }
            return;
        }
    }
    let text = match std::fs::read_to_string(&taking) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("echoctl: reading {}: {e} — left for later", taking.display());
            return;
        }
    };
    for line in text.lines() {
        let Ok(mut v) = serde_json::from_str::<Value>(line) else { continue };
        let Some(event) = v["event"].as_str().map(str::to_owned) else { continue };
        let payload = v["payload"].take();
        if stale(&payload) {
            continue;
        }
        let each = v["each"].as_object().cloned().unwrap_or_default();
        deliver(&event, &payload, &each);
    }
    if let Err(e) = std::fs::remove_file(&taking) {
        eprintln!("echoctl: removing {}: {e} — its events may be heard again", taking.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn taken() -> Vec<(String, Value, Map<String, Value>)> {
        let mut got = vec![];
        take_queued(|e, p, each| got.push((e.to_string(), p.clone(), each.clone())));
        got
    }

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
        let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(file("gh")).unwrap()).unwrap();
        assert_eq!(on_disk["v"], json!(2), "{on_disk}");
        assert_eq!(on_disk["threads"][&beta]["values"]["repos"], json!(["acme-test/api"]));
        // No app here: what the plugins hear waits in the file, `born` with it.
        let heard = taken();
        let deleted = heard.iter().find(|h| h.1["change"] == "deleted").unwrap();
        assert_eq!(deleted.1["thread"], json!(alpha));
        assert!(deleted.1["born"].is_string(), "{}", deleted.1);
        assert_eq!(deleted.2["gh"], json!({"values": {"repos": ["acme-test/web"]}}));
        assert!(heard.iter().any(|h| h.1["change"] == "renamed" && h.1["born"].is_string()));
    }

    #[test]
    fn a_file_from_465_is_read_as_it_was_whatever_its_keys() {
        let (_env, _root) = paths::testenv::folders("plugin-thread-values-465");
        let beta = crate::projects::create("Beta", None, None).unwrap().id;
        let gamma = crate::projects::create("Gamma", None, None).unwrap().id;
        // #465 wrote {thread: its values}: here a plugin's own key named "values", and one named "born".
        std::fs::create_dir_all(data_dir("gh")).unwrap();
        let old = json!({"gone": {"repos": 1}, beta.clone(): {"values": {"a": 1}}, gamma.clone(): {"values": {"b": 2}, "born": "mine"}});
        std::fs::write(file("gh"), old.to_string()).unwrap();
        assert_eq!(get("gh", &beta, "values").unwrap(), json!({"a": 1}), "the plugin's own key, not the shape");
        assert_eq!(get("gh", &gamma, "born").unwrap(), json!("mine"));
        assert_eq!(list("gh", "values").unwrap(), json!([{"thread": beta, "value": {"a": 1}}, {"thread": gamma, "value": {"b": 2}}]));
        // Written at its next change in the new shape, the keys kept.
        set("gh", &beta, "repos", &json!(2)).unwrap();
        assert_eq!(get("gh", &beta, "values").unwrap(), json!({"a": 1}));
        assert_eq!(get("gh", &gamma, "born").unwrap(), json!("mine"));
        let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(file("gh")).unwrap()).unwrap();
        assert_eq!(on_disk["threads"][&beta]["values"], json!({"values": {"a": 1}, "repos": 2}));
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
        assert_eq!(read("gh").unwrap()[&a].values, json!({"board": 1}).as_object().unwrap().clone(), "the old values dropped when it is written");
        // The old thread's "deleted", queued while the app was closed, never reaches the new one.
        assert!(taken().iter().all(|h| h.1["change"] != "deleted"), "dropped: the id has another thread now");
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
        // A merge into a thread that has 64: the keys that don't fit are named to the plugin.
        let c = crate::projects::create("Gamma", None, None).unwrap().id;
        set("gh", &c, "extra", &json!(1)).unwrap();
        set("gh", &c, "k1", &json!("gamma's")).unwrap();
        let _ = taken();
        changed(&c, Change::Merged { into: &a }).unwrap();
        let merged = taken().into_iter().find(|h| h.1["change"] == "merged").unwrap();
        assert_eq!(merged.1["dropped"], json!([]));
        assert_eq!(merged.2["gh"], json!({"dropped": ["extra"]}), "k1 Alpha had already: kept, not dropped");
        // The whole file: 4 MB at most (a file near it, then one more value).
        let b = crate::projects::create("Beta", None, None).unwrap().id;
        let mut all = read("gh").unwrap();
        all.insert(b.clone(), Entry { born: None, values: Map::from_iter([("filler".to_string(), json!("x".repeat(MAX_FILE - 8 * 1024)))]) });
        std::fs::write(file("gh"), text(&all).unwrap()).unwrap();
        let err = set("gh", &b, "big", &json!("x".repeat(MAX_VALUE - 2))).unwrap_err();
        assert!(err.to_string().contains("4 MB"), "{err}");
        set("gh", &b, "filler", &Value::Null).unwrap();
        // A file over 4 MB already (#465 had no cap): it can still lose values, never gain them.
        let mut old = Map::new();
        old.insert(a.clone(), json!({"keep": 1}));
        for i in 0..5 {
            old.insert(b.clone() + &i.to_string(), json!({"filler": "x".repeat(MAX_FILE / 4)}));
        }
        old.insert(b.clone(), json!({"filler": "x".repeat(MAX_FILE / 4)}));
        std::fs::write(file("gh"), Value::Object(old).to_string()).unwrap();
        assert!(set("gh", &a, "more", &json!(1)).unwrap_err().to_string().contains("4 MB"));
        set("gh", &a, "keep", &Value::Null).unwrap();
        crate::projects::delete(&b).unwrap();
        assert!(!read("gh").unwrap().contains_key(&b), "a deleted thread's values go even from a file over the cap");
        // A file that isn't JSON: get and list say so; a change moves it aside and starts again, never over it.
        std::fs::write(file("gh"), "{\"half").unwrap();
        assert!(get("gh", &a, "k0").is_err());
        assert!(list("gh", "k0").is_err());
        set("gh", &a, "k0", &json!(1)).unwrap();
        let aside: Vec<_> = std::fs::read_dir(data_dir("gh")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("threads.json.bad-")).collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read_to_string(aside[0].path()).unwrap(), "{\"half");
        assert_eq!(get("gh", &a, "k0").unwrap(), json!(1));
        // Removed with its data: its lock goes too.
        assert!(lock_file("gh").exists());
        forget("gh");
        assert!(!lock_file("gh").exists());
    }

    #[test]
    fn a_terminals_change_waits_for_the_app_and_is_heard_at_least_once() {
        let (_env, _root) = paths::testenv::folders("plugin-thread-values-queue");
        queue(&json!({"thread": "alpha", "change": "deleted", "values": {}}), &Map::from_iter([("gh".to_string(), json!({"values": {"repos": 1}}))])).unwrap();
        queue(&json!({"thread": "beta", "change": "renamed", "name": "B", "from": "Beta"}), &Map::new()).unwrap();
        let got = taken();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "threads_changed");
        assert_eq!(got[0].1["thread"], "alpha");
        assert_eq!(got[0].2["gh"], json!({"values": {"repos": 1}}));
        assert_eq!(got[1].1["from"], "Beta");
        assert!(taken().is_empty(), "taken once");
        assert!(!queue_file().exists() && !taking_file().exists());
        // Left aside by a crash: handed on first, then what came after it.
        std::fs::write(taking_file(), format!("{}\n", json!({"event": "threads_changed", "payload": {"thread": "old"}}))).unwrap();
        queue(&json!({"thread": "new"}), &Map::new()).unwrap();
        assert_eq!(taken().iter().map(|h| h.1["thread"].clone()).collect::<Vec<_>>(), vec![json!("old")]);
        assert_eq!(taken().iter().map(|h| h.1["thread"].clone()).collect::<Vec<_>>(), vec![json!("new")]);
        // One that can't be read is left, never lost.
        std::fs::create_dir_all(taking_file()).unwrap();
        queue(&json!({"thread": "later"}), &Map::new()).unwrap();
        assert!(taken().is_empty());
        assert!(taking_file().is_dir() && queue_file().exists(), "both still there");
        std::fs::remove_dir(taking_file()).unwrap();
        assert_eq!(taken().len(), 1);
        // A busy lock: nothing taken this time, nothing lost.
        queue(&json!({"thread": "busy"}), &Map::new()).unwrap();
        let held = crate::app::FileLock::try_take(&queue_lock_path().unwrap()).unwrap().unwrap();
        assert!(taken().is_empty());
        drop(held);
        assert_eq!(taken().len(), 1);
    }
}

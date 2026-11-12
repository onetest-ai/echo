//! A plugin's values per thread (`ctx.threads.get / set / list`): `<data>/.echo/plugins/<id>/threads.json`,
//! `{thread id: {key: value}}` — in the plugin's own folder, written only through here (`ctx.store.write` refuses the
//! file). They follow the thread: dropped when it is deleted, moved to the thread it is merged into.

use std::sync::Mutex;

use anyhow::{bail, Result};
use serde_json::{json, Map, Value};

use super::{data_dir, read_map};
use crate::paths;

/// The file in a plugin's store.
pub const FILE: &str = "threads.json";
/// One value, as JSON, at most this long.
const MAX_VALUE: usize = 16 * 1024;

static LOCK: Mutex<()> = Mutex::new(());

fn file(plugin: &str) -> std::path::PathBuf {
    data_dir(plugin).join(FILE)
}

fn write(plugin: &str, all: &Map<String, Value>) -> Result<()> {
    std::fs::create_dir_all(data_dir(plugin))?;
    paths::write_atomic(&file(plugin), serde_json::to_string_pretty(all)?.as_bytes())?;
    Ok(())
}

fn check_key(key: &str) -> Result<()> {
    if key.is_empty() || key.chars().count() > 80 {
        bail!("a thread value needs a key of 1–80 characters");
    }
    Ok(())
}

/// Its value `key` for `thread` (null: none).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn get(plugin: &str, thread: &str, key: &str) -> Value {
    read_map(&file(plugin)).get(thread).and_then(|t| t.get(key)).cloned().unwrap_or(Value::Null)
}

/// Keep `value` as its `key` for `thread` (null removes it): a thread there is, a value of 16 KB at most.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn set(plugin: &str, thread: &str, key: &str, value: &Value) -> Result<()> {
    check_key(key)?;
    if crate::projects::load(thread).is_err() {
        bail!("there is no thread “{thread}”");
    }
    if value.to_string().len() > MAX_VALUE {
        bail!("a thread value is 16 KB at most");
    }
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read_map(&file(plugin));
    let t = all.entry(thread.to_string()).or_insert_with(|| json!({}));
    if !t.is_object() {
        *t = json!({});
    }
    let m = t.as_object_mut().expect("an object");
    if value.is_null() {
        m.remove(key);
    } else {
        m.insert(key.to_string(), value.clone());
    }
    if m.is_empty() {
        all.remove(thread);
    }
    write(plugin, &all)
}

/// The threads that have a value `key`, with it: `[{thread, value}]` — a thread that is gone left out.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn list(plugin: &str, key: &str) -> Result<Value> {
    check_key(key)?;
    Ok(Value::Array(
        read_map(&file(plugin))
            .into_iter()
            .filter_map(|(thread, t)| t.get(key).cloned().map(|v| (thread, v)))
            .filter(|(thread, _)| crate::projects::load(thread).is_ok())
            .map(|(thread, value)| json!({"thread": thread, "value": value}))
            .collect(),
    ))
}

/// A thread was renamed, merged into `into`, or deleted: every plugin's values follow it (deleted: gone; merged: moved
/// to `into` when it has none of that plugin's), then the plugins that asked hear it (`threads_changed`).
pub fn changed(thread: &str, change: &str, into: Option<&str>) {
    if change != "renamed" {
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let stores = std::fs::read_dir(paths::state_dir().join("plugins")).into_iter().flatten().flatten();
        for plugin in stores.map(|e| e.file_name().to_string_lossy().into_owned()) {
            let mut all = read_map(&file(&plugin));
            let Some(values) = all.remove(thread) else { continue };
            if let Some(into) = into.filter(|_| change == "merged") {
                all.entry(into.to_string()).or_insert(values);
            }
            if let Err(e) = write(&plugin, &all) {
                eprintln!("echoctl: plugin {plugin}: its values for thread {thread}: {e:#}");
            }
        }
    }
    #[cfg(feature = "js")]
    {
        let mut payload = json!({"thread": thread, "change": change});
        if let Some(i) = into {
            payload["into"] = json!(i);
        }
        crate::jshost::event("threads_changed", &payload);
    }
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
        assert_eq!(get("gh", &alpha, "repos"), json!(["acme-test/api"]));
        assert_eq!(get("gh", &beta, "repos"), Value::Null, "Alpha's only");
        assert_eq!(get("jira", &alpha, "repos"), Value::Null, "the plugin's own");
        assert_eq!(list("gh", "repos").unwrap(), json!([{"thread": alpha, "value": ["acme-test/api"]}]));
        // Refused: no such thread, too big, no key.
        assert!(set("gh", "nope", "repos", &json!(1)).unwrap_err().to_string().contains("no thread"));
        assert!(set("gh", &alpha, "big", &json!("x".repeat(17 * 1024))).unwrap_err().to_string().contains("16 KB"));
        assert!(set("gh", &alpha, "", &json!(1)).is_err());
        // null removes it.
        set("gh", &alpha, "board", &Value::Null).unwrap();
        assert_eq!(get("gh", &alpha, "board"), Value::Null);
        // Merged into Beta, which has none: moved; into a thread that has its own: Beta's kept.
        changed(&alpha, "merged", Some(&beta));
        assert_eq!(get("gh", &beta, "repos"), json!(["acme-test/api"]));
        assert_eq!(get("gh", &alpha, "repos"), Value::Null);
        set("gh", &alpha, "repos", &json!(["acme-test/web"])).unwrap();
        changed(&alpha, "merged", Some(&beta));
        assert_eq!(get("gh", &beta, "repos"), json!(["acme-test/api"]));
        // Renamed: kept. Deleted: gone, and a thread gone is left out of the list.
        changed(&beta, "renamed", None);
        assert_eq!(get("gh", &beta, "repos"), json!(["acme-test/api"]));
        set("gh", &alpha, "repos", &json!(["acme-test/web"])).unwrap();
        crate::projects::delete(&alpha).unwrap();
        assert_eq!(get("gh", &alpha, "repos"), Value::Null, "deleting a thread drops its values");
        std::fs::write(file("gh"), json!({"gone": {"repos": 1}, beta.clone(): {"repos": 2}}).to_string()).unwrap();
        assert_eq!(list("gh", "repos").unwrap(), json!([{"thread": beta, "value": 2}]));
    }
}

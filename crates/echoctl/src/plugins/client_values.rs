//! A plugin's values per client (`ctx.clients.get / set / list`): `<data>/.echo/plugins/<id>/client-values.json`,
//! `{"v": 1, "clients": {client id: {key: value}}}` — in the plugin's own folder, written only through here
//! (`ctx.store.write` and `remove` refuse the file). The twin of `thread_values`, with its limits: they follow the
//! client — kept when it is renamed (its id stays), dropped when it is removed (`projects::remove_client`), so a client
//! id used again inherits nothing. Only the plugin's own keys: Echo's own client fields are never reached through here,
//! and a key with a dot (a marketplace's shared one, for threads) is refused.
//!
//! Every change is read-change-write under the plugin's lock across processes (`.locks/<id>.clients.lock`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Map, Value};

use super::data_dir;
use super::thread_values::{LOCK_WAIT, MAX_FILE, MAX_KEYS, MAX_VALUE};
use crate::paths;

/// The file in a plugin's store.
pub const FILE: &str = "client-values.json";
/// The file's shape (`"v"`).
const FORMAT: u64 = 1;

type Clients = BTreeMap<String, Map<String, Value>>;

fn stores() -> PathBuf {
    paths::state_dir().join("plugins")
}

fn file(plugin: &str) -> PathBuf {
    data_dir(plugin).join(FILE)
}

fn lock_file(plugin: &str) -> PathBuf {
    stores().join(".locks").join(format!("{plugin}.clients.lock"))
}

fn lock(plugin: &str) -> Result<crate::app::FileLock> {
    let p = lock_file(plugin);
    std::fs::create_dir_all(p.parent().unwrap_or(&stores()))?;
    super::wait_lock_for(&p, &format!("{plugin}'s client values"), LOCK_WAIT)
}

/// A plugin removed with its data: its lock goes too (the file goes with its folder).
pub fn forget(plugin: &str) {
    let p = lock_file(plugin);
    if let Ok(Some(l)) = crate::app::FileLock::try_take(&p) {
        drop(l);
        let _ = std::fs::remove_file(&p);
    }
}

/// A client there is (an account: one with a page of its own).
fn exists(client: &str) -> bool {
    crate::projects::clients().is_ok_and(|cs| cs.iter().any(|c| c.id == client))
}

fn parse(text: &str) -> Result<Clients> {
    let v: Value = serde_json::from_str(text)?;
    if v["v"].as_u64() != Some(FORMAT) {
        bail!("it isn't {{\"v\": {FORMAT}, \"clients\": …}}");
    }
    let Some(Value::Object(cs)) = v.get("clients") else { bail!("it has no “clients”") };
    Ok(cs.iter().map(|(id, vs)| (id.clone(), vs.as_object().cloned().unwrap_or_default())).collect())
}

/// Its values: a missing file is none; one that can't be read is an error.
fn read(plugin: &str) -> Result<Clients> {
    let p = file(plugin);
    match std::fs::read_to_string(&p) {
        Ok(t) => parse(&t).with_context(|| format!("{} isn't readable", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Clients::new()),
        Err(e) => Err(anyhow!("reading {}: {e}", p.display())),
    }
}

/// The same, to change it (under its lock): a file that isn't its shape is moved aside (`client-values.json.bad-<time>`,
/// logged) and its values start empty — never written over.
fn read_to_change(plugin: &str) -> Result<Clients> {
    let p = file(plugin);
    match read(plugin) {
        Err(e) if p.is_file() && std::fs::read_to_string(&p).is_ok() => {
            let aside = paths::unused(&data_dir(plugin), &format!("{FILE}.bad-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")), "");
            std::fs::rename(&p, &aside).with_context(|| format!("moving {} aside", p.display()))?;
            eprintln!("echoctl: plugin {plugin}: {e:#} — moved aside to {}", aside.display());
            Ok(Clients::new())
        }
        r => r,
    }
}

fn text(all: &Clients) -> Result<String> {
    Ok(serde_json::to_string_pretty(&json!({"v": FORMAT, "clients": all}))?)
}

/// Written (under its lock), 4 MB at most — unless it gets no bigger.
fn write(plugin: &str, all: &Clients) -> Result<()> {
    let text = text(all)?;
    if text.len() > MAX_FILE && text.len() > read(plugin).ok().and_then(|t| self::text(&t).ok()).map_or(0, |t| t.len()) {
        bail!("a plugin's client values are {} MB at most altogether", MAX_FILE / (1024 * 1024));
    }
    std::fs::create_dir_all(data_dir(plugin))?;
    paths::write_atomic(&file(plugin), text.as_bytes())?;
    Ok(())
}

fn check_key(key: &str) -> Result<()> {
    if key.is_empty() || key.chars().count() > 80 {
        bail!("a client value needs a key of 1–80 characters");
    }
    if key.contains('.') {
        bail!("a client value's key has no dot (“{key}”): values per client are the plugin's own");
    }
    Ok(())
}

/// Its value `key` for `client` (null: none, or the client is gone).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn get(plugin: &str, client: &str, key: &str) -> Result<Value> {
    check_key(key)?;
    if !exists(client) {
        return Ok(Value::Null);
    }
    Ok(read(plugin)?.remove(client).and_then(|mut vs| vs.remove(key)).unwrap_or(Value::Null))
}

/// Keep `value` as its `key` for `client` (null removes it): a client there is, a value of 16 KB at most, 64 keys a
/// client at most, the whole file 4 MB at most.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn set(plugin: &str, client: &str, key: &str, value: &Value) -> Result<()> {
    check_key(key)?;
    if value.to_string().len() > MAX_VALUE {
        bail!("a client value is 16 KB at most");
    }
    let _l = lock(plugin)?;
    // Inside the lock: a client removed meanwhile has dropped its values already, and gets none now.
    if !exists(client) {
        bail!("there is no client “{client}”");
    }
    let mut all = read_to_change(plugin)?;
    let mut vs = all.remove(client).unwrap_or_default();
    if value.is_null() {
        vs.remove(key);
    } else {
        if !vs.contains_key(key) && vs.len() >= MAX_KEYS {
            bail!("a client keeps {MAX_KEYS} values per plugin at most");
        }
        vs.insert(key.to_string(), value.clone());
    }
    if !vs.is_empty() {
        all.insert(client.to_string(), vs);
    }
    write(plugin, &all)
}

/// The clients that have a value `key`, with it: `[{client, value}]` — a client that is gone left out.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn list(plugin: &str, key: &str) -> Result<Value> {
    check_key(key)?;
    let all = read(plugin)?;
    let ids: Vec<String> = crate::projects::clients()?.into_iter().map(|c| c.id).collect();
    Ok(Value::Array(
        all.into_iter().filter(|(c, _)| ids.contains(c)).filter_map(|(c, mut vs)| vs.remove(key).map(|v| json!({"client": c, "value": v}))).collect(),
    ))
}

/// A client was removed: every plugin's values for it go (each under its lock).
pub fn removed(client: &str) {
    let mut names: Vec<String> = std::fs::read_dir(stores())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.') && !n.starts_with('@'))
        .collect();
    names.sort();
    for plugin in names {
        if !file(&plugin).exists() {
            continue;
        }
        let dropped = lock(&plugin).and_then(|_l| {
            let mut all = read_to_change(&plugin)?;
            if all.remove(client).is_some() {
                write(&plugin, &all)?;
            }
            Ok(())
        });
        if let Err(e) = dropped {
            eprintln!("echoctl: plugin {plugin}: its values for client {client}: {e:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugins_values_per_client_are_kept_listed_and_follow_the_client() {
        let (_env, _root) = paths::testenv::folders("plugin-client-values");
        let acme = crate::projects::add_client("Acme", &[], false).unwrap().id;
        let globex = crate::projects::add_client("Globex", &[], false).unwrap().id;
        set("kb", &acme, "vault", &json!("Acme vault")).unwrap();
        assert_eq!(get("kb", &acme, "vault").unwrap(), json!("Acme vault"));
        assert_eq!(get("kb", &globex, "vault").unwrap(), Value::Null, "Acme's only");
        assert_eq!(get("other", &acme, "vault").unwrap(), Value::Null, "the plugin's own");
        assert_eq!(list("kb", "vault").unwrap(), json!([{"client": acme, "value": "Acme vault"}]));
        // Refused: no such client, an organisation that isn't a client, a key with a dot (its own keys only).
        assert!(set("kb", "nope", "vault", &json!(1)).unwrap_err().to_string().contains("no client “nope”"));
        let org = crate::projects::add_org("Initech", &[], false, false).unwrap().id;
        assert!(set("kb", &org, "vault", &json!(1)).unwrap_err().to_string().contains("no client"));
        assert!(set("kb", &acme, "acme.vault", &json!(1)).unwrap_err().to_string().contains("no dot"));
        // Renamed: its id stays, and so do its values; Echo's own fields untouched by a plugin's set.
        crate::projects::update_client(&acme, |c| {
            c.name = "Acme Corp".into();
            Ok(())
        })
        .unwrap();
        assert_eq!(get("kb", &acme, "vault").unwrap(), json!("Acme vault"), "a rename keeps them");
        assert_eq!(crate::projects::clients().unwrap().iter().find(|c| c.id == acme).unwrap().name, "Acme Corp");
        // null removes it.
        set("kb", &globex, "vault", &json!("G")).unwrap();
        set("kb", &globex, "vault", &Value::Null).unwrap();
        assert_eq!(get("kb", &globex, "vault").unwrap(), Value::Null);
        assert!(!read("kb").unwrap().contains_key(&globex), "no empty entry left");
        // Removed: its values go, from every plugin; a client under the same id later inherits nothing.
        set("other", &acme, "x", &json!(1)).unwrap();
        crate::projects::remove_client(&acme).unwrap();
        assert!(!read("kb").unwrap().contains_key(&acme) && !read("other").unwrap().contains_key(&acme));
        assert_eq!(list("kb", "vault").unwrap(), json!([]));
        let again = crate::projects::add_client("Acme", &[], false).unwrap().id;
        assert_eq!(again, acme, "the id is used again");
        assert_eq!(get("kb", &acme, "vault").unwrap(), Value::Null, "not the old client's");
        let on_disk: Value = serde_json::from_str(&std::fs::read_to_string(file("kb")).unwrap()).unwrap();
        assert_eq!(on_disk, json!({"v": 1, "clients": {}}));
    }

    #[test]
    fn client_values_have_the_thread_values_limits() {
        let (_env, _root) = paths::testenv::folders("plugin-client-values-caps");
        let a = crate::projects::add_client("Acme", &[], false).unwrap().id;
        // A key of 1–80 characters; a value of 16 KB at most (exactly 16 KB kept).
        assert!(set("kb", &a, "", &json!(1)).unwrap_err().to_string().contains("1–80"));
        assert!(set("kb", &a, &"k".repeat(81), &json!(1)).unwrap_err().to_string().contains("1–80"));
        set("kb", &a, &"k".repeat(80), &json!(1)).unwrap();
        set("kb", &a, &"k".repeat(80), &Value::Null).unwrap();
        let exact = json!("x".repeat(MAX_VALUE - 2));
        set("kb", &a, "edge", &exact).unwrap();
        assert!(set("kb", &a, "edge", &json!("x".repeat(MAX_VALUE - 1))).unwrap_err().to_string().contains("16 KB"));
        set("kb", &a, "edge", &Value::Null).unwrap();
        // 64 keys a client; a key it has can still change.
        for i in 0..MAX_KEYS {
            set("kb", &a, &format!("k{i}"), &json!(i)).unwrap();
        }
        assert!(set("kb", &a, "one-more", &json!(1)).unwrap_err().to_string().contains("64 values"));
        set("kb", &a, "k0", &json!("changed")).unwrap();
        // The whole file: 4 MB at most.
        let b = crate::projects::add_client("Beta", &[], false).unwrap().id;
        let mut all = read("kb").unwrap();
        all.insert(b.clone(), Map::from_iter([("filler".to_string(), json!("x".repeat(MAX_FILE - 8 * 1024)))]));
        std::fs::write(file("kb"), text(&all).unwrap()).unwrap();
        assert!(set("kb", &b, "big", &json!("x".repeat(MAX_VALUE - 2))).unwrap_err().to_string().contains("4 MB"));
        set("kb", &b, "filler", &Value::Null).unwrap();
        // A file that isn't its shape: get and list say so; a change moves it aside, never writes over it.
        std::fs::write(file("kb"), "{\"half").unwrap();
        assert!(get("kb", &a, "k0").is_err() && list("kb", "k0").is_err());
        set("kb", &a, "k0", &json!(1)).unwrap();
        let aside: Vec<_> = std::fs::read_dir(data_dir("kb")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("client-values.json.bad-")).collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(get("kb", &a, "k0").unwrap(), json!(1));
        // Removed with its data: its lock goes too.
        assert!(lock_file("kb").exists());
        forget("kb");
        assert!(!lock_file("kb").exists());
    }
}

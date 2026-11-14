//! Values the plugins of one marketplace share: a key `<ns>.<name>`, `ns` the marketplace's declared name in lower case
//! (`EPAM` → `epam`). Per thread through `ctx.threads` (kept as `thread_values` of the store `@<ns>`, so they follow a
//! thread like a plugin's own), or one for the computer through `ctx.shared` (`@<ns>/shared.json`). Every plugin reads
//! any namespace; only the plugins of the marketplace source that owns it set it — the source of the earliest installed
//! plugin whose marketplace declares that name (worked out from `plugins.json` each time: the owner's plugins removed,
//! the next source takes it).

use std::time::Duration;

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Map, Value};

use super::{catalog, data_dir, Registry};
use crate::paths;

/// One value, as JSON, at most this long; keys for the computer per namespace.
const MAX_VALUE: usize = 16 * 1024;
const MAX_KEYS: usize = 64;
const FILE: &str = "shared.json";
const LOCK_WAIT: Duration = Duration::from_secs(5);

/// A marketplace's declared name as a namespace: lower case, a space `-`, other characters left out.
pub fn namespace(name: &str) -> String {
    let s: String = name.trim().to_lowercase().chars().filter_map(|c| if c.is_ascii_alphanumeric() || c == '-' { Some(c) } else if c.is_whitespace() { Some('-') } else { None }).collect();
    s.trim_matches('-').to_string()
}

/// The store a shared key lives in (`@<ns>`), or none for a plugin's own key (no dot).
pub fn store_of(key: &str) -> Result<Option<String>> {
    let Some((ns, name)) = key.split_once('.') else { return Ok(None) };
    if key.chars().count() > 80 {
        bail!("a shared key is 80 characters at most");
    }
    if ns.is_empty() || name.is_empty() || namespace(ns) != ns {
        bail!("a shared key is <namespace>.<name>, the namespace a marketplace's name in lower case (“{key}”)");
    }
    Ok(Some(format!("@{ns}")))
}

/// The installed plugins with their marketplace's source and namespace, the earliest installed first (each
/// marketplace's catalog read once).
fn members() -> Vec<(String, String, String)> {
    let Ok(r) = Registry::load() else { return vec![] };
    let markets = r.all_markets();
    let mut names: std::collections::HashMap<String, Option<(String, String)>> = Default::default();
    let mut v: Vec<_> = r.installed.values().collect();
    let at = |t: &str| chrono::DateTime::parse_from_rfc3339(t).map(|d| d.timestamp_micros()).unwrap_or(i64::MAX);
    v.sort_by(|a, b| at(&a.installed_at).cmp(&at(&b.installed_at)).then(a.id.cmp(&b.id)));
    v.into_iter()
        .filter_map(|p| {
            let (source, ns) = names
                .entry(p.market.clone())
                .or_insert_with(|| {
                    let (m, _) = markets.iter().find(|(m, _)| m.name == p.market)?;
                    let declared = catalog(&p.market).map(|c| c.name).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| p.market.clone());
                    Some((m.source.clone(), namespace(&declared)))
                })
                .clone()?;
            Some((p.id.clone(), source, ns))
        })
        .collect()
}

/// Which source owns `ns`: the one it is bound to while a plugin of that source declaring it is installed, else the
/// source of the earliest installed plugin that declares it (none: nobody).
fn pick(bound: Option<&str>, all: &[(String, String, String)], ns: &str) -> Option<String> {
    if let Some(b) = bound.filter(|b| all.iter().any(|(_, s, n)| s == b && n == ns)) {
        return Some(b.to_string());
    }
    all.iter().find(|(_, _, n)| n == ns).map(|(_, s, _)| s.clone())
}

/// The namespaces' owners, kept so that an update (a new install time) or a partial removal never hands one over:
/// `<config dir>/plugins/namespaces.json`, `{ns: source}`.
fn bindings() -> std::path::PathBuf {
    super::plugins_dir().join("namespaces.json")
}

fn read_bindings() -> Map<String, Value> {
    std::fs::read(bindings()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The source that owns `ns` (bound now when it wasn't, or its owner's plugins are all gone).
fn owner(all: &[(String, String, String)], ns: &str) -> Option<String> {
    let was = read_bindings().get(ns).and_then(Value::as_str).map(str::to_owned);
    let now = pick(was.as_deref(), all, ns);
    if was != now {
        // Under the namespaces' own lock: two processes binding at once agree.
        let path = bindings();
        if let Ok(_l) = super::wait_lock_for(&path.with_extension("lock"), "the plugins' namespaces", LOCK_WAIT) {
            let mut b = read_bindings();
            let now = pick(b.get(ns).and_then(Value::as_str), all, ns);
            match &now {
                Some(src) => drop(b.insert(ns.to_string(), Value::String(src.clone()))),
                None => drop(b.remove(ns)),
            }
            let _ = serde_json::to_vec_pretty(&b).map(|t| paths::write_atomic(&path, &t));
            return now;
        }
    }
    now
}

/// Every namespace installed now bound to its owner: after an install and before a removal, so the owner is kept
/// whether or not a plugin read a shared value meanwhile.
pub fn bind_all() {
    let all = members();
    let mut seen = std::collections::BTreeSet::new();
    for (_, _, ns) in &all {
        if !ns.is_empty() && seen.insert(ns.clone()) {
            owner(&all, ns);
        }
    }
}

/// The namespaces bound to `source` that no installed plugin of it declares any more let go (its marketplace removed
/// with its plugins, #616); other sources' bindings are left as they are, and the shared values themselves stay.
pub fn release_namespaces(source: &str) {
    let path = bindings();
    let Ok(_l) = super::wait_lock_for(&path.with_extension("lock"), "the plugins' namespaces", LOCK_WAIT) else { return };
    let all = members();
    let mut b = read_bindings();
    let n = b.len();
    b.retain(|ns, src| src.as_str() != Some(source) || all.iter().any(|(_, s, x)| x == ns && s == source));
    if b.len() != n {
        let _ = serde_json::to_vec_pretty(&b).map(|t| paths::write_atomic(&path, &t));
    }
}

/// The plugins that share `ns` (those from its owner's source).
pub fn sharers(ns: &str) -> Vec<String> {
    let all = members();
    let Some(src) = owner(&all, ns) else { return vec![] };
    all.into_iter().filter(|(_, s, n)| *s == src && n == ns).map(|(id, _, _)| id).collect()
}

/// Some installed marketplace owns the store `@<ns>`: a namespace nobody owns reads as empty.
pub fn owned(store: &str) -> bool {
    owner(&members(), store.trim_start_matches('@')).is_some()
}

/// `plugin` may set values of the store `@<ns>`: it comes from the source that owns `ns`.
pub fn may_set(plugin: &str, store: &str) -> Result<()> {
    let ns = store.trim_start_matches('@');
    let all = members();
    let Some(src) = owner(&all, ns) else { bail!("no installed marketplace is named “{ns}”: its values can't be set") };
    if all.iter().any(|(id, s, n)| id == plugin && *s == src && n == ns) {
        return Ok(());
    }
    Err(anyhow!("only the plugins of the marketplace that owns “{ns}” ({src}) set its values"))
}

fn lock(store: &str) -> Result<crate::app::FileLock> {
    let p = paths::state_dir().join("plugins").join(".locks").join(format!("{store}.shared.lock"));
    std::fs::create_dir_all(p.parent().expect("a folder"))?;
    super::wait_lock_for(&p, &format!("{store}'s shared values"), LOCK_WAIT)
}

fn read(store: &str) -> Result<Map<String, Value>> {
    let p = data_dir(store).join(FILE);
    match std::fs::read_to_string(&p) {
        Ok(t) => match serde_json::from_str(&t) {
            Ok(Value::Object(o)) => Ok(o),
            _ => bail!("{} isn't a JSON object", p.display()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
        Err(e) => Err(anyhow!("reading {}: {e}", p.display())),
    }
}

/// `ctx.shared.get(key)`: the value for the computer (null: none).
pub fn get(key: &str) -> Result<Value> {
    let Some(store) = store_of(key)? else { bail!("ctx.shared takes a key <namespace>.<name>") };
    if !owned(&store) {
        return Ok(Value::Null);
    }
    Ok(read(&store)?.remove(key).unwrap_or(Value::Null))
}

/// `ctx.shared.set(key, value)` by `plugin` (null removes it): its namespace's, 16 KB, 64 keys at most.
pub fn set(plugin: &str, key: &str, value: &Value) -> Result<()> {
    let Some(store) = store_of(key)? else { bail!("ctx.shared takes a key <namespace>.<name>") };
    if value.to_string().len() > MAX_VALUE {
        bail!("a shared value is 16 KB at most");
    }
    may_set(plugin, &store)?;
    let _l = lock(&store)?;
    let mut all = match read(&store) {
        Ok(a) => a,
        // Not JSON: moved aside, never written over.
        Err(e) if std::fs::read_to_string(data_dir(&store).join(FILE)).is_ok() => {
            let aside = paths::unused(&data_dir(&store), &format!("{FILE}.bad-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")), "");
            std::fs::rename(data_dir(&store).join(FILE), &aside)?;
            eprintln!("echoctl: {e:#} — moved aside to {}", aside.display());
            Map::new()
        }
        Err(e) => return Err(e),
    };
    if value.is_null() {
        all.remove(key);
    } else {
        if !all.contains_key(key) && all.len() >= MAX_KEYS {
            bail!("a namespace keeps {MAX_KEYS} shared values at most");
        }
        all.insert(key.to_string(), value.clone());
    }
    std::fs::create_dir_all(data_dir(&store))?;
    paths::write_atomic(&data_dir(&store).join(FILE), serde_json::to_string_pretty(&all)?.as_bytes())?;
    Ok(())
}

/// `shared_changed` (#612): a value `by` set under `key` (for the computer, or for `thread`) changed — told to the other
/// plugins sharing its namespace that hear it (`uses.events.on`), `{namespace, key, thread?, by}`; from another
/// process, through the app's queue (`thread_values::tell`). Nobody else shares it: nothing is told.
pub fn tell_changed(by: &str, key: &str, thread: Option<&str>) {
    let Ok(Some(store)) = store_of(key) else { return };
    let ns = store.trim_start_matches('@').to_string();
    let to: Map<String, Value> = sharers(&ns).into_iter().filter(|p| p != by).map(|p| (p, json!({}))).collect();
    if to.is_empty() {
        return;
    }
    let mut payload = json!({"namespace": ns, "key": key, "by": by});
    if let Some(t) = thread {
        payload["thread"] = json!(t);
    }
    super::thread_values::tell("shared_changed", &payload, &to);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_namespace_stays_with_its_source_while_one_of_its_plugins_is_installed() {
        let m = |id: &str, src: &str, ns: &str| (id.to_string(), src.to_string(), ns.to_string());
        // Earliest first: b (Acme), c (a copy also calling itself Acme), a (Acme, updated: a later install time).
        let all = vec![m("c", "copy", "acme"), m("b", "acme-src", "acme"), m("a", "acme-src", "acme"), m("d", "other", "other")];
        assert_eq!(pick(None, &all, "acme").as_deref(), Some("copy"), "unbound: the earliest");
        assert_eq!(pick(Some("acme-src"), &all, "acme").as_deref(), Some("acme-src"), "bound: kept whatever the order");
        let left = vec![m("c", "copy", "acme"), m("d", "other", "other")];
        assert_eq!(pick(Some("acme-src"), &left, "acme").as_deref(), Some("copy"), "its plugins all gone: the next");
        assert_eq!(pick(Some("acme-src"), &left, "nobody"), None);
        assert_eq!(pick(Some("other"), &all, "acme").as_deref(), Some("copy"), "a source that doesn't declare it isn't bound to it");
    }

    #[test]
    fn a_marketplace_name_is_its_namespace_and_a_dotted_key_its_store() {
        assert_eq!(namespace("EPAM"), "epam");
        assert_eq!(namespace(" My plugins "), "my-plugins");
        assert_eq!(namespace("Acme & Co."), "acme--co");
        assert_eq!(store_of("repos").unwrap(), None, "no dot: the plugin's own");
        assert_eq!(store_of("epam.project_codes").unwrap().as_deref(), Some("@epam"));
        assert_eq!(store_of("my-plugins.a.b").unwrap().as_deref(), Some("@my-plugins"));
        assert!(store_of(&format!("epam.{}", "x".repeat(80))).is_err(), "80 characters at most");
        for bad in ["EPAM.x", ".x", "epam.", "e pam.x", "../x.y"] {
            assert!(store_of(bad).is_err(), "{bad}");
        }
    }
}

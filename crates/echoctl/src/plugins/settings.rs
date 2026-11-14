//! A plugin's settings and its folder in the data folder: what the user sets (secrets apart, addresses only by the
//! user), read by its host half.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use super::*;
use crate::paths;

/// A plugin's own folder in the data folder (its settings and state).
pub fn data_dir(id: &str) -> PathBuf {
    paths::state_dir().join("plugins").join(id)
}

pub(super) fn secrets_file(id: &str) -> PathBuf {
    plugins_dir().join("secrets").join(format!("{id}.json"))
}

/// The addresses the user listed (`uses.network.hosts_from`), outside the plugin's store: nothing the plugin writes
/// can reach this file, whatever name it uses.
pub(super) fn addresses_file(id: &str) -> PathBuf {
    plugins_dir().join("addresses").join(format!("{id}.json"))
}

/// What the user listed in a `hosts_from` setting, read only from where `set_setting` keeps it.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn listed_addresses(id: &str, key: &str) -> Option<String> {
    read_map(&addresses_file(id)).remove(key)?.as_str().map(str::to_owned)
}

pub(super) fn read_map(path: &Path) -> serde_json::Map<String, Value> {
    fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub(super) fn installed_manifest(id: &str) -> Result<Manifest> {
    let p = Registry::load()?.installed.get(id).cloned().ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
    read_manifest(&plugin_dir(&p))
}

/// The settings as the page shows them: each definition with its value (a secret only as set / not).
pub fn settings_view(id: &str) -> Result<Vec<Value>> {
    let m = installed_manifest(id)?;
    let mut vals = read_map(&data_dir(id).join("settings.json"));
    let listed = address_settings(&m.uses);
    vals.retain(|k, _| !listed.contains(k));
    vals.extend(read_map(&addresses_file(id)));
    let secrets = read_map(&secrets_file(id));
    Ok(m.settings
        .iter()
        .map(|s| {
            let mut v = serde_json::to_value(s).unwrap_or_default();
            if s.kind == "secret" {
                v["set"] = secrets.contains_key(&s.key).into();
            } else {
                v["value"] = vals.get(&s.key).cloned().or_else(|| s.default.clone()).unwrap_or(Value::Null);
            }
            v
        })
        .collect())
}

/// A setting's value (secrets included), for the capabilities (`jshost`: only with `js`).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn setting(id: &str, key: &str) -> Option<Value> {
    read_map(&addresses_file(id)).remove(key).or_else(|| read_map(&secrets_file(id)).remove(key)).or_else(|| read_map(&data_dir(id).join("settings.json")).remove(key))
}

/// When the user last said no to a plugin's addresses from the window: for a minute after, the page's requests for it
/// are refused without a dialog, so a page can't keep asking until the user gives in.
pub(super) static REFUSED: Mutex<BTreeMap<String, std::time::Instant>> = Mutex::new(BTreeMap::new());

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(super) fn refused_lately(id: &str) -> bool {
    REFUSED.lock().unwrap_or_else(|e| e.into_inner()).get(id).is_some_and(|t| t.elapsed().as_secs() < 60)
}

/// For a setting from the window: the plugin's name and the addresses to allow, when `value` lists addresses it may
/// reach (checked first, so a wrong one is refused at once); `None` for anything else or an empty list (it narrows).
/// The list as saved is asked about too: a check against the saved file would race with a save under way.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(super) fn addresses_to_allow(id: &str, key: &str, value: &Value) -> Result<Option<(String, Vec<String>)>> {
    let m = installed_manifest(id)?;
    let text = value.as_str().unwrap_or("").trim();
    if !address_settings(&m.uses).iter().any(|k| k == key) || text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > 2000 {
        bail!("an address list is 2000 characters at most");
    }
    let label = m.settings.iter().find(|s| s.key == key).map_or(key, |s| s.label.as_str());
    addresses::parse_list(text).with_context(|| format!("“{label}” isn't an address the plugin may reach"))?;
    let name: String = m.name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    Ok(Some((name, text.split([',', ' ', '\n', '\r', '\t']).filter(|p| !p.is_empty()).map(str::to_owned).collect())))
}

/// The user's yes, in a dialog of the system's, to `name` reaching `list`. Development builds and tests:
/// `ECHOCTL_ALLOW_ADDRESSES=yes|no` answers instead (never in a release build).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(super) fn allow_addresses(name: &str, list: &[String]) -> bool {
    #[cfg(debug_assertions)]
    if let Ok(a) = std::env::var("ECHOCTL_ALLOW_ADDRESSES") {
        return a == "yes";
    }
    let msg = format!("Let {name} reach these addresses?\n\n{}\n\nAllow only if you entered them yourself.", list.join("\n"));
    crate::app::ask(&msg, &["Don't allow", "Allow"], "Don't allow").as_deref() == Some("Allow")
}

/// The settings the manifest lists in `uses.network.hosts_from`: they hold the addresses the user lets it reach.
pub fn address_settings(uses: &serde_json::Map<String, Value>) -> Vec<String> {
    uses.get("network").and_then(|n| n["hosts_from"].as_array()).into_iter().flatten().filter_map(|k| k.as_str().map(str::to_owned)).collect()
}

/// Save one setting, checked against its definition; an empty secret forgets it.
pub fn set_setting(id: &str, key: &str, value: &Value) -> Result<()> {
    let m = installed_manifest(id)?;
    let def = m.settings.iter().find(|s| s.key == key).ok_or_else(|| anyhow!("{} has no setting “{key}”", m.name))?;
    let value = match (def.kind.as_str(), value) {
        ("bool", Value::Bool(_)) => value.clone(),
        ("bool", Value::String(s)) if s == "true" || s == "false" => Value::Bool(s == "true"),
        ("string" | "folder" | "secret", Value::String(s)) if s.chars().count() <= 2000 => Value::String(s.trim().to_string()),
        _ => bail!("“{}” takes a {}", def.label, def.kind),
    };
    if def.kind == "folder" && value.as_str().is_some_and(|s| !s.is_empty() && !Path::new(s).is_dir()) {
        bail!("{} isn't a folder", value.as_str().unwrap_or(""));
    }
    let is_address = address_settings(&m.uses).iter().any(|k| k == key);
    if is_address {
        addresses::parse_list(value.as_str().unwrap_or("")).with_context(|| format!("“{}” isn't an address the plugin may reach", def.label))?;
    }
    let _lock = wait_lock(&paths::config_dir().join("plugins.lock"))?;
    if is_address {
        let f = addresses_file(id);
        fs::create_dir_all(f.parent().unwrap())?;
        let mut all = read_map(&f);
        all.insert(key.into(), value);
        paths::write_atomic(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    } else if def.kind == "secret" {
        let f = secrets_file(id);
        fs::create_dir_all(f.parent().unwrap())?;
        let mut all = read_map(&f);
        if value.as_str().is_some_and(str::is_empty) {
            all.remove(key);
        } else {
            all.insert(key.into(), value);
        }
        write_private(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    } else {
        let f = data_dir(id).join("settings.json");
        fs::create_dir_all(f.parent().unwrap())?;
        let mut all = read_map(&f);
        all.insert(key.into(), value);
        paths::write_atomic(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    }
    Ok(())
}

/// Written atomically and readable only by the user (Unix: 0600 from the start).
pub(super) fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut o = fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    std::io::Write::write_all(&mut o.open(&tmp)?, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

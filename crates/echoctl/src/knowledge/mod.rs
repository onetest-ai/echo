//! Knowledge: the user's vaults — folders of Markdown they own (an Obsidian vault, a git repository of notes, any
//! folder) — connected as they are, read, edited, graphed, and given the minutes of their calls (spec
//! `tasks/knowledge/spec.md`). Connecting writes nothing into a vault; Echo's index of one lives in Echo's data folder.
//!
//! The list of vaults is per data folder: `<home>/.echo/knowledge/vaults.json`, changed under a lock, re-read first and
//! written atomically.

pub mod activity;
pub mod api;
pub mod catalog;
pub mod checks;
pub mod detect;
pub mod diff;
pub mod evidence;
pub mod export;
pub mod fm;
pub mod git;
pub mod links;
pub mod page;
pub mod scan;
pub mod starter;

#[cfg(test)]
mod export_tests;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod page_tests;

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::paths;
pub use detect::Detected;

/// When a vault takes a call's minutes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinutesRule {
    /// `auto` (after the minutes are written), `ask` (only when the user asks) or `off`.
    pub mode: String,
    /// `all` (calls and memos), `calls` (no memos) or `some` (the calls of `clients` / `threads`).
    pub scope: String,
    #[serde(default)]
    pub clients: Vec<String>,
    #[serde(default)]
    pub threads: Vec<String>,
    /// The tag a meeting note gets (default: the taxonomy's first `meeting…` tag, else `meeting`).
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub transcript: bool,
    #[serde(default)]
    pub chat: bool,
}

impl Default for MinutesRule {
    fn default() -> Self {
        MinutesRule { mode: "auto".into(), scope: "all".into(), clients: vec![], threads: vec![], tag: None, transcript: false, chat: false }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRule {
    /// Commit what Echo writes (off for a connected vault, on for one Echo made).
    #[serde(default)]
    pub commit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vault {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    /// The AI profile that reads it (its Ask sessions); calls on other profiles are never saved here by themselves.
    pub profile: String,
    pub detected: Detected,
    /// The user's corrections of what detection found: `pages`, `index`, `notes`, `naming`, `templates`, `sources`.
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(default)]
    pub minutes: MinutesRule,
    #[serde(default)]
    pub git: GitRule,
    /// `twin@1` for a vault made from Echo's starter.
    #[serde(default)]
    pub template: Option<String>,
    pub added_at: String,
}

/// The keys a user may override.
pub const OVERRIDES: [&str; 6] = ["pages", "index", "notes", "naming", "templates", "sources"];

impl Vault {
    /// A structural path, relative to the root (`""` = the root): the user's override, else what detection found.
    pub fn path(&self, key: &str) -> Option<String> {
        if let Some(v) = self.overrides.get(key) {
            return Some(v.clone()).filter(|v| key == "pages" || !v.is_empty());
        }
        let d = &self.detected;
        match key {
            "pages" => Some(d.pages.clone()),
            "index" => d.index.clone(),
            "notes" => d.notes.clone(),
            "naming" => Some(d.naming.clone()),
            "templates" => d.templates.clone(),
            "sources" => d.sources.clone(),
            _ => None,
        }
    }

    /// The folder holding the pages.
    pub fn pages_dir(&self) -> PathBuf {
        join(&self.root, &self.path("pages").unwrap_or_default())
    }
}

/// `rel` (with `/`) under `root`.
pub fn join(root: &Path, rel: &str) -> PathBuf {
    rel.split('/').filter(|s| !s.is_empty()).fold(root.to_path_buf(), |p, s| p.join(s))
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Registry {
    #[serde(default)]
    vaults: Vec<Vault>,
}

fn dir() -> PathBuf {
    paths::state_dir().join("knowledge")
}

fn registry_file() -> PathBuf {
    dir().join("vaults.json")
}

/// Echo's own files for vault `id` (its index of the pages, state); never inside the vault.
pub fn cache_dir(id: &str) -> PathBuf {
    dir().join(id)
}

fn load() -> Result<Registry> {
    match std::fs::read(registry_file()) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| anyhow!("{}: {e}", registry_file().display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry::default()),
        Err(e) => Err(e.into()),
    }
}

/// Change the list under its lock, re-read first, so two writers never undo each other.
fn update<T>(f: impl FnOnce(&mut Registry) -> Result<T>) -> Result<T> {
    std::fs::create_dir_all(dir())?;
    let lock = dir().join("vaults.lock");
    let t = std::time::Instant::now();
    let _held = loop {
        if let Some(l) = crate::app::FileLock::try_take(&lock)? {
            break l;
        }
        if t.elapsed() > std::time::Duration::from_secs(30) {
            bail!("another Echo is changing your vaults — try again in a moment");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let mut r = load()?;
    let out = f(&mut r)?;
    paths::write_atomic(&registry_file(), serde_json::to_string_pretty(&r)?.as_bytes())?;
    Ok(out)
}

/// A folder in canonical form (no `\\?\` on Windows).
fn canonical(p: &Path) -> Result<PathBuf> {
    dunce::canonicalize(p).map_err(|e| anyhow!("{}: {e}", p.display()))
}

/// Why `root` can't be connected beside `existing`, if it can't.
fn refuse_folder(root: &Path, existing: &[Vault]) -> Result<()> {
    let home = canonical(&paths::home()).unwrap_or_else(|_| paths::home());
    if root == home || home.starts_with(root) {
        bail!("Echo's data folder can't be a vault — choose a folder of notes");
    }
    if root.parent().is_none() || dirs_home().is_some_and(|h| h == root) {
        bail!("Choose a folder of notes, not a whole disk or home folder");
    }
    for v in existing {
        let other = canonical(&v.root).unwrap_or_else(|_| v.root.clone());
        if other == root {
            bail!("“{}” is already connected", v.name);
        }
        if root.starts_with(&other) {
            bail!("This folder is inside “{}”, which is already connected", v.name);
        }
        if other.starts_with(root) {
            bail!("This folder holds “{}”, which is already connected", v.name);
        }
    }
    Ok(())
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).and_then(|h| dunce::canonicalize(h).ok())
}

fn unique_id(name: &str, taken: &[Vault]) -> String {
    let base = crate::recorder::slugify(name);
    let base = if base.is_empty() { "vault".to_string() } else { base };
    let free = |id: &str| !taken.iter().any(|v| v.id == id);
    if free(&base) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|id| free(id)).unwrap_or(base)
}

/// The vault's folder name, as a name for it.
fn folder_name(root: &Path) -> String {
    root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Vault".into())
}

const MODES: [&str; 3] = ["auto", "ask", "off"];
const SCOPES: [&str; 3] = ["all", "calls", "some"];

fn parse_bool(v: &str) -> Result<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        _ => bail!("“{v}”: on or off"),
    }
}

/// A path the user gave for `key`, checked: relative, inside the vault, there, of the right kind.
fn check_override(root: &Path, key: &str, value: &str) -> Result<String> {
    let v = value.trim().trim_matches('/').replace('\\', "/");
    if key == "naming" {
        if v != detect::DATE_SLUG && v != detect::DATE_TITLE {
            bail!("naming is {} or {}", detect::DATE_SLUG, detect::DATE_TITLE);
        }
        return Ok(v);
    }
    if v.split('/').any(|c| c == ".." || c == ".") || Path::new(&v).is_absolute() || v.contains(':') {
        bail!("“{value}” must be a path inside the vault");
    }
    let p = join(root, &v);
    let real = canonical(&p).map_err(|_| anyhow!("“{value}” isn't in this vault"))?;
    if !real.starts_with(canonical(root)?) {
        bail!("“{value}” leads out of the vault");
    }
    let want_file = key == "index";
    if want_file != real.is_file() {
        bail!("“{value}” isn't a {}", if want_file { "file" } else { "folder" });
    }
    Ok(v)
}

/// Changing a vault from one of Echo's own runs is refused (others' words must not reach this).
pub fn refuse_headless() -> Result<()> {
    if crate::notes::headless() {
        bail!("Echo's own runs can't change vaults");
    }
    Ok(())
}

pub fn list() -> Result<Vec<Vault>> {
    Ok(load()?.vaults)
}

pub fn get(id: &str) -> Result<Vault> {
    list()?.into_iter().find(|v| v.id == id).ok_or_else(|| anyhow!("no vault “{id}”"))
}

/// The folder still there.
pub fn available(v: &Vault) -> bool {
    v.root.is_dir()
}

#[derive(Debug, Default, Clone)]
pub struct AddOpts {
    pub name: Option<String>,
    pub profile: Option<String>,
    pub minutes: Option<String>,
    pub commit: Option<bool>,
    pub template: Option<String>,
}

/// Why `root` couldn't be connected now, if it couldn't (nothing changes).
pub fn refuse_add(root: &Path) -> Result<()> {
    let root = canonical(root)?;
    refuse_folder(&root, &list()?)
}

/// The same refusals for a folder that may not be there yet (a new vault's): judged where it will be.
pub fn refuse_new(root: &Path) -> Result<()> {
    let there = root.ancestors().find(|p| p.exists()).ok_or_else(|| anyhow!("{} has no folder above it", root.display()))?;
    let rest = root.strip_prefix(there).unwrap_or(Path::new(""));
    refuse_folder(&canonical(there)?.join(rest), &list()?)
}

/// The Obsidian vaults plugin's folders not connected as vaults yet.
pub fn plugin_vaults_waiting() -> Vec<PathBuf> {
    let have = list().unwrap_or_default();
    crate::grants::folders("obsidian").into_iter().filter(|f| refuse_folder(f, &have).is_ok()).collect()
}

/// Connect the Obsidian vaults plugin's folders (read from its grants; nothing of the plugin changes). How many.
pub fn import_plugin_vaults() -> Result<usize> {
    let mut n = 0;
    for f in plugin_vaults_waiting() {
        if add(&f, AddOpts { minutes: Some("ask".into()), ..Default::default() }).is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

/// Connect `root` as it is: detection only, nothing written into it.
pub fn add(root: &Path, opts: AddOpts) -> Result<Vault> {
    refuse_headless()?;
    if !root.is_dir() {
        bail!("{} isn't a folder", root.display());
    }
    let root = canonical(root)?;
    let detected = detect::detect(&root)?;
    if let Some(m) = &opts.minutes {
        if !MODES.contains(&m.as_str()) {
            bail!("minutes: auto, ask or off");
        }
    }
    let name = opts.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| folder_name(&root));
    let profile = opts.profile.filter(|p| !p.is_empty()).unwrap_or_else(crate::ai::default_id);
    update(|r| {
        refuse_folder(&root, &r.vaults)?;
        let v = Vault {
            id: unique_id(&name, &r.vaults),
            name,
            root,
            profile,
            detected,
            overrides: BTreeMap::new(),
            minutes: MinutesRule { mode: opts.minutes.unwrap_or_else(|| "auto".into()), ..Default::default() },
            git: GitRule { commit: opts.commit.unwrap_or(false) },
            template: opts.template,
            added_at: chrono::Local::now().to_rfc3339(),
        };
        r.vaults.push(v.clone());
        Ok(v)
    })
}

/// Change one setting of a vault (`name`, `profile`, `minutes`, `scope`, `clients`, `threads`, `tag`, `transcript`,
/// `chat`, `commit`, or an override of `pages` / `index` / `notes` / `naming` / `templates` / `sources`; `""` takes an
/// override back).
pub fn set(id: &str, key: &str, value: &str) -> Result<Vault> {
    refuse_headless()?;
    update(|r| {
        let v = r.vaults.iter_mut().find(|v| v.id == id).ok_or_else(|| anyhow!("no vault “{id}”"))?;
        let list = |s: &str| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>();
        match key {
            "name" => {
                let n = value.trim();
                if n.is_empty() {
                    bail!("a vault needs a name");
                }
                v.name = n.chars().take(80).collect();
            }
            "profile" => {
                if crate::ai::Settings::load().and_then(|s| s.get(value)).is_err() {
                    bail!("no AI profile “{value}”");
                }
                v.profile = value.to_string();
            }
            "minutes" if MODES.contains(&value) => v.minutes.mode = value.into(),
            "minutes" => bail!("minutes: auto, ask or off"),
            "scope" if SCOPES.contains(&value) => v.minutes.scope = value.into(),
            "scope" => bail!("scope: all, calls or some"),
            "clients" => v.minutes.clients = list(value),
            "threads" => v.minutes.threads = list(value),
            "tag" => v.minutes.tag = Some(value.trim().trim_start_matches('#').to_string()).filter(|t| !t.is_empty()),
            "transcript" => v.minutes.transcript = parse_bool(value)?,
            "chat" => v.minutes.chat = parse_bool(value)?,
            "commit" => v.git.commit = parse_bool(value)?,
            k if OVERRIDES.contains(&k) => {
                if value.trim().is_empty() {
                    v.overrides.remove(k);
                } else {
                    let checked = check_override(&v.root, k, value)?;
                    v.overrides.insert(k.to_string(), checked);
                }
            }
            other => bail!("no setting “{other}”"),
        }
        Ok(v.clone())
    })
}

/// Forget a vault: its entry and Echo's index of it. The folder is left as it is.
pub fn remove(id: &str) -> Result<()> {
    refuse_headless()?;
    update(|r| {
        let before = r.vaults.len();
        r.vaults.retain(|v| v.id != id);
        if r.vaults.len() == before {
            bail!("no vault “{id}”");
        }
        Ok(())
    })?;
    let _ = std::fs::remove_dir_all(cache_dir(id));
    Ok(())
}

/// Detect again; the user's overrides stay.
pub fn reread(id: &str) -> Result<Vault> {
    let root = get(id)?.root;
    let detected = detect::detect(&root)?;
    update(|r| {
        let v = r.vaults.iter_mut().find(|v| v.id == id).ok_or_else(|| anyhow!("no vault “{id}”"))?;
        v.detected = detected;
        Ok(v.clone())
    })
}

/// What every client shows of a vault.
pub fn vault_value(v: &Vault) -> Value {
    let ok = available(v);
    let git = if ok { git::info(&v.root) } else { None };
    let paths: BTreeMap<&str, Option<String>> = OVERRIDES.iter().map(|k| (*k, v.path(k))).collect();
    json!({
        "id": v.id,
        "name": v.name,
        "root": v.root,
        "available": ok,
        "profile": v.profile,
        "paths": paths,
        "detected": v.detected,
        "overrides": v.overrides,
        "minutes": v.minutes,
        "commit": v.git.commit,
        "git": git,
        "template": v.template,
        "added_at": v.added_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(name: &str) -> AddOpts {
        AddOpts { name: Some(name.into()), ..Default::default() }
    }

    #[test]
    fn a_vault_is_added_with_what_detection_found() {
        let (_env, root) = paths::testenv::folders("kn-add");
        let t = fixture::twin(&root.join("twin"));
        let v = add(&t, opts("My twin")).unwrap();
        assert_eq!(v.id, "my-twin");
        assert_eq!(v.detected.index.as_deref(), Some("knowledge/CATALOG.md"));
        assert_eq!(v.profile, crate::ai::default_id());
        assert_eq!(v.minutes.mode, "auto");
        assert!(!v.git.commit, "commits are off for a connected vault");
        assert_eq!(list().unwrap(), vec![v.clone()]);
        assert_eq!(get("my-twin").unwrap(), v);
        // A second vault with the same name gets its own id.
        let other = fixture::twin(&root.join("other"));
        assert_eq!(add(&other, opts("My twin")).unwrap().id, "my-twin-2");
    }

    #[test]
    fn connecting_writes_nothing_in_the_vault() {
        let (_env, root) = paths::testenv::folders("kn-nowrite");
        let t = fixture::twin(&root.join("twin"));
        let before = fixture::snapshot(&t);
        let v = add(&t, opts("Twin")).unwrap();
        reread(&v.id).unwrap();
        assert_eq!(fixture::snapshot(&t), before);
        assert!(registry_file().is_file());
    }

    #[test]
    fn nested_or_repeated_folders_are_refused() {
        let (_env, root) = paths::testenv::folders("kn-nested");
        let t = fixture::twin(&root.join("parent/twin"));
        add(&t, opts("Twin")).unwrap();
        let again = add(&t, opts("Again")).unwrap_err().to_string();
        let inside = add(&t.join("knowledge"), opts("Inside")).unwrap_err().to_string();
        let around = add(&root.join("parent"), opts("Around")).unwrap_err().to_string();
        assert!(again.contains("already connected"), "{again}");
        assert!(inside.contains("inside “Twin”"), "{inside}");
        assert!(around.contains("holds “Twin”"), "{around}");
        assert_eq!(list().unwrap().len(), 1);
        let data = add(&paths::home(), opts("Data")).unwrap_err().to_string();
        assert!(data.contains("data folder"), "{data}");
        let holding = add(&root, opts("Holding")).unwrap_err().to_string();
        assert!(holding.contains("data folder"), "a folder holding the data folder: {holding}");
        assert!(add(&root.join("nope"), opts("Nope")).is_err());
    }

    #[test]
    fn overrides_survive_read_again() {
        let (_env, root) = paths::testenv::folders("kn-override");
        let t = fixture::twin(&root.join("twin"));
        fixture::write(&t, "knowledge/README.md", "# readme\n");
        let v = add(&t, opts("Twin")).unwrap();
        let v2 = set(&v.id, "index", "knowledge/README.md").unwrap();
        assert_eq!(v2.path("index").as_deref(), Some("knowledge/README.md"));
        fixture::write(&t, "knowledge/notes/2026-02-01-d.md", "# d\n");
        let v3 = reread(&v.id).unwrap();
        assert_eq!(v3.path("index").as_deref(), Some("knowledge/README.md"));
        assert_eq!(v3.detected.notes_count, 4);
        // An override pointing out of the vault, or at nothing, is refused; "" takes it back.
        assert!(set(&v.id, "index", "../outside.md").is_err());
        assert!(set(&v.id, "notes", "knowledge/nope").is_err());
        assert_eq!(set(&v.id, "index", "").unwrap().path("index").as_deref(), Some("knowledge/CATALOG.md"));
    }

    #[test]
    fn settings_are_checked() {
        let (_env, root) = paths::testenv::folders("kn-set");
        let t = fixture::twin(&root.join("twin"));
        let v = add(&t, opts("Twin")).unwrap();
        assert_eq!(set(&v.id, "minutes", "ask").unwrap().minutes.mode, "ask");
        assert!(set(&v.id, "minutes", "sometimes").is_err());
        assert!(set(&v.id, "commit", "true").unwrap().git.commit);
        assert_eq!(set(&v.id, "threads", "checkout, promo").unwrap().minutes.threads, ["checkout", "promo"]);
        assert_eq!(set(&v.id, "name", "Second brain").unwrap().name, "Second brain");
        assert!(set(&v.id, "name", "  ").is_err());
        assert!(set(&v.id, "colour", "red").is_err());
    }

    #[test]
    fn a_missing_folder_is_shown_not_removed() {
        let (_env, root) = paths::testenv::folders("kn-missing");
        let t = fixture::twin(&root.join("twin"));
        let v = add(&t, opts("Twin")).unwrap();
        std::fs::rename(&t, root.join("moved")).unwrap();
        let listed = list().unwrap();
        assert_eq!(listed.len(), 1);
        assert!(!available(&listed[0]));
        assert_eq!(vault_value(&listed[0])["available"], false);
        std::fs::rename(root.join("moved"), &t).unwrap();
        assert!(available(&get(&v.id).unwrap()));
    }

    #[test]
    fn remove_forgets_and_keeps_the_folder() {
        let (_env, root) = paths::testenv::folders("kn-remove");
        let t = fixture::twin(&root.join("twin"));
        let before = fixture::snapshot(&t);
        let v = add(&t, opts("Twin")).unwrap();
        std::fs::create_dir_all(cache_dir(&v.id)).unwrap();
        remove(&v.id).unwrap();
        assert!(list().unwrap().is_empty());
        assert!(!cache_dir(&v.id).exists());
        assert_eq!(fixture::snapshot(&t), before);
    }

    #[test]
    fn headless_refuses_changes() {
        let (_env, root) = paths::testenv::folders("kn-headless");
        let t = fixture::twin(&root.join("twin"));
        std::env::set_var(crate::notes::HEADLESS, "1");
        let r = refuse_headless();
        std::env::remove_var(crate::notes::HEADLESS);
        assert!(r.is_err());
        assert!(add(&t, opts("Twin")).is_ok(), "outside a run it works");
    }
}

#[cfg(test)]
mod cli_shape {
    use super::*;

    #[test]
    fn vault_json_keeps_its_keys() {
        let (_env, root) = paths::testenv::folders("kn-json");
        let t = fixture::twin(&root.join("twin"));
        let v = add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        let j = vault_value(&v);
        for k in ["id", "name", "root", "available", "profile", "paths", "detected", "overrides", "minutes", "commit", "git", "template", "added_at"] {
            assert!(j.get(k).is_some(), "missing {k}");
        }
        assert_eq!(j["paths"]["index"], "knowledge/CATALOG.md");
        assert_eq!(j["paths"]["pages"], "knowledge");
        assert_eq!(j["minutes"]["mode"], "auto");
    }
}

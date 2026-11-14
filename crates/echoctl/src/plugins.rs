//! Echo plugins (`docs/plugins-design.md`, v4): packages that bring their own code — a host half
//! (`main`, run by `jshost.rs` inside Echo) and a client half (`client`, loaded into the window) — with a
//! manifest saying what of Echo they touch (`uses`), skills and settings. They come from
//! **marketplaces**: git repositories with `echo-marketplace.json`; ours (`onetest-ai/echo-plugins`) is
//! built in, the user adds any number of others (an enterprise's own catalog is one more).
//!
//! - `<config dir>/plugins.json` — the marketplaces added and the plugins installed (lock + re-read).
//! - `<config dir>/plugins/markets/<key>/` — each marketplace's last fetch (`catalog.json` + the files
//!   at that commit); `<config dir>/plugins/installed/<id>/<version>/` — what is installed, recorded with
//!   its commit and a hash of its files.
//! - `<home>/.echo/plugins/<id>/settings.json` — a plugin's settings, per data folder; its secrets in
//!   `<config dir>/plugins/secrets/<id>.json` (0600), never sent back to the page.
//!
//! Fetching: GitHub `owner/repo[@ref]` → the commit resolved, its tarball through `curl` (else `gh`),
//! unpacked by `tar`; any other git URL → `git clone --depth 1`; a local folder (plugin authors) is read
//! where it is. Every tree is checked before anything reads it: no links, size and count caps.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::paths;

pub mod addresses;
mod appfiles;
#[cfg_attr(not(feature = "js"), allow(unused_imports))]
pub use appfiles::{app_file_real, app_file_rel};
#[cfg(test)]
use appfiles::app_file;
mod capabilities;
pub use capabilities::*;
pub mod agent;
pub mod scripts;
pub mod shared;
mod settings;
pub use settings::*;
mod sources;
use sources::*;
pub(crate) use sources::copy_tree;
pub mod thread_values;
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub mod signin;

/// Our public marketplace, always listed (it can be hidden, not removed).
pub const BUILT_IN: &str = "onetest-ai/echo-plugins";
const BUILT_IN_NAME: &str = "onetest";
/// "My plugins": the user's own, the folder `plugins/` in their Echo data folder — a subfolder with a manifest a
/// plugin, its folder name its id, no catalog file. Always listed; never removed.
pub const MINE: &str = "my-plugins";

/// My plugins' folder (made when first asked for).
pub fn mine_dir() -> PathBuf {
    paths::home().join("plugins")
}
/// The marketplace's catalog file at the root of its repository.
const CATALOG: &str = "echo-marketplace.json";
/// A plugin's manifest at the root of its folder.
const MANIFEST: &str = "echo-plugin.json";
/// A marketplace refreshed by itself once its fetch is this old (window open, `refresh_stale`).
const STALE_SECS: i64 = 30 * 60;

/// Caps on what a fetch may unpack: a whole marketplace, one plugin.
const MARKET_LIMIT: Limit = Limit { files: 5000, bytes: 64 << 20 };
const PLUGIN_LIMIT: Limit = Limit { files: 400, bytes: 20 << 20 };

#[derive(Clone, Copy)]
struct Limit {
    files: usize,
    bytes: u64,
}

// ── manifest ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The Echo it needs: `>=0.7.0` (or a bare version, the same).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub echo: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub uses: serde_json::Map<String, Value>,
    #[serde(default)]
    pub settings: Vec<SettingDef>,
    /// The host half: a JavaScript module run inside Echo (`jshost.rs`), `export function apply(ctx)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    /// The client half: a JavaScript module loaded into the Echo window, `export function apply(ctx)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    /// Widgets it offers for Today (`plugin:<id>:<widget>` in the catalog), their data from its host half
    /// (`ctx.widgets.provide` / `set` / `clear`), drawn by Echo by their kind.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widgets: Vec<WidgetDef>,
    /// Its package skills: folders in its package (`skills/<name>`, Agent Skills format: `SKILL.md`, `scripts/`,
    /// `references/`) Echo copies into its Claude Code plugin for Echo's sessions (`grants::copy_package_skills`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    /// The hash of the skill folders its scripts are in (`scripts::check`): on the sheet, so a change asks again.
    /// Always Echo's: `read_manifest` sets it, whatever the file says (kept in the catalog it writes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts_digest: Option<String>,
    /// Its icon: an `.svg` in its folder (16 KB at most), shown only as an image (`<img>`, never inlined) — beside its
    /// work rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Its part for Claude in Echo: `{"summary": "<one line>"}`, with the folder `agent/` in its package (`agent.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<Value>,
    /// The hash of what its package tells Claude — its `agent/` folder and its package skills' folders: on the sheet,
    /// so a version changing any of it asks again. Always Echo's, as `scripts_digest`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_digest: Option<String>,
    /// The skills its package gives Claude in Echo (the manifest's `skills` and `agent/skills/`), by name. Always Echo's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skill_names: Vec<String>,
    /// What is wrong with its part for Claude (`agent/`, its package skills and their scripts), by a rule its installed
    /// copy may predate: the plugin still runs, Claude in Echo gets none of that part. Always Echo's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub part_problems: Vec<String>,
}

/// A problem with a plugin package: the file it is in (relative to the package) and what is wrong.
#[derive(Debug, Clone, Serialize)]
pub struct Problem {
    pub file: String,
    pub what: String,
}

/// A widget a plugin declares: Echo draws `kind` (`stat`, `stats`, `chart`, `list`, `status`, `card`, `table`) at the sizes it takes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetDef {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub about: String,
    pub kind: String,
    #[serde(default = "widget_sizes")]
    pub sizes: Vec<String>,
    #[serde(default = "widget_height")]
    pub height: String,
    /// The rows of Today's grid it may take: 1, 2 (`[1]` when not said).
    #[serde(default = "widget_heights")]
    pub heights: Vec<u8>,
    /// How often Echo asks its provider again while Today shows it, in seconds (60 at least; 60 when not said).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh: Option<u64>,
    /// What the user may set on it (edit mode's ⚙, `echoctl today set`); its provider gets them as `params`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDef>,
}

/// A widget's setting: `choice` (one of `options`), `bool`, `thread` (an open thread's id) or `client` (a client's id).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamDef {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

impl ParamDef {
    /// Whether `v` is a value this setting takes (a thread or client: one that exists).
    pub fn takes(&self, v: &Value) -> bool {
        match self.kind.as_str() {
            "choice" => v.as_str().is_some_and(|s| self.options.iter().any(|o| o == s)),
            "bool" => v.is_boolean(),
            "thread" => v.as_str().is_some_and(|s| crate::projects::list().iter().any(|p| p.id == s && !p.stage.closed())),
            "client" => v.as_str().is_some_and(|s| crate::projects::clients().is_ok_and(|cs| cs.iter().any(|c| c.id == s))),
            _ => false,
        }
    }
}

/// The widgets one plugin may declare.
const MAX_WIDGETS: usize = 24;

/// A widget's settings, checked: plain unique keys, labels, a type Echo knows, a choice's options, a default it takes.
pub fn check_params(widget: &str, params: &[ParamDef]) -> Result<()> {
    let mut keys = std::collections::HashSet::new();
    if params.len() > 6 {
        bail!("widget “{widget}” has more than 6 settings");
    }
    for p in params {
        if !valid_id(&p.key.replace('_', "-")) || !keys.insert(p.key.clone()) {
            bail!("widget “{widget}”: setting “{}” needs a plain, unique key", p.key);
        }
        if p.label.trim().is_empty() || p.label.chars().count() > 40 {
            bail!("widget “{widget}”: setting “{}” needs a label of 1–40 characters", p.key);
        }
        match p.kind.as_str() {
            "choice" if p.options.is_empty() || p.options.len() > 12 || p.options.iter().any(|o| o.is_empty() || o.chars().count() > 40) => {
                bail!("widget “{widget}”: setting “{}” is a choice of 1–12 options of 40 characters at most", p.key)
            }
            "choice" | "bool" | "thread" | "client" => {}
            k => bail!("widget “{widget}”: setting “{}” has a type Echo doesn't know ({k}; choice, bool, thread, client)", p.key),
        }
        if let Some(d) = &p.default {
            if matches!(p.kind.as_str(), "choice" | "bool") && !p.takes(d) {
                bail!("widget “{widget}”: setting “{}”'s default isn't one it takes", p.key);
            }
        }
    }
    // A dashboard sets its thread / client setting: one of each, so it knows which.
    for k in ["thread", "client"] {
        if params.iter().filter(|p| p.kind == k).count() > 1 {
            bail!("widget “{widget}” has more than one {k} setting");
        }
    }
    Ok(())
}

fn widget_sizes() -> Vec<String> {
    vec!["M".into()]
}
fn widget_height() -> String {
    "hug".into()
}
fn widget_heights() -> Vec<u8> {
    vec![1]
}

/// The kinds of widget Echo draws for a plugin (its data's shape: `today-plugin-widgets.test.mjs`).
pub const WIDGET_KINDS: [&str; 8] = ["stat", "stats", "chart", "list", "status", "card", "table", "page"];

/// Each enabled plugin's manifest (one it can't read is left out).
pub fn enabled_manifests() -> Vec<(String, Manifest)> {
    enabled().into_iter().filter_map(|p| read_manifest(&plugin_dir(&p)).ok().map(|m| (p.id.clone(), m))).collect()
}

/// The commands enabled plugins let Echo's sessions run (`uses.sessions.commands`): `(plugin, command)`.
pub fn session_commands() -> Vec<(String, String)> {
    enabled_manifests().into_iter().flat_map(|(id, m)| m.uses.get("sessions").and_then(|s| s["commands"].as_array()).into_iter().flatten().filter_map(|c| c.as_str().map(|c| (id.clone(), c.to_string()))).collect::<Vec<_>>()).collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingDef {
    pub key: String,
    pub label: String,
    /// string | bool | folder | secret
    #[serde(rename = "type", default = "string_kind")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

fn string_kind() -> String {
    "string".into()
}

const SETTING_KINDS: &[&str] = &["string", "bool", "folder", "secret"];

/// A plugin id: lowercase letters, digits and dashes (it names folders).
pub fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !id.starts_with('-')
}

fn needs_echo(req: &str) -> Result<String> {
    let v = req.trim().trim_start_matches(">=").trim();
    if !crate::update::valid_version(v) {
        bail!("“echo” must be a version like >=0.7.0");
    }
    Ok(v.to_string())
}

/// Read and check a manifest: its fields, settings and capability values (`describe`), its widgets — refused with the
/// first problem there — and its part for Claude (`agent/`, its skills and scripts), whose problems it only notes
/// (`part_problems`): a plugin installed before a newer rule keeps running. An install refuses them (`read_package`).
pub fn read_manifest(dir: &Path) -> Result<Manifest> {
    // An installed copy changes only by an install, which forgets it here: read and walked once, not on each look.
    let installed = dir.starts_with(plugins_dir().join("installed"));
    let stamp = fs::metadata(dir.join(MANIFEST)).and_then(|m| m.modified()).ok();
    if installed {
        if let Some((_, m)) = READ.lock().ok().and_then(|c| c.get(dir).cloned()).filter(|(s, _)| *s == stamp) {
            return Ok(m);
        }
    }
    let (m, problems) = check_package(dir)?;
    if let Some(p) = problems.into_iter().find(|p| !m.part_problems.contains(&p.what)) {
        bail!("{}", p.what);
    }
    if installed {
        if let Ok(mut c) = READ.lock() {
            c.insert(dir.to_path_buf(), (stamp, m.clone()));
        }
    }
    Ok(m)
}

/// A package as an install, an update and a marketplace's offer take it: every problem refuses it, its part for Claude's
/// too.
pub fn read_package(dir: &Path) -> Result<Manifest> {
    let m = read_manifest(dir)?;
    if let Some(p) = m.part_problems.first() {
        bail!("{p}");
    }
    Ok(m)
}

/// Installed copies' manifests as read (`read_manifest`), by folder, with the manifest's time then.
static READ: Mutex<BTreeMap<PathBuf, (Option<std::time::SystemTime>, Manifest)>> = Mutex::new(BTreeMap::new());

/// An installed copy replaced or removed: read it again next time.
pub(super) fn forget_read(id: &str) {
    if let Ok(mut c) = READ.lock() {
        c.retain(|d, _| !d.starts_with(installed_dir(id)));
    }
}

/// Read a manifest and find every problem of its package (`echoctl plugin check` shows them all; an install refuses
/// the first). Errs only when the manifest can't be read at all.
pub fn check_package(dir: &Path) -> Result<(Manifest, Vec<Problem>)> {
    let path = dir.join(MANIFEST);
    let text = fs::read_to_string(&path).with_context(|| format!("no {MANIFEST}"))?;
    let mut m: Manifest = serde_json::from_str(&text).with_context(|| format!("{MANIFEST} isn't valid"))?;
    let mut out: Vec<Problem> = vec![];
    let mut put = |file: &str, r: Result<()>| {
        if let Err(e) = r {
            out.push(Problem { file: file.into(), what: format!("{e:#}") });
        }
    };
    let man = MANIFEST;
    put(man, (|| {
        if !valid_id(&m.id) {
            bail!("its id “{}” isn't a plain name (a–z, 0–9, -)", m.id);
        }
        // Echo's own plugin's skills are `echo:<name>`.
        if m.id == "echo" {
            bail!("its id “echo” is Echo's own");
        }
        Ok(())
    })());
    put(man, (|| {
        if m.name.trim().is_empty() || m.name.chars().count() > 60 {
            bail!("its name must be 1–60 characters");
        }
        Ok(())
    })());
    put(man, (|| {
        if !crate::update::valid_version(&m.version) {
            bail!("its version “{}” isn't like 1.2.3", m.version);
        }
        Ok(())
    })());
    put(man, (|| {
        if m.summary.chars().count() > 240 {
            bail!("its summary is longer than 240 characters");
        }
        Ok(())
    })());
    put(man, m.echo.as_deref().map_or(Ok(()), |r| needs_echo(r).map(|_| ())));
    let mut keys = std::collections::HashSet::new();
    for s in &m.settings {
        put(man, (|| {
            if !valid_id(&s.key.replace('_', "-")) || !keys.insert(s.key.clone()) {
                bail!("setting “{}” needs a plain, unique key", s.key);
            }
            if !SETTING_KINDS.contains(&s.kind.as_str()) {
                bail!("setting “{}” has a type Echo doesn't know ({})", s.key, SETTING_KINDS.join(", "));
            }
            Ok(())
        })());
    }
    put(man, describe(&m.uses).map(|_| ()));
    // Its part for Claude in Echo (`agent` and `agent/`); the skills of `agent/skills/` with its other package skills;
    // their scripts. Noted apart (`part_problems`).
    let mut part = agent::problems(dir, &m);
    let agent_ok = part.is_empty();
    let (skills, skill_problems) = agent::skill_problems(dir, &m);
    let skills_ok = skill_problems.is_empty();
    part.extend(skill_problems);
    m.skill_names = skills.iter().map(|(n, _)| n.clone()).collect();
    // Two of its own skills laid out alike (`x` and `<prefix>-x`): as an install would refuse it.
    if let Some(p) = agent::prefix_of(&m).filter(|p| agent::valid_prefix(p)) {
        if let Err(e) = agent::own_clash(&m.id, &p, &m.skill_names) {
            part.push(Problem { file: man.into(), what: format!("{e:#}") });
        }
    }
    // Its skills are named with its prefix in Echo's sessions.
    if !skills.is_empty() && agent::prefix_of(&m).is_none() {
        part.push(Problem { file: man.into(), what: "it has skills: “agent.prefix” (2–4 of a–z, 0–9) names them in Echo's sessions".into() });
    }
    m.agent_digest = (agent_ok && skills_ok).then(|| agent::digest(dir, &m)).flatten();
    m.scripts_digest = None;
    if skills_ok {
        match scripts::check(dir, &m) {
            Ok(d) => m.scripts_digest = d,
            Err(e) => part.push(Problem { file: man.into(), what: format!("{e:#}") }),
        }
    }
    for k in m.uses.get("network").and_then(|n| n["hosts_from"].as_array()).into_iter().flatten() {
        let k = k.as_str().unwrap_or("");
        put(man, match m.settings.iter().find(|s| s.key == k && s.kind == "string") {
            None => Err(anyhow!("network.hosts_from names “{k}”, which isn't a string setting of the plugin")),
            Some(s) if s.default.is_some() => Err(anyhow!("network.hosts_from names “{k}”, which has a default: the addresses are the user's to enter")),
            Some(_) => Ok(()),
        });
    }
    let mut ids = std::collections::HashSet::new();
    put(man, (|| {
        if m.widgets.len() > MAX_WIDGETS {
            bail!("it declares more than {MAX_WIDGETS} widgets");
        }
        Ok(())
    })());
    for w in &m.widgets {
        put(man, check_widget(w, &m, &mut ids));
    }
    // Its icon goes into the page as a path (`plugin/<id>/<icon>`): lowercase letters, digits, `.`, `_`, `-` and `/`
    // only, ending `.svg` — what the window serves as an image.
    if let Some(i) = &m.icon {
        put(man, (|| {
            if !i.ends_with(".svg") || !i.bytes().all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b'/')) {
                bail!("its icon {i} must be an .svg named with a–z, 0–9, “.”, “_”, “-” and “/” only");
            }
            if fs::metadata(inside_plugin(dir, i)?)?.len() > 16 * 1024 {
                bail!("its icon {i} must be an .svg of 16 KB at most");
            }
            Ok(())
        })());
    }
    for f in [&m.main, &m.client].into_iter().flatten() {
        put(f, (|| {
            let p = inside_plugin(dir, f)?;
            if p.extension().is_none_or(|e| e != "js" && e != "mjs") {
                bail!("{f} isn't a JavaScript module (.js)");
            }
            Ok(())
        })());
    }
    m.part_problems = part.iter().map(|p| p.what.clone()).collect();
    out.extend(part);
    Ok((m, out))
}

/// One widget of the manifest, checked (`ids`: the ones seen so far).
fn check_widget(w: &WidgetDef, m: &Manifest, ids: &mut std::collections::HashSet<String>) -> Result<()> {
    if !valid_id(&w.id) || !ids.insert(w.id.clone()) {
        bail!("widget “{}” needs a plain, unique id (a–z, 0–9, -)", w.id);
    }
    if w.label.trim().is_empty() || w.label.chars().count() > 40 || w.about.chars().count() > 120 {
        bail!("widget “{}” needs a label of 1–40 characters (and an about of 120 at most)", w.id);
    }
    if !WIDGET_KINDS.contains(&w.kind.as_str()) {
        bail!("widget “{}” has a kind Echo doesn't draw ({})", w.id, WIDGET_KINDS.join(", "));
    }
    let mut seen = std::collections::HashSet::new();
    if w.sizes.is_empty() || !w.sizes.iter().all(|s| ["S", "M", "L"].contains(&s.as_str()) && seen.insert(s.clone())) {
        bail!("widget “{}” takes sizes of S, M and L, each once", w.id);
    }
    // A table needs room for its columns: M and L.
    if w.kind == "table" && w.sizes.iter().any(|s| s == "S") {
        bail!("widget “{}” is a table: it takes sizes M and L", w.id);
    }
    if !matches!(w.heights.as_slice(), [1] | [1, 2] | [2, 1]) {
        bail!("widget “{}” takes heights of 1 and 2 rows, each once, with 1", w.id);
    }
    if !["hug", "fill"].contains(&w.height.as_str()) {
        bail!("widget “{}” is hug or fill high", w.id);
    }
    if w.refresh.is_some_and(|r| r < 60) {
        bail!("widget “{}” is asked again every 60 s at most often", w.id);
    }
    if m.main.is_none() {
        bail!("widget “{}” needs a host half (main) to give its data", w.id);
    }
    if w.kind == "page" && m.client.is_none() {
        bail!("widget “{}” is a page: it needs a window half (client) to draw it", w.id);
    }
    check_params(&w.id, &w.params)
}

/// A file of the plugin's own: a relative path inside its folder that exists.
pub fn inside_plugin(dir: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim_start_matches("./");
    let p = Path::new(rel);
    if rel.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        bail!("“{rel}” isn't a file inside the plugin");
    }
    let f = dir.join(p);
    if !f.is_file() {
        bail!("{rel} isn't in the plugin");
    }
    Ok(f)
}

/// A folder of the plugin's own: a relative path inside its folder that exists.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn inside_plugin_dir(dir: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim_start_matches("./").trim_end_matches('/');
    let p = Path::new(rel);
    if rel.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        bail!("“{rel}” isn't a folder inside the plugin");
    }
    let d = dir.join(p);
    if !d.is_dir() {
        bail!("{rel} isn't in the plugin");
    }
    Ok(d)
}

/// The most skills a plugin gives Echo's sessions, its package and runtime ones together.
pub const MAX_SKILLS: usize = 10;

/// Its package skills, checked: `(name, folder in the package)` — the manifest's `skills` folders, then with an `agent`
/// part each folder of `agent/skills/`. Each is a folder inside the plugin named as a skill (a–z, 0–9, -), once; its
/// `SKILL.md` frontmatter holds only `name` (the folder's name) and a one-line `description`, and no `!` command
/// anywhere in it (Claude Code runs one when the skill loads).
pub fn package_skills(dir: &Path, m: &Manifest) -> Result<Vec<(String, PathBuf)>> {
    let (skills, problems) = agent::skill_problems(dir, m);
    match problems.into_iter().next() {
        Some(p) => bail!("{}", p.what),
        None => Ok(skills),
    }
}

/// A skill's name: lowercase words with dashes, up to 40 characters.
pub fn valid_skill_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 40 && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A package skill's `SKILL.md`: frontmatter of `name` (= `name`) and a one-line `description` only, no `!` command.
pub(super) fn check_skill_md(name: &str, text: &str) -> Result<()> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or_else(|| anyhow!("SKILL.md starts with its frontmatter (---)"))?;
    let (front, _) = rest.split_once("\n---").ok_or_else(|| anyhow!("SKILL.md's frontmatter has no end (---)"))?;
    let (mut got_name, mut got_desc) = (None, None);
    for line in front.lines().filter(|l| !l.trim().is_empty()) {
        let (k, v) = line.split_once(':').ok_or_else(|| anyhow!("SKILL.md's frontmatter holds only name and description"))?;
        let v = v.trim();
        let v = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(v);
        match k {
            "name" if got_name.is_none() => got_name = Some(v.to_string()),
            "description" if got_desc.is_none() => got_desc = Some(v.to_string()),
            _ => bail!("SKILL.md's frontmatter holds only name and description (not “{}”)", k.trim()),
        }
    }
    if got_name.as_deref() != Some(name) {
        bail!("SKILL.md's name is its folder's, “{name}”");
    }
    let desc = got_desc.unwrap_or_default();
    if desc.is_empty() || desc.chars().count() > 1024 || matches!(desc.as_str(), "|" | ">" | "|-" | ">-") {
        bail!("SKILL.md needs a description of one line, up to 1024 characters");
    }
    if text.contains("!`") {
        bail!("“!`…`” runs a command in Claude Code; a plugin's SKILL.md runs none");
    }
    Ok(())
}

/// The permission sheet: what the plugin touches (`uses`), and that it runs code of its own.
pub fn sheet(m: &Manifest) -> Result<Vec<String>> {
    let mut lines = describe(&m.uses)?;
    match (&m.main, &m.client) {
        (Some(_), Some(_)) => lines.insert(0, "Runs its own code in Echo and in the Echo window, with your rights".into()),
        (Some(_), None) => lines.insert(0, "Runs its own code in Echo, with your rights".into()),
        (None, Some(_)) => lines.insert(0, "Runs its own code in the Echo window, with your rights".into()),
        (None, None) => {}
    }
    // Its scripts' line carries their skill folders' hash: a version changing any of those files asks again.
    if let Some(d) = &m.scripts_digest {
        for l in lines.iter_mut().filter(|l| l.starts_with("Runs programs from its package")) {
            l.push_str(&format!(" (their files: {d})"));
        }
    }
    // Its part for Claude in Echo — its guide (`agent/`) and its package skills by name —, with the hash of their files:
    // a version changing any of it asks again.
    if agent::has_guide(m) || !m.skill_names.is_empty() {
        let mut what = vec![];
        if agent::has_guide(m) {
            what.push("a guide".to_string());
        }
        let names: Vec<String> = m.skill_names.iter().map(|n| format!("“{n}”")).collect();
        match names.len() {
            0 => {}
            1 => what.push(format!("the skill {}", names[0])),
            _ => what.push(format!("the skills {}", join_words(&names.iter().map(String::as_str).collect::<Vec<_>>()))),
        }
        let digest = m.agent_digest.as_deref().map(|d| format!(" (its files: {d})")).unwrap_or_default();
        lines.push(format!("Tells Claude in Echo how to work with it ({}), loaded only when you work with it{digest}", what.join(" and ")));
    }
    // Its widgets: no grant of their own (their data comes from its code); said in Echo's words, counted.
    match m.widgets.len() {
        0 => {}
        1 => lines.push("Adds a widget you can put on Today".into()),
        n => lines.push(format!("Adds {n} widgets you can put on Today")),
    }
    Ok(lines)
}

/// Why this Echo can't install it (`None` = it can).
pub(crate) fn why_not(m: &Manifest) -> Option<String> {
    let here = env!("CARGO_PKG_VERSION");
    if let Some(v) = m.echo.as_deref().and_then(|r| needs_echo(r).ok()) {
        if crate::update::newer(&v, here) {
            return Some(format!("Needs Echo {v} or later"));
        }
    }
    let missing = not_ready(&m.uses);
    (!missing.is_empty()).then(|| format!("Needs a newer Echo ({} not in this version yet)", missing.join(", ")))
}

// ── registry ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub name: String,
    /// `owner/repo[@ref]`, a git URL (`#ref` after it), or a local folder.
    pub source: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub id: String,
    pub name: String,
    pub version: String,
    pub market: String,
    pub commit: String,
    /// sha256 of the plugin's files (path, size, bytes), checked again on update.
    pub files: String,
    pub enabled: bool,
    pub installed_at: String,
    /// The permission sheet the user agreed to.
    pub uses: Vec<String>,
    /// Its marketplace's source when it was installed (`owner/repo`, a URL, a folder): what holds its skills prefix,
    /// even once that marketplace is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    /// Added by the user (the built-in one isn't stored, only whether it is hidden).
    #[serde(default)]
    pub markets: Vec<Market>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub built_in_hidden: bool,
    #[serde(default)]
    pub installed: BTreeMap<String, Installed>,
}

fn registry_file() -> PathBuf {
    paths::config_dir().join("plugins.json")
}

fn plugins_dir() -> PathBuf {
    paths::config_dir().join("plugins")
}

/// The built-in marketplace's source (`ECHOCTL_PLUGIN_MARKET` points it elsewhere, for development).
fn built_in_source() -> String {
    std::env::var("ECHOCTL_PLUGIN_MARKET").ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| BUILT_IN.into())
}

impl Registry {
    pub fn load() -> Result<Registry> {
        match fs::read_to_string(registry_file()) {
            Ok(t) => serde_json::from_str(&t).context("plugins.json can't be read"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry::default()),
            Err(e) => Err(e).context("plugins.json can't be read"),
        }
    }

    /// Change it under the lock, re-read first, so two writers never undo each other.
    pub fn update<T>(f: impl FnOnce(&mut Registry) -> Result<T>) -> Result<T> {
        fs::create_dir_all(paths::config_dir())?;
        let _lock = wait_lock(&paths::config_dir().join("plugins.lock"))?;
        let mut r = Registry::load()?;
        let out = f(&mut r)?;
        paths::write_atomic(&registry_file(), serde_json::to_string_pretty(&r)?.as_bytes())?;
        Ok(out)
    }

    /// Every marketplace, the built-in one first.
    pub fn all_markets(&self) -> Vec<(Market, bool)> {
        let mut v = vec![(Market { name: BUILT_IN_NAME.into(), source: built_in_source(), hidden: self.built_in_hidden }, true)];
        // My plugins: only with a data folder chosen (else it would be Echo's own plugins folder), and not when the user
        // already has a marketplace by that name.
        if paths::home_chosen() && !self.markets.iter().any(|m| m.name.eq_ignore_ascii_case(MINE)) {
            v.push((Market { name: MINE.into(), source: mine_dir().display().to_string(), hidden: false }, true));
        }
        v.extend(self.markets.iter().cloned().map(|m| (m, false)));
        v
    }

    fn market(&self, name: &str) -> Option<Market> {
        self.all_markets().into_iter().map(|(m, _)| m).find(|m| m.name == name)
    }
}

/// The marketplace a person means on the command line: its name in any case, or its source (`owner/repo`, a URL, a
/// folder); none or several → why, with the names there are.
pub fn market_named(said: &str) -> Result<String> {
    let said = said.trim();
    let all: Vec<Market> = Registry::load()?.all_markets().into_iter().map(|(m, _)| m).collect();
    if let Some(m) = all.iter().find(|m| m.name == said) {
        return Ok(m.name.clone());
    }
    // A folder only when what was said is one (as `market add` reads it), never a name that happens to be a subfolder.
    let folder = match parse_source(said) {
        Ok(Source::Local(p)) => Some(p),
        _ => None,
    };
    let same_folder = |src: &str| folder.as_ref().is_some_and(|f| dunce::canonicalize(src).is_ok_and(|s| &s == f));
    let hits: Vec<&Market> = all.iter().filter(|m| m.name.eq_ignore_ascii_case(said) || m.source == said || same_folder(&m.source)).collect();
    match hits.as_slice() {
        [m] => Ok(m.name.clone()),
        _ => {
            let names: Vec<&str> = all.iter().map(|m| m.name.as_str()).collect();
            let what = if hits.is_empty() { "no marketplace" } else { "more than one marketplace is" };
            bail!("{what} “{said}” — the marketplaces: {}", names.join(", "))
        }
    }
}

/// An exclusive file lock, waited for (≤ 30 s: a fetch holds a marketplace's). Echo's shared JSON files use it too.
pub(crate) fn wait_lock(path: &Path) -> Result<crate::app::FileLock> {
    wait_lock_for(path, "plugins", std::time::Duration::from_secs(30))
}

/// The same for one lock: `what` it guards, named when it stays busy longer than `wait`.
pub(crate) fn wait_lock_for(path: &Path, what: &str, wait: std::time::Duration) -> Result<crate::app::FileLock> {
    let t = std::time::Instant::now();
    loop {
        if let Some(l) = crate::app::FileLock::try_take(path)? {
            return Ok(l);
        }
        if t.elapsed() > wait {
            bail!("another Echo is changing {what} — try again in a moment");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

// ── catalogs ──

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    name: String,
    #[serde(default)]
    featured: Vec<String>,
    #[serde(default)]
    plugins: Vec<CatalogEntry>,
}

#[derive(Debug, Deserialize)]
struct CatalogEntry {
    id: String,
    source: Value,
    #[serde(default)]
    categories: Vec<String>,
}

/// One marketplace's plugins as last fetched (`catalog.json` in its folder).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub name: String,
    pub commit: String,
    pub fetched_at: String,
    /// The Echo that read it: an Echo updated since reads it again (what it may install changed).
    #[serde(default)]
    pub echo_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub plugins: Vec<Offer>,
}

/// A plugin a marketplace offers: its manifest (or why it can't be read) and where its files are.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub id: String,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub featured: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Manifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The plugin's folder on this computer, and the commit it came from.
    #[serde(default)]
    pub dir: PathBuf,
    #[serde(default)]
    pub commit: String,
}

fn market_dir(name: &str) -> PathBuf {
    plugins_dir().join("markets").join(market_key(name))
}

pub fn catalog(name: &str) -> Option<Catalog> {
    serde_json::from_str(&fs::read_to_string(market_dir(name).join("catalog.json")).ok()?).ok()
}

/// A plugin's folder in a marketplace's tree: a relative path inside it, never out of it.
fn inside(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim_start_matches("./");
    let p = Path::new(rel);
    if rel.is_empty() || p.is_absolute() || p.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
        bail!("its source “{rel}” isn't a folder inside the marketplace");
    }
    let dir = root.join(p);
    if !dir.is_dir() {
        bail!("its folder {rel} isn't in the marketplace");
    }
    Ok(dir)
}

/// Fetch a marketplace again and record its catalog (an error recorded too, the last good plugins kept).
pub fn refresh(name: &str) -> Result<Catalog> {
    let m = Registry::load()?.market(name).ok_or_else(|| anyhow!("no marketplace “{name}”"))?;
    let base = market_dir(name);
    fs::create_dir_all(&base)?;
    let _lock = wait_lock(&base.join(".lock"))?;
    let now = chrono::Local::now().to_rfc3339();
    let got = (|| -> Result<Catalog> {
        // My plugins: read as it is, plugin by plugin (each checked on its own below; a missing folder is no plugins, never
        // made here), never a catalog file in it.
        let (snap, text) = if m.name == MINE {
            (Snapshot { commit: String::new(), dir: mine_dir() }, mine_catalog(&mine_dir())?)
        } else {
            let snap = fetch(&m.source, &base, MARKET_LIMIT)?;
            let text = fs::read_to_string(snap.dir.join(CATALOG)).with_context(|| format!("it has no {CATALOG} at its root"))?;
            (snap, text)
        };
        let file: CatalogFile = serde_json::from_str(&text).with_context(|| format!("its {CATALOG} isn't valid"))?;
        let mut plugins = vec![];
        let mut seen = std::collections::HashSet::new();
        for e in file.plugins.into_iter().take(200) {
            // In My plugins a folder whose name isn't an id is listed with why (elsewhere: left out).
            if m.name == MINE && !valid_id(&e.id) && seen.insert(e.id.clone()) {
                plugins.push(Offer { id: e.id, categories: vec![], featured: false, manifest: None, error: Some("its folder name isn't a plugin id: lower-case letters, digits and -".into()), dir: PathBuf::new(), commit: String::new() });
                continue;
            }
            if !valid_id(&e.id) || !seen.insert(e.id.clone()) {
                continue;
            }
            let featured = file.featured.contains(&e.id);
            let at = (|| -> Result<(PathBuf, String)> {
                match &e.source {
                    // My plugins: each plugin by its own files (a change to one is its update, not the others').
                    Value::String(rel) if m.name == MINE => {
                        let dir = inside(&snap.dir, rel)?;
                        let h = tree_hash(&dir)?;
                        Ok((dir, format!("local-{}", &h[..12])))
                    }
                    Value::String(rel) => Ok((inside(&snap.dir, rel)?, snap.commit.clone())),
                    Value::Object(o) => {
                        let repo = o.get("github").and_then(Value::as_str).ok_or_else(|| anyhow!("its source names no github repository"))?;
                        let src = match o.get("ref").and_then(Value::as_str) {
                            Some(r) => format!("{repo}@{r}"),
                            None => repo.to_string(),
                        };
                        if !matches!(parse_source(&src)?, Source::Github { .. }) {
                            bail!("its source isn't owner/repo");
                        }
                        let s = fetch(&src, &base.join("ext").join(&e.id), PLUGIN_LIMIT)?;
                        let dir = match o.get("path").and_then(Value::as_str) {
                            Some(p) => inside(&s.dir, p)?,
                            None => s.dir,
                        };
                        Ok((dir, s.commit))
                    }
                    _ => bail!("its source must be a folder or {{\"github\": …}}"),
                }
            })();
            let offer = match at.and_then(|(dir, commit)| {
                check_tree(&dir, PLUGIN_LIMIT)?;
                let m = read_package(&dir)?;
                if m.id != e.id {
                    bail!("its manifest says “{}”", m.id);
                }
                Ok((dir, commit, m))
            }) {
                Ok((dir, commit, m)) => Offer { id: e.id, categories: e.categories, featured, manifest: Some(m), error: None, dir, commit },
                Err(err) => Offer { id: e.id, categories: e.categories, featured, manifest: None, error: Some(format!("{err:#}")), dir: PathBuf::new(), commit: String::new() },
            };
            plugins.push(offer);
        }
        Ok(Catalog { name: if file.name.trim().is_empty() { m.name.clone() } else { file.name }, commit: snap.commit, fetched_at: now.clone(), echo_version: env!("CARGO_PKG_VERSION").into(), error: None, plugins })
    })();
    let c = match got {
        Ok(c) => c,
        Err(e) => {
            let mut c = catalog(name).unwrap_or_default();
            c.fetched_at = now;
            c.echo_version = env!("CARGO_PKG_VERSION").into();
            c.error = Some(format!("{e:#}"));
            c
        }
    };
    paths::write_atomic(&base.join("catalog.json"), serde_json::to_string_pretty(&c)?.as_bytes())?;
    Ok(c)
}

/// My plugins' catalog: each subfolder holding a manifest, by its folder name (no catalog file to keep).
fn mine_catalog(dir: &Path) -> Result<String> {
    // A folder with a manifest is a plugin (one whose name isn't an id is listed, and says so when installed).
    let mut ids: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.path().join(MANIFEST).is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    ids.sort();
    let plugins: Vec<Value> = ids.iter().map(|id| json!({"id": id, "source": format!("./{id}")})).collect();
    Ok(json!({"name": "My plugins", "plugins": plugins}).to_string())
}

/// A catalog read by this Echo within `STALE_SECS`.
fn fresh(c: &Catalog) -> bool {
    c.echo_version == env!("CARGO_PKG_VERSION")
        && chrono::DateTime::parse_from_rfc3339(&c.fetched_at).is_ok_and(|t| (chrono::Local::now().fixed_offset() - t).num_seconds() < STALE_SECS)
}

/// Is any marketplace due to be read again.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn any_stale() -> bool {
    Registry::load().is_ok_and(|r| r.all_markets().into_iter().any(|(m, _)| !catalog(&m.name).as_ref().is_some_and(fresh)))
}

/// Refresh every marketplace not fetched within `STALE_SECS` by this Echo (or `all`); errors stay in each catalog.
pub fn refresh_stale(all: bool) {
    let Ok(r) = Registry::load() else { return };
    for (m, _) in r.all_markets() {
        if all || !catalog(&m.name).as_ref().is_some_and(fresh) {
            if let Err(e) = refresh(&m.name) {
                eprintln!("plugins: {}: {e:#}", m.name);
            }
        }
    }
}

// ── marketplaces: add / remove / hide ──

pub fn market_add(source: &str, name: Option<&str>) -> Result<String> {
    let src = parse_source(source)?;
    let source = match &src {
        Source::Local(p) => p.display().to_string(),
        _ => source.trim().to_string(),
    };
    let name = name.map(str::trim).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| match &src {
        Source::Github { repo, .. } => repo.split('/').next().unwrap_or(repo).to_string(),
        Source::Git { url, .. } => url.trim_end_matches(".git").rsplit(['/', ':']).next().unwrap_or("git").to_string(),
        Source::Local(p) => p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "local".into()),
    });
    if name.chars().count() > 40 || name.chars().any(char::is_control) {
        bail!("a marketplace's name is at most 40 characters");
    }
    Registry::update(|r| {
        if r.all_markets().iter().any(|(m, _)| m.name.eq_ignore_ascii_case(&name)) {
            bail!("there is a marketplace named “{name}” already");
        }
        if r.all_markets().iter().any(|(m, _)| m.source == source) {
            bail!("that marketplace is added already");
        }
        r.markets.push(Market { name: name.clone(), source: source.clone(), hidden: false });
        Ok(())
    })?;
    Ok(name)
}

/// Remove a marketplace the user added (its installed plugins stay, without updates).
pub fn market_remove(name: &str) -> Result<()> {
    if name == BUILT_IN_NAME {
        bail!("the built-in marketplace can be hidden, not removed");
    }
    if name == MINE {
        bail!("My plugins is always there: it is your own folder of plugins");
    }
    Registry::update(|r| {
        let n = r.markets.len();
        r.markets.retain(|m| m.name != name);
        if r.markets.len() == n {
            bail!("no marketplace “{name}”");
        }
        Ok(())
    })?;
    let _ = fs::remove_dir_all(market_dir(name));
    Ok(())
}

pub fn market_hide(name: &str, hidden: bool) -> Result<()> {
    Registry::update(|r| {
        if name == BUILT_IN_NAME {
            r.built_in_hidden = hidden;
            return Ok(());
        }
        if name == MINE {
            bail!("My plugins can't be hidden: it is your own folder of plugins");
        }
        let m = r.markets.iter_mut().find(|m| m.name == name).ok_or_else(|| anyhow!("no marketplace “{name}”"))?;
        m.hidden = hidden;
        Ok(())
    })
}

// ── install / update / remove ──

fn installed_dir(id: &str) -> PathBuf {
    plugins_dir().join("installed").join(id)
}

/// Where an installed plugin's files are.
pub fn plugin_dir(p: &Installed) -> PathBuf {
    installed_dir(&p.id).join(&p.version)
}

/// The offer for `id`: from `market` when given, else from the only marketplace that has it.
fn find_offer(id: &str, market: Option<&str>) -> Result<(String, Offer)> {
    let r = Registry::load()?;
    let mut found = vec![];
    for (m, _) in r.all_markets() {
        if market.is_some_and(|w| w != m.name) {
            continue;
        }
        let c = match catalog(&m.name) {
            Some(c) => c,
            None => refresh(&m.name)?,
        };
        if let Some(o) = c.plugins.into_iter().find(|o| o.id == id) {
            found.push((m.name.clone(), o));
        }
    }
    match found.len() {
        0 => bail!("no marketplace offers “{id}”{}", market.map(|m| format!(" (looked in {m})")).unwrap_or_default()),
        1 => Ok(found.pop().unwrap()),
        _ => bail!("“{id}” is in {} — say which: --from <marketplace>", found.iter().map(|(m, _)| m.as_str()).collect::<Vec<_>>().join(", ")),
    }
}

/// What installing (or updating to) an offer asks: the sheet, and what is new against `before`.
pub struct Review {
    pub id: String,
    pub name: String,
    pub version: String,
    pub market: String,
    pub commit: String,
    pub uses: Vec<String>,
    pub new_uses: Vec<String>,
    /// Why this Echo can't install it (shown; installing refuses).
    pub blocked: Option<String>,
}

fn review_of(market: &str, o: &Offer, before: Option<&Installed>) -> Result<Review> {
    let m = o.manifest.as_ref().ok_or_else(|| anyhow!("“{}” can't be installed: {}", o.id, o.error.as_deref().unwrap_or("its manifest can't be read")))?;
    let uses = sheet(m)?;
    let new_uses = uses.iter().filter(|u| before.is_none_or(|b| !b.uses.contains(u))).cloned().collect();
    Ok(Review { id: m.id.clone(), name: m.name.clone(), version: m.version.clone(), market: market.into(), commit: o.commit.clone(), uses, new_uses, blocked: why_not(m) })
}

/// The permission sheet for installing `id` (nothing changes).
pub fn review(id: &str, market: Option<&str>) -> Result<Review> {
    let (mk, o) = find_offer(id, market)?;
    let before = Registry::load()?.installed.get(id).cloned();
    review_of(&mk, &o, before.as_ref())
}

/// Install (or replace) `id` from its marketplace's last fetch. `agreed` = the permission lines the
/// user saw: the install refuses when the plugin now asks for anything else.
pub fn install(id: &str, market: Option<&str>, agreed: &[String]) -> Result<Installed> {
    let (mk, o) = find_offer(id, market)?;
    let before = Registry::load()?.installed.get(id).cloned();
    if let Some(b) = before.as_ref().filter(|b| b.market != mk && (mk == MINE || b.market == MINE)) {
        bail!("“{id}” is installed from {} — remove it first to install the one from {mk}", if b.market == MINE { "My plugins" } else { &b.market });
    }
    let rv = review_of(&mk, &o, before.as_ref())?;
    if let Some(why) = &rv.blocked {
        bail!("{} can't be installed: {why}", rv.name);
    }
    if let Some(u) = rv.uses.iter().find(|u| !agreed.contains(u)) {
        bail!("{} asks for something you haven't agreed to: {u}", rv.name);
    }
    check_tree(&o.dir, PLUGIN_LIMIT)?;
    let hash = tree_hash(&o.dir)?;
    let dest = installed_dir(id).join(&rv.version);
    let tmp = installed_dir(id).join(format!(".new-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    copy_tree(&o.dir, &tmp)?;
    // What was copied is what was checked, and what the user agreed to: the copy's own sheet (its files may have
    // changed since the marketplace was read — a local folder above all).
    if tree_hash(&tmp)? != hash {
        let _ = fs::remove_dir_all(&tmp);
        bail!("its files changed while installing — try again");
    }
    let new = match read_package(&tmp).and_then(|m| Ok((sheet(&m)?, m))) {
        Ok((uses, m)) if m.id == rv.id && uses == rv.uses => m,
        _ => {
            let _ = fs::remove_dir_all(&tmp);
            bail!("{} changed since you saw what it asks — look again (read its marketplace again)", rv.name);
        }
    };
    // No skill of it laid out under a name another plugin's has (`acme` + `kit-x` and `acme-kit` + `x`).
    // Its skills prefix held by no other marketplace's plugins; no skill laid out under a name another plugin's has.
    if let Some(prefix) = agent::prefix_of(&new) {
        let checked = agent::claim_prefix(id, &mk, &prefix, false).and_then(|()| if before.as_ref().is_none_or(|b| b.enabled) { agent::clash(id, &prefix, &new.skill_names) } else { Ok(()) });
        if let Err(e) = checked {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
    }
    // Its scripts running from the version replaced stop with everything they started.
    scripts::stop_plugin(id);
    let _ = fs::remove_dir_all(&dest);
    forget_read(id);
    fs::rename(&tmp, &dest)?;
    if let Ok(m) = read_manifest(&dest) {
        scripts::mark_runnable(&dest, &m);
    }
    // An update keeps when it was first installed (a namespace's owner is the earliest: `shared`).
    let first = before.as_ref().filter(|b| b.market == mk).map_or_else(|| chrono::Local::now().to_rfc3339(), |b| b.installed_at.clone());
    let source = Registry::load().ok().and_then(|r| r.market(&mk).map(|m| m.source.clone()));
    let rec = Installed {
        source,
        id: id.into(),
        name: rv.name,
        version: rv.version.clone(),
        market: mk,
        commit: rv.commit,
        files: hash,
        enabled: before.as_ref().is_none_or(|b| b.enabled),
        installed_at: first,
        uses: rv.uses,
    };
    Registry::update(|r| {
        r.installed.insert(id.into(), rec.clone());
        Ok(())
    })?;
    // Each namespace's owner kept from now on: a later removal or update never hands one over (`shared`); its skills
    // prefix too.
    shared::bind_all();
    if let Some(prefix) = read_manifest(&dest).ok().as_ref().and_then(agent::prefix_of) {
        if let Err(e) = agent::claim_prefix(id, &rec.market, &prefix, true) {
            eprintln!("plugin {id}: {e:#}");
        }
    }
    // A sign-in Echo kept for it stays only with the same plugin — the same id from the same marketplace — whose sheet
    // still says `keep`; one from another marketplace, or a version that no longer keeps it, starts without.
    let keeps = read_manifest(&dest).map(|m| may_keep_signin(&m.uses)).unwrap_or(false);
    if !keeps || before.as_ref().is_some_and(|b| b.market != rec.market) {
        signin::forget_plugin(id);
    }
    // Older versions' files go.
    if let Ok(rd) = fs::read_dir(installed_dir(id)) {
        for e in rd.flatten() {
            if e.path() != dest {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
    // Its package skills, as this version ships them, for Echo's sessions.
    if let Err(e) = crate::grants::copy_package_skills(id) {
        eprintln!("plugin {id}: its skills can't be copied: {e:#}");
    }
    // Its part for Claude in Echo's folder session, in the data folder (`.plugins/<id>/`).
    let _ = agent::lay_out(id);
    changed();
    Ok(rec)
}

/// An update waiting for an installed plugin: the newer offer's review (`None` = up to date or gone).
pub fn update_for(p: &Installed) -> Option<Review> {
    update_in(p, &catalog(&p.market)?)
}

/// The same, from a catalog already read.
fn update_in(p: &Installed, c: &Catalog) -> Option<Review> {
    let o = c.plugins.iter().find(|o| o.id == p.id)?;
    // From My plugins, a change to its files is its update (no version bump needed there).
    let changed = if p.market == MINE { o.commit != format!("local-{}", p.files.get(..12).unwrap_or("")) } else { crate::update::newer(&o.manifest.as_ref()?.version, &p.version) };
    changed.then(|| review_of(&p.market, o, Some(p)).ok()).flatten()
}

/// Every installed plugin of `r` with an update waiting (from its marketplace's last fetched catalog, no network;
/// each catalog read once).
pub fn updates_in(r: &Registry) -> Vec<(Installed, Review)> {
    let mut catalogs: std::collections::HashMap<String, Option<Catalog>> = std::collections::HashMap::new();
    r.installed
        .values()
        .filter_map(|p| {
            let c = catalogs.entry(p.market.clone()).or_insert_with(|| catalog(&p.market)).as_ref()?;
            update_in(p, c).map(|u| (p.clone(), u))
        })
        .collect()
}

/// Install the waiting update of `id`; one that asks for more is refused unless those lines were agreed.
pub fn update(id: &str, agreed: &[String]) -> Result<Option<Installed>> {
    let p = Registry::load()?.installed.get(id).cloned().ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
    refresh(&p.market)?;
    let Some(rv) = update_for(&p) else { return Ok(None) };
    let mut ok: Vec<String> = p.uses.clone();
    ok.extend(agreed.iter().cloned());
    if let Some(u) = rv.new_uses.iter().find(|u| !ok.contains(u)) {
        bail!("{} {} asks for more: {u}", rv.name, rv.version);
    }
    install(id, Some(&p.market), &ok).map(Some)
}

pub fn set_enabled(id: &str, on: bool) -> Result<()> {
    if on {
        let p = Registry::load()?.installed.get(id).cloned().ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
        if let Ok(m) = read_manifest(&plugin_dir(&p)) {
            if let Some(prefix) = agent::prefix_of(&m) {
                agent::claim_prefix(id, &p.market, &prefix, true)?;
                // Its package skills and the runtime ones its host half set before.
                let mut names = m.skill_names.clone();
                names.extend(crate::grants::runtime_skill_dirs(id, &m.skill_names).into_iter().map(|(n, _)| n));
                agent::clash(id, &prefix, &names)?;
            }
        }
    }
    Registry::update(|r| {
        let p = r.installed.get_mut(id).ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
        p.enabled = on;
        Ok(())
    })?;
    // Its part for Claude goes with it, and comes back with it.
    if on {
        let _ = agent::lay_out(id);
    } else {
        agent::remove(id);
    }
    changed();
    Ok(())
}

/// Remove `id`: its files and record; its settings and data too unless `keep_data`.
pub fn remove(id: &str, keep_data: bool) -> Result<()> {
    // Its namespace's owner kept before it goes (`shared`): a plugin of the same source left keeps it.
    shared::bind_all();
    Registry::update(|r| r.installed.remove(id).map(|_| ()).ok_or_else(|| anyhow!("“{id}” isn't installed")))?;
    scripts::stop_plugin(id);
    let _ = fs::remove_dir_all(installed_dir(id));
    forget_read(id);
    crate::grants::remove_plugin(id);
    agent::remove(id);
    // A sign-in Echo kept for it goes with it, data kept or not: no plugin installed later gets it.
    signin::forget_plugin(id);
    if !keep_data {
        let _ = fs::remove_dir_all(data_dir(id));
        let _ = fs::remove_file(secrets_file(id));
        let _ = fs::remove_file(addresses_file(id));
        thread_values::forget(id);
    }
    changed();
    Ok(())
}

/// Installed and switched on (what the host halves and the window load).
pub fn enabled() -> Vec<Installed> {
    Registry::load().map(|r| r.installed.into_values().filter(|p| p.enabled).collect()).unwrap_or_default()
}



// ── for the window ──

/// Long jobs the window started (`plugin_op`), by key, and the last one's error.
pub static BUSY: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());
pub static ERRORS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

fn review_json(r: &Review) -> Value {
    json!({"id": r.id, "name": r.name, "version": r.version, "market": r.market, "commit": r.commit, "uses": r.uses, "new_uses": r.new_uses, "blocked": r.blocked})
}

/// Settings → Plugins: installed, offered, marketplaces, and what runs.
pub fn snapshot() -> Value {
    let r = match Registry::load() {
        Ok(r) => r,
        Err(e) => return json!({"error": format!("{e:#}")}),
    };
    let mut markets = vec![];
    let mut discover = vec![];
    for (m, built_in) in r.all_markets() {
        let c = catalog(&m.name);
        markets.push(json!({
            "name": m.name, "source": m.source, "built_in": built_in, "hidden": m.hidden,
            "fetched_at": c.as_ref().map(|c| c.fetched_at.clone()), "commit": c.as_ref().map(|c| c.commit.clone()),
            "error": c.as_ref().and_then(|c| c.error.clone()), "count": c.as_ref().map(|c| c.plugins.len()).unwrap_or(0),
            "title": c.as_ref().map(|c| c.name.clone()).filter(|n| !n.is_empty()),
        }));
        if m.hidden {
            continue;
        }
        for o in c.map(|c| c.plugins).unwrap_or_default() {
            let (name, version, summary, uses, why) = match &o.manifest {
                Some(mf) => (mf.name.clone(), mf.version.clone(), mf.summary.clone(), sheet(mf).unwrap_or_default(), why_not(mf)),
                None => (o.id.clone(), String::new(), String::new(), vec![], o.error.clone()),
            };
            discover.push(json!({
                "id": o.id, "name": name, "version": version, "summary": summary, "market": m.name, "built_in": built_in,
                "categories": o.categories, "featured": o.featured, "uses": uses, "why_not": why,
                "installed": r.installed.get(&o.id).map(|p| p.market == m.name),
            }));
        }
    }
    let installed: Vec<Value> = r
        .installed
        .values()
        .map(|p| {
            let m = read_manifest(&plugin_dir(p)).ok();
            let m_part = m.as_ref().and_then(|m| m.part_problems.first().cloned()).or_else(|| agent::withheld(&p.id));
            let m_prefix = m.as_ref().and_then(agent::prefix_of);
            let summary = m.as_ref().map(|m| m.summary.clone()).unwrap_or_default();
            let client = m.as_ref().and_then(|m| m.client.clone());
            let caps: Vec<String> = m.map(|m| m.uses.keys().cloned().collect()).unwrap_or_default();
            json!({
                "id": p.id, "name": p.name, "version": p.version, "market": p.market, "enabled": p.enabled, "summary": summary, "caps": caps, "client": client,
                "commit": p.commit, "installed_at": p.installed_at, "uses": p.uses,
                "market_gone": r.market(&p.market).is_none(),
                "update": update_for(p).map(|u| review_json(&u)),
                "settings": settings_view(&p.id).unwrap_or_default(),
                "claude_problem": m_part,
                "prefix": m_prefix,
            })
        })
        .collect();
    json!({
        "installed": installed, "discover": discover, "markets": markets,
        "busy": BUSY.lock().map(|b| b.clone()).unwrap_or_default(),
        "errors": ERRORS.lock().map(|b| b.clone()).unwrap_or_default(),
    })
}

/// The review sheet for the window (`plugin_review`), as JSON.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn review_value(id: &str, market: Option<&str>) -> Value {
    match review(id, market) {
        Ok(r) => review_json(&r),
        Err(e) => json!({"id": id, "error": format!("{e:#}")}),
    }
}

/// Run a window operation (`plugin_op`): quick ones now, fetches in the background under `BUSY`.
/// Returns an error to show at once (a background one lands in `ERRORS` under its key).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn page_op(v: &Value) -> Result<()> {
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let list = |k: &str| v[k].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default();
    match s("op").as_str() {
        "enable" => set_enabled(&s("id"), v["on"].as_bool().unwrap_or(true)).inspect(|_| reload()),
        "remove" => remove(&s("id"), v["keep_data"].as_bool().unwrap_or(true)).inspect(|_| reload()),
        "set" => {
            let (id, key, value) = (s("id"), s("key"), v["value"].clone());
            match addresses_to_allow(&id, &key, &value)? {
                Some(_) if BUSY.lock().is_ok_and(|b| b.contains_key(&format!("plugin:{id}"))) => bail!("Not saved: {id} is busy (being installed, updated or asking you); try again in a moment"),
                Some(_) if refused_lately(&id) => bail!("Not saved: you didn't allow these addresses a moment ago"),
                // Addresses a plugin may reach: the page can't be told apart from a plugin's own window half, so the
                // user says yes outside it, in a dialog of the system's.
                Some((name, list)) => {
                    background(format!("plugin:{id}"), "Waiting for you to allow the addresses…", move || {
                        if !allow_addresses(&name, &list) {
                            REFUSED.lock().unwrap_or_else(|e| e.into_inner()).insert(id.clone(), std::time::Instant::now());
                            bail!("Not saved: you didn't allow {name} to reach {}", list.join(", "));
                        }
                        set_setting(&id, &key, &value)
                    });
                    Ok(())
                }
                None => set_setting(&id, &key, &value),
            }
        }
        "market_hide" => market_hide(&s("name"), v["hidden"].as_bool().unwrap_or(true)),
        "market_remove" => market_remove(&s("name")),
        "market_add" => {
            let src = s("source");
            let name = market_add(&src, Some(&s("name")).filter(|n| !n.is_empty()).map(String::as_str))?;
            background(format!("market:{name}"), "Reading the marketplace…", move || refresh(&name).map(|_| ()));
            Ok(())
        }
        "market_refresh" => {
            let name = s("name");
            background(format!("market:{name}"), "Reading the marketplace…", move || {
                let c = refresh(&name)?;
                c.error.map_or(Ok(()), |e| Err(anyhow!(e)))
            });
            Ok(())
        }
        "install" => {
            let (id, market, agreed) = (s("id"), s("market"), list("agreed"));
            background(format!("plugin:{id}"), "Installing…", move || install(&id, Some(&market).filter(|m| !m.is_empty()).map(String::as_str), &agreed).map(|_| reload()));
            Ok(())
        }
        "update" => {
            let (id, agreed) = (s("id"), list("agreed"));
            background(format!("plugin:{id}"), "Updating…", move || update(&id, &agreed).map(|_| reload()));
            Ok(())
        }
        o => bail!("unknown plugin operation “{o}”"),
    }
}

/// Installed, updated, switched on or off, or removed outside the app (`echoctl plugin …`): the running app's host
/// halves follow (a `plugins.request` its watcher takes); in the app its own `reload` does.
fn changed() {
    #[cfg(feature = "js")]
    if !crate::jshost::is_app() && crate::watch::running_watcher().is_some() {
        let _ = fs::write(paths::state_dir().join("plugins.request"), b"");
    }
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
/// What is installed and on, as one short stamp: the window loads its plugins' client halves again when it changes.
/// None when it can't be read just now (nothing follows a read that failed).
pub fn stamp() -> Option<String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in Registry::load().ok()?.installed.values() {
        (&p.id, &p.commit, &p.files, p.enabled).hash(&mut h);
    }
    Some(format!("{:x}", h.finish()))
}

/// The host halves follow what is installed and on (`jshost::sync`).
fn reload() {
    #[cfg(feature = "js")]
    if let Some(s) = stamp() {
        crate::jshost::follow(&s);
    }
}

/// One background job per key; its error kept for the page until the next try.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
fn background(key: String, what: &str, f: impl FnOnce() -> Result<()> + Send + 'static) {
    if let Ok(mut b) = BUSY.lock() {
        if b.contains_key(&key) {
            return;
        }
        b.insert(key.clone(), what.into());
    }
    if let Ok(mut e) = ERRORS.lock() {
        e.remove(&key);
    }
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| Err(anyhow!("it stopped unexpectedly")));
        if let Err(e) = r {
            if let Ok(mut m) = ERRORS.lock() {
                m.insert(key.clone(), format!("{e:#}"));
            }
        }
        if let Ok(mut b) = BUSY.lock() {
            b.remove(&key);
        }
        // Installed, updated or removed: the Inbox (its update items) is read again, not from its cache.
        crate::attention::stale();
    });
}

// ── CLI ──

pub fn print_review(r: &Review) {
    println!("{} {} — from {} ({})", r.name, r.version, r.market, &r.commit[..r.commit.len().min(12)]);
    if r.uses.is_empty() {
        println!("  uses nothing of Echo's yet");
    }
    if let Some(b) = &r.blocked {
        println!("  ⚠ {b}");
    }
    for u in &r.uses {
        let new = if r.new_uses.contains(u) { " (new)" } else { "" };
        println!("  • {u}{new}");
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_plugin_runs_claude_and_sends_only_as_its_install_sheet_said() {
        let uses = |v: Value| v.as_object().cloned().unwrap();
        // No `claude`: no run at all; `claude` without `skip`: any run but one with permission checks skipped.
        assert!(may_run_claude(&uses(json!({})), "none").is_err());
        let plain = uses(json!({"claude": {}}));
        assert!(may_run_claude(&plain, "read").is_ok() && may_run_claude(&plain, "folder").is_ok());
        assert!(may_run_claude(&plain, "skip").is_err());
        assert!(may_run_claude(&uses(json!({"claude": {"skip": true}})), "skip").is_ok());
        assert!(describe_claude(&json!({"skip": true})).unwrap().contains("skipped"));
        // Sends: the ways its sheet named, or its own way.
        let sched = uses(json!({"deliver": {"to": ["teams", "telegram"]}}));
        assert!(may_send(&sched, "teams").is_ok() && may_send(&sched, "telegram").is_ok());
        assert!(may_send(&sched, "mail").is_err());
        let tg = uses(json!({"deliver_provider": {"kinds": ["telegram"]}}));
        assert!(may_send(&tg, "telegram").is_ok() && may_send(&tg, "teams").is_err());
    }
    use super::*;

    #[test]
    fn events_are_heard_as_asked_and_minutes_only_with_sources() {
        let u = |v: Value| v.as_object().cloned().unwrap();
        assert_eq!(heard_events(&u(json!({"events": {"on": ["recording_started", "brain_synced"]}}))), ["recording_started"], "one this Echo has");
        assert_eq!(heard_events(&u(json!({"echo": {"read": ["sources"]}}))), ["minutes_written", "sources_changed"], "as before events existed");
        assert!(heard_events(&u(json!({"echo": {"read": ["dictionary", "clients"]}}))).is_empty(), "names only: no call titles");
        assert!(describe_events(&json!({"on": ["meeting_soon"]})).unwrap().contains("(its subject)"), "the sheet says what text comes");
        assert!(describe_events(&json!({"on": []})).is_err());
    }

    fn plugin(root: &Path, id: &str, version: &str, uses: Value) -> PathBuf {
        let d = root.join("plugins").join(id);
        fs::create_dir_all(d.join("skills")).unwrap();
        fs::write(
            d.join(MANIFEST),
            json!({"id": id, "name": format!("Plugin {id}"), "version": version, "summary": "A test", "uses": uses,
                   "settings": [{"key": "folder", "label": "Folder", "type": "folder"}, {"key": "token", "label": "Token", "type": "secret"}, {"key": "on", "label": "On", "type": "bool", "default": true}]})
                .to_string(),
        )
        .unwrap();
        fs::write(d.join("skills/README.md"), "hi").unwrap();
        d
    }

    fn market(root: &Path, ids: &[&str]) {
        let plugins: Vec<Value> = ids.iter().map(|i| json!({"id": i, "source": format!("./plugins/{i}"), "categories": ["test"]})).collect();
        fs::write(root.join(CATALOG), json!({"name": "Test", "featured": [ids[0]], "plugins": plugins}).to_string()).unwrap();
    }

    #[test]
    fn sources_are_parsed_strictly() {
        assert_eq!(parse_source("onetest-ai/echo-plugins").unwrap(), Source::Github { repo: "onetest-ai/echo-plugins".into(), git_ref: None });
        assert_eq!(parse_source("acme/x@v1.2").unwrap(), Source::Github { repo: "acme/x".into(), git_ref: Some("v1.2".into()) });
        assert_eq!(parse_source("https://git.acme.example/p.git#main").unwrap(), Source::Git { url: "https://git.acme.example/p.git".into(), git_ref: Some("main".into()) });
        assert!(parse_source("acme/x@--upload-pack=evil").is_err());
        assert!(parse_source("acme/x@a..b").is_err());
        assert!(parse_source("just-a-word").is_err());
        assert!(parse_source("a/b/c").is_err());
    }

    #[test]
    fn capabilities_are_described_in_echos_words() {
        let uses = |v: Value| v.as_object().unwrap().clone();
        let w = describe(&uses(json!({"deliver": {"to": ["telegram", "mail"]}}))).unwrap();
        assert_eq!(w, vec!["Sends messages: Telegram (through the plugin that adds it) and mail (to you, or to the addresses you set on a routine)"]);
        assert!(describe(&uses(json!({"deliver": {"to": ["Slack!"]}}))).is_err());
        assert_eq!(describe(&uses(json!({"network": {"hosts": ["api.telegram.org"]}}))).unwrap(), vec!["Talks to api.telegram.org over HTTPS"]);
        assert!(describe(&uses(json!({"network": {"hosts": ["https://x"]}}))).is_err());
        assert!(describe(&uses(json!({"deliver_provider": {"kinds": ["mail"]}}))).is_err());
        assert!(describe(&uses(json!({"teleport": {}}))).unwrap_err().to_string().contains("newer Echo"));
        assert!(describe(&uses(json!({"channel": {"plugin": "evil@somewhere"}}))).is_err());
        assert!(describe(&uses(json!({"channel": {"plugin": "telegram@claude-plugins-official"}}))).is_ok());
        // Known but not implemented yet: listed, not installable.
        let m = Manifest { id: "x".into(), name: "X".into(), version: "0.1.0".into(), echo: None, summary: String::new(), widgets: vec![], skills: vec![], scripts_digest: None, uses: uses(json!({"brain_source": {"kind": "vault"}})), settings: vec![], main: None, client: None, icon: None, agent: None, agent_digest: None, skill_names: vec![], part_problems: vec![] };
        assert!(why_not(&m).unwrap().contains("brain_source"));
        let m = Manifest { echo: Some(">=99.0.0".into()), uses: Default::default(), ..m };
        assert_eq!(why_not(&m).unwrap(), "Needs Echo 99.0.0 or later");
    }

    #[test]
    fn the_sheet_says_whose_sign_in_a_plugin_uses_and_only_those_hosts_are_handed_over() {
        let uses = |v: Value| v.as_object().unwrap().clone();
        assert_eq!(
            describe(&uses(json!({"signin": {"hosts": ["github.com"]}}))).unwrap(),
            vec!["Gets the sign-in saved on this computer for github.com (GitHub CLI or git): it can act as you there and send it to the places it talks to"]
        );
        assert_eq!(
            describe(&uses(json!({"signin": {"hosts": ["github.com", "ghe.acme.example"]}}))).unwrap(),
            vec!["Gets the sign-in saved on this computer for github.com and ghe.acme.example (GitHub CLI or git): it can act as you there and send it to the places it talks to"]
        );
        for bad in [json!({}), json!({"hosts": []}), json!({"hosts": ["a.com", "b.com", "c.com", "d.com", "e.com"]}), json!({"hosts": "github.com"}), json!({"hosts": ["github.com", "github.com"]})] {
            assert!(describe(&uses(json!({"signin": bad}))).is_err(), "{bad}");
        }
        for bad in ["https://github.com", "GitHub.com", "github", "github.com:443", "-x.com", "a..com", "user@github.com", "github.com/x", "git hub.com", "github.com\nhost=evil.com", "10.0.0.1", "192.168.1.20"] {
            assert!(describe(&uses(json!({"signin": {"hosts": [bad]}}))).is_err(), "{bad}");
        }
        // It grants no network of its own.
        assert!(describe(&uses(json!({"signin": {"hosts": ["github.com"]}}))).unwrap().iter().all(|l| !l.contains("Talks to")));
        // `keep`: the plugin signs in itself and Echo keeps that sign-in for it.
        assert_eq!(
            describe(&uses(json!({"signin": {"hosts": ["github.com"], "keep": true}}))).unwrap(),
            ["Gets the sign-in saved on this computer for github.com (GitHub CLI or git): it can act as you there and send it to the places it talks to; it may sign you in to github.com itself and have Echo keep that sign-in for it"]
        );
        assert_eq!(describe(&uses(json!({"signin": {"hosts": ["github.com"], "keep": false}}))).unwrap(), describe(&uses(json!({"signin": {"hosts": ["github.com"]}}))).unwrap());
        assert!(describe(&uses(json!({"signin": {"hosts": ["github.com"], "keep": "yes"}}))).is_err());
        let granted = uses(json!({"signin": {"hosts": ["github.com"]}, "network": {"hosts": ["api.github.com"]}}));
        assert!(may_use_signin(&granted, "github.com").is_ok());
        for other in ["gitlab.com", "api.github.com", "GITHUB.COM", ""] {
            assert!(may_use_signin(&granted, other).unwrap_err().to_string().contains("didn't ask to use your sign-in"), "{other}");
        }
        assert!(may_use_signin(&uses(json!({"network": {"hosts": ["api.github.com"]}})), "github.com").is_err());
    }

    #[test]
    fn the_sheet_says_a_plugin_reaches_the_addresses_the_user_lists() {
        let uses = |v: Value| v.as_object().unwrap().clone();
        assert_eq!(
            describe(&uses(json!({"network": {"hosts_from": ["tts"]}}))).unwrap(),
            vec!["Talks to the addresses you list in its setting “tts” (wildcards allowed; plain http only inside your own network)"]
        );
        assert_eq!(
            describe(&uses(json!({"network": {"hosts": ["api.acme.example"], "hosts_from": ["a", "b"]}}))).unwrap(),
            vec!["Talks to api.acme.example over HTTPS and to the addresses you list in its settings “a” and “b” (wildcards allowed; plain http only inside your own network)"]
        );
        assert!(describe(&uses(json!({"network": {}}))).is_err(), "neither a host nor a setting");
    }

    #[test]
    fn addresses_from_the_window_wait_for_the_users_yes_and_nothing_else_does() {
        let (_env, root) = paths::testenv::folders("plugins-allow");
        let mk = root.join("market");
        let d = mk.join("plugins/sneak");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join(MANIFEST),
            json!({"id": "sneak", "name": "Acme sneak", "version": "0.1.0", "uses": {"network": {"hosts_from": ["tts"]}},
                   "settings": [{"key": "tts", "label": "TTS", "type": "string"}, {"key": "voice", "label": "Voice", "type": "string"}]})
            .to_string(),
        )
        .unwrap();
        fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "sneak", "source": "./plugins/sneak"}]}).to_string()).unwrap();
        let name = market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        refresh(&name).unwrap();
        install("sneak", Some(&name), &review("sneak", Some(&name)).unwrap().uses).unwrap();
        let ask = |key: &str, v: &str| addresses_to_allow("sneak", key, &json!(v));
        assert_eq!(ask("tts", "http://10.0.0.*:*, https://a.acme.example").unwrap(), Some(("Acme sneak".to_string(), vec!["http://10.0.0.*:*".to_string(), "https://a.acme.example".to_string()])));
        assert!(ask("tts", "http://example.com").unwrap_err().to_string().contains("“TTS” isn't an address"), "a wrong list is refused at once, no dialog, by its label");
        assert_eq!(ask("voice", "alto").unwrap(), None, "not an address setting");
        assert_eq!(ask("tts", "  ").unwrap(), None, "emptied: it only narrows");
        set_setting("sneak", "tts", &json!("http://10.0.0.*:*")).unwrap();
        assert!(ask("tts", "http://10.0.0.*:*").unwrap().is_some(), "the list as saved is asked about too (no race with a save)");
        assert!(ask("tts", &format!("http://10.0.0.1,{}", " ".repeat(2001))).is_ok(), "spaces trimmed");
        assert!(ask("tts", &"http://10.0.0.1,".repeat(200)).unwrap_err().to_string().contains("2000"));
        assert!(ask("tts", "http://10.0.*.*:*").unwrap().is_some(), "a wider one asks again");
    }

    #[test]
    fn a_plugin_reads_only_the_other_apps_files_it_named() {
        let uses = |v: Value| v.as_object().unwrap().clone();
        assert_eq!(
            describe(&uses(json!({"files": {"read": ["~/Library/Application Support/obsidian/obsidian.json", "%APPDATA%/obsidian/obsidian.json"]}}))).unwrap(),
            vec!["Reads these files of other apps on this computer: “~/Library/Application Support/obsidian/obsidian.json” and “%APPDATA%/obsidian/obsidian.json”"]
        );
        for bad in [
            json!([]), json!(["/etc/passwd"]), json!(["~/../x"]), json!(["~/.ssh/id_rsa"]), json!(["~/Library/Keychains/login.keychain-db"]), json!(["~/notes/*.md"]), json!(["~/"]), json!(["~/a//b"]),
            json!(["~/Library/Application Support/echo/graph.json"]), json!(["%APPDATA%/echo/graph.json"]), json!(["~/.config/gh/hosts.yml"]), json!(["~/.netrc"]),
            json!(["~/Library/Application Support/Google/Chrome/Default/Cookies"]), json!(["~/.ssh./id_rsa"]), json!(["~/SSH~1/id_rsa"]), json!(["%APPDATA%/acme /x.json"]),
            json!(["~/.config/acme/credentials.json"]), json!(["~/.config/acme/token"]), json!(["~/.zsh_history"]),
        ] {
            assert!(describe(&uses(json!({"files": {"read": bad.clone()}}))).is_err(), "{bad}");
        }
        let env = |k: &str| match k {
            "HOME" => Some(PathBuf::from("/Users/sam")),
            "APPDATA" => Some(PathBuf::from("C:/Users/sam/AppData/Roaming")),
            _ => None,
        };
        assert_eq!(app_file("~/Library/Application Support/obsidian/obsidian.json", env).unwrap(), Some(PathBuf::from("/Users/sam/Library/Application Support/obsidian/obsidian.json")));
        assert_eq!(app_file("%APPDATA%/obsidian/obsidian.json", env).unwrap(), Some(PathBuf::from("C:/Users/sam/AppData/Roaming/obsidian/obsidian.json")));
        assert_eq!(app_file("%LOCALAPPDATA%/x/y.json", env).unwrap(), None, "not on this computer");
        assert!(app_file_rel("~/.config/acme/app.json").is_ok());
        // Read only as named: a link anywhere below the base (here a folder above the file) leads elsewhere: not read.
        let (_env, root) = paths::testenv::folders("plugins-app-file");
        let home = root.join("home-of-sam");
        fs::create_dir_all(home.join(".config/acme")).unwrap();
        fs::write(home.join(".config/acme/app.json"), "{}").unwrap();
        let h = home.clone();
        let env = move |k: &str| (k == "HOME").then(|| h.clone());
        assert!(app_file_real("~/.config/acme/app.json", env.clone()).unwrap().is_some_and(|f| f.ends_with(".config/acme/app.json")));
        assert_eq!(app_file_real("~/.config/acme/none.json", env.clone()).unwrap(), None, "not there");
        #[cfg(unix)]
        {
            let elsewhere = root.join("echo-config");
            fs::create_dir_all(&elsewhere).unwrap();
            fs::write(elsewhere.join("app.json"), "secret").unwrap();
            std::os::unix::fs::symlink(&elsewhere, home.join(".config/other")).unwrap();
            assert_eq!(app_file_real("~/.config/other/app.json", env.clone()).unwrap(), None, "a linked folder above the file");
            std::os::unix::fs::symlink(elsewhere.join("app.json"), home.join(".config/acme/linked.json")).unwrap();
            assert_eq!(app_file_real("~/.config/acme/linked.json", env).unwrap(), None, "the file itself a link");
        }
    }

    #[test]
    fn hosts_from_names_a_string_setting_of_the_plugin() {
        let (_env, root) = paths::testenv::folders("plugins-hosts-from");
        let d = root.join("p");
        fs::create_dir_all(&d).unwrap();
        let manifest = |hosts_from: Value| {
            fs::write(
                d.join(MANIFEST),
                json!({"id": "p", "name": "P", "version": "0.1.0", "uses": {"network": {"hosts_from": hosts_from}},
                       "settings": [{"key": "tts", "label": "TTS", "type": "string"}, {"key": "on", "label": "On", "type": "bool"}]})
                .to_string(),
            )
            .unwrap();
            read_manifest(&d)
        };
        assert!(manifest(json!(["tts"])).is_ok());
        assert!(manifest(json!(["nope"])).unwrap_err().to_string().contains("isn't a string setting"), "no such setting");
        assert!(manifest(json!(["on"])).unwrap_err().to_string().contains("isn't a string setting"), "not a string");
        fs::write(
            d.join(MANIFEST),
            json!({"id": "p", "name": "P", "version": "0.1.0", "uses": {"network": {"hosts_from": ["tts"]}},
                   "settings": [{"key": "tts", "label": "TTS", "type": "string", "default": "https://tts.acme.example"}]})
            .to_string(),
        )
        .unwrap();
        assert!(read_manifest(&d).unwrap_err().to_string().contains("has a default"), "the author doesn't choose the address");
    }

    #[test]
    fn a_tree_with_a_link_is_refused() {
        let (_env, root) = paths::testenv::folders("plugins-link");
        let d = root.join("t");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("a"), "x").unwrap();
        assert_eq!(check_tree(&d, PLUGIN_LIMIT).unwrap(), (1, 1));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/passwd", d.join("l")).unwrap();
            assert!(check_tree(&d, PLUGIN_LIMIT).unwrap_err().to_string().contains("link"));
        }
        assert!(inside(&d, "../x").is_err());
        assert!(inside(&d, "/etc").is_err());
    }

    #[test]
    fn my_plugins_are_the_data_folders_plugins_each_change_an_update_that_asks() {
        let (_env, root) = paths::testenv::folders("plugins-mine");
        // Always listed, never removed or hidden; its folder made when read.
        assert!(Registry::load().unwrap().all_markets().iter().any(|(m, built)| m.name == MINE && *built));
        assert!(market_remove(MINE).is_err() && market_hide(MINE, true).is_err());
        let d = plugin(&paths::home(), "standup", "0.1.0", json!({}));
        fs::create_dir_all(mine_dir().join("notes-only")).unwrap(); // no manifest: not a plugin
        let c = refresh(MINE).unwrap();
        assert!(c.error.is_none(), "{:?}", c.error);
        assert_eq!(c.plugins.iter().map(|o| o.id.as_str()).collect::<Vec<_>>(), vec!["standup"]);
        // What the user agreed to is what is installed: a manifest changed after the sheet was read is refused.
        let rv = review("standup", Some(MINE)).unwrap();
        let first = fs::read_to_string(d.join(MANIFEST)).unwrap();
        fs::write(d.join(MANIFEST), first.replace("\"uses\":{}", "\"uses\":{\"network\":{\"hosts\":[\"evil.example.com\"]}}")).unwrap();
        assert!(install("standup", Some(MINE), &rv.uses).unwrap_err().to_string().contains("changed since you saw"));
        fs::write(d.join(MANIFEST), &first).unwrap();
        refresh(MINE).unwrap();
        let p = install("standup", Some(MINE), &rv.uses).unwrap();
        assert!(plugin_dir(&p).starts_with(paths::config_dir()), "a copy, not the folder itself");
        assert!(update_for(&p).is_none(), "nothing changed");
        // A change to its files is its update — same version; it asks only for what is new (nothing here).
        fs::write(d.join("skills/README.md"), "changed").unwrap();
        refresh(MINE).unwrap();
        let p = Registry::load().unwrap().installed["standup"].clone();
        let up = update_for(&p).expect("a change to its files is an update");
        assert_eq!((up.version.as_str(), up.new_uses.len()), ("0.1.0", 0));
        assert_eq!(update("standup", &up.uses).unwrap().unwrap().version, "0.1.0");
        assert!(update_for(&Registry::load().unwrap().installed["standup"]).is_none());
        // One installed from another marketplace is never replaced by one of yours (nor the other way).
        let mk = root.join("market");
        plugin(&mk, "standup", "0.2.0", json!({}));
        market(&mk, &["standup"]);
        let other = market_add(mk.to_str().unwrap(), Some("acme")).unwrap();
        refresh(&other).unwrap();
        assert!(install("standup", Some(&other), &[]).unwrap_err().to_string().contains("remove it first"));
    }

    #[test]
    fn install_update_and_remove_from_a_local_marketplace() {
        let (_env, root) = paths::testenv::folders("plugins-flow");
        let mk = root.join("market");
        plugin(&mk, "demo", "0.1.0", json!({}));
        plugin(&mk, "later", "0.1.0", json!({"brain_source": {"kind": "vault"}}));
        fs::create_dir_all(mk.join("plugins/broken")).unwrap();
        market(&mk, &["demo", "later", "broken"]);
        let name = market_add(mk.to_str().unwrap(), Some("acme")).unwrap();
        assert!(market_add(mk.to_str().unwrap(), Some("again")).is_err(), "the same source twice");
        let c = refresh(&name).unwrap();
        assert!(c.error.is_none(), "{:?}", c.error);
        assert_eq!(c.plugins.len(), 3);
        assert!(c.plugins[0].featured);
        assert!(c.plugins[2].error.as_deref().unwrap().contains(MANIFEST));
        // Not installable yet, and nothing agreed → refused.
        assert!(install("later", Some(&name), &[]).unwrap_err().to_string().contains("newer Echo"));
        let p = install("demo", Some(&name), &[]).unwrap();
        assert_eq!(p.version, "0.1.0");
        assert!(plugin_dir(&p).join("skills/README.md").is_file());
        // Settings: a folder is checked, a secret is never shown.
        assert!(set_setting("demo", "folder", &json!("/no/such/folder")).is_err());
        set_setting("demo", "token", &json!("s3cret")).unwrap();
        set_setting("demo", "on", &json!("false")).unwrap();
        let view = settings_view("demo").unwrap();
        assert_eq!(view[1]["set"], true);
        assert!(!view[1].to_string().contains("s3cret"));
        assert_eq!(view[2]["value"], false);
        assert_eq!(setting("demo", "token").unwrap(), "s3cret");
        // A new version that asks for more waits for the user. A catalog read by another Echo is read again at once
        // (an Echo updated since may install what it couldn't), however recent.
        let d = plugin(&mk, "demo", "0.2.0", json!({"deliver": {"to": ["mail"]}}));
        assert!(d.is_dir());
        let mut c = catalog(&name).unwrap();
        assert!(fresh(&c), "read a moment ago");
        c.echo_version = "0.0.1".into();
        assert!(!fresh(&c));
        c.echo_version = env!("CARGO_PKG_VERSION").into();
        c.fetched_at = (chrono::Local::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert!(!fresh(&c));
        refresh(&name).unwrap();
        let p = Registry::load().unwrap().installed["demo"].clone();
        let rv = update_for(&p).unwrap();
        // The Inbox lists it (`attention`): every installed plugin with an update waiting.
        let waiting = updates_in(&Registry::load().unwrap());
        assert_eq!(waiting.iter().map(|(p, u)| (p.id.as_str(), u.version.as_str())).collect::<Vec<_>>(), [("demo", "0.2.0")]);
        // Its Inbox item: what it says and where it goes.
        let item = crate::attention::update_item(&waiting[0].0, &waiting[0].1);
        assert_eq!((item["title"].as_str(), item["go"].as_str(), item["version"].as_str()), (Some("Plugin demo 0.2.0 is available"), Some("settings/plugins/demo"), Some("0.2.0")));
        assert!(item["why"].as_str().unwrap().contains("it asks for more"), "{item}");
        assert_eq!(rv.new_uses.len(), 1);
        assert!(update("demo", &[]).unwrap_err().to_string().contains("asks for more"));
        assert_eq!(update("demo", &rv.new_uses).unwrap().unwrap().version, "0.2.0");
        assert!(updates_in(&Registry::load().unwrap()).is_empty(), "updated: nothing waits");
        assert_eq!(fs::read_dir(installed_dir("demo")).unwrap().count(), 1, "the old version's files are gone");
        set_enabled("demo", false).unwrap();
        assert!(enabled().is_empty());
        let snap = snapshot();
        assert_eq!(snap["installed"][0]["id"], "demo");
        assert_eq!(snap["markets"][1]["name"], MINE, "My plugins right after the built-in one");
        assert_eq!(snap["markets"][2]["name"], "acme");
        remove("demo", false).unwrap();
        assert!(!data_dir("demo").exists() && !secrets_file("demo").exists());
        market_remove("acme").unwrap();
        assert!(market_remove(BUILT_IN_NAME).is_err());
    }

    #[test]
    fn a_plugins_icon_is_an_svg_of_its_own_16_kb_at_most() {
        let (_env, root) = paths::testenv::folders("plugins-icon");
        let d = root.join("p");
        fs::create_dir_all(&d).unwrap();
        let with = |icon: &str| {
            fs::write(d.join(MANIFEST), json!({"id": "p", "name": "P", "version": "0.1.0", "icon": icon, "uses": {"work": {}}}).to_string()).unwrap();
            read_manifest(&d)
        };
        fs::write(d.join("icon.svg"), "<svg/>").unwrap();
        fs::write(d.join("icon.png"), "png").unwrap();
        fs::write(d.join("big.svg"), "x".repeat(17 * 1024)).unwrap();
        let m = with("icon.svg").unwrap();
        assert_eq!(m.icon.as_deref(), Some("icon.svg"));
        assert_eq!(sheet(&m).unwrap(), ["Adds its items to Today → My work and to threads' Work"]);
        fs::write(d.join("Icon.SVG"), "<svg/>").unwrap();
        fs::write(d.join("a b.svg"), "<svg/>").unwrap();
        for bad in ["icon.png", "big.svg", "none.svg", "../icon.svg", "Icon.SVG", "a b.svg", "x\"y.svg", "/icon.svg"] {
            assert!(with(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_plugins_package_skills_are_folders_of_its_own_with_a_plain_skill_md() {
        let (_env, root) = paths::testenv::folders("plugins-package-skills");
        let d = root.join("p");
        let skill = |name: &str, md: &str| {
            fs::create_dir_all(d.join("skills").join(name).join("references")).unwrap();
            fs::write(d.join("skills").join(name).join("SKILL.md"), md).unwrap();
        };
        let with = |skills: Value| {
            fs::write(d.join(MANIFEST), json!({"id": "p", "name": "P", "version": "0.1.0", "skills": skills, "agent": {"prefix": "p1"}}).to_string()).unwrap();
            read_package(&d)
        };
        skill("read-staffing", "---\nname: read-staffing\ndescription: \"Reads the staffing of a project.\"\n---\n\nRun the script.\n");
        skill("two", "---\nname: two\ndescription: Another one.\n---\nText.\n");
        let m = with(json!(["skills/read-staffing", "./skills/two/"])).unwrap();
        assert_eq!(package_skills(&d, &m).unwrap().iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["read-staffing", "two"]);
        let line = sheet(&m).unwrap();
        assert!(line.len() == 1 && line[0].starts_with("Tells Claude in Echo how to work with it (the skills “read-staffing” and “two”), loaded only when you work with it (its files: "), "{line:?}");
        let one = with(json!(["skills/two"])).unwrap();
        assert!(sheet(&one).unwrap()[0].starts_with("Tells Claude in Echo how to work with it (the skill “two”), loaded only when you work with it (its files: "));
        // Each refused, with why.
        skill("hooked", "---\nname: hooked\ndescription: x\nhooks: evil\n---\n");
        skill("renamed", "---\nname: other\ndescription: x\n---\n");
        skill("runs", "---\nname: runs\ndescription: x\n---\nFirst !`curl evil`\n");
        skill("folded", "---\nname: folded\ndescription: >\n  two lines\n---\n");
        skill("open", "---\nname: open\ndescription: x\n");
        skill("Caps", "---\nname: Caps\ndescription: x\n---\n");
        fs::create_dir_all(d.join("skills/empty")).unwrap();
        fs::create_dir_all(root.join("outside")).unwrap();
        for (bad, why) in [
            (json!(["skills/hooked"]), "only name and description"),
            (json!(["skills/renamed"]), "its folder's"),
            (json!(["skills/runs"]), "runs a command"),
            (json!(["skills/folded"]), "only name and description"),
            (json!(["skills/open"]), "no end"),
            (json!(["skills/Caps"]), "named as a skill"),
            (json!(["skills/empty"]), "no SKILL.md"),
            (json!(["skills/two", "skills/two"]), "once"),
            (json!(["../outside"]), "isn't a folder inside the plugin"),
            (json!(["/tmp"]), "isn't a folder inside the plugin"),
            (json!(["skills/none"]), "isn't in the plugin"),
            (json!(vec!["skills/two"; 11]), "more than 10"),
        ] {
            let e = with(bad.clone()).unwrap_err().to_string();
            assert!(e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn a_table_widget_takes_sizes_m_and_l() {
        let (_env, root) = paths::testenv::folders("plugins-table-widget");
        let d = root.join("p");
        fs::create_dir_all(&d).unwrap();
        let with = |sizes: Value| {
            fs::write(d.join(MANIFEST), json!({"id": "p", "name": "P", "version": "0.1.0", "main": "main.js", "widgets": [{"id": "rows", "label": "Rows", "kind": "table", "sizes": sizes, "heights": [1, 2]}]}).to_string()).unwrap();
            read_manifest(&d)
        };
        fs::write(d.join("main.js"), "export function apply(ctx) {}").unwrap();
        assert_eq!(with(json!(["M", "L"])).unwrap().widgets[0].kind, "table");
        assert!(with(json!(["S", "M"])).unwrap_err().to_string().contains("is a table: it takes sizes M and L"));
        assert!(with(json!(["M"])).is_ok(), "M alone");
    }

    #[test]
    fn a_marketplace_is_found_by_its_name_in_any_case_or_by_its_source() {
        let (_env, root) = paths::testenv::folders("plugins-market-named");
        let mk = root.join("market");
        fs::create_dir_all(&mk).unwrap();
        fs::write(mk.join("echo-marketplace.json"), json!({"name": "Acme", "plugins": []}).to_string()).unwrap();
        let name = market_add(mk.to_str().unwrap(), Some("Acme")).unwrap();
        assert_eq!(market_named("Acme").unwrap(), name);
        assert_eq!(market_named("acme").unwrap(), name, "any case");
        assert_eq!(market_named(mk.to_str().unwrap()).unwrap(), name, "its folder");
        assert_eq!(market_named(&format!(" {} ", mk.display())).unwrap(), name, "its folder, spaces around");
        let e = market_named("globex").unwrap_err().to_string();
        assert!(e.contains("no marketplace “globex”") && e.contains("Acme"), "names the ones there are: {e}");
        // The built-in one, said in another case, is found — and still can't be removed.
        assert_eq!(market_named(&BUILT_IN_NAME.to_uppercase()).unwrap(), BUILT_IN_NAME);
        assert!(market_remove(&market_named(&BUILT_IN_NAME.to_uppercase()).unwrap()).is_err());
        // Two whose names differ only in case (a hand-edited plugins.json): the exact one, else neither.
        Registry::update(|r| {
            r.markets.push(Market { name: "ACME".into(), source: "acme/plugins".into(), hidden: false });
            Ok(())
        })
        .unwrap();
        assert_eq!(market_named("ACME").unwrap(), "ACME");
        assert!(market_named("acme").unwrap_err().to_string().contains("more than one"));
        assert_eq!(market_named("acme/plugins").unwrap(), "ACME", "by its owner/repo");
    }

}

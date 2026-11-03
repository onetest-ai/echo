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
use std::process::Command;
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::app::NoWindow;
use crate::paths;

/// Our public marketplace, always listed (it can be hidden, not removed).
pub const BUILT_IN: &str = "onetest-ai/echo-plugins";
const BUILT_IN_NAME: &str = "onetest";
/// The marketplace's catalog file at the root of its repository.
const CATALOG: &str = "echo-marketplace.json";
/// A plugin's manifest at the root of its folder.
const MANIFEST: &str = "echo-plugin.json";
/// A marketplace refreshed by itself once its fetch is this old (window open, `refresh_stale`).
const STALE_SECS: i64 = 6 * 3600;

/// Caps on what a fetch may unpack: a whole marketplace, one plugin.
const MARKET_LIMIT: Limit = Limit { files: 5000, bytes: 64 << 20 };
const PLUGIN_LIMIT: Limit = Limit { files: 400, bytes: 20 << 20 };

#[derive(Clone, Copy)]
struct Limit {
    files: usize,
    bytes: u64,
}

// ── capabilities ──

/// What a plugin may ask of Echo. `ready` = this Echo implements it; a plugin using one that isn't
/// is listed but can't be installed ("needs a newer Echo"). The words are the permission sheet's —
/// written here, never taken from the plugin.
struct Capability {
    id: &'static str,
    ready: bool,
    describe: fn(&Value) -> Result<String>,
}

const CAPABILITIES: &[Capability] = &[
    Capability { id: "claude", ready: true, describe: describe_claude },
    Capability { id: "schedule", ready: true, describe: describe_schedule },
    Capability { id: "deliver", ready: true, describe: describe_deliver },
    Capability { id: "deliver_provider", ready: true, describe: describe_deliver_provider },
    Capability { id: "network", ready: true, describe: describe_network },
    Capability { id: "channel", ready: true, describe: describe_channel },
    Capability { id: "brain_source", ready: false, describe: describe_brain_source },
    Capability { id: "export", ready: false, describe: describe_export },
    Capability { id: "sessions", ready: true, describe: describe_sessions },
    Capability { id: "folders", ready: true, describe: describe_folders },
    Capability { id: "echo", ready: true, describe: describe_echo },
];

fn strings(v: &Value, key: &str) -> Result<Vec<String>> {
    match &v[key] {
        Value::Null => Ok(vec![]),
        Value::Array(a) => a.iter().map(|x| x.as_str().map(str::to_string).ok_or_else(|| anyhow!("“{key}” must list words"))).collect(),
        _ => bail!("“{key}” must be a list"),
    }
}

fn only(what: &str, got: &[String], allowed: &[&str]) -> Result<()> {
    match got.iter().find(|g| !allowed.contains(&g.as_str())) {
        Some(g) => bail!("{what} “{g}” isn't one Echo knows ({})", allowed.join(", ")),
        None => Ok(()),
    }
}

fn join_words(w: &[&str]) -> String {
    match w {
        [] => String::new(),
        [a] => a.to_string(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn describe_claude(_: &Value) -> Result<String> {
    Ok("Runs Claude Code on your AI profiles, with the permissions you pick".into())
}

fn describe_schedule(_: &Value) -> Result<String> {
    Ok("Works on its own on a schedule while Echo runs".into())
}

fn describe_deliver(v: &Value) -> Result<String> {
    let to = strings(v, "to")?;
    if let Some(k) = to.iter().find(|k| !crate::deliver::valid_kind(k)) {
        bail!("a delivery “{k}” isn't a plain name");
    }
    if to.is_empty() {
        bail!("“deliver” names no way to send (teams, mail, or one a plugin adds)");
    }
    let words: Vec<String> = to
        .iter()
        .map(|t| match t.as_str() {
            "teams" => "Teams (your chat with yourself)".to_string(),
            "mail" => "mail (to you, or to the addresses you set on a routine)".to_string(),
            k => format!("{} (through the plugin that adds it)", title_case(k)),
        })
        .collect();
    Ok(format!("Sends messages: {}", join_words(&words.iter().map(String::as_str).collect::<Vec<_>>())))
}

fn title_case(k: &str) -> String {
    let mut c = k.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn describe_deliver_provider(v: &Value) -> Result<String> {
    let kinds = strings(v, "kinds")?;
    if kinds.is_empty() || kinds.iter().any(|k| !crate::deliver::valid_kind(k) || crate::deliver::BUILTIN.contains(&k.as_str())) {
        bail!("“deliver_provider” names its ways by plain new names");
    }
    Ok(format!("Adds a way Echo sends things to you: {}", join_words(&kinds.iter().map(|k| title_case(k)).collect::<Vec<_>>().iter().map(String::as_str).collect::<Vec<_>>())))
}

fn describe_network(v: &Value) -> Result<String> {
    let hosts = strings(v, "hosts")?;
    if hosts.is_empty() || hosts.iter().any(|h| h.is_empty() || !h.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') || !h.contains('.')) {
        bail!("“network” names hosts (e.g. api.example.com)");
    }
    Ok(format!("Talks to {} over HTTPS", join_words(&hosts.iter().map(String::as_str).collect::<Vec<_>>())))
}

fn describe_channel(v: &Value) -> Result<String> {
    let p = v["plugin"].as_str().unwrap_or("");
    // Claude Code's own channel plugins only, from Anthropic's marketplace (an org allowlist would
    // refuse any other anyway).
    let Some(name) = p.strip_suffix("@claude-plugins-official").filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')) else {
        bail!("“channel” must name a channel plugin of claude-plugins-official, e.g. telegram@claude-plugins-official");
    };
    let shown: String = name.split('-').map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ");
    Ok(format!("Lets you reach your Claude · Echo folder session through {shown} while it is open in Echo"))
}

fn describe_brain_source(v: &Value) -> Result<String> {
    match v["kind"].as_str() {
        Some("vault") => Ok("Adds a folder you pick (an Obsidian vault) to the Brain: read and indexed on this computer".into()),
        _ => bail!("“brain_source” kind must be one Echo knows (vault)"),
    }
}

fn describe_export(v: &Value) -> Result<String> {
    if v["kind"].as_str() != Some("vault") {
        bail!("“export” kind must be one Echo knows (vault)");
    }
    let what = strings(v, "what")?;
    only("An export of", &what, &["minutes", "people"])?;
    if what.is_empty() {
        bail!("“export” names nothing to write (minutes, people)");
    }
    Ok(format!("Writes copies of your {} into a folder you pick — only files it wrote, never yours", join_words(&what.iter().map(String::as_str).collect::<Vec<_>>())))
}

fn describe_sessions(v: &Value) -> Result<String> {
    let mut parts = vec![];
    if v["skills"].as_bool() == Some(true) {
        parts.push("gives Claude in Echo skills of its own — text only, in Echo's sessions, not your Claude Code".to_string());
    }
    if v["read"].as_bool() == Some(true) {
        parts.push("lets Claude in Echo read the folders you give it (your Echo folder session may also change them, with your permissions)".to_string());
    }
    if v["hints"].as_bool() == Some(true) {
        parts.push("tells Claude what it knows about the client or thread open in the window".to_string());
    }
    if parts.is_empty() {
        bail!("“sessions” adds nothing (skills, read, hints)");
    }
    let mut text = parts.join("; ");
    text[..1].make_ascii_uppercase();
    Ok(text)
}

fn describe_folders(v: &Value) -> Result<String> {
    if v["write"].as_bool() == Some(true) {
        bail!("“folders” is read-only (Echo doesn't let plugins write to your folders)");
    }
    let what = v["what"].as_str().unwrap_or("folders").trim();
    if what.is_empty() || what.chars().count() > 60 {
        bail!("“folders” what: a few words, e.g. “Obsidian vaults”");
    }
    Ok(format!("Reads the {what} you pick, on this computer — only those"))
}

fn describe_echo(v: &Value) -> Result<String> {
    let read = strings(v, "read")?;
    only("Echo data", &read, &["clients", "threads"])?;
    if read.is_empty() {
        bail!("“echo” reads nothing (clients, threads)");
    }
    Ok(format!("Reads the names of your {} in Echo", read.join(" and ")))
}

/// The permission sheet's lines for a manifest's `uses`, in order; an unknown capability or a bad
/// value is an error naming it.
pub fn describe(uses: &serde_json::Map<String, Value>) -> Result<Vec<String>> {
    uses.iter()
        .map(|(k, v)| {
            let c = CAPABILITIES.iter().find(|c| c.id == k).ok_or_else(|| anyhow!("it uses “{k}”, which this Echo doesn't know — it may need a newer Echo"))?;
            (c.describe)(v).with_context(|| format!("“{k}”"))
        })
        .collect()
}

/// The capabilities a manifest uses that this Echo doesn't implement yet.
fn not_ready(uses: &serde_json::Map<String, Value>) -> Vec<String> {
    uses.keys().filter(|k| CAPABILITIES.iter().find(|c| c.id == k.as_str()).is_none_or(|c| !c.ready)).cloned().collect()
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

/// Read and check a manifest: its fields, settings and capability values (`describe`).
pub fn read_manifest(dir: &Path) -> Result<Manifest> {
    let path = dir.join(MANIFEST);
    let text = fs::read_to_string(&path).with_context(|| format!("no {MANIFEST}"))?;
    let m: Manifest = serde_json::from_str(&text).with_context(|| format!("{MANIFEST} isn't valid"))?;
    if !valid_id(&m.id) {
        bail!("its id “{}” isn't a plain name (a–z, 0–9, -)", m.id);
    }
    if m.name.trim().is_empty() || m.name.chars().count() > 60 {
        bail!("its name must be 1–60 characters");
    }
    if !crate::update::valid_version(&m.version) {
        bail!("its version “{}” isn't like 1.2.3", m.version);
    }
    if m.summary.chars().count() > 240 {
        bail!("its summary is longer than 240 characters");
    }
    if let Some(r) = &m.echo {
        needs_echo(r)?;
    }
    let mut keys = std::collections::HashSet::new();
    for s in &m.settings {
        if !valid_id(&s.key.replace('_', "-")) || !keys.insert(s.key.clone()) {
            bail!("setting “{}” needs a plain, unique key", s.key);
        }
        if !SETTING_KINDS.contains(&s.kind.as_str()) {
            bail!("setting “{}” has a type Echo doesn't know ({})", s.key, SETTING_KINDS.join(", "));
        }
    }
    describe(&m.uses)?;
    for f in [&m.main, &m.client].into_iter().flatten() {
        let p = inside_plugin(dir, f)?;
        if p.extension().is_none_or(|e| e != "js" && e != "mjs") {
            bail!("{f} isn't a JavaScript module (.js)");
        }
    }
    Ok(m)
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

/// The permission sheet: what the plugin touches (`uses`), and that it runs code of its own.
pub fn sheet(m: &Manifest) -> Result<Vec<String>> {
    let mut lines = describe(&m.uses)?;
    match (&m.main, &m.client) {
        (Some(_), Some(_)) => lines.insert(0, "Runs its own code in Echo and in the Echo window, with your rights".into()),
        (Some(_), None) => lines.insert(0, "Runs its own code in Echo, with your rights".into()),
        (None, Some(_)) => lines.insert(0, "Runs its own code in the Echo window, with your rights".into()),
        (None, None) => {}
    }
    Ok(lines)
}

/// Why this Echo can't install it (`None` = it can).
fn why_not(m: &Manifest) -> Option<String> {
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
        v.extend(self.markets.iter().cloned().map(|m| (m, false)));
        v
    }

    fn market(&self, name: &str) -> Option<Market> {
        self.all_markets().into_iter().map(|(m, _)| m).find(|m| m.name == name)
    }
}

/// An exclusive file lock, waited for (≤ 30 s: a fetch holds a marketplace's).
fn wait_lock(path: &Path) -> Result<crate::app::FileLock> {
    let t = std::time::Instant::now();
    loop {
        if let Some(l) = crate::app::FileLock::try_take(path)? {
            return Ok(l);
        }
        if t.elapsed() > std::time::Duration::from_secs(30) {
            bail!("another Echo is changing plugins — try again in a moment");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

// ── sources and fetching ──

#[derive(Debug, PartialEq)]
enum Source {
    Github { repo: String, git_ref: Option<String> },
    Git { url: String, git_ref: Option<String> },
    Local(PathBuf),
}

fn plain_ref(r: &str) -> bool {
    !r.is_empty() && r.len() <= 100 && !r.starts_with('-') && r.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-/".contains(&b)) && !r.contains("..")
}

fn parse_source(s: &str) -> Result<Source> {
    let s = s.trim();
    if s.starts_with("https://") || s.starts_with("ssh://") || s.starts_with("git@") {
        let (url, r) = match s.rsplit_once('#') {
            Some((u, r)) => (u, Some(r)),
            None => (s, None),
        };
        if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
            bail!("“{s}” isn't a git address");
        }
        if let Some(r) = r.filter(|r| !plain_ref(r)) {
            bail!("“{r}” isn't a plain branch, tag or commit");
        }
        return Ok(Source::Git { url: url.into(), git_ref: r.map(str::to_string) });
    }
    let local = s.starts_with('/') || s.starts_with('.') || s.starts_with('~') || Path::new(s).is_absolute();
    if local {
        let p = if let Some(rest) = s.strip_prefix("~/") { paths::user_home().join(rest) } else { PathBuf::from(s) };
        let p = dunce::canonicalize(&p).with_context(|| format!("no folder {s}"))?;
        if !p.is_dir() {
            bail!("{s} isn't a folder");
        }
        return Ok(Source::Local(p));
    }
    let (repo, r) = match s.split_once('@') {
        Some((repo, r)) => (repo, Some(r)),
        None => (s, None),
    };
    let ok = |p: &str| !p.is_empty() && p.len() <= 100 && !p.starts_with('.') && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    match repo.split_once('/') {
        Some((o, n)) if ok(o) && ok(n) => {}
        _ => bail!("“{s}” isn't owner/repo, a git address (https://…) or a folder"),
    }
    if let Some(r) = r.filter(|r| !plain_ref(r)) {
        bail!("“{r}” isn't a plain branch, tag or commit");
    }
    Ok(Source::Github { repo: repo.into(), git_ref: r.map(str::to_string) })
}

/// A fetched tree: where its files are, and the commit they are (`local-<hash>` for a folder).
struct Snapshot {
    dir: PathBuf,
    commit: String,
}

fn hex40(s: &str) -> Option<String> {
    let s = s.trim();
    (s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())).then(|| s.to_ascii_lowercase())
}

fn run_out(c: &mut Command) -> Result<Vec<u8>> {
    let out = c.output().with_context(|| format!("{:?} can't start", c.get_program()))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{}", err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("failed").trim());
    }
    Ok(out.stdout)
}

/// GitHub: the commit a ref (default: the default branch) points to.
fn github_commit(repo: &str, git_ref: Option<&str>) -> Result<String> {
    let r = git_ref.unwrap_or("HEAD");
    let api = format!("https://api.github.com/repos/{repo}/commits/{r}");
    let curl = run_out(Command::new("curl").no_window().args(["-fsSL", "-H", "Accept: application/vnd.github.sha", &api]));
    if let Some(sha) = curl.as_ref().ok().and_then(|o| hex40(&String::from_utf8_lossy(o))) {
        return Ok(sha);
    }
    // A private repository: the user's own `gh` sign-in.
    let gh = crate::app::find_program("gh");
    let out = run_out(Command::new(&gh).no_window().args(["api", "-H", "Accept: application/vnd.github.sha", &format!("repos/{repo}/commits/{r}")]))
        .map_err(|e| anyhow!("can't read {repo} on GitHub ({}; gh: {e})", curl.err().map(|e| e.to_string()).unwrap_or_else(|| "no commit".into())))?;
    hex40(&String::from_utf8_lossy(&out)).ok_or_else(|| anyhow!("GitHub gave no commit for {repo}@{r}"))
}

/// Unpack a GitHub commit's tarball into `dest` (empty, created).
fn github_unpack(repo: &str, sha: &str, dest: &Path, work: &Path) -> Result<()> {
    let tgz = work.join("tree.tar.gz");
    let url = format!("https://codeload.github.com/{repo}/tar.gz/{sha}");
    let curl = Command::new("curl").no_window().args(["-fsSL", "--max-filesize", "200000000", "-o"]).arg(&tgz).arg(&url).status();
    if !curl.is_ok_and(|s| s.success()) {
        let gh = crate::app::find_program("gh");
        let bytes = run_out(Command::new(&gh).no_window().args(["api", &format!("repos/{repo}/tarball/{sha}")])).with_context(|| format!("downloading {repo}"))?;
        fs::write(&tgz, bytes)?;
    }
    fs::create_dir_all(dest)?;
    // bsdtar (macOS, Windows 10+) and GNU tar both drop absolute paths and refuse `..`; links are
    // refused after, by `check_tree`.
    run_out(Command::new("tar").no_window().arg("-xzf").arg(&tgz).arg("-C").arg(dest).arg("--strip-components").arg("1")).context("unpacking")?;
    let _ = fs::remove_file(&tgz);
    Ok(())
}

fn git_fetch(url: &str, git_ref: Option<&str>, dest: &Path) -> Result<String> {
    let git = crate::app::find_program("git");
    let mut c = Command::new(&git).no_window();
    c.args(["clone", "--quiet", "--depth", "1", "--no-tags", "-c", "core.symlinks=false"]);
    if let Some(r) = git_ref {
        c.args(["--branch", r]);
    }
    c.arg("--").arg(url).arg(dest).env("GIT_TERMINAL_PROMPT", "0");
    run_out(&mut c).with_context(|| format!("git can't fetch {url}"))?;
    let sha = run_out(Command::new(&git).no_window().arg("-C").arg(dest).args(["rev-parse", "HEAD"]))?;
    let sha = hex40(&String::from_utf8_lossy(&sha)).ok_or_else(|| anyhow!("git gave no commit"))?;
    fs::remove_dir_all(dest.join(".git"))?;
    Ok(sha)
}

/// Files, bytes: refuses links, special files and anything over the limit.
fn check_tree(dir: &Path, limit: Limit) -> Result<(usize, u64)> {
    let (mut files, mut bytes) = (0usize, 0u64);
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let e = e?;
            if e.file_name() == ".git" {
                continue;
            }
            let md = fs::symlink_metadata(e.path())?;
            let rel = e.path().strip_prefix(dir).unwrap_or(&e.path()).display().to_string();
            if md.file_type().is_symlink() {
                bail!("it holds a link ({rel}) — refused");
            } else if md.is_dir() {
                stack.push(e.path());
            } else if md.is_file() {
                files += 1;
                bytes += md.len();
            } else {
                bail!("it holds a special file ({rel})");
            }
            if files > limit.files || bytes > limit.bytes {
                bail!("it is too big (over {} files or {} MB)", limit.files, limit.bytes >> 20);
            }
        }
    }
    Ok((files, bytes))
}

/// sha256 over the tree's files in path order (path, size, bytes); what an install records.
fn tree_hash(dir: &Path) -> Result<String> {
    let mut list = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d)? {
            let p = e?.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            let md = fs::symlink_metadata(&p)?;
            if md.is_dir() {
                stack.push(p);
            } else if md.is_file() {
                list.push(p);
            }
        }
    }
    let rel = |p: &Path| p.strip_prefix(dir).unwrap_or(p).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    list.sort_by_key(|p| rel(p));
    let mut h = Sha256::new();
    for p in list {
        let b = fs::read(&p)?;
        h.update(rel(&p).as_bytes());
        h.update([0]);
        h.update((b.len() as u64).to_le_bytes());
        h.update(&b);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)? {
        let e = e?;
        if e.file_name() == ".git" {
            continue;
        }
        let md = fs::symlink_metadata(e.path())?;
        let dest = to.join(e.file_name());
        if md.is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else if md.is_file() {
            fs::copy(e.path(), &dest)?;
        }
    }
    Ok(())
}

/// A folder name for a marketplace (its files and lock).
fn market_key(name: &str) -> String {
    let k: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' }).collect();
    let h = format!("{:x}", Sha256::digest(name.as_bytes()));
    format!("{}-{}", k.trim_matches('-').chars().take(40).collect::<String>(), &h[..8])
}

/// Fetch a source into `<base>/<commit>/` (kept if already there); older commits are removed.
fn fetch(source: &str, base: &Path, limit: Limit) -> Result<Snapshot> {
    match parse_source(source)? {
        Source::Local(p) => {
            check_tree(&p, limit)?;
            Ok(Snapshot { commit: format!("local-{}", &tree_hash(&p)?[..12]), dir: p })
        }
        src => {
            fs::create_dir_all(base)?;
            let work = base.join(format!(".fetch-{}", std::process::id()));
            let _ = fs::remove_dir_all(&work);
            fs::create_dir_all(&work)?;
            let tree = work.join("tree");
            let got = (|| -> Result<String> {
                match &src {
                    Source::Github { repo, git_ref } => {
                        let sha = github_commit(repo, git_ref.as_deref())?;
                        if base.join(&sha).is_dir() {
                            return Ok(sha);
                        }
                        github_unpack(repo, &sha, &tree, &work)?;
                        Ok(sha)
                    }
                    Source::Git { url, git_ref } => git_fetch(url, git_ref.as_deref(), &tree),
                    Source::Local(_) => unreachable!(),
                }
            })();
            let sha = match got {
                Ok(s) => s,
                Err(e) => {
                    let _ = fs::remove_dir_all(&work);
                    return Err(e);
                }
            };
            let dir = base.join(&sha);
            if !dir.is_dir() {
                if let Err(e) = check_tree(&tree, limit) {
                    let _ = fs::remove_dir_all(&work);
                    return Err(e);
                }
                fs::rename(&tree, &dir)?;
            }
            let _ = fs::remove_dir_all(&work);
            for e in fs::read_dir(base)?.flatten() {
                if e.path() != dir && e.file_name().to_string_lossy().len() == 40 {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
            Ok(Snapshot { dir, commit: sha })
        }
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
        let snap = fetch(&m.source, &base, MARKET_LIMIT)?;
        let text = fs::read_to_string(snap.dir.join(CATALOG)).with_context(|| format!("it has no {CATALOG} at its root"))?;
        let file: CatalogFile = serde_json::from_str(&text).with_context(|| format!("its {CATALOG} isn't valid"))?;
        let mut plugins = vec![];
        let mut seen = std::collections::HashSet::new();
        for e in file.plugins.into_iter().take(200) {
            if !valid_id(&e.id) || !seen.insert(e.id.clone()) {
                continue;
            }
            let featured = file.featured.contains(&e.id);
            let at = (|| -> Result<(PathBuf, String)> {
                match &e.source {
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
                let m = read_manifest(&dir)?;
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
        Ok(Catalog { name: if file.name.trim().is_empty() { m.name.clone() } else { file.name }, commit: snap.commit, fetched_at: now.clone(), error: None, plugins })
    })();
    let c = match got {
        Ok(c) => c,
        Err(e) => {
            let mut c = catalog(name).unwrap_or_default();
            c.fetched_at = now;
            c.error = Some(format!("{e:#}"));
            c
        }
    };
    paths::write_atomic(&base.join("catalog.json"), serde_json::to_string_pretty(&c)?.as_bytes())?;
    Ok(c)
}

/// Refresh every marketplace not fetched within `STALE_SECS` (or `all`); errors stay in each catalog.
pub fn refresh_stale(all: bool) {
    let Ok(r) = Registry::load() else { return };
    for (m, _) in r.all_markets() {
        let fresh = catalog(&m.name).and_then(|c| chrono::DateTime::parse_from_rfc3339(&c.fetched_at).ok()).is_some_and(|t| (chrono::Local::now().fixed_offset() - t).num_seconds() < STALE_SECS);
        if all || !fresh {
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
    // What was copied is what was checked.
    if tree_hash(&tmp)? != hash {
        let _ = fs::remove_dir_all(&tmp);
        bail!("its files changed while installing — try again");
    }
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest)?;
    let rec = Installed {
        id: id.into(),
        name: rv.name,
        version: rv.version.clone(),
        market: mk,
        commit: rv.commit,
        files: hash,
        enabled: before.as_ref().is_none_or(|b| b.enabled),
        installed_at: chrono::Local::now().to_rfc3339(),
        uses: rv.uses,
    };
    Registry::update(|r| {
        r.installed.insert(id.into(), rec.clone());
        Ok(())
    })?;
    // Older versions' files go.
    if let Ok(rd) = fs::read_dir(installed_dir(id)) {
        for e in rd.flatten() {
            if e.path() != dest {
                let _ = fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(rec)
}

/// An update waiting for an installed plugin: the newer offer's review (`None` = up to date or gone).
pub fn update_for(p: &Installed) -> Option<Review> {
    let o = catalog(&p.market)?.plugins.into_iter().find(|o| o.id == p.id)?;
    let m = o.manifest.as_ref()?;
    if !crate::update::newer(&m.version, &p.version) {
        return None;
    }
    review_of(&p.market, &o, Some(p)).ok()
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
    Registry::update(|r| {
        let p = r.installed.get_mut(id).ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
        p.enabled = on;
        Ok(())
    })
}

/// Remove `id`: its files and record; its settings and data too unless `keep_data`.
pub fn remove(id: &str, keep_data: bool) -> Result<()> {
    Registry::update(|r| r.installed.remove(id).map(|_| ()).ok_or_else(|| anyhow!("“{id}” isn't installed")))?;
    let _ = fs::remove_dir_all(installed_dir(id));
    crate::grants::remove_plugin(id);
    if !keep_data {
        let _ = fs::remove_dir_all(data_dir(id));
        let _ = fs::remove_file(secrets_file(id));
    }
    Ok(())
}

/// Installed and switched on (what the host halves and the window load).
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn enabled() -> Vec<Installed> {
    Registry::load().map(|r| r.installed.into_values().filter(|p| p.enabled).collect()).unwrap_or_default()
}



// ── settings ──

/// A plugin's own folder in the data folder (its settings and state).
pub fn data_dir(id: &str) -> PathBuf {
    paths::state_dir().join("plugins").join(id)
}

fn secrets_file(id: &str) -> PathBuf {
    plugins_dir().join("secrets").join(format!("{id}.json"))
}

fn read_map(path: &Path) -> serde_json::Map<String, Value> {
    fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn installed_manifest(id: &str) -> Result<Manifest> {
    let p = Registry::load()?.installed.get(id).cloned().ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
    read_manifest(&plugin_dir(&p))
}

/// The settings as the page shows them: each definition with its value (a secret only as set / not).
pub fn settings_view(id: &str) -> Result<Vec<Value>> {
    let m = installed_manifest(id)?;
    let vals = read_map(&data_dir(id).join("settings.json"));
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

/// A setting's value (secrets included), for the capabilities.
#[allow(dead_code)] // read by the capabilities from phase 2 on
pub fn setting(id: &str, key: &str) -> Option<Value> {
    read_map(&secrets_file(id)).remove(key).or_else(|| read_map(&data_dir(id).join("settings.json")).remove(key))
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
    let _lock = wait_lock(&paths::config_dir().join("plugins.lock"))?;
    if def.kind == "secret" {
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
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
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
            let summary = m.as_ref().map(|m| m.summary.clone()).unwrap_or_default();
            let client = m.as_ref().and_then(|m| m.client.clone());
            let caps: Vec<String> = m.map(|m| m.uses.keys().cloned().collect()).unwrap_or_default();
            json!({
                "id": p.id, "name": p.name, "version": p.version, "market": p.market, "enabled": p.enabled, "summary": summary, "caps": caps, "client": client,
                "commit": p.commit, "installed_at": p.installed_at, "uses": p.uses,
                "market_gone": r.market(&p.market).is_none(),
                "update": update_for(p).map(|u| review_json(&u)),
                "settings": settings_view(&p.id).unwrap_or_default(),
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
        "set" => set_setting(&s("id"), &s("key"), &v["value"]),
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

/// The host halves follow what is installed and on (`jshost::sync`).
fn reload() {
    #[cfg(feature = "js")]
    crate::jshost::sync();
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
    use super::*;

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
        let m = Manifest { id: "x".into(), name: "X".into(), version: "0.1.0".into(), echo: None, summary: String::new(), uses: uses(json!({"brain_source": {"kind": "vault"}})), settings: vec![], main: None, client: None };
        assert!(why_not(&m).unwrap().contains("brain_source"));
        let m = Manifest { echo: Some(">=99.0.0".into()), uses: Default::default(), ..m };
        assert_eq!(why_not(&m).unwrap(), "Needs Echo 99.0.0 or later");
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
        // A new version that asks for more waits for the user.
        let d = plugin(&mk, "demo", "0.2.0", json!({"deliver": {"to": ["mail"]}}));
        assert!(d.is_dir());
        refresh(&name).unwrap();
        let p = Registry::load().unwrap().installed["demo"].clone();
        let rv = update_for(&p).unwrap();
        assert_eq!(rv.new_uses.len(), 1);
        assert!(update("demo", &[]).unwrap_err().to_string().contains("asks for more"));
        assert_eq!(update("demo", &rv.new_uses).unwrap().unwrap().version, "0.2.0");
        assert_eq!(fs::read_dir(installed_dir("demo")).unwrap().count(), 1, "the old version's files are gone");
        set_enabled("demo", false).unwrap();
        assert!(enabled().is_empty());
        let snap = snapshot();
        assert_eq!(snap["installed"][0]["id"], "demo");
        assert_eq!(snap["markets"][1]["name"], "acme");
        remove("demo", false).unwrap();
        assert!(!data_dir("demo").exists() && !secrets_file("demo").exists());
        market_remove("acme").unwrap();
        assert!(market_remove(BUILT_IN_NAME).is_err());
    }
}

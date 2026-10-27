//! Model profiles: which model, and which account, Echo's AI jobs use — the live notes and the
//! questions during a call (`livenotes.rs`) and the minutes after it (`notes.rs`).
//!
//! Stored per computer in `<config dir>/ai.json` (sign-ins and local models belong to the machine,
//! not to the data folder). Without the file there is one profile, `system`: Claude Code as the
//! user set it up — what Echo did before profiles existed.
//!
//! - `system`: Claude Code through the user's login shell with all of their settings (a gateway,
//!   Bedrock, a data folder's `.claude/settings.json`); `config.notes_model` / `config.live_model`.
//! - `local`: a server that speaks Anthropic's Messages API at `base_url` — Ollama on this computer
//!   (nothing checks it is local; the window names the host when it isn't). Live jobs call it
//!   directly with thinking off — Claude Code can't turn a local model's thinking off (measured:
//!   ~50 s an update instead of ~7 s); the minutes go through Claude Code pointed at it.
//! - `claude`: a claude.ai account in a Claude Code config folder of its own
//!   (`<config dir>/claude/<id>`), signed in once. The terminal's current `/login` never matters.
//!
//! A call pins its profile when its recording starts (`meta.json` → `ai_profile`). Routing is
//! strict: a missing profile, an unreadable `ai.json` or `meta.json`, a signed-out or different
//! account stops the AI job with the reason; Echo never falls back to another profile. Recording
//! itself never depends on any of this.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{config::Config, paths};

pub const FILE: &str = "ai.json";
/// The built-in profile: Claude Code as the user configured it.
pub const SYSTEM: &str = "system";
/// Pinned when the AI settings couldn't be read as a recording started (never a valid id).
pub const UNPINNED: &str = "!unreadable";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    System,
    Local,
    Claude,
}

impl Kind {
    /// Echo sets the route (own config folder, routing variables replaced, own settings only).
    pub fn managed(self) -> bool {
        match self {
            Kind::System => false,
            Kind::Local | Kind::Claude => true,
        }
    }

    /// May run on a model on this computer, so it shares the local model's lock and waits for
    /// transcription (`system` may point at a local model too).
    pub fn gated(self) -> bool {
        match self {
            Kind::System | Kind::Local => true,
            Kind::Claude => false,
        }
    }

    /// Needs a claude.ai sign-in in its own folder.
    pub fn signs_in(self) -> bool {
        match self {
            Kind::Claude => true,
            Kind::System | Kind::Local => false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    /// local: the server (its Messages API is `<base_url>/v1/messages`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// local: the server's token (Ollama takes any; a gateway's key)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_model: Option<String>,
    /// claude: the account first seen signed in to this profile's folder (`Account::key`: email ·
    /// organisation); another one later stops jobs until `echoctl ai accept` (a plan change doesn't)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Settings {
    /// The profile a new recording pins when none is chosen for it (absent = `system`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Live notes during calls. Off until turned on: they send the call's text to a model.
    #[serde(default)]
    pub live_notes: bool,
    /// The user's profiles; `system` is built in and never stored.
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

fn file() -> PathBuf {
    paths::config_dir().join(FILE)
}

impl Settings {
    /// No file = defaults; a file that can't be read, or holds invalid or duplicate profile ids,
    /// is an error (and nothing is saved over it).
    pub fn load() -> Result<Settings> {
        let s: Settings = match fs::read(file()) {
            Ok(b) => serde_json::from_slice(&b).with_context(|| format!("reading {}", file().display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Settings::default()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", file().display())),
        };
        s.check().with_context(|| format!("in {}", file().display()))?;
        Ok(s)
    }

    /// Every stored id names a folder: valid (`system` is built in) and unique.
    fn check(&self) -> Result<()> {
        let mut seen = std::collections::HashSet::new();
        for p in &self.profiles {
            if !valid_id(&p.id) {
                bail!("\"{}\" isn't a valid profile id (lower-case letters, digits and dashes; not \"system\")", p.id);
            }
            if !seen.insert(p.id.as_str()) {
                bail!("the profile id \"{}\" is there twice", p.id);
            }
        }
        Ok(())
    }

    /// Atomic and owner-only from the start (a gateway token may be in it): a temporary file of its
    /// own, created 0600, then renamed. Never what `load` would refuse.
    pub fn save(&self) -> Result<()> {
        use std::io::Write;
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        self.check()?;
        fs::create_dir_all(paths::config_dir())?;
        let bytes = serde_json::to_vec_pretty(self)?;
        let tmp = paths::config_dir().join(format!(".{FILE}.{}.{}.tmp", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let written = opts.open(&tmp).and_then(|mut f| f.write_all(&bytes)).and_then(|()| fs::rename(&tmp, file()));
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        Ok(written?)
    }

    /// Change the settings under a lock every writer takes (the tray, the app's sign-in checks,
    /// the CLI), re-read inside it: one change never undoes another made meanwhile.
    pub fn update<R>(f: impl FnOnce(&mut Settings) -> Result<R>) -> Result<R> {
        fs::create_dir_all(paths::config_dir())?;
        let path = paths::config_dir().join("ai.json.lock");
        let t = Instant::now();
        let _lock = loop {
            match crate::app::FileLock::try_take(&path).with_context(|| format!("opening {}", path.display()))? {
                Some(l) => break l,
                None if t.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(50)),
                None => bail!("the AI settings are being changed by another process — try again"),
            }
        };
        let mut s = Settings::load()?;
        let before = s.default.clone();
        let r = f(&mut s)?;
        // A new default must name a profile (every new call pins it): one removed while a menu
        // offering it was open is refused.
        if let Some(d) = s.default.as_deref().filter(|d| s.default != before && *d != SYSTEM && !s.profiles.iter().any(|p| p.id == *d)) {
            bail!("there is no profile \"{d}\" any more");
        }
        s.save()?;
        Ok(r)
    }

    pub fn default_id(&self) -> String {
        self.default.clone().unwrap_or_else(|| SYSTEM.into())
    }

    /// Every profile, `system` first.
    pub fn all(&self) -> Vec<Profile> {
        std::iter::once(system_profile()).chain(self.profiles.iter().cloned()).collect()
    }

    /// A profile by id — an error when it doesn't exist (never another one instead).
    pub fn get(&self, id: &str) -> Result<Profile> {
        if id == SYSTEM {
            return Ok(system_profile());
        }
        if id == UNPINNED {
            bail!("no AI profile could be chosen when this call started (the AI settings couldn't be read) — choose one for this call in the Echo window");
        }
        self.profiles.iter().find(|p| p.id == id).cloned().ok_or_else(|| {
            anyhow!("the AI profile \"{id}\" doesn't exist on this computer — choose another for this call in the Echo window (Live tab or Write notes)")
        })
    }
}

/// `system`: its models come from the data folder's settings, as before profiles.
fn system_profile() -> Profile {
    let cfg = Config::load().unwrap_or_default();
    Profile {
        id: SYSTEM.into(),
        name: "Claude Code (as configured)".into(),
        kind: Kind::System,
        base_url: None,
        token: None,
        live_model: Some(cfg.live_model.clone().unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into())),
        notes_model: cfg.notes_model_arg(),
        account: None,
    }
}

/// Where `system` runs its jobs: `notes_in`, else the data folder. Strict: that folder's
/// `.claude/settings.json` may route it (a model on this computer), so a `config.json` that can't
/// be read, or a `notes_in` that isn't a folder, fails the job instead of running it elsewhere.
pub fn system_folder() -> Result<PathBuf> {
    let cfg = Config::load().map_err(|e| anyhow!("the data folder's settings (config.json) can't be read: {e:#}"))?;
    match cfg.notes_in {
        Some(d) if !d.is_dir() => bail!("the folder call notes are written in ({}) isn't there — menu → Claude Code → Write call notes in", d.display()),
        Some(d) => Ok(d),
        None => Ok(paths::home()),
    }
}

impl Profile {
    /// The model for live notes and questions.
    pub fn live_model(&self) -> String {
        self.live_model.clone().or_else(|| self.notes_model.clone()).unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into())
    }

    /// The model for the minutes (None = Claude Code's own choice: `system` without
    /// `config.notes_model`, or a `local` profile naming only a live model).
    pub fn notes_model(&self) -> Option<String> {
        self.notes_model.clone().or_else(|| self.kind.signs_in().then(|| crate::config::DEFAULT_NOTES_MODEL.into()))
    }

    /// The server is this computer (`local`): `localhost`, `127.x.x.x`, `::1`.
    pub fn on_this_computer(&self) -> bool {
        self.base_url.as_deref().is_some_and(loopback)
    }

    /// Where this profile's Claude Code keeps its settings and sign-in (`local` / `claude`).
    pub fn config_folder(&self) -> PathBuf {
        paths::config_dir().join("claude").join(&self.id)
    }
}

/// Profile ids name folders: lower-case letters, digits and dashes.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && id != SYSTEM
}

/// The profile a new recording pins: the one chosen for it (`arm --ai`, the window), else its
/// meeting's, else the default now; `UNPINNED` when the AI settings can't be read (its AI jobs
/// then say so).
pub fn pin(chosen: Option<&str>, meeting: Option<&crate::meeting::MeetingContext>) -> String {
    chosen
        .map(str::to_owned)
        .or_else(|| meeting.and_then(|m| m.ai_profile.clone()))
        .filter(|p| !p.is_empty())
        .or_else(|| Settings::load().ok().map(|s| s.default_id()))
        .unwrap_or_else(|| UNPINNED.into())
}

/// `localhost`, `127.x.x.x`, `::1` in a URL: this computer.
pub fn loopback(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = if let Some(v6) = rest.strip_prefix('[') {
        v6.split(']').next().unwrap_or("")
    } else {
        rest.split(['/', ':']).next().unwrap_or("")
    };
    host.eq_ignore_ascii_case("localhost") || host.starts_with("127.") || host == "::1"
}

// ── which profile a call uses ───────────────────────────────────────────────────────────────

/// The profile id pinned in the call's `meta.json`. A readable `meta.json` without one (recorded
/// before profiles) uses the current default; one that can't be read is an error — never a guess.
pub fn call_profile_id(dir: &Path) -> Result<String> {
    let path = dir.join("meta.json");
    let read = || -> Result<Value> { Ok(serde_json::from_slice(&fs::read(&path)?)?) };
    // Writers replace it atomically; one retry covers a writer that doesn't (yet).
    let meta = read().or_else(|_| {
        std::thread::sleep(Duration::from_millis(150));
        read()
    }).with_context(|| format!("reading {}", path.display()))?;
    match meta["ai_profile"].as_str().filter(|s| !s.is_empty()) {
        Some(id) => Ok(id.to_string()),
        None => Ok(Settings::load()?.default_id()),
    }
}

/// What `set_call_profile` did.
#[derive(Debug, PartialEq)]
pub enum ProfileChange {
    /// Written to `meta.json` now.
    Applied,
    /// The call is being recorded: the watcher applies it (it owns `meta.json`) and tells the
    /// live notes.
    Requested,
}

/// Choose another profile for a call. While it records, the recording owns `meta.json`: the
/// change goes to the watcher (`ai-profile.request`); afterwards it is written here.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn set_call_profile(dir: &Path, id: &str) -> Result<ProfileChange> {
    Settings::load()?.get(id)?;
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let recording = crate::watch::running_watcher().and_then(|w| w.recording).is_some_and(|r| Path::new(&r) == dir);
    if recording {
        let req = json!({ "call": name, "profile": id });
        paths::write_atomic(&paths::state_dir().join("ai-profile.request"), req.to_string().as_bytes())?;
        return Ok(ProfileChange::Requested);
    }
    crate::calls::update_meta(dir, |meta| {
        meta["ai_profile"] = json!(id);
        Ok(())
    })?;
    Ok(ProfileChange::Applied)
}

/// The call's profile, ready to use (signed in, on the account it was set up with).
pub fn for_call(dir: &Path) -> Result<Profile> {
    let id = call_profile_id(dir)?;
    let p = Settings::load()?.get(&id)?;
    ready(&p)?;
    Ok(p)
}

// ── Claude Code with a profile ──────────────────────────────────────────────────────────────

/// Does this variable pick Claude Code's provider, endpoint, account or model? Matched by family,
/// not a fixed list: Claude Code reads dozens of them (`CLAUDE_CODE_USE_GATEWAY`, `_MANTLE`,
/// `ANTHROPIC_UNIX_SOCKET`, identity tokens, …) and adds more. A `local` or `claude` profile
/// removes every one of them, then sets its own, so nothing in the user's shell picks the
/// provider, endpoint, account or model (proxy variables are kept on purpose). The login-shell path
/// uses the same families as a `sed` filter (`UNSET_ROUTING`): change both together.
pub fn routing_var(name: &str) -> bool {
    // Windows' variable names ignore case, and so does Claude Code reading them there.
    let upper;
    let name = if cfg!(windows) {
        upper = name.to_ascii_uppercase();
        upper.as_str()
    } else {
        name
    };
    name.starts_with("ANTHROPIC_")
        || name.starts_with("CLAUDE_CODE_USE_")
        || (name.starts_with("CLAUDE_CODE_SKIP_") && name.ends_with("_AUTH"))
        || name.starts_with("CLAUDE_CODE_OAUTH_")
        || name.starts_with("CLAUDE_CODE_API_KEY_")
        || name == "CLAUDE_CONFIG_DIR"
}

/// `routing_var`'s families for the shell (`sed` over `env`), applied after the login shell's
/// start-up files ran.
const UNSET_ROUTING: &str = "for v in $(env | sed -n \
-e 's/^\\(ANTHROPIC_[A-Za-z0-9_]*\\)=.*/\\1/p' \
-e 's/^\\(CLAUDE_CODE_USE_[A-Za-z0-9_]*\\)=.*/\\1/p' \
-e 's/^\\(CLAUDE_CODE_SKIP_[A-Za-z0-9_]*_AUTH\\)=.*/\\1/p' \
-e 's/^\\(CLAUDE_CODE_OAUTH_[A-Za-z0-9_]*\\)=.*/\\1/p' \
-e 's/^\\(CLAUDE_CODE_API_KEY_[A-Za-z0-9_]*\\)=.*/\\1/p' \
-e 's/^\\(CLAUDE_CONFIG_DIR\\)=.*/\\1/p'); do unset \"$v\"; done; ";

/// The profile's own values reach the shell under this prefix and are exported under their real
/// names once the routing variables are gone (so no value is ever in the script's text or `ps`).
const PASS: &str = "ECHOCTL_P_";

/// What a `local` / `claude` profile sets for Claude Code.
pub fn routing_env(p: &Profile) -> Vec<(&'static str, String)> {
    let mut v = vec![("CLAUDE_CONFIG_DIR", p.config_folder().display().to_string())];
    if p.kind == Kind::Local {
        let model = p.notes_model().unwrap_or_else(|| p.live_model());
        v.push(("ANTHROPIC_BASE_URL", p.base_url.clone().unwrap_or_default()));
        v.push(("ANTHROPIC_AUTH_TOKEN", p.token.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "ollama".into())));
        // Whatever alias Claude Code asks for (its small background model too) is the local one.
        for k in ["ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"] {
            v.push((k, model.clone()));
        }
        v.push(("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".into()));
        // "This computer" stays on this computer, whatever proxy the environment names.
        if p.on_this_computer() {
            v.push(("NO_PROXY", "localhost,127.0.0.1,::1".into()));
            v.push(("no_proxy", "localhost,127.0.0.1,::1".into()));
        }
    }
    v
}

/// The `sh -c` script that starts Claude Code after the login shell's start-up files ran: the
/// Echo CLI first on PATH, then (managed profiles) the routing variables removed and the profile's
/// values (passed as `ECHOCTL_P_<name>`) exported.
fn shell_script(names: Option<&[&'static str]>) -> String {
    let mut s = String::from("export PATH=\"$ECHOCTL_BIN_DIR:$PATH\"; ");
    if let Some(names) = names {
        s.push_str(UNSET_ROUTING);
        for k in names {
            s.push_str(&format!("export {k}=\"${PASS}{k}\"; unset {PASS}{k}; "));
        }
    }
    s.push_str("exec \"$0\" \"$@\"");
    s
}

/// Give a child its own session (and so its own process group): stopping it stops what it
/// started, and an interactive login shell started from a terminal doesn't stop on SIGTTOU (it
/// would in a background process group of that terminal).
pub fn detach(c: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: setsid is async-signal-safe and touches nothing but the child.
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    let _ = c;
}

/// The command for Claude Code with profile `p` (built, not started; tested).
fn build_command(p: &Profile, claude: &Path, args: &[String], cwd: &Path, stdin: bool, use_shell: bool) -> Command {
    let managed = p.kind.managed();
    let env = managed.then(|| routing_env(p));
    let mut args = args.to_vec();
    // Model runs load only the profile's own settings; `claude auth …` takes no such option.
    if managed && args.first().is_none_or(|a| a != "auth") {
        args.extend(["--setting-sources".into(), "user".into()]);
    }
    let mut c = if use_shell {
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
        let names: Option<Vec<&'static str>> = env.as_ref().map(|e| e.iter().map(|(k, _)| *k).collect());
        let mut c = Command::new(shell);
        c.args(["-l", "-i", "-c", &shell_script(names.as_deref())]).arg(claude).args(&args);
        if let Some(env) = &env {
            c.envs(env.iter().map(|(k, v)| (format!("{PASS}{k}"), v)));
        }
        c
    } else {
        let mut c = Command::new(claude);
        #[cfg(windows)]
        {
            use crate::app::NoWindow;
            c = c.no_window();
        }
        c.args(&args);
        if let Some(env) = &env {
            for (k, _) in std::env::vars_os() {
                if k.to_str().is_some_and(routing_var) {
                    c.env_remove(k);
                }
            }
            c.envs(env.iter().map(|(k, v)| (*k, v)));
        }
        c
    };
    c.current_dir(cwd)
        .env(crate::notes::HEADLESS, "1")
        .env("ECHOCTL_HOME", paths::home())
        .stdin(if stdin { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Ok(exe) = std::env::current_exe() {
        if let Some(bin_dir) = exe.parent() {
            let path = std::env::var_os("PATH").unwrap_or_default();
            let joined = std::env::join_paths(std::iter::once(bin_dir.to_path_buf()).chain(std::env::split_paths(&path))).unwrap_or(path);
            c.env("ECHOCTL_BIN_DIR", bin_dir).env("PATH", joined);
        }
        c.env("ECHOCTL_BIN", exe); // the plugin's CLI launcher uses this very binary
    }
    detach(&mut c);
    c
}

/// Start Claude Code for `p`: the one way Echo runs it on call text or checks a profile's sign-in
/// (minutes, live jobs, `claude auth status`; `login`'s terminal script and `claude connect` start
/// it directly). `system` keeps the user's own setup; `local` / `claude` apply the profile on top of
/// the login shell and load only their own settings (`--setting-sources user` = their folder).
pub fn claude_command(p: &Profile, args: &[String], cwd: &Path, stdin: bool) -> Result<Child> {
    let claude = crate::app::claude_bin().ok_or_else(|| anyhow!("Claude Code is not installed — get it at https://claude.com/claude-code"))?;
    if p.kind.managed() {
        if let Some(why) = policy_conflict(p) {
            bail!("{why}");
        }
        fs::create_dir_all(p.config_folder())?;
    }
    let use_shell = cfg!(unix) && std::env::var_os("ECHOCTL_NOTES_NO_SHELL").is_none();
    Ok(build_command(p, &claude, args, cwd, stdin, use_shell).spawn()?)
}

/// Admin-managed Claude Code policy always applies, and its `env` beats the environment Echo
/// sets. If a policy source could route requests (a routing variable, an API key or policy
/// helper), or exists but can't be read, a Claude Code run for a `local` / `claude` profile is
/// refused rather than sent somewhere other than the profile shown (`local` live jobs don't start
/// Claude Code). Sources: the managed settings files, macOS managed preferences, Windows policy
/// keys (not parsed: any key refuses), and for a `claude` profile the server-managed settings
/// Claude Code cached in the profile's own folder.
pub fn policy_conflict(p: &Profile) -> Option<String> {
    const WHY: &str = "your organisation's Claude Code policy decides where requests go — use the \"Claude Code (as configured)\" profile";
    let mut sources: Vec<(String, Result<Value, String>)> = vec![];
    // A source counts unless it surely isn't there: "can't tell" refuses.
    let present = |f: &Path| !matches!(f.try_exists(), Ok(false));
    let files = match managed_files() {
        Ok(f) => f,
        Err(e) => return Some(format!("{WHY} ({e})")),
    };
    for f in files {
        if present(&f) {
            sources.push((f.display().to_string(), read_policy_json(&f)));
        }
    }
    #[cfg(target_os = "macos")]
    for f in managed_plists() {
        if present(&f) {
            let parsed = Command::new("plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&f)
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).map_err(|e| e.to_string()));
            sources.push((f.display().to_string(), parsed));
        }
    }
    #[cfg(windows)]
    for key in [r"HKLM\SOFTWARE\Policies\ClaudeCode", r"HKCU\SOFTWARE\Policies\ClaudeCode"] {
        use crate::app::NoWindow;
        match Command::new("reg").no_window().args(["query", key]).output() {
            Ok(o) if !o.status.success() => {}
            _ => return Some(format!("{WHY} ({key})")),
        }
    }
    if p.kind.signs_in() {
        let f = p.config_folder().join("remote-settings.json");
        if present(&f) {
            sources.push((f.display().to_string(), read_policy_json(&f)));
        }
    }
    for (name, v) in sources {
        match v {
            Err(_) => return Some(format!("{WHY} ({name} can't be read)")),
            Ok(v) if routes(&v) => return Some(format!("{WHY} ({name})")),
            Ok(_) => {}
        }
    }
    None
}

fn read_policy_json(f: &Path) -> Result<Value, String> {
    fs::read(f).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
}

/// A policy document that can decide the route: a routing variable in its `env`, or a helper.
/// Server-managed settings may wrap the settings in a `settings` object.
fn routes(v: &Value) -> bool {
    let doc = |v: &Value| {
        v["env"].as_object().is_some_and(|env| env.keys().any(|k| routing_var(k)))
            || v.get("apiKeyHelper").is_some()
            || v.get("policyHelper").is_some()
            || v.get("policyHelpers").is_some()
    };
    doc(v) || doc(&v["settings"])
}

/// The managed settings files; an error when the drop-in folder exists but can't be listed.
fn managed_files() -> Result<Vec<PathBuf>, String> {
    let base = PathBuf::from(if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode"
    } else if cfg!(windows) {
        r"C:\Program Files\ClaudeCode"
    } else {
        "/etc/claude-code"
    });
    let mut files = vec![base.join("managed-settings.json")];
    let dropins = base.join("managed-settings.d");
    match fs::read_dir(&dropins) {
        Ok(rd) => {
            let mut more: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
            more.sort();
            files.extend(more);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{} can't be read: {e}", dropins.display())),
    }
    Ok(files)
}

#[cfg(target_os = "macos")]
fn managed_plists() -> Vec<PathBuf> {
    let root = Path::new("/Library/Managed Preferences");
    let user = std::env::var("USER").ok().filter(|u| !u.is_empty()).or_else(|| {
        std::env::var_os("HOME").map(PathBuf::from).and_then(|h| h.file_name().map(|n| n.to_string_lossy().into_owned()))
    });
    let mut v = vec![root.join("com.anthropic.claudecode.plist")];
    if let Some(u) = user {
        v.push(root.join(u).join("com.anthropic.claudecode.plist"));
    }
    v
}

/// Stop a child started by `claude_command` or `detach`ed, and everything in its group: TERM, then
/// KILL to whatever of the group is left after 3 s — the leader exiting first doesn't end the
/// watch (a descendant may ignore TERM and hold the pipes). Reaps the leader.
pub fn stop_child(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id();
        let child = std::sync::Mutex::new(child);
        stop_group_with(pid, &|| {
            let _ = child.lock().unwrap_or_else(|e| e.into_inner()).try_wait(); // reap the leader once it exits
        });
        let c = child.into_inner().unwrap_or_else(|e| e.into_inner());
        let _ = c.kill();
        let _ = c.wait();
    }
    #[cfg(windows)]
    {
        stop_group(child.id());
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Stop process group `pid` (a child of ours started as its leader): TERM, wait up to 3 s for the
/// whole group to be gone, then KILL what is left. On Windows the tree (`taskkill /T /F`).
pub fn stop_group(pid: u32) {
    #[cfg(unix)]
    stop_group_with(pid, &|| {});
    #[cfg(windows)]
    {
        use crate::app::NoWindow;
        let _ = Command::new("taskkill").no_window().args(["/PID", &pid.to_string(), "/T", "/F"]).status();
    }
}

#[cfg(unix)]
fn stop_group_with(pid: u32, reap: &dyn Fn()) {
    let group = -(pid as i32);
    // SAFETY: signalling a process group we created for a child of ours.
    unsafe {
        libc::kill(group, libc::SIGTERM);
    }
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(3) {
        reap();
        // No member left (an unreaped leader still counts, which `reap` takes care of).
        if unsafe { libc::kill(group, 0) } != 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    unsafe {
        libc::kill(group, libc::SIGKILL);
    }
    // KILLed members linger a moment as zombies until reaped (orphans by init).
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(1) {
        reap();
        if unsafe { libc::kill(group, 0) } != 0 {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

// ── sign-in ─────────────────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub email: String,
    pub org: String,
    pub subscription: String,
}

impl Account {
    /// Who it is, for comparing: email · organisation, lower-case (a plan change keeps it).
    pub fn key(&self) -> String {
        if self.org.is_empty() || self.org.to_lowercase().contains(&self.email.to_lowercase()) {
            self.email.to_lowercase()
        } else {
            format!("{} · {}", self.email.to_lowercase(), self.org.to_lowercase())
        }
    }

    /// For people: the key plus the plan.
    pub fn label(&self) -> String {
        let sub = if self.subscription.is_empty() { String::new() } else { format!(" ({})", self.subscription) };
        format!("{}{sub}", self.key())
    }
}

/// Who a `claude` profile's folder is signed in to (`claude auth status`); None = signed out.
pub fn account(p: &Profile) -> Result<Option<Account>> {
    let mut child = claude_command(p, &["auth".into(), "status".into()], &paths::config_dir(), false)?;
    let (out, err, _) = wait_output(&mut child, Duration::from_secs(20), &|| false)
        .with_context(|| format!("checking the sign-in of {}", p.name))?;
    // The JSON ends the output; the login shell's start-up may print a `{` of its own before it.
    let v: Value = out.match_indices('{').find_map(|(i, _)| serde_json::from_str(out[i..].trim()).ok()).ok_or_else(|| {
        let why = err.lines().chain(out.lines()).map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("no output").to_string();
        anyhow!("unexpected answer from `claude auth status` for {}: {why}", p.name)
    })?;
    if v["loggedIn"].as_bool() != Some(true) {
        return Ok(None);
    }
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let a = Account { email: s("email"), org: s("orgName"), subscription: s("subscriptionType") };
    if a.email.is_empty() {
        bail!("`claude auth status` for {} names no account", p.name);
    }
    Ok(Some(a))
}

/// Run a child to the end: (stdout, stderr, exited successfully). Stopped (TERM, KILL after 3 s,
/// reaped) at the deadline or as soon as `cancelled()` says so; both are errors.
pub fn wait_output(child: &mut Child, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<(String, String, bool)> {
    let (out, err, ok, _) = wait_output_with(child, None, deadline, cancelled)?;
    Ok((out, err, ok))
}

/// `wait_output`, also handing `input` to the child's stdin (on a thread of its own: a child that
/// never reads it is supervised all the same); the last value says whether the input got through.
pub fn wait_output_with(child: &mut Child, input: Option<Vec<u8>>, deadline: Duration, cancelled: &dyn Fn() -> bool) -> Result<(String, String, bool, std::io::Result<()>)> {
    use std::io::{Read, Write};
    let stdin = child.stdin.take();
    let writer = std::thread::spawn(move || match (stdin, input) {
        (Some(mut i), Some(input)) => i.write_all(&input), // dropped after: closed, the input is complete
        _ => Ok(()),
    });
    fn drain(r: Option<impl Read + Send + 'static>) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            String::from_utf8_lossy(&b).into_owned()
        })
    }
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let t = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if cancelled() {
            stop_child(child);
            bail!("stopped");
        }
        if t.elapsed() > deadline {
            stop_child(child);
            bail!("no answer within {} s", deadline.as_secs());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    // The leader is gone; something it started may still hold the pipes (a background process
    // from shell start-up): its stdout / stderr, or its stdin with the input half written. Give
    // them 2 s to end — still under the deadline and stopping — then stop what is left of the group,
    // which closes them.
    let exited = Instant::now();
    while !(out.is_finished() && err.is_finished() && writer.is_finished()) {
        let give_up = cancelled() || t.elapsed() > deadline || exited.elapsed() > Duration::from_secs(2);
        if give_up {
            stop_group(child.id());
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let wrote = writer.join().unwrap_or(Ok(()));
    Ok((out.join().unwrap_or_default(), err.join().unwrap_or_default(), status.success(), wrote))
}

/// Recent sign-in checks (`claude auth status` takes about a second), per profile id: when, and
/// the account (or why it can't be used). A success is kept 30 s, a failure 10 s.
type Checks = HashMap<String, (Instant, Result<String, String>)>;
static CHECKED: Mutex<Option<Checks>> = Mutex::new(None);

/// Can a job run on `p` now? `local`: an http(s) server and a model. `claude`: signed in, and to the
/// account the profile was first used with (recorded then).
pub fn ready(p: &Profile) -> Result<()> {
    if p.kind == Kind::Local {
        let url = p.base_url.as_deref().unwrap_or("");
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("the AI profile \"{}\" has no http(s) server address", p.name);
        }
        if p.live_model.is_none() && p.notes_model.is_none() {
            bail!("the AI profile \"{}\" names no model", p.name);
        }
    }
    if !p.kind.signs_in() {
        return Ok(());
    }
    if let Some((at, r)) = CHECKED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(&p.id) {
        let ttl = Duration::from_secs(if r.is_ok() { 30 } else { 10 });
        if at.elapsed() < ttl {
            return r.clone().map(|_| ()).map_err(|e| anyhow!(e));
        }
    }
    let r = check_account(p).map_err(|e| format!("{e:#}"));
    CHECKED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(p.id.clone(), (Instant::now(), r.clone()));
    r.map(|_| ()).map_err(|e| anyhow!(e))
}

/// Forget the last sign-in check (after signing in or accepting another account).
pub fn forget_check(id: &str) {
    if let Some(m) = CHECKED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(id);
    }
}

/// What a sign-in check says (`None` = signed out): the account's key when it may be used, and
/// whether it should be recorded as the profile's (its first use).
fn judge(p: &Profile, a: Option<&Account>) -> Result<(String, bool)> {
    let Some(a) = a else {
        bail!("{} isn't signed in — menu → AI for calls → Sign in…", p.name);
    };
    let key = a.key();
    match &p.account {
        Some(want) if want.to_lowercase() != key => bail!(
            "{} is signed in as {key}, not {want} — menu → AI for calls → Use the account signed in now, or sign in again",
            p.name
        ),
        Some(_) => Ok((key, false)),
        None => Ok((key, true)),
    }
}

fn check_account(p: &Profile) -> Result<String> {
    let a = account(p)?;
    let (key, first) = judge(p, a.as_ref())?;
    if first {
        // First use: remember the account, so a later change is noticed (unless another check
        // recorded one meanwhile).
        Settings::update(|s| {
            if let Some(stored) = s.profiles.iter_mut().find(|x| x.id == p.id && x.account.is_none()) {
                stored.account = Some(key.clone());
            }
            Ok(())
        })?;
    }
    Ok(key)
}

/// Accept the account a `claude` profile's folder is signed in to now (checked first, then
/// recorded: the check takes seconds and must not hold or overwrite the settings).
pub fn accept_account(id: &str) -> Result<String> {
    let p = Settings::load()?.get(id)?;
    let a = account(&p)?.ok_or_else(|| anyhow!("{} isn't signed in", p.name))?;
    Settings::update(|s| {
        let stored = s.profiles.iter_mut().find(|x| x.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        stored.account = Some(a.key());
        Ok(())
    })?;
    forget_check(id);
    Ok(a.label())
}

/// One argument for `sh`: single-quoted.
#[cfg(unix)]
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Open a terminal that signs a `claude` profile's folder in (`claude auth login`).
pub fn login(p: &Profile) -> Result<()> {
    if !p.kind.signs_in() {
        bail!("only a Claude account profile signs in");
    }
    let claude = crate::app::claude_bin().ok_or_else(|| anyhow!("Claude Code is not installed"))?;
    fs::create_dir_all(p.config_folder())?;
    forget_check(&p.id);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = p.config_folder().with_extension("login.command");
        let mut s = String::from("#!/bin/sh\n");
        s.push_str(UNSET_ROUTING.trim_end());
        s.push('\n');
        s.push_str(&format!("echo {}\n", sh_quote(&format!("Echo: sign in for \"{}\"", p.name))));
        s.push_str(&format!("CLAUDE_CONFIG_DIR={} {} auth login\n", sh_quote(&p.config_folder().display().to_string()), sh_quote(&claude.display().to_string())));
        s.push_str("echo; echo 'Done — you can close this window.'\n");
        fs::write(&script, s)?;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700))?;
        #[cfg(target_os = "macos")]
        Command::new("open").args(["-a", "Terminal"]).arg(&script).spawn()?;
        #[cfg(not(target_os = "macos"))]
        Command::new("x-terminal-emulator").arg("-e").arg(&script).spawn()?;
    }
    #[cfg(windows)]
    {
        let script = p.config_folder().with_extension("login.cmd");
        let mut s = String::from("@echo off\r\n");
        for (k, _) in std::env::vars() {
            if routing_var(&k) {
                s.push_str(&format!("set {k}=\r\n"));
            }
        }
        s.push_str(&format!("echo Echo: sign in for \"{}\"\r\n", p.name.replace(['"', '%', '^', '&', '|', '<', '>'], "")));
        s.push_str(&format!("set \"CLAUDE_CONFIG_DIR={}\"\r\n", p.config_folder().display()));
        s.push_str(&format!("\"{}\" auth login\r\npause\r\n", claude.display()));
        fs::write(&script, s)?;
        Command::new("cmd").args(["/c", "start", "", &script.display().to_string()]).spawn()?;
    }
    Ok(())
}

// ── `echoctl ai …` ──────────────────────────────────────────────────────────────────────────

/// `echoctl ai default <id>`
pub fn set_default(id: &str) -> Result<()> {
    let name = Settings::update(|s| {
        let name = s.get(id)?.name;
        s.default = (id != SYSTEM).then(|| id.to_string());
        Ok(name)
    })?;
    println!("new calls use {name}");
    Ok(())
}

/// `echoctl ai live on|off`
pub fn set_live(on: bool) -> Result<()> {
    Settings::update(|s| {
        s.live_notes = on;
        Ok(())
    })?;
    println!("live notes during calls: {}", if on { "on" } else { "off" });
    Ok(())
}

fn add(p: Profile) -> Result<()> {
    if !valid_id(&p.id) {
        bail!("a profile id is lower-case letters, digits and dashes (and not \"system\")");
    }
    let (id, name) = (p.id.clone(), p.name.clone());
    Settings::update(|s| {
        if s.profiles.iter().any(|x| x.id == p.id) {
            bail!("there is already a profile \"{}\" — `echoctl ai remove {}` first", p.id, p.id);
        }
        s.profiles.push(p);
        Ok(())
    })?;
    println!("added {name} ({id})");
    Ok(())
}

/// `echoctl ai add-local <id> --model … [--live-model …] [--url …] [--token …] [--name …]`
pub fn add_local(id: &str, name: Option<String>, model: &str, live_model: Option<String>, url: &str, token: Option<String>) -> Result<()> {
    let url = url.trim().trim_end_matches('/').to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("the server address starts with http:// or https:// (Ollama: http://localhost:11434)");
    }
    add(Profile {
        id: id.into(),
        name: name.unwrap_or_else(|| format!("{model} · this computer")),
        kind: Kind::Local,
        base_url: Some(url),
        token,
        live_model: live_model.or_else(|| Some(model.to_string())),
        notes_model: Some(model.to_string()),
        account: None,
    })
}

/// `echoctl ai add-claude <id> [--name …] [--live-model haiku] [--notes-model sonnet]`
pub fn add_claude(id: &str, name: Option<String>, live_model: Option<String>, notes_model: Option<String>) -> Result<()> {
    add(Profile {
        id: id.into(),
        name: name.unwrap_or_else(|| format!("Claude · {id}")),
        kind: Kind::Claude,
        base_url: None,
        token: None,
        live_model: Some(live_model.unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into())),
        notes_model: Some(notes_model.unwrap_or_else(|| crate::config::DEFAULT_NOTES_MODEL.into())),
        account: None,
    })?;
    println!("sign it in once: echoctl ai login {id}");
    Ok(())
}

/// `echoctl ai remove <id>` (its Claude Code folder, with the sign-in, is kept).
pub fn remove(id: &str) -> Result<()> {
    let was_default = Settings::update(|s| {
        let before = s.profiles.len();
        s.profiles.retain(|p| p.id != id);
        if s.profiles.len() == before {
            bail!("no profile \"{id}\"");
        }
        let was = s.default.as_deref() == Some(id);
        if was {
            s.default = None;
        }
        Ok(was)
    })?;
    if was_default {
        println!("new calls now use Claude Code (as configured)");
    }
    println!("removed {id}");
    Ok(())
}

/// `echoctl ai status [<id>]`: each profile, its account (Claude profiles sign-in checked now).
pub fn status(id: Option<&str>) -> Result<()> {
    let s = Settings::load()?;
    let mut out = vec![];
    for p in s.all().into_iter().filter(|p| id.is_none_or(|i| i == p.id)) {
        let (signed_in, account, problem) = match p.kind {
            Kind::Claude => {
                forget_check(&p.id);
                match account(&p) {
                    Ok(Some(a)) => (Some(true), Some(a.label()), ready(&p).err().map(|e| format!("{e:#}"))),
                    Ok(None) => (Some(false), None, None),
                    Err(e) => (None, None, Some(format!("{e:#}"))),
                }
            }
            _ => (None, None, ready(&p).err().map(|e| format!("{e:#}"))),
        };
        let conflict = (p.kind != Kind::System).then(|| policy_conflict(&p)).flatten();
        out.push(json!({
            "id": p.id, "name": p.name, "kind": p.kind, "default": s.default_id() == p.id,
            "live_model": p.live_model(), "notes_model": p.notes_model(), "base_url": p.base_url,
            "signed_in": signed_in, "account": account, "expected_account": p.account,
            "problem": problem.or(conflict),
        }));
    }
    println!("{}", serde_json::to_string_pretty(&json!({ "live_notes": s.live_notes, "profiles": out }))?);
    Ok(())
}

/// For the window.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn summary() -> Value {
    match Settings::load() {
        Ok(s) => json!({
            "default": s.default_id(),
            "live_notes": s.live_notes,
            "profiles": s.all().iter().map(|p| json!({
                "id": p.id, "name": p.name, "kind": p.kind, "live_model": p.live_model(),
                "notes_model": p.notes_model(), "account": p.account,
                // `local`: where the requests go when it isn't this computer
                "host": (p.kind == Kind::Local && !p.on_this_computer()).then(|| p.base_url.clone()).flatten(),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e:#}") }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_script_removes_routing_and_passes_values_outside_its_text() {
        let env = [("CLAUDE_CONFIG_DIR", "/Users/me/Library/Application Support/echo/claude/a".to_string()), ("ANTHROPIC_AUTH_TOKEN", "it's \"secret\"".to_string())];
        // `system`: the user's environment as it is.
        assert_eq!(shell_script(None), "export PATH=\"$ECHOCTL_BIN_DIR:$PATH\"; exec \"$0\" \"$@\"");
        let names: Vec<&'static str> = env.iter().map(|(k, _)| *k).collect();
        let script = shell_script(Some(&names));
        assert!(!script.contains("secret"), "values never in the script's text (or `ps`)");
        #[cfg(unix)]
        // Run it in each shell a user may have: what the started program sees.
        for sh in ["/bin/sh", "/bin/zsh", "/bin/bash"].into_iter().filter(|s| Path::new(s).exists()) {
            let mut c = Command::new(sh);
            c.args(["-c", &script, "/usr/bin/env"])
                .env("ECHOCTL_BIN_DIR", "/echo/bin")
                .env("ANTHROPIC_BASE_URL", "https://gateway.example")
                .env("ANTHROPIC_UNIX_SOCKET", "/tmp/s")
                .env("CLAUDE_CODE_USE_GATEWAY", "1")
                .env("CLAUDE_CODE_SKIP_BEDROCK_AUTH", "1")
                .env("CLAUDE_CODE_OAUTH_TOKEN", "t")
                .env("CLAUDE_CONFIG_DIR", "/old")
                .env("HTTPS_PROXY", "http://proxy:1");
            c.envs(env.iter().map(|(k, v)| (format!("{PASS}{k}"), v)));
            let out = c.output().unwrap();
            let seen = String::from_utf8_lossy(&out.stdout);
            let vars: Vec<&str> = seen.lines().collect();
            assert!(vars.contains(&"CLAUDE_CONFIG_DIR=/Users/me/Library/Application Support/echo/claude/a"), "{sh}: {seen}");
            assert!(vars.contains(&"ANTHROPIC_AUTH_TOKEN=it's \"secret\""), "{sh}");
            assert!(vars.contains(&"HTTPS_PROXY=http://proxy:1"));
            assert!(vars.iter().any(|v| v.starts_with("PATH=/echo/bin:")));
            assert!(!vars.iter().any(|v| v.starts_with(PASS)), "{sh}: the pass-through names are gone");
            for gone in ["ANTHROPIC_BASE_URL", "ANTHROPIC_UNIX_SOCKET", "CLAUDE_CODE_USE_GATEWAY", "CLAUDE_CODE_SKIP_BEDROCK_AUTH", "CLAUDE_CODE_OAUTH_TOKEN"] {
                assert!(!vars.iter().any(|v| v.starts_with(&format!("{gone}="))), "{sh}: {gone} left: {seen}");
            }
        }
    }

    #[test]
    fn the_command_without_a_shell_routes_by_the_profile() {
        let (_env, root) = crate::paths::testenv::folders("ai-command");
        // The user's own routing, in the app's environment (removed again even if this fails).
        const USERS: [&str; 4] = ["ANTHROPIC_BASE_URL", "ANTHROPIC_API_KEY", "CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_OAUTH_TOKEN"];
        struct Unset;
        impl Drop for Unset {
            fn drop(&mut self) {
                USERS.iter().for_each(|k| std::env::remove_var(k));
            }
        }
        let _unset = Unset;
        USERS.iter().for_each(|k| std::env::set_var(k, "the user's own"));
        std::env::set_var("ANTHROPIC_BASE_URL", "https://gateway.example");
        let local = Profile { id: "q".into(), name: "Q".into(), kind: Kind::Local, base_url: Some("http://localhost:11434".into()), token: None, live_model: Some("m".into()), notes_model: Some("m".into()), account: None };
        let c = build_command(&local, Path::new("/bin/claude"), &["-p".into()], &root, false, false);
        let envs: HashMap<String, Option<String>> = c.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect();
        assert_eq!(envs.get("ANTHROPIC_BASE_URL"), Some(&Some("http://localhost:11434".into())), "the profile's, not the shell's");
        assert_eq!(envs.get("ANTHROPIC_DEFAULT_HAIKU_MODEL"), Some(&Some("m".into())));
        assert_eq!(envs.get("NO_PROXY"), Some(&Some("localhost,127.0.0.1,::1".into())));
        for k in &USERS[1..] {
            assert_eq!(envs.get(*k), Some(&None), "{k}: removed for a local profile");
        }
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["-p", "--setting-sources", "user"]);
        let claude = Profile { id: "c".into(), name: "C".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None };
        let c = build_command(&claude, Path::new("/bin/claude"), &["-p".into()], &root, false, false);
        let envs: HashMap<String, Option<String>> = c.get_envs().map(|(k, v)| (k.to_string_lossy().into_owned(), v.map(|v| v.to_string_lossy().into_owned()))).collect();
        for k in USERS {
            assert_eq!(envs.get(k), Some(&None), "{k}: removed for a Claude account");
        }
        assert_eq!(envs.get("CLAUDE_CONFIG_DIR"), Some(&Some(claude.config_folder().display().to_string())));
        let auth = build_command(&local, Path::new("/bin/claude"), &["auth".into(), "status".into()], &root, false, false);
        assert_eq!(auth.get_args().count(), 2, "`claude auth` takes no --setting-sources");
        let system = build_command(&system_profile(), Path::new("/bin/claude"), &["-p".into()], &root, false, false);
        assert!(!system.get_envs().any(|(k, _)| k == "ANTHROPIC_BASE_URL"), "system: the user's own setup");
        assert_eq!(system.get_args().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn the_login_shell_command_starts_claude_with_the_profiles_routing() {
        let (_env, root) = crate::paths::testenv::folders("ai-shell-command");
        // What Claude Code would get: its arguments and environment.
        let stand_in = root.join("claude");
        fs::write(&stand_in, "#!/bin/sh\necho \"ARGS $*\"\nenv\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&stand_in, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let p = Profile { id: "c".into(), name: "C".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None };
        let mut c = build_command(&p, &stand_in, &["-p".into()], &root, false, true);
        // The user's login shell (no start-up files of theirs here) and their own routing.
        c.env("HOME", &root).env_remove("ZDOTDIR").env("ANTHROPIC_API_KEY", "the user's own").env("CLAUDE_CODE_USE_BEDROCK", "1");
        detach(&mut c);
        let out = c.output().unwrap();
        let seen = String::from_utf8_lossy(&out.stdout);
        let vars: Vec<&str> = seen.lines().collect();
        assert!(vars.contains(&"ARGS -p --setting-sources user"), "{seen}");
        assert!(vars.contains(&format!("CLAUDE_CONFIG_DIR={}", p.config_folder().display()).as_str()), "{seen}");
        assert!(vars.contains(&"ECHOCTL_HEADLESS=1"), "{seen}");
        for gone in ["ANTHROPIC_API_KEY=", "CLAUDE_CODE_USE_BEDROCK=", PASS] {
            assert!(!vars.iter().any(|v| v.starts_with(gone)), "{gone} left: {seen}");
        }
    }

    #[test]
    fn changes_made_at_once_never_undo_each_other() {
        let (_env, _root) = crate::paths::testenv::folders("ai-update-race");
        let writer = |id: &'static str| {
            std::thread::spawn(move || {
                for i in 0..20 {
                    Settings::update(|s| {
                        s.profiles.retain(|p| p.id != id);
                        s.profiles.push(Profile { id: id.into(), name: format!("{id} {i}"), kind: Kind::Local, base_url: Some("http://localhost:1".into()), token: None, live_model: Some("m".into()), notes_model: Some("m".into()), account: None });
                        Ok(())
                    })
                    .unwrap();
                }
            })
        };
        let (a, b) = (writer("a"), writer("b"));
        a.join().unwrap();
        b.join().unwrap();
        let mut names: Vec<String> = Settings::load().unwrap().profiles.into_iter().map(|p| p.name).collect();
        names.sort();
        assert_eq!(names, ["a 19", "b 19"]);
        // A default that names no profile is refused (a menu offered one removed since).
        assert!(Settings::update(|s| {
            s.default = Some("gone".into());
            Ok(())
        })
        .is_err());
    }

    #[test]
    fn the_account_is_who_not_which_plan() {
        let p = |account: Option<&str>| Profile { id: "w".into(), name: "Work".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: account.map(str::to_owned) };
        let a = |sub: &str| Account { email: "Pat@Example.com".into(), org: "Example Corp".into(), subscription: sub.into() };
        assert!(format!("{:#}", judge(&p(None), None).unwrap_err()).contains("isn't signed in"));
        assert_eq!(judge(&p(None), Some(&a("pro"))).unwrap(), ("pat@example.com · example corp".into(), true), "first use: recorded");
        assert!(!judge(&p(Some("pat@example.com · example corp")), Some(&a("max"))).unwrap().1, "a plan change is the same account");
        assert!(judge(&p(Some("someone@else.com")), Some(&a("max"))).is_err());
        assert_eq!(Account { email: "a@b.c".into(), org: "a@b.c's Organization".into(), subscription: "max".into() }.label(), "a@b.c (max)");
    }

    #[test]
    fn settings_are_checked_and_saved_privately() {
        let (_env, root) = crate::paths::testenv::folders("ai-settings");
        let f = root.join("config").join(FILE);
        assert_eq!(Settings::load().unwrap().all().len(), 1, "no file: system only");
        fs::write(&f, r#"{"profiles":[{"id":"system","name":"x","kind":"local"}]}"#).unwrap();
        assert!(Settings::load().is_err(), "system is built in");
        fs::write(&f, r#"{"profiles":[{"id":"a","name":"A","kind":"local"},{"id":"a","name":"B","kind":"claude"}]}"#).unwrap();
        assert!(Settings::load().is_err(), "duplicate ids");
        fs::write(&f, r#"{"profiles":[{"id":"../x","name":"A","kind":"local"}]}"#).unwrap();
        assert!(Settings::load().is_err(), "an id names a folder");
        let broken = fs::read(&f).unwrap();
        assert!(set_live(true).is_err());
        assert_eq!(fs::read(&f).unwrap(), broken, "nothing saved over a file that can't be read");
        fs::remove_file(&f).unwrap();
        add_local("q", None, "m", None, "http://localhost:11434", Some("tok".into())).unwrap();
        set_default("q").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600, "it can hold a token");
        }
        remove("q").unwrap();
        assert_eq!(Settings::load().unwrap().default_id(), SYSTEM, "removing the default resets it");
    }

    #[test]
    fn a_cached_server_policy_that_routes_refuses_the_profile() {
        let (_env, _root) = crate::paths::testenv::folders("ai-policy");
        #[cfg(target_os = "macos")]
        let plist = managed_plists().iter().any(|p| p.exists());
        #[cfg(not(target_os = "macos"))]
        let plist = false;
        if managed_files().is_ok_and(|f| f.iter().any(|p| p.exists())) || plist {
            eprintln!("skipped: this machine has a real Claude Code policy, and the result would be about it");
            return;
        }
        let p = Profile { id: "w".into(), name: "Work".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None };
        fs::create_dir_all(p.config_folder()).unwrap();
        let f = p.config_folder().join("remote-settings.json");
        fs::write(&f, r#"{"permissions":{"deny":["WebFetch"]}}"#).unwrap();
        #[cfg(not(windows))]
        assert_eq!(policy_conflict(&p), None);
        fs::write(&f, r#"{"env":{"ANTHROPIC_BASE_URL":"https://gw.example"}}"#).unwrap();
        assert!(policy_conflict(&p).is_some());
        fs::write(&f, "{ broken").unwrap();
        assert!(policy_conflict(&p).unwrap().contains("can't be read"));
    }

    #[cfg(unix)]
    #[test]
    fn output_held_by_a_leftover_process_doesnt_hang_the_wait() {
        // The leader prints and exits; a process it started keeps stdout open for 30 s.
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "sleep 30 & echo hi"]).stdout(Stdio::piped()).stderr(Stdio::piped());
        detach(&mut c);
        let mut child = c.spawn().unwrap();
        let group = -(child.id() as i32);
        let t = Instant::now();
        let (out, _, ok) = wait_output(&mut child, Duration::from_secs(60), &|| false).unwrap();
        assert_eq!((out.trim(), ok), ("hi", true));
        assert!(t.elapsed() < Duration::from_secs(10), "took {:?}", t.elapsed());
        assert_ne!(unsafe { libc::kill(group, 0) }, 0, "the leftover was stopped");
    }

    #[cfg(unix)]
    #[test]
    fn input_held_by_a_leftover_process_doesnt_hang_the_wait() {
        // The leader exits without reading; a process it started holds stdin (only) and never reads.
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "sleep 30 <&0 >/dev/null 2>&1 & echo hi"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        detach(&mut c);
        let mut child = c.spawn().unwrap();
        let group = -(child.id() as i32);
        let big = "привіт, ".repeat(10_000).into_bytes(); // well over a pipe's capacity
        let t = Instant::now();
        let (out, _, _, wrote) = wait_output_with(&mut child, Some(big), Duration::from_secs(60), &|| false).unwrap();
        assert_eq!(out.trim(), "hi");
        assert!(wrote.is_err(), "the input couldn't all get through");
        assert!(t.elapsed() < Duration::from_secs(10), "took {:?}", t.elapsed());
        assert_ne!(unsafe { libc::kill(group, 0) }, 0, "the leftover was stopped");
    }

    #[cfg(unix)]
    #[test]
    fn stopping_a_child_stops_its_whole_group() {
        // The leader exits on TERM; its child ignores TERM (and would hold the pipes).
        let ready = std::env::temp_dir().join(format!("echo-test-trap-{}", std::process::id()));
        let _ = fs::remove_file(&ready);
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "(trap '' TERM; touch \"$0\"; exec sleep 30) & wait"]).arg(&ready).stdout(Stdio::piped());
        detach(&mut c);
        let mut child = c.spawn().unwrap();
        let group = -(child.id() as i32);
        let t = Instant::now();
        while !ready.exists() {
            assert!(t.elapsed() < Duration::from_secs(10), "the child never started");
            std::thread::sleep(Duration::from_millis(20));
        }
        let t = Instant::now();
        stop_child(&mut child);
        let _ = fs::remove_file(&ready);
        assert_ne!(unsafe { libc::kill(group, 0) }, 0, "nothing of the group is left");
        assert!(t.elapsed() >= Duration::from_secs(2), "the child ignoring TERM took KILL");
    }

    #[test]
    fn routing_variables_by_family() {
        for v in ["ANTHROPIC_API_KEY", "ANTHROPIC_DEFAULT_FABLE_MODEL", "CLAUDE_CODE_USE_MANTLE", "CLAUDE_CODE_SKIP_VERTEX_AUTH", "CLAUDE_CODE_OAUTH_REFRESH_TOKEN", "CLAUDE_CODE_API_KEY_FILE_DESCRIPTOR", "CLAUDE_CONFIG_DIR"] {
            assert!(routing_var(v), "{v}");
        }
        for v in ["PATH", "HOME", "HTTPS_PROXY", "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "CLAUDE_CODE_SKIP_PROMPT_HISTORY", "ECHOCTL_HOME"] {
            assert!(!routing_var(v), "{v}");
        }
    }

    #[test]
    fn policies_that_route_are_found() {
        assert!(routes(&json!({"env": {"ANTHROPIC_BASE_URL": "https://gw"}})));
        assert!(routes(&json!({"apiKeyHelper": "/bin/key"})));
        assert!(routes(&json!({"policyHelper": {"command": "x"}})));
        assert!(routes(&json!({"policyHelpers": {"darwin": {"command": "x"}}})));
        assert!(routes(&json!({"settings": {"env": {"CLAUDE_CODE_USE_BEDROCK": "1"}}})));
        assert!(!routes(&json!({"env": {"DISABLE_TELEMETRY": "1"}, "permissions": {"deny": ["WebFetch"]}})));
    }

    #[test]
    fn a_call_uses_its_pinned_profile_or_none() {
        let (_env, root) = crate::paths::testenv::folders("ai-call");
        let call = root.join("home/calls/2026-10-23_090000-x");
        fs::create_dir_all(&call).unwrap();
        fs::write(call.join("meta.json"), "{}").unwrap();
        // Unpinned (recorded before profiles): the default — which must exist.
        Settings { default: Some("gone".into()), live_notes: true, profiles: vec![] }.save().unwrap();
        assert!(format!("{:#}", for_call(&call).unwrap_err()).contains("doesn't exist"));
        set_call_profile(&call, SYSTEM).unwrap();
        assert_eq!(for_call(&call).unwrap().kind, Kind::System);
        // A meta.json that can't be read: an error, not the default.
        fs::write(call.join("meta.json"), "{ half wri").unwrap();
        assert!(for_call(&call).is_err());
        // A profile removed after the call pinned it: an error, not another profile.
        fs::write(call.join("meta.json"), r#"{"ai_profile": "local-old"}"#).unwrap();
        assert!(for_call(&call).is_err());
        // Unreadable settings: an error too.
        fs::write(root.join("config").join(FILE), "{ not json").unwrap();
        assert!(for_call(&call).is_err());
    }

    #[test]
    fn missing_profiles_are_errors_not_fallbacks() {
        let (_env, _root) = crate::paths::testenv::folders("ai-missing"); // system reads the data folder's settings
        let s = Settings { default: Some("gone".into()), live_notes: true, profiles: vec![] };
        assert!(s.get(UNPINNED).is_err());
        assert!(s.get("gone").is_err());
        assert_eq!(s.get(SYSTEM).unwrap().kind, Kind::System);
    }

    #[test]
    fn ids() {
        assert!(valid_id("claude-enterprise") && valid_id("local-qwen"));
        assert!(!valid_id("system") && !valid_id("../x") && !valid_id("A") && !valid_id(""));
    }
}

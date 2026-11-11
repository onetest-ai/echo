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

mod checks;
mod command;
mod policy;
mod signin;
pub use checks::*;
pub use command::*;
pub use policy::*;
pub use signin::*;

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
    /// organisation); another one later stops jobs until it is accepted (Use <account> in the window
    /// or the tray, `echoctl ai accept`; a plan change doesn't count)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// claude: pre-filled on the sign-in page (else the recorded account's email)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// claude: the organisation signs in with single sign-on (`claude auth login --sso`)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sso: bool,
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
            anyhow!("the AI profile \"{id}\" doesn't exist on this computer — choose another for this call in the Echo window (Live tab or Write minutes)")
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
        email: None,
        sso: false,
    }
}

/// Where `system` runs its jobs: `notes_in`, else the data folder. Strict: that folder's
/// `.claude/settings.json` may route it (a model on this computer), so a `config.json` that can't
/// be read, or a `notes_in` that isn't a folder, fails the job instead of running it elsewhere.
pub fn system_folder() -> Result<PathBuf> {
    let cfg = Config::load().map_err(|e| anyhow!("the data folder's settings (config.json) can't be read: {e:#}"))?;
    match cfg.notes_in {
        Some(d) if !d.is_dir() => bail!("the folder Echo runs Claude Code in for minutes ({}) isn't there — menu → Claude Code → Echo runs Claude Code for minutes in", d.display()),
        Some(d) => Ok(d),
        None => Ok(paths::home()),
    }
}

impl Profile {
    /// The model for live notes and questions.
    pub fn live_model(&self) -> String {
        self.live_model.clone().or_else(|| self.notes_model.clone()).unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into())
    }

    /// The model that writes the minutes — what the run uses, the menu and the window show, and the
    /// minutes record: `local` its minutes model, else its live model; `claude` its own, else
    /// sonnet; `system` `config.notes_model_arg()` (None = Claude Code's own choice: "default").
    pub fn notes_model(&self) -> Option<String> {
        match self.kind {
            Kind::Local => self.notes_model.clone().or_else(|| self.live_model.clone()),
            _ => self.notes_model.clone().or_else(|| self.kind.signs_in().then(|| crate::config::DEFAULT_NOTES_MODEL.into())),
        }
    }

    /// `notes_model()` in words.
    pub fn notes_model_label(&self) -> String {
        self.notes_model().unwrap_or_else(|| "Claude Code's default model".into())
    }

    /// The server is this computer (`local`): `localhost`, `127.x.x.x`, `::1`.
    pub fn on_this_computer(&self) -> bool {
        self.base_url.as_deref().is_some_and(loopback)
    }

    /// Where this profile's Claude Code keeps its settings and sign-in (`local` / `claude`).
    pub fn config_folder(&self) -> PathBuf {
        paths::config_dir().join("claude").join(&self.id)
    }

    /// `claude`: may jobs use account `a`? The one recorded (compared by `Account::key`), or any
    /// before one is recorded — the one rule for jobs (`judge`), the window and the tray.
    pub fn expects(&self, a: &Account) -> bool {
        self.account.as_deref().is_none_or(|want| want.to_lowercase() == a.key())
    }

    /// `claude`: the lock jobs and checks hold shared and a sign-in exclusively; its text is the
    /// sign-in token (`Lease`).
    pub fn lock_path(&self) -> PathBuf {
        paths::config_dir().join("claude").join(format!("{}.lock", self.id))
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

/// The call's profile, ready to use (signed in, on the account it was set up with), with the lease
/// its job holds until it is done. Errors: `SigningIn` (a sign-in holds the lock — it never waits:
/// the caller retries), `SignInNeeded`.
pub fn for_call(dir: &Path) -> Result<Ready> {
    ready(&call_profile_id(dir)?)
}

/// A profile by id, ready to use, with its lease — the same checks as a call's (`for_call`): the
/// Echo window's Claude sessions start on it, and hold the lease while they run.
pub fn ready(id: &str) -> Result<Ready> {
    let p = Settings::load()?.get(id)?;
    let lease = if p.kind.signs_in() { Some(lease(&p)?) } else { None };
    check(&p, lease.as_ref())?;
    Ok(Ready { profile: p, _lease: lease })
}

/// The default profile's id (`system` when none is set or the settings can't be read).
pub fn default_id() -> String {
    Settings::load().map(|s| s.default_id()).unwrap_or_else(|_| SYSTEM.into())
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

/// The marks of a Claude Code session Echo itself was started from (`echoctl app start` in a session's Bash tool): a
/// session Echo starts is its own, not that one's child — inherited, they turn its transcript saving off (nothing to
/// resume) and point it at the other session's messaging and bridge.
const PARENT_SESSION: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_BRIDGE_SESSION_ID",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
    "AI_AGENT",
];

/// The command for Claude Code with profile `p` (built, not started; tested).
fn build_command(p: &Profile, claude: &Path, args: &[String], cwd: &Path, stdin: bool, use_shell: bool) -> Command {
    let managed = p.kind.managed();
    let env = managed.then(|| routing_env(p));
    let mut args = args.to_vec();
    // Model runs load only the profile's own settings; `claude auth …` and `--version` take no such option.
    if managed && args.first().is_none_or(|a| a != "auth" && a != "--version") {
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
    for k in PARENT_SESSION {
        c.env_remove(k);
    }
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
    c
}

/// Start Claude Code for `p`: the one way Echo runs it on call text, checks a sign-in or signs in
/// from the app (minutes, live jobs, `claude auth status`, `claude auth login`); `echoctl ai login`
/// (`login_here`) and `claude connect` start it otherwise. `system` keeps the user's own setup; `local` / `claude` apply the profile on top of
/// the login shell and load only their own settings (`--setting-sources user` = their folder).
pub fn claude_command(p: &Profile, args: &[String], cwd: &Path, stdin: bool) -> Result<Child> {
    Ok(prepare_claude(p, args, cwd, stdin)?.spawn()?)
}

/// `claude_command` up to the spawn (the checks it makes may take a moment: the policy check runs
/// processes on Windows), so a caller can decide right before starting it.
pub fn prepare_claude(p: &Profile, args: &[String], cwd: &Path, stdin: bool) -> Result<Command> {
    let fake = fake_claude();
    let claude = fake.clone().or_else(crate::app::claude_bin).ok_or_else(|| anyhow!("Claude Code is not installed — get it at https://claude.com/claude-code"))?;
    if p.kind.managed() {
        if let Some(why) = policy_conflict(p) {
            bail!("{why}");
        }
        fs::create_dir_all(p.config_folder())?;
    }
    let mut c = build_command(p, &claude, args, cwd, stdin, use_shell() && fake.is_none());
    detach(&mut c);
    Ok(c)
}

/// Before a guarded run (minutes, a thread's sync, a routine's or plugin's job): is the Claude Code `p` would start new
/// enough for the guard (`guard::MIN_CLAUDE`)? Asked with `--version`, before it is started or given anything (#400).
pub fn guard_ready(p: &Profile, cwd: &Path, what: &str) -> Result<()> {
    let mut child = claude_command(p, &["--version".into()], cwd, false)?;
    let (out, ..) = wait_output(&mut child, Duration::from_secs(20), &|| false).context("asking Claude Code its version")?;
    // Its line "2.1.310 (Claude Code)", wherever a login shell's own output put it (only that line: another
    // program's version must not pass for it).
    let v = out.lines().find(|l| l.contains("(Claude Code")).and_then(|l| l.split_whitespace().next()).unwrap_or("").to_string();
    if crate::guard::new_enough(&v) {
        return Ok(());
    }
    let [a, b, c] = crate::guard::MIN_CLAUDE;
    let found = if v.is_empty() { "(of unknown version)".to_string() } else { v };
    bail!("Claude Code {found} is too old to {what} — update it (`claude update`) to {a}.{b}.{c} or later")
}

/// Claude Code for `p` in a terminal (the Echo window's Claude sessions): the command and
/// environment `claude_command` would use — the login shell, the profile's routing, Echo's CLI on
/// PATH — for a pseudo-terminal. `headless`: as in a minutes run (Echo's mod idle, the CLI only on
/// `ECHOCTL_RUN_CALL`), for the guarded call session; else the user's own session.
#[cfg(feature = "tray")]
pub fn terminal_command(p: &Profile, args: &[String], cwd: &Path, env: &[(&str, String)], headless: bool) -> Result<portable_pty::CommandBuilder> {
    let fake = fake_claude();
    let claude = fake.clone().or_else(crate::app::claude_bin).ok_or_else(|| anyhow!("Claude Code is not installed — get it at https://claude.com/claude-code"))?;
    if p.kind.managed() {
        if let Some(why) = policy_conflict(p) {
            bail!("{why}");
        }
        fs::create_dir_all(p.config_folder())?;
    }
    let c = build_command(p, &claude, args, cwd, false, use_shell() && fake.is_none());
    let mut b = portable_pty::CommandBuilder::new(c.get_program());
    b.args(c.get_args());
    for (k, v) in c.get_envs() {
        match v {
            Some(v) => b.env(k, v),
            None => b.env_remove(k),
        }
    }
    if let Some(d) = c.get_current_dir() {
        b.cwd(d);
    }
    if !headless {
        b.env_remove(crate::notes::HEADLESS);
    }
    for (k, v) in env {
        b.env(k, v);
    }
    if std::env::var_os("ECHOCTL_DEBUG").is_some() {
        // What a session runs (never the environment's values: tokens).
        eprintln!("  debug: claude session: {:?} {:?} in {:?}", c.get_program(), c.get_args().collect::<Vec<_>>(), c.get_current_dir());
    }
    Ok(b)
}

/// Claude Code through the user's login shell (Unix; `ECHOCTL_NOTES_NO_SHELL` starts it directly).
fn use_shell() -> bool {
    cfg!(unix) && std::env::var_os("ECHOCTL_NOTES_NO_SHELL").is_none()
}

/// (tests) A stand-in for Claude Code, started directly.
#[cfg(test)]
pub static FAKE_CLAUDE: Mutex<Option<PathBuf>> = Mutex::new(None);

fn fake_claude() -> Option<PathBuf> {
    // Never the real one in a test: without the fake, starting Claude Code fails.
    #[cfg(test)]
    return Some(FAKE_CLAUDE.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_else(|| PathBuf::from("/nonexistent/claude-in-a-test")));
    #[cfg(not(test))]
    None
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

/// Why a `claude` profile can't be used now: signed out, or signed in to another account than the
/// one it was set up with. Kept typed (through the readiness cache too), so the window can offer
/// Sign in… / Use <account>, and live notes resume once it is fixed.
#[derive(Clone, Debug, PartialEq)]
pub struct SignInNeeded {
    /// The profile's id (`name` is for people).
    pub profile: String,
    pub name: String,
    /// The account the folder is signed in to instead (None: signed out).
    pub now: Option<String>,
    /// The account it was set up with (None: none recorded yet).
    pub expected: Option<String>,
}

impl SignInNeeded {
    pub fn signed_out(p: &Profile) -> SignInNeeded {
        SignInNeeded { profile: p.id.clone(), name: p.name.clone(), now: None, expected: p.account.clone() }
    }

    pub fn other_account(p: &Profile, now: String, expected: String) -> SignInNeeded {
        SignInNeeded { profile: p.id.clone(), name: p.name.clone(), now: Some(now), expected: Some(expected) }
    }
}

impl std::fmt::Display for SignInNeeded {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match (&self.now, &self.expected) {
            (Some(now), Some(expected)) => write!(f, "{} is signed in as {now}, not {expected} — Use {now} in the Echo window, or sign in again", self.name),
            _ => write!(f, "{} isn't signed in — Sign in… in the Echo window", self.name),
        }
    }
}

impl std::error::Error for SignInNeeded {}

/// A sign-in of this profile holds its lock: `lease` / `for_call` refuse meanwhile (they never
/// wait; live jobs and minutes runs try again).
#[derive(Clone, Debug, PartialEq)]
pub struct SigningIn {
    pub profile: String,
    pub name: String,
}

impl std::fmt::Display for SigningIn {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} is being signed in right now", self.name)
    }
}

impl std::error::Error for SigningIn {}

/// A hold on a `claude` profile's lock (`Profile::lock_path`). Shared: jobs and checks, any number
/// at once, and no sign-in can change the account under them. Exclusive: a sign-in. `token` is the
/// lock file's text, rewritten when a sign-in takes the lock: each process caches its own checks,
/// so one cached under an older token (a sign-in since, here or in another process) doesn't count.
pub struct Lease {
    _lock: crate::app::FileLock,
    token: String,
}

fn open_lock(p: &Profile, shared: bool) -> Result<Option<crate::app::FileLock>> {
    let path = p.lock_path();
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    let lock = if shared { crate::app::FileLock::try_take_shared(&path) } else { crate::app::FileLock::try_take(&path) };
    lock.with_context(|| format!("opening {}", path.display()))
}

/// A call's profile, ready for a job, and the lease the job holds while it runs (`claude`: no
/// sign-in can change the account until it is dropped). The profile is borrowed from it, so the
/// lease lives as long as it is used.
pub struct Ready {
    profile: Profile,
    _lease: Option<Lease>,
}

impl Ready {
    pub fn profile(&self) -> &Profile {
        &self.profile
    }
}

/// (tests) A stand-in `claude`: `auth status` prints `<profile folder>/status.json` (each run
/// counted in `status.count`; `slow` there makes it take a second, `stall` 30, `stubborn` ignore
/// TERM — stopping it then takes the 3 s until KILL); `auth login` records its
/// arguments in `login.args` and does what `login` says: `ok` (`after.json` becomes the status),
/// `fail`, `hang` (its pid in `login.pid`, then it sleeps); `-p` reads its prompt and prints
/// `p.reply` (with `p.hang` there: prints that to stderr, then sleeps, a run that never ends). Installing it forgets this process's observations and cached checks.
#[cfg(test)]
#[cfg_attr(not(unix), allow(dead_code))] // the fake is a shell script: only Unix tests use it
pub mod fake {
    use super::*;

    /// Installed until dropped.
    pub struct Fake;

    impl Drop for Fake {
        fn drop(&mut self) {
            *FAKE_CLAUDE.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
    }

    pub fn install(root: &Path) -> Fake {
        let f = root.join("fake-claude");
        fs::write(
            &f,
            r#"#!/bin/sh
d="$CLAUDE_CONFIG_DIR"
[ -f "$d/stubborn" ] && trap '' TERM
case "$1 $2" in
"--version ") cat "$d/version" 2>/dev/null || echo "2.1.400 (Claude Code)" ;;
"auth status") echo x >> "$d/status.count"; [ -f "$d/slow" ] && sleep 1; [ -f "$d/stall" ] && sleep 30; cat "$d/status.json" ;;
"-p "*) case "$*" in *stream-json*) [ -f "$d/p.hang" ] && { cat "$d/p.hang" >&2; exec sleep 60; }; while IFS= read -r l; do printf '%s\n' "$l" >> "$d/p.turns"; cat "$d/p.reply"; echo; done;; *) cat > "$d/p.stdin"; [ -f "$d/p.hang" ] && { cat "$d/p.hang" >&2; exec sleep 60; }; cat "$d/p.reply";; esac ;;
"auth login") echo "$@" > "$d/login.args"
  case "$(cat "$d/login" 2>/dev/null)" in
  ok) cp "$d/after.json" "$d/status.json"; echo "Login successful." ;;
  fail) echo "Login failed: Request failed with status code 400" >&2; exit 1 ;;
  hang) echo $$ > "$d/login.pid"; exec sleep 60 ;;
  esac ;;
esac
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&f, fs::Permissions::from_mode(0o755)).unwrap();
        }
        *FAKE_CLAUDE.lock().unwrap_or_else(|e| e.into_inner()) = Some(f);
        *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *CHECKED.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *REFUSED.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Fake
    }

    pub fn signed_in(email: &str) -> String {
        json!({ "loggedIn": true, "email": email, "orgName": "Example Corp", "subscriptionType": "max" }).to_string()
    }

    pub const SIGNED_OUT: &str = r#"{"loggedIn": false, "authMethod": "none"}"#;

    /// A `claude` profile `id` saved in the settings, its folder answering `status`.
    pub fn profile(id: &str, account: Option<&str>, status: &str) -> Profile {
        let p = Profile {
            id: id.into(),
            name: format!("Claude · {id}"),
            kind: Kind::Claude,
            base_url: None,
            token: None,
            live_model: None,
            notes_model: None,
            account: account.map(str::to_owned),
            email: None,
            sso: false,
        };
        Settings::update(|s| {
            s.live_notes = true;
            s.profiles.retain(|x| x.id != id);
            s.profiles.push(p.clone());
            Ok(())
        })
        .unwrap();
        fs::create_dir_all(p.config_folder()).unwrap();
        fs::write(p.config_folder().join("status.json"), status).unwrap();
        p
    }

    pub fn file(p: &Profile, name: &str) -> PathBuf {
        p.config_folder().join(name)
    }

    pub fn status_runs(p: &Profile) -> usize {
        fs::read_to_string(file(p, "status.count")).map(|s| s.lines().count()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_status_is_read_strictly() {
        let a = parse_status(&fake::signed_in("Pat@Example.com"), "").unwrap().unwrap();
        assert_eq!(a.key(), "pat@example.com · example corp");
        assert_eq!(parse_status(fake::SIGNED_OUT, "").unwrap(), None, "only an explicit false is signed out");
        assert!(parse_status(r#"{"error": "keychain locked"}"#, "").is_err(), "an answer without loggedIn isn't 'signed out'");
        assert!(parse_status("", "Error: something broke").unwrap_err().to_string().contains("something broke"));
        assert!(parse_status(r#"{"loggedIn": true}"#, "").is_err(), "signed in to nobody");
    }

    #[test]
    fn the_sign_in_page_is_pre_filled() {
        let mut p = Profile { id: "w".into(), name: "W".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
        assert_eq!(login_args(&p), ["auth", "login", "--claudeai"], "a claude.ai account always");
        p.account = Some("pat@example.com · example corp".into());
        assert_eq!(login_args(&p)[3..], ["--email", "pat@example.com"], "the recorded account's email");
        p.email = Some("me@work.example".into());
        p.sso = true;
        assert_eq!(login_args(&p)[3..], ["--email", "me@work.example", "--sso"]);
        assert!(email_arg(Some("not an email".into())).is_err());
        assert_eq!(email_arg(Some("  ".into())).unwrap(), None);
    }

    #[test]
    fn settling_judges_against_the_settings_as_they_are_now() {
        let (_env, _root) = crate::paths::testenv::folders("ai-settle");
        let a = Account { email: "pat@example.com".into(), org: "Example Corp".into(), subscription: "max".into() };
        let snapshot = Profile { id: "w".into(), name: "Work".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
        Settings { default: None, live_notes: true, profiles: vec![snapshot.clone()] }.save().unwrap();
        // First use: recorded.
        assert_eq!(settle(&snapshot, Some(&a)).unwrap(), a.key());
        assert_eq!(Settings::load().unwrap().get("w").unwrap().account, Some(a.key()));
        // Another writer recorded another account meanwhile: the old snapshot doesn't let it through.
        Settings::update(|s| {
            s.profiles[0].account = Some("someone@else.example".into());
            Ok(())
        })
        .unwrap();
        let e = settle(&snapshot, Some(&a)).unwrap_err();
        let typed = e.downcast_ref::<SignInNeeded>().expect("typed");
        assert_eq!((typed.now.as_deref(), typed.expected.as_deref()), (Some(a.key().as_str()), Some("someone@else.example")));
        assert!(settle(&snapshot, None).unwrap_err().downcast_ref::<SignInNeeded>().is_some_and(|s| s.now.is_none()));
        // A removed profile: an error, never a pass.
        Settings::update(|s| {
            s.profiles.clear();
            Ok(())
        })
        .unwrap();
        assert!(settle(&snapshot, Some(&a)).is_err());
    }

    #[test]
    fn a_sign_in_excludes_jobs_and_changes_the_token() {
        let (_env, _root) = crate::paths::testenv::folders("ai-lease");
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        let before = token_now(&p).unwrap();
        let (a, b) = (lease(&p).unwrap(), lease(&p).unwrap());
        assert!(sign_in_lease(&p).unwrap().is_none(), "not while jobs hold it");
        assert!(!signing_in_now(&p));
        drop((a, b));
        crate::paths::testenv::eventually("free now", || sign_in_lease(&p).unwrap().is_some());
        let s = sign_in_lease(&p).unwrap().expect("free now");
        assert!(signing_in_now(&p));
        let e = lease(&p).err().expect("jobs wait for the sign-in");
        assert_eq!(e.downcast_ref::<SigningIn>().map(|s| s.profile.as_str()), Some("w"));
        assert_ne!(s.token, before);
        let token = s.token.clone();
        drop(s);
        assert_eq!(lease(&p).unwrap().token, token, "the new token is what jobs read");
        assert_eq!(token_now(&p).unwrap(), token);
    }

    /// #400: a guarded run asks Claude Code's version first; one too old (or of no version) is refused before it is
    /// started or given anything, and says how to update — no window's button named.
    #[cfg(unix)]
    #[test]
    fn a_claude_code_too_old_for_the_guard_is_refused_before_it_gets_anything() {
        let (_env, root) = crate::paths::testenv::folders("ai-guard-ready");
        let _fake = fake::install(&root);
        let p = fake::profile("old", None, &fake::signed_in("pat@example.com"));
        assert!(guard_ready(&p, &root, "write minutes").is_ok(), "the fake's own version is new enough");
        fs::write(fake::file(&p, "version"), "2.1.200 (Claude Code)\n").unwrap();
        let e = guard_ready(&p, &root, "write minutes").unwrap_err().to_string();
        let [a, b, c] = crate::guard::MIN_CLAUDE;
        assert!(e.contains("Claude Code 2.1.200 is too old to write minutes") && e.contains(&format!("{a}.{b}.{c} or later")), "{e}");
        assert!(e.contains("claude update") && !e.contains("Try again"), "{e}");
        // Another program's version from the login shell doesn't pass for Claude Code's.
        fs::write(fake::file(&p, "version"), "Python 3.12.1\n2.1.200 (Claude Code)\n").unwrap();
        assert!(guard_ready(&p, &root, "write minutes").unwrap_err().to_string().contains("Claude Code 2.1.200 is too old"));
        fs::write(fake::file(&p, "version"), "").unwrap();
        assert!(guard_ready(&p, &root, "write minutes").unwrap_err().to_string().contains("(of unknown version)"));
        fs::write(fake::file(&p, "version"), format!("{a}.{b}.{c} (Claude Code)\n")).unwrap();
        assert!(guard_ready(&p, &root, "write minutes").is_ok());
        for f in ["p.stdin", "p.turns"] {
            assert!(!fake::file(&p, f).exists(), "nothing was given to Claude Code: {f}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn readiness_is_cached_per_token_and_stays_typed() {
        let (_env, root) = crate::paths::testenv::folders("ai-ready");
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, &fake::signed_in("pat@example.com"));
        let call = root.join("home/calls/2026-10-23_100000-x");
        fs::create_dir_all(&call).unwrap();
        fs::write(call.join("meta.json"), r#"{"ai_profile": "w"}"#).unwrap();
        forget_check("w");
        assert!(for_call(&call).is_ok());
        assert!(for_call(&call).is_ok());
        assert_eq!(fake::status_runs(&p), 1, "the second job used the cached check");
        // A sign-in (here or in another process) …
        crate::paths::testenv::eventually("the jobs' leases are gone", || sign_in_lease(&p).unwrap().is_some());
        fs::write(fake::file(&p, "status.json"), fake::SIGNED_OUT).unwrap();
        let e = for_call(&call).err().unwrap();
        assert_eq!(fake::status_runs(&p), 2, "… voids the cached check");
        assert!(e.downcast_ref::<SignInNeeded>().is_some(), "typed on a fresh check");
        let e = for_call(&call).err().unwrap();
        assert_eq!(fake::status_runs(&p), 2);
        assert!(e.downcast_ref::<SignInNeeded>().is_some(), "and typed on a cache hit");
        assert!(matches!(observation("w").unwrap().result, Ok(None)), "published");
        // A model said it isn't signed in: neither the cache nor the observation counts.
        auth_failed(&p);
        assert!(observation("w").unwrap().stale);
        let _ = for_call(&call);
        assert_eq!(fake::status_runs(&p), 3);
    }

    #[cfg(unix)]
    #[test]
    fn a_job_holds_its_lease_while_it_runs() {
        let (_env, root) = crate::paths::testenv::folders("ai-job-lease");
        let _fake = fake::install(&root);
        let p = fake::profile("w", None, &fake::signed_in("pat@example.com"));
        let call = root.join("home/calls/2026-10-23_100000-x");
        fs::create_dir_all(&call).unwrap();
        fs::write(call.join("meta.json"), r#"{"ai_profile": "w"}"#).unwrap();
        let ready = for_call(&call).unwrap();
        assert_eq!(ready.profile().id, "w");
        assert!(sign_in_lease(&p).unwrap().is_none(), "no sign-in under a running job");
        drop(ready);
        crate::paths::testenv::eventually("free once the job is done", || sign_in_lease(&p).unwrap().is_some());
    }

    #[test]
    fn observations_never_go_back() {
        let (_env, _root) = crate::paths::testenv::folders("ai-order"); // `fake::install` clears them
        let o = |seq| Observation { seq, token: String::new(), at: Instant::now(), result: Ok(None), stale: false };
        publish("order-test", o(10));
        publish("order-test", o(9)); // a check that started earlier and ended later
        assert_eq!(observation("order-test").unwrap().seq, 10);
    }

    #[cfg(unix)]
    #[test]
    fn the_cli_sign_in_records_reaps_and_releases() {
        let (_env, root) = crate::paths::testenv::folders("ai-cli-login");
        let fake = fake::install(&root);
        let claude = FAKE_CLAUDE.lock().unwrap().clone().unwrap();
        let p = fake::profile("w", None, fake::SIGNED_OUT);
        let free = || crate::paths::testenv::eventually("the lease is released", || sign_in_lease(&p).unwrap().is_some());
        // It fails: nothing recorded.
        fs::write(fake::file(&p, "login"), "fail").unwrap();
        assert!(login_with(&p, &claude, false, &|| false).unwrap_err().to_string().contains("didn't finish"));
        assert_eq!(Settings::load().unwrap().get("w").unwrap().account, None);
        free();
        // It works: the account is recorded.
        fs::write(fake::file(&p, "login"), "ok").unwrap();
        fs::write(fake::file(&p, "after.json"), fake::signed_in("pat@example.com")).unwrap();
        assert_eq!(login_with(&p, &claude, false, &|| false).unwrap(), "pat@example.com · example corp (max)");
        assert_eq!(Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some("pat@example.com · example corp"));
        free();
        // Another account: an error that says how to use it (a non-zero exit).
        let p = fake::profile("w", Some("pat@example.com · example corp"), fake::SIGNED_OUT);
        fs::write(fake::file(&p, "after.json"), fake::signed_in("other@example.com")).unwrap();
        assert!(login_with(&p, &claude, false, &|| false).unwrap_err().to_string().contains("echoctl ai accept w"));
        free();
        // Stopped during the final check: that check's child is stopped too, then the lease goes.
        fs::write(fake::file(&p, "status.json"), fake::SIGNED_OUT).unwrap();
        fs::write(fake::file(&p, "slow"), "").unwrap();
        let count = fake::file(&p, "status.count");
        let _ = fs::remove_file(&count);
        let e = login_with(&p, &claude, false, &|| count.exists()).unwrap_err();
        assert!(e.to_string().contains("stopped"), "{e:#}");
        free();
        drop(fake);
    }

    #[cfg(unix)]
    #[test]
    fn accepting_takes_the_account_that_was_shown_or_nothing() {
        let (_env, root) = crate::paths::testenv::folders("ai-accept");
        let _fake = fake::install(&root);
        let p = fake::profile("w", Some("pat@example.com · example corp"), &fake::signed_in("other@example.com"));
        let e = accept_account("w", Some("someone@third.example"), &|| false).unwrap_err();
        assert!(e.to_string().contains("now signed in as other@example.com"), "{e:#}");
        assert_eq!(Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some("pat@example.com · example corp"), "nothing recorded");
        accept_account("w", Some("other@example.com · example corp"), &|| false).unwrap();
        assert_eq!(Settings::load().unwrap().get("w").unwrap().account.as_deref(), Some("other@example.com · example corp"));
        let _ = p;
    }

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
        let local = Profile { id: "q".into(), name: "Q".into(), kind: Kind::Local, base_url: Some("http://localhost:11434".into()), token: None, live_model: Some("m".into()), notes_model: Some("m".into()), account: None, email: None, sso: false };
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
        let claude = Profile { id: "c".into(), name: "C".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
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
        // Started from inside a Claude Code session, Echo's are still sessions of their own (transcripts saved).
        assert!(system.get_envs().any(|(k, v)| k == "CLAUDE_CODE_CHILD_SESSION" && v.is_none()), "not a child of the session Echo came from");
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
        let p = Profile { id: "c".into(), name: "C".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
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
                        s.profiles.push(Profile { id: id.into(), name: format!("{id} {i}"), kind: Kind::Local, base_url: Some("http://localhost:1".into()), token: None, live_model: Some("m".into()), notes_model: Some("m".into()), account: None, email: None, sso: false });
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
        let p = |account: Option<&str>| Profile { id: "w".into(), name: "Work".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: account.map(str::to_owned), email: None, sso: false };
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
        let p = Profile { id: "w".into(), name: "Work".into(), kind: Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
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
        assert!(format!("{:#}", for_call(&call).err().unwrap()).contains("doesn't exist"));
        set_call_profile(&call, SYSTEM).unwrap();
        assert_eq!(for_call(&call).unwrap().profile.kind, Kind::System);
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

    #[test]
    fn the_minutes_model_is_one_answer_and_set_per_profile() {
        let (_env, _root) = crate::paths::testenv::folders("ai-notes-model");
        // A local profile with only a live model: that one writes the minutes (as the run routes it).
        let mut q = Profile { id: "q".into(), name: "Q".into(), kind: Kind::Local, base_url: Some("http://localhost:11434".into()), token: None, live_model: Some("qwen".into()), notes_model: None, account: None, email: None, sso: false };
        assert_eq!(q.notes_model().as_deref(), Some("qwen"));
        q.notes_model = Some("big".into());
        assert_eq!(q.notes_model_label(), "big");
        add_claude("c", None, None, None, None, false).unwrap();
        add_local("l", None, "m", Some("live".into()), "http://localhost:11434", None).unwrap();
        assert_eq!(set_notes_model("c", " opus ").unwrap().1, "opus");
        assert_eq!(Settings::load().unwrap().get("c").unwrap().notes_model().as_deref(), Some("opus"));
        assert_eq!(set_notes_model("c", "").unwrap().1, crate::config::DEFAULT_NOTES_MODEL, "empty: back to sonnet");
        assert_eq!(set_notes_model("l", "").unwrap().1, "live", "empty: its live model");
        // The built-in profile: config.json, as `config --notes-model` writes it.
        assert_eq!(set_notes_model(SYSTEM, "haiku").unwrap().1, "haiku");
        assert_eq!(Config::load().unwrap().notes_model.as_deref(), Some("haiku"));
        assert_eq!(set_notes_model(SYSTEM, "default").unwrap().1, "Claude Code's default model");
        assert_eq!(set_notes_model(SYSTEM, "sonnet").unwrap().1, "sonnet");
        assert_eq!(Config::load().unwrap().notes_model, None, "sonnet is the default: unset");
        assert!(set_notes_model("nobody", "x").is_err());
    }

    /// Root reads any file: a permission test means nothing then.
    #[cfg(unix)]
    fn nix_root() -> bool {
        std::process::Command::new("id").arg("-u").output().is_ok_and(|o| o.stdout.starts_with(b"0"))
    }

    #[test]
    fn a_policy_that_switches_hooks_off_is_found() {
        assert!(restricts_hooks(&json!({"allowManagedHooksOnly": true})) && restricts_hooks(&json!({"settings": {"disableAllHooks": true}})));
        assert!(!restricts_hooks(&json!({"disableAllHooks": false, "env": {"ANTHROPIC_MODEL": "x"}})));
        assert!(restricts_hooks(&json!({"policyHelper": {"command": "x"}})), "a helper may switch them off");
        let (_env, _root) = crate::paths::testenv::folders("ai-hooks-policy");
        add_claude("c", None, None, None, None, false).unwrap();
        let p = Settings::load().unwrap().get("c").unwrap();
        fs::create_dir_all(p.config_folder()).unwrap();
        let cached = p.config_folder().join("remote-settings.json");
        fs::write(&cached, r#"{"settings": {"allowManagedHooksOnly": true}}"#).unwrap();
        assert!(hooks_conflict(&p, Path::new(".")).is_err_and(|w| w.contains("remote-settings.json")));
        fs::write(&cached, "not json").unwrap();
        assert!(hooks_conflict(&p, Path::new(".")).is_err_and(|w| w.contains("can't be read")));
        // #404: the profile's own cached settings unreadable — the choice offered is Claude Code (as configured); a
        // yes to go without a file lets go only one the system won't let this user read (Claude Code can't either).
        assert_eq!(unreadable(&p, Path::new(".")), Some((cached.display().to_string(), "system")));
        let file = cached.display().to_string();
        assert!(without_policy(std::slice::from_ref(&file), || hooks_conflict(&p, Path::new("."))).is_err(), "readable, not JSON: not let go");
        #[cfg(unix)]
        if !nix_root() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&cached, fs::Permissions::from_mode(0o000)).unwrap();
            assert!(without_policy(std::slice::from_ref(&file), || hooks_conflict(&p, Path::new("."))).is_ok(), "denied: let go");
            assert!(hooks_conflict(&p, Path::new(".")).is_err(), "the yes was for those runs only");
            fs::set_permissions(&cached, fs::Permissions::from_mode(0o600)).unwrap();
        }
        fs::remove_file(&cached).unwrap();
        assert_eq!(unreadable(&p, Path::new(".")), None);
        assert_eq!(hooks_conflict(&p, Path::new(".")).unwrap(), [p.config_folder()], "checked: the profile's folder");
    }

    #[test]
    fn the_shells_answer_is_only_what_lies_between_the_markers() {
        let out = "motd\n<<echo:config-dir>>/Users/me\n.claude-work<</echo:config-dir>>bye\n";
        assert_eq!(config_dir_answer(out), Some(("/Users/me", ".claude-work")), "relative: from the shell's folder");
        assert_eq!(config_dir_answer("<<echo:config-dir>>/x\n<</echo:config-dir>>"), Some(("/x", "")), "not set");
        assert_eq!(config_dir_answer("<<echo:config-dir>>/x\n/y/.claude-work"), None, "no end: can't tell");
        assert_eq!(config_dir_answer("nothing"), None);
    }

}

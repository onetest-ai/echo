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

/// The minutes run's guard is a settings-file hook (`guard.rs`): admin policy that switches those
/// off (`disableAllHooks`, `allowManagedHooksOnly`), or that can't be read, refuses the run —
/// whatever the profile. Sources: the managed settings files, macOS managed preferences, Windows
/// policy keys (their `Settings` JSON; one that can't be read refuses), and the server-managed
/// settings Claude Code cached
/// in the profile's folder (`system`: every folder its Claude Code may use, `system_config_dirs`).
/// Claude Code fetches those again when it starts: an organisation turning such a policy on is
/// seen from the run after that (known residual).
/// Ok: the Claude Code settings folders checked (for the run's log).
pub fn hooks_conflict(p: &Profile, cwd: &Path) -> Result<Vec<PathBuf>, String> {
    const WHY: &str = "the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session";
    let present = |f: &Path| !matches!(f.try_exists(), Ok(false));
    let mut files = match managed_files() {
        Ok(f) => f,
        Err(e) => return Err(format!("{WHY} ({e})")),
    };
    let folders = if p.kind == Kind::System {
        match system_config_dirs(cwd) {
            Ok(f) => f,
            Err(e) => return Err(format!("Echo can't tell which Claude Code settings the minutes run would use ({e}), so it can't check they allow its guard")),
        }
    } else {
        vec![p.config_folder()]
    };
    files.extend(folders.iter().map(|f| f.join("remote-settings.json")));
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(unused_mut))] // macOS and Windows add their own
    let mut sources: Vec<(String, Result<Value, String>)> = files.iter().filter(|f| present(f)).map(|f| (f.display().to_string(), read_policy_json(f))).collect();
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
    for (hklm, key) in [(true, r"HKLM\SOFTWARE\Policies\ClaudeCode"), (false, r"HKCU\SOFTWARE\Policies\ClaudeCode")] {
        match windows_policy(hklm) {
            Ok(None) => {}
            Ok(Some(json)) => sources.push((key.to_string(), serde_json::from_str::<Value>(&json).map_err(|e| e.to_string()))),
            Err(e) => return Err(format!("{WHY} ({key} can't be read: {e})")),
        }
    }
    for (name, v) in sources {
        match v {
            Err(_) => return Err(format!("{WHY} ({name} can't be read)")),
            Ok(v) if restricts_hooks(&v) => return Err(format!("{WHY} ({name})")),
            Ok(_) => {}
        }
    }
    Ok(folders)
}

/// The folders the built-in profile's Claude Code may keep its settings in: what its login shell
/// sets `CLAUDE_CONFIG_DIR` to (the run starts through that shell), Echo's own value, and
/// `~/.claude` — all are checked, since Echo can't tell which one the run will use. The shell is
/// asked in the run's own folder (`cwd`), as the run starts there. Err when it can't say (it fails,
/// hangs or prints no bounded answer): the check can't be made.
fn system_config_dirs(cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v = vec![];
    if use_shell() && fake_claude().is_none() {
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
        let mut c = Command::new(shell);
        // Its folder after its startup files (they may `cd`; Claude Code starts there) and its value.
        c.args(["-l", "-i", "-c", "printf '<<echo:config-dir>>%s\\n%s<</echo:config-dir>>' \"$PWD\" \"$CLAUDE_CONFIG_DIR\""])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        detach(&mut c);
        let child = c.spawn().map_err(|e| format!("your login shell can't be started: {e}"))?;
        let pid = child.id();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });
        let out = match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(format!("your login shell failed: {e}")),
            Err(_) => {
                stop_group(pid);
                return Err("your login shell didn't answer within 15 s".into());
            }
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let (pwd, d) = config_dir_answer(&text).ok_or("your login shell didn't say which settings folder it uses")?;
        // A relative one as Claude Code sees it: from the folder the shell ends up in.
        v.extend(Some(d).filter(|d| !d.is_empty()).map(|d| cwd.join(pwd).join(d)));
    }
    v.extend(std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()).map(|d| cwd.join(d)));
    v.push(paths::user_home().join(".claude"));
    v.dedup();
    Ok(v)
}

/// The shell's folder and `CLAUDE_CONFIG_DIR`, between the probe's two markers (what the shell
/// prints before or after — its rc files, a logout script — stays out); None without both.
fn config_dir_answer(out: &str) -> Option<(&str, &str)> {
    let (_, rest) = out.rsplit_once("<<echo:config-dir>>")?;
    rest.split_once("<</echo:config-dir>>")?.0.split_once('\n')
}

/// Claude Code's Windows policy: the JSON in `<hive>\SOFTWARE\Policies\ClaudeCode`, value
/// `Settings`. None when there is none; Err when it is there but can't be read (access denied, …).
#[cfg(windows)]
fn windows_policy(hklm: bool) -> Result<Option<String>, String> {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ};
    let hive = if hklm { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
    let (key, value) = (w!(r"SOFTWARE\Policies\ClaudeCode"), w!("Settings"));
    let mut len = 0u32;
    // SAFETY: plain registry reads into a buffer of the size the first call reports.
    let r = unsafe { RegGetValueW(hive, key, value, RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND, None, None, Some(&mut len)) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if r.is_err() {
        return Err(format!("Windows error {}", r.0));
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2)];
    let r = unsafe { RegGetValueW(hive, key, value, RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut len)) };
    if r.is_err() {
        return Err(format!("Windows error {}", r.0));
    }
    Ok(Some(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string()))
}

/// A policy document that switches settings-file hooks off, or a helper that may. Server-managed
/// settings may wrap the settings in a `settings` object.
fn restricts_hooks(v: &Value) -> bool {
    // A policy helper decides the settings when Claude Code starts: Echo can't tell what it allows.
    let doc = |v: &Value| v["disableAllHooks"] == true || v["allowManagedHooksOnly"] == true || v.get("policyHelper").is_some() || v.get("policyHelpers").is_some();
    doc(v) || doc(&v["settings"])
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

/// The shared lease (a job, a check); a `SigningIn` error while a sign-in holds the lock. Tried
/// for 300 ms: a lock released a moment ago can still look held while another thread here starts a
/// process (its child holds a copy of the descriptor until it execs — measured on macOS).
pub fn lease(p: &Profile) -> Result<Lease> {
    let t = Instant::now();
    let lock = loop {
        match open_lock(p, true)? {
            Some(l) => break l,
            None if t.elapsed() < Duration::from_millis(300) => std::thread::sleep(Duration::from_millis(30)),
            None => return Err(anyhow::Error::new(SigningIn { profile: p.id.clone(), name: p.name.clone() })),
        }
    };
    let token = lock.text().with_context(|| format!("reading {}", p.lock_path().display()))?;
    Ok(Lease { _lock: lock, token })
}

/// A sign-in's exclusive lease, under a new token (None: a job, a check or another sign-in holds
/// the lock).
pub fn sign_in_lease(p: &Profile) -> Result<Option<Lease>> {
    let Some(lock) = open_lock(p, false)? else { return Ok(None) };
    let token = format!("{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default());
    lock.set_text(&token).with_context(|| format!("writing {}", p.lock_path().display()))?;
    forget_check(&p.id);
    Ok(Some(Lease { _lock: lock, token }))
}

/// The token now, without the lock (`""` before the first sign-in); None when the file can't be
/// read — on Windows, while a sign-in holds it.
pub fn token_now(p: &Profile) -> Option<String> {
    match fs::read_to_string(p.lock_path()) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(String::new()),
        Err(_) => None,
    }
}

/// A sign-in holds the profile's lock (probed with a shared take, released at once; a sign-in
/// starting at that very moment tries again).
pub fn signing_in_now(p: &Profile) -> bool {
    matches!(open_lock(p, true), Ok(None))
}

/// What this process last learned of a profile's sign-in (a `claude auth status` check, or
/// `auth_failed`): `seq` taken when the check started (a check finishing late never replaces a
/// newer one) and the token it ran under.
#[derive(Clone, Debug)]
pub struct Observation {
    pub seq: u64,
    pub token: String,
    pub at: Instant,
    pub result: Result<Option<Account>, String>,
    /// Published by `auth_failed` (a model said the folder isn't signed in; `result` is then an
    /// error), not by a check: it needs another look.
    pub stale: bool,
}

static LAST: Mutex<Option<HashMap<String, Observation>>> = Mutex::new(None);
static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_seq() -> u64 {
    SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
}

fn publish(id: &str, o: Observation) {
    let mut g = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    if m.get(id).is_none_or(|old| old.seq < o.seq) {
        m.insert(id.to_string(), o);
    }
}

/// The last observation of a profile's sign-in (this process).
pub fn observation(id: &str) -> Option<Observation> {
    LAST.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(id).cloned())
}

/// Who `p`'s folder is signed in to (None: signed out), checked under a shared lease and
/// published; a `SigningIn` error while a sign-in holds the lock (nothing published: it is a lock
/// state, not an answer). A lock that can't be opened at all is published as the check's error.
pub fn observe(p: &Profile, cancelled: &dyn Fn() -> bool) -> Result<Option<Account>> {
    let seq = next_seq();
    let lease = match lease(p) {
        Ok(l) => l,
        Err(e) if e.downcast_ref::<SigningIn>().is_some() => return Err(e),
        Err(e) => {
            let token = token_now(p).unwrap_or_default();
            publish(&p.id, Observation { seq, token, at: Instant::now(), result: Err(format!("{e:#}")), stale: false });
            return Err(e);
        }
    };
    observe_under(p, &lease, cancelled)
}

/// The check, by a holder of the lock: a sign-in under its exclusive lease (`observe` would be
/// refused — a shared take fails against it, in this process too), a job under its shared one
/// (`check`), so the account can't change before it is used. Published under the lease's token; a
/// stopped check publishes nothing.
pub fn observe_under(p: &Profile, lease: &Lease, cancelled: &dyn Fn() -> bool) -> Result<Option<Account>> {
    let seq = next_seq();
    let r = account_with(p, cancelled);
    if !cancelled() {
        let result = r.as_ref().map(Clone::clone).map_err(|e| format!("{e:#}"));
        publish(&p.id, Observation { seq, token: lease.token.clone(), at: Instant::now(), result, stale: false });
    }
    r
}

/// `claude auth status` for `p`'s folder (stopped when `cancelled()` says so).
fn account_with(p: &Profile, cancelled: &dyn Fn() -> bool) -> Result<Option<Account>> {
    let mut child = claude_command(p, &["auth".into(), "status".into()], &paths::config_dir(), false)?;
    let (out, err, _) = wait_output(&mut child, Duration::from_secs(20), cancelled)
        .with_context(|| format!("checking the sign-in of {}", p.name))?;
    parse_status(&out, &err).with_context(|| format!("checking the sign-in of {}", p.name))
}

/// Profiles a model refused (id → the sign-in token then): `auth status` saying "signed in"
/// doesn't undo that under the same token — a sign-in (a new token), an accepted account or a job
/// that works does.
static REFUSED: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// A model run said `p` isn't signed in (or refused its token): neither the cached check nor the
/// last observation counts any more, and the refusal holds until a sign-in, an accepted account,
/// or a job that works. The job's own failure (`needs_sign_in`, a `needs_login` run) stays as long.
pub fn auth_failed(p: &Profile) {
    forget_check(&p.id);
    let token = token_now(p).unwrap_or_default();
    REFUSED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(p.id.clone(), token.clone());
    let result = Err("Claude Code said it isn't signed in".to_string());
    publish(&p.id, Observation { seq: next_seq(), token, at: Instant::now(), result, stale: true });
}

/// A model refused `p` under the sign-in token it has now (`auth_failed`).
pub fn refused_by_model(p: &Profile) -> bool {
    let token = token_now(p);
    REFUSED.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(&p.id)).is_some_and(|t| token.as_deref() == Some(t.as_str()))
}

/// A job on profile `id` worked (or its account was accepted): a model's refusal no longer holds.
pub fn job_worked(id: &str) {
    if let Some(m) = REFUSED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(id);
    }
}

/// `claude auth status`'s answer: signed in (with an email), or explicitly signed out
/// (`loggedIn: false`); anything else is an error, never "signed out" (that would open a sign-in).
fn parse_status(out: &str, err: &str) -> Result<Option<Account>> {
    // The JSON ends the output; the login shell's start-up may print a `{` of its own before it.
    let v: Value = out
        .match_indices('{')
        .find_map(|(i, _)| serde_json::from_str(out[i..].trim()).ok())
        .ok_or_else(|| anyhow!("unexpected answer from `claude auth status`: {}", last_words(out, err)))?;
    match v["loggedIn"].as_bool() {
        Some(false) => return Ok(None),
        Some(true) => {}
        None => bail!("unexpected answer from `claude auth status`: {}", clip_line(&v.to_string())),
    }
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let a = Account { email: s("email"), org: s("orgName"), subscription: s("subscriptionType") };
    if a.email.is_empty() {
        bail!("`claude auth status` names no account");
    }
    Ok(Some(a))
}

/// A failed child's reason: the last line it wrote on stderr, else on stdout (not the sign-in's
/// own chatter: its URLs and prompts).
pub fn last_words(out: &str, err: &str) -> String {
    let said = |s: &str| {
        s.lines().map(str::trim).rfind(|l| !l.is_empty() && !l.contains("://") && !l.starts_with("Opening browser") && !l.starts_with("Paste code")).map(str::to_owned)
    };
    clip_line(&said(err).or_else(|| said(out)).unwrap_or_else(|| "no output".into()))
}

fn clip_line(s: &str) -> String {
    let s = s.lines().next().unwrap_or("");
    if s.chars().count() > 160 { format!("{}…", s.chars().take(160).collect::<String>()) } else { s.to_string() }
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

/// Why a readiness check failed, kept typed so a cache hit gives the same error as the check.
#[derive(Clone, Debug)]
enum Problem {
    SignIn(SignInNeeded),
    Other(String),
}

impl Problem {
    fn of(e: &anyhow::Error) -> Problem {
        match e.downcast_ref::<SignInNeeded>() {
            Some(s) => Problem::SignIn(s.clone()),
            None => Problem::Other(format!("{e:#}")),
        }
    }

    fn error(&self) -> anyhow::Error {
        match self {
            Problem::SignIn(s) => anyhow::Error::new(s.clone()),
            Problem::Other(s) => anyhow!("{s}"),
        }
    }
}

/// Recent sign-in checks (`claude auth status` takes about a second), per profile id: the token
/// they ran under, when, and the account (or why it can't be used). A success is kept 30 s, a
/// failure 10 s — and neither once a sign-in changed the token.
type Checks = HashMap<String, (String, Instant, Result<String, Problem>)>;
static CHECKED: Mutex<Option<Checks>> = Mutex::new(None);

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

/// Can a job run on `p` now? `local`: an http(s) server and a model. `claude` (checked under the
/// caller's `lease`): signed in, and to the account the profile was first used with (recorded then).
fn check(p: &Profile, lease: Option<&Lease>) -> Result<()> {
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
    let Some(lease) = lease else { bail!("{} is checked without its lock", p.name) };
    let cached = CHECKED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(&p.id).cloned();
    if let Some((token, at, r)) = cached {
        let ttl = Duration::from_secs(if r.is_ok() { 30 } else { 10 });
        if token == lease.token && at.elapsed() < ttl {
            return r.map(|_| ()).map_err(|e| e.error());
        }
    }
    let r = observe_under(p, lease, &|| false).and_then(|a| settle(p, a.as_ref())).map_err(|e| Problem::of(&e));
    CHECKED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(p.id.clone(), (lease.token.clone(), Instant::now(), r.clone()));
    r.map(|_| ()).map_err(|e| e.error())
}

/// Forget the last sign-in check (a sign-in started, an account accepted, a model said it isn't
/// signed in).
pub fn forget_check(id: &str) {
    if let Some(m) = CHECKED.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(id);
    }
}

/// What a sign-in check says (`None` = signed out): the account's key when it may be used, and
/// whether it should be recorded as the profile's (its first use); else `SignInNeeded`.
fn judge(p: &Profile, a: Option<&Account>) -> Result<(String, bool)> {
    let Some(a) = a else { return Err(anyhow::Error::new(SignInNeeded::signed_out(p))) };
    match &p.account {
        Some(want) if !p.expects(a) => Err(anyhow::Error::new(SignInNeeded::other_account(p, a.key(), want.clone()))),
        Some(_) => Ok((a.key(), false)),
        None => Ok((a.key(), true)),
    }
}

/// Judge who `p`'s folder is signed in to against the profile as stored now (re-read: another
/// process may have recorded or accepted an account meanwhile): the account's key, or
/// `SignInNeeded`. The first use records it (unless another writer recorded a different one).
pub fn settle(p: &Profile, a: Option<&Account>) -> Result<String> {
    let now = Settings::load()?.get(&p.id)?;
    let (key, first) = judge(&now, a)?;
    if first {
        Settings::update(|s| {
            let stored = s.profiles.iter_mut().find(|x| x.id == p.id).ok_or_else(|| anyhow!("the AI profile \"{}\" was removed", p.id))?;
            match stored.account.clone() {
                None => stored.account = Some(key.clone()),
                Some(want) if want.to_lowercase() == key => {}
                Some(want) => return Err(anyhow::Error::new(SignInNeeded::other_account(stored, key.clone(), want))),
            }
            Ok(())
        })?;
    }
    Ok(key)
}

/// Accept the account a `claude` profile's folder is signed in to now — the one the user was shown
/// (`shown`), when given: another one meanwhile is refused. Checked and recorded under the
/// profile's lease (no sign-in in between); the check stops when `cancelled()` says so (its child
/// reaped before the lease goes).
pub fn accept_account(id: &str, shown: Option<&str>, cancelled: &dyn Fn() -> bool) -> Result<String> {
    let p = Settings::load()?.get(id)?;
    if !p.kind.signs_in() {
        bail!("{} has no account of its own to accept", p.name);
    }
    let lease = lease(&p)?;
    let a = observe_under(&p, &lease, cancelled)?.ok_or_else(|| anyhow::Error::new(SignInNeeded::signed_out(&p)))?;
    if cancelled() {
        bail!("stopped");
    }
    if let Some(shown) = shown.filter(|s| s.to_lowercase() != a.key()) {
        bail!("{} is now signed in as {}, not {shown} — look again", p.name, a.key());
    }
    Settings::update(|s| {
        let stored = s.profiles.iter_mut().find(|x| x.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        stored.account = Some(a.key());
        Ok(())
    })?;
    forget_check(id);
    job_worked(id);
    drop(lease);
    Ok(a.label())
}

/// The email pre-filled on the sign-in page: the profile's, else the recorded account's.
pub fn login_email(p: &Profile) -> Option<String> {
    let recorded = p.account.as_deref().and_then(|k| k.split(" · ").next()).map(str::to_owned);
    p.email.clone().or(recorded).filter(|e| e.contains('@'))
}

/// `claude auth login` for `p`: a claude.ai account always (a Console sign-in may be stored outside
/// the profile's folder), the email pre-filled (`login_email`), single sign-on when the profile
/// says so.
pub fn login_args(p: &Profile) -> Vec<String> {
    let mut a: Vec<String> = ["auth", "login", "--claudeai"].map(String::from).to_vec();
    if let Some(email) = login_email(p) {
        a.extend(["--email".into(), email]);
    }
    if p.sso {
        a.push("--sso".into());
    }
    a
}

/// `echoctl ai login <id>`: sign in here, in the terminal it was run from (Claude Code's own
/// prompts, its paste-code fallback included). Ctrl-C stops it.
pub fn login_here(id: &str) -> Result<()> {
    use std::sync::atomic::{AtomicBool, Ordering};
    let p = Settings::load()?.get(id)?;
    let claude = fake_claude().or_else(crate::app::claude_bin).ok_or_else(|| anyhow!("Claude Code is not installed — get it at https://claude.com/claude-code"))?;
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    let _ = ctrlc::set_handler(move || s.store(true, Ordering::SeqCst));
    let label = login_with(&p, &claude, true, &|| stop.load(Ordering::SeqCst))?;
    println!("{} is signed in as {label}", p.name);
    Ok(())
}

/// The CLI's sign-in, under the profile's exclusive lease (taken within 10 s, as the app does),
/// the child in this terminal's foreground (`terminal`: inherited stdio). One way out for every
/// child: stopped and reaped before the lease is released. The account's label, or why not.
fn login_with(p: &Profile, claude: &Path, terminal: bool, stopped: &dyn Fn() -> bool) -> Result<String> {
    if !p.kind.signs_in() {
        bail!("only a Claude account profile signs in");
    }
    if let Some(why) = policy_conflict(p) {
        bail!("{why}");
    }
    let t = Instant::now();
    let lease = loop {
        match sign_in_lease(p)? {
            Some(l) => break l,
            None if stopped() => bail!("stopped — {} isn't signed in", p.name),
            None if t.elapsed() < Duration::from_secs(10) => std::thread::sleep(Duration::from_millis(200)),
            None => bail!("{} is being signed in already, or Echo is using it (writing minutes, or a Claude session open in Echo's window on it — close that session) — try again in a moment", p.name),
        }
    };
    fs::create_dir_all(p.config_folder())?;
    // In this terminal's foreground, not `detach`ed: it reads the paste code, and Ctrl-C reaches
    // it. No login shell (its job control would take the terminal): the terminal's environment is
    // already the user's; the profile's routing is applied on top.
    let mut c = build_command(p, claude, &login_args(p), &paths::config_dir(), false, false);
    if terminal {
        c.stdin(Stdio::inherit()).stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        c.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let mut child = c.spawn()?;
    let watched = (|| -> Result<std::process::ExitStatus> {
        loop {
            if let Some(s) = child.try_wait()? {
                return Ok(s);
            }
            if stopped() {
                bail!("stopped");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    })();
    let status = match watched {
        Ok(s) => s,
        Err(e) => {
            stop_foreground(&mut child);
            if !stopped() {
                return Err(e);
            }
            child.wait()?
        }
    };
    if stopped() {
        bail!("stopped — {} isn't signed in", p.name);
    }
    if !status.success() {
        bail!("the sign-in didn't finish");
    }
    let a = match observe_under(p, &lease, stopped) {
        Err(_) if stopped() => bail!("stopped — the sign-in of {} wasn't checked", p.name),
        r => r?,
    };
    let settled = settle(p, a.as_ref());
    drop(lease);
    match settled {
        Ok(_) => Ok(a.map(|a| a.label()).unwrap_or_default()),
        Err(e) if e.downcast_ref::<SignInNeeded>().is_some_and(|s| s.now.is_some()) => Err(anyhow!("{e:#} (or: echoctl ai accept {})", p.id)),
        Err(e) => Err(e),
    }
}

/// Stop a child in our own process group (a terminal's foreground: the group is the terminal's,
/// so only its pid is signalled): TERM, then KILL after 3 s. On Windows its whole tree at once
/// (`taskkill /T /F`: a `claude.cmd` launcher runs Claude Code as its child).
fn stop_foreground(child: &mut Child) {
    #[cfg(windows)]
    stop_group(child.id());
    #[cfg(unix)]
    {
        // SAFETY: signalling our own child by pid.
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(3) {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = child.kill();
    let _ = child.wait();
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
        email: None,
        sso: false,
    })
}

/// `echoctl ai add-claude <id> [--name …] [--live-model haiku] [--notes-model sonnet] [--email …] [--sso]`
pub fn add_claude(id: &str, name: Option<String>, live_model: Option<String>, notes_model: Option<String>, email: Option<String>, sso: bool) -> Result<()> {
    add(Profile {
        id: id.into(),
        name: name.unwrap_or_else(|| format!("Claude · {id}")),
        kind: Kind::Claude,
        base_url: None,
        token: None,
        live_model: Some(live_model.unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into())),
        notes_model: Some(notes_model.unwrap_or_else(|| crate::config::DEFAULT_NOTES_MODEL.into())),
        account: None,
        email: email_arg(email)?,
        sso,
    })?;
    println!("sign it in once: Sign in… in the Echo window, or echoctl ai login {id}");
    Ok(())
}

fn email_arg(email: Option<String>) -> Result<Option<String>> {
    let email = email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    if email.as_deref().is_some_and(|e| !e.contains('@') || e.chars().any(char::is_whitespace)) {
        bail!("that isn't an email address");
    }
    Ok(email)
}

/// `echoctl ai set <id> [--email … | --no-email] [--sso on|off]`: how a Claude profile signs in.
pub fn set_sign_in(id: &str, email: Option<String>, no_email: bool, sso: Option<bool>) -> Result<()> {
    let email = email_arg(email)?;
    let (name, email, sso) = Settings::update(|s| {
        let p = s.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        if !p.kind.signs_in() {
            bail!("{} doesn't sign in (only Claude account profiles do)", p.name);
        }
        if no_email {
            p.email = None;
        } else if email.is_some() {
            p.email = email.clone();
        }
        if let Some(sso) = sso {
            p.sso = sso;
        }
        Ok((p.name.clone(), p.email.clone(), p.sso))
    })?;
    println!("{name}: email {}, single sign-on {}", email.as_deref().unwrap_or("not set"), if sso { "on" } else { "off" });
    Ok(())
}

/// `echoctl ai set <id> --notes-model <m>`, the menu's "Minutes model for …": the model that writes
/// a profile's minutes. `system` → `config.notes_model` ("default" = Claude Code's own; empty or
/// sonnet = unset); `claude` / `local` → its `notes_model` (empty = back to sonnet / its live
/// model). Returns the profile's name and its minutes model now, in words.
pub fn set_notes_model(id: &str, model: &str) -> Result<(String, String)> {
    let m = model.trim().to_string();
    if id == SYSTEM {
        let mut cfg = Config::load()?;
        cfg.notes_model = Some(m).filter(|m| !m.is_empty() && m != crate::config::DEFAULT_NOTES_MODEL);
        cfg.save()?;
        let p = system_profile();
        return Ok((p.name.clone(), p.notes_model_label()));
    }
    Settings::update(|s| {
        let p = s.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        p.notes_model = Some(m.clone()).filter(|m| !m.is_empty());
        Ok((p.name.clone(), p.notes_model_label()))
    })
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
                match observe(&p, &|| false) {
                    Ok(Some(a)) => (Some(true), Some(a.label()), settle(&p, Some(&a)).err().map(|e| format!("{e:#}"))),
                    Ok(None) => (Some(false), None, None),
                    Err(e) => (None, None, Some(format!("{e:#}"))),
                }
            }
            _ => (None, None, check(&p, None).err().map(|e| format!("{e:#}"))),
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
                "notes_model": p.notes_model(), "notes_model_label": p.notes_model_label(), "account": p.account, "email": p.email, "sso": p.sso,
                // `local`: where the requests go when it isn't this computer
                "host": (p.kind == Kind::Local && !p.on_this_computer()).then(|| p.base_url.clone()).flatten(),
                // `claude`: its sign-in now (`signin::state`; no process runs for it)
                "signin": crate::signin::state(p),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e:#}") }),
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
"auth status") echo x >> "$d/status.count"; [ -f "$d/slow" ] && sleep 1; [ -f "$d/stall" ] && sleep 30; cat "$d/status.json" ;;
"-p "*) cat > /dev/null; [ -f "$d/p.hang" ] && { cat "$d/p.hang" >&2; exec sleep 60; }; cat "$d/p.reply" ;;
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
        fs::remove_file(&cached).unwrap();
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

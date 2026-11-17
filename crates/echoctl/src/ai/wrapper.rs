//! A wrapper profile (claude-code-runs R12–R15): a command the user installed that starts Claude Code its own way (a
//! company's CLI with its gateway, sign-in and hooks), started by Echo in place of `claude`, on the user's own setup as
//! `system` is. The wrapper can add its own settings, hooks and flags, so a run through it proves Echo's own settings
//! are in force before anything acts on the text it is given:
//!
//! - a run in print mode goes through `echoctl wrapper-run` (`relay`): its prompt reaches the wrapper only on stdin as
//!   a stream-json message, sent once the session-start hook of the run's own settings file has answered with a value
//!   made for that run; Echo's guard there holds every tool call until Echo has checked the run's start event (a Claude
//!   Code new enough for the guard, the permission mode, tools and MCP servers as Echo set them) and marked the run
//!   checked; a start event that doesn't match stops the run, saying why;
//! - a terminal session (`terminal_proof`): that hook's answer is the check — it writes the mark itself, and the
//!   call session's first prompt is typed in only after it.
//!
//! The marks live in `.plugins/.proof/` of the data folder, which every session's guard refuses to write to; each is
//! named by its run's value, which only that run's settings file and Echo know.

use super::*;

/// The command a wrapper profile starts, and the plugin that offered it, if one did.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Wrapper {
    /// The absolute path Echo resolved when the profile was added (links followed); every start uses it.
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offered_by: Option<String>,
    /// That plugin's marketplace (another one's plugin of the same id offers nothing to this profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offered_market: Option<String>,
    /// That plugin's name, as shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offered_name: Option<String>,
    /// The program name that plugin offered when the user took it (an update offering another is asked on the profile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offered_command: Option<String>,
}

/// The prompt of a print run for `wrapper-run`, passed in its environment (never on a command line).
pub const PROMPT_VAR: &str = "ECHOCTL_WRAP_PROMPT";
/// What the session-start hook prints: this, then the run's value.
const HELLO: &str = "echo-proof:";
/// How long a wrapper may take to start Claude Code (a sign-in waiting, a network check) before the run stops (R15).
const START_SECS: u64 = 90;
/// How long the guard holds a tool call for the run's mark: within its hook time limit (`guard::settings`, 10 s).
const HOLD_SECS: u64 = 8;

/// Where the marks are: inside `.plugins/`, which the sessions' guards refuse to write to (`plugins::agent::inside_root`)
/// and the layout's prune keeps (`plugins::agent::PROOF`).
pub fn proof_dir() -> PathBuf {
    crate::plugins::agent::root().join(crate::plugins::agent::PROOF)
}

fn mark(value: &str) -> PathBuf {
    proof_dir().join(value)
}

/// A terminal session's mark (its hook writes it): `<value>.t`, cleared when Echo starts, as no session outlives it.
fn terminal_mark(value: &str) -> PathBuf {
    proof_dir().join(format!("{value}.t"))
}

/// Echo starts: no terminal session of an earlier run of it is alive, so their marks go.
pub fn clear_terminal_marks() {
    for e in fs::read_dir(proof_dir()).into_iter().flatten().flatten() {
        if e.file_name().to_string_lossy().ends_with(".t") {
            let _ = fs::remove_file(e.path());
        }
    }
}

fn random_hex(n: usize) -> Result<String> {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).map_err(|e| anyhow!("no secure random source: {e}"))?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// A fresh value for one run (32 hex digits). A run's mark goes when it ends (the relay's end, a terminal's exit), never
/// by age; its files (`proof-<id>…`, read only when Claude Code starts) go a day after.
pub fn new_value() -> Result<String> {
    sweep_files();
    random_hex(16)
}

/// The proof's files a day old, left by a run that was killed.
fn sweep_files() {
    for e in fs::read_dir(paths::config_dir().join("minutes-run")).into_iter().flatten().flatten() {
        let old = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(86_400));
        if old && e.file_name().to_string_lossy().starts_with("proof-") {
            let _ = fs::remove_file(e.path());
        }
    }
}

/// The run with `value` is over: its mark goes.
pub fn unmark(value: &str) {
    if valid_value(value) {
        let _ = fs::remove_file(mark(value));
        let _ = fs::remove_file(terminal_mark(value));
    }
}

fn valid_value(v: &str) -> bool {
    v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The profile's command, as stored: a full path (one that isn't fails only this profile's jobs).
pub fn command_of(p: &Profile) -> Result<PathBuf> {
    let w = p.wrapper.as_ref().filter(|_| p.kind == Kind::Wrapper).ok_or_else(|| anyhow!("{} names no command", p.name))?;
    let c = PathBuf::from(&w.command);
    if !c.is_absolute() {
        bail!("{} names its program without a full path ({}) — add it again", p.name, w.command);
    }
    Ok(c)
}

/// Can a job start on it now: its command is still there.
pub(super) fn check(p: &Profile) -> Result<()> {
    let c = command_of(p)?;
    if !c.is_file() {
        bail!("{} starts {}, which isn't there any more — install it again, or choose another AI profile", p.name, c.display());
    }
    Ok(())
}

// ── adding one ──────────────────────────────────────────────────────────────────────────────

/// `command` as a run's login shell finds it (a name on its PATH, or a path), links followed.
pub fn resolve(command: &str) -> Result<PathBuf> {
    let c = command.trim();
    if c.is_empty() {
        bail!("no command given");
    }
    let found = if c.contains('/') || c.contains('\\') {
        PathBuf::from(c)
    } else {
        if !program_name(c) {
            bail!("“{c}” isn't a program name (letters, digits, “.”, “_”, “-”) or a path");
        }
        find_in_login_shell(c)?
    };
    if !found.is_absolute() {
        bail!("{} isn't a full path", found.display());
    }
    let real = dunce::canonicalize(&found).map_err(|_| anyhow!("{} isn't there", found.display()))?;
    if !real.is_file() {
        bail!("{} isn't a program", real.display());
    }
    Ok(real)
}

/// One program name, no arguments: letters, digits, `.`, `_`, `-` (echo-plugins R24).
pub fn program_name(c: &str) -> bool {
    !c.is_empty() && c.len() <= 64 && !c.starts_with(['.', '-']) && c.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

#[cfg(unix)]
fn find_in_login_shell(name: &str) -> Result<PathBuf> {
    if !super::use_shell() {
        // Runs start without the login shell here (the test data's environment): look where they would, on this
        // process's own PATH, never through the user's start-up files.
        return std::env::var_os("PATH")
            .and_then(|path| on_path(name, &path))
            .ok_or_else(|| anyhow!("“{name}” isn't on your PATH — install it, or give its full path"));
    }
    // The name goes in as `$0`, never into the script's text.
    let out = in_login_shell("command -v -- \"$0\"", name).context("asking your login shell where it is")?;
    out.lines().rev().map(str::trim).find(|l| l.starts_with('/')).map(PathBuf::from).ok_or_else(|| anyhow!("“{name}” isn't on your PATH — install it, or give its full path"))
}

/// What `script` prints run by the user's login shell as a terminal starts it, `arg` its `$0`, within 20 s.
#[cfg(unix)]
fn in_login_shell(script: &str, arg: &str) -> Result<String> {
    let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
    let mut c = Command::new(shell);
    c.args(["-l", "-i", "-c", script]).arg(arg).current_dir(paths::home()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    detach(&mut c);
    let mut child = c.spawn()?;
    Ok(wait_output(&mut child, Duration::from_secs(20), &|| false)?.0)
}

/// The PATH a terminal has: the login shell's, asked once a process (the app started from the Finder has a minimal
/// one) — or this process's own where runs start without the login shell (the test data's environment), and on Windows.
pub(crate) fn login_path() -> Option<std::ffi::OsString> {
    #[cfg(unix)]
    if super::use_shell() {
        static SEEN: std::sync::OnceLock<Option<std::ffi::OsString>> = std::sync::OnceLock::new();
        return SEEN
            .get_or_init(|| {
                let out = in_login_shell("printf '<<echo:path>>%s<</echo:path>>' \"$PATH\"", "echo").ok()?;
                Some(out.rsplit_once("<<echo:path>>")?.1.split_once("<</echo:path>>")?.0.into())
            })
            .clone();
    }
    std::env::var_os("PATH")
}

/// The first `name` a program can be started from in the absolute folders of a `PATH` value.
#[cfg(unix)]
fn on_path(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(path)
        .filter(|d| d.is_absolute())
        .map(|d| d.join(name))
        .find(|f| std::fs::metadata(f).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

#[cfg(not(unix))]
fn find_in_login_shell(name: &str) -> Result<PathBuf> {
    let p = PathBuf::from(crate::app::find_program(name));
    p.is_absolute().then_some(p).ok_or_else(|| anyhow!("“{name}” isn't on your PATH — install it, or give its full path"))
}

/// Why `path` can't be a wrapper: inside Echo's data or config folder (a plugin's package or store among them).
pub fn outside_echo(path: &Path) -> Result<()> {
    let real = |p: PathBuf| dunce::canonicalize(&p).unwrap_or(p);
    // macOS and Windows file systems ignore case by default: so does this.
    let fold = |p: &Path| if cfg!(any(target_os = "macos", windows)) { PathBuf::from(p.to_string_lossy().to_lowercase()) } else { p.to_path_buf() };
    for (root, what) in [(real(paths::home()), "Echo's data folder"), (real(paths::config_dir()), "Echo's own folder")] {
        if fold(path).starts_with(fold(&root)) {
            bail!("{} is inside {what} (where plugins and what they keep are): a wrapper is a program you installed", path.display());
        }
    }
    Ok(())
}

/// What `path --version` answers when it is Claude Code (its first line naming it), within 20 s — asked as a run on it
/// would start it (the login shell's environment: a `#!/usr/bin/env node` wrapper finds its node).
pub fn claude_version(path: &Path) -> Result<String> {
    let p = Profile { id: "wrapper-check".into(), name: path.display().to_string(), kind: Kind::Wrapper, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false,
        models: Default::default(),
        wrapper: Some(Wrapper { command: path.display().to_string(), offered_by: None, offered_market: None, offered_name: None, offered_command: None }) };
    let mut child = super::claude_command(&p, &["--version".into()], &paths::home(), false).map_err(|e| anyhow!("{} can't be started: {e:#}", path.display()))?;
    let (out, ..) = wait_output(&mut child, Duration::from_secs(20), &|| false).with_context(|| format!("asking {} its version", path.display()))?;
    let v = version_line(&out).ok_or_else(|| anyhow!("{} doesn't answer --version as Claude Code does", path.display()))?;
    if !v.contains("(Claude Code") && !crate::guard::new_enough(&v) {
        let [a, b, c] = crate::guard::MIN_CLAUDE;
        bail!("{} starts Claude Code {v} — Echo's guard needs {a}.{b}.{c} or later", path.display());
    }
    Ok(v)
}

/// The line of a `--version` answer that names Claude Code, or a bare version of Claude Code's 2.x as its last line
/// ("2.1.325": a wrapper passing on the Claude Code it starts, after what a login shell's startup files printed). A line
/// naming another program never, nor another program's bare version ("10.9.2"). How new it is: `guard::new_enough`.
pub(crate) fn version_line(out: &str) -> Option<String> {
    let mut lines = out.lines().map(str::trim).filter(|l| !l.is_empty());
    if let Some(l) = lines.clone().find(|l| l.contains("(Claude Code")) {
        return Some(l.to_owned());
    }
    let bare = |l: &str| l.starts_with("2.") && l.split('.').count() == 3 && l.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
    lines.next_back().filter(|l| bare(l)).map(str::to_owned)
}

/// The checks before a wrapper profile is added (R13): resolved, outside Echo's folders, answering as Claude Code.
/// The absolute path and its version line.
pub fn admit(command: &str) -> Result<(PathBuf, String)> {
    let path = resolve(command)?;
    outside_echo(&path)?;
    let v = claude_version(&path)?;
    Ok((path, v))
}

// ── the run's own settings ──────────────────────────────────────────────────────────────────

fn hook(exe: &Path, args: &str) -> String {
    let p = exe.display().to_string();
    if cfg!(windows) {
        format!("\"{}\" proof {args}", p.replace('\\', "/"))
    } else {
        format!("'{}' proof {args}", p.replace('\'', r"'\''"))
    }
}

/// `base` (the run's settings, or `{}`) with the proof's hooks: the session-start hook answering with `value` (in a
/// terminal it marks the run itself), and the gate before every tool call (refusing all of them: `deny_all`).
pub fn proof_settings(base: Value, exe: &Path, value: &str, deny_all: bool, terminal: bool) -> Value {
    let mut s = if base.is_object() { base } else { json!({}) };
    s["disableAllHooks"] = json!(false);
    let hello = hook(exe, &format!("hello {value}{}", if terminal { " --mark" } else { "" }));
    let gate = hook(exe, &format!("gate {value}{}", if deny_all { " --deny-all" } else { "" }));
    let push = |s: &mut Value, event: &str, entry: Value| {
        if !s["hooks"].is_object() {
            s["hooks"] = json!({});
        }
        if !s["hooks"][event].is_array() {
            s["hooks"][event] = json!([]);
        }
        if let Some(a) = s["hooks"][event].as_array_mut() {
            a.push(entry);
        }
    };
    push(&mut s, "SessionStart", json!({"hooks": [{"type": "command", "command": hello, "timeout": 10}]}));
    push(&mut s, "PreToolUse", json!({"matcher": "*", "hooks": [{"type": "command", "command": gate, "timeout": 10, "onFailure": "block"}]}));
    s
}

/// Owner-only, in Echo's run folder.
fn write_private(name: &str, bytes: &[u8]) -> Result<PathBuf> {
    use std::io::Write;
    let dir = paths::config_dir().join("minutes-run");
    fs::create_dir_all(&dir)?;
    let f = dir.join(name);
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(&f)?.write_all(bytes)?;
    Ok(f)
}

fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].as_str())
}

/// Replace `--settings <f>` with the proof's settings built on it (or add them): the new file's path, named by `id` (never
/// by the run's value).
fn with_proof_settings(args: &mut Vec<String>, exe: &Path, value: &str, id: &str, deny_all: bool, terminal: bool) -> Result<PathBuf> {
    let at = args.iter().position(|a| a == "--settings");
    let base = match at.and_then(|i| args.get(i + 1)) {
        Some(f) => serde_json::from_slice(&fs::read(f).with_context(|| format!("reading the run's settings {f}"))?).with_context(|| format!("the run's settings {f} aren't JSON"))?,
        None => json!({}),
    };
    let f = write_private(&format!("proof-{id}.json"), &serde_json::to_vec_pretty(&proof_settings(base, exe, value, deny_all, terminal))?)?;
    match at {
        Some(i) if i + 1 < args.len() => args[i + 1] = f.display().to_string(),
        _ => args.splice(0..0, ["--settings".to_string(), f.display().to_string()]).for_each(drop),
    }
    Ok(f)
}

/// A terminal session on a wrapper profile: its settings with the proof's hooks (the hook's answer is the check, R14).
/// The run's value: the session waits for its mark (`marked`) before anything is typed in.
#[cfg_attr(not(feature = "tray"), allow(dead_code))] // the window's terminals (`tray`)
pub fn terminal_proof(args: &mut Vec<String>) -> Result<String> {
    let value = new_value()?;
    let exe = std::env::current_exe()?;
    with_proof_settings(args, &exe, &value, &random_hex(6)?, false, true)?;
    Ok(value)
}

/// Has the run with `value` been marked checked?
pub fn marked(value: &str) -> bool {
    valid_value(value) && (mark(value).is_file() || terminal_mark(value).is_file())
}

/// `echoctl proof hello|gate <value> [--mark|--deny-all]`: the proof's hooks. Hello prints the value (and, `--mark`, in
/// a terminal, marks the run); the gate holds a tool call until the run is marked, then lets the guard judge it (exit
/// 0, no decision), or refuses it (exit 2) — always, with `--deny-all`.
pub fn proof_command(args: &[String]) -> i32 {
    let (what, value, flag) = (args.first().map(String::as_str), args.get(1).map(String::as_str).unwrap_or(""), args.get(2).map(String::as_str));
    if !valid_value(value) {
        eprintln!("Echo: this run's proof is malformed");
        return 2;
    }
    match (what, flag) {
        (Some("hello"), mark_it) => {
            if mark_it == Some("--mark") && (fs::create_dir_all(proof_dir()).is_err() || fs::write(terminal_mark(value), b"").is_err()) {
                eprintln!("Echo: this session can't be marked checked");
                return 2;
            }
            println!("{HELLO}{value}");
            0
        }
        (Some("gate"), deny) => {
            let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut vec![]);
            match crate::guard::checked(value, deny == Some("--deny-all"), Duration::from_secs(HOLD_SECS)) {
                Ok(()) => 0,
                Err(why) => {
                    eprintln!("{why}");
                    2
                }
            }
        }
        _ => {
            eprintln!("Echo: unknown proof step");
            2
        }
    }
}

// ── a print run through the wrapper ─────────────────────────────────────────────────────────

/// Is this a print run (`-p`), the kind `wrapper-run` relays?
pub fn is_print(args: &[String]) -> bool {
    args.first().is_some_and(|a| a == "-p")
}

/// The command for a print run on a wrapper profile: this program's `wrapper-run`, the wrapper and its arguments, the
/// prompt (`-p <prompt>`) taken out and given back as an environment variable. (args for `wrapper-run`, the prompt)
pub fn relay_args(wrapper: &Path, args: &[String]) -> (Vec<String>, Option<String>) {
    let mut rest = args.to_vec();
    // After `-p`, a flag (`--output-format`) or the prompt: a prompt is anything that isn't one plain flag word.
    let flag = |a: &str| a.strip_prefix("--").is_some_and(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
    let prompt = (rest.len() > 1 && rest[0] == "-p" && !flag(&rest[1])).then(|| rest.remove(1));
    let mut out = vec!["wrapper-run".to_string(), "--".to_string(), wrapper.display().to_string()];
    out.extend(rest);
    (out, prompt)
}

/// What a run's start event must show: the guard's Claude Code, and nothing beyond what Echo set.
#[derive(Debug, Default, PartialEq)]
pub struct Expected {
    /// The permission mode Echo set (None: it set none — the user's own, but never one that skips permissions).
    pub mode: Option<String>,
    /// Echo's `--tools`: the built-in tools it may have (None: Echo set none, the session's own).
    pub tools: Option<Vec<String>>,
    /// With `--strict-mcp-config`: the MCP servers from Echo's `--mcp-config` files.
    pub mcp: Option<Vec<String>>,
}

impl Expected {
    pub fn of(args: &[String]) -> Expected {
        let mode = if args.iter().any(|a| a == "--dangerously-skip-permissions") {
            Some("bypassPermissions".to_string())
        } else {
            value_after(args, "--permission-mode").map(str::to_owned)
        };
        let tools = value_after(args, "--tools").map(|t| t.split(',').map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).collect());
        let mcp = args.iter().any(|a| a == "--strict-mcp-config").then(|| {
            args.windows(2)
                .filter(|w| w[0] == "--mcp-config")
                .flat_map(|w| {
                    let v: Value = fs::read(&w[1]).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
                    v["mcpServers"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()
                })
                .collect()
        });
        Expected { mode, tools, mcp }
    }

    /// No tool and no MCP server: every tool call is refused.
    fn none(&self) -> bool {
        self.tools.as_ref().is_some_and(|t| t.is_empty()) && self.mcp.as_ref().is_some_and(|m| m.is_empty())
    }

    /// The start event (`system/init`) checked against it, or why it doesn't match.
    pub fn check(&self, init: &Value) -> Result<(), String> {
        let v = init["claude_code_version"].as_str().unwrap_or("");
        if !crate::guard::new_enough(v) {
            let [a, b, c] = crate::guard::MIN_CLAUDE;
            return Err(format!("it started Claude Code {} — Echo's guard needs {a}.{b}.{c} or later", if v.is_empty() { "of unknown version" } else { v }));
        }
        // A run with no tools refuses every call at the gate: its mode doesn't matter.
        let mode = init["permissionMode"].as_str().unwrap_or("");
        match &self.mode {
            _ if self.none() => {}
            Some(want) if mode != want => return Err(format!("it started Claude Code in permission mode {mode:?}, not Echo's {want:?}")),
            None if mode == "bypassPermissions" => return Err("it started Claude Code skipping permissions, which Echo didn't".into()),
            _ => {}
        }
        let tools: Vec<&str> = init["tools"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        if let Some(want) = &self.tools {
            if let Some(t) = tools.iter().find(|t| !t.starts_with("mcp__") && !want.iter().any(|w| w == *t)) {
                return Err(format!("it gave Claude Code the tool {t}, which Echo didn't"));
            }
        }
        if let Some(want) = &self.mcp {
            let servers: Vec<&str> = init["mcp_servers"].as_array().map(|a| a.iter().filter_map(|s| s["name"].as_str()).collect()).unwrap_or_default();
            if let Some(s) = servers.iter().find(|s| !want.iter().any(|w| w == *s)) {
                return Err(format!("it gave Claude Code the MCP server {s}, which Echo didn't"));
            }
            if let Some(t) = tools.iter().find(|t| t.strip_prefix("mcp__").is_some_and(|r| !want.iter().any(|w| r.starts_with(&format!("{w}__"))))) {
                return Err(format!("it gave Claude Code the tool {t}, which Echo didn't"));
            }
        }
        Ok(())
    }
}

/// Is `v` the session-start hook's answer with `value`?
fn says_hello(v: &Value, value: &str) -> bool {
    let want = format!("{HELLO}{value}");
    v["type"] == "system" && v["subtype"] == "hook_response" && ["output", "stdout"].iter().any(|k| v[*k].as_str().is_some_and(|o| o.lines().any(|l| l.trim() == want)))
}

/// What one line of a run through a wrapper means for Echo's proof (`wrapper_ready`).
#[derive(Debug, PartialEq)]
pub enum Ready {
    /// The run's own session-start hook answered with its value: its text may go now.
    Hello,
    /// Its start event shows Echo's settings in force: mark it checked (the gate lets tool calls through to the guard).
    Checked,
    /// It doesn't hold: stop the run, saying why.
    Stop(String),
    /// Nothing for the proof.
    Pass,
}

/// One line `v` of a run through a wrapper, given whether its hook answered (`hello`) and it was `checked` (R14).
pub fn wrapper_ready(v: &Value, value: &str, exp: &Expected, hello: bool, checked: bool) -> Ready {
    if v["type"] == "system" && v["subtype"] == "init" && !checked {
        if !hello {
            return Ready::Stop("Claude Code started its work before Echo's checks answered".into());
        }
        return match exp.check(v) {
            Ok(()) => Ready::Checked,
            Err(why) => Ready::Stop(why),
        };
    }
    if !hello && says_hello(v, value) {
        return Ready::Hello;
    }
    Ready::Pass
}

/// How the run's text reaches the wrapper.
enum Input {
    /// One prompt (Echo's `-p <prompt>`, or the text on this relay's stdin): sent as one message, then stdin closed.
    Once(String),
    /// Echo writes stream-json messages itself: passed on as they come.
    Stream,
}

/// A run's arguments made ready for the wrapper: stream-json in and out, system prompts in files, the proof's settings.
/// (args, the files made for it)
fn prepare(mut args: Vec<String>, exe: &Path, value: &str, exp: &Expected) -> Result<(Vec<String>, Vec<PathBuf>)> {
    let mut made = vec![];
    let id = random_hex(6)?;
    if let Some(i) = args.iter().position(|a| a == "--output-format") {
        match args.get(i + 1).map(String::as_str) {
            Some("json") => args[i + 1] = "stream-json".into(),
            Some("stream-json") => {}
            other => bail!("a run through a wrapper needs JSON output, not {}", other.unwrap_or("none")),
        }
    } else {
        args.extend(["--output-format".into(), "stream-json".into()]);
    }
    if !args.iter().any(|a| a == "--verbose") {
        args.push("--verbose".into());
    }
    if !args.iter().any(|a| a == "--input-format") {
        args.extend(["--input-format".into(), "stream-json".into()]);
    }
    for flag in ["--system-prompt", "--append-system-prompt"] {
        if let Some(i) = args.iter().position(|a| a == flag) {
            if let Some(text) = args.get(i + 1).cloned() {
                let f = write_private(&format!("proof-{id}{flag}.txt"), text.as_bytes())?;
                args[i] = format!("{flag}-file");
                args[i + 1] = f.display().to_string();
                made.push(f);
            }
        }
    }
    made.push(with_proof_settings(&mut args, exe, value, &id, exp.none(), false)?);
    Ok((args, made))
}

/// A user message for stream-json input.
fn message(text: &str) -> String {
    json!({"type": "user", "message": {"role": "user", "content": text}}).to_string()
}

/// A result line the run's reader takes as its end: Echo stopped it, saying why.
fn stopped_line(why: &str) -> String {
    json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "result": format!("Echo stopped this run: {why}")}).to_string()
}

/// `echoctl wrapper-run -- <wrapper> <args…>`: run a print run through the wrapper (the module's header). Its output
/// is the wrapper's, line by line; the exit status the wrapper's, or 1 when Echo stopped it.
pub fn relay(argv: &[String]) -> i32 {
    let argv: Vec<String> = argv.iter().skip_while(|a| *a == "--").cloned().collect();
    let Some((wrapper, args)) = argv.split_first() else {
        eprintln!("Echo: no wrapper to run");
        return 2;
    };
    let prompt = std::env::var(PROMPT_VAR).ok();
    // Stopped (a job's time limit, Echo quitting): the wrapper's whole group goes, and the run's files and mark.
    // Quickly: whoever stopped it kills it outright 3 s later.
    let _ = ctrlc::set_handler(|| {
        // Stopped while the wrapper is being started (pid 0): wait for it, so it isn't left running.
        let t = Instant::now();
        while RUNNING.lock().ok().and_then(|r| r.as_ref().map(|x| x.0)) == Some(0) && t.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some((pid, files, value)) = RUNNING.lock().ok().and_then(|r| r.clone()).filter(|r| r.0 != 0) {
            remove_all(&files, &value);
            #[cfg(unix)]
            // SAFETY: signalling the process group of the wrapper this relay started.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGTERM);
                std::thread::sleep(Duration::from_millis(800));
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            stop_wrapper(pid);
        }
        std::process::exit(1);
    });
    let out = std::io::stdout();
    match relay_with(Path::new(wrapper), args.to_vec(), prompt, Box::new(std::io::stdin()), &mut out.lock(), START_SECS) {
        Ok(code) => code,
        Err(why) => {
            eprintln!("Echo stopped this run: {why}");
            1
        }
    }
}

/// The wrapper this relay runs (its pid, its group's leader), the run's files and value: what a stop takes down.
static RUNNING: std::sync::Mutex<Option<(u32, Vec<PathBuf>, String)>> = std::sync::Mutex::new(None);

fn remove_all(made: &[PathBuf], value: &str) {
    for f in made {
        let _ = fs::remove_file(f);
    }
    unmark(value);
}

/// Stop the wrapper and everything it started (it leads its own group; on Windows its tree).
fn stop_wrapper(pid: u32) {
    super::stop_group(pid);
}

/// `relay`, its parts given (tests): the wrapper, its arguments, the prompt, this relay's stdin and stdout, how long the
/// wrapper may take to start Claude Code and answer.
pub fn relay_with(wrapper: &Path, args: Vec<String>, prompt: Option<String>, mut stdin: Box<dyn std::io::Read + Send>, out: &mut dyn std::io::Write, start_secs: u64) -> Result<i32, String> {
    use std::io::{BufRead, Write};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let value = new_value().map_err(|e| e.to_string())?;
    let exp = Expected::of(&args);
    let stream = args.windows(2).any(|w| w[0] == "--input-format" && w[1] == "stream-json");
    let input = match (prompt, stream) {
        (Some(p), _) => Input::Once(p),
        (None, true) => Input::Stream,
        (None, false) => {
            let mut t = String::new();
            stdin.read_to_string(&mut t).map_err(|e| format!("the prompt can't be read: {e}"))?;
            Input::Once(t)
        }
    };
    let (args, made) = prepare(args, &exe, &value, &exp).map_err(|e| format!("{e:#}"))?;
    let cleanup = |made: &[PathBuf]| {
        remove_all(made, &value);
        if let Ok(mut r) = RUNNING.lock() {
            *r = None;
        }
    };
    let mut c = Command::new(wrapper);
    #[cfg(windows)]
    {
        use crate::app::NoWindow;
        c = c.no_window();
    }
    c.args(&args).env_remove(PROMPT_VAR).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit());
    // Its own group: a stop reaches a Claude Code it started as a child, too.
    detach(&mut c);
    if let Ok(mut r) = RUNNING.lock() {
        *r = Some((0, made.clone(), value.clone()));
    }
    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => {
            cleanup(&made);
            return Err(format!("{} can't be started: {e}", wrapper.display()));
        }
    };
    if let Ok(mut r) = RUNNING.lock() {
        *r = Some((child.id(), made.clone(), value.clone()));
    }
    let mut child_in = child.stdin.take();
    let child_out = child.stdout.take().expect("piped");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(child_out).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let started = Instant::now();
    let mut asked: Option<Instant> = None;
    let (mut hello, mut checked) = (false, false);
    let mut stdin = Some(stdin);
    let failed: Option<String> = loop {
        // Until it is checked, the wrapper has `start_secs` to answer each step (R15); after, the job's own limit holds.
        let deadline = match asked {
            _ if checked => None,
            None => Some(started + Duration::from_secs(start_secs)),
            Some(at) => Some(at + Duration::from_secs(start_secs)),
        };
        let got = match deadline {
            Some(d) => rx.recv_timeout(d.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected),
        };
        let line = match got {
            Ok(l) => l,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) if checked || !hello => break (!checked).then(|| format!("{} ended before Echo's checks answered", wrapper.display())),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break Some(format!("Claude Code started through {} ended before reporting its start", wrapper.display())),
            Err(_) if !hello => break Some(format!("{} didn't start Claude Code with Echo's checks within {start_secs} s (waiting for a sign-in?)", wrapper.display())),
            Err(_) => break Some(format!("Claude Code started through {} didn't report its start within {start_secs} s", wrapper.display())),
        };
        let v: Value = serde_json::from_str(line.trim()).unwrap_or(Value::Null);
        let step = super::wrapper_ready(&v, &value, &exp, hello, checked);
        match &step {
            Ready::Stop(why) => break Some(format!("{why} (through {})", wrapper.display())),
            Ready::Checked => {
                if fs::create_dir_all(proof_dir()).and_then(|()| fs::write(mark(&value), b"")).is_err() {
                    break Some("the run can't be marked checked".into());
                }
                checked = true;
            }
            Ready::Hello | Ready::Pass => {}
        }
        if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
            break Some("Echo stopped reading the run".into());
        }
        if step == Ready::Hello {
            hello = true;
            asked = Some(Instant::now());
            match &input {
                Input::Once(text) => {
                    let sent = child_in.take().map(|mut i| writeln!(i, "{}", message(text)).and_then(|()| i.flush()));
                    if let Some(Err(e)) = sent {
                        break Some(format!("the prompt couldn't be sent: {e}"));
                    }
                }
                Input::Stream => {
                    if let (Some(mut from), Some(mut to)) = (stdin.take(), child_in.take()) {
                        std::thread::spawn(move || {
                            let _ = std::io::copy(&mut from, &mut to);
                        });
                    }
                }
            }
        }
    };
    let code = match failed {
        Some(why) => {
            stop_wrapper(child.id());
            let _ = child.wait();
            let _ = writeln!(out, "{}", stopped_line(&why)).and_then(|()| out.flush());
            cleanup(&made);
            return Err(why);
        }
        None => {
            drop(child_in);
            child.wait().ok().and_then(|s| s.code()).unwrap_or(1)
        }
    };
    cleanup(&made);
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn init(version: &str, mode: &str, tools: &[&str], servers: &[&str]) -> Value {
        json!({"type": "system", "subtype": "init", "claude_code_version": version, "permissionMode": mode, "tools": tools,
            "mcp_servers": servers.iter().map(|s| json!({"name": s, "status": "connected"})).collect::<Vec<_>>()})
    }

    #[cfg(unix)]
    #[test]
    fn without_the_login_shell_a_program_is_found_on_the_path_given_in_its_order() {
        let root = std::env::temp_dir().join(format!("echo-on-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (first, second, empty) = (root.join("first"), root.join("second"), root.join("empty"));
        for d in [&first, &second, &empty] {
            std::fs::create_dir_all(d).unwrap();
        }
        use std::os::unix::fs::PermissionsExt;
        let program = |f: PathBuf, mode: u32| {
            std::fs::write(&f, "").unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(mode)).unwrap();
        };
        program(empty.join("acme-claude"), 0o644); // can't be started: skipped, as the shell skips it
        program(first.join("acme-claude"), 0o755);
        program(second.join("acme-claude"), 0o755);
        std::fs::create_dir_all(empty.join("globex-claude")).unwrap(); // a folder of that name is no program
        // An empty or relative folder (the current one) is never looked in.
        let path = std::env::join_paths([PathBuf::new(), PathBuf::from("."), empty.clone(), first.clone(), second.clone()]).unwrap();
        assert_eq!(on_path("acme-claude", &path), Some(first.join("acme-claude")));
        assert_eq!(on_path("globex-claude", &path), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_plugin_offers_one_program_name_with_no_arguments() {
        for ok in ["acme-claude", "claude", "acme_claude.2"] {
            assert!(program_name(ok), "{ok}");
        }
        for bad in ["", "acme claude", "acme-claude --x", "../bin/claude", "-p", ".hidden", "a;b", "a$(b)", "a/b"] {
            assert!(!program_name(bad), "{bad}");
        }
    }

    #[test]
    fn the_runs_settings_keep_its_guard_and_add_the_hello_and_the_gate() {
        let base = json!({"claudeMdExcludes": ["x"], "hooks": {"PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "guard"}]}]}});
        let v = "0123456789abcdef0123456789abcdef";
        let s = proof_settings(base, Path::new("/Apps/echoctl"), v, false, false);
        assert_eq!(s["claudeMdExcludes"][0], "x");
        assert_eq!(s["disableAllHooks"], false);
        let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre[0]["hooks"][0]["command"], "guard", "Echo's guard stays");
        assert_eq!(pre[1]["hooks"][0]["command"], format!("'/Apps/echoctl' proof gate {v}"));
        assert_eq!(pre[1]["hooks"][0]["onFailure"], "block");
        assert_eq!(s["hooks"]["SessionStart"][0]["hooks"][0]["command"], format!("'/Apps/echoctl' proof hello {v}"));
        let none = proof_settings(json!({}), Path::new("/Apps/echoctl"), v, true, true);
        assert_eq!(none["hooks"]["PreToolUse"][0]["hooks"][0]["command"], format!("'/Apps/echoctl' proof gate {v} --deny-all"));
        assert_eq!(none["hooks"]["SessionStart"][0]["hooks"][0]["command"], format!("'/Apps/echoctl' proof hello {v} --mark"), "a terminal marks itself");
    }

    #[test]
    fn a_print_runs_prompt_leaves_the_command_line() {
        let (a, prompt) = relay_args(Path::new("/opt/acme/acme-claude"), &args(&["-p", "the meeting “Acme” & calc", "--output-format", "stream-json"]));
        assert_eq!(prompt.as_deref(), Some("the meeting “Acme” & calc"));
        assert_eq!(a, args(&["wrapper-run", "--", "/opt/acme/acme-claude", "-p", "--output-format", "stream-json"]));
        let (a, prompt) = relay_args(Path::new("/w"), &args(&["-p", "--input-format", "stream-json"]));
        assert_eq!(prompt, None, "a stream run sends its own messages");
        assert_eq!(a[3..], args(&["-p", "--input-format", "stream-json"]));
        assert!(is_print(&args(&["-p", "x"])) && !is_print(&args(&["--version"])) && !is_print(&args(&["plugin", "list"])));
    }

    #[test]
    fn the_start_event_must_show_what_echo_set() {
        let exp = Expected::of(&args(&["-p", "x", "--tools", "Bash,Read,Skill", "--strict-mcp-config", "--permission-mode", "dontAsk"]));
        assert_eq!(exp, Expected { mode: Some("dontAsk".into()), tools: Some(args(&["Bash", "Read", "Skill"])), mcp: Some(vec![]) });
        assert!(exp.check(&init("2.1.400", "dontAsk", &["Bash", "Read"], &[])).is_ok(), "fewer tools are fine");
        assert!(exp.check(&init("2.1.200", "dontAsk", &["Bash"], &[])).unwrap_err().contains("2.1.200"), "too old for the guard");
        assert!(exp.check(&init("", "dontAsk", &["Bash"], &[])).unwrap_err().contains("unknown version"));
        assert!(exp.check(&init("2.1.400", "bypassPermissions", &["Bash"], &[])).unwrap_err().contains("bypassPermissions"));
        assert!(exp.check(&init("2.1.400", "dontAsk", &["Bash", "Write"], &[])).unwrap_err().contains("Write"));
        assert!(exp.check(&init("2.1.400", "dontAsk", &["Bash"], &["acme-audit"])).unwrap_err().contains("acme-audit"));
        assert!(exp.check(&init("2.1.400", "dontAsk", &["mcp__acme__send"], &[])).unwrap_err().contains("mcp__acme__send"));
        // A routine acting on its own: Echo sets no tools list; its mode is checked.
        let acts = Expected::of(&args(&["-p", "x", "--dangerously-skip-permissions"]));
        assert_eq!((acts.mode.as_deref(), acts.tools.is_none(), acts.mcp.is_none()), (Some("bypassPermissions"), true, true));
        assert!(acts.check(&init("2.1.400", "bypassPermissions", &["Bash", "Write"], &["anything"])).is_ok());
        // Acting on its own in auto mode (the default): the start event must show auto mode, never skipping permissions.
        let auto = Expected::of(&args(&["-p", "x", "--permission-mode", "auto"]));
        assert!(auto.check(&init("2.1.400", "auto", &["Bash", "Write"], &["anything"])).is_ok());
        assert!(auto.check(&init("2.1.400", "bypassPermissions", &["Bash"], &[])).unwrap_err().contains("bypassPermissions"));
        // Echo set no mode (a routine preparing and asking): the user's own default mode, never one skipping permissions.
        let own = Expected::of(&args(&["-p", "--tools", "Bash"]));
        assert_eq!(own.mode, None);
        assert!(own.check(&init("2.1.400", "acceptEdits", &["Bash"], &[])).is_ok());
        assert!(own.check(&init("2.1.400", "default", &["Bash"], &[])).is_ok());
        assert!(own.check(&init("2.1.400", "bypassPermissions", &["Bash"], &[])).unwrap_err().contains("skipping permissions"));
        // No tools and no server (a lean job): every call refused at the gate, so its mode isn't checked.
        let lean = Expected::of(&args(&["-p", "--tools", "", "--strict-mcp-config"]));
        assert!(lean.check(&init("2.1.400", "plan", &[], &[])).is_ok());
        assert!(Expected::of(&args(&["-p", "--tools", "", "--strict-mcp-config"])).none(), "no tools, no servers: every call refused");
    }

    #[test]
    fn the_runs_mcp_servers_come_from_echos_config_files() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-mcp");
        let f = root.join("mcp.json");
        fs::write(&f, r#"{"mcpServers": {"echo_live": {"command": "x"}}}"#).unwrap();
        let exp = Expected::of(&args(&["-p", "--tools", "", "--mcp-config", f.to_str().unwrap(), "--strict-mcp-config"]));
        assert_eq!(exp.mcp, Some(args(&["echo_live"])));
        assert!(!exp.none());
        assert!(exp.check(&init("2.1.400", "default", &["mcp__echo_live__wait_for_new_lines"], &["echo_live"])).is_ok());
        assert!(exp.check(&init("2.1.400", "default", &["mcp__echo_livex__w"], &["echo_live"])).is_err(), "only its own server's tools");
    }

    #[test]
    fn only_the_hook_answer_with_the_runs_value_counts() {
        let v = "0123456789abcdef0123456789abcdef";
        let r = |out: &str| json!({"type": "system", "subtype": "hook_response", "hook_event": "SessionStart", "output": out, "stdout": out});
        assert!(says_hello(&r(&format!("{HELLO}{v}\n")), v));
        assert!(!says_hello(&r(&format!("{HELLO}{}\n", "f".repeat(32))), v), "another run's value");
        assert!(!says_hello(&r(&format!("prefix {HELLO}{v}")), v), "the whole line");
        assert!(!says_hello(&json!({"type": "system", "subtype": "hook_started", "output": format!("{HELLO}{v}")}), v));
    }

    #[test]
    fn a_wrapper_is_never_inside_echos_folders() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-outside");
        let inside = [paths::home().join("plugins/acme/bin/acme-claude"), paths::config_dir().join("plugins/x/acme-claude")];
        for p in inside {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, "").unwrap();
            assert!(outside_echo(&dunce::canonicalize(&p).unwrap()).is_err(), "{}", p.display());
        }
        let out = root.join("bin/acme-claude");
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        fs::write(&out, "").unwrap();
        assert!(outside_echo(&dunce::canonicalize(&out).unwrap()).is_ok());
    }

    #[test]
    fn the_gate_holds_a_tool_call_until_echo_marked_the_run() {
        let (_env, _root) = crate::paths::testenv::folders("wrapper-gate");
        let v = new_value().unwrap();
        assert!(valid_value(&v));
        assert!(crate::guard::checked(&v, false, Duration::from_millis(300)).unwrap_err().contains("hasn't checked"));
        assert!(crate::guard::checked(&v, true, Duration::from_millis(300)).unwrap_err().contains("no tools"));
        let w = v.clone();
        let marker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            fs::create_dir_all(proof_dir()).unwrap();
            fs::write(mark(&w), b"").unwrap();
        });
        assert!(crate::guard::checked(&v, false, Duration::from_secs(5)).is_ok(), "held, then let through to the guard");
        marker.join().unwrap();
        assert!(crate::guard::checked(&v, true, Duration::from_secs(1)).is_err(), "no tools stays no tools");
        assert!(crate::plugins::agent::inside_root(&mark(&v)), "where only Echo writes");
    }

    /// A stand-in wrapper: it answers the session-start hook as Claude Code would (the value from the run's own settings
    /// file), keeps its arguments and the first message it is sent, then reports the start event in `init.json`.
    #[cfg(unix)]
    fn fake_wrapper(root: &Path, hello: bool, hang: bool) -> PathBuf {
        let w = root.join("acme-claude");
        let say = if hello {
            r#"v=$(sed -n 's/.*proof hello \([0-9a-f]*\).*/\1/p' "$s" | head -1)
printf '{"type":"system","subtype":"hook_response","hook_event":"SessionStart","output":"echo-proof:%s\\n","stdout":"echo-proof:%s\\n"}\n' "$v" "$v""#
        } else {
            ""
        };
        let wait = if hang { "sleep 30" } else { "" };
        fs::write(
            &w,
            format!(
                r#"#!/bin/sh
d="$(dirname "$0")"
printf '%s\n' "$@" > "$d/args.txt"
s=""; prev=""
for a in "$@"; do [ "$prev" = "--settings" ] && s="$a"; prev="$a"; done
cp "$s" "$d/settings.json"
{wait}
{say}
IFS= read -r line
printf '%s\n' "$line" > "$d/turn.txt"
cat "$d/init.json"
sleep 1
printf '{{"type":"result","subtype":"success","is_error":false,"result":"done"}}\n'
"#
            ),
        )
        .unwrap();
        std::os::unix::fs::PermissionsExt::set_mode(&mut fs::metadata(&w).unwrap().permissions(), 0o755);
        fs::set_permissions(&w, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        w
    }

    #[cfg(unix)]
    fn minutes_like(root: &Path) -> Vec<String> {
        let s = root.join("run-settings.json");
        fs::write(&s, r#"{"hooks": {"PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "guard"}]}]}}"#).unwrap();
        args(&["-p", "--output-format", "stream-json", "--verbose", "--settings", s.to_str().unwrap(), "--tools", "Bash,Read", "--strict-mcp-config", "--permission-mode", "dontAsk"])
    }

    #[cfg(unix)]
    #[test]
    fn a_run_through_a_wrapper_gets_its_prompt_on_stdin_only_after_the_hook_answered_and_is_marked_once_checked() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-relay");
        let w = fake_wrapper(&root, true, false);
        fs::write(root.join("init.json"), init("2.1.400", "dontAsk", &["Bash", "Read"], &[]).to_string() + "\n").unwrap();
        let mut out = vec![];
        let prompt = "Write the minutes of “Acme weekly” — \"$(rm -rf ~)\"\nline two";
        let code = relay_with(&w, minutes_like(&root), Some(prompt.into()), Box::new(std::io::empty()), &mut out, 10).unwrap();
        assert_eq!(code, 0);
        let out = String::from_utf8(out).unwrap();
        assert!(out.lines().any(|l| l.contains(r#""subtype":"init""#)) && out.lines().last().unwrap().contains(r#""result":"done""#), "{out}");
        let turn: Value = serde_json::from_str(&fs::read_to_string(root.join("turn.txt")).unwrap()).unwrap();
        assert_eq!(turn["message"]["content"], prompt, "the prompt, whole, on stdin");
        let argv = fs::read_to_string(root.join("args.txt")).unwrap();
        assert!(!argv.contains("Acme weekly"), "never on its command line: {argv}");
        assert!(argv.contains("--input-format\nstream-json"));
        let s: Value = serde_json::from_str(&fs::read_to_string(root.join("settings.json")).unwrap()).unwrap();
        assert_eq!(s["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "guard", "the run's guard stays");
        let gate = s["hooks"]["PreToolUse"][1]["hooks"][0]["command"].as_str().unwrap();
        let value = gate.rsplit(' ').next().unwrap();
        assert!(valid_value(value), "{gate}");
        assert!(!marked(value), "the mark goes with the run");
        let left: Vec<_> = fs::read_dir(paths::config_dir().join("minutes-run")).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with("proof-")).collect();
        assert!(left.is_empty(), "its files go too: {left:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_start_event_that_doesnt_match_stops_the_run_saying_why() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-mismatch");
        let w = fake_wrapper(&root, true, false);
        fs::write(root.join("init.json"), init("2.1.400", "bypassPermissions", &["Bash", "Read"], &[]).to_string() + "\n").unwrap();
        let mut out = vec![];
        let e = relay_with(&w, minutes_like(&root), Some("x".into()), Box::new(std::io::empty()), &mut out, 10).unwrap_err();
        assert!(e.contains("bypassPermissions") && e.contains("acme-claude"), "{e}");
        let last: Value = serde_json::from_str(String::from_utf8(out).unwrap().lines().last().unwrap()).unwrap();
        assert_eq!((last["type"].as_str(), last["is_error"].as_bool()), (Some("result"), Some(true)));
        assert!(last["result"].as_str().unwrap().contains("Echo stopped this run"));
    }

    #[cfg(unix)]
    #[test]
    fn a_wrapper_whose_settings_never_answer_gets_no_prompt() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-silent");
        let w = fake_wrapper(&root, false, false);
        fs::write(root.join("init.json"), init("2.1.400", "dontAsk", &["Bash"], &[]).to_string() + "\n").unwrap();
        let e = relay_with(&w, minutes_like(&root), Some("secret words".into()), Box::new(std::io::empty()), &mut vec![], 2).unwrap_err();
        assert!(e.contains("within 2 s") || e.contains("before Echo's checks"), "{e}");
        assert!(!fs::read_to_string(root.join("turn.txt")).unwrap_or_default().contains("secret words"), "nothing sent");
    }

    #[cfg(unix)]
    #[test]
    fn a_wrapper_waiting_on_a_sign_in_ends_within_its_limit_naming_it() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-hang");
        let w = fake_wrapper(&root, true, true);
        let t = Instant::now();
        let e = relay_with(&w, minutes_like(&root), Some("x".into()), Box::new(std::io::empty()), &mut vec![], 1).unwrap_err();
        assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
        assert!(e.contains("acme-claude") && e.contains("within 1 s"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_wrapper_is_added_only_when_it_answers_as_claude_code_from_outside_echos_folders() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-admit");
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let script = |dir: &Path, name: &str, answer: &str| {
            let f = dir.join(name);
            fs::write(&f, format!("#!/bin/sh\necho '{answer}'\n")).unwrap();
            fs::set_permissions(&f, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
            f
        };
        let good = script(&bin, "acme-claude", "2.1.400 (Claude Code)");
        let link = bin.join("acme-link");
        std::os::unix::fs::symlink(&good, &link).unwrap();
        let (path, v) = admit(link.to_str().unwrap()).unwrap();
        assert_eq!((path, v.as_str()), (dunce::canonicalize(&good).unwrap(), "2.1.400 (Claude Code)"), "links followed");
        let other = script(&bin, "node-ish", "v22.1.0");
        assert!(admit(other.to_str().unwrap()).unwrap_err().to_string().contains("as Claude Code"));
        let bare = script(&bin, "passes-on", "2.1.325");
        assert_eq!(admit(bare.to_str().unwrap()).unwrap().1, "2.1.325", "a bare version, a wrapper passing on Claude Code's");
        let mine = paths::home().join("plugins/acme/bin");
        fs::create_dir_all(&mine).unwrap();
        let planted = script(&mine, "acme-claude", "2.1.400 (Claude Code)");
        assert!(admit(planted.to_str().unwrap()).unwrap_err().to_string().contains("inside"));
        std::os::unix::fs::symlink(&planted, bin.join("via-link")).unwrap();
        assert!(admit(bin.join("via-link").to_str().unwrap()).unwrap_err().to_string().contains("inside"), "a link into Echo's folders too");
        assert!(admit("acme claude --x").is_err());
        assert!(admit("relative/acme-claude").is_err());
    }

    #[test]
    fn a_terminal_session_on_a_wrapper_marks_itself_from_its_own_settings_hook() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-terminal");
        let f = root.join("session.json");
        fs::write(&f, r#"{"hooks": {"PreToolUse": [{"matcher": "*", "hooks": [{"type": "command", "command": "guard"}]}]}}"#).unwrap();
        let mut a = args(&["--settings", f.to_str().unwrap(), "--tools", "Bash"]);
        let v = terminal_proof(&mut a).unwrap();
        assert_ne!(a[1], f.to_str().unwrap());
        let s: Value = serde_json::from_slice(&fs::read(&a[1]).unwrap()).unwrap();
        assert!(s["hooks"]["SessionStart"][0]["hooks"][0]["command"].as_str().unwrap().ends_with(&format!("proof hello {v} --mark")));
        assert_eq!(s["hooks"]["PreToolUse"][0]["hooks"][0]["command"], "guard");
        let mut bare = args(&["--tools", "Bash"]);
        terminal_proof(&mut bare).unwrap();
        assert_eq!(bare[0], "--settings", "one is added when the session has none");
        assert!(!marked(&v));
        assert_eq!(proof_command(&args(&["hello", &v, "--mark"])), 0);
        assert!(marked(&v), "its hook's answer is the check");
        assert_eq!(proof_command(&args(&["hello", "../../x", "--mark"])), 2, "a malformed value marks nothing");
    }

    #[test]
    fn each_line_of_a_wrapper_run_moves_the_proof_on_only_in_order() {
        let v = "0123456789abcdef0123456789abcdef";
        let exp = Expected::of(&args(&["-p", "--tools", "Read", "--strict-mcp-config", "--permission-mode", "dontAsk"]));
        let hello = json!({"type": "system", "subtype": "hook_response", "output": format!("{HELLO}{v}")});
        let good = init("2.1.400", "dontAsk", &["Read"], &[]);
        assert_eq!(wrapper_ready(&hello, v, &exp, false, false), Ready::Hello);
        assert_eq!(wrapper_ready(&hello, v, &exp, true, false), Ready::Pass, "once");
        assert!(matches!(wrapper_ready(&good, v, &exp, false, false), Ready::Stop(w) if w.contains("before Echo's checks")));
        assert_eq!(wrapper_ready(&good, v, &exp, true, false), Ready::Checked);
        assert_eq!(wrapper_ready(&good, v, &exp, true, true), Ready::Pass, "checked once");
        assert!(matches!(wrapper_ready(&init("2.1.400", "default", &["Read"], &[]), v, &exp, true, false), Ready::Stop(_)));
        assert_eq!(wrapper_ready(&json!({"type": "assistant"}), v, &exp, true, true), Ready::Pass);
    }

    #[test]
    fn the_marks_outlive_the_plugin_layouts_prune() {
        let (_env, _root) = crate::paths::testenv::folders("wrapper-prune");
        let v = new_value().unwrap();
        fs::create_dir_all(proof_dir()).unwrap();
        fs::write(mark(&v), b"").unwrap();
        crate::plugins::agent::sync_all();
        assert!(marked(&v), "a run's mark is kept while it runs");
        unmark(&v);
        assert!(!marked(&v));
    }

    #[test]
    fn the_proofs_files_go_a_day_on_but_a_marks_never_goes_by_age() {
        let (_env, _root) = crate::paths::testenv::folders("wrapper-sweep");
        let dir = paths::config_dir().join("minutes-run");
        fs::create_dir_all(&dir).unwrap();
        let day_ago = std::time::SystemTime::now() - Duration::from_secs(2 * 86_400);
        let old = dir.join("proof-0a0b0c.json");
        fs::write(&old, "{}").unwrap();
        fs::File::options().write(true).open(&old).unwrap().set_modified(day_ago).unwrap();
        let fresh = dir.join("proof-0d0e0f.json");
        fs::write(&fresh, "{}").unwrap();
        let other = dir.join("session-x.json");
        fs::write(&other, "{}").unwrap();
        fs::File::options().write(true).open(&other).unwrap().set_modified(day_ago).unwrap();
        let v = "0123456789abcdef0123456789abcdef";
        fs::create_dir_all(proof_dir()).unwrap();
        fs::write(mark(v), b"").unwrap();
        fs::File::options().write(true).open(mark(v)).unwrap().set_modified(day_ago).unwrap();
        new_value().unwrap();
        assert!(!old.exists() && fresh.exists() && other.exists());
        assert!(marked(v), "a terminal open for days keeps its mark");
    }

    #[test]
    fn a_prompt_after_dash_p_is_never_taken_for_a_flag() {
        let (_, prompt) = relay_args(Path::new("/w"), &args(&["-p", "--help me with the Acme call", "--verbose"]));
        assert_eq!(prompt.as_deref(), Some("--help me with the Acme call"));
        let (_, prompt) = relay_args(Path::new("/w"), &args(&["-p", "--output-format", "json"]));
        assert_eq!(prompt, None);
    }

    #[test]
    fn a_run_through_a_wrapper_needs_json_output() {
        let (_env, _root) = crate::paths::testenv::folders("wrapper-text");
        let exp = Expected::of(&args(&["-p"]));
        assert!(prepare(args(&["-p", "--output-format", "text"]), Path::new("/e"), "0123456789abcdef0123456789abcdef", &exp).unwrap_err().to_string().contains("JSON output"));
        let (a, made) = prepare(args(&["-p", "--output-format", "json", "--system-prompt", "Echo's words"]), Path::new("/e"), "0123456789abcdef0123456789abcdef", &exp).unwrap();
        assert!(a.windows(2).any(|w| w[0] == "--output-format" && w[1] == "stream-json"));
        assert!(made.iter().all(|f| !f.display().to_string().contains("0123456789abcdef0123456789abcdef")), "files never named by the run's value: {made:?}");
        assert!(a.iter().all(|x| !x.contains("Echo's words")));
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn a_path_into_echos_folders_spelt_in_other_case_is_refused() {
        let (_env, _root) = crate::paths::testenv::folders("wrapper-case");
        let p = PathBuf::from(dunce::canonicalize(paths::home()).unwrap().join("plugins/x/acme-claude").to_string_lossy().to_uppercase());
        assert!(outside_echo(&p).is_err());
    }

    #[test]
    fn a_bare_version_is_claude_codes_only_as_the_last_line_and_2_x() {
        assert_eq!(version_line("2.1.325\n").as_deref(), Some("2.1.325"));
        assert_eq!(version_line("Now using node v24.15.0\n\n2.1.325\n").as_deref(), Some("2.1.325"), "after a login shell's own output");
        assert_eq!(version_line("2.1.200\n").as_deref(), Some("2.1.200"), "how new: the caller's");
        assert_eq!(version_line("\n 2.1.400 (Claude Code)\nmore\n").as_deref(), Some("2.1.400 (Claude Code)"));
        for no in ["10.9.2\n", "3.0.0\n", "1.4.0 (Gemini CLI)\n", "v2.1.325\n", "2.1.325\nsomething else\n", "2.1\n", "2.1.325-beta\n", ""] {
            assert_eq!(version_line(no), None, "{no:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_version_is_asked_as_a_run_would_start_the_wrapper() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-version-env");
        let _fake = super::super::fake::install(&root);
        // Answers as Claude Code only with what Echo gives every run it starts (a wrapper finding its runtime there).
        let w = root.join("acme-claude");
        fs::write(&w, "#!/bin/sh\n[ \"$ECHOCTL_HEADLESS\" = 1 ] && echo '2.1.400 (Claude Code)'\n").unwrap();
        fs::set_permissions(&w, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        assert_eq!(claude_version(&w).unwrap(), "2.1.400 (Claude Code)");
    }

    #[cfg(unix)]
    #[test]
    fn a_stopped_run_takes_down_what_its_wrapper_started() {
        let (_env, root) = crate::paths::testenv::folders("wrapper-group");
        let w = root.join("acme-claude");
        // Starts "Claude Code" as a child (not exec), then reports a start Echo refuses.
        fs::write(&w, format!("#!/bin/sh\nsleep 30 &\necho $! > '{}'\ns=\"\"; prev=\"\"; for a in \"$@\"; do [ \"$prev\" = \"--settings\" ] && s=\"$a\"; prev=\"$a\"; done\nv=$(sed -n 's/.*proof hello \\([0-9a-f]*\\).*/\\1/p' \"$s\" | head -1)\nprintf '{{\"type\":\"system\",\"subtype\":\"hook_response\",\"output\":\"echo-proof:%s\"}}\\n' \"$v\"\nIFS= read -r line\necho '{{\"type\":\"system\",\"subtype\":\"init\",\"claude_code_version\":\"2.1.400\",\"permissionMode\":\"bypassPermissions\",\"tools\":[]}}'\nwait\n", root.join("child.pid").display())).unwrap();
        fs::set_permissions(&w, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let e = relay_with(&w, args(&["-p", "--tools", "Bash", "--permission-mode", "dontAsk", "--output-format", "stream-json"]), Some("x".into()), Box::new(std::io::empty()), &mut vec![], 10).unwrap_err();
        assert!(e.contains("bypassPermissions"), "{e}");
        let pid: i32 = fs::read_to_string(root.join("child.pid")).unwrap().trim().parse().unwrap();
        crate::paths::testenv::eventually("its child gone", || unsafe { libc::kill(pid, 0) } != 0);
    }

    #[test]
    fn echo_starting_clears_terminal_marks_but_not_a_running_print_runs() {
        let (_env, _root) = crate::paths::testenv::folders("wrapper-clear");
        let (t, r) = (new_value().unwrap(), new_value().unwrap());
        assert_eq!(proof_command(&args(&["hello", &t, "--mark"])), 0);
        fs::create_dir_all(proof_dir()).unwrap();
        fs::write(mark(&r), b"").unwrap();
        assert!(marked(&t) && marked(&r));
        clear_terminal_marks();
        assert!(!marked(&t), "no terminal outlives the Echo that opened it");
        assert!(marked(&r), "a print run may be another process's, still running");
    }
}

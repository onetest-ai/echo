//! Notes without an open Claude Code session: the app runs Claude Code headless on a call
//! (`claude -p "/echo:process-call <dir>"`), the same skill a session runs. The app still
//! never writes notes itself; it only starts Claude Code when the user asks (`echoctl notes`,
//! the window's Write notes button, the pane's buttons).
//!
//! - The plugin comes with the app (`Echo.app/Contents/Resources/plugin`, the repo's `plugin/`
//!   in a dev build) and is loaded with `--plugin-dir`, so the run doesn't depend on where, or
//!   whether, the user installed the plugin, and its skill matches the app's CLI.
//! - Claude Code runs through the user's login shell, so the environment their own setup needs
//!   (gateways, `ANTHROPIC_*`, Bedrock / Vertex credentials) is there; the model is
//!   `config.notes_model` (`default` = Claude Code's own choice).
//! - Tools are limited to what the skill needs: reading the data folder, writing inside
//!   `calls/`, and the Echo CLI. Transcripts and chat are other people's words, so the run gets
//!   no web, no other shell commands and no files elsewhere.
//! - `<call>/notes-run.json` holds the state (running / done / skipped / failed / needs_login);
//!   `<call>/claude.log` a readable log of what Claude did.
use crate::app::NoWindow;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{calls, config::Config, paths};

pub const RUN: &str = "notes-run.json";
pub const LOG: &str = "claude.log";
/// A run longer than this is stopped (a long call takes a few minutes).
const TIMEOUT: Duration = Duration::from_secs(20 * 60);
/// A session's claim older than this, with no notes, is treated as abandoned.
const CLAIM_STALE: Duration = Duration::from_secs(30 * 60);
/// Set for the Claude Code run (and inherited by the Echo CLI and the mod inside it).
pub const HEADLESS: &str = "ECHOCTL_HEADLESS";

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Run {
    /// running | done | skipped | failed | needs_login
    pub state: String,
    pub model: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
    /// Why it failed or was skipped, in words for the user.
    pub message: Option<String>,
    pub pid: Option<u32>,
    pub session_id: Option<String>,
}

/// Inside a run Echo started (the mod and the CLI stay out of the way there).
pub fn headless() -> bool {
    std::env::var_os(HEADLESS).is_some()
}

pub fn status(dir: &Path) -> Option<Run> {
    let mut r: Run = serde_json::from_slice(&fs::read(dir.join(RUN)).ok()?).ok()?;
    // A run whose process is gone (the app quit, a crash) didn't finish.
    if r.state == "running" && !r.pid.is_some_and(crate::app::process_alive) {
        r.state = "failed".into();
        r.message = Some("The run stopped before it finished (Echo quit or Claude Code crashed).".into());
    }
    Some(r)
}

fn save(dir: &Path, r: &Run) {
    let tmp = dir.join(format!("{RUN}.tmp"));
    if fs::write(&tmp, serde_json::to_vec_pretty(r).unwrap_or_default()).is_ok() {
        let _ = fs::rename(tmp, dir.join(RUN));
    }
}

/// The plugin this app ships: inside the bundle, next to the binary (Windows zip), or the
/// repo's `plugin/` for a dev build; else wherever Claude Code installed it.
pub fn plugin_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok().and_then(|e| dunce::canonicalize(e).ok())?;
    let near = [
        exe.parent().and_then(Path::parent).map(|c| c.join("Resources/plugin")), // Echo.app/Contents/MacOS/echoctl
        exe.parent().map(|d| d.join("plugin")),                                  // Windows: next to echoctl.exe
        exe.ancestors().nth(3).map(|r| r.join("plugin")),                        // repo/target/release/echoctl
    ];
    near.into_iter()
        .flatten()
        .find(|p| p.join(".claude-plugin/plugin.json").is_file())
        .or_else(crate::app::plugin_root)
}

/// What the window shows next to the Write notes button.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn readiness() -> Value {
    let cfg = Config::load().unwrap_or_default();
    serde_json::json!({
        "claude": crate::app::claude_bin().is_some(),
        "plugin": plugin_dir().is_some(),
        "model": cfg.notes_model_label(),
    })
}

/// `echoctl notes <call>`: in the running app (it outlives this command), else right here.
pub fn request(call: &str, wait: bool) -> Result<()> {
    let dir = calls::resolve_call(call)?;
    check(&dir)?;
    if !wait && crate::watch::running_watcher().is_some() {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        fs::write(paths::state_dir().join(format!("notes-{name}.request")), &name)?;
        println!("Echo is writing the notes with Claude Code. Log: {}", dir.join(LOG).display());
        return Ok(());
    }
    let dir = run(&dir)?;
    let r = status(&dir).unwrap_or_default();
    match r.state.as_str() {
        "done" => println!("Notes written: {}", dir.join("notes.md").display()),
        "skipped" => println!("No notes: {}", r.message.unwrap_or_default()),
        _ => bail!("{} (log: {})", r.message.unwrap_or_else(|| "failed".into()), dir.join(LOG).display()),
    }
    Ok(())
}

/// Start runs the CLI or the page asked for (`notes-<call>.request`); called by the app.
pub fn handle_requests() {
    let Ok(rd) = fs::read_dir(paths::state_dir()) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(call) = name.strip_prefix("notes-").and_then(|n| n.strip_suffix(".request")) {
            let _ = fs::remove_file(e.path());
            if let Ok(dir) = calls::resolve_call(call) {
                start_in_background(dir);
            }
        }
    }
}

/// Run on a thread of the app; the outcome lands in `notes-run.json` and a notification.
pub fn start_in_background(dir: PathBuf) {
    std::thread::spawn(move || {
        if let Err(e) = check(&dir) {
            crate::app::notify("Couldn't write notes", &format!("{e:#}"));
            return;
        }
        match run(&dir) {
            Ok(dir) => notify_outcome(&dir),
            // Refused before Claude Code started (a session has the call, …): only the reason.
            Err(e) => crate::app::notify("Couldn't write notes", &format!("{e:#}")),
        }
    });
}

fn notify_outcome(dir: &Path) {
    let Some(r) = status(dir) else { return };
    let title = call_title(dir);
    match r.state.as_str() {
        // The watcher says "Notes ready" for calls it transcribed; the rest are said here.
        "done" if !crate::watch::awaiting_notes(dir) => crate::app::notify("Notes ready", &title),
        "needs_login" => crate::app::notify("Claude Code needs you to sign in", &r.message.unwrap_or_default()),
        "failed" => crate::app::notify("Couldn't write notes", &format!("{title}: {}", r.message.unwrap_or_default())),
        _ => {}
    }
}

fn call_title(dir: &Path) -> String {
    let meta: Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    meta["title"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| "your call".into())
}

/// Everything that must hold before Claude Code is started (said in words the user can act on).
fn check(dir: &Path) -> Result<()> {
    if !dir.join("transcript.md").exists() {
        bail!("this call has no transcript yet");
    }
    if dir.join(crate::transcribe_marker()).exists() || crate::watch::running_watcher().and_then(|w| w.recording).is_some_and(|r| Path::new(&r) == dir) {
        bail!("this call is still being recorded or transcribed");
    }
    if status(dir).is_some_and(|r| r.state == "running") {
        bail!("Claude Code is already writing the notes for this call");
    }
    if crate::app::claude_bin().is_none() {
        bail!("Claude Code is not installed — get it at https://claude.com/claude-code");
    }
    if plugin_dir().is_none() {
        bail!("the Echo plugin wasn't found — menu → Claude Code → Connect Claude Code");
    }
    Ok(())
}

/// Take the call for this run: a session working on it right now wins; an old claim, or one
/// from an earlier run of ours, is taken over.
fn claim(dir: &Path) -> Result<()> {
    let marker = dir.join(calls::CLAIMED);
    if let Ok(m) = fs::metadata(&marker) {
        let fresh = m.modified().ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age < CLAIM_STALE);
        if fresh && !dir.join(RUN).exists() && !dir.join("notes.md").exists() {
            bail!("a Claude Code session is writing the notes for this call right now");
        }
    }
    fs::write(&marker, format!("{} echo notes\n", Local::now().to_rfc3339()))?;
    Ok(())
}

/// The run itself (blocking). Errors before Claude Code starts are returned; everything after
/// is recorded in `notes-run.json` and the log. Returns the call's folder afterwards (the run
/// may have retitled it).
pub fn run(dir: &Path) -> Result<PathBuf> {
    check(dir)?;
    claim(dir)?;
    let cfg = Config::load().unwrap_or_default();
    let model = cfg.notes_model_arg();
    let home = paths::home();
    let cwd = cfg.notes_in.clone().filter(|d| d.is_dir()).unwrap_or_else(|| home.clone());
    let claude = crate::app::claude_bin().ok_or_else(|| anyhow!("Claude Code is not installed"))?;
    let plugin = plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;

    let mut log = fs::File::create(dir.join(LOG)).context("creating claude.log")?;
    let mut r = Run { state: "running".into(), model: Some(cfg.notes_model_label()), started_at: Some(Local::now()), ..Default::default() };
    let started = Instant::now();
    let _ = writeln!(log, "{}  Echo starts Claude Code to write notes", stamp());
    let _ = writeln!(log, "          call:   {}", dir.display());
    let _ = writeln!(log, "          model:  {}", cfg.notes_model_label());
    let _ = writeln!(log, "          folder: {}", cwd.display());
    let _ = writeln!(log, "          plugin: {}", plugin.display());

    let mut child = match command(&claude, &args(dir, &home, &plugin, model.as_deref()), &cwd) {
        Ok(c) => c,
        Err(e) => {
            r.state = "failed".into();
            r.message = Some(format!("couldn't start Claude Code: {e:#}"));
            let _ = writeln!(log, "{}  {}", stamp(), r.message.as_deref().unwrap_or(""));
            r.finished_at = Some(Local::now());
            save(dir, &r);
            let _ = fs::remove_file(dir.join(calls::CLAIMED));
            return Ok(dir.to_path_buf());
        }
    };
    r.pid = Some(child.id());
    save(dir, &r);

    // stderr: kept whole in the log (sign-in errors and shell noise land here).
    let stderr = child.stderr.take().map(|e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                s.push_str(&line);
                s.push('\n');
            }
            s
        })
    });
    // The watchdog: stop a run that hangs.
    let pid = child.id();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let done = done.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            while t.elapsed() < TIMEOUT {
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            kill(pid);
        });
    }

    let mut result: Option<Value> = None;
    let mut seen = String::new(); // non-JSON output, for the sign-in check
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(ev) => {
                    if ev["type"] == "system" && ev["subtype"] == "init" {
                        r.session_id = ev["session_id"].as_str().map(str::to_owned);
                        save(dir, &r);
                    }
                    for l in describe(&ev) {
                        let _ = writeln!(log, "{}  {l}", stamp());
                    }
                    if ev["type"] == "result" {
                        result = Some(ev);
                    }
                }
                Err(_) if !line.trim().is_empty() => {
                    let _ = writeln!(log, "{}  {line}", stamp());
                    seen.push_str(&line);
                    seen.push('\n');
                }
                Err(_) => {}
            }
        }
    }
    let status_code = child.wait().ok();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    let err_text = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    for l in err_text.lines().filter(|l| !l.trim().is_empty()) {
        let _ = writeln!(log, "{}  stderr: {l}", stamp());
    }

    let result_text = result.as_ref().and_then(|v| v["result"].as_str()).unwrap_or("").to_string();
    let is_error = result.as_ref().is_some_and(|v| v["is_error"].as_bool().unwrap_or(false)) || result.is_none();
    let refused: Vec<String> = result.as_ref()
        .and_then(|v| v["permission_denials"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d["tool_name"].as_str().map(str::to_owned))
        .collect();
    let all = format!("{seen}\n{err_text}\n{result_text}");
    r.finished_at = Some(Local::now());
    if dir.join("notes.md").exists() && !is_error {
        r.state = "done".into();
    } else if needs_login(&all) {
        r.state = "needs_login".into();
        r.message = Some("Claude Code isn't signed in. Open Terminal, run claude, type /login, then click Try again.".into());
    } else if model_rejected(is_error, &all) {
        r.state = "failed".into();
        r.message = Some(format!(
            "Claude Code can't use the model \"{}\". Change it under menu → Claude Code → Notes model (\"default\" uses Claude Code's own setting).",
            cfg.notes_model_label()
        ));
    } else if started.elapsed() >= TIMEOUT {
        r.state = "failed".into();
        r.message = Some(format!("stopped after {} minutes without finishing", TIMEOUT.as_secs() / 60));
    } else if !refused.is_empty() {
        r.state = "failed".into();
        r.message = Some(format!("Claude Code was refused a tool it needed ({}); see the log", refused.join(", ")));
    } else if !is_error {
        // Claude finished but wrote no notes: the call was too short or thin (the skill's gate).
        r.state = "skipped".into();
        r.message = Some(first_line(&result_text).unwrap_or_else(|| "Claude Code wrote no notes for this call".into()));
    } else {
        r.state = "failed".into();
        r.message = Some(
            first_line(&result_text)
                .or_else(|| first_line(&err_text))
                .or_else(|| first_line(&seen))
                .unwrap_or_else(|| format!("Claude Code exited with {}", status_code.and_then(|s| s.code()).map_or("a signal".into(), |c| c.to_string()))),
        );
    }
    let _ = writeln!(log, "{}  {} ({}s)", stamp(), match r.state.as_str() {
        "done" => "Notes written.".to_string(),
        s => format!("{s}: {}", r.message.as_deref().unwrap_or("")),
    }, started.elapsed().as_secs());
    if r.state != "done" {
        // Leave the call to an open session or a retry.
        let _ = fs::remove_file(dir.join(calls::CLAIMED));
    }
    save(dir, &r);
    // A title set during the run: Windows couldn't rename the folder while the log was open.
    drop(log);
    let dir = calls::rename_to_title(dir).unwrap_or_else(|_| dir.to_path_buf());
    // The minutes page (opt-in), under the call's final title.
    if r.state == "done" && cfg.minutes_page {
        let line = match crate::minutes::write(&dir) {
            Ok(page) => format!("Minutes page: {}", page.display()),
            Err(e) => format!("No minutes page: {e:#}"),
        };
        if let Ok(mut log) = fs::OpenOptions::new().append(true).open(dir.join(LOG)) {
            let _ = writeln!(log, "{}  {line}", stamp());
        }
    }
    Ok(dir)
}

fn stamp() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

fn first_line(s: &str) -> Option<String> {
    s.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(300).collect())
}

/// Claude Code's own words when it can't use the account.
fn needs_login(text: &str) -> bool {
    let t = text.to_lowercase();
    ["not logged in", "please run /login", "invalid api key", "oauth token has expired", "authentication_error", "credit balance is too low"]
        .iter()
        .any(|p| t.contains(p))
}

/// Claude Code refused the notes model. Only when the run failed: it warns `unrecognized_model`
/// for any model it doesn't know (a local one through Ollama, a gateway's) even when it works, and
/// a call too short for notes would otherwise be reported as a model problem.
fn model_rejected(is_error: bool, text: &str) -> bool {
    is_error && (text.contains("unrecognized_model") || text.contains("issue with the selected model"))
}

/// A path as Claude Code's permission rules compare it. On Windows that is POSIX-style with a
/// lower-case drive: `C:\Users\me` → `/c/Users/me` (measured: `/C:\…` and `/C:/…` never match,
/// so a run in don't-ask mode was refused every Write). Elsewhere the path as it is.
fn rule_path(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    match s.as_bytes() {
        [d, b':', rest @ ..] if d.is_ascii_alphabetic() => format!("/{}{}", (*d as char).to_ascii_lowercase(), String::from_utf8_lossy(rest)),
        _ => s,
    }
}

/// The arguments for `claude`.
fn args(dir: &Path, home: &Path, plugin: &Path, model: Option<&str>) -> Vec<String> {
    let calls = paths::calls_dir();
    let abs = |p: &Path| format!("/{}", rule_path(p)); // permission rules spell absolute paths `//…`
    // Windows: Claude Code's Bash doesn't see the PATH this run gets, so the plugin's launcher
    // by its full path (forward slashes, as allowed below).
    // Forward slashes: Bash (Git Bash on Windows) drops the backslashes of a `C:\…` path.
    let fwd = |p: &Path| p.display().to_string().replace('\\', "/");
    let launcher = format!("{}/scripts/echoctl", fwd(plugin));
    let cli = if cfg!(windows) {
        format!("`{launcher} …` (the plugin's launcher, by this exact path)")
    } else {
        "plain `echoctl …` (it is on PATH)".to_string()
    };
    let prompt = format!(
        "/echo:process-call {}\n\n\
         This run was started from the Echo app and nobody is watching it: do every step without \
         asking anything. Echo has already claimed this call for this run, so don't claim it again. \
         Run the Echo CLI as {cli}: one command per Bash call, with no \
         variables, `cd`, pipes, redirects or `&&`. Write and edit files with the Write and Edit \
         tools, never through the shell. Skip the follow-up offers at the end; finish with a \
         two-line report.",
        fwd(dir)
    );
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    if let Some(m) = model {
        a.extend(["--model".into(), m.into()]);
    }
    a.extend(["--plugin-dir".into(), plugin.display().to_string()]);
    a.extend(["--add-dir".into(), home.display().to_string()]);
    // Only the tools below, whatever the user's own default mode is (auto / bypass would let a
    // transcript talk the run into anything): anything else is refused without asking.
    a.extend(["--permission-mode".into(), "dontAsk".into()]);
    a.push("--allowedTools".into());
    a.extend([
        format!("Read({}/**)", abs(home)),
        format!("Glob({}/**)", abs(home)),
        format!("Grep({}/**)", abs(home)),
        format!("Edit({}/**)", abs(&calls)),
        format!("Write({}/**)", abs(&calls)),
        format!("Bash({}/scripts/echoctl:*)", plugin.display()),
        format!("Bash({launcher}:*)"),
        "Bash(echoctl:*)".into(),
        "Skill(echo:process-call)".into(),
    ]);
    a
}

/// `claude` with those arguments: through the login shell on macOS / Linux, so the user's
/// environment is there (a GUI app starts with almost none).
fn command(claude: &Path, args: &[String], cwd: &Path) -> Result<std::process::Child> {
    let mut c = if cfg!(unix) && std::env::var_os("ECHOCTL_NOTES_NO_SHELL").is_none() {
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
        let mut c = Command::new(shell);
        // The Echo CLI first on PATH (the profile may reset PATH, so it's added after it ran).
        c.args(["-l", "-i", "-c", "export PATH=\"$ECHOCTL_BIN_DIR:$PATH\"; exec \"$0\" \"$@\""]).arg(claude).args(args);
        c
    } else {
        let mut c = Command::new(claude).no_window();
        c.args(args);
        c
    };
    c.current_dir(cwd)
        .env(HEADLESS, "1")
        .env("ECHOCTL_HOME", paths::home())
        .stdin(Stdio::null())
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
    Ok(c.spawn()?)
}

fn kill(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    #[cfg(windows)]
    let _ = Command::new("taskkill").no_window().args(["/PID", &pid.to_string(), "/T", "/F"]).status();
}

/// One stream-json event as log lines a person can follow.
fn describe(ev: &Value) -> Vec<String> {
    let clip = |s: &str, n: usize| -> String {
        let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if one.chars().count() > n { format!("{}…", one.chars().take(n).collect::<String>()) } else { one }
    };
    match ev["type"].as_str() {
        Some("system") if ev["subtype"] == "init" => vec![format!(
            "Claude Code started · model {} · session {}",
            ev["model"].as_str().unwrap_or("?"),
            ev["session_id"].as_str().unwrap_or("?")
        )],
        Some("assistant") => ev["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| match c["type"].as_str() {
                Some("text") => c["text"].as_str().filter(|t| !t.trim().is_empty()).map(|t| format!("Claude: {}", clip(t, 400))),
                Some("tool_use") => {
                    let i = &c["input"];
                    let what = i["command"].as_str().or(i["file_path"].as_str()).or(i["skill"].as_str()).or(i["pattern"].as_str()).unwrap_or("");
                    Some(format!("→ {} {}", c["name"].as_str().unwrap_or("tool"), clip(what, 200)))
                }
                _ => None,
            })
            .collect(),
        Some("user") => ev["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "tool_result" && c["is_error"].as_bool() == Some(true))
            .map(|c| {
                let text = c["content"].as_str().map(str::to_owned).unwrap_or_else(|| {
                    c["content"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ")
                });
                format!("  ✗ {}", clip(&text, 300))
            })
            .collect(),
        Some("result") => {
            let mut out = vec![format!(
                "Finished: {} · {:.0}s · ${:.2}",
                if ev["is_error"].as_bool() == Some(true) { "error" } else { "ok" },
                ev["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0,
                ev["total_cost_usd"].as_f64().unwrap_or(0.0)
            )];
            if let Some(t) = ev["result"].as_str().filter(|t| !t.trim().is_empty()) {
                out.push(format!("Result: {}", clip(t, 600)));
            }
            for d in ev["permission_denials"].as_array().into_iter().flatten() {
                out.push(format!("Not allowed: {} {}", d["tool_name"].as_str().unwrap_or("?"), clip(&d["tool_input"].to_string(), 200)));
            }
            out
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_rule_paths_are_posix_with_a_lower_case_drive() {
        assert_eq!(rule_path(Path::new(r"C:\Users\User\Echo\calls")), "/c/Users/User/Echo/calls");
        assert_eq!(rule_path(Path::new("D:/data/echo")), "/d/data/echo");
        assert_eq!(rule_path(Path::new("/Users/me/Echo")), "/Users/me/Echo");
    }

    #[test]
    fn sign_in_errors_are_recognised() {
        assert!(needs_login("Not logged in · Please run /login"));
        assert!(needs_login("API Error: 401 {\"type\":\"error\",\"error\":{\"type\":\"authentication_error\"}}"));
        assert!(!needs_login("Notes written to notes.md"));
    }

    #[test]
    fn an_unknown_model_fails_only_a_failed_run() {
        let warn = "[claude-code:unrecognized_model] {\"model\":\"qwen3.6:35b-a3b\",\"query_source\":\"sdk\"}";
        // A local model that worked, on a call too short for notes: skipped, not a model problem.
        assert!(!model_rejected(false, warn));
        assert!(model_rejected(true, warn));
        assert!(model_rejected(true, "There's an issue with the selected model (x)."));
        assert!(!model_rejected(true, "Not logged in · Please run /login"));
    }

    #[test]
    fn log_lines_from_stream_events() {
        let tool = serde_json::json!({ "type": "assistant", "message": { "content": [
            { "type": "text", "text": "Reading the context." },
            { "type": "tool_use", "name": "Bash", "input": { "command": "echoctl context /x" } } ] } });
        assert_eq!(describe(&tool), vec!["Claude: Reading the context.", "→ Bash echoctl context /x"]);
        let res = serde_json::json!({ "type": "result", "is_error": false, "duration_ms": 61000.0, "total_cost_usd": 0.4, "result": "Done.",
            "permission_denials": [{ "tool_name": "WebFetch", "tool_input": { "url": "https://x" } }] });
        let lines = describe(&res);
        assert_eq!(lines[0], "Finished: ok · 61s · $0.40");
        assert!(lines[2].starts_with("Not allowed: WebFetch"));
    }

    #[test]
    fn model_setting() {
        let mut c = Config::default();
        assert_eq!(c.notes_model_arg().as_deref(), Some("sonnet"));
        c.notes_model = Some("default".into());
        assert_eq!(c.notes_model_arg(), None);
        c.notes_model = Some("claude-opus-5-5".into());
        assert_eq!(c.notes_model_arg().as_deref(), Some("claude-opus-5-5"));
    }
}

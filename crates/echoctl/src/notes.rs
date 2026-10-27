//! Notes without an open Claude Code session: the app runs Claude Code headless on a call
//! (`claude -p "/echo:process-call <dir>"`), the same skill a session runs. The app still
//! never writes notes itself; it starts Claude Code when the user asks (`echoctl notes`, the
//! window's Write notes button, the pane's buttons) or the pane asks after a call (`notes --auto`).
//!
//! - The plugin comes with the app (`Echo.app/Contents/Resources/plugin`, the repo's `plugin/`
//!   in a dev build) and is loaded with `--plugin-dir`, so the run doesn't depend on where, or
//!   whether, the user installed the plugin, and its skill matches the app's CLI.
//! - Claude Code starts through `ai::claude_command` with the call's AI profile: `system` = the
//!   user's login shell and settings (gateways, `ANTHROPIC_*`, Bedrock / Vertex), model
//!   `config.notes_model`; `local` / `claude` = their own config folder and model, the shell's
//!   routing variables replaced.
//! - Tools are limited to what the skill needs: reading the data folder, writing inside
//!   `calls/`, and the Echo CLI. Transcripts and chat are other people's words, so the run gets
//!   no web, no other shell commands and no files elsewhere.
//! - One run per call (`notes-run.lock`); `<call>/notes-run.json` holds the state (queued /
//!   running / done / skipped / failed / needs_login); `<call>/claude.log` a readable log of what
//!   Claude did.
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
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
    /// queued | running | done | skipped | failed | needs_login
    pub state: String,
    /// The AI profile the run uses (`ai.rs`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
    /// Why it failed or was skipped, in words for the user.
    pub message: Option<String>,
    /// The process that owns the run from its place to its final state (the app, or a `notes
    /// --wait` CLI): it gone while `queued` / `running` = interrupted.
    pub pid: Option<u32>,
    /// Claude Code, while it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_pid: Option<u32>,
    pub session_id: Option<String>,
}

/// Inside a run Echo started (the mod and the CLI stay out of the way there).
pub fn headless() -> bool {
    std::env::var_os(HEADLESS).is_some()
}

pub fn status(dir: &Path) -> Option<Run> {
    let mut r: Run = serde_json::from_slice(&fs::read(dir.join(RUN)).ok()?).ok()?;
    if !pending(&r) || r.pid.is_some_and(crate::app::process_alive) {
        return Some(r);
    }
    // Its owner is gone (Echo quit or crashed, a CLI was stopped).
    if r.child_pid.is_some_and(crate::app::process_alive) {
        // Claude Code from that run is still going: it stays the call's only writer.
        r.message = Some("Claude Code from a run Echo started before it stopped is still writing; wait for it to end.".into());
        return Some(r);
    }
    r.message = Some(if r.state == "queued" {
        "Echo stopped before the notes were written — click Write notes again.".into()
    } else {
        "The run stopped before it finished (Echo quit or Claude Code crashed).".into()
    });
    r.state = "failed".into();
    // Its claim is freed once nobody holds the call's place, and under that place (a new run
    // takes the place before it touches the claim).
    if let Ok(Some(_place)) = crate::app::FileLock::try_take(&dir.join(PLACE)) {
        release_claim(dir);
    }
    Some(r)
}

/// Waiting for its turn or running.
pub(crate) fn pending(r: &Run) -> bool {
    r.state == "queued" || r.state == "running"
}

/// The call's place: an OS lock on `<call>/notes-run.lock`, held by the run's owner from its start
/// to its final state (released by the OS if that process dies; it moves with a renamed folder).
const PLACE: &str = "notes-run.lock";

/// Admit a run to the call (never waits): its place first, then — under it — the claim, then
/// `queued`. Refused while another run holds the place, while Claude Code from an earlier run whose
/// owner died is still writing, or while a session has the call; a refusal changes nothing.
fn admit(dir: &Path) -> Result<crate::app::FileLock> {
    let place = crate::app::FileLock::try_take(&dir.join(PLACE)).with_context(|| format!("opening {}", dir.join(PLACE).display()))?;
    let Some(place) = place else { bail!("Echo is already writing the notes for this call") };
    if let Some(r) = status(dir).filter(pending) {
        if r.child_pid.is_some_and(crate::app::process_alive) {
            bail!("Claude Code from an earlier run is still writing the notes for this call");
        }
    }
    claim(dir)?;
    save(dir, &Run { state: "queued".into(), pid: Some(std::process::id()), started_at: Some(Local::now()), message: Some("Starting…".into()), ..Default::default() });
    Ok(place)
}

/// Claude Code children of minutes runs in this process, and whether the app is quitting. A child
/// is started and registered under this lock, so once `shutdown` set `stopping` none is started,
/// and every one started before is stopped.
struct Children {
    stopping: bool,
    pids: Vec<u32>,
}
static CHILDREN: std::sync::Mutex<Children> = std::sync::Mutex::new(Children { stopping: false, pids: Vec::new() });

/// Start a minutes run's Claude Code, unless the app is quitting.
fn admit_child(start: impl FnOnce() -> Result<std::process::Child>) -> Result<std::process::Child> {
    let mut c = CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
    if c.stopping {
        bail!("Echo is quitting");
    }
    let child = start()?;
    c.pids.push(child.id());
    Ok(child)
}

fn quitting() -> bool {
    CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).stopping
}

/// The app is quitting: no new minutes run starts Claude Code, and those running are stopped (TERM
/// to each group, KILL after 3 s), so none goes on writing on its own.
pub fn shutdown() {
    let pids: Vec<u32> = {
        let mut c = CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
        c.stopping = true;
        c.pids.clone()
    };
    let handles: Vec<_> = pids.into_iter().map(|p| std::thread::spawn(move || kill(p))).collect();
    for h in handles {
        let _ = h.join();
    }
}

/// Our own claim (an Echo run's), released when the run doesn't finish.
fn release_claim(dir: &Path) {
    let marker = dir.join(calls::CLAIMED);
    if fs::read_to_string(&marker).is_ok_and(|t| t.contains("echo notes")) {
        let _ = fs::remove_file(marker);
    }
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
    // Run here, in this terminal: Ctrl-C stops Claude Code too (it runs in a session of its own),
    // and the run ends as it does when Echo quits — its locks released once Claude Code is gone.
    let _ = ctrlc::set_handler(shutdown);
    let dir = run(&dir)?;
    let r = status(&dir).unwrap_or_default();
    match r.state.as_str() {
        "done" => println!("Notes written: {}", dir.join("notes.md").display()),
        "skipped" => println!("No notes: {}", r.message.unwrap_or_default()),
        _ => bail!("{} (log: {})", r.message.unwrap_or_else(|| "failed".into()), dir.join(LOG).display()),
    }
    Ok(())
}

/// `echoctl notes <call> --auto --for <session>` (the terminal pane's automatic notes). The CLI
/// checks the call is due (as `pending --for` lists them, and never tried before: a run that was
/// skipped or failed is left to Write notes, not retried by every new session), takes the claim
/// atomically (two sessions can't both start it) and hands the run to the app. Prints one JSON
/// line: `{"requested": true|false, "reason": …}`.
pub fn request_auto(call: &str, session: &Path) -> Result<()> {
    let dir = calls::resolve_call(call)?;
    let say = |requested: bool, reason: &str| println!("{}", json!({ "requested": requested, "reason": reason }));
    let why_not = if headless() {
        Some("inside a run Echo started")
    } else if !Config::load()?.notes_allowed(session) {
        Some("this session's folder doesn't write notes (`echoctl config --notes-in`)")
    } else if dir.join(RUN).exists() {
        Some("notes were already tried for this call (Write notes runs them again)")
    } else if !calls::pending_calls().iter().any(|d| d == &dir) {
        Some("not due for automatic notes (too short, too old, claimed or done)")
    } else {
        None
    };
    if let Some(why) = why_not {
        say(false, why);
        return Ok(());
    }
    let marker = dir.join(calls::CLAIMED);
    match fs::OpenOptions::new().write(true).create_new(true).open(&marker) {
        Ok(mut f) => writeln!(f, "{} echo notes", Local::now().to_rfc3339())?,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            say(false, "another session has this call");
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    }
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if crate::watch::running_watcher().is_none() {
        release_claim(&dir);
        bail!("Echo isn't running — start it with `echoctl app start`");
    }
    if let Err(e) = paths::write_atomic(&paths::state_dir().join(format!("notes-{name}.request")), name.as_bytes()) {
        release_claim(&dir);
        return Err(e.into());
    }
    say(true, "Echo is writing the notes");
    Ok(())
}

/// Start runs the CLI or the page asked for (`notes-<call>.request`); called by the app.
pub fn handle_requests() {
    let Ok(rd) = fs::read_dir(paths::state_dir()) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(call) = name.strip_prefix("notes-").and_then(|n| n.strip_suffix(".request")) {
            let _ = fs::remove_file(e.path());
            // By name, else by stamp (retitled since it was asked for).
            match calls::resolve_call(call).ok().or_else(|| calls::by_stamp(&calls::stamp_of(Path::new(call)))) {
                Some(dir) => start_in_background(dir),
                None => eprintln!("notes: dropped the request for {call} — the call can't be found"),
            }
        }
    }
}

/// Say in `notes-run.json` why a run didn't start (the window shows it) and free an automatic
/// claim — under the call's place, and only when no run holds it.
pub fn refuse(dir: &Path, why: &str) {
    let Ok(Some(_place)) = crate::app::FileLock::try_take(&dir.join(PLACE)) else { return };
    if !status(dir).is_some_and(|r| pending(&r)) {
        release_claim(dir);
        save(dir, &Run { state: "failed".into(), message: Some(why.to_string()), finished_at: Some(Local::now()), ..Default::default() });
    }
}

/// Run on a thread of the app; the outcome lands in `notes-run.json` and a notification. A run
/// refused before it starts (no Claude Code, no plugin, …) releases an automatic claim and says
/// why in `notes-run.json` too, unless another run of this call is on (its state stays).
pub fn start_in_background(dir: PathBuf) {
    std::thread::spawn(move || {
        if let Err(e) = check(&dir) {
            refuse(&dir, &format!("{e:#}"));
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
    if status(dir).is_some_and(|r| pending(&r)) {
        bail!("Echo is already writing the notes for this call");
    }
    if crate::app::claude_bin().is_none() {
        bail!("Claude Code is not installed — get it at https://claude.com/claude-code");
    }
    if plugin_dir().is_none() {
        bail!("the Echo plugin wasn't found — menu → Claude Code → Connect Claude Code");
    }
    Ok(())
}

/// Take the call for this run: a session working on it right now (its fresh claim, no notes yet)
/// wins; an old claim, one from an earlier run of ours, or one for notes already written is taken
/// over. A new claim is created exclusively, so a session claiming at the same moment isn't
/// overwritten.
fn claim(dir: &Path) -> Result<()> {
    let marker = dir.join(calls::CLAIMED);
    let mine = format!("{} echo notes\n", Local::now().to_rfc3339());
    match fs::OpenOptions::new().write(true).create_new(true).open(&marker) {
        Ok(mut f) => return Ok(f.write_all(mine.as_bytes())?),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    let fresh = fs::metadata(&marker).ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).is_some_and(|age| age < CLAIM_STALE);
    let ours = fs::read_to_string(&marker).is_ok_and(|t| t.contains("echo notes"));
    if fresh && !ours && !dir.join("notes.md").exists() {
        bail!("a Claude Code session is writing the notes for this call right now");
    }
    fs::write(&marker, mine)?;
    Ok(())
}

/// The run itself (blocking). Refusals before it takes the call's place (another run, a
/// session's claim, no plugin) are returned; from `queued` on, everything is recorded in
/// `notes-run.json` and the log. Returns the call's folder afterwards (the run may have retitled it).
pub fn run(dir: &Path) -> Result<PathBuf> {
    check(dir)?;
    // Held until the run's final state and claim are settled (the run lets go of it only for the
    // folder's final rename, after which nothing fails).
    let mut place = Some(admit(dir)?);
    let stamp = calls::stamp_of(dir);
    let out = run_in_place(dir, &mut place).map_err(|e| {
        // From `queued` on, every failure is recorded: the call never stays "already writing".
        let now = calls::by_stamp(&stamp).unwrap_or_else(|| dir.to_path_buf());
        let mut r = status(&now).unwrap_or_default();
        if pending(&r) && r.pid == Some(std::process::id()) {
            r.state = "failed".into();
            r.message = Some(format!("{e:#}"));
            r.finished_at = Some(Local::now());
            save(&now, &r);
            release_claim(&now);
        }
        e
    });
    drop(place);
    out
}

/// Something is being recorded or transcribed (any call): a local model waits for it.
fn calls_busy() -> bool {
    crate::watch::running_watcher().and_then(|w| w.recording).is_some()
        || calls::all_calls().iter().any(|d| {
            fs::read_to_string(d.join(crate::transcribe_marker()))
                .ok()
                .and_then(|t| t.split_whitespace().next().and_then(|p| p.parse::<u32>().ok()))
                .is_some_and(crate::app::process_alive)
        })
}

/// One look at the local model's turn: the lock, or why not yet.
fn local_model_turn() -> Result<std::result::Result<crate::app::FileLock, &'static str>> {
    if calls_busy() {
        return Ok(Err("waits for the call to end"));
    }
    let path = crate::livenotes::local_lock_path();
    let _ = fs::create_dir_all(paths::config_dir());
    match crate::app::FileLock::try_take(&path).with_context(|| format!("opening {}", path.display()))? {
        Some(lock) => Ok(Ok(lock)),
        None => Ok(Err("waits for the model to finish another job")),
    }
}

/// A gated profile (a local model, or Claude Code as configured, which may be one): wait until no
/// call is recorded or transcribed, then for the local model's lock (held by this run until it
/// ends). The reason is in `notes-run.json` and on stderr (`notes --wait`).
fn wait_for_local_model(stamp: &str, dir: &Path, r: &mut Run, name: &str) -> Result<crate::app::FileLock> {
    loop {
        if quitting() {
            bail!("Echo quit before the notes were written — click Write notes again");
        }
        let why = match local_model_turn()? {
            Ok(lock) => return Ok(lock),
            Err(why) => why,
        };
        let msg = format!("{name} {why}");
        if r.message.as_deref() != Some(msg.as_str()) {
            eprintln!("{msg}");
            r.message = Some(msg);
            // The folder may be retitled while it waits.
            save(&calls::by_stamp(stamp).unwrap_or_else(|| dir.to_path_buf()), r);
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn run_in_place(dir: &Path, place: &mut Option<crate::app::FileLock>) -> Result<PathBuf> {
    let call_stamp = calls::stamp_of(dir);
    let cfg = Config::load().unwrap_or_default();
    let home = paths::home();
    let plugin =plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    // The place is held by this process until the run ends (a dead holder = an interrupted run).
    let mut r = Run { state: "queued".into(), pid: Some(std::process::id()), started_at: Some(Local::now()), ..Default::default() };
    let fail = |r: &mut Run, msg: String| -> Result<PathBuf> {
        r.state = "failed".into();
        r.message = Some(msg);
        r.finished_at = Some(Local::now());
        save(dir, r);
        release_claim(dir);
        Ok(dir.to_path_buf())
    };
    // The call's profile: strict, never another one (`ai.rs`).
    let profile = match crate::ai::for_call(dir) {
        Ok(p) => p,
        Err(e) => return fail(&mut r, format!("{e:#}")),
    };
    let model = profile.notes_model();
    let label = format!("{} · {}", profile.name, model.as_deref().unwrap_or("Claude Code's default model"));
    r.profile = Some(profile.id.clone());
    r.model = Some(label.clone());
    save(dir, &r);
    let _lease = if profile.kind.gated() { Some(wait_for_local_model(&call_stamp, dir, &mut r, &profile.name)?) } else { None };
    // Retitled while it waited: carry on in the folder as it is named now.
    let moved = calls::by_stamp(&call_stamp).filter(|d| d != dir);
    let dir: &Path = moved.as_deref().unwrap_or(dir);
    let fail = |r: &mut Run, msg: String| -> Result<PathBuf> {
        r.state = "failed".into();
        r.message = Some(msg);
        r.finished_at = Some(Local::now());
        save(dir, r);
        release_claim(dir);
        Ok(dir.to_path_buf())
    };
    // `system` runs where it is set up (that folder's `.claude/settings.json` may route it), never
    // elsewhere; the others load no project settings.
    let cwd = if profile.kind == crate::ai::Kind::System {
        match crate::ai::system_folder() {
            Ok(d) => d,
            Err(e) => return fail(&mut r, format!("{e:#}")),
        }
    } else {
        cfg.notes_in.clone().filter(|d| d.is_dir()).unwrap_or_else(|| home.clone())
    };

    let mut log = match fs::File::create(dir.join(LOG)) {
        Ok(f) => f,
        Err(e) => return fail(&mut r, format!("couldn't create claude.log: {e}")),
    };
    r.state = "running".into();
    r.message = None;
    r.started_at = Some(Local::now());
    let started = Instant::now();
    let _ = writeln!(log, "{}  Echo starts Claude Code to write notes", stamp());
    let _ = writeln!(log, "          call:   {}", dir.display());
    let _ = writeln!(log, "          model:  {label}");
    let _ = writeln!(log, "          folder: {}", cwd.display());
    let _ = writeln!(log, "          plugin: {}", plugin.display());

    let mut child = match admit_child(|| crate::ai::claude_command(&profile, &args(dir, &home, &plugin, model.as_deref()), &cwd, false)) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("couldn't start Claude Code: {e:#}");
            let _ = writeln!(log, "{}  {msg}", stamp());
            return fail(&mut r, msg);
        }
    };
    r.child_pid = Some(child.id()); // `pid` stays this process: the owner of the run
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
    CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).pids.retain(|p| *p != pid);
    // The skill may have retitled the call (`echoctl title` renames its folder at once on macOS):
    // finish in the folder as it is named now.
    let renamed = calls::by_stamp(&call_stamp).filter(|d| d != dir);
    let dir: &Path = renamed.as_deref().unwrap_or(dir);
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
    r.finished_at = Some(Local::now());
    let ended = Ended {
        wrote_notes: dir.join("notes.md").exists(),
        is_error,
        result_text: &result_text,
        err_text: &err_text,
        seen: &seen,
        timed_out: started.elapsed() >= TIMEOUT,
        refused: &refused,
        exit: status_code.and_then(|s| s.code()),
    };
    conclude(&mut r, &profile, &ended, &label, &cfg.notes_model_label());
    let _ = writeln!(log, "{}  {} ({}s)", stamp(), match r.state.as_str() {
        "done" => "Notes written.".to_string(),
        s => format!("{s}: {}", r.message.as_deref().unwrap_or("")),
    }, started.elapsed().as_secs());
    if r.state != "done" {
        // Leave the call to an open session or a retry.
        let _ = fs::remove_file(dir.join(calls::CLAIMED));
    }
    drop(_lease); // the local model is free for the next job
    save(dir, &r);
    // A title set during the run: Windows couldn't rename the folder while the log (or the place)
    // was open.
    drop(log);
    drop(place.take());
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

/// How Claude Code left a minutes run.
struct Ended<'a> {
    wrote_notes: bool,
    is_error: bool,
    result_text: &'a str,
    err_text: &'a str,
    /// Its output that isn't stream JSON.
    seen: &'a str,
    timed_out: bool,
    /// Tools it was refused.
    refused: &'a [String],
    /// Its exit code (None: a signal).
    exit: Option<i32>,
}

/// The run's final state and message. A `claude` profile's cached sign-in check is dropped when a
/// failed run says it isn't signed in.
fn conclude(r: &mut Run, profile: &crate::ai::Profile, e: &Ended, label: &str, notes_model_label: &str) {
    use crate::ai::Kind;
    let all = format!("{}\n{}\n{}", e.seen, e.err_text, e.result_text);
    let (state, message) = if e.wrote_notes && !e.is_error {
        ("done", None)
    } else if e.is_error && needs_login(&all) && profile.kind == Kind::Local {
        // Its token is the profile's: /login in a terminal would only sign in the user's own Claude Code.
        ("failed", Some(format!(
            "{} refused the profile's token — set the right one: `echoctl ai remove {id}`, then `echoctl ai add-local {id} … --token …`",
            profile.name,
            id = profile.id
        )))
    } else if e.is_error && needs_login(&all) {
        // Only a failed run: a skipped one's report may quote what was said ("not logged in").
        ("needs_login", Some(if profile.kind == Kind::Claude {
            crate::ai::forget_check(&profile.id);
            format!("{} isn't signed in — menu → AI for calls → Sign in, then click Try again.", profile.name)
        } else {
            "Claude Code isn't signed in. Open Terminal, run claude, type /login, then click Try again.".into()
        }))
    } else if model_rejected(e.is_error, &all) {
        ("failed", Some(if profile.kind == Kind::System {
            format!("Claude Code can't use the model \"{notes_model_label}\". Change it under menu → Claude Code → Notes model (\"default\" uses Claude Code's own setting).")
        } else {
            format!("Claude Code can't use the model of {} ({label}) — `echoctl ai status`", profile.name)
        }))
    } else if e.timed_out {
        ("failed", Some(format!("stopped after {} minutes without finishing", TIMEOUT.as_secs() / 60)))
    } else if !e.refused.is_empty() {
        ("failed", Some(format!("Claude Code was refused a tool it needed ({}); see the log", e.refused.join(", "))))
    } else if !e.is_error {
        // Claude finished but wrote no notes: the call was too short or thin (the skill's gate).
        ("skipped", Some(first_line(e.result_text).unwrap_or_else(|| "Claude Code wrote no notes for this call".into())))
    } else {
        ("failed", Some(
            first_line(e.result_text)
                .or_else(|| first_line(e.err_text))
                .or_else(|| first_line(e.seen))
                .unwrap_or_else(|| format!("Claude Code exited with {}", e.exit.map_or("a signal".into(), |c| c.to_string()))),
        ))
    };
    r.state = state.into();
    r.message = message;
}

/// Claude Code's own words when it can't use the account.
pub(crate) fn needs_login(text: &str) -> bool {
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

/// Stop a run and what it started (its whole group; the run's thread reaps the leader).
fn kill(pid: u32) {
    crate::ai::stop_group(pid);
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

    /// The pid of a process that has just exited (any fixed number could be a live process).
    fn dead_pid() -> u32 {
        let mut c = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" });
        if cfg!(windows) {
            c.args(["/c", "exit"]);
        }
        let mut child = c.spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn automatic_notes_only_where_and_when_they_are_due() {
        let (_env, root) = crate::paths::testenv::folders("notes-auto");
        let dir = root.join("home/calls/2026-10-23_120000-x");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), "{}").unwrap();
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        Config { notes_in: Some(root.join("home")), ..Default::default() }.save().unwrap();
        request_auto(&dir.display().to_string(), &root.join("elsewhere")).unwrap();
        assert!(!dir.join(calls::CLAIMED).exists(), "a session outside notes_in claims nothing");
        save(&dir, &Run { state: "skipped".into(), ..Default::default() });
        request_auto(&dir.display().to_string(), &root.join("home")).unwrap();
        assert!(!dir.join(calls::CLAIMED).exists(), "tried before: left to Write notes");
    }

    #[test]
    fn one_run_per_call_and_interrupted_runs_free_it() {
        let dir = std::env::temp_dir().join(format!("echo-place-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let place = admit(&dir).unwrap();
        assert_eq!(status(&dir).unwrap().state, "queued", "admission says so at once");
        assert!(admit(&dir).is_err(), "one run at a time (even in this process)");
        // While the place is held, an "interrupted" look at the state frees nothing.
        let gone = dead_pid();
        save(&dir, &Run { state: "queued".into(), pid: Some(gone), ..Default::default() });
        fs::write(dir.join(calls::CLAIMED), "2026-10-23T01:00:00+02:00 echo notes\n").unwrap();
        assert_eq!(status(&dir).unwrap().state, "failed");
        assert!(dir.join(calls::CLAIMED).exists(), "the place is held: a new run owns the claim");
        drop(place); // its owner is gone: the OS releases the place
        // A queued run whose owner died: interrupted, and its claim is released.
        assert_eq!(status(&dir).unwrap().state, "failed");
        assert!(!dir.join(calls::CLAIMED).exists());
        // A session's claim is not ours to release.
        save(&dir, &Run { state: "running".into(), pid: Some(gone), ..Default::default() });
        fs::write(dir.join(calls::CLAIMED), "2026-10-23T01:00:00+02:00\n").unwrap();
        assert_eq!(status(&dir).unwrap().state, "failed");
        assert!(dir.join(calls::CLAIMED).exists());
        // Claude Code from a run whose owner died still writes: still the only writer.
        #[cfg(unix)]
        {
            let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
            save(&dir, &Run { state: "running".into(), pid: Some(gone), child_pid: Some(child.id()), ..Default::default() });
            assert_eq!(status(&dir).unwrap().state, "running");
            assert!(admit(&dir).is_err());
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_admission_touches_neither_claim_nor_state() {
        let dir = std::env::temp_dir().join(format!("echo-admit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let other = crate::app::FileLock::try_take(&dir.join(PLACE)).unwrap().unwrap(); // another run holds the place
        fs::write(dir.join(calls::CLAIMED), "2026-10-23T01:00:00+02:00 echo notes\n").unwrap();
        save(&dir, &Run { state: "running".into(), pid: Some(std::process::id()), ..Default::default() });
        assert!(admit(&dir).is_err());
        assert_eq!(fs::read_to_string(dir.join(calls::CLAIMED)).unwrap(), "2026-10-23T01:00:00+02:00 echo notes\n");
        assert_eq!(status(&dir).unwrap().state, "running");
        drop(other);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn quitting_stops_every_child_and_starts_none() {
        let _q = QUITTING.lock().unwrap_or_else(|e| e.into_inner());
        let child = admit_child(|| {
            let mut c = std::process::Command::new("sleep");
            c.arg("30");
            crate::ai::detach(&mut c);
            Ok(c.spawn()?)
        })
        .unwrap();
        let pid = child.id();
        let reaper = std::thread::spawn(move || {
            let mut child = child;
            child.wait().unwrap()
        });
        shutdown();
        assert!(!reaper.join().unwrap().success(), "the running child was stopped");
        let called = std::sync::atomic::AtomicBool::new(false);
        assert!(admit_child(|| {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            bail!("started")
        })
        .is_err());
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst), "none starts after");
        assert!(quitting());
        CHILDREN.lock().unwrap().stopping = false; // other tests in this process
        CHILDREN.lock().unwrap().pids.retain(|p| *p != pid);
    }

    #[test]
    fn a_session_working_on_the_call_keeps_it() {
        let dir = std::env::temp_dir().join(format!("echo-claim-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(calls::CLAIMED), "2026-10-23T01:00:00+02:00\n").unwrap(); // a session's, fresh
        save(&dir, &Run { state: "failed".into(), ..Default::default() }); // an earlier Echo run left its state
        assert!(claim(&dir).is_err(), "Write notes doesn't overwrite a session's fresh claim");
        fs::write(dir.join("notes.md"), "# done").unwrap();
        claim(&dir).unwrap(); // its notes exist: a re-run may take over
        assert!(fs::read_to_string(dir.join(calls::CLAIMED)).unwrap().contains("echo notes"));
        let _ = fs::remove_dir_all(&dir);
    }

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
    fn minutes_on_a_local_model_wait_their_turn() {
        let _q = QUITTING.lock().unwrap_or_else(|e| e.into_inner());
        let (_env, root) = crate::paths::testenv::folders("notes-local-turn");
        let d = root.join("home/calls/2026-10-23_220000-x");
        fs::create_dir_all(&d).unwrap();
        let busy = crate::app::FileLock::try_take(&crate::livenotes::local_lock_path()).unwrap().unwrap();
        // Another job holds the model: the run says so, and goes on once it is free.
        let d2 = d.clone();
        let other = std::thread::spawn(move || {
            let t = Instant::now();
            while !status(&d2).is_some_and(|r| r.message.is_some_and(|m| m.contains("waits for the model"))) {
                assert!(t.elapsed() < Duration::from_secs(20), "the run never said it waits");
                std::thread::sleep(Duration::from_millis(20));
            }
            drop(busy);
        });
        let mut r = Run::default();
        let turn = wait_for_local_model(&calls::stamp_of(&d), &d, &mut r, "Qwen");
        other.join().unwrap();
        assert!(turn.is_ok());
        assert_eq!(r.message.as_deref(), Some("Qwen waits for the model to finish another job"));
    }

    #[test]
    fn a_run_is_judged_by_how_it_ended() {
        use crate::ai::Kind;
        let (_env, _root) = crate::paths::testenv::folders("notes-conclude");
        let profile = |kind| crate::ai::Profile { id: "judged".into(), name: "J".into(), kind, base_url: None, token: None, live_model: None, notes_model: None, account: None };
        let ended = |wrote_notes, is_error, result_text| Ended { wrote_notes, is_error, result_text, err_text: "", seen: "", timed_out: false, refused: &[], exit: Some(1) };
        let judge = |kind, e: &Ended| {
            let mut r = Run::default();
            conclude(&mut r, &profile(kind), e, "J · m", "sonnet");
            (r.state, r.message.unwrap_or_default())
        };
        assert_eq!(judge(Kind::System, &ended(true, false, "")).0, "done");
        // A call skipped as too short, whose report quotes what was said, needs no sign-in.
        let (state, message) = judge(Kind::Claude, &ended(false, false, "Too short for notes: they said they're not logged in."));
        assert_eq!((state.as_str(), message.as_str()), ("skipped", "Too short for notes: they said they're not logged in."));
        // A failed run that says so does.
        assert_eq!(judge(Kind::System, &ended(false, true, "Not logged in · Please run /login")).0, "needs_login");
        let (state, message) = judge(Kind::Claude, &ended(false, true, "Not logged in · Please run /login"));
        assert_eq!(state, "needs_login");
        assert!(message.contains("Sign in"), "{message}");
        // A local server refusing the profile's token: the fix is the token, not /login.
        let (state, message) = judge(Kind::Local, &ended(false, true, "Invalid API key · Please run /login"));
        assert_eq!(state, "failed");
        assert!(message.contains("token") && !message.contains("/login"), "{message}");
        assert_eq!(judge(Kind::System, &ended(false, true, "")).1, "Claude Code exited with 1");
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

    /// Tests that set or depend on `quitting()` (process-wide).
    static QUITTING: std::sync::Mutex<()> = std::sync::Mutex::new(());
}

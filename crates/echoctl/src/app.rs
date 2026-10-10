//! The echoctl app: the tray icon + call watcher as a standalone background app.
//!
//! It does not need Claude Code. It starts at login (`app autostart on`) or on demand
//! (`app start`), records the calls the user opts in to, transcribes them on-device and asks
//! its own questions (native dialogs) when a setting says "ask". Claude Code is one client:
//! the plugin's skills and mod drive it through the same CLI and control files as the tray
//! (`<home>/.echo/*.request`, `armed.json`), read its events, and write the notes.
//!
//! Permissions (macOS) belong to the app bundle (Echo.app), not to a terminal, so
//! `doctor` asks the running app to check them in its own process.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::Utc;
use serde_json::{json, Value};

use crate::apps::TargetApp;
use crate::meeting::MeetingContext;
use crate::{models, paths, permissions, watch};

/// Set before stopping to have the app start again (after the data folder changed).
pub static RESTART: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const BUNDLE_ID: &str = "ai.onetest.echo";

fn state_file(name: &str) -> PathBuf {
    paths::state_dir().join(name)
}

/// The .app bundle this binary runs from, if any (macOS).
pub fn bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // Echo.app/Contents/MacOS/echoctl
    let bundle = exe.parent()?.parent()?.parent()?;
    bundle.extension().is_some_and(|e| e == "app").then(|| bundle.to_path_buf())
}

/// An installed app: the bundle we run from, else /Applications or ~/Applications (macOS);
/// on Windows the binary itself is the app.
pub fn installed_app() -> Option<PathBuf> {
    if let Some(b) = bundle_path() {
        return Some(b);
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        for p in [PathBuf::from("/Applications/Echo.app"), home.join("Applications/Echo.app")] {
            if p.join("Contents/MacOS/echoctl").exists() {
                return Some(p);
            }
        }
    }
    None
}

/// `echoctl app run`: the app itself. Never returns until the user quits it.
pub fn run(app: TargetApp, tray: bool, background: bool) -> Result<()> {
    // One app per user: an exclusive lock held for the process's life (the heartbeat alone
    // leaves a window where a login launch and `app start` both get in).
    if !single_instance()? {
        let pid = watch::running_watcher().map(|w| w.pid.to_string()).unwrap_or_else(|| "?".into());
        bail!("Echo is already running (pid {pid}) — `echoctl app quit` stops it");
    }
    if background {
        detach_output();
    }
    if !paths::home_chosen() {
        first_run();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    ctrlc::set_handler(move || s.store(true, Ordering::SeqCst))?; // SIGINT + SIGTERM
    std::thread::spawn(|| write_health(None, false, false));
    std::thread::spawn(check_updates);
    #[cfg(feature = "tray")]
    if tray {
        crate::tray::run(app, stop); // never returns
    }
    let _ = tray;
    watch::run_loop(app, stop, |_| {})?;
    if RESTART.load(Ordering::SeqCst) {
        let _ = spawn_app(true);
    }
    Ok(())
}

/// Look for a new release at start and every 6 h; notify once per version.
fn check_updates() {
    let mut told: Option<String> = None;
    loop {
        if let Ok(Some(r)) = crate::update::check() {
            if told.as_deref() != Some(r.version.as_str()) {
                notify("Update available", &format!("Echo {} — install it from the menu.", r.version));
                told = Some(r.version);
            }
        }
        std::thread::sleep(Duration::from_secs(6 * 3600));
    }
}

static INSTANCE_LOCK: std::sync::OnceLock<fs::File> = std::sync::OnceLock::new();

/// Set on an app started by a restart: wait for the previous instance's lock.
const WAIT_FOR_LOCK: &str = "ECHOCTL_WAIT_FOR_LOCK";

/// Take the per-user app lock, waiting up to 15 s when restarting.
fn single_instance() -> Result<bool> {
    let patient = std::env::var_os(WAIT_FOR_LOCK).is_some();
    let t0 = Instant::now();
    loop {
        if try_lock()? {
            return Ok(true);
        }
        if !patient || t0.elapsed() > Duration::from_secs(15) {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Take the per-user app lock (`<config dir>/app.lock`); false if another app holds it.
fn try_lock() -> Result<bool> {
    fs::create_dir_all(paths::config_dir())?;
    let path = paths::config_dir().join("app.lock");
    #[cfg(unix)]
    let file = {
        use std::os::fd::AsRawFd;
        let f = fs::OpenOptions::new().create(true).truncate(false).write(true).open(&path)?;
        // SAFETY: flock on a valid, open fd; the lock lives as long as the File (process life).
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Ok(false);
        }
        f
    };
    #[cfg(windows)]
    let file = {
        use std::os::windows::fs::OpenOptionsExt;
        // share_mode(0): no other process can open the file while we hold it.
        match fs::OpenOptions::new().create(true).truncate(false).write(true).share_mode(0).open(&path) {
            Ok(f) => f,
            Err(_) => return Ok(false),
        }
    };
    let _ = INSTANCE_LOCK.set(file);
    Ok(true)
}

/// Background launches have no terminal: send output to `<home>/.echo/app.log` (Unix) or
/// drop the console window (Windows).
fn detach_output() {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let _ = fs::create_dir_all(paths::state_dir());
        let log = state_file("app.log");
        // Keep the log small: start over once it passes 5 MB.
        if fs::metadata(&log).is_ok_and(|m| m.len() > 5_000_000) {
            let _ = fs::remove_file(&log);
        }
        if let Ok(f) = fs::OpenOptions::new().create(true).append(true).open(&log) {
            unsafe {
                libc::dup2(f.as_raw_fd(), 1);
                libc::dup2(f.as_raw_fd(), 2);
            }
        }
    }
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Console::FreeConsole();
    }
}

/// Start the app without waiting for it (the bundle via LaunchServices when installed, so
/// macOS attributes permissions to Echo.app).
/// `restart`: the new instance waits up to 15 s for a previous one's lock instead of giving up
/// (a restart after a data-folder change, or `app start` right after `app quit`); on macOS it
/// is a new instance (`open -n`), since plain `open` would just activate the quitting one.
pub fn spawn_app(restart: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    if let Some(bundle) = installed_app() {
        let mut open = Command::new("open");
        if restart {
            open.args(["-n", "--env", &format!("{WAIT_FOR_LOCK}=1")]);
        }
        open.arg("-g").arg(&bundle).status()?;
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.args(["app", "run", "--background"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    if restart {
        cmd.env(WAIT_FOR_LOCK, "1");
    }
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn()?;
    Ok(())
}

/// Show a folder in Finder / Explorer.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn reveal(dir: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(dir).status();
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(dir).status();
}

/// Is a process with this pid alive?
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    return unsafe { libc::kill(pid as i32, 0) } == 0;
    #[cfg(windows)]
    return Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()));
    #[allow(unreachable_code)]
    true
}

/// `echoctl app start`: launch the app if it isn't running; waits until it heartbeats.
pub fn start() -> Result<()> {
    if let Some(w) = watch::running_watcher() {
        println!("Echo is running (pid {})", w.pid);
        return Ok(());
    }
    // Patient: a just-quit instance may still be releasing the lock.
    spawn_app(true)?;
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(if paths::home_chosen() { 8 } else { 120 }) {
        if let Some(w) = watch::running_watcher() {
            println!("Echo started (pid {})", w.pid);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("Echo did not start — see {}", state_file("app.log").display())
}

/// `echoctl app quit`: stop the app (a recording in progress is saved first).
pub fn quit() -> Result<()> {
    let Some(w) = watch::running_watcher() else {
        println!("Echo is not running");
        return Ok(());
    };
    fs::write(state_file("quit.request"), b"")?;
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(30) {
        // Gone means the process has exited too (it holds the single-instance lock until then).
        if watch::running_watcher().is_none() && !process_alive(w.pid) {
            println!("Echo stopped (was pid {})", w.pid);
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("Echo (pid {}) did not stop within 30 s", w.pid)
}

/// `echoctl app open [--call <name>]`: a request for the tray loop to open the Echo window.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
static OPEN_WINDOW: std::sync::Mutex<Option<Option<String>>> = std::sync::Mutex::new(None);

/// Taken by the tray loop on each tick.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn take_open_request() -> Option<Option<String>> {
    OPEN_WINDOW.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// `echoctl app open`: ask the running app to show its window (at a call).
pub fn open_window(call: Option<String>) -> Result<()> {
    if watch::running_watcher().is_none() {
        start()?;
    }
    fs::write(state_file("open.request"), call.unwrap_or_default())?;
    println!("opening the Echo window");
    Ok(())
}

/// Called every second by the watcher loop: control requests from the CLI / Claude.
pub fn handle_requests(stop: &AtomicBool) {
    if let Ok(call) = fs::read_to_string(state_file("open.request")) {
        let _ = fs::remove_file(state_file("open.request"));
        let call = call.trim().to_string();
        *OPEN_WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = Some((!call.is_empty()).then_some(call));
    }
    if fs::remove_file(state_file("quit.request")).is_ok() {
        stop.store(true, Ordering::SeqCst);
    }
    if let Ok(bytes) = fs::read(state_file("doctor.request")) {
        let _ = fs::remove_file(state_file("doctor.request"));
        let req: Value = serde_json::from_slice(&bytes).unwrap_or_default();
        let id = req["id"].as_str().map(str::to_owned);
        let (request, probe) = (req["request"].as_bool().unwrap_or(false), req["probe"].as_bool().unwrap_or(false));
        std::thread::spawn(move || write_health(id, request, probe));
    }
}

/// Trigger the permission prompts for the app (tray: Health → Ask for permissions).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn request_permissions() {
    std::thread::spawn(|| write_health(None, true, false));
}

/// Check permissions in this (the app's) process and write `health.json`.
fn write_health(id: Option<String>, request: bool, probe: bool) {
    let report = permissions::collect(request, probe);
    let out = match report {
        Ok(r) => json!({ "id": id, "at": Utc::now(), "report": r }),
        Err(e) => json!({ "id": id, "at": Utc::now(), "error": format!("{e:#}") }),
    };
    let tmp = state_file("health.json.tmp");
    if fs::write(&tmp, serde_json::to_vec_pretty(&out).unwrap_or_default()).is_ok() {
        let _ = fs::rename(tmp, state_file("health.json"));
    }
}

/// Ask the running app for its permission report (JSON), or None if no app is running.
pub fn app_health(request: bool, probe: bool) -> Result<Option<Value>> {
    let Some(w) = watch::running_watcher() else { return Ok(None) };
    if w.pid == std::process::id() {
        return Ok(None);
    }
    let id = format!("{}-{}", std::process::id(), Utc::now().timestamp_millis());
    fs::write(state_file("doctor.request"), serde_json::to_vec(&json!({ "id": id, "request": request, "probe": probe }))?)?;
    // Answering the macOS permission dialogs can take a while.
    let limit = Duration::from_secs(if request { 180 } else { 15 });
    let t = Instant::now();
    while t.elapsed() < limit {
        if let Ok(b) = fs::read(state_file("health.json")) {
            if let Ok(v) = serde_json::from_slice::<Value>(&b) {
                if v["id"].as_str() == Some(id.as_str()) {
                    if let Some(e) = v["error"].as_str() {
                        bail!("{e}");
                    }
                    return Ok(Some(v["report"].clone()));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    bail!("the app did not answer the permission check within {} s", limit.as_secs())
}

// ── first run ──────────────────────────────────────────────────────────────────────────────

/// First launch: pick the data folder, offer the model download, trigger permission prompts.
fn first_run() {
    let default = paths::default_home();
    let shown = tilde(&default);
    let choice = ask(
        &format!("Welcome to Echo.\n\nWhere should call transcripts and notes be kept?\n\nDefault: {shown}"),
        &["Choose folder…", "Use default"],
        "Use default",
    );
    let dir = match choice.as_deref() {
        Some("Choose folder…") => choose_folder("Folder for Echo calls and notes").unwrap_or(default),
        _ => default,
    };
    match paths::set_home(&dir) {
        Ok(d) => eprintln!("data folder: {}", d.display()),
        Err(e) => eprintln!("could not use {}: {e:#}", dir.display()),
    }

    let cfg = crate::config::Config::load().unwrap_or_default();
    if cfg.model.is_none() && !models::CATALOG.iter().any(|m| models::model_path(m).exists()) {
        let (rec, _) = models::recommend(&models::hardware());
        let size = models::find(rec).map(|m| m.size_mb).unwrap_or(0);
        let yes = ask(
            &format!("Echo transcribes on this computer. Download the speech model now?\n\n{rec} · {size} MB"),
            &["Later", "Download"],
            "Download",
        );
        if yes.as_deref() == Some("Download") {
            download_model_in_background(rec);
        }
    }
    // macOS shows the microphone / system audio / accessibility prompts for the app now,
    // rather than in the middle of the first call.
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| write_health(None, true, false));
}

static DOWNLOADING: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);

/// The model the app is downloading, with its progress in percent.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn downloading() -> Option<(&'static str, u8)> {
    let name = (*DOWNLOADING.lock().unwrap())?;
    Some((name, models::PROGRESS.load(Ordering::Relaxed)))
}

/// Download a model off the UI thread (one at a time); notifies when done.
pub fn download_model_in_background(name: &'static str) {
    {
        let mut d = DOWNLOADING.lock().unwrap();
        if d.is_some() {
            return;
        }
        *d = Some(name);
    }
    models::PROGRESS.store(0, Ordering::Relaxed);
    std::thread::spawn(move || {
        let r = models::download(name);
        *DOWNLOADING.lock().unwrap() = None;
        match r {
            Ok(()) => notify("Speech model ready", &format!("{name} is downloaded.")),
            Err(e) => notify("Model download failed", &format!("{e:#}")),
        }
    });
}

// ── offers (config "ask") ──────────────────────────────────────────────────────────────────

static ASKING: AtomicBool = AtomicBool::new(false);

/// A calendar meeting is about to start and `scheduled` is "ask": offer to record it.
pub fn offer_meeting(m: MeetingContext) {
    if ASKING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let subject = m.subject.clone().unwrap_or_else(|| "A meeting".into());
        let buttons: &[&str] = if m.join_url.is_some() { &["Not now", "Record", "Join & record"] } else { &["Not now", "Record"] };
        let last = crate::calls::recap(&subject);
        let recap = if last.is_empty() { String::new() } else { format!("\n\nLast time:\n• {}", last.join("\n• ")) };
        let answer = ask(&format!("{subject} is starting.{recap}\n\nRecord and transcribe it?"), buttons, "Record");
        match answer.as_deref() {
            Some("Join & record") => drop(watch::join(m)),
            Some("Record") => drop(watch::arm(m.subject.clone(), Some(m), None)),
            _ => {}
        }
        ASKING.store(false, Ordering::SeqCst);
    });
}

/// A call started that isn't on the calendar and `unscheduled` is "ask".
pub fn offer_call(app: TargetApp) {
    if ASKING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let answer = ask(&format!("A {app:?} call started.\n\nRecord and transcribe it?"), &["Not now", "Record"], "Record");
        if answer.as_deref() == Some("Record") {
            let _ = watch::arm(None, crate::calendar::meeting_now(Utc::now()), None);
        }
        ASKING.store(false, Ordering::SeqCst);
    });
}

// ── autostart ──────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn launch_agent() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(format!("Library/LaunchAgents/{BUNDLE_ID}.plist"))
}

pub fn autostart_enabled() -> bool {
    #[cfg(target_os = "macos")]
    return launch_agent().exists();
    #[cfg(windows)]
    return Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run", "/v", "Echo"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    #[allow(unreachable_code)]
    false
}

/// `echoctl app autostart [on|off]`: start the app at login.
pub fn set_autostart(on: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist = launch_agent();
        if !on {
            let _ = fs::remove_file(&plist);
            return Ok(());
        }
        let args: Vec<String> = match installed_app() {
            Some(b) => vec!["/usr/bin/open".into(), "-g".into(), b.display().to_string()],
            None => vec![std::env::current_exe()?.display().to_string(), "app".into(), "run".into(), "--background".into()],
        };
        let items: String = args.iter().map(|a| format!("    <string>{}</string>\n", xml_escape(a))).collect();
        let body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
             <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
             <plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{BUNDLE_ID}</string>\n\
             \x20 <key>ProgramArguments</key>\n  <array>\n{items}  </array>\n  <key>RunAtLoad</key>\n  <true/>\n\
             \x20 <key>ProcessType</key>\n  <string>Interactive</string>\n</dict>\n</plist>\n"
        );
        fs::create_dir_all(plist.parent().unwrap())?;
        fs::write(&plist, body)?;
        Ok(())
    }
    #[cfg(windows)]
    {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let status = if on {
            let exe = std::env::current_exe()?;
            let value = format!("\"{}\" app run --background", exe.display());
            Command::new("reg").args(["add", key, "/v", "Echo", "/t", "REG_SZ", "/d", &value, "/f"]).status()?
        } else {
            Command::new("reg").args(["delete", key, "/v", "Echo", "/f"]).status()?
        };
        if on && !status.success() {
            bail!("could not write the Run registry key");
        }
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = on;
        bail!("autostart is not supported on this platform")
    }
}

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `echoctl app` / `echoctl home`: where things are and whether the app runs (JSON).
pub fn info() -> Value {
    json!({
        "home": paths::home(),
        "home_source": paths::home_source(),
        "calls": paths::calls_dir(),
        "state": paths::state_dir(),
        "config_dir": paths::config_dir(),
        "models": paths::models_dir(),
        "app": installed_app(),
        "running": watch::running_watcher().map(|w| w.pid),
        "autostart": autostart_enabled(),
        "cli_installed": cli_installed(),
        "notes_in": crate::config::Config::load().ok().and_then(|c| c.notes_in),
        "version": env!("CARGO_PKG_VERSION"),
    })
}

// ── programs, Claude Code, command-line tool ───────────────────────────────────────────────

fn user_home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

/// A program by name: PATH first, then the usual install locations — the app started at login
/// gets a minimal PATH without Homebrew's, nvm's or Claude Code's directories.
pub fn find_program(name: &str) -> String {
    let home = user_home();
    let names: Vec<String> = if cfg!(windows) { vec![format!("{name}.exe"), format!("{name}.cmd")] } else { vec![name.to_string()] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend([home.join(".local/bin"), home.join(".claude/local"), home.join(".npm-global/bin")]);
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    if let Some(d) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(d).join("npm"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(pf).join("nodejs"));
    }
    // nvm: newest installed version first
    let mut versions: Vec<PathBuf> = fs::read_dir(home.join(".nvm/versions/node"))
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path().join("bin"))).collect())
        .unwrap_or_default();
    versions.sort();
    dirs.extend(versions.into_iter().rev());
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| names[0].clone())
}

/// Where the plugin's marketplace comes from (`ECHOCTL_MARKETPLACE` overrides, e.g. a local
/// checkout for development).
fn marketplace_source() -> String {
    std::env::var("ECHOCTL_MARKETPLACE").unwrap_or_else(|_| "onetest-ai/echo".into())
}
const MARKETPLACE: &str = "echo";

fn claude_bin() -> Option<PathBuf> {
    let p = PathBuf::from(find_program("claude"));
    p.is_file().then_some(p)
}

/// The Echo plugin's installs, from Claude Code's own records: [(scope, project)].
fn plugin_installs() -> Vec<(String, Option<String>)> {
    let path = user_home().join(".claude/plugins/installed_plugins.json");
    let v: Value = fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let plugins = v.get("plugins").unwrap_or(&v);
    plugins
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k.starts_with("echo@"))
        .flat_map(|(_, list)| list.as_array().cloned().unwrap_or_default())
        .map(|i| (i["scope"].as_str().unwrap_or("?").to_string(), i["projectPath"].as_str().map(str::to_owned)))
        .collect()
}

/// `echoctl claude`: is Claude Code installed, and is the plugin?
pub fn claude_status() -> Value {
    let installs = plugin_installs();
    json!({
        "claude": claude_bin(),
        "plugin_installed": !installs.is_empty(),
        "installs": installs.iter().map(|(s, p)| json!({ "scope": s, "project": p })).collect::<Vec<_>>(),
        "marketplace": marketplace_source(),
    })
}

/// `echoctl claude connect`: add the marketplace and install the plugin at `scope`.
pub fn connect_claude(scope: &str, project: Option<&Path>) -> Result<()> {
    let Some(claude) = claude_bin() else {
        bail!("Claude Code is not installed (no `claude` command found) — see https://claude.com/claude-code");
    };
    let cwd = match project {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| user_home()),
    };
    let run = |args: &[&str]| -> Result<String> {
        let out = Command::new(&claude).args(args).current_dir(&cwd).output()?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        if !out.status.success() {
            bail!("`claude {}` failed: {}", args.join(" "), text.trim());
        }
        Ok(text)
    };
    let known = run(&["plugin", "marketplace", "list"]).unwrap_or_default();
    if !known.lines().any(|l| l.trim().trim_start_matches('❯').trim() == MARKETPLACE) {
        run(&["plugin", "marketplace", "add", &marketplace_source(), "--scope", scope])?;
    }
    run(&["plugin", "install", &format!("echo@{MARKETPLACE}"), "--scope", scope])?;
    println!("Echo plugin installed ({scope} scope{}) — open a new Claude Code session or run /reload-plugins",
        if scope == "user" { String::new() } else { format!(", {}", cwd.display()) });
    Ok(())
}

/// The binary the command-line tool should point at: the installed app's, else this one.
fn app_binary() -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    if let Some(b) = installed_app() {
        return Ok(b.join("Contents/MacOS/echoctl"));
    }
    Ok(std::env::current_exe()?)
}

#[cfg(target_os = "macos")]
const CLI_LINK: &str = "/usr/local/bin/echoctl";

/// Is `echoctl` available in a terminal?
pub fn cli_installed() -> bool {
    #[cfg(target_os = "macos")]
    return fs::symlink_metadata(CLI_LINK).is_ok();
    #[allow(unreachable_code)]
    Path::new(&find_program("echoctl")).is_file()
}

/// `echoctl app install-cli`: put `echoctl` on the PATH. macOS: a /usr/local/bin symlink to
/// the app's binary (macOS asks for an administrator password when that folder is root-owned);
/// Windows: the app's folder is added to the user's PATH.
pub fn install_cli() -> Result<()> {
    let target = app_binary()?;
    #[cfg(target_os = "macos")]
    {
        let direct = fs::create_dir_all("/usr/local/bin").is_ok() && {
            let _ = fs::remove_file(CLI_LINK);
            std::os::unix::fs::symlink(&target, CLI_LINK).is_ok()
        };
        if !direct {
            let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
            let shell = format!("mkdir -p /usr/local/bin && ln -sf {} {}", q(&target.display().to_string()), q(CLI_LINK));
            let script = format!("do shell script {} with prompt \"Echo wants to add its command-line tool to /usr/local/bin.\" with administrator privileges", applescript_str(&shell));
            let ok = Command::new("osascript").args(["-e", &script]).status()?.success();
            if !ok {
                bail!("not installed (cancelled)");
            }
        }
        println!("{CLI_LINK} → {}", target.display());
        Ok(())
    }
    #[cfg(windows)]
    {
        let dir = target.parent().unwrap().display().to_string();
        let script = format!(
            "$p = [Environment]::GetEnvironmentVariable('Path','User'); \
             if (($p -split ';') -notcontains '{d}') {{ [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';') + ';{d}'), 'User') }}",
            d = dir.replace('\'', "''")
        );
        let ok = Command::new("powershell").args(["-NoProfile", "-Command", &script]).status()?.success();
        if !ok {
            bail!("could not update the user PATH");
        }
        println!("added {dir} to your PATH (open a new terminal)");
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = target;
        bail!("not supported on this platform")
    }
}

// ── native dialogs ─────────────────────────────────────────────────────────────────────────

fn tilde(p: &Path) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    match home.and_then(|h| p.strip_prefix(&h).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => p.display().to_string(),
    }
}

#[cfg(target_os = "macos")]
fn applescript_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A dialog with buttons; the clicked label, or None (closed / timed out after 2 min).
pub fn ask(message: &str, buttons: &[&str], default: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let list = buttons.iter().map(|b| applescript_str(b)).collect::<Vec<_>>().join(", ");
        let script = format!(
            "display dialog {} with title \"Echo\" buttons {{{list}}} default button {} giving up after 120",
            applescript_str(message),
            applescript_str(default)
        );
        let out = Command::new("osascript").args(["-e", &script]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("gave up:true") {
            return None;
        }
        return text.split("button returned:").nth(1).map(|r| r.split(',').next().unwrap_or("").trim().to_string());
    }
    #[cfg(windows)]
    {
        // Yes/No popup (yes = the default action), 2-minute timeout.
        let _ = buttons;
        let script = format!(
            "(New-Object -ComObject WScript.Shell).Popup('{}', 120, 'echo', 4 + 32)",
            message.replace('\'', "''").replace('\n', " ")
        );
        let out = Command::new("powershell").args(["-NoProfile", "-Command", &script]).output().ok()?;
        let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (code == "6").then(|| default.to_string());
    }
    #[allow(unreachable_code)]
    {
        let _ = (message, buttons, default);
        None
    }
}

/// A native folder picker.
pub fn choose_folder(prompt: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let script = format!("POSIX path of (choose folder with prompt {} default location (path to home folder))", applescript_str(prompt));
        let out = Command::new("osascript").args(["-e", &script]).output().ok()?;
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (out.status.success() && !p.is_empty()).then(|| PathBuf::from(p));
    }
    #[cfg(windows)]
    {
        let script = format!(
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; \
             $d.Description = '{}'; if ($d.ShowDialog() -eq 'OK') {{ $d.SelectedPath }}",
            prompt.replace('\'', "''")
        );
        let out = Command::new("powershell").args(["-NoProfile", "-STA", "-Command", &script]).output().ok()?;
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (!p.is_empty()).then(|| PathBuf::from(p));
    }
    #[allow(unreachable_code)]
    {
        let _ = prompt;
        None
    }
}

/// A one-line text prompt; the text, or None if cancelled / empty.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn ask_text(message: &str, default: &str, ok: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "text returned of (display dialog {} with title \"Echo\" default answer {} buttons {{\"Cancel\", {}}} default button {})",
            applescript_str(message),
            applescript_str(default),
            applescript_str(ok),
            applescript_str(ok)
        );
        let out = Command::new("osascript").args(["-e", &script]).output().ok()?;
        let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (out.status.success() && !t.is_empty()).then_some(t);
    }
    #[cfg(windows)]
    {
        let script = format!(
            "Add-Type -AssemblyName Microsoft.VisualBasic; [Microsoft.VisualBasic.Interaction]::InputBox('{}', 'echo', '{}')",
            message.replace('\'', "''"),
            default.replace('\'', "''")
        );
        let _ = ok;
        let out = Command::new("powershell").args(["-NoProfile", "-Command", &script]).output().ok()?;
        let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (!t.is_empty()).then_some(t);
    }
    #[allow(unreachable_code)]
    {
        let _ = (message, default, ok);
        None
    }
}

/// A passive notification (no buttons).
pub fn notify(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        let script = format!("display notification {} with title \"Echo\" subtitle {}", applescript_str(body), applescript_str(title));
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (title, body); // TODO(windows): tray balloon / toast
    }
}

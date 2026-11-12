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

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::{json, Value};

use crate::apps::TargetApp;
use crate::meeting::MeetingContext;
use crate::{models, paths, permissions, watch};

mod programs;
pub use programs::*;

/// Set before stopping to have the app start again (after the data folder changed).
pub static RESTART: AtomicBool = AtomicBool::new(false);
/// The app runs without its tray (`app run --no-tray`, a dev or test run): its restart starts it the same way, as does
/// `home set`'s, which reads it from the app it quits (`WatchState::no_tray`, #351).
pub static NO_TRAY: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const BUNDLE_ID: &str = "ai.onetest.echo";

fn state_file(name: &str) -> PathBuf {
    paths::state_dir().join(name)
}

/// The .app bundle this binary runs from, if any (macOS).
pub fn bundle_path() -> Option<PathBuf> {
    let exe = dunce::canonicalize(std::env::current_exe().ok()?).ok()?;
    // Echo.app/Contents/MacOS/echoctl
    let bundle = exe.parent()?.parent()?.parent()?;
    bundle.extension().is_some_and(|e| e == "app").then(|| bundle.to_path_buf())
}

/// A path macOS made up for a quarantined app it runs where it lies (App Translocation): a read-only
/// copy under `…/AppTranslocation/<uuid>/`, gone once the app quits — never a place to point at (#132).
pub fn translocated(p: &Path) -> bool {
    p.components().any(|c| c.as_os_str() == "AppTranslocation")
}

/// An installed app: the bundle we run from (unless translocated), else /Applications or ~/Applications
/// (macOS); on Windows the binary itself is the app.
pub fn installed_app() -> Option<PathBuf> {
    if let Some(b) = bundle_path().filter(|b| !translocated(b)) {
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
    NO_TRAY.store(!tray, Ordering::SeqCst);
    // One app per user: an exclusive lock held for the process's life (the heartbeat alone
    // leaves a window where a login launch and `app start` both get in).
    if !single_instance()? {
        let pid = watch::running_watcher().map(|w| w.pid.to_string()).unwrap_or_else(|| "?".into());
        bail!("Echo is already running (pid {pid}) — `echoctl app quit` stops it");
    }
    if background {
        detach_output();
    }
    record_app_path();
    if !paths::home_chosen() {
        first_run();
    }
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    ctrlc::set_handler(move || s.store(true, Ordering::SeqCst))?; // SIGINT + SIGTERM
    std::thread::spawn(|| write_health(None, false, false));
    std::thread::spawn(check_updates);
    // Live notes during calls (`livenotes.rs`): one worker for the AI jobs.
    crate::livenotes::start();
    // Minutes written by themselves, 15 minutes after a call (`notes::auto_due`).
    crate::notes::start_scheduler();
    crate::projectsync::start_scheduler();
    // Preps for today's meetings of threads (`prep.rs`).
    crate::prep::start_scheduler();
    // Today → Needs you now: Teams and mail of the last day, every 5 minutes (`needs.rs`).
    crate::needs::start_scheduler();
    // Plugins' host halves (`jshost.rs`): their code, inside Echo.
    #[cfg(feature = "js")]
    crate::jshost::sync();
    #[cfg(feature = "brain")]
    crate::brain::start_scheduler();
    // Who each Claude profile is signed in to, for the window (`signin.rs`; in the background).
    crate::signin::refresh_all();
    // Windows: try the GPU now (in a child process), not when the first recording loads a model.
    #[cfg(all(windows, feature = "transcribe"))]
    std::thread::spawn(crate::gpu::allowed);
    // Transcribes for other computers while Settings says so (`remote.rs`).
    #[cfg(feature = "transcribe")]
    std::thread::spawn(crate::remote::run_in_app);
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

/// Where the plugin's launchers find this app wherever it is installed (any folder on Windows):
/// `<config dir>/app-path` holds this executable's path, rewritten at each start.
pub const APP_PATH_FILE: &str = "app-path";

#[cfg(target_os = "macos")]
const MOVE_TO_APPLICATIONS: &str = "macOS runs Echo from a temporary copy: drag Echo.app into Applications in Finder and open it from there, or the command line and Claude Code can't find it.";

/// Nothing may point at a translocated Echo with no copy in Applications (the link, the login item): it is gone
/// once the app quits (#132).
fn refuse_translocated() -> Result<()> {
    #[cfg(target_os = "macos")]
    if bundle_path().is_some_and(|b| translocated(&b)) && installed_app().is_none() {
        bail!("Move Echo to Applications first: {MOVE_TO_APPLICATIONS}");
    }
    Ok(())
}

fn record_app_path() {
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = dunce::canonicalize(&exe).unwrap_or(exe);
    // Translocated: the user is told to move it, every time (the copy running may be newer than one in
    // Applications); meanwhile the copy in Applications, if there is one, else nothing.
    #[cfg(target_os = "macos")]
    let exe = if translocated(&exe) {
        notify("Move Echo to Applications", MOVE_TO_APPLICATIONS);
        match installed_app() {
            Some(b) => b.join("Contents/MacOS/echoctl"),
            None => return,
        }
    } else {
        exe
    };
    let dir = paths::config_dir();
    let _ = fs::create_dir_all(&dir);
    if fs::read_to_string(dir.join(APP_PATH_FILE)).ok().as_deref() != Some(&*exe.to_string_lossy()) {
        let _ = fs::write(dir.join(APP_PATH_FILE), exe.to_string_lossy().as_bytes());
    }    #[cfg(windows)]
    register_link_scheme(&exe);
}

/// Windows: echo:// links (a note in Obsidian, a mail) open this Echo — `HKCU\Software\Classes\echo`, its command
/// `"<echoctl.exe>" app open --link "%1"`, written at each start so it follows the install folder. (macOS: Info.plist.)
#[cfg(windows)]
fn register_link_scheme(exe: &std::path::Path) {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    let set = |key: &str, name: Option<&str>, value: &str| {
        let wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
        let name = name.map(HSTRING::from);
        unsafe {
            let _ = RegSetKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), name.as_ref().map(|n| windows::core::PCWSTR(n.as_ptr())).unwrap_or(windows::core::PCWSTR::null()), REG_SZ.0, Some(wide.as_ptr() as *const _), (wide.len() * 2) as u32);
        }
    };
    set(r"Software\Classes\echo", None, "URL:Echo link");
    set(r"Software\Classes\echo", Some("URL Protocol"), "");
    set(r"Software\Classes\echo\shell\open\command", None, &format!("\"{}\" app open --link \"%1\"", exe.display()));
}

/// Look for a new release at start and every 6 h; notify once per version.
fn check_updates() {
    let mut told: Option<String> = None;
    loop {
        if let Ok(Some(r)) = crate::update::check() {
            if told.as_deref() != Some(r.version.as_str()) {
                notify_click("Update available", &format!("Echo {} — click to install.", r.version), Click::Update);
                told = Some(r.version);
            }
        }
        std::thread::sleep(Duration::from_secs(6 * 3600));
    }
}

static INSTANCE_LOCK: std::sync::OnceLock<FileLock> = std::sync::OnceLock::new();

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
    let Some(lock) = FileLock::try_take(&path).with_context(|| format!("opening {}", path.display()))? else { return Ok(false) };
    let _ = INSTANCE_LOCK.set(lock);
    Ok(true)
}

/// A lock across processes, held until dropped and released when its process exits: `flock` on
/// Unix (a separate open's lock conflicts within this process too), share modes on Windows.
/// Exclusive, or shared (any number of shared holders, never together with an exclusive one).
pub struct FileLock {
    file: fs::File,
    shared: bool,
}

impl FileLock {
    /// The exclusive lock; `Ok(None)` when another holder has it (never waits); an error when the
    /// file can't be opened at all.
    pub fn try_take(path: &Path) -> std::io::Result<Option<FileLock>> {
        Self::take(path, false)
    }

    /// The shared lock (`Ok(None)` while an exclusive holder has it).
    pub fn try_take_shared(path: &Path) -> std::io::Result<Option<FileLock>> {
        Self::take(path, true)
    }

    fn take(path: &Path, shared: bool) -> std::io::Result<Option<FileLock>> {
        #[cfg(unix)]
        let file = {
            use std::os::fd::AsRawFd;
            let f = fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(path)?;
            let mode = if shared { libc::LOCK_SH } else { libc::LOCK_EX };
            // SAFETY: flock on a valid, open fd; the lock lives as long as the File.
            if unsafe { libc::flock(f.as_raw_fd(), mode | libc::LOCK_NB) } != 0 {
                let e = std::io::Error::last_os_error();
                return if e.kind() == std::io::ErrorKind::WouldBlock { Ok(None) } else { Err(e) };
            }
            f
        };
        #[cfg(windows)]
        let file = {
            use std::os::windows::fs::OpenOptionsExt;
            const FILE_SHARE_READ: u32 = 1;
            const FILE_SHARE_WRITE: u32 = 2;
            // Exclusive: read + write access, no sharing — nobody else can open the file. Shared:
            // read access, sharing read only — other shared holders can, an exclusive one can't.
            let mut o = fs::OpenOptions::new();
            if shared {
                o.read(true).share_mode(FILE_SHARE_READ);
            } else {
                o.read(true).write(true).share_mode(0);
            }
            // A shared holder can't create it (no write access): make sure it exists first (an
            // error here shows in the open below).
            if shared && !path.exists() {
                let _ = fs::OpenOptions::new().create(true).truncate(false).write(true).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE).open(path);
            }
            match o.create(!shared).truncate(false).open(path) {
                Ok(f) => f,
                Err(e) if e.raw_os_error() == Some(32) => return Ok(None), // ERROR_SHARING_VIOLATION
                Err(e) => return Err(e),
            }
        };
        Ok(Some(FileLock { file, shared }))
    }

    /// The lock file's text (a holder of either kind may read it).
    pub fn text(&self) -> std::io::Result<String> {
        use std::io::{Read, Seek};
        let mut f = &self.file;
        f.rewind()?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        Ok(s)
    }

    /// Replace the lock file's text — the exclusive holder only (a shared one can't on Windows, so
    /// it can't anywhere).
    pub fn set_text(&self, text: &str) -> std::io::Result<()> {
        use std::io::{Seek, Write};
        if self.shared {
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "a shared lock's text can't be changed"));
        }
        let mut f = &self.file;
        f.set_len(0)?;
        f.rewind()?;
        f.write_all(text.as_bytes())?;
        f.flush()
    }
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

/// Windows: console programs started by the background app (which has no console) would each
/// open a console window; this runs them windowless. In a terminal they keep sharing it.
pub trait NoWindow {
    fn no_window(self) -> Self;
}

impl NoWindow for Command {
    #[allow(unused_mut)]
    fn no_window(mut self) -> Self {
        #[cfg(windows)]
        if unsafe { windows::Win32::System::Console::GetConsoleWindow() }.is_invalid() {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

/// Start the app without waiting for it (the bundle via LaunchServices when installed, so
/// macOS attributes permissions to Echo.app).
/// `restart`: the new instance waits up to 15 s for a previous one's lock instead of giving up
/// (a restart after a data-folder change, or `app start` right after `app quit`); on macOS it
/// is a new instance (`open -n`), since plain `open` would just activate the quitting one.
pub fn spawn_app(restart: bool) -> Result<()> {
    // The bundle only when this binary is an app's: a dev build starts itself, with its own folders (#142).
    #[cfg(target_os = "macos")]
    if let Some(bundle) = bundle_path().and_then(|_| installed_app()) {
        let mut open = Command::new("open");
        if restart {
            open.args(["-n", "--env", &format!("{WAIT_FOR_LOCK}=1")]);
        }
        open.arg("-g").arg(&bundle).status()?;
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.args(run_args(NO_TRAY.load(Ordering::SeqCst))).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
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

/// The arguments a started app runs with: in the background, and without its tray when the app it follows had none.
fn run_args(no_tray: bool) -> Vec<&'static str> {
    let mut a = vec!["app", "run", "--background"];
    if no_tray {
        a.push("--no-tray");
    }
    a
}

/// Test data's switch (`ECHOCTL_TEST_OPEN_LOG_ONLY`, set by `tests/fixtures/make_data.py`'s `<dir>/env`): nothing
/// leaves Echo for the browser or another app — what would open is appended to `ECHOCTL_TEST_OPEN_LOG` (the test data's
/// `<dir>/open.log`, the line its `open` stand-in writes), and `true` says it was kept. Any build: a run on test data
/// never brings up a browser tab, whichever `open` (or `rundll32`) is on PATH or the OS has; unset, nothing changes.
pub fn open_log_only(what: &str) -> bool {
    if std::env::var_os("ECHOCTL_TEST_OPEN_LOG_ONLY").is_none_or(|v| v.is_empty()) {
        return false;
    }
    if let Some(f) = std::env::var_os("ECHOCTL_TEST_OPEN_LOG").filter(|f| !f.is_empty()) {
        use std::io::Write;
        let logged = fs::OpenOptions::new().create(true).append(true).open(&f).and_then(|mut l| writeln!(l, "{what}"));
        if let Err(e) = logged {
            eprintln!("test data: not opened, and not logged ({}): {e}", Path::new(&f).display());
        }
    } else {
        eprintln!("test data: not opened: {what}");
    }
    true
}

/// Show a folder in Finder / Explorer.
pub fn reveal(dir: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(dir).status();
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(dir).status();
}

/// Open a file or folder with its default app (the minutes page: the browser).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn open_path(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).status();
    #[cfg(windows)]
    let _ = Command::new("explorer").arg(path).status();
}

/// Open a folder in VS Code (the skills folder), else with its default app.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn open_in_code(path: &Path) {
    #[cfg(target_os = "macos")]
    {
        if Command::new("open").args(["-b", "com.microsoft.VSCode"]).arg(path).status().is_ok_and(|s| s.success()) {
            return;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        if Command::new("cmd").args(["/C", "code"]).arg(path).creation_flags(NO_WINDOW).status().is_ok_and(|s| s.success()) {
            return;
        }
    }
    open_path(path);
}

/// Show a file selected in Finder / Explorer.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn reveal_file(path: &Path) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg("-R").arg(path).status();
    // Explorer parses its own command line: the path quoted after the comma (one with spaces).
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("explorer").raw_arg(format!("/select,\"{}\"", path.display())).status();
    }
}

/// Is a process with this pid alive?
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    return unsafe { libc::kill(pid as i32, 0) } == 0;
    #[cfg(windows)]
    return Command::new("tasklist").no_window()
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

/// `echoctl app quit`: stop the app (a recording in progress is saved first). Exits 0 once Echo
/// stopped (or wasn't running), 1 when it didn't — notes typed in its window couldn't be saved and
/// the user kept Echo (or didn't answer its question in 2 min; `quit.cancelled` says why), or 30 s
/// passed (while that Echo asks the user, `quit.asking`, it waits on). The Windows installer stops
/// on a 1 for an Echo it installed, unless it is Echo's own update.
pub fn quit() -> Result<()> {
    let Some(w) = watch::running_watcher() else {
        println!("Echo is not running");
        return Ok(());
    };
    let _ = fs::remove_file(state_file("quit.cancelled"));
    fs::write(state_file("quit.request"), b"")?;
    let mut t = Instant::now();
    while t.elapsed() < Duration::from_secs(30) {
        // Gone means the process has exited too (it holds the single-instance lock until then).
        if watch::running_watcher().is_none() && !process_alive(w.pid) {
            println!("Echo stopped (was pid {})", w.pid);
            return Ok(());
        }
        if let Ok(why) = fs::read_to_string(state_file("quit.cancelled")) {
            let _ = fs::remove_file(state_file("quit.cancelled"));
            bail!("Echo didn't quit: {}", if why.trim().is_empty() { "quitting was cancelled" } else { why.trim() });
        }
        // The user is deciding — in this Echo (a file an Echo that died left behind doesn't count).
        if fs::read_to_string(state_file("quit.asking")).is_ok_and(|p| p.trim() == w.pid.to_string()) && process_alive(w.pid) {
            t = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    bail!("Echo (pid {}) did not stop within 30 s", w.pid)
}

/// `echoctl app open [--call <name>]`: a request for the tray loop to open the Echo window.
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

/// The tray's event loop runs (it owns the window, so it does the quitting).
pub static UI_LOOP: AtomicBool = AtomicBool::new(false);
/// `echoctl app quit` asked; taken by the tray's loop.
static QUIT_ASKED: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn take_quit_request() -> bool {
    QUIT_ASKED.swap(false, Ordering::SeqCst)
}

/// `quit.asking` while Echo asks the user about the quit `echoctl app quit` waits for (it keeps
/// waiting). It holds this process's id: one left by an Echo that died doesn't hold a later quit.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn asking_to_quit(on: bool) {
    let f = state_file("quit.asking");
    if on {
        let _ = fs::write(f, std::process::id().to_string());
    } else {
        let _ = fs::remove_file(f);
    }
}

/// The quit `echoctl app quit` waits for didn't happen: why, for it to say.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn quit_cancelled(why: &str) {
    let _ = fs::write(state_file("quit.cancelled"), why);
}

/// Called every second by the watcher loop: control requests from the CLI / Claude.
pub fn handle_requests(stop: &AtomicBool) {
    if let Ok(call) = fs::read_to_string(state_file("open.request")) {
        let _ = fs::remove_file(state_file("open.request"));
        let call = call.trim().to_string();
        *OPEN_WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = Some((!call.is_empty()).then_some(call));
    }
    if fs::remove_file(state_file("quit.request")).is_ok() {
        // With the tray's loop, it quits once the window's notes are saved (`tray::Leaving`).
        if UI_LOOP.load(Ordering::SeqCst) {
            QUIT_ASKED.store(true, Ordering::SeqCst);
        } else {
            stop.store(true, Ordering::SeqCst);
        }
    }
    if let Ok(bytes) = fs::read(state_file("doctor.request")) {
        let _ = fs::remove_file(state_file("doctor.request"));
        let req: Value = serde_json::from_slice(&bytes).unwrap_or_default();
        let id = req["id"].as_str().map(str::to_owned);
        let (request, probe) = (req["request"].as_bool().unwrap_or(false), req["probe"].as_bool().unwrap_or(false));
        std::thread::spawn(move || write_health(id, request, probe));
    }
    crate::notes::handle_requests();
    crate::livenotes::handle_requests();
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
    if cfg.model.is_none() && !models::CATALOG.iter().any(|m| m.role == models::Role::Transcribe && models::model_path(m).exists()) {
        let (rec, _) = models::recommend(&models::hardware());
        let size = models::find(rec).map(|m| m.size_mb).unwrap_or(0);
        let yes = ask(
            &format!("Echo transcribes on this computer. Download the speech model now?\n\n{rec} · {size} MB"),
            &["Later", "Download"],
            "Download",
        );
        if yes.as_deref() == Some("Download") {
            models::download_in_background(rec);
        }
    }
    // Start at login, on by default (docs/install.md) — for an installed app only, not a dev build (#133).
    #[cfg(target_os = "macos")]
    let installed = bundle_path().is_some() && installed_app().is_some();
    #[cfg(not(target_os = "macos"))]
    let installed = crate::update::can_install();
    if installed && !autostart_enabled() {
        if let Err(e) = set_autostart(true) {
            eprintln!("start at login: {e:#}");
        }
    }
    // macOS shows the microphone / system audio / accessibility prompts for the app now,
    // rather than in the middle of the first call.
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| write_health(None, true, false));
}

/// Remove a speech model after the user's yes (what it frees, and which calls it transcribes besides the active one).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn remove_model_asking(name: &'static str) {
    std::thread::spawn(move || {
        let size = models::find(name).map(|m| m.size_mb).unwrap_or(0);
        let model = crate::config::Config::load().ok().and_then(|c| c.model);
        let warning = models::used_for(name, model.as_deref()).map(|r| format!("\n\nNote: it transcribes {r} — without it they go to the active model.")).unwrap_or_default();
        let msg = format!("Remove the speech model {name}?\n\nFrees {size} MB. You can download it again later.{warning}");
        if ask(&msg, &["Cancel", "Remove"], "Remove").as_deref() == Some("Remove") {
            if let Err(e) = models::remove(name) {
                notify("Could not remove the model", &format!("{e:#}"));
            }
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
        let buttons: &[&str] = if m.teams_join_url().is_some() { &["Not now", "Record", "Join & record"] } else { &["Not now", "Record"] };
        let last = crate::calls::recap(&subject);
        let recap = if last.is_empty() { String::new() } else { format!("\n\nLast time:\n• {}", last.join("\n• ")) };
        let answer = ask(&format!("{subject} is starting.{recap}\n\nRecord and transcribe it?"), buttons, "Record");
        match answer.as_deref() {
            Some("Join & record") => drop(watch::join(m)),
            Some("Record") => drop(watch::arm(m.subject.clone(), Some(m), None, None)),
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
            let _ = watch::arm(None, crate::calendar::meeting_now(Utc::now()), None, None);
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
    return Command::new("reg").no_window()
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
        refuse_translocated()?;
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
            Command::new("reg").no_window().args(["add", key, "/v", "Echo", "/t", "REG_SZ", "/d", &value, "/f"]).status()?
        } else {
            Command::new("reg").no_window().args(["delete", key, "/v", "Echo", "/f"]).status()?
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
        "notes_model": crate::config::Config::load().unwrap_or_default().notes_model_label(),
        "live_model": crate::config::Config::load().ok().and_then(|c| c.live_model).unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into()),
        // Live notes during calls: the app writes them, the terminal pane only shows them.
        "live_notes": crate::livenotes::enabled().unwrap_or(false),
        "ai_error": crate::ai::Settings::load().err().map(|e| format!("{e:#}")),
        "version": env!("CARGO_PKG_VERSION"),
    })
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

/// A dialog with buttons; the clicked label, or None (closed / timed out after 2 min). Windows: a
/// Yes/No popup (`yes_no`): No returns None, like closing it. A dialog that can't be shown is
/// logged and counts as None (`ask_checked` tells them apart).
pub fn ask(message: &str, buttons: &[&str], default: &str) -> Option<String> {
    ask_checked(message, buttons, default).unwrap_or_else(|e| {
        eprintln!("dialog: {e:#}");
        None
    })
}

/// `ask`, with Err when the dialog couldn't be shown (or its answer read) — not answered is None.
pub fn ask_checked(message: &str, buttons: &[&str], default: &str) -> Result<Option<String>> {
    debug_assert!(buttons.contains(&default), "the default is one of the buttons");
    #[cfg(target_os = "macos")]
    {
        let list = buttons.iter().map(|b| applescript_str(b)).collect::<Vec<_>>().join(", ");
        let script = format!(
            "display dialog {} with title \"Echo\" buttons {{{list}}} default button {} giving up after 120",
            applescript_str(message),
            applescript_str(default)
        );
        let out = Command::new("osascript").args(["-e", &script]).output().context("couldn't run osascript")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("(-128)") {
                return Ok(None); // a button named Cancel (or Esc): "User canceled"
            }
            bail!("osascript: {}", err.trim());
        }
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("gave up:true") {
            return Ok(None);
        }
        return Ok(text.split("button returned:").nth(1).map(|r| r.split(',').next().unwrap_or("").trim().to_string()));
    }
    #[cfg(windows)]
    {
        // Yes/No popup, 2-minute timeout: 6 = Yes, 7 = No, -1 = no answer.
        // The text goes in through the environment, never quoted: PowerShell also ends a '…' string
        // at a typographic quote (a meeting called "Alex’s sync").
        let (yes, flags) = yes_no(buttons, default);
        let script = format!("(New-Object -ComObject WScript.Shell).Popup($env:ECHO_ASK, 120, 'echo', {flags})");
        let out = Command::new("powershell")
            .no_window()
            .env("ECHO_ASK", message)
            .args(["-NoProfile", "-Command", &script])
            .output()
            .context("couldn't run PowerShell")?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        return match stdout.lines().map(str::trim).rfind(|l| !l.is_empty()) {
            Some("6") => Ok(Some(yes.to_string())),
            Some("7") | Some("-1") => Ok(None),
            _ => bail!("the popup gave no answer: {}", String::from_utf8_lossy(&out.stderr).trim()),
        };
    }
    #[allow(unreachable_code)]
    {
        let _ = (message, buttons, default);
        Ok(None)
    }
}

/// `ask` on Windows: Yes stands for the default button, No for the others — unless the default
/// is the first one (Cancel, Not now): then Yes is the last button (the action) and No is focused.
/// The label Yes returns, and the popup's flags: MB_YESNO + MB_ICONQUESTION + MB_SYSTEMMODAL (on
/// top: a popup behind other windows would time out unseen), + MB_DEFBUTTON2.
#[cfg_attr(not(windows), allow(dead_code))]
fn yes_no<'a>(buttons: &[&'a str], default: &'a str) -> (&'a str, u32) {
    const FLAGS: u32 = 4 + 32 + 4096;
    match buttons {
        [first, .., last] if *first == default => (*last, FLAGS + 256),
        _ => (default, FLAGS),
    }
}

/// An information dialog: the clicked label (macOS: `buttons`, the last one the default;
/// Windows: one OK button, which counts as the first label).
#[cfg_attr(not(all(feature = "tray", feature = "transcribe")), allow(dead_code))]
pub fn message(text: &str, buttons: &[&str]) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let list = buttons.iter().map(|b| applescript_str(b)).collect::<Vec<_>>().join(", ");
        let default = buttons.last().copied().unwrap_or("OK");
        let script = format!("display dialog {} with title \"Echo\" buttons {{{list}}} default button {}", applescript_str(text), applescript_str(default));
        let out = Command::new("osascript").args(["-e", &script]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        return text.split("button returned:").nth(1).map(|r| r.split(',').next().unwrap_or("").trim().to_string());
    }
    #[cfg(windows)]
    {
        // Line breaks survive as PowerShell "`n" between single-quoted pieces.
        let lines = text.split('\n').map(|l| format!("'{}'", l.replace('\'', "''"))).collect::<Vec<_>>().join(" + \"`n\" + ");
        let script = format!("$null = (New-Object -ComObject WScript.Shell).Popup(({lines}), 0, 'Echo', 64)");
        Command::new("powershell").no_window().args(["-NoProfile", "-Command", &script]).status().ok()?;
        return buttons.first().map(|b| b.to_string());
    }
    #[allow(unreachable_code)]
    {
        let _ = (text, buttons);
        None
    }
}

/// A native folder picker.
pub fn choose_folder(prompt: &str) -> Option<PathBuf> {
    // Development builds and tests: the folder given, with no dialog (never in a release build).
    #[cfg(debug_assertions)]
    if let Some(d) = std::env::var_os("ECHOCTL_PICK_FOLDER") {
        let _ = prompt;
        return Some(PathBuf::from(d)).filter(|p| p.is_dir());
    }
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
        let out = Command::new("powershell").no_window().args(["-NoProfile", "-STA", "-Command", &script]).output().ok()?;
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
        let out = Command::new("powershell").no_window().args(["-NoProfile", "-Command", &script]).output().ok()?;
        let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (!t.is_empty()).then_some(t);
    }
    #[allow(unreachable_code)]
    {
        let _ = (message, default, ok);
        None
    }
}

/// What clicking a notification does (while the app runs).
#[derive(Clone, Copy, PartialEq)]
pub enum Click {
    OpenWindow,
    /// The same as the menu's "Update to …": asks, then installs.
    Update,
}

/// Set by a click on an "Update available" notification; taken by the tray loop.
static UPDATE_REQUEST: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn take_update_request() -> bool {
    UPDATE_REQUEST.swap(false, Ordering::SeqCst)
}

/// Ask the tray loop to start the update ("Check for updates" found one).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn request_update() {
    UPDATE_REQUEST.store(true, Ordering::SeqCst);
}

fn clicked(c: Click) {
    match c {
        Click::OpenWindow => *OPEN_WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(None),
        Click::Update => UPDATE_REQUEST.store(true, Ordering::SeqCst),
    }
}

/// A passive notification (no buttons); a click opens the Echo window.
pub fn notify(title: &str, body: &str) {
    notify_click(title, body, Click::OpenWindow)
}

/// A notification whose click does `click`.
pub fn notify_click(title: &str, body: &str, click: Click) {
    if cfg!(test) {
        return; // tests say nothing on the screen
    }
    #[cfg(target_os = "macos")]
    if bundle_path().is_some() {
        notifications::post(title, body, click);
        return;
    }
    #[cfg(target_os = "macos")]
    {
        // A dev build outside Echo.app has no bundle to post as.
        let script = format!("display notification {} with title \"Echo\" subtitle {}", applescript_str(body), applescript_str(title));
        let _ = Command::new("osascript").args(["-e", &script]).status();
    }
    #[cfg(windows)]
    notifications::post(title, body, click);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (title, body, click);
    }
}

/// Windows toasts as "Echo" (an AppUserModelID registered under HKCU with Echo's name and
/// icon, which unpackaged apps need): a click opens the Echo window while the app runs.
#[cfg(windows)]
pub mod notifications {
    use std::sync::{Mutex, OnceLock};

    use windows::core::HSTRING;
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::TypedEventHandler;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

    const AUMID: &str = "onetest.Echo";
    static ICON: &[u8] = include_bytes!("../../../packaging/macos/onetest-logo-1024.png");
    /// Posted toasts keep their click handler only while they are alive.
    static LIVE: Mutex<Vec<ToastNotification>> = Mutex::new(Vec::new());

    fn set(name: &str, value: &str) -> bool {
        let key = HSTRING::from(format!(r"Software\Classes\AppUserModelId\{AUMID}"));
        let wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                &HSTRING::from(name),
                REG_SZ.0,
                Some(wide.as_ptr() as *const _),
                (wide.len() * 2) as u32,
            )
        }
        .is_ok()
    }

    /// Once per process: Echo's icon in the config dir + the AppUserModelID's name and icon.
    fn registered() -> bool {
        static DONE: OnceLock<bool> = OnceLock::new();
        *DONE.get_or_init(|| {
            let icon = crate::paths::config_dir().join("echo.png");
            if std::fs::read(&icon).ok().as_deref() != Some(ICON) {
                let _ = std::fs::create_dir_all(icon.parent().unwrap());
                let _ = std::fs::write(&icon, ICON);
            }
            set("DisplayName", "Echo") && set("IconUri", &icon.display().to_string())
        })
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    pub fn post(title: &str, body: &str, click: super::Click) {
        if let Err(e) = try_post(title, body, click) {
            if std::env::var_os("ECHOCTL_DEBUG").is_some() {
                eprintln!("toast failed: {e}");
            }
        }
    }

    fn try_post(title: &str, body: &str, click: super::Click) -> windows::core::Result<()> {
        if !registered() {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(0x8000_4005u32 as i32)));
        }
        let xml = format!(
            "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
            xml_escape(title),
            xml_escape(body)
        );
        let doc = XmlDocument::new()?;
        doc.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&doc)?;
        toast.Activated(&TypedEventHandler::new(move |_, _| {
            super::clicked(click);
            Ok(())
        }))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(AUMID))?.Show(&toast)?;
        let mut live = LIVE.lock().unwrap_or_else(|e| e.into_inner());
        live.push(toast);
        let n = live.len();
        if n > 20 {
            live.drain(..n - 20);
        }
        Ok(())
    }
}

/// macOS notifications through UserNotifications, posted as Echo.app (bundle builds only):
/// they carry Echo's icon and a click opens the Echo window.
#[cfg(target_os = "macos")]
pub mod notifications {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AllocAnyThread};
    use objc2_foundation::{NSError, NSString, NSUUID};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions,
        UNNotificationRequest, UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    /// Marks a notification whose click starts the update (`Click::Update`).
    const UPDATE_CATEGORY: &str = "update";

    /// Set in the app process (which keeps running); a CLI process waits for its post instead.
    static IN_APP: AtomicBool = AtomicBool::new(false);

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "EchoNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            // Show banners while the Echo window is in front too.
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                handler.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, handler: &DynBlock<dyn Fn()>) {
                let category = response.notification().request().content().categoryIdentifier().to_string();
                super::clicked(if category == UPDATE_CATEGORY { super::Click::Update } else { super::Click::OpenWindow });
                handler.call(());
            }
        }
    );

    /// At app start (inside Echo.app): handle clicks, and ask for permission up front.
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    pub fn init() {
        if super::bundle_path().is_none() {
            return;
        }
        IN_APP.store(true, Ordering::SeqCst);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        std::mem::forget(delegate); // the center holds it weakly; it lives as long as the app
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert,
            &RcBlock::new(|granted: Bool, err: *mut NSError| debug_permission("at start", granted, err)),
        );
    }

    /// `ECHOCTL_DEBUG=1`: macOS's answer (a missing setting reads "not allowed for this application").
    fn debug_permission(when: &str, granted: Bool, err: *mut NSError) {
        if std::env::var_os("ECHOCTL_DEBUG").is_some() {
            // SAFETY: the completion handler's error is nil or a valid NSError.
            let err = unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string());
            eprintln!("notification permission {when}: granted={} error={err:?}", granted.as_bool());
        }
    }

    pub fn post(title: &str, body: &str, click: super::Click) {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        if click == super::Click::Update {
            content.setCategoryIdentifier(&NSString::from_str(UPDATE_CATEGORY));
        }
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&NSUUID::new().UUIDString(), &content, None);
        let (tx, rx) = mpsc::channel::<()>();
        let poster = center.clone();
        // Authorization is asked once (the first time this shows the system prompt); after that
        // it answers at once. Denied: nothing is shown, as the user chose.
        let then = RcBlock::new(move |granted: Bool, err: *mut NSError| {
            debug_permission("", granted, err);
            if granted.as_bool() {
                let done = tx.clone();
                poster.addNotificationRequest_withCompletionHandler(&request, Some(&RcBlock::new(move |_: *mut NSError| { let _ = done.send(()); })));
            } else {
                let _ = tx.send(());
            }
        });
        center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert, &then);
        if !IN_APP.load(Ordering::SeqCst) {
            let _ = rx.recv_timeout(Duration::from_secs(5));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run_args, translocated};

    #[test]
    fn a_started_app_runs_without_its_tray_when_the_one_it_follows_had_none() {
        assert_eq!(run_args(false), ["app", "run", "--background"]);
        // A dev or test run (`app run --no-tray`): its restart and `home set`'s start it the same way (#351).
        assert_eq!(run_args(true), ["app", "run", "--background", "--no-tray"]);
        // An older app's state, without the field, had its tray.
        let w: crate::watch::WatchState = serde_json::from_str(r#"{"pid":1,"heartbeat":"2026-11-10T10:00:00Z","app":"teams","in_call":false,"armed":false,"recording":null}"#).unwrap();
        assert!(!w.no_tray);
    }

    #[test]
    fn a_translocated_app_is_recognised_and_a_real_one_is_not() {
        assert!(translocated(std::path::Path::new("/private/var/folders/x1/abc/T/AppTranslocation/0F3A-11/d/Echo.app/Contents/MacOS/echoctl")));
        assert!(!translocated(std::path::Path::new("/Applications/Echo.app/Contents/MacOS/echoctl")));
        assert!(!translocated(std::path::Path::new("/Users/sam/AppTranslocationNotes/Echo.app")));
    }

    use super::FileLock;

    #[test]
    fn app_quit_goes_through_the_tray_loop_when_there_is_one() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (_env, _root) = crate::paths::testenv::folders("app-quit-route");
        std::fs::create_dir_all(crate::paths::state_dir()).unwrap();
        let ask = || std::fs::write(super::state_file("quit.request"), b"").unwrap();
        // No tray loop (`app run --no-tray`): it stops at once.
        let stop = AtomicBool::new(false);
        ask();
        super::handle_requests(&stop);
        assert!(stop.load(Ordering::SeqCst) && !super::take_quit_request());
        // With it: the loop takes it, once (it saves the window's notes first).
        super::UI_LOOP.store(true, Ordering::SeqCst);
        let stop = AtomicBool::new(false);
        ask();
        super::handle_requests(&stop);
        let taken = (super::take_quit_request(), super::take_quit_request());
        super::UI_LOOP.store(false, Ordering::SeqCst);
        assert!(!stop.load(Ordering::SeqCst) && taken == (true, false));
    }

    #[test]
    fn windows_yes_is_the_default_button_unless_that_is_cancel() {
        const FLAGS: u32 = 4 + 32 + 4096;
        // "Cancel" is the default of the leave dialog: Yes must still mean "Go on".
        assert_eq!(super::yes_no(&["Cancel", "Go on"], "Cancel"), ("Go on", FLAGS + 256));
        assert_eq!(super::yes_no(&["Later", "Update"], "Update"), ("Update", FLAGS));
        // A meeting with a join link: Yes records, it doesn't join.
        assert_eq!(super::yes_no(&["Not now", "Record", "Join & record"], "Record"), ("Record", FLAGS));
    }

    #[test]
    fn shared_holders_exclude_an_exclusive_one_and_back() {
        let dir = std::env::temp_dir().join(format!("echo-test-filelock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.lock");
        let _ = std::fs::remove_file(&p);
        let a = FileLock::try_take_shared(&p).unwrap().expect("first shared");
        let b = FileLock::try_take_shared(&p).unwrap().expect("a second shared one too");
        assert!(FileLock::try_take(&p).unwrap().is_none(), "no exclusive while shared are held");
        drop(a);
        assert!(FileLock::try_take(&p).unwrap().is_none(), "one shared holder is enough to refuse it");
        drop(b);
        let x = std::cell::RefCell::new(None);
        crate::paths::testenv::eventually("exclusive once all shared are gone", || {
            *x.borrow_mut() = FileLock::try_take(&p).unwrap();
            x.borrow().is_some()
        });
        let x = x.into_inner().unwrap();
        assert!(FileLock::try_take_shared(&p).unwrap().is_none(), "no shared while exclusive is held");
        x.set_text("token-1").unwrap();
        x.set_text("t2").unwrap();
        drop(x);
        let s = std::cell::RefCell::new(None);
        crate::paths::testenv::eventually("shared again", || {
            *s.borrow_mut() = FileLock::try_take_shared(&p).unwrap();
            s.borrow().is_some()
        });
        let s = s.into_inner().unwrap();
        assert_eq!(s.text().unwrap(), "t2", "a shorter text replaces a longer one");
        assert!(s.set_text("x").is_err(), "a shared holder can't change it (it can't on Windows)");
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

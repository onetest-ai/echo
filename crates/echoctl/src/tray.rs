//! Menu-bar (macOS) / notification-area (Windows) icon of the echoctl app.
//!
//! The app's whole UI: status, Record / Stop, upcoming meetings, recent calls, settings,
//! permissions and Quit. Clicks call the same `arm` / `stop` as the CLI and Claude Code's
//! skills, so every client sees the same events.
//!
//! macOS needs the event loop on the main thread, so the watcher loop runs on a worker thread.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Local, Utc};

use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::apps::TargetApp;
use crate::config::Config;
use crate::meeting::MeetingContext;
use crate::watch::{self, UiState};
use crate::{app, models, paths, permissions};

mod icons;
mod leaving;
mod menu;
pub use icons::*;
use leaving::*;

enum UiEvent {
    State(UiState),
    Menu(MenuId),
    /// A message from the Echo window's page.
    Ipc(String),
    /// Close the window, quit, update or change the data folder — once the notes typed in the
    /// window are saved (`Leaving`).
    Leave(Then),
    /// The window's page didn't answer that save handshake (its number) in time.
    LeaveTimeout(u64),
    /// After a handshake that failed, the user chose: go on anyway (true) or keep Echo (false) —
    /// or couldn't be asked (why).
    LeaveDecided(Result<bool, String>),
    Quit,
}

/// What a leave does once the window has saved the notes it holds.
#[derive(Debug)]
enum Then {
    CloseWindow,
    Quit,
    /// `echoctl app quit` asked (it waits for the outcome).
    QuitAsked,
    Update(crate::update::Staged),
    ChangeHome(PathBuf),
}

impl Then {
    fn question(&self) -> &'static str {
        match self {
            Then::CloseWindow => "Close the window anyway?",
            Then::Quit | Then::QuitAsked => "Quit Echo anyway?",
            Then::Update(_) => "Install the update anyway?",
            Then::ChangeHome(_) => "Change the data folder anyway?",
        }
    }
}

/// (icon, is_template). Idle is a template icon (follows the macOS menu-bar light/dark look);
/// armed is green and recording is Claude orange so the state reads at a glance.
/// (Windows has no templates: idle is black, or white on a dark taskbar.)
fn icon_for(state: &UiState) -> (Icon, bool) {
    match state {
        UiState::Idle { .. } if taskbar_dark() => (logo(WHITE), true),
        UiState::Idle { .. } => (logo(BLACK), true),
        UiState::Armed => (logo(GREEN), false),
        UiState::Recording { .. } => (logo(ORANGE), false),
    }
}

/// Windows: is the taskbar dark? (The system theme, not the apps' one, colours the taskbar.)
fn taskbar_dark() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows::core::w;
        use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
        let mut light = 1u32;
        let mut len = 4u32;
        let ok = RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut light as *mut u32 as *mut _),
            Some(&mut len),
        )
        .is_ok();
        return ok && light == 0;
    }
    #[allow(unreachable_code)]
    false
}

/// Is a menu open right now? macOS tracks an open menu in the event-tracking run loop mode, and
/// the event loop keeps running meanwhile: a menu replaced then closes under the pointer.
fn menu_open() -> bool {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::NSRunLoop;
        NSRunLoop::mainRunLoop().currentMode().is_some_and(|m| m.to_string() == "NSEventTrackingRunLoopMode")
    }
    #[cfg(not(target_os = "macos"))]
    false
}

fn status_text(state: &UiState) -> String {
    match state {
        UiState::Idle { in_call: Some(true) } => "Call in progress — not recording".into(),
        UiState::Idle { .. } => "Not recording".into(),
        UiState::Armed => "Armed — recording starts with the next call".into(),
        UiState::Recording { since, memo: true, .. } => format!("● Recording a memo · since {}", since.format("%H:%M")),
        UiState::Recording { title, since, .. } => format!(
            "● Recording{} · since {}",
            title.as_deref().map(|t| format!(" · {t}")).unwrap_or_default(),
            since.format("%H:%M")
        ),
    }
}

/// Everything the menu shows besides the live status line. The menu is rebuilt only when this
/// changes (checked every few seconds), so it stays cheap.
#[derive(Clone, PartialEq)]
struct Model {
    upcoming: Vec<MeetingContext>,
    /// (menu label, call folder name)
    recent: Vec<(String, String)>,
    unscheduled_manual: bool,
    scheduled_manual: bool,
    keep_audio: bool,
    minutes_page: bool,
    auto_minutes: bool,
    capture_frames: bool,
    active_model: Option<String>,
    downloaded_models: Vec<&'static str>,
    languages: Vec<String>,
    health: Vec<(String, String, Option<String>)>, // (name, status, settings url)
    home: PathBuf,
    autostart: bool,
    recommended: &'static str,
    /// The model being downloaded, with percent
    downloading: Option<(&'static str, u8)>,
    notes_in: Option<PathBuf>,
    /// AI profiles (`ai.rs`); None = ai.json unreadable
    ai: Option<AiMenu>,
    has_claude: bool,
    plugin_installed: bool,
    cli_installed: bool,
    /// The transcription server this computer uses ("<name> (<url>)").
    server: Option<String>,
    /// This computer transcribes for others: the URL they connect to.
    serving: Option<String>,
    update: Option<String>,
    calendar_account: Option<String>,
    graph_client: (String, bool),
}

/// The "AI for calls" menu: live notes on, the default id, [(id, name, kind, sign-in line, the
/// account to offer — `claude` profiles signed in to another one than they were set up with —,
/// the model that writes its minutes)].
type AiMenu = (bool, String, Vec<(String, String, crate::ai::Kind, String, Option<String>, String)>);

/// A Claude profile's sign-in, in a menu line (`signin::state`).
fn sign_in_line(p: &crate::ai::Profile, st: Option<&crate::signin::SignInState>) -> String {
    use crate::signin::SignInState as S;
    let what = match st {
        Some(S::SignedIn { account, .. }) => account.clone(),
        Some(S::OtherAccount { account, expected }) => format!("signed in as {account}, set up for {expected}"),
        Some(S::SignedOut) => "not signed in".into(),
        Some(S::Starting | S::Checking) => "checking…".into(),
        Some(S::Waiting { .. }) => "waiting for you in the browser…".into(),
        Some(S::Stopping) => "stopping the sign-in…".into(),
        Some(S::Elsewhere) => "being signed in in a terminal…".into(),
        Some(S::Failed { error }) => format!("the sign-in didn't finish: {error}"),
        Some(S::Unknown { error: Some(e) }) => format!("couldn't check the sign-in: {e}"),
        Some(S::Unknown { error: None }) | None => p.account.clone().map_or_else(|| "not checked yet".into(), |a| format!("{a} (not checked yet)")),
    };
    format!("{}: {what}", p.name)
}

const UPCOMING_MAX: usize = 6;
const RECENT_MAX: usize = 5;

fn load_model(health: &[(String, String, Option<String>)]) -> Model {
    let now = Utc::now();
    let today = Local::now().date_naive();
    let mut upcoming: Vec<MeetingContext> = watch::agenda()
        .into_iter()
        .filter(|m| {
            m.recordable().is_ok() && watch::meeting_start(m).is_some_and(|s| {
                s + chrono::Duration::minutes(4) > now && s.with_timezone(&Local).date_naive() == today
            })
        })
        .collect();
    upcoming.sort_by_key(watch::meeting_start);
    upcoming.truncate(UPCOMING_MAX);

    // The calls in the app's one order (`calls::all_calls`: undated folders oldest), newest first below.
    let dirs: Vec<PathBuf> = crate::calls::all_calls();
    let recording_dir = watch::running_watcher().and_then(|w| w.recording).map(PathBuf::from);
    let recent = dirs
        .iter()
        .rev()
        .take(RECENT_MAX)
        .map(|d| {
            let name = d.file_name().unwrap().to_string_lossy().into_owned();
            let meta: serde_json::Value = std::fs::read(d.join("meta.json"))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            // "2026-10-09_174948-switch-test" → "Oct 09 17:49 · switch test"
            let when = chrono::NaiveDateTime::parse_from_str(name.get(..17).unwrap_or(&name), "%Y-%m-%d_%H%M%S")
                .map(|t| t.format("%b %d %H:%M").to_string())
                .unwrap_or_default();
            let title = meta["title"]
                .as_str()
                .filter(|t| !t.is_empty())
                .map(str::to_owned)
                .or_else(|| name.get(18..).filter(|t| !t.is_empty()).map(|t| t.replace('-', " ")))
                .unwrap_or("call".into());
            let parse = |k: &str| meta[k].as_str().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
            let minutes = match (parse("started_at"), parse("ended_at")) {
                (Some(a), Some(b)) => format!(" · {} min", ((b - a).num_seconds() + 30).max(60) / 60),
                _ => String::new(),
            };
            let status = if recording_dir.as_deref().is_some_and(|r| d.ends_with(r) || r.ends_with(d)) {
                "● recording"
            } else if d.join(crate::transcribe_marker()).exists() {
                "transcribing…"
            } else if d.join("notes.md").exists() {
                "✓ minutes"
            } else if d.join("transcript.md").exists() {
                "✓ transcript"
            } else {
                "not transcribed"
            };
            let label = format!("{when} · {}{minutes} · {status}", ellipsize(&title, 32));
            (label, name)
        })
        .collect();

    let cfg = Config::load().unwrap_or_default();
    let downloaded_models: Vec<&'static str> =
        models::CATALOG.iter().filter(|m| models::model_path(m).exists()).map(|m| m.name).collect();
    let claude = app::claude_status();
    Model {
        upcoming,
        recent,
        unscheduled_manual: !cfg.ask_unscheduled(),
        scheduled_manual: !cfg.ask_scheduled(),
        keep_audio: cfg.keep_audio,
        minutes_page: cfg.minutes_page,
        auto_minutes: cfg.auto_minutes,
        capture_frames: cfg.capture_frames,
        active_model: cfg.model.clone(),
        downloaded_models,
        languages: cfg.languages.clone(),
        health: health.to_vec(),
        home: paths::home(),
        autostart: app::autostart_enabled(),
        recommended: models::recommend(&models::hardware()).0,
        downloading: models::downloading(),
        notes_in: cfg.notes_in.clone(),
        ai: crate::ai::Settings::load().ok().map(|s| {
            let all = s
                .all()
                .into_iter()
                .map(|p| {
                    let st = crate::signin::state(&p);
                    let other = match &st {
                        Some(crate::signin::SignInState::OtherAccount { account, .. }) => Some(account.clone()),
                        _ => None,
                    };
                    (p.id.clone(), p.name.clone(), p.kind, sign_in_line(&p, st.as_ref()), other, p.notes_model_label())
                })
                .collect();
            (s.live_notes, s.default_id(), all)
        }),
        has_claude: !claude["claude"].is_null(),
        plugin_installed: claude["plugin_installed"] == true,
        cli_installed: app::cli_installed(),
        #[cfg(feature = "transcribe")]
        server: crate::remote::client_label(),
        #[cfg(not(feature = "transcribe"))]
        server: None,
        #[cfg(feature = "transcribe")]
        serving: Some(crate::remote::serve_settings()).filter(|s| s.on).map(|s| crate::remote::serve_url(&s)),
        #[cfg(not(feature = "transcribe"))]
        serving: None,
        calendar_account: crate::graph::account(),
        graph_client: {
            let (id, _) = crate::graph::app_registration();
            let default = id == crate::graph::DEFAULT_CLIENT_ID;
            (id, default)
        },
        update: crate::update::AVAILABLE.lock().unwrap().as_ref().map(|r| r.version.clone()),
    }
}

/// Permission checks for the Health submenu (no probe: that needs audio playing).
fn health() -> Vec<(String, String, Option<String>)> {
    match permissions::report() {
        Ok(r) => r
            .checks
            .iter()
            .map(|c| {
                let status = match c.status {
                    permissions::Status::Granted => "✓",
                    permissions::Status::Denied => "✗",
                    _ => "?",
                };
                (c.name.to_string(), status.to_string(), c.settings_url.map(str::to_owned))
            })
            .collect(),
        Err(_) => vec![],
    }
}

/// Display names for language codes offered in the Record menu.
fn language_name(code: &str) -> String {
    match code {
        "en" => "English".into(),
        "uk" => "Українська".into(),
        "ru" => "Русский".into(),
        "es" => "Español".into(),
        "de" => "Deutsch".into(),
        "fr" => "Français".into(),
        "pl" => "Polski".into(),
        other => other.to_string(),
    }
}

#[derive(Clone)]
enum Action {
    Record,
    /// Record the user alone now (`watch::memo_start`).
    Memo,
    CheckUpdates,
    RecordIn(String),
    Stop,
    ArmMeeting(Box<MeetingContext>),
    JoinMeeting(Box<MeetingContext>),
    Open(PathBuf),
    SetUnscheduled(&'static str),
    SetScheduled(&'static str),
    ToggleKeepAudio,
    ToggleMinutesPage,
    ToggleAutoMinutes,
    ToggleFrames,
    UseModel(&'static str),
    RemoveModel(&'static str),
    NotesAnywhere,
    NotesChoose,
    /// Settings → Calendar & chat: the folder OneDrive keeps on this computer (#469).
    OnedriveChoose,
    ConnectClaude,
    /// Claude Code's folder on the PATH of new terminals (Settings → Health).
    ClaudePath,
    InstallCli,
    Update,
    EditProfile,
    AddNote,
    /// Open the Echo window (at a call, by folder name; an echo:// link; or with What's new, `window::WHATS_NEW`).
    OpenWindow(Option<String>),
    CalendarLogin,
    PhotosLogin,
    CalendarLogout,
    CalendarSync,
    CalendarClientId,
    NotesModel,
    ToggleLiveNotes,
    DefaultProfile(String),
    SignIn(String),
    /// (profile, the account shown)
    AcceptAccount(String, String),
    Recheck,
    AskPermissions,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    RetestGpu,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    ConnectServer,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    DisconnectServer,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    ToggleServing,
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    ShowServing,
    OpenUrl(String),
    ChangeHome,
    ToggleAutostart,
    DownloadModel(&'static str),
    SwitchModel(&'static str),
    Quit,
}

struct Ui {
    proxy: tao::event_loop::EventLoopProxy<UiEvent>,
    quit: MenuItem,
    tray: TrayIcon,
    status: MenuItem,
    record: Submenu,
    memo: MenuItem,
    stop: MenuItem,
    note: MenuItem,
    actions: HashMap<MenuId, Action>,
    state: Option<UiState>,
    /// The taskbar theme the icon was drawn for (Windows: the idle icon follows it).
    dark: bool,
    model: Option<Model>,
    health: Vec<(String, String, Option<String>)>,
    last_refresh: Instant,
    /// A refresh waits for the open menu to close (`menu_open`).
    refresh_owed: bool,
    /// Set by a click: open (or focus) the window, at this call; the event loop does it.
    open_window: Option<Option<String>>,
}

fn local_hm(m: &MeetingContext) -> String {
    watch::meeting_start(m).map(|s| s.with_timezone(&Local).format("%H:%M").to_string()).unwrap_or_default()
}

/// Settings → Connect to a transcription server…: its URL, then its API key; checked before
/// it is kept.
#[cfg(feature = "transcribe")]
fn connect_server() {
    let Some(url) = app::ask_text(
        "Transcription server URL (http:// or https://).\n\nAnother Echo shows it under Settings → Show URL and API key…",
        "http://",
        "Next",
    ) else {
        return;
    };
    let Some(key) = app::ask_text("API key for this server:", "", "Connect") else { return };
    match crate::remote::connect(&url, &key) {
        Ok(h) => app::notify("Transcription server", &format!("Calls are transcribed on {} ({}, {}).", h.name, h.model, h.engine)),
        Err(e) => app::notify("Could not connect", &format!("{e:#}")),
    }
}

/// What other computers enter to transcribe here; the dialog's Copy button copies both lines.
#[cfg(feature = "transcribe")]
fn show_serving(s: &crate::remote::ServeSettings) {
    let (url, key) = (crate::remote::serve_url(s), s.key.clone());
    let text = format!(
        "Other computers can transcribe their calls on this one while Echo runs here.\n\n\
         In their Echo: Settings → Connect to a transcription server…\n\nURL:  {url}\nAPI key:  {key}\n\n\
         The audio of each utterance is sent over your network unencrypted. Nothing is kept on this computer."
    );
    if app::message(&text, &["Copy", "OK"]).as_deref() == Some("Copy") {
        crate::window::copy_to_clipboard(&format!("URL: {url}\nAPI key: {key}"));
    }
}

fn ellipsize(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n - 1).collect::<String>())
    }
}

/// macOS's own Quit — the Dock icon's menu while the window is open, a logout — asks the app
/// (`applicationShouldTerminate:`), which tao's delegate doesn't answer: the process would end at
/// once, losing what is typed in the window and skipping the recording's close. Answered here
/// "later" (`NSTerminateLater`), and the app quits its own way (`Leaving` → the watcher stops), then
/// answers "yes" (macOS terminates it); a quit cancelled after a failed save is answered "no".
#[cfg(target_os = "macos")]
mod terminate {
    use super::{Then, UiEvent};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    static PROXY: OnceLock<Mutex<tao::event_loop::EventLoopProxy<UiEvent>>> = OnceLock::new();
    /// macOS waits for our answer.
    static PENDING: AtomicBool = AtomicBool::new(false);

    const NS_TERMINATE_NOW: usize = 1;
    const NS_TERMINATE_LATER: usize = 2;

    unsafe extern "C-unwind" fn should_terminate(_this: &objc2::runtime::AnyObject, _cmd: objc2::runtime::Sel, _app: &objc2::runtime::AnyObject) -> usize {
        let Some(p) = PROXY.get() else { return NS_TERMINATE_NOW };
        if !PENDING.swap(true, Ordering::SeqCst) {
            let _ = p.lock().unwrap_or_else(|e| e.into_inner()).send_event(UiEvent::Leave(Then::Quit));
        }
        NS_TERMINATE_LATER
    }

    /// Answer `applicationShouldTerminate:` for tao's app delegate (it has no answer of its own).
    pub fn guard(proxy: tao::event_loop::EventLoopProxy<UiEvent>) {
        let _ = PROXY.set(Mutex::new(proxy));
        let Some(cls) = objc2::runtime::AnyClass::get(c"TaoAppDelegateParent") else {
            eprintln!("macOS's Quit doesn't wait for the window's notes: tao's app delegate wasn't found");
            return;
        };
        let sel = objc2::sel!(applicationShouldTerminate:);
        if cls.instance_method(sel).is_some() {
            eprintln!("macOS's Quit doesn't wait for the window's notes: tao answers applicationShouldTerminate: itself");
            return;
        }
        // SAFETY: the function has the selector's signature (`- (NSApplicationTerminateReply)
        // applicationShouldTerminate:(NSApplication *)sender`, type "Q@:@"), and the class exists.
        unsafe {
            let imp: objc2::runtime::Imp = std::mem::transmute(should_terminate as unsafe extern "C-unwind" fn(_, _, _) -> usize);
            objc2::ffi::class_addMethod(cls as *const _ as *mut _, sel, imp, c"Q@:@".as_ptr());
        }
    }

    /// A quit was cancelled (the user kept Echo, `Step::Stay`): so is the one macOS asked for, if it
    /// waits — it asks nothing again until it gets an answer. A quit of `echoctl app quit` may carry
    /// it (`takes_over`).
    pub fn cancelled() {
        reply(false);
    }

    /// Echo has stopped its own way (`Leaving`, the watcher's close): macOS may terminate it now — it
    /// waits for this answer, and the event loop's stop alone doesn't release it.
    pub fn finished() {
        reply(true);
    }

    /// On the event loop's (main) thread only: elsewhere it would take the answer without giving it.
    fn reply(go: bool) {
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else { return };
        if PENDING.swap(false, Ordering::SeqCst) {
            objc2_app_kit::NSApplication::sharedApplication(mtm).replyToApplicationShouldTerminate(go);
        }
    }
}

/// Do what the leave was for; whether Echo stops now.
fn go(then: Then, win: &mut Option<crate::window::EchoWindow>, stop: &Arc<AtomicBool>, ui: &mut Option<Ui>) -> bool {
    match then {
        Then::CloseWindow => {
            *win = None; // closing the window leaves Echo running, in the menu bar (and the Dock unless hidden)
            apply_dock(false);
            false
        }
        Then::Quit | Then::QuitAsked => {
            if let Some(u) = ui.as_mut() {
                u.quitting();
            }
            stop.store(true, Ordering::SeqCst);
            true
        }
        Then::Update(staged) => match crate::update::commit(&staged, std::process::id()) {
            Ok(()) => {
                stop.store(true, Ordering::SeqCst); // the swap runs after exit
                true
            }
            Err(e) => {
                app::notify("Update not installed", &format!("{e:#}"));
                thaw(win);
                false
            }
        },
        Then::ChangeHome(dir) => match paths::set_home(&dir) {
            Ok(_) => {
                app::RESTART.store(true, Ordering::SeqCst);
                stop.store(true, Ordering::SeqCst);
                true
            }
            Err(e) => {
                app::notify("Couldn't change the data folder", &format!("{e:#}"));
                thaw(win);
                false
            }
        },
    }
}

/// The window's editors were frozen for a leave that didn't happen: writable again.
fn thaw(win: &Option<crate::window::EchoWindow>) {
    if let Some(w) = win {
        let _ = w.webview.evaluate_script("window.echo && window.echo.thaw && window.echo.thaw()");
    }
}

/// Settings → General → "Hide from the Dock when the window is closed".
#[cfg(target_os = "macos")]
fn dock_hidden() -> bool {
    Config::load().is_ok_and(|c| c.hide_dock_icon)
}

/// macOS: the Dock icon as the setting and the window say — shown while the window is open, else unless hidden.
/// On the event loop's (main) thread; elsewhere it does nothing.
fn apply_dock(window_open: bool) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        let Some(mtm) = objc2_foundation::MainThreadMarker::new() else { return };
        let policy = if window_open || !dock_hidden() { NSApplicationActivationPolicy::Regular } else { NSApplicationActivationPolicy::Accessory };
        let app = NSApplication::sharedApplication(mtm);
        if app.activationPolicy() != policy {
            app.setActivationPolicy(policy);
            // Back in the Dock for a window just opened: AppKit doesn't make it the active app by itself (its window
            // stayed behind the one in front), so it is activated as a Dock app's window would be.
            if policy == NSApplicationActivationPolicy::Regular && window_open {
                #[allow(deprecated)]
                app.activateIgnoringOtherApps(true);
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window_open;
}

/// Open the Echo window, or bring it forward (on `call` when one is named).
fn show_window(target: &tao::event_loop::EventLoopWindowTarget<UiEvent>, win: &mut Option<crate::window::EchoWindow>, call: Option<&str>, proxy: &tao::event_loop::EventLoopProxy<UiEvent>) {
    match win {
        Some(w) => {
            match call {
                Some(l) if l.starts_with("echo://") => w.open_link(l),
                Some(crate::window::WHATS_NEW) => w.whats_new(),
                Some(c) => w.show_call(c),
                None => {}
            }
            w.window.set_visible(true);
            w.window.set_minimized(false);
            w.window.set_focus();
        }
        None => {
            let p = proxy.clone();
            match crate::window::open(target, call, move |m| drop(p.send_event(UiEvent::Ipc(m)))) {
                Ok(w) => {
                    apply_dock(true);
                    w.window.set_focus();
                    *win = Some(w);
                }
                Err(e) => app::notify("Could not open the Echo window", &e.to_string()),
            }
        }
    }
}

/// The onetest logo as the Dock icon of a dev build, which has no bundle icon. Echo.app keeps
/// macOS: the app's menu bar — Echo (Hide, Hide Others, Quit) and Edit (Undo, Redo, Cut, Copy, Paste, Select All). WebKit
/// takes ⌘C, ⌘V, ⌘X, ⌘A and ⌘Z only from a menu: without one, nothing in the window could be copied or pasted. Quit
/// goes through `applicationShouldTerminate:` as ⌘Q always did (`terminate`).
#[cfg(target_os = "macos")]
fn app_menu() -> Option<Menu> {
    let m = Menu::new();
    let app = Submenu::with_items("Echo", true, &[&PredefinedMenuItem::hide(None), &PredefinedMenuItem::hide_others(None), &PredefinedMenuItem::separator(), &PredefinedMenuItem::quit(None)]).ok()?;
    let edit = Submenu::with_items(
        "Edit",
        true,
        &[&PredefinedMenuItem::undo(None), &PredefinedMenuItem::redo(None), &PredefinedMenuItem::separator(), &PredefinedMenuItem::cut(None), &PredefinedMenuItem::copy(None), &PredefinedMenuItem::paste(None), &PredefinedMenuItem::select_all(None)],
    )
    .ok()?;
    m.append_items(&[&app, &edit]).ok()?;
    m.init_for_nsapp();
    Some(m)
}

/// its icns (the full rounded-square icon).
#[cfg(target_os = "macos")]
fn set_app_icon() {
    if crate::app::bundle_path().is_some() {
        return;
    }
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::{MainThreadMarker, NSData};
    const LOGO: &[u8] = include_bytes!("../assets/onetest-logo-512.png");
    let Some(mtm) = MainThreadMarker::new() else { return }; // the tao loop runs on the main thread
    let data = NSData::with_bytes(LOGO);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // SAFETY: called on the main thread (checked above) with a valid NSImage.
        unsafe { NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

/// Run the watcher with a tray icon. Never returns; exits once the watcher loop has finished
/// (and closed any recording).
pub fn run(app: TargetApp, stop: Arc<AtomicBool>) -> ! {
    let stop_for_ui = stop.clone();
    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UiEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        // A Dock icon for as long as Echo runs, not only the menu bar's: macOS hides menu-bar icons
        // that don't fit (beside the notch, many of them), and Echo then looked gone. Its click opens
        // the window. Echo.app stays `LSUIElement`, so the CLI from the same binary never shows one.
        // Settings → General → Hide from the Dock (off by default): only while the window is open.
        event_loop.set_activation_policy(if dock_hidden() { ActivationPolicy::Accessory } else { ActivationPolicy::Regular });
        event_loop.set_activate_ignoring_other_apps(false); // starting at login never takes the focus
        crate::app::notifications::init();
    }

    let proxy = event_loop.create_proxy();
    // macOS's own Quit (the Dock's) goes through `Leaving` too.
    #[cfg(target_os = "macos")]
    terminate::guard(proxy.clone());
    let p = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = p.send_event(UiEvent::Menu(e.id));
    }));
    let p = proxy.clone();
    std::thread::spawn(move || {
        // `Quit` ends the event loop once the watcher has finished — also if it panics: a quit
        // macOS waits on is answered only then.
        struct Finished(tao::event_loop::EventLoopProxy<UiEvent>);
        impl Drop for Finished {
            fn drop(&mut self) {
                let _ = self.0.send_event(UiEvent::Quit);
            }
        }
        let _finished = Finished(p.clone());
        if let Err(e) = watch::run_loop(app, stop, |s| {
            let _ = p.send_event(UiEvent::State(s));
        }) {
            eprintln!("watcher failed: {e:#}");
        }
    });

    let mut ui: Option<Ui> = None;
    // macOS: the app's own menu bar (Echo, Edit), kept for as long as the loop runs.
    #[cfg(target_os = "macos")]
    let mut _app_menu: Option<Menu> = None;
    let mut win: Option<crate::window::EchoWindow> = None;
    let mut leaving = Leaving::default();
    app::UI_LOOP.store(true, Ordering::SeqCst);
    event_loop.run(move |event, target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { window_id, event: WindowEvent::CloseRequested, .. }
                if win.as_ref().is_some_and(|w| w.window.id() == window_id) =>
            {
                let step = leaving.start(Then::CloseWindow, win.is_some());
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            #[cfg(windows)] // Echo's own title bar: the maximise button shows restore while the window is maximised.
            Event::WindowEvent { window_id, event: WindowEvent::Resized(_), .. } => win.iter().filter(|w| w.window.id() == window_id).for_each(|w| w.report_maximized()),
            Event::UserEvent(UiEvent::Leave(then)) => {
                let step = leaving.start(then, win.is_some());
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            Event::UserEvent(UiEvent::LeaveTimeout(n)) => {
                let step = leaving.answered(n, None);
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            Event::UserEvent(UiEvent::LeaveDecided(answer)) => {
                if let Err(why) = &answer {
                    app::notify("Echo stays open", &format!("Notes typed in the Echo window couldn't be saved, and Echo couldn't ask what to do: {why}"));
                }
                let step = leaving.decided(answer);
                act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
            }
            Event::UserEvent(UiEvent::Ipc(msg)) => match crate::window::closing(&msg) {
                // The page saved what it holds (or couldn't): go on, or ask — a close the page
                // itself explains (its Try again / Close without saving).
                Some(crate::window::Closing::Ready { ok, n }) => {
                    let step = leaving.answered(n, Some(ok));
                    act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
                }
                Some(crate::window::Closing::Now) => {
                    let step = leaving.close_now();
                    act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
                }
                Some(crate::window::Closing::Request) => {
                    let step = leaving.start(Then::CloseWindow, win.is_some());
                    act(step, &mut leaving, &mut win, &stop_for_ui, &mut ui, &proxy);
                }
                // Settings in the window started a flow the menu has too: run it the same way.
                None if crate::window::settings_action(&msg).is_some() => {
                    if let (Some((name, arg)), Some(u)) = (crate::window::settings_action(&msg), ui.as_mut()) {
                        match Ui::page_action(&name, &arg) {
                            Some(a) => {
                                u.run(a);
                                u.refresh_model(true);
                            }
                            None => eprintln!("settings: no action {name:?}"),
                        }
                    }
                    if let Some(w) = &win {
                        w.refresh();
                    }
                }
                // The page's own title bar (macOS; Windows with Echo's own title bar): drag, zoom, minimise, resize.
                None if crate::window::window_cmd(&msg).is_some() => win.iter().for_each(|w| w.page_cmd(&msg)),
                None => {
                    let done = crate::window::handle(&msg);
                    #[cfg(windows)] // Settings → Layout → "Echo's own title bar": the window follows at once.
                    win.iter().for_each(|w| w.follow_settings(&msg));
                    if let (Some(js), Some(w)) = (&done.reply, &win) {
                        let _ = w.webview.evaluate_script(js);
                    }
                    if done.refresh {
                        // The action changed something: refresh the page and the menu.
                        if let Some(w) = &win {
                            w.refresh();
                        }
                        if let Some(u) = ui.as_mut() {
                            u.refresh_model(true);
                        }
                    }
                }
            },
            // The Dock icon clicked (or Echo.app opened again) with no window showing: open it — also
            // when the menu-bar icon is hidden (no room beside the notch) or couldn't be made.
            Event::Reopen { has_visible_windows: false, .. } if leaving.may_open() => show_window(target, &mut win, None, &proxy),
            // macOS: create the icon only once the event loop is running.
            Event::NewEvents(StartCause::Init) => {
                #[cfg(target_os = "macos")]
                {
                    set_app_icon(); // once AppKit has finished launching: earlier it would put its own back
                    if let Some(m) = app_menu() { _app_menu.replace(m); } // held by the loop: the menu bar lasts as long as its `Menu` (`=` reads as never read)
                }
                match Ui::new(proxy.clone()) {
                    Ok(u) => ui = Some(u),
                    Err(e) => eprintln!("tray icon unavailable: {e}"),
                }
            }
            // The system opens an echo:// link (a note in Obsidian, a mail): the window, there.
            Event::Opened { urls } => {
                for u in urls.iter().map(|u| u.to_string()).filter(|u| crate::links::Link::parse(u).is_some()).take(1) {
                    if leaving.may_open() {
                        show_window(target, &mut win, Some(&u), &proxy);
                    }
                }
            }
            Event::UserEvent(UiEvent::State(s)) => {
                // `echoctl app quit` — also when the tray icon couldn't be made (no window then).
                if app::take_quit_request() {
                    let _ = proxy.send_event(UiEvent::Leave(Then::QuitAsked));
                }
                if let Some(u) = ui.as_mut() {
                    u.apply(s);
                    u.refresh_model(false);
                    if app::take_update_request() {
                        u.start_update();
                    }
                    if let Some(call) = app::take_open_request() {
                        u.open_window = Some(call);
                        let _ = proxy.send_event(UiEvent::Menu(MenuId::new("echo-open-window")));
                    }
                }
            }
            Event::UserEvent(UiEvent::Menu(id)) => {
                if let Some(u) = ui.as_mut() {
                    u.click(&id);
                    if let Some(call) = u.open_window.take().filter(|_| leaving.may_open()) {
                        show_window(target, &mut win, call.as_deref(), &proxy);
                    }
                }
            }
            Event::UserEvent(UiEvent::Quit) => {
                // Terminals in the window (Claude sessions) end with Echo; they resume by id later.
                crate::terminal::close_all();
                // Plugins' scripts stop with Echo, with everything they started.
                crate::plugins::scripts::stop_all();
                if app::RESTART.load(Ordering::SeqCst) {
                    let _ = app::spawn_app(true);
                }
                // A quit macOS asked for waits for its answer: the watcher has closed everything.
                #[cfg(target_os = "macos")]
                terminate::finished();
                *control_flow = ControlFlow::Exit
            }
            _ => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_announcements_switch_is_the_trays_own_action() {
        let a = Ui::page_action("switch_model", crate::models::PARAKEET);
        assert!(matches!(a, Some(Action::SwitchModel(n)) if n == crate::models::PARAKEET));
        assert!(Ui::page_action("switch_model", "no-such-model").is_none());
    }

    #[test]
    fn leaving_waits_for_the_notes_in_the_window() {
        assert!(matches!(after_handshake(Then::Quit, Some(true)), Next::Go(Then::Quit)));
        assert!(matches!(after_handshake(Then::CloseWindow, Some(false)), Next::Stay), "the page says why, with its own buttons");
        assert!(matches!(after_handshake(Then::QuitAsked, Some(false)), Next::Ask(Then::QuitAsked, _)));
        assert!(matches!(after_handshake(Then::ChangeHome("/x".into()), Some(false)), Next::Ask(Then::ChangeHome(_), _)));
        assert!(matches!(after_handshake(Then::CloseWindow, None), Next::Ask(Then::CloseWindow, _)), "no answer: asked, never closed quietly");
        assert!(matches!(after_handshake(Then::Quit, None), Next::Ask(Then::Quit, _)));
    }

    #[test]
    fn a_quit_is_never_dropped_for_another_leave() {
        let home = || Then::ChangeHome("/x".into());
        assert!(takes_over(&Then::CloseWindow, &Then::Quit) && takes_over(&Then::CloseWindow, &home()));
        assert!(takes_over(&home(), &Then::Quit) && takes_over(&home(), &Then::QuitAsked), "macOS waits for its answer");
        assert!(takes_over(&Then::Quit, &Then::QuitAsked), "`app quit` waits for the outcome");
        for (pending, new) in [(Then::QuitAsked, Then::Quit), (Then::Quit, Then::CloseWindow), (Then::Quit, home()), (home(), Then::CloseWindow)] {
            assert!(!takes_over(&pending, &new), "{pending:?} stays for {new:?}");
        }
    }

    #[test]
    fn one_leave_at_a_time_from_the_save_to_echo_stopping() {
        let mut l = Leaving::default();
        // A close with the window open: its page saves first.
        assert!(matches!(l.start(Then::CloseWindow, true), Step::Save { n: 1, close: true }));
        // A quit meanwhile takes its place; a close then doesn't.
        assert!(matches!(l.start(Then::Quit, true), Step::Nothing));
        assert!(matches!(l.start(Then::CloseWindow, true), Step::Nothing));
        // An answer for another handshake changes nothing; this one's failure asks the user.
        assert!(matches!(l.answered(7, Some(true)), Step::Nothing));
        assert!(matches!(l.answered(1, Some(false)), Step::Ask(q) if q.contains("Quit Echo anyway?")));
        assert!(matches!(l.answered(1, None), Step::Nothing), "the answered handshake's timeout");
        // While the user is asked: no second handshake; `app quit` waits for the answer (and the
        // CLI with it: quit.asking), it doesn't take this question over.
        assert!(!l.asking_quit());
        assert!(matches!(l.start(Then::QuitAsked, true), Step::Nothing));
        assert!(l.asking_quit() && l.next().is_none());
        // Kept open: then `app quit` gets its own turn — its own save and question.
        assert!(matches!(l.decided(Ok(false)), Step::Stay(Then::Quit, why) if why.contains("kept open")));
        let next = l.next();
        assert!(matches!(next, Some(Then::QuitAsked)) && !l.asking_quit());
        assert!(matches!(l.start(next.unwrap(), true), Step::Save { n: 2, close: false }));
        assert!(matches!(l.answered(2, Some(false)), Step::Ask(_)));
        assert!(l.asking_quit());
        assert!(matches!(l.decided(Ok(false)), Step::Stay(Then::QuitAsked, _)));
        // A folder change that goes ahead: Echo stops, and nothing else starts.
        assert!(matches!(l.start(Then::ChangeHome("/x".into()), true), Step::Save { n: 3, close: false }));
        assert!(matches!(l.answered(3, Some(true)), Step::Go(Then::ChangeHome(_))));
        l.went(true);
        assert!(matches!(l.start(Then::Quit, true), Step::Nothing));
        assert!(matches!(l.close_now(), Step::Nothing));
    }

    #[test]
    fn a_question_is_answered_only_for_its_own_leave() {
        // "Close the window anyway?" open; a quit arrives: it waits, the answer is the close's.
        let mut l = Leaving::default();
        l.start(Then::CloseWindow, true);
        assert!(matches!(l.answered(1, None), Step::Ask(q) if q.contains("Close the window anyway?")));
        l.start(Then::Quit, true);
        assert!(matches!(l.decided(Ok(true)), Step::Go(Then::CloseWindow)));
        l.went(false);
        assert!(matches!(l.next(), Some(Then::Quit)), "then the quit goes on (no window: at once)");
        // Only one waits: what would take over the waiting one replaces it, else it's dropped.
        let mut l = Leaving::default();
        l.start(Then::ChangeHome("/x".into()), true);
        l.answered(1, None);
        l.start(Then::Quit, true);
        l.start(Then::CloseWindow, true);
        l.start(Then::QuitAsked, true);
        l.decided(Ok(false));
        assert!(matches!(l.next(), Some(Then::QuitAsked)));
    }

    #[test]
    fn without_a_window_a_leave_goes_at_once_and_close_without_saving_goes_on() {
        let mut l = Leaving::default();
        assert!(matches!(l.start(Then::QuitAsked, false), Step::Go(Then::QuitAsked)), "no window (or no tray icon): nothing to save");
        let mut l = Leaving::default();
        assert!(matches!(l.close_now(), Step::Go(Then::CloseWindow)));
        l.went(false);
        // Still saving: the leave goes on without the notes.
        assert!(matches!(l.start(Then::ChangeHome("/x".into()), true), Step::Save { .. }));
        assert!(matches!(l.close_now(), Step::Go(Then::ChangeHome(_))));
        l.went(false); // the folder couldn't be changed: Echo stays
        // Asking the user: the window closes, the question stays.
        assert!(matches!(l.start(Then::Quit, true), Step::Save { n: 2, .. }));
        assert!(matches!(l.answered(2, None), Step::Ask(_)));
        assert!(matches!(l.close_now(), Step::CloseWindow));
        assert!(matches!(l.decided(Err("no dialog".into())), Step::Stay(Then::Quit, why) if why.contains("couldn't ask")));
    }

    #[test]
    fn the_menus_whats_new_opens_a_place_the_window_routes() {
        let w = crate::window::WHATS_NEW;
        assert!(w.starts_with('#'), "a place of the window's own, never taken for a call's folder");
        let script = format!("{}{}", include_str!("../ui/app/08-data.js"), include_str!("../ui/app/99-refresh.js"));
        assert!(script.contains(&format!("h === '{}'", &w[1..])), "a new window's route() opens {w}");
        assert!(script.contains("whatsNew()"), "an open window's page has window.echo.whatsNew");
    }
}

#[cfg(feature = "transcribe")]
mod aec;
mod ai;
mod app;
mod apps;
mod audio;
#[cfg(any(target_os = "macos", windows))]
mod ax;
mod calendar;
mod calls;
mod config;
#[cfg(any(target_os = "macos", windows))]
mod frames;
#[cfg(feature = "transcribe")]
mod gpu;
mod graph;
mod guard;
mod livenotes;
#[cfg(feature = "transcribe")]
mod live;
mod meeting;
mod minutes;
mod notes;
mod models;
mod paths;
mod vocab;
mod permissions;
mod recorder;
#[cfg(feature = "transcribe")]
mod nemo;
#[cfg(feature = "transcribe")]
mod remote;
#[cfg(feature = "transcribe")]
mod speakers;
#[cfg(feature = "transcribe")]
mod speaking;
mod schedule;
mod signin;
mod watch;
#[cfg(feature = "transcribe")]
mod transcribe;
#[cfg(feature = "tray")]
mod mark;
#[cfg(feature = "tray")]
mod tray;
#[cfg(feature = "tray")]
mod window;
mod update;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "echoctl", version, about = "Records the calls you opt in to and transcribes them on this computer (tray app + CLI)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check OS permissions needed for capture and explain how to grant them
    Doctor {
        /// Emit machine-readable JSON (used by the /echo:doctor skill)
        #[arg(long)]
        json: bool,
        /// Trigger the OS permission prompts that can be triggered programmatically
        #[arg(long)]
        request: bool,
        /// macOS: verify System Audio Recording with a 1.5 s test tap (needs audio playing;
        /// only the peak level is measured, the audio is discarded)
        #[arg(long)]
        probe: bool,
        /// Open the settings page for a permission: microphone | system-audio | accessibility
        #[arg(long)]
        open: Option<String>,
    },
    /// List processes currently using audio (debug aid for call detection)
    Procs,
    /// Transcription models: list (with a hardware-based recommendation), download, use
    Model {
        #[command(subcommand)]
        cmd: ModelCmd,
    },
    /// The echoctl app (tray icon + call watcher): run, start, quit, start at login
    App {
        #[command(subcommand)]
        cmd: Option<AppCmd>,
    },
    /// The data folder (calls + settings): show it, or choose another one
    Home {
        #[command(subcommand)]
        cmd: Option<HomeCmd>,
    },
    /// Calls transcribed but without minutes yet (JSON); always empty for a session (--for): the
    /// app writes minutes itself
    Pending {
        /// The asking session's folder (older panes)
        #[arg(long = "for")]
        session: Option<std::path::PathBuf>,
    },
    /// Take a call for writing minutes in a session; fails while Echo or another session writes them
    Claim {
        call: String,
        /// The claiming session's folder: refused outside the one set by `config --notes-in`
        #[arg(long = "for")]
        session: Option<std::path::PathBuf>,
    },
    /// Claude Code integration: status, or install the Echo plugin
    Claude {
        #[command(subcommand)]
        cmd: Option<ClaudeCmd>,
    },
    /// Show or change settings (<home>/.echo/config.json)
    Config {
        /// Languages spoken in calls, comma-separated ISO codes (e.g. en,uk,ru); "any" clears
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
        /// Glossary for transcription, comma-separated (e.g. "Miro,ServiceNow,IPAM"); "" clears
        #[arg(long, value_delimiter = ',')]
        vocabulary: Option<Vec<String>>,
        /// Calls not on the calendar: "ask" (prompt when they start) or "manual" (only when
        /// you say record)
        #[arg(long, value_parser = ["ask", "manual"])]
        unscheduled: Option<String>,
        /// Keep call audio after transcription (default false: text only)
        #[arg(long)]
        keep_audio: Option<bool>,
        /// Also write the minutes page (<title>-minutes.html) when notes are written
        #[arg(long)]
        minutes_page: Option<bool>,
        /// Write a call's minutes by itself, 15 minutes after the call (or after your last note)
        #[arg(long)]
        auto_minutes: Option<bool>,
        /// Calendar meetings: "ask" (prompt ~1 min before) or "manual" (only when you record)
        #[arg(long, value_parser = ["ask", "manual"])]
        scheduled: Option<String>,
        /// Your name, used in notes ("Me")
        #[arg(long)]
        me_name: Option<String>,
        /// Your email, used to tell colleagues from external attendees
        #[arg(long)]
        me_email: Option<String>,
        /// The folder Echo runs Claude Code in for minutes (with the built-in AI profile its
        /// .claude/settings.json applies), and the only one `claim --for` accepts; "any" clears
        #[arg(long)]
        notes_in: Option<String>,
        /// Save snapshots of shared screens during calls (macOS; needs Screen Recording)
        #[arg(long)]
        capture_frames: Option<bool>,
        /// Record the call app's window text during calls (for speaker names)
        #[arg(long)]
        capture_ui: Option<bool>,
        /// Microsoft 365 app (client) id for the calendar sign-in; "default" = CodeMie
        #[arg(long)]
        graph_client_id: Option<String>,
        /// Entra tenant for the sign-in: common | organizations | <tenant id>
        #[arg(long)]
        graph_tenant: Option<String>,
        /// Model Claude Code uses when Echo writes notes: sonnet | opus | haiku | <model id> |
        /// default (Claude Code's own setting)
        #[arg(long)]
        notes_model: Option<String>,
        /// Model for the live pane's summary and quick questions: haiku | <model id>
        #[arg(long)]
        live_model: Option<String>,
    },
    /// Write minutes and action items for a call with Claude Code (headless, the same
    /// process-call skill a session runs); the running app does it unless --wait
    Notes {
        /// Call folder or name, or "latest"
        call: String,
        /// Run here and wait for it, instead of handing it to the app
        #[arg(long)]
        wait: bool,
        /// Automatic minutes from the terminal pane: always refused now — the app writes them
        /// itself, 15 minutes after the call (`config --auto-minutes`); kept for older panes
        #[arg(long, requires = "session")]
        auto: bool,
        /// The asking session's folder (with --auto)
        #[arg(long = "for")]
        session: Option<std::path::PathBuf>,
    },
    /// AI profiles: which model (and account) writes a call's live notes and minutes
    Ai {
        #[command(subcommand)]
        cmd: Option<AiCmd>,
    },
    /// Live notes during a call: ask a question, pause or resume them, show them (JSON)
    Live {
        #[command(subcommand)]
        cmd: LiveCmd,
    },
    /// "Last time" for a recurring call (JSON): the previous occurrence's open actions and highlights
    Prep {
        #[arg(default_value = "latest")]
        call: String,
    },
    /// Vocabulary (names/terms Whisper should spell right), learned automatically
    Vocab {
        #[command(subcommand)]
        cmd: VocabCmd,
    },
    /// Input bundle for writing a call's notes (JSON): sizes, languages, attendees, gate
    Context {
        #[arg(default_value = "latest")]
        call: String,
    },
    /// Set a call's title (one line, clamped) and rename its folder (inside a minutes run, when it ends)
    Title { call: String, title: String },
    /// Add your own note to the call being recorded ("# Heading" lines shape the minutes)
    Note { text: Vec<String> },
    /// Mark one of a call's action items done or to do (by its id, from `echoctl actions`)
    Action {
        call: String,
        id: String,
        #[arg(value_parser = ["done", "todo"])]
        status: String,
    },
    /// A call's action items as JSON (each with its id); --apply replaces them with a new list,
    /// keeping each known item's status (how the minutes skill writes them)
    Actions {
        call: String,
        #[arg(long)]
        apply: Option<std::path::PathBuf>,
    },
    /// Open a pre-filled email (follow-up) in the default mail app; it is never sent from here
    Compose {
        #[arg(long, value_delimiter = ',')]
        to: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        cc: Vec<String>,
        #[arg(long)]
        subject: String,
        /// Plain-text body
        #[arg(long)]
        body_file: std::path::PathBuf,
    },
    /// Your profile for notes and follow-ups (<home>/.echo/me.md): print its path (creates it)
    Me,
    /// Keep only the shared-screen frames the notes use; delete the others
    Frames {
        call: String,
        /// Frame files to keep (names or paths), comma-separated; "none" deletes all
        #[arg(long, value_delimiter = ',', required = true)]
        keep: Vec<String>,
    },
    /// Write the call's minutes as one self-contained HTML page next to its notes (<title>-minutes.html)
    Html {
        /// Call folder (path or name under calls/), or "latest"
        #[arg(default_value = "latest")]
        call: String,
        /// Open the page in the default browser afterwards
        #[arg(long)]
        open: bool,
        /// Only if the minutes-page setting is on (what the notes skill runs)
        #[arg(long)]
        auto: bool,
    },
    /// Move a call to .echo/trash (recoverable for 30 days)
    Delete { call: String },
    /// Bring a deleted call back from .echo/trash
    Restore { name: String },
    /// Notes of the last call with this subject (recurring meetings)
    Previous {
        #[arg(long)]
        subject: String,
    },
    /// Optional meeting context (subject, attendees, glossary) for a call
    Meeting {
        #[command(subcommand)]
        cmd: MeetingCmd,
    },
    /// Transcribe a recorded call into transcript.md + segments.json
    #[cfg(feature = "transcribe")]
    Transcribe {
        /// Call folder (path or name under calls/), or "latest"
        #[arg(default_value = "latest")]
        call: String,
        /// Use this model instead of the active one; output gets a .<model> suffix (for A/B)
        #[arg(long)]
        model: Option<String>,
        /// Keep the WAV files afterwards (default: deleted once the transcript is written)
        #[arg(long)]
        keep_audio: bool,
    },
    /// Remote speakers a call's speaker detection found (id, time, first lines); with N=Name
    /// pairs, name them in the transcript (N= clears a name)
    #[cfg(feature = "transcribe")]
    Speakers {
        /// Call folder (path or name under calls/), or "latest"
        call: String,
        /// N=Name, e.g. 2="Daniel Osei"
        names: Vec<String>,
    },
    /// Transcription on another computer (local network): use a server (URL + API key), or let
    /// other computers transcribe on this one
    #[cfg(feature = "transcribe")]
    Server {
        #[command(subcommand)]
        cmd: Option<ServerCmd>,
    },
    /// (internal) Try whisper.cpp on the GPU once: the app runs this as a child (`gpu.rs`)
    #[cfg(feature = "transcribe")]
    #[command(hide = true)]
    GpuCheck,
    /// (internal) The minutes run's guard: Claude Code asks it before every tool call of a run
    /// Echo started (`guard.rs`); exit 2 refuses the call
    #[command(hide = true)]
    RunGuard,
    /// (old name of `app run`)
    #[command(hide = true)]
    Watch {
        #[arg(long, default_value = "teams")]
        app: apps::TargetApp,
        /// Don't show the menu-bar / tray icon
        #[arg(long)]
        no_tray: bool,
    },
    /// Print watcher events as they happen (for Claude Code's Monitor)
    Events {
        #[arg(long)]
        follow: bool,
    },
    /// Opt in to recording the current or next call
    Arm {
        #[arg(long)]
        title: Option<String>,
        /// Pin this call's language(s), e.g. "uk" or "en,uk" (default: the configured list)
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
        /// Meeting context JSON (as for `meeting attach`)
        #[arg(long)]
        meeting: Option<std::path::PathBuf>,
        /// The AI profile for this call's live notes and minutes (`echoctl ai`); default: the one set
        /// with `echoctl ai default`
        #[arg(long = "ai")]
        ai_profile: Option<String>,
    },
    /// Open a meeting's join link in the desktop app and arm recording for it
    Join {
        /// Meeting context JSON with join_url
        meeting: std::path::PathBuf,
    },
    /// Stop the current recording (or cancel an arm)
    Stop,
    /// Watcher / arm / agenda state as JSON
    Status,
    /// Preflight in one step: app installed/running, permissions, model, agenda (JSON)
    Up,
    /// Microsoft 365 calendar (read-only): sign in, sign out, status
    Calendar {
        #[command(subcommand)]
        cmd: Option<CalendarCmd>,
    },
    /// Upcoming meetings the watcher offers to join & record
    Agenda {
        #[command(subcommand)]
        cmd: AgendaCmd,
    },
    /// (dev) Replay a WAV through live transcription into <out>/live.jsonl
    #[cfg(feature = "transcribe")]
    #[command(hide = true)]
    LiveReplay { wav: std::path::PathBuf, out: std::path::PathBuf },
    /// (dev) Snapshot Teams' accessibility tree to .echo/ax/ — to find speaker info
    #[cfg(any(target_os = "macos", windows))]
    #[command(hide = true)]
    AxDump {
        #[arg(long, default_value_t = 20)]
        seconds: u64,
    },
    /// (dev) Capture the Teams call window once, as the frame recorder would, to a PNG/JPEG
    #[cfg(any(target_os = "macos", windows))]
    #[command(hide = true)]
    FrameGrab { out: std::path::PathBuf },
    /// (dev) Teams participants as the speaker reader sees them; live, every token that flips on
    /// them (talk, pause, talk), or what a saved window tree (--tree ui-tree-120s.json) holds
    #[cfg(any(target_os = "macos", windows))]
    #[command(hide = true)]
    AxSpeaking {
        #[arg(long, default_value_t = 60)]
        seconds: u64,
        #[arg(long)]
        tree: Option<std::path::PathBuf>,
        /// Every change anywhere in the call window, not just on participants
        #[arg(long)]
        all: bool,
    },
    /// (dev) Teams' speaking marker learned for a recorded call, and who it names
    #[cfg(feature = "transcribe")]
    #[command(hide = true)]
    Speaking {
        call: String,
    },
    /// (dev) Show what the Teams mute detector sees, for N seconds
    #[cfg(any(target_os = "macos", windows))]
    #[command(hide = true)]
    AxMic {
        #[arg(long, default_value_t = 30)]
        seconds: u64,
    },
    /// (dev) Who an ad-hoc call is with: the Teams call window's title and the chat it matches
    /// (`--name` skips the window, to check the chat match)
    #[cfg(any(target_os = "macos", windows))]
    #[command(hide = true)]
    CallPeople {
        #[arg(long)]
        name: Option<String>,
    },
    /// (dev, macOS) Follow the call window and who shares, as the frame recorder does
    #[cfg(target_os = "macos")]
    #[command(hide = true)]
    AxShare {
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },
    /// (dev) Show a notification the way the app does
    #[command(hide = true)]
    Notify { title: String, body: String },
    /// (dev) Open the Echo window on its own; --selftest checks echo:// + IPC and exits
    #[cfg(feature = "tray")]
    #[command(hide = true)]
    Window {
        #[arg(long)]
        selftest: bool,
        /// Write the page and API responses to a folder (review the UI in a browser)
        #[arg(long)]
        export: Option<std::path::PathBuf>,
    },
    /// (build) Write the app icon: an .iconset folder for `iconutil`, or a Windows <file>.ico
    #[cfg(feature = "tray")]
    #[command(hide = true)]
    DevIcon {
        dir: std::path::PathBuf,
        /// Build the icon from this image (the onetest logo) instead of the drawn mark
        #[arg(long)]
        from: Option<std::path::PathBuf>,
    },
    /// Record the target app's audio and the microphone as two tracks
    Capture {
        /// App to capture
        #[arg(long, default_value = "teams")]
        app: apps::TargetApp,
        /// Stop automatically after N seconds (default: until Ctrl-C)
        #[arg(long)]
        seconds: Option<u64>,
        /// Optional title used in the call folder name
        #[arg(long)]
        title: Option<String>,
        /// Optional meeting context JSON (see `echoctl meeting attach --help`)
        #[arg(long)]
        meeting: Option<std::path::PathBuf>,
        /// Pin this call's language(s), e.g. "uk" or "en,uk"
        #[arg(long, value_delimiter = ',')]
        languages: Option<Vec<String>>,
    },
}

#[derive(Subcommand)]
enum AppCmd {
    /// Run the app in the foreground (what login / `app start` launch)
    Run {
        #[arg(long, default_value = "teams")]
        app: apps::TargetApp,
        /// Don't show the menu-bar / tray icon
        #[arg(long)]
        no_tray: bool,
        /// Launched without a terminal: log to <home>/.echo/app.log
        #[arg(long)]
        background: bool,
    },
    /// Start the app in the background if it isn't running
    Start,
    /// Show the Echo window (calls, notes, transcripts), optionally at one call
    Open {
        /// Call folder name
        #[arg(long)]
        call: Option<String>,
    },
    /// Stop the app (a recording in progress is saved first)
    Quit,
    /// Check GitHub Releases for a newer version and install it (the app restarts)
    Update {
        /// Only report whether an update is available
        #[arg(long)]
        check: bool,
    },
    /// Make `echoctl` available in terminals (macOS: /usr/local/bin link; Windows: user PATH)
    InstallCli,
    /// Start the app at login: on | off (no argument: show)
    Autostart {
        #[arg(value_parser = ["on", "off"])]
        state: Option<String>,
    },
}

#[derive(Subcommand)]
enum AiCmd {
    /// The profiles and the default (JSON); Claude profiles' sign-ins are checked now
    Status {
        id: Option<String>,
    },
    /// The profile new calls use
    Default { id: String },
    /// Live notes during calls (they send the call's text to the call's model)
    Live {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// A model server on this computer with Anthropic's Messages API (Ollama)
    AddLocal {
        id: String,
        /// The model for the minutes (and live notes, unless --live-model)
        #[arg(long)]
        model: String,
        #[arg(long)]
        live_model: Option<String>,
        #[arg(long, default_value = "http://localhost:11434")]
        url: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// A claude.ai account in a Claude Code folder of its own (sign it in from the Echo window, or
    /// with `ai login`)
    AddClaude {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        live_model: Option<String>,
        #[arg(long)]
        notes_model: Option<String>,
        /// Pre-filled on the sign-in page
        #[arg(long)]
        email: Option<String>,
        /// The organisation signs in with single sign-on
        #[arg(long)]
        sso: bool,
    },
    /// A profile's minutes model; how a Claude profile signs in: the email pre-filled, single sign-on
    Set {
        id: String,
        /// The email pre-filled on the sign-in page
        #[arg(long, conflicts_with = "no_email")]
        email: Option<String>,
        /// Pre-fill the recorded account's email instead (or none)
        #[arg(long)]
        no_email: bool,
        /// Sign in with the organisation's single sign-on
        #[arg(long, value_parser = ["on", "off"])]
        sso: Option<String>,
        /// The model that writes this profile's minutes (empty: its default; for `system`,
        /// "default" = Claude Code's own setting)
        #[arg(long)]
        notes_model: Option<String>,
    },
    Remove { id: String },
    /// Sign a Claude profile in, here in this terminal (`claude auth login`; Ctrl-C stops it)
    Login { id: String },
    /// Accept the account a Claude profile is signed in to now (after signing in to another)
    Accept { id: String },
}

#[derive(Subcommand)]
enum LiveCmd {
    /// Ask about a call: --kind missed (the last 5 min) | question (suggest one) | ask (the text)
    Ask {
        call: String,
        #[arg(long, default_value = "ask", value_parser = ["missed", "question", "ask"])]
        kind: String,
        text: Vec<String>,
    },
    Pause { call: String },
    Resume { call: String },
    /// The call's live notes, answers and state (JSON)
    Show {
        #[arg(default_value = "latest")]
        call: String,
    },
    /// (dev) Run one summary (or, with --ask, one question) here and print the result
    #[command(hide = true)]
    Test {
        call: String,
        #[arg(long, value_parser = ["missed", "question", "ask"])]
        ask: Option<String>,
        text: Vec<String>,
    },
}

#[derive(Subcommand)]
enum CalendarCmd {
    /// Sign in through the browser (uses `config --graph-client-id`, default CodeMie)
    Login,
    /// Forget the sign-in
    Logout,
    /// Fetch the meeting chat of a recorded call into <call>/chat.json (also done live)
    Chat { call: String },
    /// (diagnostics) Call start/end events in your recent chats, last N hours
    #[command(hide = true)]
    CallEvents {
        #[arg(long, default_value_t = 12)]
        hours: i64,
    },
    /// List calendar events from today (unfiltered), to check the connection
    Events {
        #[arg(long, default_value_t = 1)]
        days: i64,
    },
}

#[derive(Subcommand)]
enum ClaudeCmd {
    /// Install the Echo plugin into Claude Code (adds its marketplace first)
    Connect {
        /// user = every session (default), project = shared via the repo, local = only you, in one repo
        #[arg(long, default_value = "user", value_parser = ["user", "project", "local"])]
        scope: String,
        /// The repo for project/local scope (default: the current directory)
        #[arg(long)]
        project: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
enum HomeCmd {
    /// Use this folder for calls and settings (created if missing; restarts the app)
    Set { path: std::path::PathBuf },
    /// Pick the folder in a native dialog
    Choose,
}

#[derive(Subcommand)]
enum VocabCmd {
    /// Learn terms from a call (2–50 chars, ≤ 6 words): used again only in calls with its
    /// subject or people, or whose invite names them. --always: in every call
    Add {
        terms: Vec<String>,
        /// The call they come from (default: the call whose notes were written last, < 3 h ago)
        #[arg(long)]
        call: Option<String>,
        /// Use in every call (names, products you always need)
        #[arg(long, conflicts_with = "call")]
        always: bool,
    },
    /// Remove terms (from both lists)
    Remove { terms: Vec<String> },
    /// The always-on terms and the learned ones with where they apply
    List,
}

#[derive(Subcommand)]
enum AgendaCmd {
    /// Refresh today's agenda from the calendar (msgraph skill if installed); quiet no-op without
    Sync {
        #[arg(long)]
        quiet: bool,
    },
    /// Replace the agenda with a JSON array of meetings (file or "-" for stdin)
    Set { file: std::path::PathBuf },
    /// Clear the agenda
    Clear,
}

#[derive(Subcommand)]
enum MeetingCmd {
    /// Attach context to a call (writes <call>/meeting.json). FILE is JSON or "-" for stdin:
    /// {"subject", "start", "end", "organizer": {"name","email"}, "attendees": [{"name","email"}],
    ///  "join_url", "vocabulary": [..], "source"} — every field optional.
    Attach {
        /// Call folder (path or name under calls/), or "latest"
        call: String,
        file: std::path::PathBuf,
    },
}

#[cfg(feature = "transcribe")]
#[derive(Subcommand)]
enum ServerCmd {
    /// The server this computer uses, and whether it serves others
    Status,
    /// Transcribe on this server: http(s)://host:port and its API key (checked before it is kept)
    Use {
        url: String,
        #[arg(long)]
        key: String,
    },
    /// Stop using the server: transcribe on this computer again
    Off,
    /// Let other computers transcribe on this one (on | off); prints the URL and API key
    Share {
        #[arg(value_parser = ["on", "off"])]
        state: String,
        /// Replace the API key (computers using the old one must connect again)
        #[arg(long)]
        new_key: bool,
    },
    /// Serve in the foreground until Ctrl-C (testing)
    #[command(hide = true)]
    Run {
        #[arg(long)]
        port: Option<u16>,
    },
}

#[derive(Subcommand)]
enum ModelCmd {
    /// Show the catalog, what's downloaded/active, and the recommendation for this machine
    List {
        #[arg(long)]
        json: bool,
    },
    /// Download (resumable) and verify a model; becomes active if none is set
    Download { name: String },
    /// Switch the active model
    Use { name: String },
    /// Delete a downloaded model (not the active one)
    Remove {
        name: String,
        /// Also remove the live / language-detection helper model
        #[arg(long)]
        force: bool,
    },
}

/// Name of the "transcription in progress" marker (a no-op name when built without whisper).
#[allow(dead_code)]
fn transcribe_marker() -> &'static str {
    #[cfg(feature = "transcribe")]
    return transcribe::IN_PROGRESS;
    #[cfg(not(feature = "transcribe"))]
    ".transcribing"
}

/// Opened as an app (Finder, Dock, login item, Explorer double-click) rather than typed in a
/// terminal: no arguments, and inside Echo.app / alone in its console window.
fn launched_as_app() -> bool {
    let args = std::env::args().skip(1).filter(|a| !a.starts_with("-psn_")).count();
    if args > 0 {
        return false;
    }
    #[cfg(windows)]
    {
        let mut pids = [0u32; 4];
        // Only our own process attached = Explorer created this console for us.
        return unsafe { windows::Win32::System::Console::GetConsoleProcessList(&mut pids) } == 1;
    }
    #[allow(unreachable_code)]
    app::bundle_path().is_some()
}

fn main() -> Result<()> {
    if launched_as_app() {
        // Opened again (Start menu, a second double-click) while it runs: show its window.
        if watch::running_watcher().is_some() {
            return app::open_window(None);
        }
        return app::run(apps::TargetApp::Teams, true, true);
    }
    match Cli::parse().cmd {
        Cmd::Doctor { json, request, probe, open } => permissions::doctor(json, request, probe, open.as_deref()),
        Cmd::Model { cmd } => match cmd {
            ModelCmd::List { json } => models::list(json),
            ModelCmd::Download { name } => models::download(&name),
            ModelCmd::Use { name } => models::use_model(&name),
            ModelCmd::Remove { name, force } => models::remove(&name, force),
        },
        Cmd::Config { languages, vocabulary, unscheduled, keep_audio, minutes_page, auto_minutes, scheduled, me_name, me_email, notes_in, capture_frames, capture_ui, graph_client_id, graph_tenant, notes_model, live_model } => {
            let mut cfg = config::Config::load()?;
            if let Some(m) = live_model {
                cfg.live_model = Some(m.trim().to_string()).filter(|m| !m.is_empty() && m != config::DEFAULT_LIVE_MODEL && m != "default");
            }
            if let Some(m) = notes_model {
                cfg.notes_model = Some(m.trim().to_string()).filter(|m| !m.is_empty() && m != config::DEFAULT_NOTES_MODEL);
            }
            if let Some(id) = graph_client_id {
                cfg.graph_client_id = (id != "default" && !id.trim().is_empty()).then(|| id.trim().to_string());
            }
            if let Some(t) = graph_tenant {
                cfg.graph_tenant = (t != "default" && !t.trim().is_empty()).then(|| t.trim().to_string());
            }
            if let Some(on) = capture_frames {
                cfg.capture_frames = on;
            }
            if capture_ui.is_some() {
                cfg.capture_ui = capture_ui;
            }
            if let Some(dir) = notes_in {
                cfg.notes_in = match dir.as_str() {
                    "any" | "" => None,
                    d => Some(dunce::canonicalize(d).map_err(|e| anyhow::anyhow!("{d}: {e}"))?),
                };
            }
            if let Some(langs) = languages {
                cfg.languages = langs.into_iter().filter(|l| l != "any").map(|l| l.to_lowercase()).collect();
            }
            if unscheduled.is_some() {
                cfg.unscheduled = unscheduled;
            }
            if let Some(k) = keep_audio {
                cfg.keep_audio = k;
            }
            if let Some(m) = minutes_page {
                cfg.minutes_page = m;
            }
            if let Some(on) = auto_minutes {
                cfg.set_auto_minutes(on);
            }
            if scheduled.is_some() {
                cfg.scheduled = scheduled;
            }
            if me_name.is_some() || me_email.is_some() {
                let mut me = cfg.me.take().unwrap_or_default();
                if let Some(n) = me_name {
                    me.name = n;
                }
                if me_email.is_some() {
                    me.email = me_email;
                }
                cfg.me = Some(me);
            }
            if let Some(words) = vocabulary {
                cfg.vocabulary = words.into_iter().map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect();
            }
            cfg.save()?;
            println!("{}", serde_json::to_string_pretty(&cfg)?);
            Ok(())
        }
        Cmd::RunGuard => std::process::exit(guard::command()),
        #[cfg(feature = "transcribe")]
        Cmd::GpuCheck => gpu::check(),
        #[cfg(feature = "transcribe")]
        Cmd::Server { cmd } => match cmd.unwrap_or(ServerCmd::Status) {
            ServerCmd::Status => remote::print_status(),
            ServerCmd::Use { url, key } => {
                let h = remote::connect(&url, &key)?;
                println!("transcribing on {} ({}, {}, Echo {})", h.name, h.model, h.engine, h.version);
                Ok(())
            }
            ServerCmd::Off => {
                remote::disconnect()?;
                println!("transcribing on this computer");
                Ok(())
            }
            ServerCmd::Share { state, new_key } => {
                let s = remote::set_serving(state == "on", new_key)?;
                if s.on {
                    println!("other computers can transcribe here (while Echo runs):\n  URL      {}\n  API key  {}", remote::serve_url(&s), s.key);
                    println!("audio is sent over the network unencrypted; nothing is kept here");
                } else {
                    println!("not serving other computers");
                }
                Ok(())
            }
            ServerCmd::Run { port } => remote::run_foreground(port),
        },
        #[cfg(feature = "transcribe")]
        Cmd::Transcribe { call, model, keep_audio } => {
            // A run Echo started can't keep audio the user's setting deletes (text only by default).
            if let Some(why) = notes::refused_in_run(false, keep_audio, model.is_some()).filter(|_| notes::headless()) {
                anyhow::bail!(why);
            }
            transcribe::transcribe(&call, model.as_deref(), keep_audio)
        }
        #[cfg(feature = "transcribe")]
        Cmd::Speakers { call, names } => speakers::command(&call, &names),
        Cmd::Vocab { cmd } => {
            let mut cfg = config::Config::load()?;
            match cmd {
                VocabCmd::Add { terms, call, always } => {
                    // A run Echo started learns a call's terms, never the user's own always-on list.
                    if let Some(why) = notes::refused_in_run(always, false, false).filter(|_| notes::headless()) {
                        anyhow::bail!(why);
                    }
                    let msg = vocab::add(&mut cfg, &terms, call.as_deref(), always)?;
                    cfg.save()?;
                    println!("{msg}");
                }
                VocabCmd::Remove { terms } => {
                    vocab::remove(&mut cfg, &terms);
                    cfg.save()?;
                    print!("{}", vocab::list(&cfg));
                }
                VocabCmd::List => print!("{}", vocab::list(&cfg)),
            }
            Ok(())
        }
        Cmd::Context { call } => calls::context(&call),
        Cmd::Title { call, title } => calls::set_title(&call, &title),
        Cmd::Note { text } => calls::note(&text.join(" ")),
        Cmd::Action { call, id, status } => calls::action_command(&call, &id, &status),
        Cmd::Actions { call, apply } => calls::actions_command(&call, apply.as_deref()),
        Cmd::Previous { subject } => calls::previous(&subject),
        Cmd::Delete { call } => calls::delete(&call),
        Cmd::Frames { call, keep } => calls::prune_frames(&call, &keep),
        Cmd::Html { call, open, auto } => minutes::command(&call, open, auto),
        Cmd::Compose { to, cc, subject, body_file } => calls::compose(&to, &cc, &subject, &body_file),
        Cmd::Me => {
            println!("{}", calls::ensure_me_profile()?.display());
            Ok(())
        }
        Cmd::Restore { name } => calls::restore(&name),
        #[cfg(feature = "transcribe")]
        Cmd::LiveReplay { wav, out } => live::replay(&wav, &out),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::AxDump { seconds } => ax::dump(seconds),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::FrameGrab { out } => frames::grab(&out),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::AxMic { seconds } => ax::mic_probe(seconds),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::AxSpeaking { seconds, tree, all } => ax::speaking_probe(seconds, tree.as_deref(), all),
        #[cfg(feature = "transcribe")]
        Cmd::Speaking { call } => speaking::report(&call),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::CallPeople { name } => {
            let name = match name {
                Some(n) => n,
                None => {
                    let title = ax::call_title().ok_or_else(|| anyhow::anyhow!("no Teams call window (or Accessibility not granted)"))?;
                    println!("call window: {title}");
                    ax::call_name(&title).ok_or_else(|| anyhow::anyhow!("the title names nobody"))?
                }
            };
            println!("name: {name} (looks like a person: {})", ax::is_person_name(&name));
            match graph::chat_named(&name)? {
                Some((_, topic, members)) => {
                    println!("chat: {}", topic.as_deref().unwrap_or("(1:1)"));
                    for m in members {
                        println!("  {}{}", m.name, m.email.map(|e| format!(" <{e}>")).unwrap_or_default());
                    }
                }
                None => println!("no recent chat with that name"),
            }
            Ok(())
        }
        #[cfg(target_os = "macos")]
        Cmd::AxShare { seconds } => ax::share_probe(seconds),
        Cmd::Notify { title, body } => {
            app::notify(&title, &body);
            Ok(())
        }
        Cmd::Procs => audio::print_audio_processes(),
        #[cfg(feature = "tray")]
        Cmd::DevIcon { dir, from } => tray::write_iconset(&dir, from.as_deref()),
        #[cfg(feature = "tray")]
        Cmd::Window { selftest, export } => match export {
            Some(dir) => window::export(&dir),
            None => {
                anyhow::ensure!(selftest, "the window opens from the menu-bar icon (use --selftest to check it)");
                window::selftest()
            }
        },
        Cmd::Capture { app, seconds, title, meeting, languages } => {
            let ctx = meeting.as_deref().map(meeting::MeetingContext::read_file).transpose()?;
            recorder::capture(app, seconds, title.as_deref(), ctx, languages)
        }
        Cmd::Meeting { cmd: MeetingCmd::Attach { call, file } } => meeting::attach(&call, &file),
        Cmd::Watch { app, no_tray } => app::run(app, !no_tray, false),
        Cmd::App { cmd } => match cmd {
            None => {
                println!("{}", serde_json::to_string_pretty(&app::info())?);
                Ok(())
            }
            Some(AppCmd::Run { app: target, no_tray, background }) => app::run(target, !no_tray, background),
            Some(AppCmd::Start) => app::start(),
            Some(AppCmd::Open { call }) => app::open_window(call),
            Some(AppCmd::Quit) => app::quit(),
            Some(AppCmd::InstallCli) => app::install_cli(),
            Some(AppCmd::Update { check }) => update::cli(check),
            Some(AppCmd::Autostart { state }) => {
                if let Some(s) = state {
                    app::set_autostart(s == "on")?;
                }
                println!("start at login: {}", if app::autostart_enabled() { "on" } else { "off" });
                Ok(())
            }
        },
        Cmd::Home { cmd } => {
            let dir = match cmd {
                None => {
                    println!("{}", serde_json::to_string_pretty(&app::info())?);
                    return Ok(());
                }
                Some(HomeCmd::Set { path }) => path,
                Some(HomeCmd::Choose) => match app::choose_folder("Folder for Echo calls and notes") {
                    Some(p) => p,
                    None => anyhow::bail!("no folder chosen"),
                },
            };
            // The running app keeps its files in the old folder: restart it on the new one.
            let was_running = watch::running_watcher().is_some();
            if was_running {
                app::quit()?;
            }
            let d = paths::set_home(&dir)?;
            println!("data folder: {}", d.display());
            if was_running {
                app::start()?;
            }
            Ok(())
        }
        Cmd::Calendar { cmd } => match cmd {
            None => {
                let (client_id, tenant) = graph::app_registration();
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "signed_in": graph::account(),
                    "client_id": client_id,
                    "default_client": client_id == graph::DEFAULT_CLIENT_ID,
                    "tenant": tenant,
                    "agenda_meetings": watch::agenda().len(),
                }))?);
                Ok(())
            }
            Some(CalendarCmd::Login) => {
                let who = graph::login()?;
                println!("signed in as {who}");
                calendar::sync(false)
            }
            Some(CalendarCmd::Chat { call }) => watch::fetch_call_chat(&call),
            Some(CalendarCmd::CallEvents { hours }) => {
                for (chat, ev, t) in graph::recent_call_events(chrono::Utc::now() - chrono::Duration::hours(hours))? {
                    println!("{}  {ev:<8} {chat}", t.with_timezone(&chrono::Local).format("%H:%M:%S"));
                }
                Ok(())
            }
            Some(CalendarCmd::Events { days }) => {
                let today = chrono::Local::now().date_naive();
                let after = today.checked_add_days(chrono::Days::new(days.clamp(1, 14) as u64));
                let (Some(from), Some(to)) = (calendar::local_midnight(today), after.and_then(calendar::local_midnight)) else {
                    anyhow::bail!("can't tell where today starts")
                };
                let events = graph::calendar_view(from, to)?;
                for e in &events {
                    let online = e["onlineMeeting"]["joinUrl"].as_str().is_some();
                    println!(
                        "{}  {}{}  ({} attendees{})",
                        e["start"]["dateTime"].as_str().unwrap_or("?").get(..16).unwrap_or("?"),
                        e["subject"].as_str().unwrap_or("(no subject)"),
                        if e["isCancelled"] == true { " [cancelled]" } else { "" },
                        e["attendees"].as_array().map(|a| a.len()).unwrap_or(0),
                        if online { ", Teams" } else { "" }
                    );
                }
                println!("{} event(s); times in UTC", events.len());
                Ok(())
            }
            Some(CalendarCmd::Logout) => {
                graph::logout()?;
                println!("signed out of Microsoft 365");
                Ok(())
            }
        },
        Cmd::Notes { call, wait, auto, session: _ } => {
            if auto {
                notes::request_auto(&call)
            } else {
                notes::request(&call, wait)
            }
        }
        Cmd::Ai { cmd } => match cmd.unwrap_or(AiCmd::Status { id: None }) {
            AiCmd::Status { id } => ai::status(id.as_deref()),
            AiCmd::Default { id } => ai::set_default(&id),
            AiCmd::Live { state } => ai::set_live(state == "on"),
            AiCmd::AddLocal { id, model, live_model, url, token, name } => ai::add_local(&id, name, &model, live_model, &url, token),
            AiCmd::AddClaude { id, name, live_model, notes_model, email, sso } => ai::add_claude(&id, name, live_model, notes_model, email, sso),
            AiCmd::Set { id, email, no_email, sso, notes_model } => {
                anyhow::ensure!(notes_model.is_some() || email.is_some() || no_email || sso.is_some(), "nothing to set: --notes-model, --email, --no-email or --sso");
                if let Some(m) = notes_model {
                    let (name, now) = ai::set_notes_model(&id, &m)?;
                    println!("{name}: minutes model {now}");
                }
                if email.is_some() || no_email || sso.is_some() {
                    ai::set_sign_in(&id, email, no_email, sso.map(|s| s == "on"))?;
                }
                Ok(())
            }
            AiCmd::Remove { id } => ai::remove(&id),
            AiCmd::Login { id } => ai::login_here(&id),
            AiCmd::Accept { id } => {
                println!("{id} now expects {}", ai::accept_account(&id, None, &|| false)?);
                Ok(())
            }
        },
        Cmd::Live { cmd } => match cmd {
            LiveCmd::Ask { call, kind, text } => livenotes::request(&call, "ask", livenotes::AskKind::parse(&kind), &text.join(" ")),
            LiveCmd::Test { call, ask, text } => {
                let dir = calls::resolve_call(&call)?;
                livenotes::test_once(&dir, ask.as_deref().and_then(livenotes::AskKind::parse).map(|k| (k, text.join(" "))))
            }
            LiveCmd::Pause { call } => livenotes::request(&call, "pause", None, ""),
            LiveCmd::Resume { call } => livenotes::request(&call, "resume", None, ""),
            LiveCmd::Show { call } => {
                let dir = calls::resolve_call(&call)?;
                let state: serde_json::Value = std::fs::read(dir.join(livenotes::STATE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
                println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                    "profile": ai::call_profile_id(&dir).ok(),
                    "state": state,
                    "notes": livenotes::read_notes(&dir),
                    "answers": livenotes::read_answers(&dir),
                }))?);
                Ok(())
            }
        },
        Cmd::Prep { call } => {
            println!("{}", serde_json::to_string_pretty(&calls::prep(&calls::resolve_call(&call)?))?);
            Ok(())
        }
        Cmd::Pending { session } => calls::pending(session.as_deref()),
        Cmd::Claim { call, session } => calls::claim(&call, session.as_deref()),
        Cmd::Claude { cmd } => match cmd {
            None => {
                println!("{}", serde_json::to_string_pretty(&app::claude_status())?);
                Ok(())
            }
            Some(ClaudeCmd::Connect { scope, project }) => app::connect_claude(&scope, project.as_deref()),
        },
        Cmd::Events { follow } => {
            anyhow::ensure!(follow, "only `events --follow` is supported");
            watch::follow_events()
        }
        Cmd::Arm { title, languages, meeting, ai_profile } => {
            // Typed by hand: a misspelt profile is refused here (arming never checks it itself).
            if let Some(id) = &ai_profile {
                ai::Settings::load()?.get(id)?;
            }
            let ctx = meeting.as_deref().map(meeting::MeetingContext::read_file).transpose()?;
            watch::arm(title, ctx, languages, ai_profile)
        }
        Cmd::Join { meeting } => watch::join(meeting::MeetingContext::read_file(&meeting)?),
        Cmd::Stop => watch::stop(),
        Cmd::Status => watch::status(),
        Cmd::Up => {
            let installed = app::installed_app().is_some() || cfg!(windows);
            if watch::running_watcher().is_none() && paths::home_chosen() {
                let _ = app::start(); // quiet best effort; reported below
            }
            let running = watch::running_watcher().map(|w| w.pid);
            // Permissions as the app sees them (they belong to the app, not this terminal).
            let doctor = match app::app_health(false, false) {
                Ok(Some(r)) => r,
                _ => serde_json::to_value(permissions::collect(false, false)?)?,
            };
            let failing: Vec<String> = doctor["checks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| c["required"] == true && c["status"] != "granted")
                .filter_map(|c| c["id"].as_str().map(str::to_owned))
                .collect();
            calendar::sync(true)?;
            let cfg = config::Config::load()?;
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "home": paths::home(),
                "home_chosen": paths::home_chosen(),
                "app_installed": installed,
                "app_pid": running,
                "ready": doctor["ready"] == true,
                "failing": failing,
                "model": cfg.model,
                "agenda_meetings": watch::agenda().len(),
                "next": if !paths::home_chosen() || cfg.model.is_none() { "run the doctor skill" }
                        else if running.is_none() { "the app is not running: `echoctl app start` (see app.log if it fails)" }
                        else if doctor["ready"] != true { "run the doctor skill (permissions)" }
                        else { "nothing — Echo is running" },
            }))?);
            Ok(())
        }
        Cmd::Agenda { cmd } => match cmd {
            AgendaCmd::Set { file } => {
                let bytes = if file.as_os_str() == "-" {
                    let mut b = vec![];
                    std::io::Read::read_to_end(&mut std::io::stdin(), &mut b)?;
                    b
                } else {
                    std::fs::read(&file)?
                };
                watch::set_agenda(serde_json::from_slice(&bytes)?)
            }
            AgendaCmd::Clear => watch::set_agenda(vec![]),
            AgendaCmd::Sync { quiet } => calendar::sync(quiet),
        },
    }
}

#[cfg(feature = "transcribe")]
mod aec;
#[cfg(feature = "brain")]
mod brain;
mod ai;
mod app;
mod apps;
mod attention;
mod audio;
#[cfg(any(target_os = "macos", windows))]
mod ax;
mod calendar;
mod calls;
mod config;
mod deliver;
#[cfg(any(target_os = "macos", windows))]
mod frames;
#[cfg(feature = "transcribe")]
mod gpu;
mod graph;
mod grants;
mod guard;
mod ledger;
mod livenotes;
#[cfg(feature = "transcribe")]
mod live;
mod meeting;
mod minutes;
mod notes;
mod models;
mod paths;
mod plugins;
mod projects;
mod people;
mod projectsync;
mod tasks;
mod teams_transcript;
mod view;
mod vocab;
mod permissions;
mod recorder;
#[cfg(feature = "transcribe")]
mod nemo;
#[cfg(feature = "transcribe")]
mod remote;
#[cfg(feature = "js")]
mod claudejob;
#[cfg(feature = "js")]
mod jshost;
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
mod terminal;
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
    /// What waits for your decision (the window's Needs you), and what was done this week (JSON)
    Attention,
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
        /// The claiming Claude Code session's id (required: an older plugin without it can't claim)
        #[arg(long = "session")]
        session_id: Option<String>,
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
        /// Model for live notes and quick questions during a call: haiku | <model id>
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
    /// The Brain: cross-call intelligence over calls (later channels, mail, folders) (JSON)
    #[cfg(feature = "brain")]
    Brain {
        #[command(subcommand)]
        cmd: BrainCmd,
    },
    /// Projects: which calls belong (rules, pins), stage, glossary, people (JSON)
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Clients: the organisations projects are for, with their mail domains (JSON)
    Client {
        #[command(subcommand)]
        cmd: ClientCmd,
    },
    /// Organisations: whose people are whose (accounts — clients — and the others, a thread's
    /// counterparts), with their mail domains (JSON)
    Org {
        #[command(subcommand)]
        cmd: OrgCmd,
    },
    /// Echo plugins from marketplaces: list, install, update, remove, settings
    Plugin {
        #[command(subcommand)]
        cmd: PluginCmd,
    },
    /// Where Echo sends things to you: Teams (your chat with yourself), mail, and the ways plugins add
    Deliver {
        #[command(subcommand)]
        cmd: DeliverCmd,
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
        /// Name the voices from Teams' speaking ring again (names set by hand stay)
        #[arg(long)]
        relabel: bool,
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
    /// (internal) Echo's Claude terminals ask it before each prompt: what the window shows (`view.rs`)
    #[command(hide = true)]
    ViewHook,
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
    /// Stop the current recording (or, when nothing records, cancel the pending arm)
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
    /// Make a call of a meeting nobody recorded, from its Teams transcript (and its chat): the event's
    /// Graph id, or words of its subject (the latest such meeting on --date, else in the last 14 days)
    Import {
        event: String,
        /// The meeting's day (YYYY-MM-DD)
        #[arg(long)]
        date: Option<chrono::NaiveDate>,
    },
    /// (diagnostics) Which ended Teams meetings of the last N days with no call have a transcript
    #[command(hide = true)]
    Transcripts {
        #[arg(long, default_value_t = 3)]
        days: i64,
    },
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
        /// project = Echo's data folder, where Echo's sessions run (default); user = every Claude Code session;
        /// local = one folder, only you
        #[arg(long, default_value = "project", value_parser = ["user", "project", "local"])]
        scope: String,
        /// The folder for project/local scope (default: Echo's data folder)
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
enum ProjectCmd {
    /// Every project with its client, stage and call count
    List,
    /// One project, with its calls and why each belongs
    Show { id: String },
    /// A new project (stage active unless --stage)
    Create {
        name: String,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        stage: Option<String>,
    },
    /// Change a project's fields
    Set {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// A client id; "" for none
        #[arg(long)]
        client: Option<String>,
        /// lead, proposal, negotiation, kickoff, active, on_hold, wrap_up, done, lost, cancelled
        #[arg(long)]
        stage: Option<String>,
        /// Why it moved (kept with the stage change)
        #[arg(long, requires = "stage")]
        reason: Option<String>,
        /// YYYY-MM-DD: older calls don't belong; "" for none
        #[arg(long)]
        since: Option<String>,
        /// YYYY-MM-DD: its documents, posts and mail dated before it are history in the Brain; "" for none
        #[arg(long)]
        history_before: Option<String>,
        #[arg(long)]
        color: Option<String>,
        /// What Claude should know about it
        #[arg(long)]
        notes: Option<String>,
        /// The AI profile of its jobs; "" for the default
        #[arg(long)]
        profile: Option<String>,
        /// The model of its jobs; "" for the profile's minutes model
        #[arg(long)]
        model: Option<String>,
        /// The brief, from a file ("-" for stdin)
        #[arg(long)]
        brief: Option<std::path::PathBuf>,
        /// A symbol from the window's set (rocket, database, shield…); "" for its identicon
        #[arg(long)]
        icon: Option<String>,
        /// A logo image (PNG, JPEG or WebP, ≤ 512 KB); "" removes it
        #[arg(long)]
        logo: Option<std::path::PathBuf>,
    },
    /// Add a rule (one of --series, --people, --chat, --subject) or remove one by number (--remove)
    Rule {
        id: String,
        /// Every occurrence of this recurring meeting (its subject)
        #[arg(long)]
        series: Option<String>,
        /// Calls where half the other people are from these domains (none given: the client's)
        #[arg(long, num_args = 0.., value_delimiter = ',')]
        people: Option<Vec<String>>,
        /// Calls whose Teams chat is this chat id
        #[arg(long)]
        chat: Option<String>,
        /// Calls whose subject contains these words
        #[arg(long)]
        subject: Option<String>,
        /// Remove the rule with this number (1 = the first, as `show` lists them)
        #[arg(long)]
        remove: Option<usize>,
    },
    /// A source the project reads: --mail <domain or address>, --channel <team id>|<channel id> (--name
    /// to show), --folder <path> (--exclude <glob>…: what in it not to read) into the Brain; --chat <chat
    /// id> (19:…) a meeting chat its work reads between calls; or --remove <kind:id> as `show` lists them
    #[cfg(feature = "brain")]
    Source {
        id: String,
        #[arg(long)]
        mail: Option<String>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long)]
        folder: Option<std::path::PathBuf>,
        #[arg(long)]
        chat: Option<String>,
        #[arg(long)]
        exclude: Vec<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        remove: Option<String>,
    },
    /// Put a call in the project by hand
    Include { id: String, call: String },
    /// Take a call out of the project (the rules won't put it back)
    Exclude { id: String, call: String },
    /// Leave a call to the rules again
    Reset { id: String, call: String },
    /// Add or remove glossary terms (they hint transcription on its calls)
    Term {
        id: String,
        terms: Vec<String>,
        /// What they are: system, term, acronym, milestone, deliverable…
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        remove: bool,
    },
    /// Add or remove a person (email, or a name)
    Person {
        id: String,
        name: String,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        remove: bool,
    },
    /// The projects a call belongs to, and the ones it may belong to
    Of { call: String },
    /// Its people and organisations: who the calls show (calls, talk share, tasks), what you set
    People {
        id: String,
        /// Ask Microsoft 365 (the People API) about them first: title, department, company, office, phone
        #[arg(long)]
        refresh: bool,
    },
    /// Open tasks across open projects by who has to move (I owe, owed to me, waiting, nobody named), and asks by person
    Tasks,
    /// Projects Echo would make from calls in none (last 90 days); --accept, --merge or --reject one
    Found {
        /// Make this one (its key)
        #[arg(long, conflicts_with_all = ["merge", "reject"])]
        accept: Option<String>,
        /// Its name (with --accept)
        #[arg(long, requires = "accept")]
        name: Option<String>,
        /// Its client: an id, "new" for the one it proposes, "" for none (with --accept)
        #[arg(long, requires = "accept")]
        client: Option<String>,
        /// Add this one's rules to a project (with --into)
        #[arg(long, requires = "into", conflicts_with = "reject")]
        merge: Option<String>,
        #[arg(long)]
        into: Option<String>,
        /// Never suggest this one again
        #[arg(long)]
        reject: Option<String>,
    },
    /// Move a project to the trash (its calls are untouched)
    Delete { id: String },
    /// The project's work ledger: stories, tasks, decisions, risks, questions from its calls
    Ledger {
        id: String,
        #[command(subcommand)]
        cmd: LedgerCmd,
    },
}

#[derive(Subcommand)]
enum LedgerCmd {
    /// Bring the evidence up to date with the project's calls (--limit: only the newest N); --chats also
    /// reads its meeting chats between calls, --mail its ✉ Mail sources (Microsoft 365)
    Sync {
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        chats: bool,
        #[arg(long)]
        mail: bool,
    },
    /// Mails as evidence (Graph message ids): one item a mail, read by the next sync like a call;
    /// --conversation adds the rest of each one's conversation
    Mail {
        #[arg(required = true)]
        ids: Vec<String>,
        #[arg(long)]
        conversation: bool,
    },
    /// Evidence not processed yet, oldest first
    Pending,
    /// The live tree as the extractor reads it
    Context,
    /// Apply proposals (a JSON array file) for one evidence item
    Apply {
        #[arg(long)]
        evidence: String,
        #[arg(long)]
        proposals: std::path::PathBuf,
    },
    /// Mark an evidence item processed (also when it changed nothing)
    Done {
        #[arg(long)]
        evidence: String,
        #[arg(long, default_value = "")]
        note: String,
    },
    /// The claims a verifier must judge (--weak: those judged weak too)
    VerifyPrep {
        #[arg(long)]
        weak: bool,
        #[arg(long)]
        out: Option<std::path::PathBuf>,
    },
    /// Apply verdicts (a JSON array file of {mid, verdict, reason})
    VerifyApply {
        #[arg(long)]
        file: std::path::PathBuf,
    },
    /// The people pass: the thread's people and organisations, these calls' files, what was proposed
    PeopleContext {
        /// Evidence ids (comma-separated)
        #[arg(long, value_delimiter = ',')]
        items: Vec<String>,
    },
    /// Propose changes about people for one evidence item (a JSON array file of {who, field, value, quote, to?, note?})
    PeopleApply {
        #[arg(long)]
        evidence: String,
        #[arg(long)]
        file: std::path::PathBuf,
    },
    /// The whole tree (JSON)
    Tree,
    /// Sync now: the calls into evidence, then Claude reads them in (extract, then verify, then people)
    Run,
    /// The last sync's state
    Status,
    /// An edit of yours (JSON: {op: edit|ac_add|ac_set|link|unlink|merge|delete|accept|reject|undo|create, …})
    Edit { json: String },
}

#[derive(Subcommand)]
enum ClientCmd {
    List,
    /// Its people and organisations across its threads: who the calls show, what you set
    People { id: String },
    Add {
        name: String,
        /// Mail domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Vec<String>,
        /// The user's own company
        #[arg(long)]
        internal: bool,
    },
    Set {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// Replaces the domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Option<Vec<String>>,
        #[arg(long)]
        internal: Option<bool>,
        /// A logo image (PNG, JPEG or WebP, ≤ 512 KB); "" removes it
        #[arg(long)]
        logo: Option<std::path::PathBuf>,
    },
    /// Remove a client without projects
    Remove { id: String },
}

#[derive(Subcommand)]
enum PluginCmd {
    /// Installed plugins, what the marketplaces offer, and the marketplaces (JSON)
    List,
    /// Install a plugin (shows what it uses; --yes agrees)
    Install {
        id: String,
        /// The marketplace, when more than one offers it
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Update a plugin (or all) to its marketplace's newer version; more permissions need --yes
    Update {
        id: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Remove a plugin (its settings and data stay unless --delete-data)
    Remove {
        id: String,
        #[arg(long)]
        delete_data: bool,
    },
    Enable { id: String },
    Disable { id: String },
    /// Show (no value) or set a plugin's setting
    Set { id: String, key: String, value: Option<String> },
    /// Run a plugin's command (its host half's `ctx.command`) with JSON arguments; prints its JSON answer
    Call {
        id: String,
        command: String,
        /// JSON arguments (default {})
        args: Option<String>,
    },
    /// Marketplaces: list | add <owner/repo | git URL | folder> [--name] | remove <name> | update [<name>]
    Marketplace {
        #[command(subcommand)]
        cmd: MarketCmd,
    },
}

#[derive(Subcommand)]
enum DeliverCmd {
    /// What can be sent now (JSON)
    Status,
    /// Send a test message: teams | mail | a way a plugin adds (telegram…)
    Test { kind: String },
    /// Send a Markdown file to yourself: teams | mail | a way a plugin adds
    Send {
        kind: String,
        #[arg(long)]
        title: String,
        #[arg(long)]
        file: std::path::PathBuf,
        /// One line on where it comes from, under the title
        #[arg(long)]
        about: Option<String>,
        /// A model wrote it (its name): the message says so
        #[arg(long)]
        ai: Option<String>,
        /// Mail to these addresses instead of you (comma-separated)
        #[arg(long, default_value = "")]
        to: String,
    },
}

#[derive(Subcommand)]
enum MarketCmd {
    List,
    Add {
        source: String,
        #[arg(long)]
        name: Option<String>,
    },
    Remove { name: String },
    /// Leave a marketplace out of Discover (the built-in one can't be removed, only hidden)
    Hide { name: String },
    Show { name: String },
    /// Fetch again (all, or one)
    Update { name: Option<String> },
}

#[derive(Subcommand)]
enum OrgCmd {
    /// Every organisation (`account: false` = not a client)
    List,
    /// A new organisation (not an account unless --account); --thread makes it that thread's counterpart
    Add {
        name: String,
        /// Mail domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Vec<String>,
        #[arg(long)]
        thread: Option<String>,
        /// A client: threads may sit under it
        #[arg(long)]
        account: bool,
    },
    Set {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// Replaces the domains (comma-separated)
        #[arg(long, value_delimiter = ',')]
        domains: Option<Vec<String>>,
        /// Replaces the other names (comma-separated)
        #[arg(long, value_delimiter = ',')]
        aliases: Option<Vec<String>>,
        /// The organisation it is part of (its id; "" for none)
        #[arg(long)]
        parent: Option<String>,
        /// A client or not (one with threads under it stays one)
        #[arg(long)]
        account: Option<bool>,
        /// A logo image (PNG, JPEG or WebP, ≤ 512 KB); "" removes it
        #[arg(long)]
        logo: Option<std::path::PathBuf>,
        /// Make it this thread's counterpart
        #[arg(long)]
        thread: Option<String>,
        /// No longer this thread's counterpart
        #[arg(long)]
        unlink: Option<String>,
    },
    /// Remove an organisation (an account only without threads); threads stop listing it
    Remove { id: String },
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
    /// Transcribe on this server: http(s)://host:port and its API key (checked before it is kept);
    /// --openai for an OpenAI-compatible server (Speaches, vLLM, LocalAI, parakeet-api…), --model to pick its model
    Use {
        url: String,
        #[arg(long, default_value = "")]
        key: String,
        #[arg(long)]
        openai: bool,
        #[arg(long)]
        model: Option<String>,
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
        Cmd::Attention => {
            print_json(&attention::peek());
            Ok(())
        }
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
        Cmd::ViewHook => std::process::exit(view::hook()),
        #[cfg(feature = "transcribe")]
        Cmd::GpuCheck => gpu::check(),
        #[cfg(feature = "transcribe")]
        Cmd::Server { cmd } => match cmd.unwrap_or(ServerCmd::Status) {
            ServerCmd::Status => remote::print_status(),
            ServerCmd::Use { url, key, openai: true, model } => {
                let c = remote::connect_openai(&url, &key, model.as_deref())?;
                println!(
                    "transcribing on {url} with {} (OpenAI-compatible; test answer in {:.1}s; word timestamps: {})",
                    c.model,
                    c.seconds,
                    if c.words { "yes" } else { "no — speaker turns are split by sentence only" }
                );
                Ok(())
            }
            ServerCmd::Use { url, key, .. } => {
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
        Cmd::Speakers { call, names, relabel } => speakers::command(&call, &names, relabel),
        Cmd::Vocab { cmd } => {
            let mut cfg = config::Config::load()?;
            // The old learned list moves into the taxonomy once (never from inside a minutes run).
            #[cfg(feature = "brain")]
            if !notes::headless() {
                match vocab::migrate_learned(&mut cfg) {
                    Ok(0) => {}
                    Ok(n) => eprintln!("moved {n} learned terms into the Brain's taxonomy (the old list is kept in .echo/learned-before-taxonomy.json)"),
                    Err(e) => eprintln!("warning: moving the learned terms: {e:#}"),
                }
            }
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
        #[cfg(feature = "brain")]
        Cmd::Brain { cmd } => brain_command(cmd),
        Cmd::Project { cmd } => project_command(cmd),
        Cmd::Client { cmd } => client_command(cmd),
        Cmd::Org { cmd } => org_command(cmd),
        Cmd::Plugin { cmd } => plugin_command(cmd),
        Cmd::Deliver { cmd } => deliver_command(cmd),
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
            Some(CalendarCmd::Transcripts { days }) => {
                let now = chrono::Utc::now();
                teams_transcript::check(&graph::calendar_view(now - chrono::Duration::days(days), now)?, false)?;
                print_json(&serde_json::Value::Object(teams_transcript::offers()));
                Ok(())
            }
            Some(CalendarCmd::Import { event, date }) => {
                let dir = teams_transcript::import(&teams_transcript::find_event(&event, date)?)?;
                println!("{}", dir.display());
                Ok(())
            }
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
        Cmd::Claim { call, session, session_id } => calls::claim(&call, session.as_deref(), session_id.as_deref()),
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

fn print_json(v: &serde_json::Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn project_command(cmd: ProjectCmd) -> Result<()> {
    use projects::{Member, Rule, Stage, Term};
    if let ProjectCmd::Ledger { id, cmd } = cmd {
        return ledger_command(&id, cmd);
    }
    // A run Echo started works on its own call (or project) only; projects are the user's. A read-only
    // routine (the user's own prompt) may read them.
    let reads = matches!(cmd, ProjectCmd::List | ProjectCmd::Show { .. } | ProjectCmd::Of { .. } | ProjectCmd::Tasks);
    if notes::headless() && !(reads && routine_reads()) {
        anyhow::bail!("projects can't be changed or listed from a run Echo started");
    }
    let empty = |s: String| if s.trim().is_empty() { None } else { Some(s) };
    let show = |p: &projects::Project| -> Result<()> {
        print_json(&projects::view(p, true)?);
        Ok(())
    };
    match cmd {
        ProjectCmd::List => print_json(&projects::overview()?),
        ProjectCmd::Show { id } => show(&projects::load(&id)?)?,
        ProjectCmd::Create { name, client, stage } => {
            let stage = stage.as_deref().map(Stage::parse).transpose()?;
            show(&projects::create(&name, client.as_deref(), stage)?)?
        }
        ProjectCmd::Set { id, name, client, stage, reason, since, history_before, color, notes, profile, model, brief, icon, logo } => {
            if let Some(f) = logo {
                projects::set_logo(&id, (!f.as_os_str().is_empty()).then_some(f.as_path()))?;
            }
            if let Some(f) = brief {
                let text = if f.as_os_str() == "-" { std::io::read_to_string(std::io::stdin())? } else { std::fs::read_to_string(&f)? };
                projects::set_brief(&id, &text)?;
            }
            if let Some(st) = stage {
                projects::set_stage(&id, Stage::parse(&st)?, reason.as_deref())?;
            }
            let p = projects::update(&id, |p| {
                if let Some(n) = name {
                    p.name = n.trim().to_string();
                }
                if let Some(c) = client {
                    p.client = empty(c);
                }
                if let Some(s) = since {
                    p.since = empty(s);
                }
                if let Some(s) = history_before {
                    p.history_before = empty(s);
                }
                if let Some(c) = color {
                    p.color = c;
                }
                if let Some(n) = notes {
                    p.notes = n;
                }
                if let Some(a) = profile {
                    p.ai_profile = empty(a);
                }
                if let Some(m) = model {
                    p.model = empty(m);
                }
                if let Some(i) = icon {
                    p.icon = empty(i);
                }
                Ok(())
            })?;
            show(&p)?
        }
        #[cfg(feature = "brain")]
        ProjectCmd::Source { id, mail, channel, folder, chat, exclude, name, remove } => {
            let add = match (mail, channel, folder) {
                _ if chat.is_some() => {
                    let c = chat.clone().unwrap_or_default().trim().to_string();
                    anyhow::ensure!(c.starts_with("19:") && !c.contains(char::is_whitespace), "--chat takes a Teams chat id (19:…@thread.v2)");
                    Some(("teams_chat".to_string(), c))
                }
                (Some(m), _, _) => Some(("mail".to_string(), brain::sources::mail_source(&m)?)),
                (None, Some(c), _) if c.split_once('|').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty()) => Some(("teams_channel".to_string(), c)),
                (None, Some(_), _) => anyhow::bail!("--channel takes <team id>|<channel id>"),
                (None, None, Some(f)) => {
                    let f = dunce::canonicalize(&f).map_err(|e| anyhow::anyhow!("{}: {e}", f.display()))?;
                    anyhow::ensure!(f.is_dir(), "{} isn't a folder", f.display());
                    Some(("folder".to_string(), f.to_string_lossy().to_string()))
                }
                _ => None,
            };
            let p = projects::update(&id, |p| {
                if let Some(r) = &remove {
                    let before = p.sources.len();
                    p.sources.retain(|s| format!("{}:{}", s.kind, s.id) != *r);
                    if p.sources.len() == before {
                        anyhow::bail!("no source {r}");
                    }
                }
                if let Some((kind, sid)) = &add {
                    match p.sources.iter_mut().find(|s| &s.kind == kind && &s.id == sid) {
                        Some(s) if !exclude.is_empty() => s.exclude = exclude.clone(),
                        Some(_) => {}
                        None => p.sources.push(projects::Source { kind: kind.clone(), id: sid.clone(), name: name.clone(), exclude: exclude.clone() }),
                    }
                }
                Ok(())
            })?;
            print_json(&serde_json::json!(p.sources));
        }
        ProjectCmd::Rule { id, series, people, chat, subject, remove } => {
            let p = projects::update(&id, |p| {
                if let Some(n) = remove {
                    if n == 0 || n > p.rules.len() {
                        anyhow::bail!("no rule {n} (it has {})", p.rules.len());
                    }
                    p.rules.remove(n - 1);
                }
                let mut add = vec![];
                if let Some(s) = series {
                    add.push(Rule::Series { subject: s, series_id: None });
                }
                if let Some(d) = people {
                    add.push(Rule::People { domains: d });
                }
                if let Some(c) = chat {
                    add.push(Rule::Chat { id: c.trim().to_string(), name: None });
                }
                if let Some(s) = subject {
                    add.push(Rule::Subject { contains: s });
                }
                for r in add {
                    if !p.rules.contains(&r) {
                        p.rules.push(r);
                    }
                }
                Ok(())
            })?;
            show(&p)?
        }
        ProjectCmd::Include { id, call } => show(&projects::set_membership(&id, &calls::resolve_call(&call)?, "include")?)?,
        ProjectCmd::Exclude { id, call } => show(&projects::set_membership(&id, &calls::resolve_call(&call)?, "exclude")?)?,
        ProjectCmd::Reset { id, call } => show(&projects::set_membership(&id, &calls::resolve_call(&call)?, "rules")?)?,
        ProjectCmd::Term { id, terms, kind, remove } => {
            let p = projects::update(&id, |p| {
                for t in terms.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
                    p.terms.retain(|x| !x.term.eq_ignore_ascii_case(t));
                    if !remove {
                        if !config::valid_term(t) {
                            anyhow::bail!("{t:?}: a term is 2–50 characters, at most 6 words");
                        }
                        p.terms.push(Term { term: t.to_string(), kind: kind.clone(), note: None });
                    }
                }
                Ok(())
            })?;
            show(&p)?
        }
        ProjectCmd::Person { id, name, email, role, remove } => {
            let p = projects::update(&id, |p| {
                let key = email.clone().unwrap_or_else(|| name.clone()).to_lowercase();
                p.people.retain(|m| m.email.clone().unwrap_or_else(|| m.name.clone()).to_lowercase() != key);
                if !remove {
                    p.people.push(Member { name: name.trim().to_string(), email: email.clone(), role: role.clone(), ..Default::default() });
                }
                Ok(())
            })?;
            show(&p)?
        }
        ProjectCmd::People { id, refresh } => {
            let today = chrono::Local::now().date_naive();
            if refresh {
                let v = people::of_thread(&id, today)?;
                let who: Vec<(String, String)> = v["people"].as_array().into_iter().flatten().filter_map(|p| Some((p["name"].as_str()?.to_string(), p["email"].as_str()?.to_string()))).collect();
                eprintln!("asked Microsoft 365 about {} people", people::refresh_m365(&who)?);
            }
            print_json(&people::of_thread(&id, today)?)
        }
        ProjectCmd::Tasks => {
            let now = chrono::Utc::now();
            print_json(&tasks::across(&watch::agenda(), chrono::Local::now().date_naive(), &now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))?);
        }
        ProjectCmd::Of { call } => {
            let dir = calls::resolve_call(&call)?;
            let of: Vec<serde_json::Value> = projects::of_call(&dir)?.into_iter().map(|(p, w)| serde_json::json!({"id": p.id, "name": p.name, "why": w})).collect();
            print_json(&serde_json::json!({"projects": of, "suggested": projects::suggest(&dir)?}));
        }
        ProjectCmd::Found { accept, name, client, merge, into, reject } => {
            if let Some(k) = accept {
                show(&projects::found_accept(&k, name.as_deref(), client.as_deref())?)?
            } else if let (Some(k), Some(into)) = (merge, into) {
                show(&projects::found_merge(&k, &into)?)?
            } else if let Some(k) = reject {
                projects::found_reject(&k)?;
                println!("{k}: not suggested again");
            } else {
                print_json(&serde_json::json!(projects::found()?));
            }
        }
        ProjectCmd::Ledger { .. } => unreachable!(),
        ProjectCmd::Delete { id } => {
            projects::delete(&id)?;
            println!("{id} moved to the trash");
        }
    }
    Ok(())
}

#[cfg(feature = "brain")]
#[derive(Subcommand)]
enum BrainCmd {
    /// What the Brain holds, and its settings
    Status,
    /// Bring it up to date with the calls (only what changed; vectors for sections without one)
    Sync,
    /// Search it: keywords in any language and meaning (when an embedding server is set), fused
    Search {
        query: String,
        #[arg(long, default_value_t = 8)]
        limit: usize,
        /// Only sources whose id contains this (e.g. call/2026-10)
        #[arg(long)]
        within: Option<String>,
        /// Only one client's sections (its id; "-": calls with no client)
        #[arg(long, conflicts_with = "thread")]
        client: Option<String>,
        /// Only one thread's sections (its id; "-": calls in no thread)
        #[arg(long)]
        thread: Option<String>,
    },
    /// The clients and threads it covers, with their documents and sections
    Scopes,
    /// Who may speak into the Brain through mail and channels, per client: trusted, ignored, waiting.
    /// A decision drops what sources gave and reads them again at the next sync
    Senders {
        /// The client (a thread with none: "-")
        #[arg(long)]
        client: Option<String>,
        /// Trust this sender (an address, or teams:<id> as listed)
        #[arg(long, requires = "client", conflicts_with_all = ["ignore", "forget"])]
        trust: Option<String>,
        /// Never read this sender
        #[arg(long, requires = "client", conflicts_with = "forget")]
        ignore: Option<String>,
        /// Forget a decision (the sender waits again)
        #[arg(long, requires = "client")]
        forget: Option<String>,
    },
    /// The thread folders' files: what was read, how, and why the others weren't
    Files,
    /// The numbers lane: governed metrics from a client's reporting workbooks
    Numbers {
        #[command(subcommand)]
        cmd: NumbersCmd,
    },
    /// (dev) What the Brain's readers make of files: method, pages, routing (why a page goes to the page reader), date
    #[command(hide = true)]
    ReadFile {
        paths: Vec<std::path::PathBuf>,
        /// Print each page's text too
        #[arg(long)]
        text: bool,
    },
    /// (dev) Where a work item comes up elsewhere in its thread: the Work tab's "Mentioned also in"
    #[command(hide = true)]
    Mentions {
        #[arg(long)]
        thread: String,
        #[arg(long)]
        title: String,
        /// JSON [[call stamp, quote], …]
        #[arg(long, default_value = "[]")]
        quotes: String,
    },
    /// A client's or a thread's insights (--run: refresh them now, Claude on the default profile)
    Insights {
        #[arg(long, conflicts_with = "thread")]
        client: Option<String>,
        #[arg(long)]
        thread: Option<String>,
        #[arg(long)]
        run: bool,
    },
    /// The sections nearest to one, from other sources
    Related {
        chunk: i64,
        #[arg(long, default_value_t = 6)]
        limit: usize,
    },
    /// Serve it to Claude Code as an MCP server over stdio (read-only; Brain's tool names). Only one AI
    /// profile's sources (--profile; default: the default profile's), narrowed for a call's session
    /// to its client (--client), else its threads (--thread), else the call itself (--call)
    Mcp {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        thread: Vec<String>,
        #[arg(long)]
        call: Option<String>,
        /// Who runs it (folder | call | routine), for the usage counts in Settings → Brain
        #[arg(long)]
        who: Option<String>,
    },
    /// The taxonomy: topics and terms (it supersedes thread glossaries)
    Taxonomy {
        #[command(subcommand)]
        cmd: TaxonomyCmd,
    },
    /// The embedding server (Settings → Brain): Ollama or OpenAI-compatible
    Set {
        #[arg(long)]
        endpoint: Option<String>,
        /// ollama | openai
        #[arg(long)]
        api: Option<String>,
        #[arg(long)]
        model: Option<String>,
        /// A bearer token for an OpenAI-compatible server ("" removes it)
        #[arg(long)]
        key: Option<String>,
        /// The Brain at all: false turns it off (nothing read, indexed or offered to Claude; the store stays)
        #[arg(long)]
        enabled: Option<bool>,
    },
}

#[cfg(feature = "brain")]
#[derive(Subcommand)]
enum NumbersCmd {
    /// Profile the client's workbooks (its threads' folders, or --root): sheets, headers, samples
    Profile {
        client: String,
        /// A folder to read instead of the client's threads' (testing)
        #[arg(long)]
        root: Option<std::path::PathBuf>,
    },
    /// Claude proposes the families and metrics from the profiles (saved as proposed)
    Propose {
        client: String,
        #[arg(long)]
        root: Option<std::path::PathBuf>,
    },
    /// Use a families config written by hand or by Brain (families.<corpus>.json), and its metrics.json
    Import {
        client: String,
        families: std::path::PathBuf,
        #[arg(long)]
        metrics: Option<std::path::PathBuf>,
        /// Accept it at once
        #[arg(long)]
        accept: bool,
    },
    /// Accept the proposed families (the build reads them from now on)
    Accept { client: String },
    /// Build the client's facts from its accepted families
    Build {
        client: String,
        #[arg(long)]
        root: Option<std::path::PathBuf>,
        /// Build a config that isn't accepted yet (testing)
        #[arg(long)]
        force: bool,
    },
    /// Every client's families, catalog and last build
    Show,
    /// The governed metrics (MCP list_metrics)
    List {
        #[arg(long)]
        client: Option<String>,
    },
    /// A metric's values (MCP get_metric)
    Get {
        name: String,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        grain: Option<String>,
        #[arg(long)]
        entity: Option<String>,
        #[arg(long)]
        month: Option<String>,
    },
}

#[cfg(feature = "brain")]
#[derive(Subcommand)]
enum TaxonomyCmd {
    /// Every topic and term, with its sections and threads, and the coverage
    Show,
    /// Build it from the calls (Claude, on the default AI profile): every layer, or one (--layer shared | a client's id);
    /// a provisional version to review, then the layer's sections tagged
    Build {
        #[arg(long)]
        layer: Option<String>,
    },
    /// Whether each layer's topics still fit its calls (no model): new sections fitting no topic, one
    /// topic taking most, calls outgrowing its build (JSON)
    Drift,
    /// Topics that look in the wrong place (no model): clients' topics naming one thing → up to the
    /// practice; a practice topic only one client uses → down to it. --apply KEY does one, --dismiss KEY
    /// keeps it where it is
    Moves {
        #[arg(long)]
        apply: Option<String>,
        #[arg(long)]
        dismiss: Option<String>,
    },
    /// Split a topic that is too broad into 2–6 subtopics (Claude, on the default AI profile), then tag only
    /// its sections again (layer: shared | a client's id)
    Split { layer: String, topic: String },
    /// Tag sections not tagged yet (--again: all of them; --layer: only one layer's sections)
    Classify {
        #[arg(long)]
        again: bool,
        #[arg(long)]
        layer: Option<String>,
    },
    /// The user's change (JSON: {op: accept|add|rename|merge|move|remove|set, label|name, term?, to?, into?, parent?, …})
    Edit { json: String },
}

#[cfg(feature = "brain")]
fn brain_command(cmd: BrainCmd) -> Result<()> {
    // Reading is allowed anywhere (a session Echo started reads it through MCP); changing it isn't.
    if let BrainCmd::Mcp { profile, client, thread, call, who } = cmd {
        let access = brain::access::Access { profile: profile.unwrap_or_else(ai::default_id), client, threads: thread, call };
        return brain::mcp::serve(Some(access), who);
    }
    if notes::headless() {
        anyhow::bail!("the Brain isn't changed from a run Echo started");
    }
    match cmd {
        BrainCmd::Mcp { .. } => unreachable!(),
        BrainCmd::Taxonomy { cmd } => {
            let progress = |m: &str| eprintln!("{m}");
            match cmd {
                TaxonomyCmd::Show => print_json(&brain::taxonomy::view()?),
                TaxonomyCmd::Drift => print_json(&serde_json::json!(brain::taxonomy::drift()?)),
                TaxonomyCmd::Split { layer, topic } => print_json(&brain::taxonomy::split(&layer, &topic, &progress)?),
                TaxonomyCmd::Moves { apply, dismiss } => {
                    if let Some(k) = apply {
                        println!("{}", brain::taxonomy::apply_move(&k)?);
                    }
                    if let Some(k) = dismiss {
                        brain::taxonomy::dismiss_move(&k)?;
                    }
                    print_json(&serde_json::json!(brain::taxonomy::moves()?))
                }
                TaxonomyCmd::Build { layer } => print_json(&match layer {
                    Some(l) => brain::taxonomy::induce(&l, &progress)?,
                    None => brain::taxonomy::induce_all(&progress)?,
                }),
                TaxonomyCmd::Classify { again, layer } => print_json(&serde_json::json!({"classified": brain::taxonomy::classify(layer.as_deref(), again, &progress)?})),
                TaxonomyCmd::Edit { json } => {
                    let op: serde_json::Value = serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("not JSON: {e}"))?;
                    brain::taxonomy::edit(&op)?;
                    print_json(&brain::taxonomy::view()?)
                }
            }
        }
        BrainCmd::Status => print_json(&brain::status()?),
        BrainCmd::Sync => print_json(&serde_json::to_value(brain::sync()?)?),
        BrainCmd::Search { query, limit, within, client, thread } => {
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            print_json(&brain::search(&query, limit, within.as_deref(), scope)?)
        }
        BrainCmd::Scopes => print_json(&brain::store::scopes(&brain::store_path())?),
        BrainCmd::Senders { client, trust, ignore, forget } => {
            let pick = trust.map(|k| ("trust", k)).or(ignore.map(|k| ("ignore", k))).or(forget.map(|k| ("forget", k)));
            if let (Some(c), Some((d, k))) = (&client, pick) {
                brain::senders::decide(c, &k, d)?;
            }
            let mut v = brain::senders::view();
            if let Some(c) = &client {
                v["clients"] = serde_json::json!({c.clone(): v["clients"][c].clone()});
            }
            print_json(&v)
        }
        BrainCmd::Files => print_json(&brain::files::folder::status()?),
        BrainCmd::Numbers { cmd } => {
            use brain::numbers as n;
            let root = |r: Option<std::path::PathBuf>| r.map(|r| vec![(r, vec![])]);
            match cmd {
                NumbersCmd::Profile { client, root: r } => print_json(&n::profile::run(&client, root(r))?["summary"]),
                NumbersCmd::Propose { client, root: r } => print_json(&n::profile::propose(&client, root(r))?),
                NumbersCmd::Import { client, families, metrics, accept } => {
                    let c = n::import_config(&client, &families, accept)?;
                    if let Some(m) = metrics {
                        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&m)?)?;
                        let cat: std::collections::BTreeMap<String, serde_json::Value> = serde_json::from_value(v.get("metrics").cloned().unwrap_or(v))?;
                        n::save_catalog(&client, &cat)?;
                    }
                    print_json(&serde_json::json!({"families": c.families.len(), "accepted": c.accepted()}));
                }
                NumbersCmd::Accept { client } => {
                    let mut c = n::load_config(&client).ok_or_else(|| anyhow::anyhow!("no families for {client}: propose or import them first"))?;
                    c.check()?;
                    c._echo = Some(serde_json::json!({"status": "accepted", "at": chrono::Utc::now().to_rfc3339(), "was": c._echo}));
                    n::save_config(&client, &c)?;
                    print_json(&serde_json::json!({"accepted": c.families.len()}));
                }
                NumbersCmd::Build { client, root: r, force } => {
                    let c = n::load_config(&client).ok_or_else(|| anyhow::anyhow!("no families for {client}: propose or import them first"))?;
                    anyhow::ensure!(force || c.accepted(), "the families for {client} are proposed, not accepted: accept them first (or --force)");
                    print_json(&n::build::run(&client, &c, root(r))?);
                }
                NumbersCmd::Show => print_json(&n::overview()?),
                NumbersCmd::List { client } => print_json(&n::list_metrics(client.as_deref())?),
                NumbersCmd::Get { name, client, grain, entity, month } => print_json(&n::get_metric(&serde_json::json!({"name": name, "client": client, "grain": grain, "entity": entity, "month": month}))?),
            }
        }
        BrainCmd::ReadFile { paths, text } => {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            for p in paths {
                match brain::files::read(&p) {
                    Ok(r) => {
                        let body = r.md.clone().unwrap_or_else(|| r.pages.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join("\n"));
                        let (date, from) = brain::files::exact_date(&body, &p, &today);
                        let pages: Vec<serde_json::Value> = r
                            .pages
                            .iter()
                            .map(|x| {
                                let mut v = serde_json::json!({"n": x.n, "title": x.title, "chars": x.text.chars().count(), "tables": x.tables.len(), "drawings": x.drawings,
                                    "pictures": x.pictures, "connectors": x.connectors, "charts": x.charts, "smartart": x.smartart, "route": brain::files::route(x)});
                                if text {
                                    v["text"] = serde_json::json!(x.text);
                                }
                                v
                            })
                            .collect();
                        let flagged = r.pages.iter().filter(|x| brain::files::route(x).is_some()).count();
                        print_json(&serde_json::json!({"file": p, "method": r.method, "fidelity": r.fidelity, "skip": r.skip, "date": date, "date_from": from,
                            "chars": r.md.as_ref().map(|m| m.chars().count()), "figures": r.figures, "pages": r.pages.len(), "flagged": flagged,
                            "page_list": pages, "md": if text { r.md } else { None }}));
                    }
                    Err(e) => print_json(&serde_json::json!({"file": p, "error": format!("{e:#}")})),
                }
            }
        }
        BrainCmd::Mentions { thread, title, quotes } => {
            let q: Vec<(String, String)> = serde_json::from_str(&quotes).map_err(|e| anyhow::anyhow!("--quotes: {e}"))?;
            print_json(&brain::views::mentions(&thread, &title, &q)?)
        }
        BrainCmd::Insights { client, thread, run } => {
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t))).ok_or_else(|| anyhow::anyhow!("--client or --thread"))?;
            if run {
                print_json(&brain::insights::run(scope, &|m: &str| eprintln!("{m}"))?);
            }
            print_json(&brain::insights::view(scope)?)
        }
        BrainCmd::Related { chunk, limit } => print_json(&brain::store::related(&brain::store_path(), chunk, limit)?),
        BrainCmd::Set { endpoint, api, model, key, enabled } => {
            let mut s = brain::settings();
            if let Some(on) = enabled {
                s.enabled = Some(on);
            }
            if let Some(e) = endpoint {
                s.endpoint = e.trim().to_string();
            }
            if let Some(a) = api {
                s.api = a.trim().to_string();
            }
            if let Some(m) = model {
                s.model = m.trim().to_string();
            }
            if let Some(k) = key {
                s.key = (!k.is_empty()).then_some(k);
            }
            brain::save_settings(&s)?;
            print_json(&brain::status()?)
        }
    }
    Ok(())
}

/// Inside a read-only routine Echo started (`guard::RUN_MODE=routine`, set by the app, never by the
/// run): the user's own prompt may read projects, clients and organisations.
fn routine_reads() -> bool {
    std::env::var(guard::RUN_MODE).is_ok_and(|m| m == "routine")
}

/// What a delivery test sends: the shapes a routine's answer has (headings, a list, a table).
const TEST_MESSAGE: &str = "This is how Echo's messages reach you.\n\n## Today\n\n1. **Send the deck** to Acme — due today\n2. Review `T012` before the 15:00 call\n\n## Meetings\n\n| Time | Meeting | Prepare |\n|---|---|---|\n| 10:00 | Weekly sync | Open asks |\n| 15:00 | Acme review | The deck |\n\n> Nothing here is real: it's a test.";

fn deliver_command(cmd: DeliverCmd) -> Result<()> {
    // The ways plugins add live in their host halves: load them (commands only, no timers).
    #[cfg(feature = "js")]
    {
        jshost::no_timers();
        jshost::sync();
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    match cmd {
        DeliverCmd::Status => print_json(&deliver::status()),
        DeliverCmd::Send { kind, title, file, about, ai, to } => {
            deliver::ready(&kind)?;
            let to: Vec<String> = to.split(',').map(str::trim).filter(|a| !a.is_empty()).map(str::to_owned).collect();
            let markdown = std::fs::read_to_string(file)?;
            deliver::send(&kind, &deliver::Msg { title: &title, markdown: &markdown, about: about.as_deref(), ai: ai.as_deref(), mail_to: &to })?;
            println!("Sent");
        }
        DeliverCmd::Test { kind } => {
            deliver::ready(&kind)?;
            deliver::send(&kind, &deliver::Msg { title: "Echo test", markdown: TEST_MESSAGE, about: Some("A test from Settings"), ..Default::default() })?;
            println!("Sent");
        }
    }
    Ok(())
}

fn plugin_command(cmd: PluginCmd) -> Result<()> {
    use plugins::*;
    // Install / update ask in the terminal's own words; --yes is the answer.
    let agree = |r: &Review, yes: bool| -> Result<Vec<String>> {
        print_review(r);
        if let Some(b) = &r.blocked {
            anyhow::bail!("{} can't be installed: {b}", r.name);
        }
        if !yes && !(r.new_uses.is_empty()) {
            anyhow::bail!("run again with --yes to agree");
        }
        Ok(r.uses.clone())
    };
    match cmd {
        PluginCmd::List => print_json(&snapshot()),
        PluginCmd::Install { id, from, yes } => {
            let r = review(&id, from.as_deref())?;
            let agreed = agree(&r, yes)?;
            let p = install(&id, from.as_deref(), &agreed)?;
            println!("Installed {} {}", p.name, p.version);
        }
        PluginCmd::Update { id, yes } => {
            let ids: Vec<String> = match id {
                Some(i) => vec![i],
                None => Registry::load()?.installed.into_keys().collect(),
            };
            for id in ids {
                let p = Registry::load()?.installed.get(&id).cloned().ok_or_else(|| anyhow::anyhow!("“{id}” isn't installed"))?;
                refresh(&p.market)?;
                let Some(r) = update_for(&p) else {
                    println!("{} {} is up to date", p.name, p.version);
                    continue;
                };
                let agreed = agree(&r, yes)?;
                if let Some(n) = update(&id, &agreed)? {
                    println!("Updated {} to {}", n.name, n.version);
                }
            }
        }
        PluginCmd::Remove { id, delete_data } => remove(&id, !delete_data)?,
        PluginCmd::Enable { id } => set_enabled(&id, true)?,
        PluginCmd::Disable { id } => set_enabled(&id, false)?,
        PluginCmd::Set { id, key, value: None } => print_json(&serde_json::json!(settings_view(&id)?.into_iter().find(|s| s["key"] == key.as_str()))),
        PluginCmd::Set { id, key, value: Some(v) } => set_setting(&id, &key, &serde_json::Value::String(v))?,
        #[cfg(feature = "js")]
        PluginCmd::Call { id, command, args } => {
            let args: serde_json::Value = serde_json::from_str(args.as_deref().unwrap_or("{}")).map_err(|e| anyhow::anyhow!("the arguments aren't JSON: {e}"))?;
            // A terminal's host runs commands, never the plugins' timers: those are the app's.
            jshost::no_timers();
            jshost::sync();
            print_json(&jshost::call_within(&id, &command, &args, std::time::Duration::from_secs(3600))?);
        }
        #[cfg(not(feature = "js"))]
        PluginCmd::Call { .. } => anyhow::bail!("this build of Echo runs no plugin code"),
        PluginCmd::Marketplace { cmd } => match cmd {
            MarketCmd::List => print_json(&snapshot()["markets"]),
            MarketCmd::Add { source, name } => {
                let n = market_add(&source, name.as_deref())?;
                let c = refresh(&n)?;
                match c.error {
                    Some(e) => println!("Added {n}, but it can't be read yet: {e}"),
                    None => println!("Added {n}: {} plugins", c.plugins.len()),
                }
            }
            MarketCmd::Remove { name } => market_remove(&name)?,
            MarketCmd::Hide { name } => market_hide(&name, true)?,
            MarketCmd::Show { name } => market_hide(&name, false)?,
            MarketCmd::Update { name } => match name {
                Some(n) => {
                    let c = refresh(&n)?;
                    if let Some(e) = c.error {
                        anyhow::bail!("{e}");
                    }
                }
                None => refresh_stale(true),
            },
        },
    }
    Ok(())
}

fn org_command(cmd: OrgCmd) -> Result<()> {
    if notes::headless() && !(matches!(cmd, OrgCmd::List) && routine_reads()) {
        anyhow::bail!("organisations can't be changed or listed from a run Echo started");
    }
    match cmd {
        OrgCmd::List => print_json(&serde_json::to_value(projects::orgs()?)?),
        OrgCmd::Add { name, domains, thread, account } => {
            let o = projects::add_org(&name, &domains, false, account)?;
            if let Some(t) = thread {
                people::link_org(&t, &o.id, true)?;
            }
            print_json(&serde_json::to_value(o)?)
        }
        OrgCmd::Set { id, name, domains, aliases, parent, account, logo, thread, unlink } => {
            if let Some(f) = logo {
                let bytes = if f.as_os_str().is_empty() { None } else { Some(std::fs::read(&f)?) };
                projects::set_client_logo(&id, bytes.as_deref())?;
            }
            let known = projects::orgs()?;
            if let Some(p) = parent.as_deref().filter(|p| !p.is_empty()) {
                if p == id || !known.iter().any(|c| c.id == p) {
                    anyhow::bail!("no other organisation {p:?}");
                }
            }
            let o = projects::update_client(&id, |c| {
                if let Some(n) = name {
                    c.name = n;
                }
                if let Some(d) = domains {
                    c.domains = d;
                }
                if let Some(a) = aliases {
                    c.aliases = a.into_iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
                }
                if let Some(p) = parent {
                    c.parent = Some(p).filter(|p| !p.is_empty());
                }
                if let Some(a) = account {
                    c.account = a;
                }
                Ok(())
            })?;
            if let Some(t) = thread {
                people::link_org(&t, &id, true)?;
            }
            if let Some(t) = unlink {
                people::link_org(&t, &id, false)?;
            }
            print_json(&serde_json::to_value(o)?)
        }
        OrgCmd::Remove { id } => {
            projects::remove_client(&id)?;
            println!("{id} removed");
        }
    }
    Ok(())
}

fn client_command(cmd: ClientCmd) -> Result<()> {
    if notes::headless() && !(matches!(cmd, ClientCmd::List) && routine_reads()) {
        anyhow::bail!("clients can't be changed or listed from a run Echo started");
    }
    match cmd {
        ClientCmd::List => print_json(&serde_json::to_value(projects::clients()?)?),
        ClientCmd::People { id } => print_json(&people::of_client(&id, chrono::Local::now().date_naive())?),
        ClientCmd::Add { name, domains, internal } => print_json(&serde_json::to_value(projects::add_client(&name, &domains, internal)?)?),
        ClientCmd::Set { id, name, domains, internal, logo } => {
            if let Some(f) = logo {
                let bytes = if f.as_os_str().is_empty() { None } else { Some(std::fs::read(&f)?) };
                projects::set_client_logo(&id, bytes.as_deref())?;
            }
            let c = projects::update_client(&id, |c| {
                if let Some(n) = name {
                    c.name = n;
                }
                if let Some(d) = domains {
                    c.domains = d;
                }
                if let Some(i) = internal {
                    c.internal = i;
                }
                Ok(())
            })?;
            print_json(&serde_json::to_value(c)?)
        }
        ClientCmd::Remove { id } => {
            projects::remove_client(&id)?;
            println!("{id} removed");
        }
    }
    Ok(())
}

fn ledger_command(id: &str, cmd: LedgerCmd) -> Result<()> {
    // Inside a run Echo started: the agent's steps, on that run's project only (a read-only routine:
    // the tree).
    if notes::headless() && !(matches!(cmd, LedgerCmd::Tree) && routine_reads()) {
        let own = std::env::var(guard::RUN_PROJECT).unwrap_or_default();
        let agent_step = matches!(cmd, LedgerCmd::Pending | LedgerCmd::Context | LedgerCmd::Apply { .. } | LedgerCmd::Done { .. } | LedgerCmd::VerifyPrep { .. } | LedgerCmd::VerifyApply { .. } | LedgerCmd::PeopleContext { .. } | LedgerCmd::PeopleApply { .. });
        if own.is_empty() || own != id || !agent_step {
            anyhow::bail!("a run Echo started works only on its own project's ledger, with the agent's steps");
        }
    }
    let read = |p: &std::path::Path| -> Result<Vec<serde_json::Value>> {
        // A run reads its inputs only from its own folder (not another project's files).
        if notes::headless() {
            let run = std::env::var_os(guard::RUN_DIR).and_then(|d| dunce::canonicalize(d).ok());
            let parent = dunce::canonicalize(p).ok().and_then(|f| f.parent().map(std::path::Path::to_path_buf));
            if run.is_none() || parent != run {
                anyhow::bail!("a run reads its files only from its own folder");
            }
        }
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p)?).map_err(|e| anyhow::anyhow!("{}: not JSON ({e})", p.display()))?;
        v.as_array().cloned().ok_or_else(|| anyhow::anyhow!("{}: expected a JSON array", p.display()))
    };
    match cmd {
        LedgerCmd::Sync { limit, chats, mail } => {
            let mut written = ledger::sync(id, limit)?;
            if chats {
                written.extend(ledger::chat_sync(id)?);
            }
            if mail {
                written.extend(ledger::mail_sync(id)?);
            }
            print_json(&serde_json::json!({ "written": written, "pending": ledger::pending(id)?, "chats": ledger::chat_state(id), "mail": ledger::mail_state(id) }))
        }
        LedgerCmd::Mail { ids, conversation } => print_json(&serde_json::json!({ "mail": ledger::add_mail(id, &ids, conversation)?, "pending": ledger::pending(id)? })),
        LedgerCmd::Pending => print_json(&serde_json::json!(ledger::pending(id)?)),
        LedgerCmd::Context => print_json(&ledger::context(id)?),
        LedgerCmd::Apply { evidence, proposals } => print_json(&ledger::apply(id, &evidence, &read(&proposals)?)?),
        LedgerCmd::Done { evidence, note } => {
            ledger::mark_processed(id, &evidence, &note)?;
            print_json(&serde_json::json!({ "done": evidence }));
        }
        LedgerCmd::VerifyPrep { weak, out } => {
            // A run writes only into its own folder.
            if let (true, Some(f)) = (notes::headless(), &out) {
                let run = std::env::var_os(guard::RUN_DIR).and_then(|d| dunce::canonicalize(d).ok());
                let parent = f.parent().and_then(|d| dunce::canonicalize(d).ok());
                if run.is_none() || parent != run {
                    anyhow::bail!("a run writes its packets only into its own folder");
                }
            }
            let packets = serde_json::json!(ledger::verify_prep(id, weak)?);
            match out {
                Some(f) => {
                    std::fs::write(&f, serde_json::to_vec_pretty(&packets)?)?;
                    // How the user works (About me), for judging what counts as done or handed over.
                    let me = calls::me_profile().and_then(|t| calls::me_parts(&t, &["identity", "patterns"], 2500));
                    print_json(&serde_json::json!({ "packets": packets.as_array().map(|a| a.len()), "file": f, "me_patterns": me }));
                }
                None => print_json(&packets),
            }
        }
        LedgerCmd::VerifyApply { file } => print_json(&ledger::verify_apply(id, &read(&file)?)?),
        LedgerCmd::PeopleContext { items } => print_json(&people::context(id, &items)?),
        LedgerCmd::PeopleApply { evidence, file } => print_json(&people::apply_suggestions(id, &evidence, &read(&file)?)?),
        LedgerCmd::Tree => print_json(&ledger::tree(id)?),
        LedgerCmd::Run => print_json(&projectsync::run(id)?),
        LedgerCmd::Status => print_json(&projectsync::state(id)),
        LedgerCmd::Edit { json } => {
            let body: serde_json::Value = serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("not JSON: {e}"))?;
            print_json(&ledger::human_event(id, &body)?)
        }
    }
    Ok(())
}

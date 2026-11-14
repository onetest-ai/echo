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
mod calls_week;
#[cfg(feature = "tray")]
mod announce;
mod call_language;
mod config;
mod deliver;
mod m365;
mod memo;
mod needs;
#[cfg(any(target_os = "macos", windows))]
mod frames;
#[cfg(feature = "transcribe")]
mod gpu;
mod graph;
mod grants;
mod evidence;
mod guard;
mod ledger;
mod kept_minutes;
mod claude_session;
mod livenotes;
mod links;
#[cfg(feature = "transcribe")]
mod live;
mod meeting;
#[cfg(feature = "tray")]
mod diagrams;
mod drafts;
mod folder_args;
mod onedrive;
mod minutes;
mod notes;
mod ollama;
mod models;
mod paths;
mod plugins;
mod projects;
mod people;
mod photos;
mod prep;
mod projectsync;
mod tasks;
mod threadlinks;
mod taskmodes;
mod glance;
mod today;
mod today_layout;
mod boards;
mod widgets;
mod yolo;
mod teams_transcript;
mod view;
mod vocab;
mod permissions;
mod recorder;
#[cfg(feature = "transcribe")]
mod nemo;
#[cfg(feature = "transcribe")]
mod remote;
mod run_log;
mod cli;
mod claude_setup;
#[cfg(feature = "js")]
mod claudejob;
#[cfg(feature = "js")]
mod jshost;
#[cfg(feature = "transcribe")]
mod speakers;
#[cfg(feature = "transcribe")]
mod speaking;
mod schedule;
mod series;
mod skills;
#[cfg(feature = "js")]
mod share;
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
#[cfg(feature = "tray")]
mod whats_new;

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
        /// Skip the local AI profiles' context check (it loads each local model, up to 2 min each)
        #[arg(long)]
        no_ai: bool,
    },
    /// List processes currently using audio (debug aid for call detection)
    Procs,
    /// Transcription models: list (with a hardware-based recommendation), download, use
    Model {
        #[command(subcommand)]
        cmd: cli::app::ModelCmd,
    },
    /// The echoctl app (tray icon + call watcher): run, start, quit, start at login
    App {
        #[command(subcommand)]
        cmd: Option<cli::app::AppCmd>,
    },
    /// The data folder (calls + settings): show it, or choose another one
    Home {
        #[command(subcommand)]
        cmd: Option<cli::app::HomeCmd>,
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
    /// Claude Code integration: is it set up (it runs, a terminal finds it, Echo's plugin is in), install the plugin, or put it on the PATH
    Claude {
        #[command(subcommand)]
        cmd: Option<cli::app::ClaudeCmd>,
    },
    /// Show or change settings (<home>/.echo/config.json)
    Config(cli::app::ConfigArgs),
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
        /// Write them with this AI profile from now on (`echoctl ai list`): the call's profile changes
        #[arg(long)]
        profile: Option<String>,
        /// Write them without this Claude Code policy file, which Claude Code can't read either (when a run said so)
        #[arg(long = "without-policy")]
        without_policy: Option<String>,
    },
    /// AI profiles: which model (and account) writes a call's live notes and minutes
    Ai {
        #[command(subcommand)]
        cmd: Option<cli::app::AiCmd>,
    },
    /// Live notes during a call: ask a question, pause or resume them, show them (JSON)
    Live {
        #[command(subcommand)]
        cmd: cli::capture::LiveCmd,
    },
    /// "Last time" for a recurring call (JSON): the previous occurrence's open actions and highlights. With
    /// --upcoming: today's meetings and their preps; --show / --write ID: a meeting's prep (written by its threads'
    /// AI profile)
    Prep {
        #[arg(default_value = "latest")]
        call: String,
        #[arg(long)]
        upcoming: bool,
        #[arg(long)]
        show: Option<String>,
        #[arg(long)]
        write: Option<String>,
    },
    /// The Brain: cross-call intelligence over calls (later channels, mail, folders) (JSON)
    #[cfg(feature = "brain")]
    Brain {
        #[command(subcommand)]
        cmd: cli::brain::BrainCmd,
    },
    /// Projects: which calls belong (rules, pins), stage, glossary, people (JSON)
    Project {
        #[command(subcommand)]
        cmd: cli::project::ProjectCmd,
    },
    /// Clients: the organisations projects are for, with their mail domains (JSON)
    Client {
        #[command(subcommand)]
        cmd: cli::project::ClientCmd,
    },
    /// Organisations: whose people are whose (accounts — clients — and the others, a thread's
    /// counterparts), with their mail domains (JSON)
    Org {
        #[command(subcommand)]
        cmd: cli::project::OrgCmd,
    },
    /// Minutes templates (JSON): each one's words, header and sections, the {values} they may use, and the folder for
    /// your own (a template there named like a built-in changes it)
    Templates,
    /// Today's page on this computer: its widgets in order (`today`), add / move / size / remove one, reset it;
    /// with --thread or --client, that thread's or client's dashboard instead (--board: one of its own dashboards)
    Today {
        /// A thread's dashboard (its Brief) instead of Today
        #[arg(long, global = true)]
        thread: Option<String>,
        /// A client's dashboard (its Overview) instead of Today
        #[arg(long, global = true, conflicts_with = "thread")]
        client: Option<String>,
        /// One of the dashboards beside the Brief (the Overview), by its id (`echoctl dashboard list`): its layout is
        /// every thread's (client's), shown here with this one's data
        #[arg(long, global = true)]
        board: Option<String>,
        #[command(subcommand)]
        cmd: Option<cli::widget::TodayCmd>,
    },
    /// The dashboards beside every thread's Brief or every client's Overview: list, add, rename, move, remove
    Dashboard {
        #[command(subcommand)]
        cmd: cli::widget::DashboardCmd,
    },
    /// Today's widgets: `list` the catalog, `show <id>` a widget's data (JSON, as Today gets it)
    Widget {
        #[command(subcommand)]
        cmd: cli::widget::WidgetCmd,
    },
    /// The plugins' work rows: `show [--thread <id>]` (JSON, as My work or a thread's Work tab gets them)
    Work {
        #[command(subcommand)]
        cmd: cli::widget::WorkCmd,
    },
    /// Today → Needs you now (JSON): what waits on you from the last day in Teams and mail; --refresh reads them again
    #[command(hide = true)]
    Needs {
        #[arg(long)]
        refresh: bool,
    },
    /// What Echo does with every meeting of a series: record always / ask / never, and in which language
    Series {
        #[command(subcommand)]
        cmd: cli::calendar::SeriesCmd,
    },
    /// Echo plugins from marketplaces: list, install, update, remove, settings
    Plugin {
        #[command(subcommand)]
        cmd: cli::plugin::PluginCmd,
    },
    /// Microsoft 365 on Echo's sign-in: search mail, chats and files; mail; Teams; people; OneDrive (JSON). Sending
    /// needs --yes: without it, what would be sent
    M365 {
        #[command(subcommand)]
        cmd: cli::m365::M365Cmd,
    },
    /// Where Echo sends things to you: Teams (your chat with yourself), mail, and the ways plugins add
    Deliver {
        #[command(subcommand)]
        cmd: cli::m365::DeliverCmd,
    },
    /// Vocabulary (names/terms Whisper should spell right), learned automatically
    Vocab {
        #[command(subcommand)]
        cmd: cli::capture::VocabCmd,
    },
    /// Echo's MCP server for Claude sessions (stdio): what was said, people, echo:// links — read-only, one AI
    /// profile's sources (narrowed for a call's session)
    #[cfg(feature = "brain")]
    #[command(hide = true)]
    Mcp {
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        client: Option<String>,
        #[arg(long)]
        thread: Vec<String>,
        #[arg(long)]
        call: Option<String>,
        /// Who runs it (folder | call | routine), for the usage counts
        #[arg(long)]
        who: Option<String>,
    },
    /// What Echo holds behind an echo:// link, as text (for Claude): a call's minutes and the turns around its moment,
    /// a thread, a client, a person, a mail's facts. Read-only
    Read { link: String },
    /// What Claude in Echo's folder session starts with (Markdown): who Echo is, its rules and the plugins with a guide
    /// to Read first; --hook: as a SessionStart hook's JSON (each plugin's folder laid out again first if it changed)
    Core {
        #[arg(long)]
        hook: bool,
    },
    /// Input bundle for writing a call's notes (JSON): sizes, languages, attendees, gate
    Context {
        #[arg(default_value = "latest")]
        call: String,
    },
    /// Type the agenda's meetings by their invites now, and guess who does each of your open tasks (JSON)
    #[command(hide = true)]
    Classify,
    /// Set what kind of meeting a call was: client, partner, one-on-one, internal, industry, other (yours is kept)
    Type { call: String, kind: String },
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
    /// Write a mail as a draft where you keep drafts (Microsoft 365 Drafts, the mail app or the app you chose:
    /// `config --drafts`); it is never sent from here
    Compose {
        #[arg(long, value_delimiter = ',')]
        to: Vec<String>,
        #[arg(long, value_delimiter = ',')]
        cc: Vec<String>,
        #[arg(long)]
        subject: String,
        /// Plain-text body, from a file
        #[arg(long, required_unless_present = "body", conflicts_with = "body")]
        body_file: Option<std::path::PathBuf>,
        /// Plain-text body, as given (a run that writes no files)
        #[arg(long)]
        body: Option<String>,
        /// A model wrote it (its name): the draft says so
        #[arg(long)]
        ai: Option<String>,
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
        cmd: cli::capture::MeetingCmd,
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
    /// Change a call's language (Echo never detects it): while it records, what is said from then on is in it and the
    /// whole call is transcribed again at its end; after, it is transcribed again when its audio was kept
    Language {
        /// Language code: en, ru, uk, es, …
        code: String,
        /// Call folder (path or name under calls/), or "latest"
        #[arg(default_value = "latest")]
        call: String,
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
        cmd: Option<cli::app::ServerCmd>,
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
    /// (internal) The folder session and Yolo's runs ask it before a write: never into `.plugins/`, which Echo lays out
    /// from the installed plugins; exit 2 refuses it
    #[command(hide = true)]
    PluginsHook,
    /// (internal) Echo's Claude terminals ask it before a write: never a folder here that OneDrive keeps online but
    /// doesn't sync to this computer (`onedrive.rs`); exit 2 refuses it
    #[command(hide = true)]
    OnedriveHook,
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
    /// Memos: record yourself, no call needed — start, stop; its threads are found from what you said
    Memo {
        #[command(subcommand)]
        cmd: cli::capture::MemoCmd,
    },
    /// Watcher / arm / agenda state as JSON
    Status,
    /// Preflight in one step: app installed/running, permissions, model, agenda (JSON)
    Up,
    /// Microsoft 365 calendar (read-only): sign in, sign out, status
    Calendar {
        #[command(subcommand)]
        cmd: Option<cli::calendar::CalendarCmd>,
    },
    /// Upcoming meetings the watcher offers to join & record
    Agenda {
        #[command(subcommand)]
        cmd: cli::calendar::AgendaCmd,
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
    /// (internal) The tools of a call's live-notes session, over MCP on stdin/stdout (`livenotes::agent`)
    #[command(hide = true)]
    LiveMcp {
        #[arg(long)]
        call: std::path::PathBuf,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        app: Option<u32>,
    },
    /// (dev) Echo's call-window reader on Teams' call window for a while, into a folder (ui.jsonl, speaking.jsonl);
    /// --full-only reads the whole window every poll, as before #236
    #[cfg(target_os = "macos")]
    #[command(hide = true)]
    AxRecord {
        dir: std::path::PathBuf,
        #[arg(long, default_value_t = 60)]
        seconds: u64,
        #[arg(long)]
        full_only: bool,
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
    /// (dev) Which call Teams is in, as a recording sees it: when it began (its timer) and its name — what tells
    /// back-to-back calls apart
    #[cfg(any(target_os = "macos", windows))]
    #[command(hide = true)]
    CallSeen,
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
        /// (dev) Answer the page's requests live on 127.0.0.1:<port>, as the window does — review the UI in a
        /// browser against real data (no IPC: clicks that send to the app do nothing)
        #[arg(long, hide = true)]
        serve: Option<u16>,
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

/// Name of the "transcription in progress" marker (a no-op name when built without whisper).
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
    // What a command an unattended run makes changes or sends is noted for its outcome (`notes::note_action`).
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let out = match Cli::parse().cmd {
        Cmd::Attention => {
            cli::print_json(&attention::peek());
            Ok(())
        }
        Cmd::Doctor { json, request, probe, open, no_ai } => permissions::doctor(json, request, probe, open.as_deref(), no_ai),
        Cmd::Model { cmd } => cli::app::model(cmd),
        Cmd::Config(args) => cli::app::config(args),
        Cmd::RunGuard => std::process::exit(guard::command()),
        Cmd::ViewHook => std::process::exit(view::hook()),
        Cmd::OnedriveHook => std::process::exit(onedrive::hook()),
        #[cfg(feature = "transcribe")]
        Cmd::GpuCheck => gpu::check(),
        #[cfg(feature = "transcribe")]
        Cmd::Server { cmd } => cli::app::server(cmd),
        #[cfg(feature = "transcribe")]
        Cmd::Transcribe { call, model, keep_audio } => cli::capture::transcribe(call, model, keep_audio),
        Cmd::Language { code, call } => cli::capture::language(&code, &call),
        #[cfg(feature = "transcribe")]
        Cmd::Speakers { call, names, relabel } => speakers::command(&call, &names, relabel),
        Cmd::Vocab { cmd } => cli::capture::vocab(cmd),
        #[cfg(feature = "brain")]
        Cmd::Brain { cmd } => cli::brain::brain_command(cmd),
        Cmd::Project { cmd } => cli::project::project_command(cmd),
        Cmd::Client { cmd } => cli::project::client_command(cmd),
        Cmd::Org { cmd } => cli::project::org_command(cmd),
        Cmd::Templates => cli::calls::templates(),
        Cmd::Needs { refresh } => cli::calendar::needs(refresh),
        Cmd::Widget { cmd } => cli::widget::widget(cmd),
        Cmd::Work { cmd } => cli::widget::work(cmd),
        Cmd::Today { thread, client, board, cmd } => cli::widget::today(thread.as_deref(), client.as_deref(), board.as_deref(), cmd),
        Cmd::Dashboard { cmd } => cli::widget::dashboard(cmd),
        Cmd::Series { cmd } => cli::calendar::series(cmd),
        Cmd::Plugin { cmd } => cli::plugin::plugin_command(cmd),
        Cmd::Deliver { cmd } => cli::m365::deliver_command(cmd),
        Cmd::M365 { cmd } => cli::m365::run(cmd),
        #[cfg(feature = "brain")]
        Cmd::Mcp { profile, client, thread, call, who } => cli::brain::mcp(profile, client, thread, call, who),
        Cmd::Read { link } => {
            print!("{}", links::read(&link)?);
            Ok(())
        }
        Cmd::Core { hook } => {
            if hook {
                std::process::exit(plugins::agent::hook());
            }
            print!("{}", plugins::agent::core());
            Ok(())
        }
        Cmd::PluginsHook => std::process::exit(plugins::agent::write_hook()),
        Cmd::Context { call } => calls::context(&call),
        Cmd::Title { call, title } => calls::set_title(&call, &title),
        Cmd::Classify => cli::calls::classify(),
        Cmd::Type { call, kind } => {
            println!("{}", serde_json::to_string(&calls::set_meeting_type(&call, &kind)?)?);
            Ok(())
        }
        Cmd::Note { text } => calls::note(&text.join(" ")),
        Cmd::Action { call, id, status } => calls::action_command(&call, &id, &status),
        Cmd::Actions { call, apply } => calls::actions_command(&call, apply.as_deref()),
        Cmd::Previous { subject } => calls::previous(&subject),
        Cmd::Delete { call } => calls::delete(&call),
        Cmd::Frames { call, keep } => calls::prune_frames(&call, &keep),
        Cmd::Html { call, open, auto } => minutes::command(&call, open, auto),
        Cmd::Compose { to, cc, subject, body_file, body, ai } => {
            let body = match (body, body_file) {
                (Some(b), _) => b,
                (None, Some(f)) => std::fs::read_to_string(&f).map_err(|e| anyhow::anyhow!("reading {}: {e}", f.display()))?,
                (None, None) => anyhow::bail!("give the mail's text: --body or --body-file"),
            };
            let v = drafts::compose(&to, &cc, &subject, &body, ai.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        Cmd::Me => {
            println!("{}", calls::ensure_me_profile()?.display());
            Ok(())
        }
        Cmd::Restore { name } => calls::restore(&name),
        #[cfg(feature = "transcribe")]
        Cmd::LiveReplay { wav, out } => live::replay(&wav, &out),
        Cmd::LiveMcp { call, profile, model, app } => livenotes::serve_agent(&call, &profile, &model, app),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::AxDump { seconds } => ax::dump(seconds),
        #[cfg(target_os = "macos")]
        Cmd::AxRecord { dir, seconds, full_only } => ax::record_probe(seconds, &dir, full_only),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::FrameGrab { out } => frames::grab(&out),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::AxMic { seconds } => ax::mic_probe(seconds),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::AxSpeaking { seconds, tree, all } => ax::speaking_probe(seconds, tree.as_deref(), all),
        #[cfg(feature = "transcribe")]
        Cmd::Speaking { call } => speaking::report(&call),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::CallSeen => cli::dev::call_seen(),
        #[cfg(any(target_os = "macos", windows))]
        Cmd::CallPeople { name } => cli::dev::call_people(name),
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
        Cmd::Window { selftest, export, serve } => cli::dev::window(selftest, export, serve),
        Cmd::Capture { app, seconds, title, meeting, languages } => cli::capture::capture(app, seconds, title, meeting, languages),
        Cmd::Meeting { cmd: cli::capture::MeetingCmd::Attach { call, file } } => meeting::attach(&call, &file),
        Cmd::Watch { app, no_tray } => app::run(app, !no_tray, false),
        Cmd::App { cmd } => cli::app::app(cmd),
        Cmd::Home { cmd } => cli::app::home(cmd),
        Cmd::Calendar { cmd } => cli::calendar::calendar(cmd),
        Cmd::Notes { call, wait, auto, session: _, profile, without_policy } => cli::calls::notes(call, wait, auto, profile, without_policy),
        Cmd::Ai { cmd } => cli::app::ai(cmd),
        Cmd::Live { cmd } => cli::capture::live(cmd),
        Cmd::Prep { call, upcoming, show, write } => cli::calendar::prep(call, upcoming, show, write),
        Cmd::Pending { session } => calls::pending(session.as_deref()),
        Cmd::Claim { call, session, session_id } => calls::claim(&call, session.as_deref(), session_id.as_deref()),
        Cmd::Claude { cmd } => cli::app::claude(cmd),
        Cmd::Events { follow } => cli::capture::events(follow),
        Cmd::Arm { title, languages, meeting, ai_profile } => cli::capture::arm(title, languages, meeting, ai_profile),
        Cmd::Join { meeting } => watch::join(meeting::MeetingContext::read_file(&meeting)?),
        Cmd::Stop => watch::stop(),
        Cmd::Memo { cmd } => cli::capture::memo(cmd),
        Cmd::Status => watch::status(),
        Cmd::Up => cli::app::up(),
        Cmd::Agenda { cmd } => cli::calendar::agenda(cmd),
    };
    // A change that failed is noted too, with why.
    notes::note_action(&argv, out.as_ref().err().map(|e| format!("failed: {e:#}")).as_deref());
    out
}

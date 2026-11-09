//! The app and its setup: run / start / quit, the data folder, settings, AI profiles, Claude Code, speech
//! models and the transcription server.

use anyhow::Result;
use clap::Subcommand;

#[cfg(feature = "transcribe")]
use crate::remote;
use crate::{ai, app, apps, calendar, claude_setup, config, links, models, paths, permissions, update, watch};

#[derive(Subcommand)]
pub enum AppCmd {
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
        #[arg(long, conflicts_with = "link")]
        call: Option<String>,
        /// An echo:// link (a call at a moment, a thread, a client, a person, a mail) — what the system calls
        #[arg(long)]
        link: Option<String>,
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
pub enum ClaudeCmd {
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
    /// Put Claude Code's folder on the PATH of new terminals (zsh / bash startup file; Windows: the user PATH)
    Path,
}

#[derive(Subcommand)]
pub enum HomeCmd {
    /// Use this folder for calls and settings (created if missing; restarts the app)
    Set { path: std::path::PathBuf },
    /// Pick the folder in a native dialog
    Choose,
}

#[derive(Subcommand)]
pub enum AiCmd {
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
pub enum ModelCmd {
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
    Remove { name: String },
}

#[cfg(feature = "transcribe")]
#[derive(Subcommand)]
pub enum ServerCmd {
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

pub fn app(cmd: Option<AppCmd>) -> Result<()> {
    match cmd {
        None => {
            println!("{}", serde_json::to_string_pretty(&app::info())?);
            Ok(())
        }
        Some(AppCmd::Run { app: target, no_tray, background }) => app::run(target, !no_tray, background),
        Some(AppCmd::Start) => app::start(),
        Some(AppCmd::Open { call, link }) => match link {
            Some(l) => {
                anyhow::ensure!(links::Link::parse(&l).is_some(), "not an echo:// link Echo knows: {l}");
                app::open_window(Some(l))
            }
            None => app::open_window(call),
        },
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
    }
}

pub fn home(cmd: Option<HomeCmd>) -> Result<()> {
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

/// `echoctl config`'s flags.
#[derive(clap::Args)]
pub struct ConfigArgs {
    /// The languages offered for a call, the first its default (Echo never detects one), comma-separated ISO codes (e.g. en,uk,ru); "any" clears them (English)
    #[arg(long, value_delimiter = ',')]
    pub languages: Option<Vec<String>>,
    /// Glossary for transcription, comma-separated (e.g. "Miro,ServiceNow,IPAM"); "" clears
    #[arg(long, value_delimiter = ',')]
    pub vocabulary: Option<Vec<String>>,
    /// Calls not on the calendar: "ask" (prompt when they start) or "manual" (only when
    /// you say record)
    #[arg(long, value_parser = ["ask", "manual"])]
    pub unscheduled: Option<String>,
    /// Keep call audio after transcription (default false: text only)
    #[arg(long)]
    pub keep_audio: Option<bool>,
    /// Also write the minutes page (<title>-minutes.html) when notes are written
    #[arg(long)]
    pub minutes_page: Option<bool>,
    /// Write a call's minutes by itself, 15 minutes after the call (or after your last note)
    #[arg(long)]
    pub auto_minutes: Option<bool>,
    /// Calendar meetings: "ask" (prompt ~1 min before) or "manual" (only when you record)
    #[arg(long, value_parser = ["ask", "manual"])]
    pub scheduled: Option<String>,
    /// Your name, used in notes ("Me")
    #[arg(long)]
    pub me_name: Option<String>,
    /// Your email, used to tell colleagues from external attendees
    #[arg(long)]
    pub me_email: Option<String>,
    /// The folder Echo runs Claude Code in for minutes (with the built-in AI profile its
    /// .claude/settings.json applies), and the only one `claim --for` accepts; "any" clears
    #[arg(long)]
    pub notes_in: Option<String>,
    /// Save snapshots of shared screens during calls (macOS; needs Screen Recording)
    #[arg(long)]
    pub capture_frames: Option<bool>,
    /// Record the call app's window text during calls (for speaker names)
    #[arg(long)]
    pub capture_ui: Option<bool>,
    /// Microsoft 365 app (client) id for the calendar sign-in; "default" = CodeMie
    #[arg(long)]
    pub graph_client_id: Option<String>,
    /// Entra tenant for the sign-in: common | organizations | <tenant id>
    #[arg(long)]
    pub graph_tenant: Option<String>,
    /// Model Claude Code uses when Echo writes notes: sonnet | opus | haiku | <model id> |
    /// default (Claude Code's own setting)
    #[arg(long)]
    pub notes_model: Option<String>,
    /// Model for live notes and quick questions during a call: haiku | <model id>
    #[arg(long)]
    pub live_model: Option<String>,
}

pub fn config(args: ConfigArgs) -> Result<()> {
    let ConfigArgs { languages, vocabulary, unscheduled, keep_audio, minutes_page, auto_minutes, scheduled, me_name, me_email, notes_in, capture_frames, capture_ui, graph_client_id, graph_tenant, notes_model, live_model } = args;
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

pub fn up() -> Result<()> {
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

pub fn ai(cmd: Option<AiCmd>) -> Result<()> {
    match cmd.unwrap_or(AiCmd::Status { id: None }) {
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
    }
}

pub fn claude(cmd: Option<ClaudeCmd>) -> Result<()> {
    match cmd {
        None => {
            // What Claude Code lists, and whether it really works: it runs, a terminal finds it.
            let mut v = app::claude_status();
            v["check"] = claude_setup::check();
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(())
        }
        Some(ClaudeCmd::Connect { scope, project }) => app::connect_claude(&scope, project.as_deref()),
        Some(ClaudeCmd::Path) => {
            println!("{} Open a new terminal to use `claude`.", claude_setup::add_to_path()?);
            Ok(())
        }
    }
}

pub fn model(cmd: ModelCmd) -> Result<()> {
    match cmd {
        ModelCmd::List { json } => models::list(json),
        ModelCmd::Download { name } => models::download(&name),
        ModelCmd::Use { name } => models::use_model(&name),
        ModelCmd::Remove { name } => models::remove(&name),
    }
}

#[cfg(feature = "transcribe")]
pub fn server(cmd: Option<ServerCmd>) -> Result<()> {
    match cmd.unwrap_or(ServerCmd::Status) {
        ServerCmd::Status => remote::print_status(),
        ServerCmd::Use { url, key, openai: true, model } => {
            let c = remote::connect_openai(&url, &key, model.as_deref(), "name one with --model")?;
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
    }
}

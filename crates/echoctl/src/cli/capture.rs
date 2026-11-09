//! Recording and transcription: arm, capture, memos, live notes, vocabulary.

use anyhow::Result;
use clap::Subcommand;

#[cfg(feature = "transcribe")]
use crate::transcribe;
use crate::{ai, apps, calls, config, livenotes, meeting, memo, notes, recorder, vocab, watch};

#[derive(Subcommand)]
pub enum MemoCmd {
    /// Record a memo now (the default microphone; nothing else records meanwhile)
    Start {
        /// Pin its language(s), e.g. "uk" or "en,uk" (default: the configured list)
        #[arg(long)]
        languages: Option<String>,
        /// The AI profile for its minutes (default: the default profile)
        #[arg(long)]
        ai: Option<String>,
    },
    /// Stop the memo (the same as `echoctl stop`)
    Stop,
    /// A memo's parts by thread, from its minutes run: a JSON file of [{thread, from, to}] (seconds or mm:ss)
    Threads {
        call: String,
        #[arg(long)]
        file: std::path::PathBuf,
    },
}

#[derive(Subcommand)]
pub enum LiveCmd {
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
pub enum VocabCmd {
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
pub enum MeetingCmd {
    /// Attach context to a call (writes <call>/meeting.json). FILE is JSON or "-" for stdin:
    /// {"subject", "start", "end", "organizer": {"name","email"}, "attendees": [{"name","email"}],
    ///  "join_url", "vocabulary": [..], "source"} — every field optional.
    Attach {
        /// Call folder (path or name under calls/), or "latest"
        call: String,
        file: std::path::PathBuf,
    },
}

pub fn arm(title: Option<String>, languages: Option<Vec<String>>, meeting: Option<std::path::PathBuf>, ai_profile: Option<String>) -> Result<()> {
    // Typed by hand: a misspelt profile is refused here (arming never checks it itself).
    if let Some(id) = &ai_profile {
        ai::Settings::load()?.get(id)?;
    }
    let ctx = meeting.as_deref().map(meeting::MeetingContext::read_file).transpose()?;
    watch::arm(title, ctx, languages, ai_profile)
}

pub fn capture(app: apps::TargetApp, seconds: Option<u64>, title: Option<String>, meeting: Option<std::path::PathBuf>, languages: Option<Vec<String>>) -> Result<()> {
    let ctx = meeting.as_deref().map(meeting::MeetingContext::read_file).transpose()?;
    recorder::capture(app, seconds, title.as_deref(), ctx, languages)
}

pub fn memo(cmd: MemoCmd) -> Result<()> {
    match cmd {
        MemoCmd::Start { languages, ai } => {
            if std::env::var_os("ECHOCTL_HEADLESS").is_some() {
                anyhow::bail!("refused inside a run Echo started");
            }
            watch::memo_start(languages.map(|l| l.split(',').map(str::to_owned).collect()), ai)
        }
        MemoCmd::Stop => watch::stop(),
        MemoCmd::Threads { call, file } => {
            println!("{}", serde_json::to_string_pretty(&memo::set_threads(&call, &file)?)?);
            Ok(())
        }
    }
}

pub fn events(follow: bool) -> Result<()> {
    anyhow::ensure!(follow, "only `events --follow` is supported");
    watch::follow_events()
}

#[cfg(feature = "transcribe")]
pub fn transcribe(call: String, model: Option<String>, keep_audio: bool) -> Result<()> {
    // A run Echo started can't keep audio the user's setting deletes (text only by default).
    if let Some(why) = notes::refused_in_run(false, keep_audio, model.is_some()).filter(|_| notes::headless()) {
        anyhow::bail!(why);
    }
    transcribe::transcribe(&call, model.as_deref(), keep_audio)
}

pub fn live(cmd: LiveCmd) -> Result<()> {
    match cmd {
        LiveCmd::Ask { call, kind, text } => live_request(&call, "ask", livenotes::AskKind::parse(&kind), &text.join(" ")),
        LiveCmd::Test { call, ask, text } => {
            let dir = calls::resolve_call(&call)?;
            livenotes::test_once(&dir, ask.as_deref().and_then(livenotes::AskKind::parse).map(|k| (k, text.join(" "))))
        }
        LiveCmd::Pause { call } => live_request(&call, "pause", None, ""),
        LiveCmd::Resume { call } => live_request(&call, "resume", None, ""),
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
    }
}

pub fn vocab(cmd: VocabCmd) -> Result<()> {
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

/// `echoctl live ask|pause|resume <call>`: a request file the running app picks up.
fn live_request(call: &str, cmd: &str, kind: Option<livenotes::AskKind>, text: &str) -> Result<()> {
    let dir = calls::resolve_call(call)?;
    if crate::watch::running_watcher().is_none() {
        anyhow::bail!("Echo isn't running — start it with `echoctl app start`");
    }
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let req = serde_json::json!({ "call": name, "cmd": cmd, "kind": kind, "text": text });
    std::fs::create_dir_all(crate::paths::state_dir())?;
    let path = crate::paths::state_dir().join(format!("live-{}-{}.request", calls::stamp_of(&dir), chrono::Local::now().timestamp_millis()));
    crate::paths::write_atomic(&path, req.to_string().as_bytes())?;
    println!("sent to Echo");
    Ok(())
}

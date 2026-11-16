//! Meetings and the day: the Microsoft 365 calendar, the agenda, series rules, preps and Needs you now.

use anyhow::Result;
use clap::Subcommand;

use super::print_json;
use crate::{calendar, calls, graph, needs, prep, series, teams_transcript, watch};

#[derive(Subcommand)]
pub enum CalendarCmd {
    /// Sign in through the browser (uses `config --graph-client-id`, default CodeMie)
    Login,
    /// Forget the sign-in
    Logout,
    /// Sign in again, allowing Echo to read colleagues' photos (a sign-in of its own: the tenant may refuse it)
    LoginPhotos,
    /// (diagnostics) Fetch one colleague's photo by address into Echo's photo cache
    #[command(hide = true)]
    Photo { email: String },
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
pub enum AgendaCmd {
    /// Refresh today's agenda from the calendar (Echo's Microsoft 365 sign-in, else `calendar_command`, else the msgraph skill if installed); quiet no-op without
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
pub enum SeriesCmd {
    /// Every rule; with --thread, that thread's series (from its calls and its calendar) with theirs
    List {
        #[arg(long)]
        thread: Option<String>,
    },
    /// Change a series' rule: KEY as `series list` gives it (id:… or subject:…); JSON with the fields to set —
    /// record (always|ask|never), languages; null = Echo's default
    Set {
        key: String,
        json: String,
        #[arg(long)]
        subject: Option<String>,
    },
}

pub fn calendar(cmd: Option<CalendarCmd>) -> Result<()> {
    match cmd {
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
        Some(CalendarCmd::LoginPhotos) => {
            let who = graph::login_photos()?;
            println!("signed in as {who}; photos {}", if graph::photos_allowed() { "allowed" } else { "not allowed by the tenant" });
            Ok(())
        }
        Some(CalendarCmd::Photo { email }) => {
            let out = crate::photos::file(&email);
            std::fs::create_dir_all(out.parent().unwrap_or(std::path::Path::new(".")))?;
            { print_json(&serde_json::json!({ "email": email, "allowed": graph::photos_allowed(), "got": graph::photo(&email, &out)?, "file": out })); Ok(()) }
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
                    "{}  {}{}  ({} attendees{}; you: {})",
                    e["start"]["dateTime"].as_str().unwrap_or("?").get(..16).unwrap_or("?"),
                    e["subject"].as_str().unwrap_or("(no subject)"),
                    if e["isCancelled"] == true { " [cancelled]" } else { "" },
                    e["attendees"].as_array().map(|a| a.len()).unwrap_or(0),
                    if online { ", Teams" } else { "" },
                    // The user's answer to the invite: Today and Coming up today list only accepted / organizer.
                    e["responseStatus"]["response"].as_str().unwrap_or("not said")
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
    }
}

pub fn agenda(cmd: AgendaCmd) -> Result<()> {
    match cmd {
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
    }
}

pub fn series(cmd: SeriesCmd) -> Result<()> {
    let v = match cmd {
        SeriesCmd::List { thread: Some(t) } => series::of_thread(&t)?,
        SeriesCmd::List { thread: None } => serde_json::to_value(series::load())?,
        SeriesCmd::Set { key, json, subject } => {
            anyhow::ensure!(std::env::var_os("ECHOCTL_HEADLESS").is_none(), "not inside a run Echo started");
            let patch: serde_json::Value = anyhow::Context::context(serde_json::from_str(&json), "the change isn't JSON")?;
            serde_json::json!({"rule": series::set(&key, subject.as_deref(), &patch)?})
        }
    };
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

pub fn prep(call: String, upcoming: bool, show: Option<String>, write: Option<String>) -> Result<()> {
    let v = if upcoming {
        prep::upcoming()
    } else if let Some(id) = show {
        prep::load(&id).ok_or_else(|| anyhow::anyhow!("no prep {id:?}"))?
    } else if let Some(id) = write {
        anyhow::ensure!(std::env::var_os("ECHOCTL_HEADLESS").is_none(), "not inside a run Echo started");
        prep::write(&prep::meeting(&id).ok_or_else(|| anyhow::anyhow!("that meeting isn't in today's calendar"))?)?
    } else {
        calls::prep(&calls::resolve_call(&call)?)
    };
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

pub fn needs(refresh: bool) -> Result<()> {
    if refresh {
        needs::refresh()?;
    }
    println!("{}", serde_json::to_string_pretty(&needs::view())?);
    Ok(())
}

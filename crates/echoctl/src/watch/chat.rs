//! The watcher: a call's meeting chat, captured and fetched (split out of `watch.rs`, moved as it was).

use super::*;

/// Follow the meeting's Teams chat during the call (Microsoft 365 sign-in, `Chat.Read`): what is
/// written but not said — links, files, questions. `<call>/chat.json`, rewritten when it
/// changes; one last read 60 s after the call for late messages. Config `capture_chat`.
pub(super) fn start_chat_capture(dir: &std::path::Path, started: DateTime<Local>, meeting: Option<&MeetingContext>) -> Option<Arc<AtomicBool>> {
    if !crate::config::Config::load().map(|c| c.capture_chat.unwrap_or(true)).unwrap_or(true) || crate::graph::account().is_none() {
        return None;
    }
    let known = meeting.and_then(|m| m.join_url.as_deref()).and_then(crate::graph::chat_id_from_join_url);
    // Only what was written around the call: a recurring meeting keeps one chat for all its
    // occurrences, and a group chat goes on for months.
    let scheduled = meeting.and_then(meeting_start).map(|s| s.with_timezone(&Local));
    let alive = Arc::new(AtomicBool::new(true));
    let a = alive.clone();
    let dir = dir.to_path_buf();
    std::thread::spawn(move || {
        let wait = |a: &AtomicBool, d: Duration| {
            for _ in 0..(d.as_millis() / 250) {
                if !a.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
        };
        // Which chat: the meeting's (join link), or for an ad-hoc call the chat whose "call
        // started" event matches — looked for during the first minutes (the event can lag).
        let (chat_id, call_start) = match known {
            Some(id) => (id, scheduled.map(|s| s.min(started)).unwrap_or(started)),
            None => {
                let t0 = Instant::now();
                let mut began = None;
                loop {
                    // Found by name (`find_call_people`): a call started from a chat.
                    if let Some(id) = fs::read_to_string(dir.join("chat.id")).ok().filter(|i| !i.trim().is_empty()) {
                        break (id.trim().to_string(), started);
                    }
                    // When the call began by Teams' timer: without it a meeting left hours ago
                    // can pass for this call. Not readable yet ⇒ look again next time.
                    if began.is_none() && crate::config::reads_call_window() {
                        began = crate::ax::call_elapsed().map(|e| Utc::now() - chrono::Duration::seconds(e as i64));
                    }
                    if began.is_some() || t0.elapsed() > CHAT_POLL {
                        match crate::graph::find_call_chat(started.with_timezone(&Utc), began, left_a_call_before(started.with_timezone(&Utc))) {
                            Ok(Some((id, at))) => break (id, at.with_timezone(&Local).min(started)),
                            Ok(None) => {}
                            Err(e) => eprintln!("call chat: {e:#}"),
                        }
                    }
                    if t0.elapsed() > CHAT_FIND_FOR || !a.load(Ordering::SeqCst) {
                        return;
                    }
                    wait(&a, CHAT_POLL);
                }
            }
        };
        let _ = fs::write(dir.join("chat.id"), &chat_id);
        let since = (call_start - CHAT_MARGIN).with_timezone(&Utc);
        let mut last = String::new();
        while a.load(Ordering::SeqCst) {
            if let Err(e) = write_chat(&dir, &chat_id, since, Utc::now(), started, &mut last) {
                eprintln!("meeting chat: {e:#}");
            }
            wait(&a, CHAT_POLL);
        }
        // The call ended: one last read for messages sent right after it.
        let ended = Utc::now();
        std::thread::sleep(Duration::from_secs(60));
        if let Err(e) = write_chat(&dir, &chat_id, since, ended + CHAT_MARGIN, started, &mut last) {
            eprintln!("meeting chat: {e:#}");
        }
    });
    Some(alive)
}

/// Read the chat and rewrite `<dir>/chat.json` if it changed; returns the message count.
pub(super) fn write_chat(
    dir: &std::path::Path,
    chat_id: &str,
    since: DateTime<Utc>,
    until: DateTime<Utc>,
    started: DateTime<Local>,
    last: &mut String,
) -> Result<usize> {
    let msgs = crate::graph::chat_messages(chat_id, since, until)?;
    let lines: Vec<serde_json::Value> = msgs.iter().map(|m| chat_line(m, started)).collect();
    let text = serde_json::to_string_pretty(&lines)?;
    if text != *last {
        fs::write(dir.join("chat.json"), &text)?;
        *last = text;
    }
    Ok(lines.len())
}

/// `echoctl calendar chat <call>`: fetch the chat of a recorded call (backfill). A calendar
/// meeting's chat comes from its join link (looked up in that day's calendar if missing); an
/// ad-hoc call's from the chat whose "call started" event matches the recording.
pub fn fetch_call_chat(call: &str) -> Result<()> {
    let dir = crate::calls::resolve_call(call)?;
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json"))?)?;
    let time = |k: &str| meta[k].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok());
    let started = time("started_at").map(|d| d.with_timezone(&Local)).unwrap_or_else(Local::now);
    let ended = time("ended_at").map(|d| d.with_timezone(&Utc)).unwrap_or_else(Utc::now);
    let mut meeting = MeetingContext::load(&dir)?;
    if let Some(m) = meeting.as_mut().filter(|m| m.join_url.is_none()) {
        // Attached without a link: find the call's calendar event (same day, same subject).
        let day = started.date_naive();
        let range = crate::calendar::local_midnight(day).zip(day.succ_opt().and_then(crate::calendar::local_midnight));
        let subject = m.subject.clone().unwrap_or_default().to_lowercase();
        if let Some((from, to)) = range {
            let events = crate::graph::calendar_view(from, to)?;
            m.join_url = events
                .iter()
                .find(|e| e["subject"].as_str().is_some_and(|s| s.trim().to_lowercase() == subject.trim()))
                .and_then(|e| e["onlineMeeting"]["joinUrl"].as_str().map(str::to_owned));
            if m.join_url.is_some() {
                m.save(&dir)?;
            }
        }
    }
    let linked = meeting.as_ref().and_then(|m| m.join_url.as_deref()).and_then(crate::graph::chat_id_from_join_url);
    let (chat_id, call_start) = match linked {
        Some(id) => {
            let scheduled = meeting.as_ref().and_then(meeting_start).map(|s| s.with_timezone(&Local));
            (id, scheduled.map(|s| s.min(started)).unwrap_or(started))
        }
        None => match fs::read_to_string(dir.join("chat.id")).ok().map(|i| i.trim().to_string()).filter(|i| !i.is_empty()) {
            Some(id) => (id, started),
            None => match chat_by_title(&dir)? {
                Some(id) => (id, started),
                None => {
                    let began = crate::ax::call_began_in(&dir).map(|s| started.with_timezone(&Utc) + chrono::Duration::milliseconds((s * 1000.0) as i64));
                    match crate::graph::find_call_chat(started.with_timezone(&Utc), began, left_a_call_before(started.with_timezone(&Utc)))? {
                        Some((id, at)) => (id, at.with_timezone(&Local).min(started)),
                        None => bail!("no chat found for this call (no meeting link, and no chat had a call running at {})", started.format("%H:%M")),
                    }
                }
            },
        },
    };
    let _ = fs::write(dir.join("chat.id"), &chat_id);
    let since = (call_start - CHAT_MARGIN).with_timezone(&Utc);
    let n = write_chat(&dir, &chat_id, since, ended + CHAT_MARGIN, started, &mut String::new())?;
    println!("{n} chat message(s) from {} to {} → {}", since.with_timezone(&Local).format("%H:%M"), (ended + CHAT_MARGIN).with_timezone(&Local).format("%H:%M"), dir.join("chat.json").display());
    Ok(())
}

/// When you last left a call before `t`, from the watcher's `call_started` / `call_ended`
/// events: the last end followed by a real gap (≥ 60 s, not a dropout it rejoined).
pub(super) fn left_a_call_before(t: DateTime<Utc>) -> Option<DateTime<Utc>> {
    let text = fs::read_to_string(file("events.jsonl")).ok()?;
    let marks: Vec<(DateTime<Utc>, bool)> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| {
            let ended = match v["event"].as_str()? {
                "call_started" => false,
                "call_ended" => true,
                _ => return None,
            };
            Some((DateTime::parse_from_rfc3339(v["ts"].as_str()?).ok()?.with_timezone(&Utc), ended))
        })
        .filter(|(at, _)| *at < t)
        .collect();
    left_from(&marks)
}

pub(super) fn left_from(marks: &[(DateTime<Utc>, bool)]) -> Option<DateTime<Utc>> {
    marks.iter().enumerate().rev().find_map(|(i, (at, ended))| {
        let next_start = marks[i + 1..].iter().find(|m| !m.1).map(|m| m.0);
        (*ended && next_start.is_some_and(|n| n - *at >= chrono::Duration::seconds(60))).then_some(*at)
    })
}

/// Backfill for an ad-hoc call: the chat named in the recorded call window's title (what
/// `find_call_people` does during the call), saving the people it finds if the call has none.
pub(super) fn chat_by_title(dir: &std::path::Path) -> Result<Option<String>> {
    let tree: Option<serde_json::Value> = fs::read(dir.join("ui-tree-10s.json")).ok().and_then(|b| serde_json::from_slice(&b).ok());
    let title = tree.as_ref().and_then(|t| t["windows"][0]["Title"].as_str().or(t["windows"][0]["Name"].as_str()));
    let Some(name) = title.and_then(crate::ax::call_name) else { return Ok(None) };
    let Some((id, topic, attendees)) = crate::graph::chat_named(&name)? else { return Ok(None) };
    if !dir.join(crate::meeting::FILE).exists() {
        MeetingContext { subject: topic, attendees, source: Some("teams-call".into()), ..Default::default() }.save(dir)?;
    }
    Ok(Some(id))
}

/// One chat message as the notes and the pane read it: `t` = seconds from the recording start
/// (negative before it), sender, text, attached files.
pub(super) fn chat_line(m: &serde_json::Value, started: DateTime<Local>) -> serde_json::Value {
    let at = m["createdDateTime"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok());
    let body = m["body"]["content"].as_str().unwrap_or("");
    let text = if m["body"]["contentType"] == "html" { crate::graph::html_to_text(body) } else { body.trim().to_string() };
    let files: Vec<&str> = m["attachments"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).collect();
    // No user: an app or a bot (Teams' Facilitator) — kept, by its name, never a person.
    let user = m["from"]["user"]["displayName"].as_str();
    let mut line = json!({
        "t": at.map(|a| (a.with_timezone(&Local) - started).num_seconds()),
        "at": m["createdDateTime"],
        "from": user.or(m["from"]["application"]["displayName"].as_str()).unwrap_or("someone"),
        "text": text,
        "files": files,
    });
    if user.is_none() {
        line["bot"] = json!(true);
    }
    line
}

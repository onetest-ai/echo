//! A call as an evidence item of a thread's work ledger (`ledger.rs`): its transcript as turns, its chat and the
//! user's notes as text, and an item as the readable `.md` beside its JSON.

use std::fs;
use std::path::Path;

use chrono::Local;
use serde_json::{json, Value};

use crate::calls;

pub fn render_md(x: &Value) -> String {
    let mut head = vec![format!("# {}", x["title"].as_str().unwrap_or("")), format!("evidence: {} · {} · {}", x["id"].as_str().unwrap_or(""), x["date"].as_str().unwrap_or(""), x["kind"].as_str().unwrap_or(""))];
    if let Some(p) = x["people"].as_array().filter(|p| !p.is_empty()) {
        head.push(format!("people: {}", p.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
    }
    // A reply to mail the user sent from an item (`ledger::mail_sync`): which item it answers.
    if let Some(n) = x["answers"]["node"].as_str() {
        head.push(format!("answers: {n} “{}” (a reply to mail Me sent from it)", x["answers"]["title"].as_str().unwrap_or("")));
    }
    let mut body: Vec<String> = x["turns"].as_array().map(|a| a.iter().map(|t| format!("[{}] {}: {}", t["ts"].as_str().unwrap_or(""), t["speaker"].as_str().unwrap_or("?"), t["text"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
    if let Some(t) = x["text"].as_str().filter(|t| !t.is_empty()) {
        body.push(t.to_string());
    }
    format!("{}\n\n{}\n", head.join("\n"), body.join("\n\n"))
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A call as an evidence item: its transcript (turns), chat and the user's notes (text). A memo split by thread
/// (`memo.rs`) gives `thread` only its part: what the user said about another thread never becomes this one's work.
pub fn call_item(dir: &Path, thread: Option<&str>) -> Option<Value> {
    let meta = calls::read_json(&dir.join("meta.json"));
    let started = meta["started_at"].as_str()?.to_string();
    let date = chrono::DateTime::parse_from_rfc3339(&started).ok()?.with_timezone(&Local).date_naive().to_string();
    let segs = calls::read_json(&dir.join("segments.json"));
    // A memo split by thread: this thread's parts only. A thread the split didn't name gets the whole memo only when
    // the user put it there by hand; one it reaches through its rules gets nothing.
    let spans = match thread.and_then(|t| crate::memo::parts_for(dir, t).map(|s| (t, s))) {
        Some((t, s)) if s.is_empty() && !crate::memo::whole_for(dir, t) => return None,
        Some((_, s)) if !s.is_empty() => Some(s),
        _ => None,
    };
    let within = |s: &Value| {
        let at = s["start"].as_f64().unwrap_or(0.0);
        spans.as_ref().is_none_or(|sp| sp.iter().any(|&(a, b)| at >= a && at < b))
    };
    let turns: Vec<Value> = segs
        .as_array()?
        .iter()
        .filter(|s| s["text"].as_str().is_some_and(|t| !t.trim().is_empty()) && within(s))
        .map(|s| {
            let who = match s["speaker"].as_str() {
                Some("me") | Some("mic") => "Me".to_string(),
                _ => s["name"].as_str().unwrap_or("Remote").to_string(),
            };
            json!({"ts": clock(s["start"].as_f64().unwrap_or(0.0)), "speaker": who, "text": s["text"].as_str().unwrap_or("").trim()})
        })
        .collect();
    if turns.is_empty() {
        return None;
    }
    let mut text = vec![];
    if let Some(chat) = calls::read_json(&dir.join("chat.json")).as_array().filter(|c| !c.is_empty()) {
        text.push("## Chat during the call".to_string());
        for m in chat {
            text.push(format!("{}: {}", m["from"].as_str().unwrap_or("?"), m["text"].as_str().unwrap_or("")));
        }
    }
    if let Ok(n) = fs::read_to_string(dir.join(calls::USER_NOTES)) {
        if !n.trim().is_empty() {
            text.push(format!("## The user's own notes\n{}", n.trim()));
        }
    }
    let mut people: Vec<String> = vec![];
    for p in crate::meeting::MeetingContext::load(dir).ok().flatten().map(|m| m.organizer.into_iter().chain(m.attendees).collect::<Vec<_>>()).unwrap_or_default() {
        let name = p.name.trim().trim_end_matches("(external)").trim().to_string();
        if !name.is_empty() && !people.contains(&name) {
            people.push(name);
        }
    }
    let stamp = calls::stamp_of(dir);
    if crate::memo::is_memo(dir) {
        text.insert(0, format!("## A memo\nThe user alone, thinking aloud — not a meeting: what they say they will do is theirs (\"Me\") unless \
                                they name someone.{}", if spans.is_some() { " Only its part about this thread is here." } else { "" }));
    }
    Some(json!({
        "id": stamp, "kind": "meeting", "source": "call", "at": started, "date": date,
        "title": meta["title"].as_str().unwrap_or(&stamp), "call": dir.file_name().map(|n| n.to_string_lossy().into_owned()),
        "people": people, "turns": turns, "text": text.join("\n"),
    }))
}

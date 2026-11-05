//! The Today page: your day (today's meetings, each with its prep or its call), your work (from Tasks) and cards
//! delivered to Echo itself — `deliver` kind `today`: a routine's or a plugin's result lands here as a card (the
//! morning brief), one per source (its `about`, else its title; a new one replaces the old), hidden by Got it until
//! the next one from that source. `<home>/.echo/today-cards.json`.

// The page is the window's (`tray`); the delivery kind works without it.
#![cfg_attr(not(feature = "tray"), allow(dead_code))]

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use serde_json::{json, Value};

use crate::meeting::MeetingContext;
use crate::{calls, paths};

/// At most this many cards are kept (one per source).
const MAX_CARDS: usize = 20;

fn cards_file() -> PathBuf {
    paths::state_dir().join("today-cards.json")
}

fn load_cards() -> Vec<Value> {
    fs::read(cards_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_cards(cards: &[Value]) -> Result<()> {
    fs::create_dir_all(paths::state_dir())?;
    paths::write_atomic(&cards_file(), &serde_json::to_vec_pretty(cards)?)?;
    Ok(())
}

/// A card's source: what sent it (a routine's `about` line), else its title.
fn key_of(title: &str, about: Option<&str>) -> String {
    about.filter(|a| !a.trim().is_empty()).unwrap_or(title).trim().chars().take(200).collect()
}

/// `deliver::send("today", …)`: the message becomes this source's card on Today.
pub fn put_card(m: &crate::deliver::Msg) -> Result<()> {
    let key = key_of(m.title, m.about);
    let mut cards = load_cards();
    cards.retain(|c| c["key"].as_str() != Some(key.as_str()));
    cards.insert(0, json!({
        "key": key, "title": m.title.chars().take(200).collect::<String>(), "markdown": m.markdown.chars().take(20_000).collect::<String>(),
        "about": m.about, "ai": m.ai, "at": Local::now().to_rfc3339(), "seen": false,
    }));
    cards.truncate(MAX_CARDS);
    save_cards(&cards)
}

/// Got it: the card is hidden until its source sends another.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn seen(key: &str) -> Result<()> {
    let mut cards = load_cards();
    for c in cards.iter_mut().filter(|c| c["key"].as_str() == Some(key)) {
        c["seen"] = json!(true);
    }
    save_cards(&cards)
}

/// The cards still to read, rendered by Echo's safe writer (a model wrote them: the AI notice goes with them).
fn cards() -> Vec<Value> {
    load_cards()
        .into_iter()
        .filter(|c| c["seen"] != true)
        .map(|c| {
            let html = crate::minutes::markdown_html(c["markdown"].as_str().unwrap_or(""));
            let notice = c["ai"].as_str().map(crate::deliver::ai_notice);
            json!({"key": c["key"], "title": c["title"], "html": html, "about": c["about"], "at": c["at"], "notice": notice})
        })
        .collect()
}

fn seen_file() -> PathBuf {
    paths::state_dir().join("minutes-seen.json")
}

/// When each call's minutes were last opened in the window: the minutes file's time then (`stamp` → seconds).
fn seen_map() -> serde_json::Map<String, Value> {
    fs::read(seen_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn notes_time(dir: &std::path::Path) -> Option<u64> {
    fs::metadata(dir.join("notes.md")).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs())
}

/// The window opened a call with its minutes: they are read (until they are written again).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn minutes_seen(dir: &std::path::Path) -> Result<()> {
    let Some(t) = notes_time(dir) else { return Ok(()) };
    let mut m = seen_map();
    m.insert(calls::stamp_of(dir), json!(t));
    // Only this month's (a stamp starts with its date).
    let keep = (Local::now() - chrono::Duration::days(31)).format("%Y-%m-%d").to_string();
    m.retain(|k, _| k.as_str() >= keep.as_str());
    fs::create_dir_all(paths::state_dir())?;
    paths::write_atomic(&seen_file(), &serde_json::to_vec(&m)?)?;
    Ok(())
}

/// What the minutes hold, for a glance: decisions and open questions (bullets under those headings) and actions.
fn counts(dir: &std::path::Path) -> Value {
    let md = fs::read_to_string(dir.join("notes.md")).unwrap_or_default();
    let mut section = String::new();
    let (mut decisions, mut questions) = (0, 0);
    for line in md.lines() {
        let l = line.trim_start();
        if let Some(h) = l.strip_prefix("## ") {
            section = h.to_lowercase();
        } else if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("| ") && !l.contains("---") {
            if section.contains("decision") {
                decisions += 1;
            } else if section.contains("question") {
                questions += 1;
            }
        }
    }
    let actions = calls::read_json(&dir.join(calls::ACTIONS)).as_array().map_or(0, Vec::len);
    json!({"decisions": decisions, "actions": actions, "questions": questions})
}

/// A call's minutes as Today shows them: being written, coming at a time, written (new until opened, with what they
/// hold), or failed (with why).
fn minutes(dir: &std::path::Path) -> Value {
    let written = dir.join("notes.md").exists();
    let run = crate::notes::status(dir);
    if let Some(r) = run.as_ref().filter(|r| r.state == "queued" || r.state == "running") {
        let _ = r;
        return json!({"state": "writing"});
    }
    if written {
        let unread = notes_time(dir).is_some_and(|t| seen_map().get(&calls::stamp_of(dir)).and_then(Value::as_u64) != Some(t));
        return json!({"state": "ready", "unread": unread, "counts": counts(dir)});
    }
    if let Some(r) = run.as_ref().filter(|r| r.state == "failed" || r.state == "needs_login") {
        return json!({"state": "failed", "why": r.message});
    }
    let auto = crate::notes::auto_state(dir);
    match auto["due"].as_str() {
        Some("at") => json!({"state": "at", "at": auto["at"]}),
        Some("now") => json!({"state": "writing"}),
        Some("not_ready") => json!({"state": "transcribing"}),
        _ => json!({"state": "none"}),
    }
}

fn time(s: Option<&str>) -> Option<DateTime<Utc>> {
    s.and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc))
}

/// Today's meetings and calls in time order: a meeting of the calendar with its prep and whether it is set to be
/// recorded or was (its call); a call of today with no meeting on its own.
fn day() -> Vec<Value> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let calls_today: Vec<(PathBuf, Value, Option<MeetingContext>)> = calls::all_calls()
        .into_iter()
        .filter(|d| calls::stamp_of(d).starts_with(&today))
        .map(|d| {
            let meta = calls::read_json(&d.join("meta.json"));
            let m = MeetingContext::load(&d).ok().flatten();
            (d, meta, m)
        })
        .collect();
    let scheduled: Vec<String> = crate::schedule::load().into_iter().filter_map(|m| m.id).collect();
    let mut used = vec![false; calls_today.len()];
    let mut out: Vec<Value> = vec![];
    for m in crate::watch::agenda() {
        let Some(event) = m.id.clone().filter(|i| !i.is_empty()) else { continue };
        let hit = calls_today.iter().position(|(_, _, cm)| cm.as_ref().and_then(|c| c.id.as_deref()) == Some(event.as_str()));
        if let Some(i) = hit {
            used[i] = true;
        }
        let pid = crate::prep::id_of(&event);
        let minutes = hit.map(|i| minutes(&calls_today[i].0));
        out.push(json!({
            "kind": "meeting", "subject": m.subject, "start": m.start, "end": m.end, "event": event, "minutes": minutes,
            "people": m.attendees.len() + m.organizer.iter().count(),
            "threads": crate::projects::for_event(&m),
            "call": hit.map(|i| calls_today[i].0.file_name().unwrap_or_default().to_string_lossy().into_owned()),
            "scheduled": scheduled.contains(&event),
            "prep": {"id": pid, "ready": crate::prep::load(&pid).is_some(), "writing": crate::prep::writing(&pid)},
        }));
    }
    for (i, (d, meta, _)) in calls_today.iter().enumerate().filter(|(i, _)| !used[*i]) {
        let _ = i;
        out.push(json!({
            "kind": "call", "subject": meta["title"], "start": meta["started_at"], "end": meta["ended_at"], "minutes": minutes(d),
            "call": d.file_name().unwrap_or_default().to_string_lossy(),
        }));
    }
    out.sort_by_key(|v| time(v["start"].as_str()));
    out
}

/// Your work, from Tasks' piles — only what is due by the end of this week or late (the rest stays in Tasks): what you
/// owe, what is owed to you, what you wait for; late first, then by date.
fn work(tasks: &Value) -> Value {
    let today = Local::now().date_naive();
    let week_end = today + chrono::Duration::days(6 - i64::from(chrono::Datelike::weekday(&today).num_days_from_monday()));
    let this_week = |t: &Value| t["late"] == true || t["due"].as_str().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).is_some_and(|d| d <= week_end);
    let pile = |name: &str| -> Value {
        let mut all: Vec<&Value> = tasks["piles"][name].as_array().into_iter().flatten().filter(|t| this_week(t)).collect();
        all.sort_by_key(|t| (t["late"] != true, t["due"].as_str().unwrap_or("9999").to_string()));
        let items: Vec<Value> = all
            .iter()
            .map(|t| {
                json!({
                    "id": t["id"], "title": t["title"], "due": t["due"], "late": t["late"], "owner": t["owner"], "project": t["project"],
                    "waiting_for": t["waiting_for"], "story": t["story"]["title"],
                    "link": format!("echo://task/{}/{}", t["project"].as_str().unwrap_or(""), t["id"].as_str().unwrap_or("")),
                })
            })
            .collect();
        json!({"items": items, "total": items.len()})
    };
    json!({"mine": pile("mine"), "owed": pile("owed"), "waiting": pile("waiting"), "week_end": week_end.to_string()})
}

/// `/api/today`: the day, the work and the cards (`tasks`: the Tasks view, made by the window with its calendar).
pub fn view(tasks: &Value) -> Value {
    json!({"date": Local::now().to_rfc3339(), "day": day(), "work": work(tasks), "cards": cards()})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_per_source_replaced_by_the_next_and_hidden_by_got_it() {
        let (_env, _) = paths::testenv::folders("today-cards");
        let msg = |title: &'static str, md: &'static str| crate::deliver::Msg { title, markdown: md, about: Some("Routine · Morning brief"), ai: Some("sonnet"), ..Default::default() };
        put_card(&msg("Monday", "**First**")).unwrap();
        put_card(&msg("Tuesday", "Second")).unwrap();
        put_card(&crate::deliver::Msg { title: "Other", markdown: "x", ..Default::default() }).unwrap();
        let c = cards();
        assert_eq!(c.len(), 2, "one card per source");
        assert_eq!(c.iter().find(|x| x["key"] == "Routine · Morning brief").unwrap()["title"], "Tuesday");
        assert!(c[1]["html"].as_str().unwrap().contains("Second") || c[0]["html"].as_str().unwrap().contains("Second"));
        seen("Routine · Morning brief").unwrap();
        assert_eq!(cards().len(), 1);
        put_card(&msg("Wednesday", "Third")).unwrap();
        assert_eq!(cards().len(), 2, "the next card from that source shows again");
    }

    #[test]
    fn minutes_are_new_until_opened_and_say_what_they_hold() {
        let (_env, home) = paths::testenv::folders("today-minutes");
        let dir = home.join("calls").join("2026-11-05_100000-acme-sync");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.md"), "# Acme sync\n\n## Decisions\n- Pilot in Berlin\n- SSO first\n\n## Open questions\n- Who signs off?\n").unwrap();
        fs::write(dir.join(calls::ACTIONS), r#"[{"id": "a1", "task": "Send the diagram"}]"#).unwrap();
        let m = minutes(&dir);
        assert_eq!((m["state"].as_str(), m["unread"].as_bool()), (Some("ready"), Some(true)));
        assert_eq!(m["counts"], json!({"decisions": 2, "actions": 1, "questions": 1}));
        minutes_seen(&dir).unwrap();
        assert_eq!(minutes(&dir)["unread"], false);
    }

    #[test]
    fn work_is_this_weeks_and_late_tasks_late_first_with_their_links() {
        let today = Local::now().date_naive();
        let later = (today + chrono::Duration::days(30)).to_string();
        let tasks = json!({"piles": {"mine": [
            {"id": "T2", "project": "p", "title": "Today", "due": today.to_string()},
            {"id": "T1", "project": "p", "title": "Late", "late": true, "due": "2020-01-01"},
            {"id": "T3", "project": "p", "title": "Next month", "due": later},
            {"id": "T4", "project": "p", "title": "No date"}], "owed": [], "waiting": []}});
        let w = work(&tasks);
        let titles: Vec<&str> = w["mine"]["items"].as_array().unwrap().iter().map(|x| x["title"].as_str().unwrap()).collect();
        assert_eq!(titles, vec!["Late", "Today"], "this week's and late ones, late first");
        assert_eq!(w["mine"]["items"][0]["link"], "echo://task/p/T1");
        assert_eq!(w["owed"]["items"].as_array().unwrap().len(), 0);
    }
}

//! Calendar → agenda, deterministically (no model): `echoctl agenda sync`.
//!
//! Echo has no calendar code of its own. If a calendar command is available — by default
//! the user's msgraph skill CLI (`node ~/.claude/skills/msgraph/scripts/msgraph.js calendar
//! --limit 30 --json`), overridable with `config.calendar_command` — its JSON is filtered and
//! converted into `.echo/agenda.json`. Without one, sync is a quiet no-op.
//!
//! Also matches a recording to the agenda meeting it overlaps (used when recording starts).

use crate::app::NoWindow;
use std::path::PathBuf;
use std::process::Command;

use anyhow::Result;
use chrono::{DateTime, Duration, Local, NaiveDateTime, Utc};
use serde_json::{json, Value};

use crate::config::Config;
use crate::meeting::{MeetingContext, Person};
use crate::watch;

/// Offer window, as in the watcher: a meeting stays relevant until 4 min after its start.
const STILL_RELEVANT_AFTER_START: i64 = 4;
/// A recording matches a meeting from 10 min before its start until its end.
const MATCH_EARLY_MIN: i64 = 10;
/// …or until this long after its start (later only for meetings under 2 h).
const MATCH_LATE_MIN: i64 = 30;

fn default_command() -> Option<Vec<String>> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    let script = PathBuf::from(home).join(".claude/skills/msgraph/scripts/msgraph.js");
    script.exists().then(|| {
        vec![crate::app::find_program("node"), script.display().to_string(), "calendar".into(), "--limit".into(), "30".into(), "--json".into()]
    })
}


/// Graph time `{dateTime, timeZone}` (or a plain string) → UTC.
fn graph_time(v: &Value) -> Option<DateTime<Utc>> {
    let (s, tz) = match v {
        Value::String(s) => (s.as_str(), "UTC"),
        Value::Object(o) => (o.get("dateTime")?.as_str()?, o.get("timeZone").and_then(Value::as_str).unwrap_or("UTC")),
        _ => return None,
    };
    if let Ok(d) = DateTime::parse_from_rfc3339(s) {
        return Some(d.with_timezone(&Utc));
    }
    let naive = NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S%.f").ok()?;
    // Graph returns UTC unless asked otherwise; other zones are read as local time.
    Some(if tz.eq_ignore_ascii_case("UTC") { naive.and_utc() } else { naive.and_local_timezone(Local).single()?.with_timezone(&Utc) })
}

fn person(v: &Value) -> Option<Person> {
    let e = v.get("emailAddress").unwrap_or(v);
    let name = e.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let email = e.get("address").and_then(Value::as_str).map(str::to_owned);
    (!name.is_empty() || email.is_some()).then_some(Person { name, email })
}

/// One calendar event for the calendar view: times, flags and what the user can do with it.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn view_event(ev: &Value) -> Option<Value> {
    let subject = ev.get("subject").and_then(Value::as_str).unwrap_or("").to_string();
    let body = ev.pointer("/bodyPreview").and_then(Value::as_str).unwrap_or("");
    let start = graph_time(ev.get("start")?)?;
    let end = ev.get("end").and_then(graph_time).unwrap_or(start + Duration::minutes(30));
    let people = ev.get("attendees").and_then(Value::as_array).map(|a| {
        a.iter().filter(|x| x.get("type").and_then(Value::as_str) != Some("resource")).count()
    });
    Some(json!({
        "id": ev.get("id"),
        "subject": if subject.is_empty() { "(no subject)".to_string() } else { subject.clone() },
        "start": start.to_rfc3339(),
        "end": end.to_rfc3339(),
        "all_day": ev.get("isAllDay").and_then(Value::as_bool).unwrap_or(false),
        "online": ev.get("isOnlineMeeting").and_then(Value::as_bool).unwrap_or(false) || ev.pointer("/onlineMeeting/joinUrl").is_some(),
        "join_url": ev.pointer("/onlineMeeting/joinUrl"),
        "declined": ev.pointer("/responseStatus/response").and_then(Value::as_str) == Some("declined"),
        "cancelled": ev.get("isCancelled").and_then(Value::as_bool).unwrap_or(false) || subject.starts_with("Canceled:"),
        "no_notetakers": format!("{subject} {body}").to_lowercase().contains("no_notetakers"),
        "organizer": ev.pointer("/organizer/emailAddress/name"),
        "people": people,
        "location": ev.pointer("/location/displayName"),
    }))
}

/// One calendar event → an agenda meeting, or None if it should not be offered.
pub fn convert(ev: &Value, now: DateTime<Utc>) -> Option<MeetingContext> {
    let subject = ev.get("subject").and_then(Value::as_str).unwrap_or("").to_string();
    let body = ev.pointer("/bodyPreview").and_then(Value::as_str).unwrap_or("");
    let start = graph_time(ev.get("start")?)?;
    let end = ev.get("end").and_then(graph_time);
    let online = ev.get("isOnlineMeeting").and_then(Value::as_bool).unwrap_or(false)
        || ev.pointer("/onlineMeeting/joinUrl").is_some();
    let declined = ev.pointer("/responseStatus/response").and_then(Value::as_str) == Some("declined");
    let all_day = ev.get("isAllDay").and_then(Value::as_bool).unwrap_or(false);
    let cancelled = ev.get("isCancelled").and_then(Value::as_bool).unwrap_or(false) || subject.starts_with("Canceled:");
    let opted_out = format!("{subject} {body}").to_lowercase().contains("no_notetakers");
    let today = start.with_timezone(&Local).date_naive() == now.with_timezone(&Local).date_naive();
    // Kept until it ends: someone joining late still gets the meeting attached (offers stop
    // 4 min after the start on their own, in the watcher).
    let relevant = now < end.unwrap_or(start + Duration::minutes(STILL_RELEVANT_AFTER_START));
    if !online || declined || all_day || cancelled || opted_out || !today || !relevant {
        return None;
    }
    to_meeting(ev)
}

/// Any calendar event as meeting context (no filtering) — the calendar view and scheduled
/// recordings use it for days other than today.
pub fn to_meeting(ev: &Value) -> Option<MeetingContext> {
    let subject = ev.get("subject").and_then(Value::as_str).unwrap_or("").to_string();
    let start = graph_time(ev.get("start")?)?;
    let end = ev.get("end").and_then(graph_time);
    // People only: meeting rooms/equipment ("resource") and those who declined don't count.
    let attendees = ev
        .get("attendees")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter(|x| x.get("type").and_then(Value::as_str) != Some("resource"))
                .filter(|x| x.pointer("/status/response").and_then(Value::as_str) != Some("declined"))
                .filter_map(person)
                .collect()
        })
        .unwrap_or_default();
    Some(MeetingContext {
        id: ev.get("id").and_then(Value::as_str).map(str::to_owned),
        subject: Some(subject),
        series_id: ev.get("seriesMasterId").and_then(Value::as_str).map(str::to_owned),
        start: Some(start.to_rfc3339()),
        end: end.map(|e| e.to_rfc3339()),
        organizer: ev.get("organizer").and_then(person),
        attendees,
        join_url: ev.pointer("/onlineMeeting/joinUrl").and_then(Value::as_str).map(str::to_owned),
        response: ev.pointer("/responseStatus/response").and_then(Value::as_str).map(str::to_owned),
        vocabulary: vec![],
        source: Some("calendar".into()),
        languages: vec![],
        ai_profile: None,
    })
}

/// The start of `day` here, in UTC: local midnight (the earlier one when the clock goes back over
/// it, where the gap ends when it skips it). Days are counted on the calendar, never as 24 h: a day
/// the clocks change isn't 24 hours long.
pub fn local_midnight(day: chrono::NaiveDate) -> Option<DateTime<Utc>> {
    let midnight = day.and_hms_opt(0, 0, 0)?;
    match midnight.and_local_timezone(Local) {
        chrono::LocalResult::Single(d) => Some(d.with_timezone(&Utc)),
        // Not `earliest()`: on Unix chrono lists the smaller offset first, the later instant. And it
        // also counts the instant the clock goes back at midnight (24:00 → 23:00), whose clock shows
        // 23:00: keep only the candidates that read midnight, then the earlier one.
        chrono::LocalResult::Ambiguous(a, b) => {
            [a, b].into_iter().filter(|d| d.with_timezone(&Local).naive_local() == midnight).min().map(|d| d.with_timezone(&Utc))
        }
        chrono::LocalResult::None => None,
    }
}

/// `echoctl agenda sync`: refresh today's agenda from the calendar command, if any.
/// Prints one JSON line; never fails just because no calendar is available.
pub fn sync(quiet: bool) -> Result<()> {
    // The app's own Microsoft 365 sign-in, when there is one.
    if crate::graph::account().is_some() {
        let today = Local::now().date_naive();
        let (Some(from), Some(to)) = (local_midnight(today), today.succ_opt().and_then(local_midnight)) else { return Ok(()) };
        return match crate::graph::calendar_view(from, to) {
            Ok(events) => store(&events, quiet),
            Err(e) => {
                if !quiet {
                    println!("{}", json!({ "synced": false, "reason": format!("{e:#}") }));
                }
                Ok(())
            }
        };
    }
    let cfg = Config::load().unwrap_or_default();
    let cmd = cfg.calendar_command.clone().filter(|c| !c.is_empty()).or_else(default_command);
    let Some(cmd) = cmd else {
        if !quiet {
            println!("{}", json!({ "synced": false, "reason": "no calendar command (msgraph skill not installed)" }));
        }
        return Ok(());
    };
    let out = Command::new(&cmd[0]).no_window().args(&cmd[1..]).output();
    let events: Vec<Value> = match out {
        Ok(o) if o.status.success() => match serde_json::from_slice::<Value>(&o.stdout) {
            Ok(Value::Array(a)) => a,
            Ok(v) => v.get("value").and_then(Value::as_array).cloned().unwrap_or_default(),
            Err(_) => vec![],
        },
        Ok(o) => {
            if !quiet {
                let why = String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string();
                println!("{}", json!({ "synced": false, "reason": format!("calendar command failed: {why}") }));
            }
            return Ok(());
        }
        Err(e) => {
            if !quiet {
                println!("{}", json!({ "synced": false, "reason": e.to_string() }));
            }
            return Ok(());
        }
    };
    store(&events, quiet)
}

/// Filter + convert Graph-shaped events into the agenda.
fn store(events: &[Value], quiet: bool) -> Result<()> {
    let now = Utc::now();
    let mut meetings: Vec<MeetingContext> = events.iter().filter_map(|e| convert(e, now)).collect();
    meetings.sort_by(|a, b| a.start.cmp(&b.start));
    let n = meetings.len();
    watch::write_agenda(&meetings)?;
    // Meetings of a series set to record always: "Record when it starts", each once.
    if let Err(e) = crate::series::schedule_always(&meetings) {
        eprintln!("series: {e:#}");
    }
    if !quiet {
        println!("{}", json!({ "synced": true, "meetings": n }));
    }
    Ok(())
}

/// The agenda meeting a recording starting now belongs to (10 min early … end), if exactly
/// one matches; the one starting closest to now wins a tie.
pub fn meeting_now(now: DateTime<Utc>) -> Option<MeetingContext> {
    let mut hits: Vec<(i64, MeetingContext)> = watch::agenda()
        .into_iter()
        .filter_map(|m| {
            let start = watch::meeting_start(&m)?;
            let end = m.end.as_ref().and_then(|e| DateTime::parse_from_rfc3339(e).ok()).map(|e| e.with_timezone(&Utc))
                .unwrap_or(start + Duration::minutes(60));
            // A call matches a meeting it started near, or a short one still running — not a
            // long online block ("office hours", 9–17) that happens to span it.
            let near_start = now >= start - Duration::minutes(MATCH_EARLY_MIN) && now <= start + Duration::minutes(MATCH_LATE_MIN);
            let short_and_running = end - start <= Duration::hours(2) && now >= start && now <= end;
            (near_start || short_and_running).then(|| ((now - start).num_seconds().abs(), m))
        })
        .collect();
    hits.sort_by_key(|(d, _)| *d);
    hits.into_iter().next().map(|(_, m)| m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(subject: &str, start: DateTime<Utc>, online: bool) -> Value {
        json!({ "id": "e1", "subject": subject, "isOnlineMeeting": online,
                "start": { "dateTime": start.format("%Y-%m-%dT%H:%M:%S%.6f").to_string(), "timeZone": "UTC" },
                "end": { "dateTime": (start + Duration::minutes(30)).format("%Y-%m-%dT%H:%M:%S%.6f").to_string(), "timeZone": "UTC" },
                "organizer": { "emailAddress": { "name": "Daniel", "address": "d@x.com" } } })
    }

    #[test]
    fn keeps_upcoming_online_meetings_only() {
        let now = Local::now().with_timezone(&Utc);
        // keep a 1-minute margin so the test also passes just before midnight
        let soon = now + Duration::minutes(1);
        assert!(convert(&ev("Daily sync", soon, true), now).is_some());
        assert!(convert(&ev("Lunch", soon, false), now).is_none()); // not online
        assert!(convert(&ev("Canceled: TCC", soon, true), now).is_none());
        assert!(convert(&ev("Retro no_notetakers", soon, true), now).is_none());
        // Kept while it runs (late joiners still get it attached), dropped once it has ended.
        let running = now - Duration::minutes(10);
        if running.with_timezone(&Local).date_naive() == now.with_timezone(&Local).date_naive() {
            assert!(convert(&ev("Running", running, true), now).is_some());
        }
        assert!(convert(&ev("Over", now - Duration::minutes(40), true), now).is_none());
        let m = convert(&ev("Daily sync", soon, true), now).unwrap();
        assert_eq!(m.organizer.unwrap().name, "Daniel");
    }

    #[test]
    fn a_meeting_is_the_users_once_accepted() {
        let at = Utc::now();
        let with = |r: &str| {
            let mut e = ev("Sync", at, true);
            e["responseStatus"] = json!({"response": r});
            to_meeting(&e).unwrap()
        };
        assert!(with("accepted").accepted() && with("organizer").accepted());
        assert!(!with("tentativelyAccepted").accepted() && !with("notResponded").accepted() && !with("none").accepted());
        assert!(to_meeting(&ev("Sync", at, true)).unwrap().accepted(), "a source that doesn't say: kept");
    }

    #[test]
    fn graph_times_parse_as_utc() {
        let t = graph_time(&json!({ "dateTime": "2026-10-09T15:00:00.0000000", "timeZone": "UTC" })).unwrap();
        assert_eq!(t.to_rfc3339(), "2026-10-09T15:00:00+00:00");
    }
}

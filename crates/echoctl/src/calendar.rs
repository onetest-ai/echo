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
pub(crate) const MATCH_EARLY_MIN: i64 = 10;
/// …or until this long after its start (later only for meetings under 2 h).
const MATCH_LATE_MIN: i64 = 30;

/// The command that prints the calendar: the configured one, else the msgraph skill's CLI under `home` when installed.
/// Configured empty: no calendar (test data sets it, so a test run never reads the user's real meetings, #242).
fn calendar_command(configured: Option<Vec<String>>, home: Option<PathBuf>) -> Option<Vec<String>> {
    match configured {
        Some(c) => (!c.is_empty()).then_some(c),
        None => default_command(home?),
    }
}

fn default_command(home: PathBuf) -> Option<Vec<String>> {
    let script = home.join(".claude/skills/msgraph/scripts/msgraph.js");
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

/// The event's join link: Graph's own (a Teams online meeting), else the first link of a known meeting app in its
/// location, preview or body (#228: a Zoom or Google Meet invite has its link only there). `body` comes only in the
/// msgraph skill's JSON.
fn join_link(ev: &Value) -> Option<String> {
    if let Some(url) = ev.pointer("/onlineMeeting/joinUrl").and_then(Value::as_str) {
        return Some(url.to_owned());
    }
    let text = |p: &str| ev.pointer(p).and_then(Value::as_str);
    [text("/location/displayName"), text("/bodyPreview").map(whole_words), text("/body/content")].into_iter().flatten().find_map(crate::apps::find_join_link)
}

/// Graph cuts `bodyPreview` at 255 characters: a long one ends where it was cut, maybe inside a link (a Zoom id short
/// of its digits). Its last word is left out then.
fn whole_words(preview: &str) -> &str {
    const CUT_AT: usize = 250;
    // Counted as Graph does, in UTF-16 units (an emoji is two).
    if preview.encode_utf16().count() < CUT_AT {
        return preview;
    }
    preview.trim_end().rsplit_once(char::is_whitespace).map_or("", |(head, _)| head)
}

/// One calendar event for the calendar view: times, flags and what the user can do with it.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn view_event(ev: &Value) -> Option<Value> {
    let subject = ev.get("subject").and_then(Value::as_str).unwrap_or("").to_string();
    let body = ev.pointer("/bodyPreview").and_then(Value::as_str).unwrap_or("");
    let start = graph_time(ev.get("start")?)?;
    let end = ev.get("end").and_then(graph_time).unwrap_or(start + Duration::minutes(30));
    let people = ev.get("attendees").and_then(Value::as_array).map(|a| {
        a.iter()
            .filter(|x| x.get("type").and_then(Value::as_str) != Some("resource"))
            .filter(|x| x.pointer("/status/response").and_then(Value::as_str) != Some("declined"))
            .count()
    });
    // Who's coming, for the card (rooms and those who declined left out): their names as the invite has them, else
    // the address.
    let names: Vec<String> = ev.get("attendees").and_then(Value::as_array).into_iter().flatten()
        .filter(|x| x.get("type").and_then(Value::as_str) != Some("resource"))
        .filter(|x| x.pointer("/status/response").and_then(Value::as_str) != Some("declined"))
        .filter_map(|x| x.pointer("/emailAddress/name").or_else(|| x.pointer("/emailAddress/address")).and_then(Value::as_str))
        .map(str::trim).filter(|n| !n.is_empty()).take(50).map(str::to_owned).collect();
    let join_url = join_link(ev);
    Some(json!({
        "id": ev.get("id"),
        "subject": if subject.is_empty() { "(no subject)".to_string() } else { subject.clone() },
        "start": start.to_rfc3339(),
        "end": end.to_rfc3339(),
        "all_day": ev.get("isAllDay").and_then(Value::as_bool).unwrap_or(false),
        "online": ev.get("isOnlineMeeting").and_then(Value::as_bool).unwrap_or(false) || join_url.is_some(),
        "app": join_url.as_deref().and_then(crate::apps::MeetingApp::from_url).map(|a| a.key()),
        "join_url": join_url,
        "declined": ev.pointer("/responseStatus/response").and_then(Value::as_str) == Some("declined"),
        "cancelled": ev.get("isCancelled").and_then(Value::as_bool).unwrap_or(false) || subject.starts_with("Canceled:"),
        "no_notetakers": format!("{subject} {body}").to_lowercase().contains("no_notetakers"),
        "organizer": ev.pointer("/organizer/emailAddress/name"),
        "people": people,
        "names": names,
        "location": ev.pointer("/location/displayName"),
    }))
}

/// One calendar event → an agenda meeting, or None if it should not be offered.
pub fn convert(ev: &Value, now: DateTime<Utc>) -> Option<MeetingContext> {
    let subject = ev.get("subject").and_then(Value::as_str).unwrap_or("").to_string();
    let body = ev.pointer("/bodyPreview").and_then(Value::as_str).unwrap_or("");
    let start = graph_time(ev.get("start")?)?;
    let end = ev.get("end").and_then(graph_time);
    let online = ev.get("isOnlineMeeting").and_then(Value::as_bool).unwrap_or(false) || join_link(ev).is_some();
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
    // People only: meeting rooms/equipment ("resource") don't count, and those who declined are kept apart.
    let invited: Vec<&Value> = ev.get("attendees").and_then(Value::as_array).into_iter().flatten()
        .filter(|x| x.get("type").and_then(Value::as_str) != Some("resource"))
        .collect();
    let said = |x: &Value| x.pointer("/status/response").and_then(Value::as_str).map(str::to_owned);
    let (declined, attendees): (Vec<&Value>, Vec<&Value>) = invited.iter().partition(|x| said(x).as_deref() == Some("declined"));
    let answers = invited.iter()
        .filter_map(|x| Some((x.pointer("/emailAddress/address")?.as_str()?.to_lowercase(), said(x)?)))
        .collect();
    let invited_as = invited.iter()
        .filter_map(|x| Some((x.pointer("/emailAddress/address")?.as_str()?.to_lowercase(), x.get("type")?.as_str()?.to_owned())))
        .collect();
    Some(MeetingContext {
        id: ev.get("id").and_then(Value::as_str).map(str::to_owned),
        subject: Some(subject),
        series_id: ev.get("seriesMasterId").and_then(Value::as_str).map(str::to_owned),
        start: Some(start.to_rfc3339()),
        end: end.map(|e| e.to_rfc3339()),
        organizer: ev.get("organizer").and_then(person),
        attendees: attendees.into_iter().filter_map(person).collect(),
        declined: declined.into_iter().filter_map(person).collect(),
        answers,
        invited_as,
        join_url: join_link(ev),
        response: ev.pointer("/responseStatus/response").and_then(Value::as_str).map(str::to_owned),
        vocabulary: vec![],
        source: Some("calendar".into()),
        languages: vec![],
        ai_profile: None,
    })
}

/// The agenda offers a call in `app` a meeting to record now (from `before` its start until `after` it): one Echo can
/// record and of that app (or of none known), not a Zoom one, nor a Meet one for a Teams call — its arm can't start on
/// it, so that call is asked about as any call (#500).
pub fn offered_now(agenda: &[MeetingContext], app: crate::apps::TargetApp, now: DateTime<Utc>, before: Duration, after: Duration) -> bool {
    agenda.iter().filter(|m| m.recordable().is_ok() && m.is_in(app)).filter_map(watch::meeting_start).any(|st| now >= st - before && now <= st + after)
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
                failed(format!("{e:#}"), quiet);
                Ok(())
            }
        };
    }
    let cfg = Config::load().unwrap_or_default();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from);
    let cmd = calendar_command(cfg.calendar_command.clone(), home);
    let Some(cmd) = cmd else {
        // No calendar any more (signed out, the skill removed): nothing can be "unread today".
        let _ = std::fs::remove_file(failed_file());
        if !quiet {
            println!("{}", json!({ "synced": false, "reason": "no calendar command (msgraph skill not installed, or the calendar is off)" }));
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
            let why = String::from_utf8_lossy(&o.stderr).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
            // Its last words on stderr, else how it ended: never a reason ending in nothing.
            let why = if why.is_empty() { format!("it ended with {}", o.status) } else { why };
            failed(format!("calendar command failed: {why}"), quiet);
            return Ok(());
        }
        Err(e) => {
            failed(e.to_string(), quiet);
            return Ok(());
        }
    };
    store(&events, quiet)
}

/// The last failed sync (`at`, `reason`), removed by a good one: a failed sync keeps the last agenda (#131).
fn failed_file() -> PathBuf {
    crate::paths::state_dir().join("calendar-failed.json")
}

fn failed(reason: String, quiet: bool) {
    let _ = std::fs::create_dir_all(crate::paths::state_dir());
    let _ = crate::paths::write_atomic(&failed_file(), json!({"at": Local::now().to_rfc3339(), "reason": reason}).to_string().as_bytes());
    if !quiet {
        println!("{}", json!({ "synced": false, "reason": reason }));
    }
}

/// Whether today's calendar couldn't be read: a sync failed today and none has written the agenda today. Before the
/// day's first sync, or with no calendar at all, nothing is said.
pub fn unread_today() -> bool {
    let today = Local::now().date_naive();
    let failed_today = std::fs::read(failed_file()).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()))
        .is_some_and(|t| t.with_timezone(&Local).date_naive() == today);
    let synced_today = std::fs::metadata(crate::paths::state_dir().join("agenda.json")).and_then(|m| m.modified())
        .is_ok_and(|t| DateTime::<Local>::from(t).date_naive() == today);
    failed_today && !synced_today
}

/// Filter + convert Graph-shaped events into the agenda.
fn store(events: &[Value], quiet: bool) -> Result<()> {
    let now = Utc::now();
    let mut meetings: Vec<MeetingContext> = events.iter().filter_map(|e| convert(e, now)).collect();
    meetings.sort_by(|a, b| a.start.cmp(&b.start));
    let n = meetings.len();
    watch::write_agenda(&meetings)?;
    let _ = std::fs::remove_file(failed_file());
    // Meetings of a series set to record always: "Record when it starts", each once.
    if let Err(e) = crate::series::schedule_always(&meetings) {
        eprintln!("series: {e:#}");
    }
    if !quiet {
        println!("{}", json!({ "synced": true, "meetings": n }));
    }
    Ok(())
}

/// The agenda meeting a call in `app` starting now belongs to (10 min early … end), if exactly
/// one matches; the one starting closest to now wins a tie.
pub fn meeting_now(now: DateTime<Utc>, app: crate::apps::TargetApp) -> Option<MeetingContext> {
    meetings_now(now, app).into_iter().next()
}

/// Whether a meeting with a Google Meet link goes on now (as `meeting_now` matches one), whichever is closest: a sign
/// that Chrome on the mic is a Meet call (#500: one with no link starting closer hid it).
pub fn meet_meeting_now(now: DateTime<Utc>) -> bool {
    meetings_now(now, crate::apps::TargetApp::Meet).iter().any(|m| m.app() == Some(crate::apps::MeetingApp::Meet))
}

/// The agenda meetings a call in `app` starting now may belong to, the one starting closest to now first.
fn meetings_now(now: DateTime<Utc>, app: crate::apps::TargetApp) -> Vec<MeetingContext> {
    let mut hits: Vec<(i64, MeetingContext)> = watch::agenda()
        .into_iter()
        // A meeting of another app (a Zoom one, a Meet one for a Teams call): not the call being recorded.
        .filter(|m| m.recordable().is_ok() && m.is_in(app))
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
    hits.into_iter().map(|(_, m)| m).collect()
}

/// A call as `call_of_meeting` weighs it: the event id and start its `meeting.json` saved, when it started, and whether
/// it is being recorded now.
#[derive(Clone, Copy, Default)]
pub struct CallOf<'a> {
    pub id: Option<&'a str>,
    pub meeting_start: Option<DateTime<Utc>>,
    pub started: Option<DateTime<Utc>>,
    pub recording: bool,
}

/// A time as the calendar and `meeting.json` write it (RFC 3339, or Graph's naive UTC).
pub fn when(s: Option<&str>) -> Option<DateTime<Utc>> {
    watch::parse_time(&s.map(str::to_owned))
}

/// Which of `calls` is the call of the meeting `id` at `start`..`end`: one carrying its id that is this occurrence — its
/// `meeting.json` saved this start (an arm carries the meeting however early it was joined), or it started from
/// `MATCH_EARLY_MIN` before it to its end (at least `MATCH_LATE_MIN` after its start, as `meeting_now` attaches it) —
/// the one being recorded first, else the nearest its start. A meeting moved or held again keeps its id but not its
/// start: a call of it at another time is a call of its own, never this one's. The index in `calls`.
pub fn call_of_meeting<'a>(id: &str, start: DateTime<Utc>, end: Option<DateTime<Utc>>, calls: impl IntoIterator<Item = CallOf<'a>>) -> Option<usize> {
    let from = start - Duration::minutes(MATCH_EARLY_MIN);
    let to = end.unwrap_or(start).max(start + Duration::minutes(MATCH_LATE_MIN));
    calls
        .into_iter()
        .enumerate()
        .filter(|(_, c)| c.id == Some(id))
        .filter(|(_, c)| c.meeting_start == Some(start) || c.started.is_some_and(|t| (from..=to).contains(&t)))
        .min_by_key(|(_, c)| (!c.recording, c.started.map_or(i64::MAX, |t| (t - start).num_seconds().abs())))
        .map(|(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::TargetApp;

    #[test]
    fn a_meetings_call_is_the_one_of_its_id_recorded_at_its_time() {
        let start = DateTime::parse_from_rfc3339("2026-11-11T19:30:00Z").unwrap().with_timezone(&Utc);
        let at = |m: i64| Some(start + Duration::minutes(m));
        let end = at(30);
        // A call of `id` that started at minute `m`, its meeting.json saved with the meeting starting at minute `saved`.
        let c = |id: &'static str, m: i64, saved: Option<i64>| CallOf { id: Some(id), started: at(m), meeting_start: saved.and_then(at), recording: false };
        // Held at 15:31 and 15:32 (the meeting then at 15:30, since moved to 19:30, same id), recorded again at 19:32;
        // another event at 19:31.
        let calls = [c("ev", -239, Some(-240)), c("ev", -238, Some(-240)), c("other", 1, Some(0)), c("ev", 2, Some(0))];
        assert_eq!(call_of_meeting("ev", start, end, calls), Some(3));
        // Not held yet at its new time: none, never the earlier calls.
        assert_eq!(call_of_meeting("ev", start, end, calls[..3].iter().copied()), None);
        // Without its start saved: from 10 min early; the nearest its start wins; a short meeting's call up to 30 min after.
        assert_eq!(call_of_meeting("ev", start, end, [c("ev", -10, None), c("ev", -11, None)]), Some(0));
        assert_eq!(call_of_meeting("ev", start, end, [c("ev", 20, None), c("ev", -3, None)]), Some(1));
        assert_eq!(call_of_meeting("ev", start, at(15), [c("ev", 25, None)]), Some(0));
        assert_eq!(call_of_meeting("ev", start, end, [c("ev", 31, None), CallOf { started: at(0), ..Default::default() }]), None);
        // Joined 20 min early by its arm (Join & record, Record when it starts) or a Teams transcript from before it: its
        // saved start is this one's. Saved with another start, it isn't.
        assert_eq!(call_of_meeting("ev", start, end, [c("ev", -20, Some(0))]), Some(0));
        assert_eq!(call_of_meeting("ev", start, end, [c("ev", -20, Some(-240))]), None);
        // Restarted within the meeting: the part being recorded, not the first.
        let parts = [c("ev", 1, Some(0)), CallOf { recording: true, ..c("ev", 12, Some(0)) }];
        assert_eq!(call_of_meeting("ev", start, end, parts), Some(1));
        // A recording of another occurrence doesn't win.
        assert_eq!(call_of_meeting("ev", start, end, [c("ev", 1, Some(0)), CallOf { recording: true, ..c("ev", -238, Some(-240)) }]), Some(0));
    }

    /// #242: test data sets `calendar_command: []`, and its calendar stays off even where the user has the msgraph
    /// skill installed — the sync never reads their real meetings into it.
    #[test]
    fn an_empty_calendar_command_turns_the_calendar_off_whatever_is_installed() {
        let home = std::env::temp_dir().join(format!("echo-test-calendar-off-{}", std::process::id()));
        let script = home.join(".claude/skills/msgraph/scripts/msgraph.js");
        std::fs::create_dir_all(script.parent().unwrap()).unwrap();
        std::fs::write(&script, "console.log('[]')").unwrap();
        assert_eq!(calendar_command(Some(vec![]), Some(home.clone())), None, "off: never the installed skill");
        assert!(calendar_command(None, Some(home.clone())).is_some_and(|c| c.contains(&script.display().to_string())), "unset: the skill");
        assert_eq!(calendar_command(Some(vec!["cal".into()]), Some(home.clone())), Some(vec!["cal".into()]));
        assert_eq!(calendar_command(None, None), None);
        let _ = std::fs::remove_dir_all(&home);
    }

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

    /// #228: a Zoom or Google Meet invite in Outlook isn't an online meeting in Graph's terms; its link is only in its
    /// text. It is kept, with that link and its app.
    #[test]
    fn a_zoom_or_google_meet_invite_is_kept_with_the_link_from_its_text() {
        let now = Local::now().with_timezone(&Utc);
        let soon = now + Duration::minutes(1);
        let mut zoom = ev("Umbrella pricing call", soon, false);
        zoom["bodyPreview"] = json!("Kim Park is inviting you to a scheduled Zoom meeting.\r\nJoin Zoom Meeting\r\nhttps://umbrella.zoom.us/j/81234567890?pwd=Xyz1, Meeting ID: 812 3456 7890");
        let m = convert(&zoom, now).expect("a Zoom invite is kept");
        assert_eq!(m.join_url.as_deref(), Some("https://umbrella.zoom.us/j/81234567890?pwd=Xyz1"));
        let v = view_event(&zoom).unwrap();
        assert_eq!((v["online"].clone(), v["app"].clone(), v["join_url"].clone()), (json!(true), json!("zoom"), json!("https://umbrella.zoom.us/j/81234567890?pwd=Xyz1")));

        let mut meet = ev("Globex design sync", soon, false);
        meet["location"] = json!({"displayName": "meet.google.com/abc-defg-hij"});
        meet["bodyPreview"] = json!("Agenda: the design. Another link: https://zoom.us/j/81234567890");
        assert_eq!(convert(&meet, now).unwrap().join_url.as_deref(), Some("https://meet.google.com/abc-defg-hij"), "the location first");
        assert_eq!(view_event(&meet).unwrap()["app"], "meet");

        // The msgraph skill's JSON may carry the whole body (HTML).
        let mut body = ev("Acme demo", soon, false);
        body["body"] = json!({"contentType": "html", "content": "<p>Join: <a href=\"https://meet.google.com/abc-defg-hij\">Google Meet</a></p>"});
        assert_eq!(to_meeting(&body).unwrap().join_url.as_deref(), Some("https://meet.google.com/abc-defg-hij"));

        // Graph's own link wins; a Teams meeting is as it was.
        let mut teams = ev("Daily sync", soon, true);
        teams["onlineMeeting"] = json!({"joinUrl": "https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0"});
        teams["bodyPreview"] = json!("https://zoom.us/j/81234567890");
        assert_eq!(convert(&teams, now).unwrap().join_url.as_deref(), Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0"));
        assert_eq!(view_event(&teams).unwrap()["app"], "teams");

        // No known app's link, or a look-alike: still not a meeting to offer.
        let mut lunch = ev("Lunch", soon, false);
        lunch["bodyPreview"] = json!("At https://zoom.us.lunch.example/j/81234567890 or https://acme.example/menu");
        assert!(convert(&lunch, now).is_none());
        assert_eq!(view_event(&lunch).unwrap()["app"], Value::Null);
    }

    /// #253 review: a preview cut at its length limit may end inside a link; that link isn't taken.
    #[test]
    fn a_link_cut_off_at_the_end_of_the_preview_is_not_taken() {
        let soon = Utc::now() + Duration::minutes(1);
        let mut zoom = ev("Umbrella pricing call", soon, false);
        let pad = "Agenda: pricing tiers, the pilot's numbers and what the board asked for. ".repeat(3);
        zoom["bodyPreview"] = json!(format!("{pad}Join Zoom Meeting https://umbrella.zoom.us/j/8123456789"));
        assert!(zoom["bodyPreview"].as_str().unwrap().chars().count() >= 250);
        assert_eq!(view_event(&zoom).unwrap()["join_url"], Value::Null, "cut to 10 digits: not a link to join");
        // The whole link before the cut: taken.
        zoom["bodyPreview"] = json!(format!("Join Zoom Meeting https://umbrella.zoom.us/j/81234567890 {pad}Meeting ID: 812"));
        assert_eq!(view_event(&zoom).unwrap()["join_url"], "https://umbrella.zoom.us/j/81234567890");
        // Emoji count twice: 125 of them make a preview Graph has cut.
        zoom["bodyPreview"] = json!(format!("{} https://umbrella.zoom.us/j/8123456789", "🙂".repeat(125)));
        assert_eq!(view_event(&zoom).unwrap()["join_url"], Value::Null, "cut, counted in UTF-16");
        // A short preview isn't cut: its last word is whole.
        zoom["bodyPreview"] = json!("Join Zoom Meeting https://umbrella.zoom.us/j/81234567890");
        assert_eq!(view_event(&zoom).unwrap()["join_url"], "https://umbrella.zoom.us/j/81234567890");
    }

    /// #253 review: an invite from outside with its Teams link only in the text (no `onlineMeeting` of the user's
    /// own) is a Teams meeting now: kept, recordable, with Join & record; before #228 it was dropped.
    #[test]
    fn an_outside_teams_invite_with_its_link_only_in_the_text_is_kept() {
        let now = Local::now().with_timezone(&Utc);
        let mut ext = ev("Globex supplier call", now + Duration::minutes(1), false);
        let link = "https://teams.microsoft.com/l/meetup-join/19%3ameeting_Zm9v%40thread.v2/0?context=%7b%22Tid%22%3a%22t%22%7d";
        ext["bodyPreview"] = json!(format!("Microsoft Teams meeting\r\nJoin on your computer: {link}"));
        let m = convert(&ext, now).expect("kept");
        assert_eq!(m.join_url.as_deref(), Some(link));
        assert!(m.recordable().is_ok() && m.teams_join_url() == Some(link));
        assert_eq!(view_event(&ext).unwrap()["app"], "teams");
    }

    /// #253 review: a Teams call starting around a Zoom meeting's start is still asked about ("Record?"): the agenda
    /// doesn't offer the Zoom meeting.
    #[test]
    fn only_a_meeting_echo_can_record_is_offered_by_the_agenda() {
        let now = Utc::now();
        let at = |url: &str| MeetingContext { start: Some(now.to_rfc3339()), join_url: Some(url.into()), ..Default::default() };
        let (before, after) = (Duration::seconds(58), Duration::minutes(4));
        assert!(!offered_now(&[at("https://zoom.us/j/81234567890")], TargetApp::Teams, now, before, after));
        assert!(offered_now(&[at("https://zoom.us/j/81234567890"), at("https://teams.example/ev-1")], TargetApp::Teams, now, before, after));
        assert!(!offered_now(&[at("https://teams.example/ev-1")], TargetApp::Teams, now + Duration::minutes(5), before, after), "past the window");
    }

    /// #500: a Teams call inside a Google Meet meeting's offer window is still asked about: that meeting's arm can't
    /// start on Teams, so it offers nothing to a Teams call (it does to a Meet call; one with no link, to either).
    #[test]
    fn a_meeting_is_offered_only_to_a_call_of_its_app() {
        let now = Utc::now();
        let at = |url: Option<&str>| MeetingContext { start: Some(now.to_rfc3339()), join_url: url.map(Into::into), ..Default::default() };
        let (before, after) = (Duration::seconds(58), Duration::minutes(4));
        let meet = [at(Some("https://meet.google.com/abc-defg-hij"))];
        assert!(!offered_now(&meet, TargetApp::Teams, now, before, after));
        if cfg!(target_os = "macos") {
            assert!(offered_now(&meet, TargetApp::Meet, now, before, after));
            assert!(offered_now(&meet, TargetApp::MeetApp, now, before, after));
        }
        assert!(offered_now(&[at(None)], TargetApp::Teams, now, before, after), "no link: any call's");
    }

    /// #500: the Meet sign sees a Meet meeting going on though a meeting with no link starts closer to now (the call
    /// is matched to the closest, which may be that one).
    #[test]
    fn a_meet_meeting_going_on_is_seen_beside_a_closer_one_with_no_link() {
        let (_env, _home) = crate::paths::testenv::folders("calendar-meet-now");
        let now = Utc::now();
        let at = |id: &str, start: DateTime<Utc>, url: Option<&str>| MeetingContext { id: Some(id.into()), start: Some(start.to_rfc3339()), end: Some((start + Duration::minutes(30)).to_rfc3339()), join_url: url.map(Into::into), ..Default::default() };
        watch::write_agenda(&[at("meet", now - Duration::minutes(5), Some("https://meet.google.com/abc-defg-hij")), at("room", now, None)]).unwrap();
        assert_eq!(meeting_now(now, TargetApp::Meet).and_then(|m| m.id).as_deref(), Some("room"), "the closest");
        assert_eq!(meet_meeting_now(now), cfg!(target_os = "macos"), "a Meet one goes on (recordable on a Mac)");
        watch::write_agenda(&[at("room", now, None)]).unwrap();
        assert!(!meet_meeting_now(now));
    }

    /// A call is matched only to a meeting of its own app: a Teams call during a Zoom or Google Meet meeting isn't
    /// that meeting, nor a Meet call during a Teams one.
    #[test]
    fn a_call_is_matched_only_to_a_meeting_of_its_app() {
        let (_env, _home) = crate::paths::testenv::folders("calendar-now-other-app");
        let now = Utc::now();
        let at = |id: &str, url: &str| MeetingContext { id: Some(id.into()), start: Some(now.to_rfc3339()), end: Some((now + Duration::minutes(30)).to_rfc3339()), join_url: Some(url.into()), ..Default::default() };
        watch::write_agenda(&[at("zoom", "https://zoom.us/j/81234567890")]).unwrap();
        assert!(meeting_now(now, TargetApp::Teams).is_none());
        watch::write_agenda(&[at("zoom", "https://zoom.us/j/81234567890"), at("teams", "https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0"), at("meet", "https://meet.google.com/abc-defg-hij")]).unwrap();
        assert_eq!(meeting_now(now, TargetApp::Teams).and_then(|m| m.id).as_deref(), Some("teams"));
        if cfg!(target_os = "macos") {
            assert_eq!(meeting_now(now, TargetApp::Meet).and_then(|m| m.id).as_deref(), Some("meet"));
        }
    }

    #[test]
    fn a_meeting_is_the_users_once_accepted() {
        let at = Utc::now();
        let with = |r: &str| {
            let mut e = ev("Sync", at, true);
            e["responseStatus"] = json!({"response": r});
            to_meeting(&e).unwrap()
        };
        assert!(with("accepted").accepted() && with("organizer").accepted() && with("none").accepted(), "none: no answer asked");
        assert!(!with("tentativelyAccepted").accepted() && !with("notResponded").accepted());
        assert!(to_meeting(&ev("Sync", at, true)).unwrap().accepted(), "a source that doesn't say: kept");
    }

    #[test]
    fn the_card_gets_who_is_coming_by_name_rooms_and_declined_left_out() {
        let ev = json!({"id": "e1", "subject": "Design review", "start": {"dateTime": "2026-11-08T15:00:00.0000000", "timeZone": "UTC"},
            "attendees": [{"type": "required", "emailAddress": {"name": "Kim Park", "address": "kim@globex.example"}},
                          {"type": "optional", "emailAddress": {"address": "noor@globex.example"}},
                          {"type": "required", "status": {"response": "declined"}, "emailAddress": {"name": "Raj Mehta", "address": "raj@umbrella.example"}},
                          {"type": "resource", "emailAddress": {"name": "Room 4", "address": "room4@globex.example"}}]});
        let v = view_event(&ev).unwrap();
        assert_eq!(v["names"], json!(["Kim Park", "noor@globex.example"]), "rooms and who declined left out");
        assert_eq!(v["people"], 2, "and the count with them");
    }

    #[test]
    fn a_meeting_keeps_how_each_person_answered_and_who_declined_apart() {
        let ev = json!({"id": "e1", "subject": "Design review", "start": {"dateTime": "2026-11-08T15:00:00.0000000", "timeZone": "UTC"},
            "attendees": [{"type": "required", "status": {"response": "accepted"}, "emailAddress": {"name": "Kim Park", "address": "Kim@globex.example"}},
                          {"type": "optional", "status": {"response": "tentativelyAccepted"}, "emailAddress": {"address": "noor@globex.example"}},
                          {"type": "required", "status": {"response": "declined"}, "emailAddress": {"name": "Raj Mehta", "address": "raj@umbrella.example"}},
                          {"type": "resource", "status": {"response": "accepted"}, "emailAddress": {"name": "Room 4", "address": "room4@globex.example"}}]});
        let m = to_meeting(&ev).unwrap();
        assert_eq!(m.attendees.iter().map(|p| p.email.as_deref().unwrap()).collect::<Vec<_>>(), ["Kim@globex.example", "noor@globex.example"], "coming: no room, no one who declined");
        assert_eq!(m.declined.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Raj Mehta"]);
        assert_eq!(m.answers.get("kim@globex.example").map(String::as_str), Some("accepted"), "by the lowercased address");
        assert_eq!(m.answers.get("noor@globex.example").map(String::as_str), Some("tentativelyAccepted"));
        assert!(!m.answers.contains_key("room4@globex.example"));
        // How each person is invited, by the lowercased address (#402): rooms left out.
        assert_eq!(m.invited_as.get("kim@globex.example").map(String::as_str), Some("required"));
        assert_eq!(m.invited_as.get("noor@globex.example").map(String::as_str), Some("optional"));
        assert!(!m.invited_as.contains_key("room4@globex.example"));
    }

    #[test]
    fn the_calendar_is_unread_today_only_after_a_failed_sync_with_no_good_one_today() {
        let (_env, _home) = crate::paths::testenv::folders("calendar-unread");
        let yesterday = std::time::SystemTime::now() - std::time::Duration::from_secs(36 * 3600);
        // The last good sync, last night: every meeting had ended, so it wrote an empty agenda (#131).
        watch::write_agenda(&[]).unwrap();
        std::fs::File::options().write(true).open(crate::paths::state_dir().join("agenda.json")).unwrap().set_modified(yesterday).unwrap();
        assert!(!unread_today(), "no sync tried yet today (just after midnight): nothing to say");
        // Today's sync fails (an expired sign-in): the empty agenda isn't today's.
        failed("sign-in expired".into(), true);
        assert!(unread_today(), "a failed sync today and no good one: the calendar couldn't be read");
        // A failure from an earlier day says nothing of today.
        crate::paths::write_atomic(&failed_file(), json!({"at": (Local::now() - Duration::days(2)).to_rfc3339()}).to_string().as_bytes()).unwrap();
        assert!(!unread_today());
        // A good sync today, then a passing failure: the agenda is today's.
        failed("offline".into(), true);
        store(&[], true).unwrap();
        assert!(!unread_today(), "a good sync clears it");
        failed("offline".into(), true);
        assert!(!unread_today(), "the agenda was read today");
    }

    #[test]
    fn graph_times_parse_as_utc() {
        let t = graph_time(&json!({ "dateTime": "2026-10-09T15:00:00.0000000", "timeZone": "UTC" })).unwrap();
        assert_eq!(t.to_rfc3339(), "2026-10-09T15:00:00+00:00");
    }
}

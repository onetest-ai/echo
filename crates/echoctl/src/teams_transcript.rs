//! A Teams meeting nobody recorded, made a call from its Teams transcript: `<home>/calls/<stamp>-<slug>/`
//! with `meta.json` (`source: teams-transcript`, the event's times, the AI profile pinned),
//! `meeting.json` (the event), `segments.json` + `transcript.md` (the WebVTT's `<v Name>` turns) and
//! the meeting chat — so minutes, threads, the work ledger and the Brain take it as a recording.
//! An Echo recording of the meeting wins: no second call.

use std::fs;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Local, Utc};
use serde_json::{json, Value};

use crate::graph::{self, Transcripts};
use crate::meeting::MeetingContext;
use crate::{calls, paths};

/// `meta.json`'s `source` for a call made from a Teams transcript.
pub const SOURCE: &str = "teams-transcript";

/// One cue of a WebVTT transcript: seconds from its start, the speaker Teams named, the words.
#[derive(Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub name: Option<String>,
    pub text: String,
}

fn vtt_time(s: &str) -> Option<f64> {
    let p: Vec<&str> = s.trim().split(':').collect();
    let (h, m, sec) = match p.as_slice() {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// Teams' WebVTT: cues `00:01:02.345 --> 00:01:05.000`, their text `<v Jane Doe>words</v>` (a cue may
/// run over lines; one with no voice keeps the text, unnamed).
pub fn parse_vtt(vtt: &str) -> Vec<Cue> {
    let mut out = vec![];
    let lines: Vec<&str> = vtt.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let Some((a, b)) = lines[i].split_once("-->") else {
            i += 1;
            continue;
        };
        let (start, end) = (vtt_time(a), vtt_time(b.split_whitespace().next().unwrap_or("")));
        i += 1;
        let mut body = vec![];
        while i < lines.len() && !lines[i].trim().is_empty() && !lines[i].contains("-->") {
            body.push(lines[i].trim());
            i += 1;
        }
        let (Some(start), Some(end)) = (start, end) else { continue };
        let raw = body.join(" ");
        let name = raw.strip_prefix("<v ").and_then(|r| r.split_once('>')).map(|(n, _)| n.trim().to_string()).filter(|n| !n.is_empty());
        // The words without tags (`<v …>`, `</v>`, any other markup), entities read back.
        let mut text = String::new();
        let mut tag = false;
        for c in raw.chars() {
            match c {
                '<' => tag = true,
                '>' if tag => tag = false,
                c if !tag => text.push(c),
                _ => {}
            }
        }
        let text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            out.push(Cue { start, end, name, text });
        }
    }
    out
}

fn hms(t: f64) -> String {
    let t = t.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
}

/// The transcripts as one call's segments, each transcript placed by when it started (`created`)
/// against `t0`: "Me" for the user's own name, everyone else by the name Teams gave.
fn segments(transcripts: &[Value], t0: DateTime<Utc>, me: &crate::tasks::Me) -> Vec<Value> {
    let mut segs = vec![];
    for t in transcripts {
        let off = t["created"].as_str().and_then(|c| DateTime::parse_from_rfc3339(c).ok()).map(|c| (c.with_timezone(&Utc) - t0).num_milliseconds() as f64 / 1000.0).unwrap_or(0.0).max(0.0);
        for mut c in parse_vtt(t["vtt"].as_str().unwrap_or("")) {
            // Teams' unnamed speakers ("@1") are Echo's "Speaker 1": named later, as for a recording.
            if let Some(n) = c.name.as_deref().and_then(|n| n.strip_prefix('@')).filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())) {
                c.name = Some(format!("Speaker {n}"));
            }
            let mine = c.name.as_deref().is_some_and(|n| me.is(n));
            segs.push(json!({"speaker": if mine { "me" } else { "remote" }, "start": off + c.start, "end": off + c.end, "lang": "",
                             "text": c.text, "name": if mine { None } else { c.name }}));
        }
    }
    segs.sort_by(|a, b| a["start"].as_f64().partial_cmp(&b["start"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
    segs
}

/// `transcript.md` as Echo writes it, a turn per speaker's run, saying where it came from.
fn markdown(dir: &std::path::Path, title: &str, started: &DateTime<Local>, meeting: &MeetingContext, segs: &[Value]) -> String {
    let duration = segs.iter().filter_map(|s| s["end"].as_f64()).fold(0.0, f64::max);
    let mut md = format!(
        "# {title}\n\n- **Recorded:** {}\n- **Duration (speech):** {}\n- **Source:** Teams transcript (imported; no audio)\n- **Folder:** `{}`\n",
        started.to_rfc3339(),
        hms(duration),
        dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()
    );
    let people = meeting.people();
    if !people.is_empty() {
        md.push_str(&format!("- **Invited:** {}\n", people.join(", ")));
    }
    md.push('\n');
    let who = |s: &Value| if s["speaker"] == "me" { "Me".to_string() } else { s["name"].as_str().unwrap_or("Remote").to_string() };
    let mut i = 0;
    while i < segs.len() {
        let mut text = segs[i]["text"].as_str().unwrap_or("").to_string();
        let mut j = i + 1;
        while j < segs.len() && who(&segs[j]) == who(&segs[i]) && segs[j]["start"].as_f64().unwrap_or(0.0) - segs[j - 1]["end"].as_f64().unwrap_or(0.0) < 2.0 {
            text.push(' ');
            text.push_str(segs[j]["text"].as_str().unwrap_or(""));
            j += 1;
        }
        md.push_str(&format!("**{}** [{}]\n{text}\n\n", who(&segs[i]), hms(segs[i]["start"].as_f64().unwrap_or(0.0))));
        i = j;
    }
    md
}

/// When an occurrence's transcripts start: from 30 minutes before it to 30 minutes after its end (3 h
/// with no end).
pub fn occurrence_window(start: DateTime<Utc>, end: Option<DateTime<Utc>>) -> (DateTime<Utc>, DateTime<Utc>) {
    (start - Duration::minutes(30), end.unwrap_or(start + Duration::hours(3)) + Duration::minutes(30))
}

fn end_of(m: &MeetingContext) -> Option<DateTime<Utc>> {
    m.end.as_deref().and_then(|e| DateTime::parse_from_rfc3339(e).ok()).map(|e| e.with_timezone(&Utc))
}

/// The call already made of this event: its meeting's id, or a call that started during it with its
/// subject (as the calendar matches them).
pub fn call_of_event(m: &MeetingContext) -> Option<PathBuf> {
    let (s, e) = (crate::watch::meeting_start(m)?, m.end.as_deref().and_then(|e| DateTime::parse_from_rfc3339(e).ok()).map(|e| e.with_timezone(&Utc)));
    let subject = m.subject.as_deref().unwrap_or("").trim().to_lowercase();
    calls::all_calls().into_iter().find(|d| {
        let meeting = calls::read_json(&d.join("meeting.json"));
        if m.id.is_some() && meeting["id"].as_str() == m.id.as_deref() {
            return true;
        }
        let meta = calls::read_json(&d.join("meta.json"));
        let Some(cs) = meta["started_at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc)) else { return false };
        let near = cs >= s - Duration::minutes(15) && cs <= e.unwrap_or(s + Duration::hours(1));
        near && meeting["subject"].as_str().or(meta["title"].as_str()).is_some_and(|t| t.trim().to_lowercase() == subject)
    })
}

/// The calendar event an argument names: its Graph id, or words of its subject — the latest such
/// event that has started, on `day` or in the last 14 days.
pub fn find_event(arg: &str, day: Option<chrono::NaiveDate>) -> Result<Value> {
    if arg.len() > 60 && !arg.contains(' ') {
        return graph::event(arg);
    }
    let now = Utc::now();
    let (from, to) = match day {
        Some(d) => (crate::calendar::local_midnight(d).context("that day")?, d.succ_opt().and_then(crate::calendar::local_midnight).context("that day")?),
        None => (now - Duration::days(14), now),
    };
    let words: Vec<String> = arg.to_lowercase().split_whitespace().map(str::to_owned).collect();
    let events = graph::calendar_view(from, to)?;
    events
        .into_iter()
        .rev()
        .filter(|e| {
            let s = e["subject"].as_str().unwrap_or("").to_lowercase();
            words.iter().all(|w| s.contains(w.as_str()))
        })
        .find(|e| crate::calendar::to_meeting(e).and_then(|m| crate::watch::meeting_start(&m)).is_some_and(|s| s <= now))
        .with_context(|| format!("no meeting matching {arg:?} {}", day.map_or("in the last 14 days".to_string(), |d| format!("on {d}"))))
}

/// Make a call of a calendar event from its Teams transcript; its folder.
pub fn import(ev: &Value) -> Result<PathBuf> {
    let Some(mut meeting) = crate::calendar::to_meeting(ev) else { bail!("this calendar event has no start time") };
    let title = meeting.subject.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "Teams meeting".into());
    if let Some(d) = call_of_event(&meeting) {
        bail!("“{title}” is already a call in Echo: {}", d.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default());
    }
    let Some(join) = meeting.teams_join_url().map(str::to_owned) else { bail!("“{title}” isn't a Teams meeting (no Teams join link)") };
    let start = crate::watch::meeting_start(&meeting).context("the event's start")?;
    if start > Utc::now() {
        bail!("“{title}” hasn't happened yet");
    }
    let none = || anyhow::anyhow!("“{title}” has no Teams transcript: nobody turned transcription on");
    let transcripts = match graph::meeting_transcripts(&join, occurrence_window(start, end_of(&meeting)), true)? {
        // Already only this occurrence's (`graph::meeting_transcripts` filters by the window).
        Transcripts::Found(t) => t,
        Transcripts::None => return Err(none()),
        Transcripts::NotGiven(why) => bail!("“{title}”'s transcript can't be read: {why}"),
    };
    if transcripts.is_empty() {
        return Err(none()); // other occurrences of the series have one, this one doesn't
    }
    // Times from the event; a transcript that started before it moves the start (its words keep
    // their place), one that ran past it the end.
    let first = transcripts.iter().filter_map(|t| t["created"].as_str()).filter_map(|c| DateTime::parse_from_rfc3339(c).ok()).map(|c| c.with_timezone(&Utc)).min();
    let t0 = first.map_or(start, |f| f.min(start));
    let cfg = crate::config::Config::load().ok();
    // About me's name, else the Microsoft 365 account's ("alex_morgan@…" → "alex morgan").
    let account = graph::account().map(|a| crate::meeting::Person { name: a.split('@').next().unwrap_or("").replace(['_', '.'], " "), email: Some(a) });
    let me = crate::tasks::Me::of(cfg.as_ref().and_then(|c| c.me.as_ref()).or(account.as_ref()));
    let segs = segments(&transcripts, t0, &me);
    if segs.is_empty() {
        bail!("“{title}”'s Teams transcript is empty");
    }
    let spoken = t0 + Duration::milliseconds((segs.iter().filter_map(|s| s["end"].as_f64()).fold(0.0, f64::max) * 1000.0) as i64);
    let end = end_of(&meeting).map_or(spoken, |e| e.max(spoken));
    let started = t0.with_timezone(&Local);
    let dir = crate::recorder::call_dir(&started, Some(&title));
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let profile = crate::ai::pin(None, Some(&meeting));
    meeting.source = Some(SOURCE.into());
    meeting.save(&dir)?;
    paths::write_atomic(&dir.join("segments.json"), &serde_json::to_vec_pretty(&segs)?)?;
    paths::write_atomic(&dir.join("transcript.md"), markdown(&dir, &title, &started, &meeting, &segs).as_bytes())?;
    // meta.json last: the call exists for everyone else only once it is whole.
    let meta = json!({"title": title, "app": "teams", "platform": std::env::consts::OS, "source": SOURCE,
                      "started_at": started.to_rfc3339(), "ended_at": end.with_timezone(&Local).to_rfc3339(), "stop_reason": "imported from the Teams transcript",
                      "imported_at": Local::now().to_rfc3339(), "transcripts": transcripts.len(), "ai_profile": profile, "tracks": []});
    paths::write_atomic(&dir.join("meta.json"), &serde_json::to_vec_pretty(&meta)?)?;
    let name = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    // The meeting chat, as for a recording (none is no failure).
    if let Err(e) = crate::watch::fetch_call_chat(&name) {
        eprintln!("chat: {e:#}");
    }
    Ok(dir)
}

// ── offered in Needs you: meetings of the user's threads with a transcript and no call ──────────

/// What was checked: `{event id: {subject, start, threads, transcript: true|false|null, checked, why, dismissed}}`.
fn offers_file() -> PathBuf {
    paths::state_dir().join("teams-transcripts.json")
}

pub fn offers() -> serde_json::Map<String, Value> {
    fs::read(offers_file()).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn save_offers(m: &serde_json::Map<String, Value>) -> Result<()> {
    paths::write_atomic(&offers_file(), &serde_json::to_vec_pretty(m)?)?;
    Ok(())
}

/// Not this one: never offered again, and the threads it was offered for keep the meeting out (a call
/// made of it later doesn't join them by their rules).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn dismiss(id: &str) -> Result<()> {
    let mut m = offers();
    let e = m.entry(id.to_string()).or_insert_with(|| json!({}));
    e["dismissed"] = json!(true);
    for t in e["threads"].as_array().cloned().unwrap_or_default().iter().filter_map(|t| t["id"].as_str()) {
        crate::projects::update(t, |p| {
            if !p.not_events.iter().any(|x| x == id) {
                p.not_events.push(id.to_string());
            }
            Ok(())
        })?;
    }
    save_offers(&m)
}

/// One check at a time (the agenda's scan and the calendar's), so neither undoes the other's answers.
static CHECKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// At most this many meetings asked about in one check.
const CHECK_MAX: usize = 25;

/// An ended Teams meeting with no call, not cancelled or declined: one whose transcript could be imported.
/// `calls`: look for its call too (the calendar knows already).
fn importable(ev: &Value, now: DateTime<Utc>, calls: bool) -> Option<(String, MeetingContext, DateTime<Utc>)> {
    if ev["isCancelled"] == true || ev["responseStatus"]["response"] == "declined" || ev["isAllDay"] == true {
        return None;
    }
    let (id, meeting) = (ev["id"].as_str()?.to_string(), crate::calendar::to_meeting(ev)?);
    meeting.teams_join_url()?;
    let end = end_of(&meeting)?;
    (end + Duration::minutes(10) <= now && !(calls && call_of_event(&meeting).is_some())).then_some((id, meeting, end))
}

/// Known well enough not to ask again: a transcript found; or asked within the hour; or asked a day
/// after the meeting ended (Teams writes the transcript minutes after it).
fn settled(had: &Value, end: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    let checked = had["checked"].as_str().and_then(|c| DateTime::parse_from_rfc3339(c).ok()).map(|c| c.with_timezone(&Utc));
    had["dismissed"] == true || had["transcript"] == true || checked.is_some_and(|c| c > end + Duration::days(1) || now - c < Duration::hours(1))
}

/// Whether these calendar events (Graph's) have a transcript of their occurrence, into the file:
/// `threads_only` — only meetings of the user's threads (what Needs you offers).
pub fn check(events: &[Value], threads_only: bool) -> Result<()> {
    let _one = CHECKING.lock().unwrap_or_else(|e| e.into_inner());
    let now = Utc::now();
    let mut m = offers();
    // Older than a month: forgotten.
    m.retain(|_, v| v["start"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).is_none_or(|s| now - s.with_timezone(&Utc) < Duration::days(30)));
    let mut asked = 0;
    for ev in events {
        let Some((id, meeting, end)) = importable(ev, now, true) else { continue };
        let threads: Vec<Value> = crate::projects::for_event(&meeting);
        if (threads_only && threads.is_empty()) || settled(&m.get(&id).cloned().unwrap_or_default(), end, now) {
            continue;
        }
        if asked == CHECK_MAX {
            break;
        }
        asked += 1;
        let window = occurrence_window(crate::watch::meeting_start(&meeting).unwrap_or(end), Some(end));
        let (found, why) = match graph::meeting_transcripts(meeting.join_url.as_deref().unwrap_or(""), window, false) {
            Ok(Transcripts::Found(t)) => (json!(!t.is_empty()), None),
            Ok(Transcripts::None) => (json!(false), None),
            Ok(Transcripts::NotGiven(w)) => (json!(false), Some(w)),
            Err(e) => (Value::Null, Some(format!("{e:#}"))),
        };
        m.insert(id, json!({"subject": meeting.subject, "start": meeting.start, "end": meeting.end,
                            "threads": threads.iter().map(|t| json!({"id": t["id"], "name": t["name"], "client": t["client"]})).collect::<Vec<_>>(),
                            "transcript": found, "why": why, "checked": now.to_rfc3339()}));
    }
    save_offers(&m)
}

/// Look for meetings of the last 3 days in the user's threads that ended, weren't recorded and have a
/// Teams transcript (the app, with the agenda's sync).
pub fn scan() -> Result<()> {
    if graph::account().is_none() {
        return Ok(());
    }
    let now = Utc::now();
    check(&graph::calendar_view(now - Duration::days(3), now)?, true)
}

/// For the calendar: what is known of an event's transcript (`Some(true)`: one to import), and
/// whether it should be asked about now.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn known(ev: &Value, offers: &serde_json::Map<String, Value>) -> (Option<bool>, bool) {
    let now = Utc::now();
    let Some((id, _, end)) = importable(ev, now, false) else { return (None, false) };
    let had = offers.get(&id).cloned().unwrap_or_default();
    (had["transcript"].as_bool(), !settled(&had, end, now))
}

/// The meetings to offer: a transcript found, not dismissed, still no call.
pub fn to_offer() -> Vec<(String, Value)> {
    offers()
        .into_iter()
        .filter(|(_, v)| v["transcript"] == true && v["dismissed"] != true && v["threads"].as_array().is_some_and(|t| !t.is_empty()))
        .filter(|(id, v)| {
            let m = MeetingContext { id: Some(id.clone()), subject: v["subject"].as_str().map(str::to_owned), start: v["start"].as_str().map(str::to_owned), end: v["end"].as_str().map(str::to_owned), ..Default::default() };
            call_of_event(&m).is_none()
        })
        .collect()
}

/// Was this call made from a Teams transcript.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn imported(meta: &Value) -> bool {
    meta["source"] == SOURCE
}

#[cfg(test)]
mod tests {
    use super::*;

    const VTT: &str = "WEBVTT\n\n0a1b/12-0\n00:00:03.120 --> 00:00:05.400\n<v Dana Kowal>Can we start with the pilot?</v>\n\n\
                       0a1b/13-0\n00:00:05.900 --> 00:00:09.000\n<v Alex Morgan>Yes &amp; the SSO date\nfirst.</v>\n\n00:00:10.000 --> 00:00:11.000\nNo voice here.\n";

    #[test]
    fn a_teams_vtt_reads_as_named_turns() {
        let c = parse_vtt(VTT);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0], Cue { start: 3.12, end: 5.4, name: Some("Dana Kowal".into()), text: "Can we start with the pilot?".into() });
        assert_eq!((c[1].name.as_deref(), c[1].text.as_str()), (Some("Alex Morgan"), "Yes & the SSO date first."));
        assert_eq!((c[2].name.as_deref(), c[2].text.as_str()), (None, "No voice here."));
    }

    #[test]
    fn transcripts_join_in_time_order_with_me_as_me() {
        let t0 = DateTime::parse_from_rfc3339("2026-11-02T16:00:00Z").unwrap().with_timezone(&Utc);
        let me = crate::tasks::Me::of(Some(&crate::meeting::Person { name: "Alex Morgan".into(), email: None }));
        // Restarted: the second transcript began 10 minutes in; listed first, still placed second.
        let ts = vec![
            json!({"created": "2026-11-02T16:10:00Z", "vtt": "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n<v Dana Kowal>Back again.</v>\n"}),
            json!({"created": "2026-11-02T16:00:30Z", "vtt": VTT}),
        ];
        let s = segments(&ts, t0, &me);
        assert_eq!(s.len(), 4);
        assert_eq!((s[0]["start"].as_f64(), s[0]["name"].as_str()), (Some(33.12), Some("Dana Kowal")));
        assert_eq!((s[1]["speaker"].as_str(), s[1]["name"].is_null()), (Some("me"), true));
        assert_eq!((s[3]["start"].as_f64(), s[3]["text"].as_str()), (Some(601.0), Some("Back again.")));
        let m = MeetingContext { subject: Some("Aurora weekly".into()), ..Default::default() };
        let md = markdown(std::path::Path::new("/x/2026-11-02_160000-aurora"), "Aurora weekly", &t0.with_timezone(&Local), &m, &s);
        assert!(md.contains("Teams transcript (imported; no audio)") && md.contains("**Me** [00:00:35]\nYes & the SSO date first."), "{md}");
    }

    /// #228: a Zoom or Google Meet meeting is in the calendar now; Teams has no transcript of it to offer or import.
    #[test]
    fn a_zoom_or_google_meet_meeting_offers_no_teams_transcript() {
        let (_env, _home) = crate::paths::testenv::folders("tt-other-app");
        let ended = (Utc::now() - Duration::hours(2)).format("%Y-%m-%dT%H:%M:%S").to_string();
        let ev = |preview: &str| json!({"id": "ev-1", "subject": "Umbrella check-in", "bodyPreview": preview,
            "start": {"dateTime": ended, "timeZone": "UTC"}, "end": {"dateTime": ended, "timeZone": "UTC"}});
        let zoom = ev("Join Zoom Meeting https://umbrella.zoom.us/j/81234567890");
        assert_eq!(known(&zoom, &Default::default()), (None, false), "nothing to ask Teams");
        assert!(import(&zoom).unwrap_err().to_string().contains("isn't a Teams meeting"));
        assert_eq!(known(&ev("https://teams.microsoft.com/l/meetup-join/19%3a1"), &Default::default()), (None, true), "a Teams one is asked about");
    }
}

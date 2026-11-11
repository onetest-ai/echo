//! The calendar (Microsoft 365 events with what Echo knows of each) and the Inbox's busy state.

use super::*;
use super::call::{minutes, status};
use super::plugins::{refresh_markets_for_inbox, PLUGINS_UPDATING};
#[cfg(feature = "brain")]
use super::brain::BRAIN_INSIGHTS;

/// Raw Graph events by id from the last calendar fetch (to schedule one by id), and a short
/// cache so the page's refreshes don't hit Microsoft 365 each time.
pub(super) type CalendarCache = (String, std::time::Instant, Vec<Value>);
pub(super) static CALENDAR_CACHE: std::sync::Mutex<Option<CalendarCache>> = std::sync::Mutex::new(None);
pub(super) static RAW_EVENTS: std::sync::Mutex<Vec<Value>> = std::sync::Mutex::new(Vec::new());
/// A background check of which shown meetings have a Teams transcript (`teams_transcript::check`).
pub(super) static TRANSCRIPT_CHECK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Teams transcripts being imported or that failed, by event id: `{busy: true}` | `{error}`.
pub(super) static IMPORTS: std::sync::Mutex<Option<std::collections::HashMap<String, Value>>> = std::sync::Mutex::new(None);

/// Inbox items whose decision runs in the background right now, by item key → what it is doing
/// (the page shows them working, not with their buttons, until the job ends and they go), and the
/// ones whose job failed → why.
pub(super) fn attention_busy() -> (Value, Value) {
    let (mut busy, mut failed) = (serde_json::Map::new(), serde_json::Map::new());
    if let Some(i) = IMPORTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        for (id, s) in i {
            if s["busy"] == true {
                busy.insert(format!("transcript:{id}"), "Importing…".into());
            } else if let Some(e) = s["error"].as_str() {
                failed.insert(format!("transcript:{id}"), format!("Not imported: {e}").into());
            }
        }
    }
    #[cfg(feature = "brain")]
    if let Ok(r) = BRAIN_INSIGHTS.lock() {
        for layer in r.iter().filter_map(|k| k.strip_prefix("taxonomy:")) {
            busy.insert(format!("drift:{layer}"), "Rebuilding topics…".into());
        }
        for layer in r.iter().filter_map(|k| k.strip_prefix("split:")) {
            busy.insert(format!("drift:{layer}"), "Splitting a topic…".into());
        }
    }
    if PLUGINS_UPDATING.load(std::sync::atomic::Ordering::SeqCst) {
        busy.insert("plugin:stale".into(), "Updating…".into());
    }
    (Value::Object(busy), Value::Object(failed))
}

/// Make a call of a meeting from its Teams transcript, in the background (`teams_transcript::import`);
/// its state shows on the event (`import`) until the call is there.
pub(super) fn import_transcript(id: &str) -> bool {
    let mut g = IMPORTS.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(Default::default);
    if m.get(id).is_some_and(|s| s["busy"] == true) {
        return false;
    }
    m.insert(id.to_string(), json!({"busy": true}));
    let (id, raw) = (id.to_string(), RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e["id"].as_str() == Some(id)).cloned());
    std::thread::spawn(move || {
        // A panic leaves no import running forever: said as a failure.
        let end_id = id.clone();
        let _end = OnEnd(Some(move || {
            let mut g = IMPORTS.lock().unwrap_or_else(|e| e.into_inner());
            let m = g.get_or_insert_with(Default::default);
            if m.get(&end_id).is_some_and(|s| s["busy"] == true) {
                crate::attention::stale();
                m.insert(end_id, json!({"error": "the import stopped unexpectedly"}));
            }
        }));
        let r = raw.map(Ok).unwrap_or_else(|| crate::graph::event(&id)).and_then(|ev| crate::teams_transcript::import(&ev));
        // The Inbox collects again before the import stops showing as running.
        crate::attention::stale();
        let mut g = IMPORTS.lock().unwrap_or_else(|e| e.into_inner());
        let m = g.get_or_insert_with(Default::default);
        // Said where it was asked (the event's card) and wherever the user is now (Needs you, ⌘K).
        match r {
            Ok(d) => {
                crate::app::notify("Teams transcript imported", &format!("{} — its minutes and threads follow", read_json(&d.join("meta.json"))["title"].as_str().unwrap_or("The meeting")));
                m.remove(&id)
            }
            Err(e) => {
                crate::app::notify("Teams transcript not imported", &format!("{e:#}"));
                m.insert(id, json!({"error": format!("{e:#}")}))
            }
        };
    });
    true
}
pub(super) const CALENDAR_TTL: std::time::Duration = std::time::Duration::from_secs(120);

pub(super) fn rfc(v: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&chrono::Utc))
}

/// `/api/calendar?start=YYYY-MM-DD&days=N[&fresh=1]`: events in [start, start + days) local
/// time, each with `recorded` (the call made of it) and `scheduled`, plus recordings that match
/// no event. Without a Microsoft 365 sign-in: today's agenda only (`source: "agenda"`).
pub(super) fn calendar(q: &str) -> Value {
    use chrono::{Local, NaiveDate};
    let start = query_param(q, "start").and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()).unwrap_or_else(|| Local::now().date_naive());
    let days = query_param(q, "days").and_then(|d| d.parse::<i64>().ok()).unwrap_or(1).clamp(1, 14);
    let to = start.checked_add_days(chrono::Days::new(days as u64)).and_then(crate::calendar::local_midnight);
    let (Some(from), Some(to)) = (crate::calendar::local_midnight(start), to) else { return json!({ "error": "bad date" }) };
    let signed_in = crate::graph::account().is_some();

    // Each event's meeting (people, series): which projects it belongs to.
    let mut meetings: std::collections::HashMap<String, crate::meeting::MeetingContext> = std::collections::HashMap::new();
    let (events, source, error): (Vec<Value>, &str, Option<String>) = if signed_in {
        let key = format!("{start}/{days}");
        let cached = CALENDAR_CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            .filter(|(k, at, _)| *k == key && at.elapsed() < CALENDAR_TTL && query_param(q, "fresh").is_none())
            .map(|(_, _, ev)| ev.clone());
        match cached.map(Ok).unwrap_or_else(|| crate::graph::calendar_view(from, to)) {
            Ok(raw) => {
                *CALENDAR_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, std::time::Instant::now(), raw.clone()));
                *RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()) = raw.clone();
                for m in raw.iter().filter_map(crate::calendar::to_meeting) {
                    if let Some(id) = m.id.clone() {
                        meetings.insert(id, m);
                    }
                }
                (raw.iter().filter_map(crate::calendar::view_event).collect(), "microsoft365", None)
            }
            Err(e) => (vec![], "microsoft365", Some(format!("{e:#}"))),
        }
    } else {
        let agenda = watch::agenda();
        for m in &agenda {
            if let Some(id) = m.id.clone() {
                meetings.insert(id, m.clone());
            }
        }
        let ev: Vec<Value> = agenda.into_iter().map(|m| json!({
            "id": m.id, "subject": m.subject, "start": m.start, "end": m.end, "all_day": false, "online": true,
            "app": m.app().map(|a| a.key()), "join_url": m.join_url, "declined": false, "cancelled": false, "no_notetakers": false,
            "people": m.attendees.len(), "names": m.attendees.iter().filter_map(|a| Some(a.name.trim()).filter(|n| !n.is_empty()).or(a.email.as_deref())).take(50).collect::<Vec<_>>(),
            "organizer": m.organizer.map(|o| o.name),
        })).collect();
        (ev, "agenda", None)
    };

    // Recorded calls in the range.
    let calls: Vec<(String, Value, Value)> = calls::all_calls()
        .into_iter()
        .filter_map(|d| {
            let meta = read_json(&d.join("meta.json"));
            let s = rfc(&meta["started_at"])?;
            let name = d.file_name()?.to_string_lossy().into_owned();
            (s >= from - chrono::Duration::hours(1) && s < to).then(|| (name, meta, read_json(&d.join("meeting.json"))))
        })
        .collect();
    let recording = watch::running_watcher().and_then(|w| w.recording);
    let mut used = vec![false; calls.len()];
    let scheduled: Vec<(String, Vec<String>)> = crate::schedule::load().into_iter().filter_map(|m| Some((m.id?, m.languages))).collect();
    let raw_events = RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let offers = crate::teams_transcript::offers();
    let tbook = crate::calls::type_book();
    let mut to_check = vec![];
    let mut out = vec![];
    for mut e in events {
        let (es, ee) = (rfc(&e["start"]), rfc(&e["end"]));
        let subject = e["subject"].as_str().unwrap_or("").to_lowercase();
        // Its call: the one of its id recorded at its time (`calendar::call_of_meeting`: a meeting moved or held again
        // keeps its id, and a call of it at another time isn't this one's) …
        let by_id = e["id"].as_str().zip(es).and_then(|(id, s)| {
            let ids = calls.iter().enumerate().map(|(i, (_, meta, meeting))| (meeting["id"].as_str().filter(|_| !used[i]), rfc(&meta["started_at"])));
            crate::calendar::call_of_meeting(id, s, ee, ids)
        });
        // … else one of the same title near it.
        let hit = by_id.map(|i| (i, &calls[i])).or_else(|| calls.iter().enumerate().find(|(i, (_, meta, meeting))| {
            if used[*i] {
                return false;
            }
            let Some(cs) = rfc(&meta["started_at"]) else { return false };
            let near = es.zip(ee).is_some_and(|(s, en)| cs >= s - chrono::Duration::minutes(15) && cs <= en);
            let same_title = meeting["subject"].as_str().or(meta["title"].as_str()).is_some_and(|t| t.to_lowercase() == subject);
            near && same_title
        }));
        if let Some((i, (name, meta, _))) = hit {
            used[i] = true;
            let dir = paths::calls_dir().join(name);
            e["recorded"] = json!({ "name": name, "status": status(&dir, recording.as_deref()), "minutes": minutes(meta), "imported": crate::teams_transcript::imported(meta) });
            e["projects"] = json!(crate::projects::of_call(&dir).map(|ps| ps.iter().map(|(p, w)| crate::projects::chip(p, w)).collect::<Vec<_>>()).unwrap_or_default());
        } else if let Some(m) = e["id"].as_str().and_then(|id| meetings.get(id)) {
            e["projects"] = json!(crate::projects::for_event(m));
        }
        // Its prep (`prep.rs`): Show prep when written; a meeting of a thread today can be prepared now; another day's
        // gets one on its morning.
        if let Some(eid) = e["id"].as_str() {
            let id = crate::prep::id_of(eid);
            e["prep"] = json!({ "id": id, "ready": crate::prep::load(&id).is_some(), "writing": crate::prep::writing(&id) });
        }
        // What kind of meeting it is, from its invite (set right here, for the whole series).
        if let Some(m) = e["id"].as_str().and_then(|id| meetings.get(id)) {
            e["meeting_type"] = crate::calls::event_type_in(m, &tbook);
        }
        // A recurring meeting: its series and what Echo does with every meeting of it (set right here).
        if let Some(m) = e["id"].as_str().and_then(|id| meetings.get(id)).filter(|m| m.series_id.is_some()) {
            e["series"] = json!({ "key": crate::series::key_of(m), "rule": crate::series::rule_for(m) });
        }
        if let Some(st) = e["id"].as_str().and_then(|id| IMPORTS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(id).cloned())) {
            e["import"] = st;
        }
        let sched = e["id"].as_str().and_then(|id| scheduled.iter().find(|s| s.0 == id));
        e["scheduled"] = json!(sched.is_some());
        if let Some((_, l)) = sched.filter(|s| !s.1.is_empty()) {
            e["scheduled_languages"] = json!(l);
        }
        // Ended, not recorded: does Teams have its transcript? Known → `transcript`; else asked in the background.
        if let Some(raw) = e["recorded"].is_null().then(|| raw_events.iter().find(|r| r["id"] == e["id"])).flatten() {
            let (has, ask) = crate::teams_transcript::known(raw, &offers);
            if let Some(h) = has {
                e["transcript"] = json!(h);
            }
            if ask {
                to_check.push(raw.clone());
            }
        }
        if e["recorded"].is_null() {
            let recap = e["subject"].as_str().map(calls::recap).unwrap_or_default();
            if !recap.is_empty() {
                e["recap"] = json!(recap);
            }
        }
        out.push(e);
    }
    let extra: Vec<Value> = calls
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, (name, meta, _))| {
            let dir = paths::calls_dir().join(name);
            json!({ "name": name, "title": meta["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Call"),
                    "start": meta["started_at"], "end": meta["ended_at"], "status": status(&dir, recording.as_deref()) })
        })
        .collect();
    if !to_check.is_empty() && !TRANSCRIPT_CHECK.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(move || {
            if let Err(e) = crate::teams_transcript::check(&to_check, false) {
                eprintln!("teams transcripts: {e:#}");
            }
            TRANSCRIPT_CHECK.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
    json!({ "source": source, "signed_in": signed_in, "error": error, "start": start.to_string(), "days": days,
            "events": out, "calls": extra, "checking_transcripts": TRANSCRIPT_CHECK.load(std::sync::atomic::Ordering::SeqCst) })
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        "/api/attention" => {
            // Marketplaces fetched again in the background now and then: a plugin's update reaches the Inbox without
            // a visit to Settings → Plugins.
            refresh_markets_for_inbox();
            // What runs, read first: a job ending meanwhile then shows as running over a list that may
            // still hold its item, never as an item with its buttons back.
            let (busy, failed) = attention_busy();
            let mut v = crate::attention::view(req.uri().query().is_some_and(|q| q.contains("fresh")));
            v["busy"] = busy;
            v["failed"] = failed;
            json_reply(v)
        }
        "/api/calendar" => json_reply(calendar(req.uri().query().unwrap_or(""))),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, TimeZone};

    #[test]
    fn a_meeting_held_again_is_the_recording_of_its_time_in_the_calendar() {
        // A meeting moved to later the same day keeps its id: the Calendar said "recorded" with the length and minutes of
        // the call held at its old time, not the recording of it now.
        let (_env, _root) = crate::paths::testenv::folders("window-calendar-held-again");
        let day = Local::now().date_naive();
        let start = Local.from_local_datetime(&day.and_hms_opt(12, 0, 0).unwrap()).unwrap();
        let ev: crate::meeting::MeetingContext = serde_json::from_value(json!({"id": "ev-sync", "subject": "Acme weekly sync", "start": start.to_rfc3339(),
            "end": (start + chrono::Duration::minutes(30)).to_rfc3339(), "response": "accepted"})).unwrap();
        watch::write_agenda(std::slice::from_ref(&ev)).unwrap();
        let call = |at: chrono::DateTime<Local>, ended: bool| {
            let dir = paths::calls_dir().join(format!("{}-acme-weekly-sync", at.format("%Y-%m-%d_%H%M%S")));
            fs::create_dir_all(&dir).unwrap();
            let end = ended.then(|| (at + chrono::Duration::minutes(31)).to_rfc3339());
            fs::write(dir.join("meta.json"), json!({"title": "Acme weekly sync", "started_at": at.to_rfc3339(), "ended_at": end}).to_string()).unwrap();
            ev.save(&dir).unwrap();
            dir
        };
        let earlier = call(start - chrono::Duration::hours(4), true);
        fs::write(earlier.join("notes.md"), "# Acme weekly sync\n").unwrap();
        let live = call(start + chrono::Duration::minutes(2), false);
        fs::create_dir_all(paths::state_dir()).unwrap();
        fs::write(paths::state_dir().join("watch.json"), json!({"pid": std::process::id(), "heartbeat": chrono::Utc::now().to_rfc3339(), "app": "teams",
            "in_call": true, "armed": false, "recording": live.display().to_string()}).to_string()).unwrap();
        let name = |d: &Path| d.file_name().unwrap().to_string_lossy().into_owned();

        let v = calendar(&format!("start={day}&days=1"));
        let events = v["events"].as_array().unwrap();
        assert_eq!(events.len(), 1, "{v:#}");
        assert_eq!(events[0]["recorded"]["name"], name(&live), "{v:#}");
        assert_eq!(events[0]["recorded"]["status"], "recording");
        assert!(events[0]["recorded"]["minutes"].is_null(), "no length of the earlier call: {v:#}");
        // The earlier call is a recording of its own on the day, with its minutes.
        let calls = v["calls"].as_array().unwrap();
        assert_eq!(calls.iter().map(|c| c["name"].clone()).collect::<Vec<_>>(), [json!(name(&earlier))]);
        assert_eq!(calls[0]["status"], "notes");
    }
}

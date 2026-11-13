//! The watcher loop of the echoctl app (`app.rs`): records the calls the user opted in to.
//!
//! Every second it checks the agenda, whether the call app holds the mic, and control files;
//! it records only when the user armed it (tray, `echoctl arm` / `join`, or a "Record?"
//! dialog). Events are appended to `<home>/.echo/events.jsonl` for any client (the Claude
//! Code mod, `echoctl events --follow`).
//!
//! Timings: offer at T−58 s, until 4 min after the
//! start; a call has ended once the mic has been released for 2 s, and we wait a further grace
//! period for a rejoin before closing the recording.

use crate::app::NoWindow;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::apps::TargetApp;
use crate::audio;
use crate::meeting::MeetingContext;
use crate::paths;
use crate::recorder::Recording;

mod call_app;
mod chat;
pub use chat::*;

const TICK: Duration = Duration::from_millis(250);
const OFFER_BEFORE: chrono::Duration = chrono::Duration::seconds(58);
const OFFER_UNTIL_AFTER_START: chrono::Duration = chrono::Duration::minutes(4);
/// Mic released this long = the call app left the call (filters device-switch blips).
const MIC_RELEASE_DEBOUNCE: Duration = Duration::from_secs(2);
/// After the call ends, keep the recording open this long in case the user rejoins.
const REJOIN_GRACE: Duration = Duration::from_secs(10);
/// How often a recording looks at which call Teams is in (`call_seen`): calls back to back — the next one joined
/// before the last one's end was seen, or within `REJOIN_GRACE` — must not run into one recording.
const CALL_LOOK_EVERY: Duration = Duration::from_secs(5);
/// Two readings of Teams' call timer that disagree on when the call began by more than this are two calls.
const OTHER_CALL_BEGAN_S: i64 = 90;
/// "Another call" must read the same on looks at least this far apart before the recording is split: a real timer's start
/// stays put; a clock-like text the reader picked up instead (a chat time, a countdown) drifts and never confirms.
const CONFIRM_AFTER: Duration = Duration::from_secs(9);
/// Two readings of a start within this are the same reading.
const SAME_BEGAN_S: i64 = 3;
/// How long after the mic came back a different name may say "another call" (the new call's window takes a moment).
const REJOIN_NAME_WINDOW: Duration = Duration::from_secs(30);
/// An arm with no call appearing within this window expires.
const ARM_EXPIRES: Duration = Duration::from_secs(15 * 60);
/// A watcher whose heartbeat is older than this is considered dead.
const HEARTBEAT_STALE: chrono::Duration = chrono::Duration::seconds(5);
/// How late a recording watcher's heartbeat may be for Stop to still reach it.
const STOP_LATE: chrono::Duration = chrono::Duration::minutes(10);
const EVENTS_MAX_BYTES: u64 = 5_000_000;
/// Mute button unreadable this long → record the mic again; this long → look for Teams anew.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
const MUTE_BLIND_UNMUTE: Duration = Duration::from_secs(5);
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
const MUTE_BLIND_RESET: Duration = Duration::from_secs(10);
/// How often the meeting chat is read during a recording.
const CHAT_POLL: Duration = Duration::from_secs(30);
/// Chat kept from this long before the call started to this long after it ended.
const CHAT_MARGIN: chrono::Duration = chrono::Duration::minutes(2);
/// How long to look for an ad-hoc call's chat after the recording starts.
const CHAT_FIND_FOR: Duration = Duration::from_secs(5 * 60);
/// How often the call app's window text is read during a recording.
#[cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]
/// Twice a second: who is lit in Teams' tiles changes at the pace of a conversation
/// (`speaking.jsonl`), and the UI already lags the audio by ~0.5 s.
const UI_POLL: Duration = Duration::from_millis(500);
/// A wall-clock jump this large between ticks means the computer slept.
const SLEEP_GAP: Duration = Duration::from_secs(30);
/// Recovery looks at calls this recent.
const RECOVER_DAYS: i64 = 7;
/// How often the watcher refreshes today's agenda from the calendar.
const AGENDA_SYNC_EVERY: Duration = Duration::from_secs(15 * 60);

fn file(name: &str) -> PathBuf {
    paths::state_dir().join(name)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Armed {
    /// Names this arm: a recording takes only the arm that started it (`take_arm`), so one
    /// written meanwhile (the next meeting's) stays. Empty in an arm written before ids.
    #[serde(default)]
    pub id: String,
    pub armed_at: DateTime<Utc>,
    pub title: Option<String>,
    pub meeting: Option<MeetingContext>,
    /// Languages pinned for this call; None = workspace list
    #[serde(default)]
    pub languages: Option<Vec<String>>,
    /// The AI profile chosen for this call (`ai.rs`); None = the default at recording start
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WatchState {
    pub pid: u32,
    pub heartbeat: DateTime<Utc>,
    pub app: TargetApp,
    /// None = this platform can't tell (Windows, for now)
    pub in_call: Option<bool>,
    pub armed: bool,
    pub recording: Option<String>,
    /// The recording is a memo (no call behind it).
    #[serde(default)]
    pub memo: bool,
    /// The app runs without its tray (`app run --no-tray`): a restart keeps it so (#351).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_tray: bool,
}

fn read_json<T: for<'de> Deserialize<'de>>(name: &str) -> Option<T> {
    serde_json::from_slice(&fs::read(file(name)).ok()?).ok()
}

fn write_json<T: Serialize>(name: &str, v: &T) -> Result<()> {
    fs::create_dir_all(paths::state_dir())?;
    let tmp = file(&format!("{name}.tmp"));
    fs::write(&tmp, serde_json::to_vec_pretty(v)?)?;
    fs::rename(tmp, file(name))?;
    Ok(())
}

/// Writing, cancelling, expiring and taking an arm hold this lock, so none of them acts on an
/// arm another has just replaced.
fn arm_lock() -> Result<crate::app::FileLock> {
    fs::create_dir_all(paths::state_dir())?;
    let path = file("armed.lock");
    let t = Instant::now();
    loop {
        if let Some(l) = crate::app::FileLock::try_take(&path)? {
            return Ok(l);
        }
        if t.elapsed() > Duration::from_secs(5) {
            bail!("{} is held by another process", path.display());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Remove the pending arm if it is still the one named `id` (not replaced or cancelled
/// meanwhile). Whether it was.
fn take_arm(id: &str) -> bool {
    let Ok(_lock) = arm_lock() else { return false };
    if read_json::<Armed>("armed.json").is_none_or(|a| a.id != id) {
        return false;
    }
    fs::remove_file(file("armed.json")).is_ok()
}

fn new_arm_id() -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::fill(&mut b);
    format!("{}-{}", Utc::now().timestamp_millis(), b.iter().map(|x| format!("{x:02x}")).collect::<String>())
}

/// Live watcher state, or None if no watcher is running.
pub fn running_watcher() -> Option<WatchState> {
    let s: WatchState = read_json("watch.json")?;
    (Utc::now() - s.heartbeat < HEARTBEAT_STALE).then_some(s)
}

/// The watcher that names a recording, and whether its heartbeat is fresh; a late one while its process lives
/// (blocked on a device, it reads `stop.request` once free).
fn recording_watcher() -> Option<(WatchState, bool)> {
    let s: WatchState = read_json("watch.json").filter(|s: &WatchState| s.recording.is_some())?;
    let age = Utc::now() - s.heartbeat;
    let fresh = age < HEARTBEAT_STALE;
    (fresh || (age < STOP_LATE && (s.pid == std::process::id() || crate::app::process_alive(s.pid)))).then_some((s, fresh))
}

fn emit(event: &str, mut data: serde_json::Value) {
    // The plugins that asked to hear it (the app's host; none in a CLI process): ids and titles, not content.
    #[cfg(feature = "js")]
    if let Some(p) = for_plugins(event, &data) {
        crate::jshost::event(event, &p);
    }
    if let Some(obj) = data.as_object_mut() {
        obj.insert("event".into(), json!(event));
        obj.insert("ts".into(), json!(Local::now().to_rfc3339()));
    }
    let line = data.to_string();
    println!("{line}");
    let _ = fs::create_dir_all(paths::state_dir());
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(file("events.jsonl")) {
        let _ = writeln!(f, "{line}");
    }
}

/// An event as a plugin hears it (`plugins::EVENTS`): a call by its folder's name (not its path), a meeting by its
/// id, subject and start; None for one plugins don't hear.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn for_plugins(event: &str, d: &serde_json::Value) -> Option<serde_json::Value> {
    let call = || d["dir"].as_str().and_then(|p| Path::new(p).file_name()).map(|f| f.to_string_lossy().into_owned());
    Some(match event {
        "meeting_soon" => json!({"id": d["id"], "subject": d["subject"], "start": d["start"]}),
        "call_started" => json!({"app": d["app"]}),
        // `next_call`: Teams went straight on to another call (not a hang-up); the next call_started follows at once.
        "call_ended" => json!({"app": d["app"], "recording": d["recording"], "next_call": d["next_call"] == true}),
        "recording_started" => json!({"call": call(), "title": d["title"], "memo": d["memo"] == true}),
        "recording_stopped" => json!({"call": call(), "duration_s": d["duration_s"], "reason": d["reason"]}),
        "transcribed" => json!({"call": call(), "ok": d["ok"]}),
        _ => return None,
    })
}

pub(crate) fn parse_time(s: &Option<String>) -> Option<DateTime<Utc>> {
    let s = s.as_deref()?;
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| s.parse::<DateTime<Utc>>().ok())
        // Graph often returns naive UTC like "2026-10-09T13:30:00.0000000"
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s.trim_end_matches('Z'), "%Y-%m-%dT%H:%M:%S%.f")
                .ok()
                .map(|n| n.and_utc())
        })
}

/// The agenda the watcher offers meetings from.
pub fn agenda() -> Vec<MeetingContext> {
    read_json("agenda.json").unwrap_or_default()
}

/// A meeting's start time, if parseable.
pub fn meeting_start(m: &MeetingContext) -> Option<DateTime<Utc>> {
    parse_time(&m.start)
}

fn meeting_summary(m: &MeetingContext) -> serde_json::Value {
    json!({
        "id": m.id, "subject": m.subject, "start": m.start, "end": m.end,
        "join_url": m.join_url, "attendees": m.attendees.len(),
    })
}

/// What the tray icon shows.
#[derive(Debug, Clone, PartialEq)]
pub enum UiState {
    Idle { in_call: Option<bool> },
    Armed,
    /// `memo`: the user alone, started by hand (`Recording::start_memo`).
    Recording { title: Option<String>, since: DateTime<Local>, memo: bool },
}

/// The watcher loop. Returns once `stop` is set, after closing any recording.
pub fn run_loop(mut app: TargetApp, stop: Arc<AtomicBool>, on_state: impl Fn(UiState)) -> Result<()> {
    // Keep the event log bounded (clients follow it from the end and survive truncation).
    if fs::metadata(file("events.jsonl")).is_ok_and(|m| m.len() > EVENTS_MAX_BYTES) {
        let _ = fs::rename(file("events.jsonl"), file("events.1.jsonl"));
    }
    // Requests left by a previous run (a timed-out quit, say) must not act on this one.
    for f in ["stop.request", "quit.request", "doctor.request", "open.request", "ai-profile.request", "memo.request", "quit.asking", "quit.cancelled"] {
        let _ = fs::remove_file(file(f));
    }
    let detects_calls = audio::app_in_call(app).is_some();
    emit("watch_started", json!({ "app": app, "pid": std::process::id(), "detects_calls": detects_calls }));

    let mut announced: HashSet<String> = HashSet::new();
    let mut rec: Option<Recording> = None;
    let mut in_call = false;
    let mut mic_released_at: Option<Instant> = None;
    let mut call_ended_at: Option<Instant> = None;
    let mut last_check = Instant::now() - Duration::from_secs(1);
    let mut mute_alive: Option<Arc<AtomicBool>> = None;
    let mut ui_alive: Option<Arc<AtomicBool>> = None;
    let mut frames_alive: Option<Arc<AtomicBool>> = None;
    let mut chat_alive: Option<Arc<AtomicBool>> = None;
    // The arm that was pending when the user stopped a recording while the call went on: it
    // was made for another meeting, so it waits for the next call instead of recording the
    // rest of this one.
    let mut held_arm: Option<String> = None;
    // Stopped by the user while the call went on: no scheduled meeting is armed until that call
    // ends (it would record the rest of this call as that meeting).
    let mut stopped_in_call = false;
    // Which call the recording is of, as Teams' call window shows it (when it began, its name): read while it records.
    let mut rec_call: Option<CallSeen> = None;
    let mut last_call_look = Instant::now();
    // The mic was let go and taken again during the recording (a rejoin, or the next call): for a while after, a call's
    // name alone may say it is another call (a title can change within one call).
    let mut rejoined_at: Option<Instant> = None;
    // A reading that says "another call", waiting to be confirmed by a later one (`judge`).
    let mut candidate: Option<Candidate> = None;
    let looker = CallLook::default();
    let meet_sign = call_app::MeetSign::default();
    // A meeting's arm waiting while the user is on another call (#483): its id, once said.
    let mut waits_other: Option<String> = None;
    let mut unread_once = false;
    let mut waits_on: Option<String> = None;
    let mut last_arm_look: Option<Instant> = None;
    // Keep today's agenda fresh from the calendar (if one is configured), off the main loop.
    let mut last_sync: Option<Instant> = None;
    // Sleep shows up as wall-clock time passing faster than the monotonic clock.
    let mut wall = std::time::SystemTime::now();
    let mut mono = Instant::now();
    let app_started = Utc::now();
    std::thread::spawn(move || recover(app_started));

    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(TICK);
        crate::app::handle_requests(&stop);
        let slept = wall.elapsed().unwrap_or_default().saturating_sub(mono.elapsed()) > SLEEP_GAP;
        wall = std::time::SystemTime::now();
        mono = Instant::now();
        if slept {
            // The call is over (or long interrupted): close it rather than glue two halves.
            if let Some(r) = rec.take() {
                stop_mute_sync(&mut mute_alive);
                stop_flag(&mut ui_alive);
                stop_flag(&mut frames_alive);
                stop_flag(&mut chat_alive);
                finish(r, "computer_slept");
            }
            in_call = false;
            call_ended_at = None;
            mic_released_at = None;
            held_arm = None;
            stopped_in_call = false;
            emit("woke", json!({}));
        }
        if let Some(r) = rec.as_mut() {
            for (track, e) in r.poll() {
                track_failed(&r.dir, track, &e);
            }
        }
        if last_check.elapsed() < Duration::from_secs(1) {
            continue;
        }
        last_check = Instant::now();
        let now = Utc::now();
        if now.timestamp() % 5 == 0 {
            notify_notes_ready();
        }
        if last_sync.is_none_or(|t| t.elapsed() >= AGENDA_SYNC_EVERY) && rec.is_none() {
            last_sync = Some(Instant::now());
            std::thread::spawn(|| {
                let _ = crate::calendar::sync(true);
                // Meetings of the user's threads nobody recorded that Teams transcribed (Needs you).
                if let Err(e) = crate::teams_transcript::scan() {
                    eprintln!("teams transcripts: {e:#}");
                }
            });
        }
        let mut armed: Option<Armed> = read_json("armed.json");
        // A meeting chosen ahead of time ("Record when it starts"): arm it ~1 min before.
        if armed.is_none() && rec.is_none() && !stopped_in_call {
            if let Some(m) = crate::schedule::due(now) {
                let subject = m.subject.clone();
                if arm(subject.clone(), Some(m), None, None).is_ok() {
                    emit("scheduled_armed", json!({ "subject": subject }));
                    armed = read_json("armed.json");
                }
            }
        }

        // 1. Agenda → offer "join & record" once per meeting, from T−58 s to T+4 min.
        let agenda: Vec<MeetingContext> = read_json("agenda.json").unwrap_or_default();
        for m in &agenda {
            let Some(start) = parse_time(&m.start) else { continue };
            // No join link (some calendar sources omit it) still gets an offer: arming then
            // records the call when the user joins from Teams/Outlook themselves.
            if now < start - OFFER_BEFORE || now > start + OFFER_UNTIL_AFTER_START || m.recordable().is_err() {
                continue;
            }
            let key = format!("{}@{}", m.id.clone().unwrap_or_default(), start);
            // Only count it as offered when it was: a meeting coming up while another is being
            // recorded is offered once that recording ends (still within the window).
            if armed.is_none() && rec.is_none() && announced.insert(key) {
                let mut ev = meeting_summary(m);
                // `ask` (config `scheduled`): the app asks with a dialog; `previous_notes`:
                // recurring recap, for clients.
                // A series set to never record isn't asked about.
                let ask = crate::config::Config::load().map(|c| c.ask_scheduled()).unwrap_or(true) && crate::series::may_offer(m);
                ev["ask"] = json!(ask);
                if ask {
                    crate::app::offer_meeting(m.clone());
                }
                if let Some(subject) = &m.subject {
                    ev["previous_notes"] = json!(crate::calls::previous_with_notes(subject, None)
                        .map(|p| p.join("notes.md").display().to_string()));
                }
                emit("meeting_soon", ev);
            }
        }

        // 2. Call presence (mic held by a call app: Teams, or Chrome on a Meet call), debounced on release. `app` is
        // the call's app from here on.
        let (teams, chrome) = (audio::app_in_call(TargetApp::Teams), audio::app_in_call(TargetApp::Meet));
        // An arm for a Google Meet meeting is a sign too: the user asked to record that meeting.
        let meet_armed = armed.as_ref().and_then(|a| a.meeting.as_ref()).is_some_and(|m| m.app() == Some(crate::apps::MeetingApp::Meet));
        // While a call's recording is open (its grace after a hang-up too), only its app's call is that call: another app
        // taking the mic then is a new call, found once the recording has ended.
        let on = rec.as_ref().filter(|r| !r.memo).map(|r| r.app()).or(in_call.then_some(app));
        let (detected, on_app) = call_app::which_call(on, teams, chrome, || meet_sign.get().or(meet_armed.then_some(TargetApp::Meet)));
        app = on_app.unwrap_or(app);
        match detected {
            Some(true) => {
                mic_released_at = None;
                if !in_call {
                    in_call = true;
                    call_ended_at = None;
                    if rec.is_none() {
                        // `ask`: whether Claude should prompt for an unscheduled call (config).
                        let ask = armed.is_none()
                            && crate::config::Config::load().map(|c| c.ask_unscheduled()).unwrap_or(true);
                        emit("call_started", json!({ "app": app, "armed": armed.is_some(), "ask": ask }));
                        if ask {
                            crate::app::offer_call(app);
                        }
                    } else if rec.as_ref().is_some_and(|r| r.memo) {
                        // One recording at a time: a memo goes on, and this call isn't recorded.
                        emit("call_during_memo", json!({ "app": app }));
                        crate::app::notify(&format!("A {} call started", app.name()), "It isn't recorded while your memo records: stop the memo to record it.");
                    } else {
                        rejoined_at = Some(Instant::now());
                        last_call_look = Instant::now() - CALL_LOOK_EVERY; // look at once
                        emit("call_resumed", json!({ "app": app }));
                    }
                }
            }
            Some(false) if in_call => {
                let since = *mic_released_at.get_or_insert_with(Instant::now);
                if since.elapsed() >= MIC_RELEASE_DEBOUNCE {
                    in_call = false;
                    call_ended_at = Some(Instant::now());
                    held_arm = None;
                    stopped_in_call = false;
                    last_arm_look = None;
                    unread_once = false;
                    emit("call_ended", json!({ "app": app, "recording": rec.is_some() }));
                }
            }
            _ => {}
        }
        // Platforms that can't detect calls start on arm and stop on `echoctl stop`.
        let call_active = detected.map(|_| in_call).unwrap_or(true);

        // Calls back to back: the call going on now isn't the one being recorded (Teams' timer says it began at
        // another time, steadily; or its window names another call right after the mic came back) — the recording
        // ends here, and this call is a new one: recorded when an arm (or a scheduled meeting) waits for it, else
        // offered as any call that starts. Looked at off this thread (`CallLook`): a busy Teams never stalls the loop.
        if rec.as_ref().is_some_and(|r| !r.memo) && in_call && app == TargetApp::Teams {
            if last_call_look.elapsed() >= CALL_LOOK_EVERY {
                last_call_look = Instant::now();
                if crate::config::reads_call_window() { looker.start() }
            }
            if let Some(seen) = looker.take() {
                let fresh_rejoin = rejoined_at.is_some_and(|t| t.elapsed() < REJOIN_NAME_WINDOW);
                let started = rec.as_ref().map(|r| r.started_at.with_timezone(&Utc));
                match judge(&mut rec_call, &mut candidate, seen, fresh_rejoin, started, Instant::now()) {
                    Judged::Same => {}
                    Judged::Other => {
                        emit("next_call", json!({ "app": app, "was": rec_call.as_ref().and_then(|c| c.name.clone()) }));
                        // The last call is over as far as anything that reads the events can tell (`find_call_chat`).
                        emit("call_ended", json!({ "app": app, "recording": true, "next_call": true }));
                        stop_mute_sync(&mut mute_alive);
                        stop_flag(&mut ui_alive);
                        stop_flag(&mut frames_alive);
                        stop_flag(&mut chat_alive);
                        finish(rec.take().unwrap(), "next_call");
                        rec_call = None;
                        candidate = None;
                        rejoined_at = None;
                        call_ended_at = None;
                        held_arm = None;
                        stopped_in_call = false;
                        // A meeting set to record when it starts: armed now (it couldn't be while the last call recorded).
                        if armed.is_none() {
                            if let Some(m) = crate::schedule::due(now) {
                                let subject = m.subject.clone();
                                if arm(subject.clone(), Some(m), None, None).is_ok() {
                                    emit("scheduled_armed", json!({ "subject": subject }));
                                    armed = read_json("armed.json");
                                }
                            }
                        }
                        // Else asked as any call — unless the agenda offers its meeting (it does now that nothing records).
                        let offered_by_agenda = crate::calendar::offered_now(&agenda, app, now, OFFER_BEFORE, OFFER_UNTIL_AFTER_START);
                        let ask = armed.is_none() && !offered_by_agenda && crate::config::Config::load().map(|c| c.ask_unscheduled()).unwrap_or(true);
                        emit("call_started", json!({ "app": app, "armed": armed.is_some(), "ask": ask }));
                        if ask {
                            crate::app::offer_call(app);
                        }
                    }
                }
            }
        }

        // A memo (`echoctl memo start`, the window, the menu): the user alone, no call needed. One
        // recording at a time: refused while one records.
        if let Some(req) = read_json::<Value>("memo.request") {
            let _ = fs::remove_file(file("memo.request"));
            if rec.is_some() {
                emit("error", json!({ "message": "a recording is on: the memo didn't start" }));
                crate::app::notify("Memo not started", "Echo is recording already — stop that first.");
            } else {
                let languages = req["languages"].as_array().map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect::<Vec<_>>()).filter(|l| !l.is_empty());
                let ai_profile = Some(crate::ai::pin(req["ai"].as_str().filter(|p| !p.is_empty()), None));
                match Recording::start_memo(app, languages, ai_profile) {
                    Ok(r) => {
                        crate::livenotes::recording_started(&r.dir);
                        emit("recording_started", json!({ "dir": r.dir.display().to_string(), "title": "Memo", "memo": true }));
                        rec = Some(r);
                    }
                    Err(e) => {
                        emit("error", json!({ "message": format!("could not start the memo: {e:#}") }));
                        crate::app::notify("Could not start the memo", &format!("{e:#}"));
                    }
                }
            }
        }

        // 3. Start: armed + call active.
        // The arm is taken (removed) before the recording starts, under its lock: one cancelled
        // or replaced since this tick read it doesn't start, and one written during the
        // recording (the next meeting's) is never the one removed.
        // An arm waiting for its meeting's call while the user is on another one stays until the meeting ends (#483).
        // A meeting's arm starts only on a call of its app (a Meet meeting's in Chrome, a Teams one's in Teams).
        let startable = armed.as_ref().filter(|a| {
            a.meeting.as_ref().is_none_or(|m| m.is_in(app)) && held_arm.as_deref() != Some(a.id.as_str()) && ((now - a.armed_at).to_std().unwrap_or_default() <= ARM_EXPIRES || waiting_for(waits_other.as_deref(), a, now))
        });
        // A meeting's arm starts only on that meeting's call: while the user is still on another call (one that began
        // before this meeting could, or whose window names another), it waits — looked at again every few seconds —
        // and starts once they are on this one (#483). A call window that can't be read changes nothing.
        let on_its_call = match startable.filter(|_| rec.is_none() && call_active && detected.is_some() && app == TargetApp::Teams && call_window_readable()).and_then(|a| a.meeting.as_ref().map(|m| (a.id.clone(), m))) {
            None => true,
            Some((id, m)) => match looker.take() {
                Some(seen) if last_arm_look.is_some() => match meetings_call(m, &seen) {
                    // Waiting already, and this look reads nothing: one such look isn't enough to start (a window
                    // missed during a switch); the next one decides.
                    // The call that made it wait, still named in its window (its timer not read this time): still that call.
                    None if waits_other.as_deref() == Some(id.as_str()) && seen.name.is_some() && seen.name == waits_on => {
                        last_arm_look = Some(Instant::now());
                        false
                    }
                    None if waits_other.as_deref() == Some(id.as_str()) && !unread_once => {
                        unread_once = true;
                        last_arm_look = Some(Instant::now());
                        false
                    }
                    Some(false) => {
                        unread_once = false;
                        waits_on = seen.name.clone();
                        if waits_other.as_deref() != Some(id.as_str()) {
                            waits_other = Some(id.clone());
                            let subject = m.subject.clone().unwrap_or_else(|| "the meeting".into());
                            emit("arm_waits_other_call", json!({ "subject": subject, "on": seen.name }));
                            crate::app::notify(&format!("Waiting to record “{subject}”"), "You're on another call: Echo records it once you join this meeting.");
                        }
                        last_arm_look = Some(Instant::now());
                        false
                    }
                    _ => true,
                },
                _ => {
                    if last_arm_look.is_none_or(|t| t.elapsed() >= CALL_LOOK_EVERY) {
                        last_arm_look = Some(Instant::now());
                        looker.start();
                    }
                    false
                }
            },
        };
        if let Some(a) = startable.filter(|a| rec.is_none() && call_active && on_its_call && take_arm(&a.id)) {
            waits_other = None;
            unread_once = false;
            last_arm_look = None;
            // No meeting given: attach the agenda meeting this call overlaps, if any.
            let meeting = a.meeting.clone().or_else(|| crate::calendar::meeting_now(Utc::now(), app));
            // The AI profile is pinned now: changing the default later never changes this call.
            let ai_profile = Some(pinned_profile(a, meeting.as_ref()));
            match Recording::start(app, a.title.as_deref(), meeting.as_ref(), a.languages.clone(), ai_profile) {
                Ok(r) => {
                    // A scheduled meeting is done once its recording starts (stopping it
                    // must not re-arm it).
                    if let Some(id) = meeting.as_ref().and_then(|m| m.id.as_deref()) {
                        let _ = crate::schedule::remove_id(id);
                    }
                    // What Echo reads from Teams (its mute, window, shared screens, chat): not for a Meet call.
                    let teams = app == TargetApp::Teams;
                    mute_alive = start_mute_sync(teams);
                    ui_alive = if teams { start_ui_capture(&r.dir, r.t0) } else { None };
                    frames_alive = if teams { start_frame_capture(&r.dir, r.t0) } else { None };
                    if meeting.is_none() && teams {
                        find_call_people(&r.dir);
                    }
                    chat_alive = if teams { start_chat_capture(&r.dir, r.started_at, meeting.as_ref()) } else { None };
                    crate::livenotes::recording_started(&r.dir);
                    rec_call = None;
                    candidate = None;
                    rejoined_at = None;
                    last_call_look = Instant::now() - CALL_LOOK_EVERY;
                    emit("recording_started", json!({
                        "dir": r.dir.display().to_string(),
                        "title": a.title.clone().or_else(|| meeting.as_ref().and_then(|m| m.subject.clone())),
                    }));
                    // However it was started (Record, a schedule, Join, the CLI): what a Meet call's remote track holds.
                    if app == TargetApp::Meet {
                        crate::app::notify("Recording the Google Meet call", "Everything Chrome plays is recorded, from every tab.");
                    } else if app == TargetApp::MeetApp {
                        crate::app::notify("Recording the Google Meet app's call", "Its sound is Chrome's: everything Chrome plays is recorded, from every tab too.");
                    }
                    rec = Some(r);
                }
                Err(e) => {
                    // The arm is used up (taken above): retrying every second would only
                    // repeat the error (and the notification).
                    emit("error", json!({ "message": format!("could not start recording: {e:#}") }));
                    crate::app::notify("Could not start recording", &crate::schedule::start_failed(meeting.as_ref(), &e));
                }
            }
        }
        if let (None, Some(a)) = (&rec, &armed) {
            let age = (now - a.armed_at).to_std().unwrap_or_default();
            // Waiting for its meeting's call while the user is on another one: kept until the meeting ends (#483).
            if age > ARM_EXPIRES && !waiting_for(waits_other.as_deref(), a, now) && take_arm(&a.id) {
                emit("arm_expired", json!({ "title": a.title }));
            }
        }

        // The window switched the AI profile of the call being recorded (this loop owns meta.json).
        if let Some(req) = read_json::<Value>("ai-profile.request") {
            let _ = fs::remove_file(file("ai-profile.request"));
            apply_profile_request(rec.as_mut(), &req);
        }

        // 4. Stop: user request, or the call ended and no rejoin within the grace period.
        let user_stop = file("stop.request").exists();
        // A memo has no call to end: only Stop ends it.
        let ended = call_ended_at.is_some_and(|t| t.elapsed() >= REJOIN_GRACE) && !in_call && !rec.as_ref().is_some_and(|r| r.memo);
        if rec.is_some() && (user_stop || ended) {
            // Stop ends this recording only: an arm made meanwhile stays, and waits for the next
            // call when this one goes on.
            // (Not for a memo: the call going on was never this recording — arm and record it.)
            if user_stop && call_active && !rec.as_ref().is_some_and(|r| r.memo) {
                held_arm = read_json::<Armed>("armed.json").map(|a| a.id);
                stopped_in_call = true;
            }
            stop_mute_sync(&mut mute_alive);
            stop_flag(&mut ui_alive);
            stop_flag(&mut frames_alive);
            stop_flag(&mut chat_alive);
            finish(rec.take().unwrap(), if user_stop { "user" } else { "call_ended" });
            rec_call = None;
            candidate = None;
            call_ended_at = None;
        }
        // Taken, or left for a recording that ended meanwhile: never for the next one.
        if user_stop || rec.is_none() {
            let _ = fs::remove_file(file("stop.request"));
        }

        let armed_now = file("armed.json").exists();
        on_state(match &rec {
            Some(r) => UiState::Recording { title: r.title.clone(), since: r.started_at, memo: r.memo },
            None if armed_now => UiState::Armed,
            None => UiState::Idle { in_call: detected.map(|_| in_call) },
        });
        let _ = write_json("watch.json", &WatchState {
            pid: std::process::id(),
            heartbeat: Utc::now(),
            app,
            in_call: detected.map(|_| in_call),
            armed: armed_now,
            recording: rec.as_ref().map(|r| r.dir.display().to_string()),
            memo: rec.as_ref().is_some_and(|r| r.memo),
            no_tray: crate::app::NO_TRAY.load(std::sync::atomic::Ordering::SeqCst),
        });
    }

    if let Some(r) = rec.take() {
        stop_mute_sync(&mut mute_alive);
        stop_flag(&mut ui_alive);
        stop_flag(&mut frames_alive);
        stop_flag(&mut chat_alive);
        finish(r, "watch_stopped");
    }
    #[cfg(feature = "transcribe")]
    crate::transcribe::shutdown(); // no model loaded at exit (#141)
    crate::livenotes::shutdown();
    crate::signin::shutdown();
    crate::notes::shutdown();
    let _ = fs::remove_file(file("watch.json"));
    emit("watch_stopped", json!({}));
    Ok(())
}

/// Which call Teams is in, as its call window shows it: when the call began (its timer, counted from the call's start
/// — a rejoin of the same meeting reads the same), and its name (the window's title).
#[derive(Clone, Debug, Default, PartialEq)]
struct CallSeen {
    began: Option<DateTime<Utc>>,
    name: Option<String>,
    /// The window names a channel ("Meeting in “General”"), not the meeting: its name says nothing about which meeting.
    channel: bool,
}

fn call_seen() -> CallSeen {
    // Development builds and tests: the call window as a file says (`{"began_s_ago": …, "title": …}`), for calls fed
    // from files — the real Teams is never read for them.
    // A test's stand-in set: only it is read, never the real Teams (missing or unreadable: nothing read).
    #[cfg(debug_assertions)]
    if std::env::var_os("ECHOCTL_TEST_CALL_WINDOW").is_some() {
        let v = test_call_window().unwrap_or_default();
        return seen_of(v["began_s_ago"].as_i64().map(|s| s.max(0) as u64), v["title"].as_str());
    }
    let (elapsed, title) = crate::ax::call_glance();
    seen_of(elapsed, title.as_deref())
}

fn seen_of(elapsed: Option<u64>, title: Option<&str>) -> CallSeen {
    let channel = title.is_some_and(|t| t.split(" | ").map(str::trim).any(|p| p.starts_with("Meeting in ")));
    CallSeen { began: elapsed.map(|e| Utc::now() - chrono::Duration::seconds(e as i64)), name: title.and_then(crate::ax::call_name), channel }
}

/// A look at the call window, taken off the watcher's thread (an accessibility query can wait seconds on a busy
/// Teams): one at a time; its result is picked up on a later tick.
#[derive(Default)]
struct CallLook {
    busy: Arc<AtomicBool>,
    out: Arc<std::sync::Mutex<Option<CallSeen>>>,
}

impl CallLook {
    fn start(&self) {
        if self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let (busy, out) = (self.busy.clone(), self.out.clone());
        std::thread::spawn(move || {
            let seen = call_seen();
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(seen);
            busy.store(false, Ordering::SeqCst);
        });
    }

    fn take(&self) -> Option<CallSeen> {
        self.out.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// Development builds and tests: `ECHOCTL_TEST_CALL_WINDOW` names a JSON file standing for the call window.
#[cfg(debug_assertions)]
fn test_call_window() -> Option<Value> {
    let f = std::env::var_os("ECHOCTL_TEST_CALL_WINDOW")?;
    serde_json::from_slice(&fs::read(f).ok()?).ok()
}

/// Whether the call window can be read here (the setting, or a test's stand-in).
fn call_window_readable() -> bool {
    #[cfg(debug_assertions)]
    if std::env::var_os("ECHOCTL_TEST_CALL_WINDOW").is_some() {
        return true;
    }
    crate::config::reads_call_window()
}

/// An arm waiting for its meeting's call while the user is on another one (`waits`: the waiting arm's id): kept until
/// the meeting ends (#483).
fn waiting_for(waits: Option<&str>, a: &Armed, now: DateTime<Utc>) -> bool {
    waits == Some(a.id.as_str()) && a.meeting.as_ref().and_then(|m| parse_time(&m.end)).is_some_and(|end| now < end)
}

/// Whether the call going on is `meeting`'s, from one look at the call window (#483). Its name naming the meeting
/// says yes, however early the call began (people gather early, a bridge stays open). Otherwise the call's timer
/// decides: a call that began before the meeting could (`MATCH_EARLY_MIN` before its start) is another call only when
/// its window names something else too — never on a name alone (a renamed occurrence, a translated or channel title
/// read differently). `None` when the look can't tell.
fn meetings_call(meeting: &MeetingContext, seen: &CallSeen) -> Option<bool> {
    let norm = |s: &str| s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let named = seen.name.as_deref().filter(|_| !seen.channel).map(norm).filter(|n| !n.is_empty());
    let subject = meeting.subject.as_deref().map(norm).filter(|s| !s.is_empty());
    // The meeting's name in the window's, as whole words (a chat "Acme" isn't "Acme design review"); or the window's a
    // cut form of it (three words or more, the start of the meeting's name).
    let same_name = match (&named, &subject) {
        (Some(n), Some(s)) => Some(format!(" {n} ").contains(&format!(" {s} ")) || (n.split(' ').count() >= 3 && s.starts_with(n.as_str()))),
        _ => None,
    };
    if same_name == Some(true) {
        return Some(true);
    }
    let start = parse_time(&meeting.start)?;
    let began = seen.began?;
    if began >= start - chrono::Duration::minutes(crate::calendar::MATCH_EARLY_MIN) {
        return Some(true);
    }
    // Began before the meeting could, and its window doesn't name the meeting: another call (#483) — named otherwise,
    // or with no name to tell (a channel's, none).
    Some(false)
}

/// A reading that says "another call", waiting for a later one to confirm it.
#[derive(Clone, Debug)]
struct Candidate {
    seen: CallSeen,
    at: Instant,
}

#[derive(Debug, PartialEq)]
enum Judged {
    Same,
    Other,
}

/// One look at the call window against the recording's call (`rec`, filled as it is first read). `started`: when the
/// recording started — a call that began well after it isn't the recording's own, however late it is first read.
/// "Another call" only when a later look, `CONFIRM_AFTER` on, reads the same other call (`candidate`).
fn judge(rec: &mut Option<CallSeen>, candidate: &mut Option<Candidate>, seen: CallSeen, rejoined: bool, started: Option<DateTime<Utc>>, now: Instant) -> Judged {
    let known = rec.get_or_insert_with(CallSeen::default);
    // The recording's own start, first read: never one well after the recording started (that call is a later one).
    let late = |b: DateTime<Utc>| started.is_some_and(|s| (b - s).num_seconds() > OTHER_CALL_BEGAN_S);
    let other = match (known.began, seen.began) {
        (None, Some(b)) if late(b) => true,
        _ => another_call(known, &seen, rejoined),
    };
    if !other {
        *candidate = None;
        if known.began.is_none() {
            known.began = seen.began;
        }
        if known.name.is_none() {
            known.name = seen.name;
        }
        return Judged::Same;
    }
    let same_reading = |c: &CallSeen| match (c.began, seen.began) {
        (Some(a), Some(b)) => (a - b).num_seconds().abs() <= SAME_BEGAN_S,
        (None, None) => c.name.is_some() && c.name == seen.name,
        _ => false,
    };
    match candidate {
        Some(c) if same_reading(&c.seen) && now.duration_since(c.at) >= CONFIRM_AFTER => {
            *candidate = None;
            Judged::Other
        }
        Some(c) if same_reading(&c.seen) => Judged::Same, // not long enough yet
        _ => {
            *candidate = Some(Candidate { seen, at: now });
            Judged::Same
        }
    }
}

/// `echoctl call-seen` (dev): what `call_seen` reads now.
pub fn call_seen_json() -> Value {
    let s = call_seen();
    json!({ "began": s.began, "name": s.name })
}

/// Is `now` another call than `rec`, the one being recorded? By the timer when both were read (a new call starts its
/// own from zero; the same meeting rejoined keeps its start); else, right after the mic came back (`rejoined`), by
/// name. Unreadable, or a name change within an unbroken call → the same: a wrong split would cut a call short.
fn another_call(rec: &CallSeen, now: &CallSeen, rejoined: bool) -> bool {
    match (rec.began, now.began) {
        (Some(a), Some(b)) => (a - b).num_seconds().abs() > OTHER_CALL_BEGAN_S,
        _ => rejoined && matches!((&rec.name, &now.name), (Some(a), Some(b)) if !a.eq_ignore_ascii_case(b)),
    }
}

/// The profile a recording pins (`ai::pin`): the arm's choice, its meeting's, the default.
fn pinned_profile(a: &Armed, meeting: Option<&MeetingContext>) -> String {
    crate::ai::pin(a.ai_profile.as_deref(), meeting)
}

/// Apply the window's mid-call profile switch to the recording (which owns `meta.json`). A call
/// that stopped recording meanwhile gets it directly; a switch that can't be applied says why.
fn apply_profile_request(rec: Option<&mut Recording>, req: &Value) {
    let (Some(call), Some(id)) = (req["call"].as_str(), req["profile"].as_str()) else { return };
    // Applied: the live notes follow, and a plainly signed-out Claude profile gets a sign-in
    // (`signin::applied`).
    let applied = |dir: &Path| {
        crate::livenotes::profile_changed(dir);
        crate::signin::applied(call, id);
    };
    match rec {
        Some(r) if r.dir.file_name().and_then(|n| n.to_str()) == Some(call) => match r.set_ai_profile(id) {
            Ok(()) => applied(&r.dir),
            Err(e) => crate::livenotes::report(&r.dir, &format!("couldn't switch to {id}: {e:#}")),
        },
        _ => {
            // By name, else by stamp (retitled since it was asked for).
            let dir = crate::calls::resolve_call(call).ok().or_else(|| crate::calls::by_stamp(&crate::calls::stamp_of(Path::new(call))));
            let Some(dir) = dir else {
                eprintln!("ai: dropped the switch of {call} to {id} — the call can't be found");
                return;
            };
            match crate::ai::set_call_profile(&dir, id) {
                Ok(crate::ai::ProfileChange::Applied) => applied(&dir),
                Ok(crate::ai::ProfileChange::Requested) => {}
                Err(e) => crate::livenotes::report(&dir, &format!("couldn't switch to {id}: {e:#}")),
            }
        }
    }
}

/// A track's audio can no longer be written (a full disk, a volume gone): the rest of it is lost
/// while the recording goes on, so the user hears of it now, once per track.
fn track_failed(dir: &Path, track: &str, e: &str) {
    emit("track_failed", json!({ "dir": dir.display().to_string(), "track": track, "message": e }));
    let what = if track == "mic" { "your microphone" } else { "the others' audio" };
    crate::app::notify(
        "Recording problem",
        &format!("Echo can't save {what} any more: {e}. The rest of this call on that track is lost — take notes in Echo."),
    );
}

fn finish(r: Recording, reason: &str) {
    let secs = r.t0.elapsed().as_secs();
    let dir = r.dir.clone();
    crate::livenotes::recording_stopped(&r.dir);
    match r.finish(reason) {
        Ok((dir, _, live)) => {
            emit("recording_stopped", json!({
                "dir": dir.display().to_string(), "duration_s": secs, "reason": reason,
            }));
            match live {
                Some(live) => finish_transcript(dir, live),
                None => transcribe_in_background(dir),
            }
        }
        Err(e) => {
            // Closed all the same (`Recording::finish` closes both tracks and dates the end).
            emit("recording_stopped", json!({ "dir": dir.display().to_string(), "duration_s": secs, "reason": reason }));
            emit("error", json!({ "message": format!("finishing recording failed: {e:#}") }));
            crate::app::notify("Recording not closed properly", &format!("{e:#}"));
            // What was saved is transcribed all the same: `Recording::finish` closes both tracks
            // and dates the end even when one failed.
            transcribe_in_background(dir);
        }
    }
}

/// Follow the call app's mute button while recording (macOS Accessibility, Windows UI
/// Automation): muted ⇒ the mic
/// track records silence. Emits `mic_muted` / `mic_unmuted` on changes.
fn start_mute_sync(teams: bool) -> Option<Arc<AtomicBool>> {
    use crate::audio::mic::MUTED;
    MUTED.store(false, Ordering::SeqCst);
    if !teams || !crate::config::reads_call_window() || !crate::config::Config::load().map(|c| c.follow_app_mute.unwrap_or(true)).unwrap_or(true) {
        return None;
    }
    #[cfg(any(target_os = "macos", windows))]
    {
        let alive = Arc::new(AtomicBool::new(true));
        let a = alive.clone();
        std::thread::spawn(move || {
            let mut watch = crate::ax::MuteWatch::teams();
            let mut last: Option<bool> = None;
            let mut seen = Instant::now();
            while a.load(Ordering::SeqCst) {
                if watch.is_none() {
                    watch = crate::ax::MuteWatch::teams();
                }
                match watch.as_mut().and_then(|w| w.poll()) {
                    Some(state) => {
                        seen = Instant::now();
                        MUTED.store(state, Ordering::SeqCst);
                        if last != Some(state) {
                            emit(if state { "mic_muted" } else { "mic_unmuted" }, json!({}));
                            last = Some(state);
                        }
                    }
                    None => {
                        // Can't see the button (layout change, Teams restarted): fail open — losing
                        // the user's speech is worse than transcribing a muted moment — and
                        // re-find Teams after a while.
                        if seen.elapsed() > MUTE_BLIND_UNMUTE && last == Some(true) {
                            MUTED.store(false, Ordering::SeqCst);
                            emit("mic_unmuted", json!({ "reason": "mute state unknown" }));
                            last = None;
                        }
                        if seen.elapsed() > MUTE_BLIND_RESET {
                            watch = None;
                            seen = Instant::now();
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(400));
            }
            MUTED.store(false, Ordering::SeqCst);
        });
        Some(alive)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}

/// An ad-hoc call (no invite): who it is with, from Teams' call window title — the chat it was
/// started from — matched to that chat in Microsoft 365 (its members, with emails; and its id
/// for chat capture: a call from a chat posts no "call started" event). Without a sign-in, a
/// title that is a person's name. Written as the call's `meeting.json` (source `teams-call`),
/// which names the remote side of a 1:1 call and lists the participants in the notes.
fn find_call_people(dir: &std::path::Path) {
    #[cfg(any(target_os = "macos", windows))]
    if crate::config::reads_call_window() {
        let dir = dir.to_path_buf();
        std::thread::spawn(move || {
            // The call window can take a few seconds to appear.
            let name = (0..20).find_map(|i| {
                if i > 0 {
                    std::thread::sleep(Duration::from_secs(3));
                }
                crate::ax::call_title().as_deref().and_then(crate::ax::call_name)
            });
            let Some(name) = name else { return };
            let chat = crate::graph::account().and_then(|_| match crate::graph::chat_named(&name) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("call chat by name: {e:#}");
                    None
                }
            });
            let meeting = match chat {
                Some((id, topic, attendees)) => {
                    if !dir.join("chat.id").exists() {
                        let _ = fs::write(dir.join("chat.id"), &id);
                    }
                    MeetingContext { subject: topic, attendees, source: Some("teams-call".into()), ..Default::default() }
                }
                None if crate::ax::is_person_name(&name) => MeetingContext {
                    attendees: vec![crate::meeting::Person { name, email: None }],
                    source: Some("teams-call".into()),
                    ..Default::default()
                },
                None => return,
            };
            if !dir.join(crate::meeting::FILE).exists() {
                let _ = meeting.save(&dir);
            }
        });
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = dir;
}

/// Record the call app's window text during the call (Accessibility / UI Automation): the raw material
/// for naming remote speakers. Config `capture_ui` (default on). Text only, kept in the call.
fn start_ui_capture(dir: &std::path::Path, t0: Instant) -> Option<Arc<AtomicBool>> {
    if !crate::config::reads_call_window() {
        return None; // the user turned "Read the Teams window" off: `capture_ui`
    }
    #[cfg(any(target_os = "macos", windows))]
    {
        let alive = Arc::new(AtomicBool::new(true));
        let a = alive.clone();
        let dir = dir.to_path_buf();
        std::thread::spawn(move || {
            let mut rec = None;
            while a.load(Ordering::SeqCst) {
                let started = Instant::now();
                if rec.is_none() {
                    rec = crate::ax::UiRecorder::teams(&dir, t0);
                }
                if let Some(r) = rec.as_mut() {
                    if !r.poll() {
                        break;
                    }
                }
                std::thread::sleep(UI_POLL.saturating_sub(started.elapsed()));
            }
        });
        Some(alive)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (dir, t0);
        None
    }
}

/// Save key frames of the meeting window during the call (shared screens; macOS needs Screen
/// Recording). Config `capture_frames` (default off).
fn start_frame_capture(dir: &std::path::Path, t0: Instant) -> Option<Arc<AtomicBool>> {
    if crate::config::fed_from_files() || !crate::config::Config::load().map(|c| c.capture_frames).unwrap_or(false) {
        return None;
    }
    #[cfg(any(target_os = "macos", windows))]
    {
        let alive = Arc::new(AtomicBool::new(true));
        let a = alive.clone();
        let dir = dir.to_path_buf();
        std::thread::spawn(move || {
            let mut rec = None;
            while a.load(Ordering::SeqCst) {
                let started = Instant::now();
                if rec.is_none() {
                    rec = crate::frames::FrameRecorder::teams(&dir, t0);
                }
                if let Some(r) = rec.as_mut() {
                    if !r.poll() {
                        break;
                    }
                }
                std::thread::sleep(crate::frames::POLL.saturating_sub(started.elapsed()));
            }
        });
        Some(alive)
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = (dir, t0);
        None
    }
}

fn stop_flag(alive: &mut Option<Arc<AtomicBool>>) {
    if let Some(a) = alive.take() {
        a.store(false, Ordering::SeqCst);
    }
}

fn stop_mute_sync(alive: &mut Option<Arc<AtomicBool>>) {
    if let Some(a) = alive.take() {
        a.store(false, Ordering::SeqCst);
    }
    crate::audio::mic::MUTED.store(false, Ordering::SeqCst);
}

/// Transcribed calls whose minutes haven't appeared yet.
static AWAITING_NOTES: std::sync::Mutex<Vec<(PathBuf, Instant)>> = std::sync::Mutex::new(Vec::new());
const NOTES_WAIT: Duration = Duration::from_secs(3 * 3600);

/// "Minutes ready" once a Claude Code session has written them (minutes and action items; checked
/// every few seconds). A call the app runs the minutes for is said by that run (`notes.rs`), once.
fn notify_notes_ready() {
    let mut list = AWAITING_NOTES.lock().unwrap_or_else(|e| e.into_inner());
    list.retain(|(dir, since)| {
        if dir.join(crate::notes::RUN).exists() {
            return false;
        }
        if dir.join("notes.md").exists() && dir.join(crate::calls::ACTIONS).exists() {
            let meta: serde_json::Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            let title = meta["title"].as_str().map(str::to_owned).unwrap_or_else(|| "your call".into());
            crate::app::notify("Minutes ready", &title);
            #[cfg(feature = "js")]
            crate::share::minutes_written(dir);
            return false;
        }
        since.elapsed() < NOTES_WAIT
    });
}

/// Set on the transcription child the watcher starts: the call it names is closed.
pub const FROM_WATCHER: &str = "ECHOCTL_TRANSCRIBE_FROM_WATCHER";

/// One transcription at a time: whisper on two long calls at once would starve the machine.
static TRANSCRIBING: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// Written when transcription fails, so recovery doesn't retry it at every start.
const TRANSCRIBE_ERROR: &str = "transcribe.error";

/// Run `echoctl transcribe <dir>` as a child process and emit `transcribed`.
fn transcribe_now(dir: &std::path::Path) {
    let _one = TRANSCRIBING.lock().unwrap_or_else(|e| e.into_inner());
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("echoctl"));
    let out = std::process::Command::new(exe).no_window()
        .arg("transcribe")
        .arg(dir)
        .current_dir(paths::home())
        // The recording is closed (meta has ended_at) even if watch.json hasn't caught up yet.
        .env(FROM_WATCHER, dir)
        .output();
    let (ok, detail) = match out {
        Ok(o) if o.status.success() => (true, String::new()),
        Ok(o) => (false, String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("").to_string()),
        Err(e) => (false, e.to_string()),
    };
    // Someone else (a manual `echoctl transcribe`) is on it: not a failure of ours.
    if !ok && detail.contains("already being transcribed") {
        return;
    }
    transcribed(dir, ok, &detail);
}

/// A call's transcript is written (or failed): notify, and emit `transcribed` for the clients.
fn transcribed(dir: &std::path::Path, ok: bool, detail: &str) {
    if ok {
        let _ = fs::remove_file(dir.join(TRANSCRIBE_ERROR));
        AWAITING_NOTES.lock().unwrap_or_else(|e| e.into_inner()).push((dir.to_path_buf(), Instant::now()));
        // Say when the minutes come (or that they come only when asked for).
        let meta: serde_json::Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let title = meta["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Your call");
        use crate::notes::{Due, NotDue};
        match crate::config::Config::load().map(|cfg| crate::notes::auto_due(dir, &cfg, chrono::Local::now(), None)) {
            Ok(Due::At(t)) if crate::notes::auto_missing().is_none() => {
                crate::app::notify("Transcript ready", &format!("{title} — minutes at {}. Add your notes in Echo before then.", t.format("%H:%M")))
            }
            Ok(Due::At(_)) => crate::app::notify("Transcript ready", title),
            Ok(Due::Now) if crate::memo::is_memo(dir) && crate::notes::auto_missing().is_none() => {
                crate::app::notify("Memo saved", "Writing its minutes and finding its threads now.")
            }
            Ok(Due::Not(NotDue::Off)) if crate::app::claude_bin().is_some() && crate::calls::minutes_gate(dir).is_none() => {
                crate::app::notify("Transcript ready", &format!("{title} — open Echo and click Write minutes"))
            }
            _ => {}
        }
    } else {
        let _ = fs::write(dir.join(TRANSCRIBE_ERROR), detail);
        crate::app::notify("Transcription failed", detail);
    }
    emit("transcribed", json!({ "dir": dir.display().to_string(), "ok": ok, "error": detail }));
}

/// The call was transcribed while it was recorded: wait for the worker's last utterances and
/// write the transcript. Any failure falls back to the full pass (`echoctl transcribe`).
#[cfg(feature = "transcribe")]
fn finish_transcript(dir: PathBuf, live: crate::recorder::Live) {
    let busy = crate::live::busy(); // writing the transcript loads the speakers' model: the quit waits for it
    std::thread::spawn(move || {
        let t = Instant::now();
        let written = (|| -> Result<PathBuf> {
            let _marker = crate::transcribe::take_marker(&dir)?;
            let (segs, model) = live.join()?;
            let cfg = crate::config::Config::load()?;
            crate::transcribe::write_transcript(&dir, &cfg, &model, segs, "", false)
        })();
        match written {
            Ok(_) => {
                eprintln!("transcript of {} written {:.0}s after the call", dir.display(), t.elapsed().as_secs_f64());
                transcribed(&dir, true, "");
            }
            Err(_) if crate::live::quitting() => {} // the next start transcribes it (`recover`)
            Err(e) => {
                drop(busy); // the full pass holds its model in a child process, never in this one
                eprintln!("transcription during the call failed ({e:#}); transcribing the recording again");
                transcribe_now(&dir);
            }
        }
    });
}

#[cfg(not(feature = "transcribe"))]
fn finish_transcript(_dir: PathBuf, live: crate::recorder::Live) {
    match live {}
}

/// Transcription needs no Claude turn, so the watcher runs it right away as a child process
/// (`echoctl transcribe`), then emits `transcribed`; the minutes follow from the app's scheduler
/// (`notes::auto_due`) or when asked for.
pub(crate) fn transcribe_in_background(dir: PathBuf) {
    std::thread::spawn(move || transcribe_now(&dir));
}

/// Close a recording a crash left open: fix the WAV headers and date the end by the last
/// write. True if it was unfinished.
pub fn close_unfinished(dir: &std::path::Path) -> Result<bool> {
    let meta_path = dir.join("meta.json");
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(&meta_path)?)?;
    if !meta["ended_at"].is_null() {
        return Ok(false);
    }
    let wavs: Vec<PathBuf> = ["mic.wav", "remote.wav"].iter().map(|f| dir.join(f)).filter(|p| p.exists()).collect();
    for w in &wavs {
        let _ = audio::repair_wav(w);
    }
    let ended = wavs
        .iter()
        .filter_map(|w| fs::metadata(w).and_then(|m| m.modified()).ok())
        .max()
        .map(DateTime::<Local>::from)
        .unwrap_or_else(Local::now);
    // Under the call's meta lock, read again: the AI profile may have been changed meanwhile.
    crate::calls::update_meta(dir, |m| {
        if m["ended_at"].is_null() {
            m["ended_at"] = json!(ended.to_rfc3339());
            m["stop_reason"] = json!("recovered");
        }
        Ok(())
    })?;
    Ok(true)
}

/// At start: finish what a crash, a kill or a power loss left behind — recordings never closed
/// (their WAV headers say 0 bytes), stale "transcribing" markers, and calls with audio but no
/// transcript (last 7 days; ones that already failed are left alone).
fn recover(app_started: DateTime<Utc>) {
    let cutoff = Utc::now() - chrono::Duration::days(RECOVER_DAYS);
    let mut queue = vec![];
    for dir in crate::calls::all_calls() {
        let marker = dir.join(crate::transcribe_marker());
        if let Ok(pid) = fs::read_to_string(&marker) {
            match pid.trim().parse::<u32>() {
                // Still transcribing (a child of the app that ran before): if it dies without a
                // transcript, nobody else would pick the call up.
                Ok(pid) if crate::app::process_alive(pid) => {
                    let dir = dir.clone();
                    std::thread::spawn(move || {
                        while crate::app::process_alive(pid) {
                            std::thread::sleep(Duration::from_secs(5));
                        }
                        let wavs = ["mic.wav", "remote.wav"].iter().any(|f| dir.join(f).exists());
                        if wavs && !dir.join("transcript.md").exists() && !dir.join(TRANSCRIBE_ERROR).exists() {
                            let _ = fs::remove_file(dir.join(crate::transcribe_marker()));
                            transcribe_now(&dir);
                        }
                    });
                    continue;
                }
                _ => drop(fs::remove_file(&marker)),
            }
        }
        let wavs: Vec<PathBuf> = ["mic.wav", "remote.wav"].iter().map(|f| dir.join(f)).filter(|p| p.exists()).collect();
        if wavs.is_empty() || dir.join("transcript.md").exists() || dir.join(TRANSCRIBE_ERROR).exists() {
            continue;
        }
        let meta: serde_json::Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let started = meta["started_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok());
        if started.is_none_or(|s| s.with_timezone(&Utc) < cutoff) {
            continue;
        }
        // A recording started by this run (the first tick can beat this scan) is live, not lost.
        if started.is_some_and(|s| s.with_timezone(&Utc) >= app_started) {
            continue;
        }
        if meta["ended_at"].is_null() && close_unfinished(&dir).unwrap_or(false) {
            emit("recording_recovered", json!({ "dir": dir.display().to_string() }));
        }
        queue.push(dir);
    }
    for dir in queue {
        transcribe_now(&dir);
    }
}

/// `echoctl arm`: opt in to recording the current/next call.
pub fn arm(title: Option<String>, meeting: Option<MeetingContext>, languages: Option<Vec<String>>, ai_profile: Option<String>) -> Result<()> {
    // Only a meeting Echo can record (not a Zoom or Google Meet one, yet); its series' language, where nobody chose one.
    let meeting = meeting.map(|m| m.recordable().cloned()).transpose()?.map(|mut m| {
        crate::series::fill(&mut m);
        m
    });
    // A meeting scheduled or joined with a language (the window's Record ▾) carries it.
    let languages = languages.or_else(|| meeting.as_ref().map(|m| m.languages.clone()));
    let languages = languages.map(|l| l.into_iter().map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect::<Vec<_>>())
        .filter(|l| !l.is_empty());
    // A scheduled or joined meeting carries its profile too. Never checked here: a profile that
    // is gone (or unreadable settings) must not stop the recording — its AI jobs say why.
    let ai_profile = ai_profile.or_else(|| meeting.as_ref().and_then(|m| m.ai_profile.clone())).filter(|p| !p.is_empty());
    {
        let _lock = arm_lock()?;
        write_json("armed.json", &Armed { id: new_arm_id(), armed_at: Utc::now(), title, meeting, languages, ai_profile })?;
    }
    println!("{}", arm_said(running_watcher().as_ref()));
    Ok(())
}

/// What `arm` says it will record, by the running app's state: while Echo records a call, the arm is for the next one
/// (a recording starts only when none is on, #350); while it records a memo, for the call on once the memo stops.
fn arm_said(w: Option<&WatchState>) -> &'static str {
    match w {
        Some(w) if w.recording.is_some() && w.memo => "armed — Echo is recording a memo; the call is recorded once the memo stops (expires in 15 min)",
        Some(w) if w.recording.is_some() => "armed — Echo is recording this call; the next call is recorded when it begins (expires in 15 min)",
        Some(w) if w.in_call == Some(true) => "armed — call in progress, recording starts within a second",
        Some(_) => "armed — recording starts when the call begins (expires in 15 min)",
        None => "armed, but the echoctl app is not running — start it with `echoctl app start`",
    }
}

/// `echoctl memo start`: record the user alone now — no call, no thread to pick (its threads are found
/// from what was said). Refused while something records; the running app starts it within a second.
pub fn memo_start(languages: Option<Vec<String>>, ai_profile: Option<String>) -> Result<()> {
    let Some(w) = running_watcher() else { anyhow::bail!("the Echo app is not running — start it with `echoctl app start`") };
    if let Some(r) = &w.recording {
        anyhow::bail!("Echo is recording already ({r}) — stop that first");
    }
    let languages = languages.map(|l| l.into_iter().map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect::<Vec<_>>()).filter(|l| !l.is_empty());
    write_json("memo.request", &json!({ "languages": languages, "ai": ai_profile }))?;
    println!("recording a memo — stop it with `echoctl memo stop`");
    Ok(())
}

/// `echoctl stop`: end the current recording, or cancel the pending arm when nothing records.
/// A recording's Stop leaves an arm made during it (the next meeting's) as it is.
pub fn stop() -> Result<()> {
    match recording_watcher() {
        Some((w, fresh)) => {
            fs::write(file("stop.request"), b"")?;
            let busy = if fresh { "" } else { " (Echo is busy for the moment; it stops as soon as it is free)" };
            println!("stopping {}{busy}", w.recording.unwrap_or_default());
        }
        None => {
            let _lock = arm_lock()?;
            // Cancelling a scheduled meeting's arm cancels the schedule too.
            if let Some(id) = read_json::<Armed>("armed.json").and_then(|a| a.meeting).and_then(|m| m.id) {
                let _ = crate::schedule::remove_id(&id);
            }
            let disarmed = fs::remove_file(file("armed.json")).is_ok();
            println!("{}", if disarmed { "disarmed (nothing was recording)" } else { "nothing to stop" });
        }
    }
    Ok(())
}

/// Opens a meeting's link, in `app` when given (a Google Meet one in Chrome): an error when it can't be (#500: no Chrome
/// installed), so nothing is armed for a meeting that never opened.
#[cfg(target_os = "macos")]
fn open_meeting(url: &str, app: Option<&str>) -> Result<()> {
    let mut open = std::process::Command::new("open");
    if let Some(app) = app {
        open.args(["-a", app]);
    }
    let out = open.arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status()?;
    if !out.success() {
        bail!("couldn't open the meeting{}", app.map(|a| format!(" — is {a} installed?")).unwrap_or_default());
    }
    Ok(())
}

/// `echoctl join`: open the meeting in the desktop app (a Google Meet one in Chrome, where Echo records it) and arm
/// recording for it.
pub fn join(meeting: MeetingContext) -> Result<()> {
    let Some(url) = meeting.recordable()?.join_url.clone() else { bail!("meeting has no join_url") };
    let url = crate::apps::desktop_join_url(&url);
    if !crate::app::open_log_only(&url) {
        #[cfg(target_os = "macos")]
        open_meeting(&url, (meeting.app() == Some(crate::apps::MeetingApp::Meet)).then_some("Google Chrome"))?;
        #[cfg(windows)]
        // Not `cmd /C start`: cmd would split the URL at `&`.
        std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).status()?;
    }
    let title = meeting.subject.clone();
    arm(title, Some(meeting), None, None)
}

pub fn write_agenda(meetings: &[MeetingContext]) -> Result<()> {
    write_json("agenda.json", &meetings)
}

/// `echoctl agenda set`: replace the agenda the watcher offers meetings from.
pub fn set_agenda(meetings: Vec<MeetingContext>) -> Result<()> {
    write_agenda(&meetings)?;
    println!("agenda: {} meeting(s)", meetings.len());
    Ok(())
}

/// `echoctl status`: watcher, arm and agenda state as JSON.
pub fn status() -> Result<()> {
    let agenda: Vec<MeetingContext> = read_json("agenda.json").unwrap_or_default();
    let armed: Option<Armed> = read_json("armed.json");
    let out = json!({
        "watcher": running_watcher(),
        "armed": armed.map(|a| json!({ "armed_at": a.armed_at, "title": a.title })),
        "agenda": agenda.iter().map(meeting_summary).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

/// `echoctl events --follow`: print event lines as they are appended (cross-platform tail).
/// Starts at the end of the file, so re-arming a Monitor never replays old events.
pub fn follow_events() -> Result<()> {
    let path = file("events.jsonl");
    fs::create_dir_all(paths::state_dir())?;
    OpenOptions::new().create(true).append(true).open(&path)?;
    let mut f = fs::File::open(&path)?;
    let mut pos = f.seek(SeekFrom::End(0))?;
    let mut out = std::io::stdout();
    loop {
        let len = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if len < pos {
            pos = 0; // truncated/rotated
        }
        if len > pos {
            f.seek(SeekFrom::Start(pos))?;
            let mut reader = BufReader::new(&f);
            let mut line = String::new();
            while reader.read_line(&mut line)? > 0 {
                if line.ends_with('\n') {
                    write!(out, "{line}")?;
                    out.flush()?;
                    pos += line.len() as u64;
                } else {
                    break; // partial line: wait for the rest
                }
                line.clear();
            }
            f = fs::File::open(&path)?;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_meetings_arm_starts_only_on_that_meetings_call() {
        let at = |s: &str| Some(s.to_string());
        let m = MeetingContext { subject: at("Acme design review"), start: at("2026-11-12T17:30:00Z"), end: at("2026-11-12T18:00:00Z"), ..Default::default() };
        let t = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        let seen = |began: Option<&str>, name: Option<&str>| CallSeen { began: began.map(t), name: name.map(str::to_owned), channel: false };
        // Still on the call before it (began an hour earlier, another name): not its call — wait.
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T16:30:00Z"), Some("Globex weekly"))), Some(false));
        // Its own name decides, however early its call began (people gathered early, a bridge left open).
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T17:05:00Z"), Some("Acme design review"))), Some(true));
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T16:30:00Z"), Some("acme design review | Microsoft Teams"))), Some(true));
        // A call begun for it (from 10 min before), whatever its window says: a renamed occurrence, a translated title.
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T17:31:00Z"), Some("Weekly design sync (moved)"))), Some(true));
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T17:24:00Z"), None)), Some(true), "joined a few minutes early");
        // A name alone never says "another call"; an early call with no name to tell isn't known either.
        assert_eq!(meetings_call(&m, &seen(None, Some("Call with Dana Lee"))), None);
        // Begun an hour before with no name to tell (a channel's, none): another call — the call the user is still on.
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T16:30:00Z"), None)), Some(false));
        // A name that only shares words with it isn't its name; a cut form of its name (three words or more) is.
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T16:30:00Z"), Some("Acme"))), Some(false), "a chat named Acme");
        assert_eq!(meetings_call(&m, &seen(Some("2026-11-12T16:30:00Z"), Some("Acme design rev…"))), Some(true), "Teams cut the title");
        // A channel meeting's window names the channel, not the meeting: no name.
        let channel = CallSeen { began: Some(t("2026-11-12T17:31:00Z")), name: Some("General".into()), channel: true };
        assert_eq!(meetings_call(&m, &channel), Some(true), "a channel meeting begun for it");
        let early_channel = CallSeen { began: Some(t("2026-11-12T16:30:00Z")), ..channel };
        assert_eq!(meetings_call(&m, &early_channel), Some(false), "another channel's call begun an hour before");
        assert!(seen_of(Some(5), Some("Meeting in “General” | Microsoft Teams")).channel && !seen_of(Some(5), Some("Acme design review | Microsoft Teams")).channel);
        assert_eq!(meetings_call(&m, &seen(None, None)), None, "nothing read");
        // The arm waiting for its call is kept until the meeting ends, not past it; another arm isn't.
        let a = Armed { id: "a1".into(), armed_at: t("2026-11-12T17:29:00Z"), title: None, meeting: Some(m.clone()), languages: None, ai_profile: None };
        assert!(waiting_for(Some("a1"), &a, t("2026-11-12T17:50:00Z")), "past the 15 min an arm has, the meeting still on");
        assert!(!waiting_for(Some("a1"), &a, t("2026-11-12T18:01:00Z")), "the meeting is over");
        assert!(!waiting_for(Some("other"), &a, t("2026-11-12T17:50:00Z")));
    }

    use super::*;

    #[test]
    fn arm_says_what_it_will_record() {
        let w = |in_call, recording: Option<&str>| WatchState { pid: 1, heartbeat: Utc::now(), app: TargetApp::Teams, in_call, armed: true, recording: recording.map(String::from), memo: false, no_tray: false };
        // During a memo the arm records the call on once the memo stops (a memo never holds the arm back).
        let memo = WatchState { memo: true, ..w(Some(true), Some("2026-11-10_090000-memo")) };
        assert_eq!(arm_said(Some(&memo)), "armed — Echo is recording a memo; the call is recorded once the memo stops (expires in 15 min)");
        // While Echo records a call, an arm is for the next call: it doesn't start anything now (#350).
        assert_eq!(arm_said(Some(&w(Some(true), Some("2026-11-10_090000-acme-sync")))), "armed — Echo is recording this call; the next call is recorded when it begins (expires in 15 min)");
        assert_eq!(arm_said(Some(&w(Some(true), None))), "armed — call in progress, recording starts within a second");
        assert_eq!(arm_said(Some(&w(Some(false), None))), "armed — recording starts when the call begins (expires in 15 min)");
        assert_eq!(arm_said(None), "armed, but the echoctl app is not running — start it with `echoctl app start`");
    }

    #[test]
    fn a_plugin_hears_a_call_by_its_name_not_its_path() {
        let p = for_plugins("recording_started", &json!({"dir": "/Users/x/echo/calls/2026-11-06_100000-acme", "title": "Acme"})).unwrap();
        assert_eq!(p, json!({"call": "2026-11-06_100000-acme", "title": "Acme", "memo": false}));
        assert_eq!(for_plugins("transcribed", &json!({"dir": "/a/b/c", "ok": true, "error": "x"})).unwrap(), json!({"call": "c", "ok": true}));
        assert!(for_plugins("error", &json!({"message": "x"})).is_none(), "not one plugins hear");
    }

    #[test]
    fn a_split_needs_the_other_call_read_steadily() {
        let base = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
        let at = |s: i64| Some(base + chrono::Duration::seconds(s));
        let seen = |b: Option<DateTime<Utc>>| CallSeen { began: b, name: Some("x".into()), channel: false };
        let t0 = Instant::now();
        let after = |s: u64| t0 + Duration::from_secs(s);

        // The recording's call, then the next call's timer read steadily: Same at first, Other once confirmed 9 s on.
        let (mut rec, mut cand) = (None, None);
        assert_eq!(judge(&mut rec, &mut cand, seen(at(0)), false, at(5), after(0)), Judged::Same);
        assert_eq!(judge(&mut rec, &mut cand, seen(at(2400)), false, at(5), after(5)), Judged::Same, "one reading never splits");
        assert_eq!(judge(&mut rec, &mut cand, seen(at(2401)), false, at(5), after(10)), Judged::Same, "too soon");
        assert_eq!(judge(&mut rec, &mut cand, seen(at(2400)), false, at(5), after(15)), Judged::Other);

        // A clock-like text read instead of the timer (a chat time: the same "elapsed", so its start drifts with now).
        let (mut rec, mut cand) = (None, None);
        judge(&mut rec, &mut cand, seen(at(0)), false, at(5), after(0));
        for i in 1..6 {
            let drifting = at(3000 + 5 * i as i64);
            assert_eq!(judge(&mut rec, &mut cand, seen(drifting), false, at(5), after(5 * i)), Judged::Same, "look {i}");
        }

        // Two call windows flipping (the held call, then the new one): never steady, never split.
        let (mut rec, mut cand) = (None, None);
        judge(&mut rec, &mut cand, seen(at(0)), false, at(5), after(0));
        for i in 1..6 {
            let b = if i % 2 == 0 { at(0) } else { at(2400) };
            assert_eq!(judge(&mut rec, &mut cand, seen(b), false, at(5), after(5 * i)), Judged::Same);
        }

        // Nothing read until the next call: its start, well after the recording started, isn't taken as the recording's.
        let (mut rec, mut cand) = (None, None);
        judge(&mut rec, &mut cand, CallSeen::default(), false, at(5), after(0));
        judge(&mut rec, &mut cand, seen(at(900)), false, at(5), after(5));
        assert_eq!(judge(&mut rec, &mut cand, seen(at(900)), false, at(5), after(15)), Judged::Other);
        assert_eq!(rec.unwrap().began, None);
    }

    #[test]
    fn back_to_back_calls_are_told_apart_by_when_they_began() {
        let at = |s: i64| Some(DateTime::<Utc>::from_timestamp(1_800_000_000 + s, 0).unwrap());
        let seen = |b: Option<DateTime<Utc>>, n: Option<&str>| CallSeen { began: b, name: n.map(str::to_owned), channel: false };
        let rec = seen(at(0), Some("Acme weekly"));
        // The same meeting rejoined (its timer counts from its start): the same call, whatever the window says now.
        assert!(!another_call(&rec, &seen(at(20), Some("Meeting compact view")), true));
        // The next call: its own timer from zero, 40 minutes later — also with no break in the mic.
        assert!(another_call(&rec, &seen(at(2400), Some("Globex sync")), false));
        // No timer: by name, only right after the mic came back; nothing readable: never split.
        assert!(another_call(&seen(None, Some("Acme weekly")), &seen(None, Some("Globex sync")), true));
        assert!(!another_call(&seen(None, Some("Acme weekly")), &seen(None, Some("Globex sync")), false), "a title changing within one call");
        assert!(!another_call(&seen(None, Some("Acme weekly")), &seen(None, Some("acme weekly")), true));
        assert!(!another_call(&seen(None, None), &seen(at(2400), Some("Globex sync")), true));
    }

    #[test]
    fn a_profile_switch_for_a_call_not_recording_is_applied_or_says_why() {
        let (_env, root) = crate::paths::testenv::folders("watch-switch");
        crate::ai::Settings { default: None, live_notes: true, profiles: vec![] }.save().unwrap();
        let d = root.join("home/calls/2026-10-23_230000-x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("meta.json"), r#"{"ai_profile":"!unreadable"}"#).unwrap();
        let profile = |d: &std::path::Path| crate::ai::call_profile_id(d).unwrap();
        apply_profile_request(None, &json!({"call": "2026-10-23_230000-x", "profile": "system"}));
        assert_eq!(profile(&d), "system");
        apply_profile_request(None, &json!({"call": "2026-10-23_230000-x", "profile": "gone"}));
        assert_eq!(profile(&d), "system", "unchanged");
        assert!(crate::livenotes::status(&d)["error"].as_str().unwrap().contains("couldn't switch to gone"));
        // Retitled since it was asked for: found by its stamp.
        let renamed = root.join("home/calls/2026-10-23_230000-retitled");
        fs::rename(&d, &renamed).unwrap();
        fs::write(renamed.join("meta.json"), r#"{"ai_profile":"!unreadable"}"#).unwrap();
        apply_profile_request(None, &json!({"call": "2026-10-23_230000-x", "profile": "system"}));
        assert_eq!(profile(&renamed), "system");
    }

    #[test]
    fn a_profile_problem_never_stops_a_recording() {
        let (_env, root) = crate::paths::testenv::folders("watch-arm");
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        let meeting = MeetingContext { subject: Some("Sync".into()), ai_profile: Some("gone".into()), ..Default::default() };
        arm(None, Some(meeting.clone()), None, None).unwrap();
        let a: Armed = read_json("armed.json").unwrap();
        assert_eq!(a.ai_profile.as_deref(), Some("gone"), "pinned as chosen; its AI jobs say it's gone");
        // Unreadable AI settings: armed all the same.
        fs::write(root.join("config/ai.json"), "{ nope").unwrap();
        arm(None, None, None, None).unwrap();
        let a: Armed = read_json("armed.json").unwrap();
        assert_eq!(pinned_profile(&a, None), crate::ai::UNPINNED);
        // Explicit beats the meeting, which beats the default.
        fs::remove_file(root.join("config/ai.json")).unwrap();
        let mut a = a;
        assert_eq!(pinned_profile(&a, Some(&meeting)), "gone");
        a.ai_profile = Some("chosen".into());
        assert_eq!(pinned_profile(&a, Some(&meeting)), "chosen");
        a.ai_profile = None;
        assert_eq!(pinned_profile(&a, None), crate::ai::SYSTEM);
    }

    /// #228: a Zoom meeting isn't armed or joined (nor a Google Meet one off a Mac, #230): the CLI and the window get
    /// why, and nothing is left armed or opened.
    #[test]
    fn a_zoom_meeting_is_not_armed_or_joined() {
        let (_env, root) = crate::paths::testenv::folders("watch-arm-other-app");
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        let zoom = MeetingContext { subject: Some("Umbrella check-in".into()), join_url: Some("https://umbrella.zoom.us/j/81234567890".into()), ..Default::default() };
        let e = arm(zoom.subject.clone(), Some(zoom.clone()), None, None).unwrap_err();
        assert_eq!(e.to_string(), "“Umbrella check-in” is a Zoom meeting: recording isn't available for Zoom meetings yet");
        assert!(read_json::<Armed>("armed.json").is_none(), "nothing armed");
        assert!(join(zoom.clone()).unwrap_err().to_string().contains("recording isn't available for Zoom meetings yet"), "refused before the link is opened");
        assert!(read_json::<Armed>("armed.json").is_none());
        // A Google Meet meeting arms on a Mac (#230).
        let meet = MeetingContext { join_url: Some("https://meet.google.com/abc-defg-hij".into()), ..zoom.clone() };
        assert_eq!(arm(None, Some(meet), None, None).is_ok(), cfg!(target_os = "macos"));
        let _ = fs::remove_file(file("armed.json"));
        // A Teams meeting, or one with no app known (the test data's links), arms as before.
        let teams = MeetingContext { join_url: Some("https://teams.example/ev-1".into()), ..zoom };
        arm(None, Some(teams), None, None).unwrap();
        assert!(read_json::<Armed>("armed.json").is_some());
    }

    #[test]
    fn a_recording_takes_only_its_own_arm() {
        let (_env, root) = crate::paths::testenv::folders("watch-arm-own");
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        arm(Some("A".into()), None, None, None).unwrap();
        let a: Armed = read_json("armed.json").unwrap();
        // The next meeting armed while A's arm is still on disk (or A records): A's id no
        // longer takes anything.
        arm(Some("B".into()), None, None, None).unwrap();
        let b: Armed = read_json("armed.json").unwrap();
        assert_ne!(a.id, b.id);
        assert!(!take_arm(&a.id), "A's recording leaves B's arm alone");
        assert_eq!(read_json::<Armed>("armed.json").unwrap().title.as_deref(), Some("B"));
        assert!(take_arm(&b.id));
        assert!(read_json::<Armed>("armed.json").is_none());
        assert!(!take_arm(&b.id), "taken once");
        // An arm written before ids is taken by its empty id.
        fs::write(root.join("home/.echo/armed.json"), r#"{"armed_at":"2026-10-23T10:00:00Z","title":"old","meeting":null}"#).unwrap();
        assert!(take_arm(""));
        // Nothing recording: `stop` cancels a pending arm, and a scheduled meeting's schedule.
        let m = MeetingContext { id: Some("ev1".into()), subject: Some("C".into()), ..Default::default() };
        crate::schedule::add(m.clone()).unwrap();
        arm(m.subject.clone(), Some(m), None, None).unwrap();
        stop().unwrap();
        assert!(read_json::<Armed>("armed.json").is_none());
        assert!(!crate::schedule::is_scheduled("ev1"));
    }

    #[test]
    fn stop_reaches_a_recording_watcher_whose_heartbeat_is_late() {
        let (_env, root) = crate::paths::testenv::folders("watch-stop-late");
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        let m = MeetingContext { id: Some("next".into()), subject: Some("Next".into()), ..Default::default() };
        crate::schedule::add(m.clone()).unwrap();
        arm(m.subject.clone(), Some(m), None, None).unwrap();
        let state = |pid: u32, late: i64| {
            let (heartbeat, recording) = (Utc::now() - chrono::Duration::seconds(late), Some("/calls/x".into()));
            let w = WatchState { pid, heartbeat, app: TargetApp::Teams, in_call: Some(true), armed: true, recording, memo: false, no_tray: false };
            fs::write(file("watch.json"), serde_json::to_vec(&w).unwrap()).unwrap();
        };
        // Blocked on a device for 30 s: the request is left for it, and the next meeting stays armed.
        state(std::process::id(), 30);
        stop().unwrap();
        assert!(file("stop.request").exists());
        assert!(read_json::<Armed>("armed.json").is_some(), "the next meeting's arm is kept");
        assert!(crate::schedule::is_scheduled("next"));
        fs::remove_file(file("stop.request")).unwrap();
        // A watcher that is gone, or silent far too long: nothing records, so Stop disarms as before.
        let mut gone = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" }).args(if cfg!(windows) { &["/c", "exit"][..] } else { &[][..] }).spawn().unwrap();
        gone.wait().unwrap();
        for (pid, late) in [(gone.id(), 30), (std::process::id(), 3600)] {
            state(pid, late);
            assert!(recording_watcher().is_none(), "pid {pid}, {late} s late");
        }
        stop().unwrap();
        assert!(!file("stop.request").exists());
        assert!(read_json::<Armed>("armed.json").is_none());
    }

    #[test]
    fn with_the_teams_window_off_nothing_reads_it_during_a_call() {
        let (_env, root) = crate::paths::testenv::folders("watch-no-window");
        let mut c = crate::config::Config::load().unwrap();
        c.capture_ui = Some(false);
        c.follow_app_mute = Some(true);
        c.save().unwrap();
        assert!(start_ui_capture(&root, Instant::now()).is_none(), "no names read");
        assert!(start_mute_sync(true).is_none(), "no mute sync, even with Follow Teams' mute on");
    }

    #[test]
    fn last_call_left_skips_dropouts() {
        let at = |s: &str| DateTime::parse_from_rfc3339(&format!("2026-10-15T{s}+03:00")).unwrap().with_timezone(&Utc);
        // The day this was written: the 1:1 call began at 13:32 and dropped for 5 s at 13:40.
        let marks = [
            (at("11:20:48"), false),
            (at("12:25:25"), true),
            (at("12:30:39"), false),
            (at("13:10:34"), true),
            (at("13:32:23"), false),
            (at("13:40:00"), true),
            (at("13:40:05"), false),
        ];
        assert_eq!(left_from(&marks), Some(at("13:10:34")));
        assert_eq!(left_from(&marks[..1]), None);
    }

    #[test]
    fn graph_style_times_parse() {
        assert!(parse_time(&Some("2026-10-09T13:30:00.0000000".into())).is_some());
        assert!(parse_time(&Some("2026-10-09T16:30:00+03:00".into())).is_some());
        assert!(parse_time(&Some("2026-10-09T13:30:00Z".into())).is_some());
    }

    /// #500: joining a Google Meet meeting without Chrome is an error, not an arm with nothing opened.
    #[cfg(target_os = "macos")]
    #[test]
    fn opening_a_meeting_in_an_app_that_isnt_there_is_an_error() {
        assert!(open_meeting("https://meet.google.com/abc-defg-hij", Some("Echo Test No Such App")).is_err());
    }
}

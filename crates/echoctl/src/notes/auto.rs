//! Minutes: written automatically after a call (split out of `notes.rs`, moved as it was).

use super::*;

// ── automatic minutes ────────────────────────────────────────────────────────────────────────

/// Why a call isn't due for automatic minutes.
#[derive(Clone, Debug, PartialEq)]
pub enum NotDue {
    /// The switch is off, or not started yet.
    Off,
    /// The call ended before the switch was turned on.
    Before,
    /// Not closed, transcribed or still being recorded or transcribed.
    NotReady,
    /// Minutes exist or were tried, or a session is writing them.
    Tried,
    /// Too thin for minutes (`calls::minutes_gate`).
    Gate(&'static str),
    /// The user said Not yet — until they say (`LATER`).
    Held,
}

/// The user's Not yet on a call's automatic minutes (#42): `{"until": <time>}` (later) or `{"manual": true}`
/// (until they write them).
pub const LATER: &str = "minutes-later.json";

/// When the call ended for automatic minutes: its end, or its import (a Teams transcript) when later.
pub(crate) fn ended_at(dir: &Path) -> Option<DateTime<Local>> {
    let meta: Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
    let time = |k: &str| meta[k].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Local));
    let ended = time("ended_at")?;
    Some(time("imported_at").map_or(ended, |i| i.max(ended)))
}

/// What the user said about this call's automatic minutes (an unreadable file holds nothing): Some(None) = until they say, Some(Some(t)) = not before t.
pub(super) fn later(dir: &Path) -> Option<Option<DateTime<Local>>> {
    let v: Value = serde_json::from_slice(&fs::read(dir.join(LATER)).ok()?).ok()?;
    if v["manual"] == true {
        return Some(None);
    }
    v["until"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| Some(t.with_timezone(&Local)))
}

/// Not yet: `minutes` more after when they are due now (None: until the user writes them; Some(0): as before).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn set_later(dir: &Path, minutes: Option<i64>) -> Result<()> {
    let file = dir.join(LATER);
    match minutes {
        Some(0) => match fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        },
        Some(m) => {
            let m = m.clamp(1, 24 * 60);
            let base = match Config::load().map(|c| auto_due(dir, &c, Local::now(), None)) {
                Ok(Due::At(t)) => t,
                _ => Local::now(),
            };
            Ok(paths::write_atomic(&file, json!({"until": (base + chrono::Duration::minutes(m)).to_rfc3339()}).to_string().as_bytes())?)
        }
        None => Ok(paths::write_atomic(&file, br#"{"manual": true}"#)?),
    }
}

/// When a call gets its minutes automatically.
#[derive(Clone, Debug, PartialEq)]
pub enum Due {
    Now,
    At(DateTime<Local>),
    Not(NotDue),
}

/// Is the call in `dir` due for automatic minutes at `now`: the switch on and the call ended after
/// it was turned on; closed and transcribed; never tried; past the gate; and 15 minutes past both
/// its end and the last change of the user's notes (`my-notes.md`'s time: every real change moves
/// it, so nothing else needs storing). `own`: the automatic run asking right before it starts
/// Claude Code — its own record doesn't count as "tried" (an Echo claim never does; only a
/// session's fresh one).
pub fn auto_due(dir: &Path, cfg: &Config, now: DateTime<Local>, own: Option<&Run>) -> Due {
    let (true, Some(since)) = (cfg.auto_minutes, cfg.auto_minutes_since) else { return Due::Not(NotDue::Off) };
    let meta: Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let time = |k: &str| meta[k].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok());
    let Some(ended) = time("ended_at") else { return Due::Not(NotDue::NotReady) };
    // A call made from a Teams transcript reached Echo when it was imported: that is its end here.
    let ended = time("imported_at").map_or(ended, |i| i.max(ended));
    if ended < since {
        return Due::Not(NotDue::Before);
    }
    if !dir.join("transcript.md").exists() || in_progress(dir) {
        return Due::Not(NotDue::NotReady);
    }
    let mine = |r: &Run| own.is_some_and(|o| o.run_id.is_some() && o.run_id == r.run_id);
    if dir.join("notes.md").exists() || raw(dir).is_some_and(|r| !mine(&r) && !abandoned_auto(dir, &r)) {
        return Due::Not(NotDue::Tried);
    }
    if session_claim(dir) {
        return Due::Not(NotDue::Tried); // a session is writing them right now
    }
    if let Some(why) = calls::minutes_gate(dir) {
        return Due::Not(NotDue::Gate(why));
    }
    let noted = fs::metadata(dir.join(calls::USER_NOTES)).and_then(|m| m.modified()).ok().map(DateTime::<Local>::from);
    if noted.is_some() && fs::read_to_string(dir.join(calls::USER_NOTES)).is_err() {
        return Due::Not(NotDue::NotReady); // the user's notes can't be read (the Notes tab says why)
    }
    let last = noted.map_or(ended.with_timezone(&Local), |n| n.max(ended.with_timezone(&Local)));
    // A memo has no call notes to wait for: its minutes (and its threads) come as soon as it is transcribed.
    let delay = if crate::memo::is_memo(dir) { chrono::Duration::zero() } else { chrono::Duration::from_std(auto_delay()).unwrap_or_default() };
    let mut at = last + delay;
    match later(dir) {
        Some(None) => return Due::Not(NotDue::Held),
        Some(Some(t)) => at = at.max(t),
        None => {}
    }
    if now >= at { Due::Now } else { Due::At(at) }
}

/// The automatic-minutes state of a call for the window: `{due: "now"|"at"|"off"|"before"|
/// "not_ready"|"tried"|"gate"|"missing"|"error", at, reason}` — "missing": due, but this computer
/// can't write them (`auto_missing`); "error": the settings can't be read.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn auto_state(dir: &Path) -> Value {
    let cfg = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => return json!({ "due": "error", "reason": format!("Echo's settings can't be read: {e:#}") }),
    };
    if let Some(why) = gave_up(&calls::stamp_of(dir)).or_else(|| BOUNDARY_ERROR.lock().unwrap_or_else(|e| e.into_inner()).clone()) {
        return json!({ "due": "error", "reason": why });
    }
    match auto_due(dir, &cfg, Local::now(), None) {
        Due::Now | Due::At(_) if auto_missing().is_some() => json!({ "due": "missing", "reason": auto_missing() }),
        Due::Now => json!({ "due": "now" }),
        Due::At(t) => {
            // What moved it past 15 minutes after the call: the user's Not yet, or their notes.
            // Said only when that is what set the time: the Not yet, or the notes' last edit (+ 15 minutes) past the end.
            let noted = fs::metadata(dir.join(calls::USER_NOTES)).and_then(|m| m.modified()).ok().map(DateTime::<Local>::from);
            let delay = chrono::Duration::from_std(auto_delay()).unwrap_or_default();
            let moved = if matches!(later(dir), Some(Some(u)) if u == t) {
                json!({"by": "later"})
            } else if let Some(n) = noted.filter(|n| *n + delay == t && ended_at(dir).is_some_and(|e| *n > e + chrono::Duration::seconds(30))) {
                json!({"by": "notes", "at": n.to_rfc3339()})
            } else {
                Value::Null
            };
            json!({ "due": "at", "at": t.to_rfc3339(), "moved": moved })
        }
        Due::Not(NotDue::Held) => json!({ "due": "held" }),
        Due::Not(NotDue::Gate(why)) => json!({ "due": "gate", "reason": why }),
        Due::Not(NotDue::Off) => json!({ "due": "off" }),
        Due::Not(NotDue::Before) => json!({ "due": "before" }),
        Due::Not(NotDue::NotReady) => json!({ "due": "not_ready" }),
        Due::Not(NotDue::Tried) => json!({ "due": "tried", "held_by": claim_holder(dir) }),
    }
}

/// Why this computer can't write minutes automatically (Claude Code or the plugin missing), or None.
pub fn auto_missing() -> Option<&'static str> {
    if crate::app::claude_bin().is_none() {
        Some("Claude Code isn't installed")
    } else if plugin_dir().is_none() {
        Some("the Echo plugin wasn't found")
    } else {
        None
    }
}

/// Every 15 s in the app: start the minutes of each call that is due. Nothing when the settings
/// can't be read (never on defaults) or the switch is off. The switch's boundary is set first,
/// once (on from the first start: no backlog), and saved before anything is scheduled — tried
/// again at the next tick if it can't be.
pub fn auto_tick() {
    let Ok(mut cfg) = Config::load() else { return };
    if !cfg.auto_minutes {
        return;
    }
    if cfg.auto_minutes_since.is_none() {
        cfg.set_auto_minutes(true);
        let saved = cfg.save();
        let mut err = BOUNDARY_ERROR.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = saved {
            let why = format!("automatic minutes can't start: Echo's settings can't be saved ({e:#})");
            log_once("boundary", &why);
            *err = Some(why);
            return;
        }
        *err = None;
    }
    let now = Local::now();
    for dir in calls::all_calls() {
        if !quitting() && gave_up(&calls::stamp_of(&dir)).is_none() && auto_due(&dir, &cfg, now, None) == Due::Now {
            start_auto(dir);
        }
    }
}

/// Why the switch's boundary couldn't be saved (`auto_tick`; the window shows it).
pub(super) static BOUNDARY_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The app's scheduler for automatic minutes: `auto_tick` every 15 s until the app quits.
pub fn start_scheduler() {
    std::thread::spawn(|| {
        while !quitting() {
            auto_tick();
            for _ in 0..15 {
                if quitting() {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    });
}

/// Admit a due call on this thread (so the next tick sees `queued`), then run it on its own.
/// Missing prerequisites (Claude Code, the plugin) are recorded at once, so the window says why;
/// anything else that stops the admission is logged, and recorded the same way once it has
/// failed `ADMIT_TRIES` ticks in a row (a lock held a moment is tried again).
pub(super) fn start_auto(dir: PathBuf) {
    if let Err(e) = check(&dir) {
        if auto_missing().is_some() {
            refuse(&dir, &format!("{e:#}"));
        }
        return;
    }
    let stamp = calls::stamp_of(&dir);
    match admit(&dir, Intent::Auto) {
        Ok(admitted) => {
            ADMIT_FAILED.lock().unwrap_or_else(|e| e.into_inner()).retain(|(s, _)| *s != stamp);
            if let Some((place, r)) = admitted {
                std::thread::spawn(move || {
                    let _ = finish(&dir, place, r);
                    if let Some(now) = calls::by_stamp(&stamp) {
                        notify_outcome(&now);
                    }
                });
            }
        }
        Err(e) => {
            let why = format!("{e:#}");
            let tries = {
                let mut failed = ADMIT_FAILED.lock().unwrap_or_else(|e| e.into_inner());
                let n = match failed.iter_mut().find(|(s, _)| *s == stamp) {
                    Some((_, n)) => {
                        *n += 1;
                        *n
                    }
                    None => {
                        failed.push((stamp.clone(), 1));
                        1
                    }
                };
                if n >= ADMIT_TRIES {
                    failed.retain(|(s, _)| *s != stamp);
                }
                n
            };
            log_once(&stamp, &format!("{}: the automatic minutes couldn't start: {why}", dir.display()));
            if tries >= ADMIT_TRIES {
                // Recorded for the window; if even that can't be written, the call is left alone
                // until Echo starts again (the window says why: `auto_state`).
                if !refuse(&dir, &why) {
                    GAVE_UP.lock().unwrap_or_else(|e| e.into_inner()).push((stamp.clone(), format!("the automatic minutes couldn't start: {why}")));
                }
                crate::app::notify("Couldn't write minutes", &format!("{}: {why}", call_title(&dir)));
            }
        }
    }
}

/// Calls whose automatic admission failed, and how many ticks in a row (`start_auto`).
pub(super) static ADMIT_FAILED: std::sync::Mutex<Vec<(String, u32)>> = std::sync::Mutex::new(Vec::new());
/// Calls given up on whose failure couldn't be recorded either, and why (until Echo starts again).
pub(super) static GAVE_UP: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

pub(super) fn gave_up(stamp: &str) -> Option<String> {
    GAVE_UP.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(s, _)| s == stamp).map(|(_, why)| why.clone())
}
pub(super) const ADMIT_TRIES: u32 = 4;

/// A line in the app's log, unless the last one said the same for `key` (the scheduler ticks
/// every 15 s).
pub(super) fn log_once(key: &str, line: &str) {
    static SAID: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    match said.iter_mut().find(|(k, _)| k == key) {
        Some((_, l)) if l == line => return,
        Some((_, l)) => *l = line.to_string(),
        None => said.push((key.to_string(), line.to_string())),
    }
    eprintln!("minutes: {line}");
}

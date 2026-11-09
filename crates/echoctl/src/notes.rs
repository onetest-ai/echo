//! Minutes without an open Claude Code session: the app runs Claude Code headless on a call
//! (`claude -p "/echo:process-call <dir>"`), the same skill a session runs. The app still
//! never writes minutes itself; it starts Claude Code when the user asks (`echoctl notes`, the
//! window's Write minutes button, the pane's buttons) and, on its own, 15 minutes after a call
//! or after the user's last note (`auto_due`, `auto_tick`; `config.auto_minutes`).
//!
//! - The plugin comes with the app (`Echo.app/Contents/Resources/plugin`, the repo's `plugin/`
//!   in a dev build) and is loaded with `--plugin-dir`, so the run doesn't depend on where, or
//!   whether, the user installed the plugin, and its skill matches the app's CLI.
//! - Claude Code starts through `ai::claude_command` with the call's AI profile: `system` = the
//!   user's login shell and settings (gateways, `ANTHROPIC_*`, Bedrock / Vertex), model
//!   `config.notes_model`; `local` / `claude` = their own config folder and model, the shell's
//!   routing variables replaced.
//! - Tools are limited to what the skill needs: this call's folder, the previous minutes of the
//!   same meeting, and the Echo commands the minutes need, on this call only. Transcripts and chat
//!   are other people's words, so the run gets no web, no other shell commands and no files
//!   elsewhere: a guard checks every tool call (`guard.rs`, passed in the run's own settings
//!   file), and the permission rules are a second layer.
//! - One run per call (`notes-run.lock`); `<call>/notes-run.json` holds the state (queued /
//!   running / done / skipped / failed / needs_login); `<call>/claude.log` a readable log of what
//!   Claude did.
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{calls, config::Config, paths};

pub const RUN: &str = "notes-run.json";
pub const LOG: &str = "claude.log";
/// A run longer than this is stopped (a long call takes a few minutes).
const TIMEOUT: Duration = Duration::from_secs(20 * 60);
/// A session's claim older than this, with no notes, is treated as abandoned.
const CLAIM_STALE: Duration = Duration::from_secs(30 * 60);
/// Set for the Claude Code run (and inherited by the Echo CLI and the mod inside it).
pub const HEADLESS: &str = "ECHOCTL_HEADLESS";

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Run {
    /// queued | running | done | skipped | failed | needs_login
    pub state: String,
    /// The AI profile the run uses (`ai.rs`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
    /// Why it failed or was skipped, in words for the user.
    pub message: Option<String>,
    /// The process that owns the run from its place to its final state (the app, or a `notes
    /// --wait` CLI): it gone while `queued` / `running` = interrupted.
    pub pid: Option<u32>,
    /// Claude Code, while it runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_pid: Option<u32>,
    pub session_id: Option<String>,
    /// Started by the app on its own (`auto_tick`), not asked for.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto: bool,
    /// This run's id: given to its Claude Code (`calls::RUN_ID`) and recorded by `echoctl actions
    /// --apply`, so the run knows it wrote the action items itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    /// The minutes as the last `done` run left them; carried over by every later run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minutes: Option<Written>,
    /// The call had minutes when this run was admitted (an update): the window keeps showing them.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub had_minutes: bool,
    /// While `queued`, what it waits for: `own` (its own call still recorded or transcribed), `call`
    /// (another call's) or `model` (the local model's other job).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<String>,
    /// Waiting for another call, it may start anyway on the user's Write now (Claude Code as
    /// configured, not pointed at a model on this computer).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub may_start: bool,
}

/// Who wrote the minutes shown (`model`: the profile and model, as the page names them) — valid
/// only while `notes.md` still has `sha256`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Written {
    pub model: String,
    pub at: DateTime<Local>,
    pub sha256: String,
    /// The minutes template the run followed (`calls::template_of` when it started).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
}

/// Asked for (Write minutes, `echoctl notes`), or started by the app on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    Explicit,
    Auto,
}

/// How long after the call (or after the user's last note) the app writes the minutes.
pub const AUTO_DELAY: Duration = Duration::from_secs(15 * 60);

/// The env variable a debug build reads to shorten `AUTO_DELAY` for functional tests (seconds).
#[cfg(debug_assertions)]
const TEST_AUTO_DELAY: &str = "ECHOCTL_TEST_AUTO_MINUTES_DELAY_S";

/// `AUTO_DELAY`, or — functional tests only, in a debug build — the seconds `ECHOCTL_TEST_AUTO_MINUTES_DELAY_S`
/// gives (`tests/fixtures/make_data.py --clock fast`), so a case waits seconds instead of 15 minutes. A release
/// build never reads it.
pub fn auto_delay() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(d) = test_delay(std::env::var(TEST_AUTO_DELAY).ok().as_deref()) {
        return d;
    }
    AUTO_DELAY
}

/// The test delay a value of `ECHOCTL_TEST_AUTO_MINUTES_DELAY_S` gives: whole seconds, else none.
#[cfg(debug_assertions)]
fn test_delay(v: Option<&str>) -> Option<Duration> {
    v.and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs)
}

/// Inside a run Echo started (the mod and the CLI stay out of the way there).
pub fn headless() -> bool {
    std::env::var_os(HEADLESS).is_some()
}

/// What a run Echo started may not ask of the CLI it is allowed: the user's own always-on
/// vocabulary (a setting), or keeping a call's audio (`--keep-audio`, and `--model`, which keeps it
/// to compare).
pub fn refused_in_run(always: bool, keep_audio: bool, model: bool) -> Option<&'static str> {
    if always {
        Some("a minutes run can't change the always-on vocabulary")
    } else if keep_audio || model {
        Some("a minutes run can't keep a call's audio (--keep-audio, --model)")
    } else {
        None
    }
}

/// `notes-run.json` as it is on disk.
fn raw(dir: &Path) -> Option<Run> {
    serde_json::from_slice(&fs::read(dir.join(RUN)).ok()?).ok()
}

/// An automatic run whose owner died before it wrote anything — still waiting, or its Claude Code
/// stopped with it (Echo quit, crashed, the computer shut down): the call counts as never tried,
/// so the app takes it up again.
fn abandoned_auto(dir: &Path, r: &Run) -> bool {
    r.auto
        && pending(r)
        && !r.pid.is_some_and(crate::app::process_alive)
        && !r.child_pid.is_some_and(crate::app::process_alive)
        && (r.state == "queued" || !dir.join("notes.md").exists())
}

/// The run's state as the user should see it; None when minutes were never tried (also after an
/// automatic run that was withdrawn, or abandoned before it wrote them: `abandoned_auto`).
pub fn status(dir: &Path) -> Option<Run> {
    let mut r = raw(dir)?;
    if abandoned_auto(dir, &r) {
        // Never tried, so nobody holds it: Echo's claim goes too (a session may take the call).
        if let Ok(Some(_place)) = crate::app::FileLock::try_take(&dir.join(PLACE)) {
            release_claim(dir);
        }
        return None;
    }
    if !pending(&r) || r.pid.is_some_and(crate::app::process_alive) {
        return Some(r);
    }
    // Its owner is gone (Echo quit or crashed, a CLI was stopped).
    if r.child_pid.is_some_and(crate::app::process_alive) {
        // Claude Code from that run is still going: it stays the call's only writer.
        r.message = Some("Claude Code from a run Echo started before it stopped is still writing; wait for it to end.".into());
        return Some(r);
    }
    r.message = Some(if r.state == "queued" {
        "Echo stopped before the minutes were written — click Write minutes again.".into()
    } else {
        "The run stopped before it finished (Echo quit or Claude Code crashed).".into()
    });
    r.state = "failed".into();
    // Its claim is freed once nobody holds the call's place, and under that place (a new run
    // takes the place before it touches the claim).
    if let Ok(Some(_place)) = crate::app::FileLock::try_take(&dir.join(PLACE)) {
        release_claim(dir);
    }
    Some(r)
}

/// Waiting for its turn or running.
pub(crate) fn pending(r: &Run) -> bool {
    r.state == "queued" || r.state == "running"
}

/// A minutes run is on the call now: waiting or running, its owner or its Claude Code alive.
pub(crate) fn run_active(dir: &Path) -> bool {
    raw(dir).is_some_and(|r| pending(&r) && (r.pid.is_some_and(crate::app::process_alive) || r.child_pid.is_some_and(crate::app::process_alive)))
}

/// The call's place: an OS lock on `<call>/notes-run.lock`, held by the run's owner from its start
/// to its final state (released by the OS if that process dies; it moves with a renamed folder).
const PLACE: &str = "notes-run.lock";

/// Admit a run to the call (never waits): its place first, then — under it and the call's
/// `.meta.lock` (which the user's note saves take too) — an automatic run checks the call is still
/// due, the action items get their ids, the claim, then `queued`. Refused while another run holds
/// the place, while Claude Code from an earlier run whose owner died is still writing, or while a
/// session has the call; a refusal changes nothing (but the ids given to the action items, and an
/// unreadable list set aside). None: an automatic run that isn't due any more.
fn admit(dir: &Path, intent: Intent) -> Result<Option<(crate::app::FileLock, Run)>> {
    let place = crate::app::FileLock::try_take(&dir.join(PLACE)).with_context(|| format!("opening {}", dir.join(PLACE).display()))?;
    let Some(place) = place else { bail!("Echo is already writing the minutes for this call") };
    if let Some(r) = status(dir).filter(pending) {
        if r.child_pid.is_some_and(crate::app::process_alive) {
            bail!("Claude Code from an earlier run is still writing the minutes for this call");
        }
    }
    let _meta = calls::lock_meta(dir)?;
    if intent == Intent::Auto && auto_due(dir, &Config::load()?, Local::now(), None) != Due::Now {
        return Ok(None);
    }
    calls::actions_locked(dir)?;
    claim(dir, intent)?;
    let r = Run {
        state: "queued".into(),
        pid: Some(std::process::id()),
        started_at: Some(Local::now()),
        message: Some("Starting…".into()),
        auto: intent == Intent::Auto,
        run_id: Some(run_id()),
        had_minutes: dir.join("notes.md").exists(),
        ..Run::carrying(dir)
    };
    save(dir, &r);
    Ok(Some((place, r)))
}

impl Run {
    /// A new record for the call: who wrote the minutes there (`minutes`) is carried over.
    fn carrying(dir: &Path) -> Run {
        Run { minutes: raw(dir).and_then(|r| r.minutes), ..Default::default() }
    }
}

/// A new run id (the time and this process: unique on this computer).
fn run_id() -> String {
    format!("{}-{}", Local::now().format("%Y%m%dT%H%M%S%.6f"), std::process::id())
}

/// Withdraw an automatic run that wrote nothing (no longer due, or the app quit before or while its
/// Claude Code ran): its record and claim go, so the call counts as never tried and the app takes
/// it up again.
fn withdraw(dir: &Path) {
    let _ = fs::remove_file(dir.join(RUN));
    release_claim(dir);
}

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
fn later(dir: &Path) -> Option<Option<DateTime<Local>>> {
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
static BOUNDARY_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

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
fn start_auto(dir: PathBuf) {
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
static ADMIT_FAILED: std::sync::Mutex<Vec<(String, u32)>> = std::sync::Mutex::new(Vec::new());
/// Calls given up on whose failure couldn't be recorded either, and why (until Echo starts again).
static GAVE_UP: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

fn gave_up(stamp: &str) -> Option<String> {
    GAVE_UP.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(s, _)| s == stamp).map(|(_, why)| why.clone())
}
const ADMIT_TRIES: u32 = 4;

/// A line in the app's log, unless the last one said the same for `key` (the scheduler ticks
/// every 15 s).
fn log_once(key: &str, line: &str) {
    static SAID: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    match said.iter_mut().find(|(k, _)| k == key) {
        Some((_, l)) if l == line => return,
        Some((_, l)) => *l = line.to_string(),
        None => said.push((key.to_string(), line.to_string())),
    }
    eprintln!("minutes: {line}");
}

/// Claude Code children of minutes runs in this process, and whether the app is quitting. A child
/// is started and registered under this lock, so once `shutdown` set `stopping` none is started,
/// and every one started before is stopped.
struct Children {
    stopping: bool,
    pids: Vec<u32>,
}
static CHILDREN: std::sync::Mutex<Children> = std::sync::Mutex::new(Children { stopping: false, pids: Vec::new() });

/// Start a minutes run's Claude Code, unless the app is quitting.
pub(crate) fn admit_child(start: impl FnOnce() -> Result<std::process::Child>) -> Result<std::process::Child> {
    let mut c = CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
    if c.stopping {
        bail!("Echo is quitting");
    }
    let child = start()?;
    c.pids.push(child.id());
    Ok(child)
}

/// A child admitted by `admit_child` has ended.
pub(crate) fn forget_child(pid: u32) {
    CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).pids.retain(|p| *p != pid);
}

pub(crate) fn quitting() -> bool {
    CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).stopping
}

/// The app is quitting: no new minutes run starts Claude Code, and those running are stopped (TERM
/// to each group, KILL after 3 s), so none goes on writing on its own.
pub fn shutdown() {
    let pids: Vec<u32> = {
        let mut c = CHILDREN.lock().unwrap_or_else(|e| e.into_inner());
        c.stopping = true;
        c.pids.clone()
    };
    let handles: Vec<_> = pids.into_iter().map(|p| std::thread::spawn(move || kill(p))).collect();
    for h in handles {
        let _ = h.join();
    }
}

/// Our own claim (an Echo run's), released when the run doesn't finish.
fn release_claim(dir: &Path) {
    let marker = dir.join(calls::CLAIMED);
    if fs::read_to_string(&marker).is_ok_and(|t| t.contains("echo notes")) {
        let _ = fs::remove_file(marker);
    }
}

fn save(dir: &Path, r: &Run) {
    let tmp = dir.join(format!("{RUN}.tmp"));
    if fs::write(&tmp, serde_json::to_vec_pretty(r).unwrap_or_default()).is_ok() {
        let _ = fs::rename(tmp, dir.join(RUN));
    }
    // The Inbox lists failed and waiting runs: it reads the new state at its next request (#121).
    crate::attention::stale();
}

/// The plugin this app ships: inside the bundle, next to the binary (Windows zip), or the
/// repo's `plugin/` for a dev build; else wherever Claude Code installed it.
pub fn plugin_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok().and_then(|e| dunce::canonicalize(e).ok())?;
    let near = [
        exe.parent().and_then(Path::parent).map(|c| c.join("Resources/plugin")), // Echo.app/Contents/MacOS/echoctl
        exe.parent().map(|d| d.join("plugin")),                                  // Windows: next to echoctl.exe
        exe.ancestors().nth(3).map(|r| r.join("plugin")),                        // repo/target/release/echoctl
    ];
    near.into_iter()
        .flatten()
        .find(|p| p.join(".claude-plugin/plugin.json").is_file())
        .or_else(crate::app::plugin_root)
}

/// What the window shows next to the Write minutes button.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn readiness() -> Value {
    let cfg = Config::load().unwrap_or_default();
    serde_json::json!({
        "claude": crate::app::claude_bin().is_some(),
        "plugin": plugin_dir().is_some(),
        "model": cfg.notes_model_label(),
    })
}

/// `echoctl notes <call>`: in the running app (it outlives this command), else right here.
pub fn request(call: &str, wait: bool) -> Result<()> {
    let dir = calls::resolve_call(call)?;
    check(&dir)?;
    if !wait && crate::watch::running_watcher().is_some() {
        let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        fs::write(paths::state_dir().join(format!("notes-{name}.request")), &name)?;
        println!("Echo is writing the minutes with Claude Code. Log: {}", dir.join(LOG).display());
        return Ok(());
    }
    // Run here, in this terminal: Ctrl-C stops Claude Code too (it runs in a session of its own),
    // and the run ends as it does when Echo quits — its locks released once Claude Code is gone.
    let _ = ctrlc::set_handler(shutdown);
    let dir = run(&dir)?;
    let r = status(&dir).unwrap_or_default();
    match r.state.as_str() {
        "done" => println!("Minutes written: {}", dir.join("notes.md").display()),
        "skipped" => println!("No minutes: {}", r.message.unwrap_or_default()),
        _ => bail!("{} (log: {})", r.message.unwrap_or_else(|| "failed".into()), dir.join(LOG).display()),
    }
    Ok(())
}

/// `echoctl notes <call> --auto --for <session>` (the terminal pane's automatic notes, older
/// panes included): always refused now — the app writes the minutes itself (`auto_tick`), or,
/// with `config.auto_minutes` off, nobody does automatically. One JSON line: `{"requested":
/// false, "waiting": true|false, "reason": …}`.
pub fn request_auto(call: &str) -> Result<()> {
    let dir = calls::resolve_call(call)?;
    println!("{}", auto_answer(&dir, &Config::load()?));
    Ok(())
}

/// `request_auto`'s answer. `waiting`: no minutes yet and they are on their way — the call is due
/// (now or later) for the app's automatic minutes, or a run or a session is writing them — so a
/// pane keeps following the call. A prediction from the files: the app's own give-ups (`GAVE_UP`,
/// in its memory) aren't seen here.
fn auto_answer(dir: &Path, cfg: &Config) -> Value {
    let running = crate::watch::running_watcher().is_some();
    auto_answer_from(dir, auto_missing(), running, || auto_due(dir, cfg, Local::now(), None))
}

/// The scheduler may admit or withdraw a run while this looks, so the files are read after
/// `auto_due`: minutes written → its answer stands; a run queued or running, or a session's fresh
/// claim → on their way (the switch off or not); a run that ended → final. Tried with none of these
/// (a run withdrawn or abandoned, or a session's claim gone, meanwhile) asks `auto_due` again, up
/// to three times; the last answer is taken as it is. `missing`: `auto_missing()`; `running`: the
/// app is.
fn auto_answer_from(dir: &Path, missing: Option<&str>, running: bool, mut due_now: impl FnMut() -> Due) -> Value {
    let mut due = due_now();
    for _ in 0..3 {
        if dir.join("notes.md").exists() {
            break;
        }
        let run = status(dir);
        if run.as_ref().is_some_and(pending) {
            return json!({ "requested": false, "waiting": true, "reason": "Echo is writing the minutes now" });
        }
        if session_claim(dir) {
            return json!({ "requested": false, "waiting": true, "reason": "a Claude Code session is writing the minutes now" });
        }
        if due != Due::Not(NotDue::Tried) || run.is_some() {
            break; // not tried, or a run that ended
        }
        due = due_now();
    }
    // The app writes them later by itself: the pane keeps following the call until it has.
    let later = matches!(due, Due::Now | Due::At(_) | Due::Not(NotDue::NotReady));
    let waiting = later && missing.is_none();
    // `NotReady` also when the user's notes can't be read: the minutes wait for them.
    let notes = dir.join(calls::USER_NOTES);
    let unreadable = (due == Due::Not(NotDue::NotReady) && notes.exists()).then(|| fs::read_to_string(&notes).err()).flatten();
    let reason = match due {
        _ if later && missing.is_some() => format!("no automatic minutes here: {}", missing.unwrap_or_default()),
        _ if unreadable.is_some() => format!("your notes (my-notes.md) can't be read: {} — the minutes wait for them", unreadable.map(|e| e.to_string()).unwrap_or_default()),
        _ if later && !running => "Echo isn't running: it writes the minutes once it starts again".to_string(),
        Due::Not(NotDue::Off) => "automatic minutes are off (menu → Claude Code → Write minutes automatically)".to_string(),
        Due::At(t) => format!("Echo writes the minutes itself at {}", t.format("%H:%M")),
        Due::Now => "Echo is starting the minutes now".to_string(),
        Due::Not(NotDue::Gate(why)) => format!("no automatic minutes: this call is {why} — use Write minutes"),
        Due::Not(NotDue::Before) => "this call ended before automatic minutes were turned on — use Write minutes".to_string(),
        Due::Not(NotDue::NotReady) => "Echo writes the minutes itself, 15 minutes after the call".to_string(),
        Due::Not(NotDue::Tried) => "the minutes were already written or tried for this call".to_string(),
        Due::Not(NotDue::Held) => "you asked for these minutes later: Write minutes when you want them".to_string(),
    };
    json!({ "requested": false, "waiting": waiting, "reason": reason })
}

/// Start runs the CLI or the page asked for (`notes-<call>.request`); called by the app.
pub fn handle_requests() {
    let Ok(rd) = fs::read_dir(paths::state_dir()) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(call) = name.strip_prefix("notes-").and_then(|n| n.strip_suffix(".request")) {
            let _ = fs::remove_file(e.path());
            // By name, else by stamp (retitled since it was asked for).
            match calls::resolve_call(call).ok().or_else(|| calls::by_stamp(&calls::stamp_of(Path::new(call)))) {
                Some(dir) => start_in_background(dir),
                None => eprintln!("notes: dropped the request for {call} — the call can't be found"),
            }
        }
    }
}

/// Say in `notes-run.json` why a run didn't start (the window shows it) and free Echo's own
/// claim (not a session's) — under the call's place, and only when no run holds it. Who wrote the
/// minutes there is kept.
/// Whether it is recorded (or another run of the call is on).
pub fn refuse(dir: &Path, why: &str) -> bool {
    let Ok(Some(_place)) = crate::app::FileLock::try_take(&dir.join(PLACE)) else { return false };
    if status(dir).is_some_and(|r| pending(&r)) {
        return true;
    }
    release_claim(dir);
    save(dir, &Run { state: "failed".into(), message: Some(why.to_string()), finished_at: Some(Local::now()), ..Run::carrying(dir) });
    raw(dir).is_some_and(|r| r.state == "failed" && r.message.as_deref() == Some(why))
}

/// Run on a thread of the app; the outcome lands in `notes-run.json` and a notification. A run
/// refused before it starts (no Claude Code, no plugin, …) releases Echo's own claim and says
/// why in `notes-run.json` too, unless another run of this call is on (its state stays).
pub fn start_in_background(dir: PathBuf) {
    std::thread::spawn(move || {
        if let Err(e) = check(&dir) {
            refuse(&dir, &format!("{e:#}"));
            crate::app::notify("Couldn't write minutes", &format!("{e:#}"));
            return;
        }
        match run(&dir) {
            Ok(dir) => notify_outcome(&dir),
            // Refused before Claude Code started (a session has the call, …): only the reason.
            Err(e) => crate::app::notify("Couldn't write minutes", &format!("{e:#}")),
        }
    });
}

fn notify_outcome(dir: &Path) {
    let Some(r) = status(dir) else { return };
    let title = call_title(dir);
    match r.state.as_str() {
        // Said once, here, once the run is `done` (the watcher's own check skips calls with a run).
        "done" => {
            crate::app::notify("Minutes ready", &title);
            #[cfg(feature = "js")]
            crate::share::minutes_written(dir);
        }
        "needs_login" => crate::app::notify("Claude Code needs you to sign in", &r.message.unwrap_or_default()),
        "failed" => crate::app::notify("Couldn't write minutes", &format!("{title}: {}", r.message.unwrap_or_default())),
        _ => {}
    }
}

fn call_title(dir: &Path) -> String {
    let meta: Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    meta["title"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| "your call".into())
}

/// The call is still being recorded or transcribed.
fn in_progress(dir: &Path) -> bool {
    dir.join(crate::transcribe_marker()).exists() || crate::watch::running_watcher().and_then(|w| w.recording).is_some_and(|r| Path::new(&r) == dir)
}

/// A live claim may sit this long without a sign of work on the call before it stops holding it (a session reads
/// the transcript and the context before it writes anything).
const CLAIM_QUIET: Duration = Duration::from_secs(15 * 60);

/// A Claude Code session's claim that holds the call (not one of Echo's runs: "echo notes"; #43): it names its
/// session and its process, that process is alive, and it is working — the claim is under 5 minutes old, or the
/// call's minutes, action items or log changed since. A claim of an older plugin (a bare time) or of a session
/// that went away holds nothing.
pub(crate) fn session_claim(dir: &Path) -> bool {
    claim_holder(dir).is_some()
}

/// Who holds the call by a live claim: `{pid, session, at}`.
pub(crate) fn claim_holder(dir: &Path) -> Option<Value> {
    let marker = dir.join(calls::CLAIMED);
    let age = fs::metadata(&marker).ok()?.modified().ok()?.elapsed().ok()?;
    if age >= CLAIM_STALE {
        return None;
    }
    let v: Value = serde_json::from_str(fs::read_to_string(&marker).ok()?.trim()).ok()?;
    // Its process must be alive when it is known (none known: by time and work only).
    let pid = v["pid"].as_u64().map(|p| p as u32);
    if pid.is_some_and(|p| !crate::app::process_alive(p)) {
        return None;
    }
    let at = v["at"].as_str().and_then(|a| DateTime::parse_from_rfc3339(a).ok())?;
    let since = |f: PathBuf| fs::metadata(f).and_then(|m| m.modified()).ok().map(DateTime::<Local>::from).is_some_and(|t| t > at);
    let touched = since(dir.join("notes.md")) || since(dir.join(calls::ACTIONS)) || since(dir.join("claude.log"))
        || fs::read_dir(dir).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().starts_with("actions.new") && since(e.path()));
    (age < CLAIM_QUIET || touched).then(|| json!({"pid": pid, "session": v["session"], "at": v["at"]}))
}

/// Everything that must hold before Claude Code is started (said in words the user can act on).
fn check(dir: &Path) -> Result<()> {
    if !dir.join("transcript.md").exists() {
        bail!("this call has no transcript yet");
    }
    if in_progress(dir) {
        bail!("this call is still being recorded or transcribed");
    }
    if status(dir).is_some_and(|r| pending(&r)) {
        bail!("Echo is already writing the minutes for this call");
    }
    if crate::app::claude_bin().is_none() {
        bail!("Claude Code is not installed — get it at https://claude.com/claude-code");
    }
    if plugin_dir().is_none() {
        bail!("the Echo plugin wasn't found — menu → Claude Code → Connect Claude Code");
    }
    Ok(())
}

/// Take the call for this run: for an automatic run, a session working on it right now (its fresh
/// claim, no notes yet) wins; an old claim, one from an earlier run of ours, or one for notes already
/// written is taken over. The user's own Write minutes takes over a session's claim too: an older
/// Echo plugin's terminal pane claimed every transcribed call by itself, and Write minutes was then
/// refused for half an hour with nobody writing them (2026-10-30). A new claim is created exclusively,
/// so a session claiming at the same moment isn't overwritten.
fn claim(dir: &Path, intent: Intent) -> Result<()> {
    let marker = dir.join(calls::CLAIMED);
    let mine = format!("{} echo notes\n", Local::now().to_rfc3339());
    match fs::OpenOptions::new().write(true).create_new(true).open(&marker) {
        Ok(mut f) => return Ok(f.write_all(mine.as_bytes())?),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    if session_claim(dir) && !dir.join("notes.md").exists() {
        if intent != Intent::Explicit {
            bail!("a Claude Code session is writing the minutes for this call right now");
        }
        eprintln!("minutes: {}: a Claude Code session's claim taken over by Write minutes", dir.display());
    }
    fs::write(&marker, mine)?;
    Ok(())
}

/// The run itself (blocking). Refusals before it takes the call's place (another run, a
/// session's claim, no plugin) are returned; from `queued` on, everything is recorded in
/// `notes-run.json` and the log. Returns the call's folder afterwards (the run may have retitled it).
pub fn run(dir: &Path) -> Result<PathBuf> {
    check(dir)?;
    let (place, r) = admit(dir, Intent::Explicit)?.ok_or_else(|| anyhow!("the minutes weren't started"))?;
    finish(dir, place, r)
}

/// An admitted run, to its end. The place is held until the run's final state and claim are
/// settled (the run lets go of it only for the folder's final rename, after which nothing fails).
fn finish(dir: &Path, place: crate::app::FileLock, r: Run) -> Result<PathBuf> {
    let mut place = Some(place);
    let stamp = calls::stamp_of(dir);
    let out = run_in_place(dir, &mut place, r).map_err(|e| {
        // From `queued` on, every failure is recorded: the call never stays "already writing".
        let now = calls::by_stamp(&stamp).unwrap_or_else(|| dir.to_path_buf());
        let mut r = status(&now).unwrap_or_default();
        if pending(&r) && r.pid == Some(std::process::id()) {
            r.state = "failed".into();
            r.message = Some(format!("{e:#}"));
            r.finished_at = Some(Local::now());
            save(&now, &r);
            release_claim(&now);
        }
        e
    });
    drop(place);
    out
}

/// The user's "Write now" on a run waiting for another call (`<call>/notes-now.request`, the window's
/// IPC `notes_now`): Claude Code as configured starts without waiting for that call.
pub const NOW_REQUEST: &str = "notes-now.request";

/// A call named for the user: its title in quotes, and when its recording started.
fn call_words(d: &Path) -> String {
    let meta: Value = fs::read(d.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let title = meta["title"].as_str().filter(|t| !t.trim().is_empty()).map(str::to_string).unwrap_or_else(|| calls::stamp_of(d));
    let at = meta["started_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|t| format!(" (started {})", t.with_timezone(&Local).format("%H:%M"))).unwrap_or_default();
    format!("“{title}”{at}")
}

/// When the call's meeting ends by its invite, if that is still ahead: ", about 11:00".
fn ends_about(d: &Path) -> String {
    let m: Value = fs::read(d.join("meeting.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    m["end"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|t| t.with_timezone(&Local)).filter(|t| *t > Local::now()).map(|t| format!(", about {}", t.format("%H:%M"))).unwrap_or_default()
}

/// What a call being recorded or transcribed makes a gated run of `own` wait for, in words, and
/// whether that call is its own (checked first: never skipped); None: nothing is.
fn busy_call(own: &Path) -> Option<(String, bool)> {
    let same = |d: &Path| calls::stamp_of(d) == calls::stamp_of(own);
    let recording = crate::watch::running_watcher().and_then(|w| w.recording).map(PathBuf::from);
    let transcribing: Vec<PathBuf> = calls::all_calls()
        .into_iter()
        .filter(|d| {
            fs::read_to_string(d.join(crate::transcribe_marker()))
                .ok()
                .and_then(|t| t.split_whitespace().next().and_then(|p| p.parse::<u32>().ok()))
                .is_some_and(crate::app::process_alive)
        })
        .collect();
    if recording.as_deref().is_some_and(same) {
        return Some(("waits for the call to end".into(), true));
    }
    if transcribing.iter().any(|d| same(d)) {
        return Some(("waits for its transcript".into(), true));
    }
    if let Some(d) = recording {
        return Some((format!("waits for the recording of {} to end{}", call_words(&d), ends_about(&d)), false));
    }
    let d = transcribing.first()?;
    Some((format!("waits for the transcript of {} to be written", call_words(d)), false))
}

/// Claude Code as configured pointed at a model on this computer (its folder's settings route it to
/// a loopback `ANTHROPIC_BASE_URL`): it would take the computer from a recording, as a local profile.
fn system_runs_locally() -> bool {
    let Ok(dir) = crate::ai::system_folder() else { return true };
    ["settings.json", "settings.local.json"].iter().any(|f| {
        let v: Value = fs::read(dir.join(".claude").join(f)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        v["env"]["ANTHROPIC_BASE_URL"].as_str().is_some_and(|u| ["localhost", "127.0.0.1", "[::1]", "0.0.0.0"].iter().any(|h| u.contains(h)))
    })
}

/// One look at the local model's turn for `own`: the lock, or why not yet and what it waits for
/// (`own` | `call` | `model`). `skip_calls`: the user said Write now, so another call doesn't hold it back.
fn local_model_turn(own: &Path, skip_calls: bool) -> Result<std::result::Result<crate::app::FileLock, (String, &'static str)>> {
    if let Some((why, is_own)) = busy_call(own) {
        if is_own {
            return Ok(Err((why, "own")));
        }
        if !skip_calls {
            return Ok(Err((why, "call")));
        }
    }
    let path = crate::livenotes::local_lock_path();
    let _ = fs::create_dir_all(paths::config_dir());
    match crate::app::FileLock::try_take(&path).with_context(|| format!("opening {}", path.display()))? {
        Some(lock) => Ok(Ok(lock)),
        None => Ok(Err(("waits for the model to finish another job".into(), "model"))),
    }
}

/// A gated profile (a local model, or Claude Code as configured, which may be one): wait until no
/// call is recorded or transcribed, then for the local model's lock (held by this run until it
/// ends). The reason is in `notes-run.json` and on stderr (`notes --wait`).
/// `may_skip`: Claude Code as configured, which the user may start anyway (Write now); a local model
/// never — it would take the computer from the recording.
fn wait_for_local_model(stamp: &str, dir: &Path, r: &mut Run, name: &str, may_skip: bool) -> Result<crate::app::FileLock> {
    let now_dir = || calls::by_stamp(stamp).unwrap_or_else(|| dir.to_path_buf());
    // A Write now left by an earlier run doesn't count for this one.
    let _ = fs::remove_file(now_dir().join(NOW_REQUEST));
    loop {
        if quitting() {
            bail!("Echo quit before the minutes were written — click Write minutes again");
        }
        let d = now_dir();
        let skip = may_skip && d.join(NOW_REQUEST).exists();
        let (why, wait) = match local_model_turn(&d, skip)? {
            Ok(lock) => {
                let _ = fs::remove_file(d.join(NOW_REQUEST));
                r.wait = None;
                r.may_start = false;
                return Ok(lock);
            }
            Err(w) => w,
        };
        let msg = format!("{name} {why}");
        let may_start = may_skip && wait == "call" && !skip;
        if r.message.as_deref() != Some(msg.as_str()) || r.wait.as_deref() != Some(wait) || r.may_start != may_start {
            eprintln!("{msg}");
            r.message = Some(msg);
            r.wait = Some(wait.into());
            r.may_start = may_start;
            // The folder may be retitled while it waits.
            save(&calls::by_stamp(stamp).unwrap_or_else(|| dir.to_path_buf()), r);
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

/// The call's profile, waiting while it is being signed in (a sign-in takes 10 minutes at most):
/// the minutes then go to the account it ends with, or say why they can't. With the call's folder
/// as it is named now (it may be retitled meanwhile).
fn ready_for_notes(stamp: &str, dir: &Path, r: &mut Run) -> (PathBuf, Result<crate::ai::Ready>) {
    let t = Instant::now();
    loop {
        let before = calls::by_stamp(stamp).unwrap_or_else(|| dir.to_path_buf());
        let ready = crate::ai::for_call(&before);
        // The check may take seconds: where the call is after it.
        let now = calls::by_stamp(stamp).unwrap_or(before);
        match ready {
            Err(e) if e.downcast_ref::<crate::ai::SigningIn>().is_some() && t.elapsed() < Duration::from_secs(12 * 60) && !quitting() => {
                let msg = format!("{e:#} — the minutes wait for it");
                if r.message.as_deref() != Some(msg.as_str()) {
                    r.message = Some(msg);
                    save(&now, r);
                }
                std::thread::sleep(Duration::from_secs(2));
            }
            other => return (now, other),
        }
    }
}

/// The call's profile can't be used: a sign-in problem (signed out, another account, a sign-in
/// that didn't end in time) becomes `needs_login` naming the profile — the window offers Sign in…
/// or Use <account> — anything else `failed`.
fn not_ready(r: &mut Run, e: &anyhow::Error) {
    r.message = Some(format!("{e:#}"));
    r.finished_at = Some(Local::now());
    let profile = e.downcast_ref::<crate::ai::SignInNeeded>().map(|s| s.profile.clone()).or_else(|| e.downcast_ref::<crate::ai::SigningIn>().map(|s| s.profile.clone()));
    match profile {
        Some(id) => {
            r.state = "needs_login".into();
            r.profile = Some(id);
        }
        None => r.state = "failed".into(),
    }
}

fn run_in_place(dir: &Path, place: &mut Option<crate::app::FileLock>, mut r: Run) -> Result<PathBuf> {
    let call_stamp = calls::stamp_of(dir);
    let cfg = Config::load().unwrap_or_default();
    let home = paths::home();
    let plugin = plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    // The place is held by this process until the run ends (a dead holder = an interrupted run).
    // The call's profile: strict, never another one (`ai.rs`). Its lease is held for the whole
    // run: no sign-in changes the account under it.
    let (now, ready) = ready_for_notes(&call_stamp, dir, &mut r);
    let dir: &Path = &now; // retitled while it waited: carry on in the folder as it is named now
    if r.auto && quitting() {
        withdraw(dir); // started again at the next launch
        return Ok(dir.to_path_buf());
    }
    let ready = match ready {
        Ok(p) => p,
        Err(e) => {
            not_ready(&mut r, &e);
            save(dir, &r);
            release_claim(dir);
            return Ok(dir.to_path_buf());
        }
    };
    let profile = ready.profile();
    let model = profile.notes_model();
    let label = format!("{} · {}", profile.name, model.as_deref().unwrap_or("Claude Code's default model"));
    // The template this run follows, named on the minutes it writes.
    let template = crate::calls::template_of(dir)["id"].as_str().filter(|t| *t != "memo").map(str::to_owned);
    r.profile = Some(profile.id.clone());
    r.model = Some(label.clone());
    save(dir, &r);
    let _local_turn = if profile.kind.gated() {
        match wait_for_local_model(&call_stamp, dir, &mut r, &profile.name, profile.kind == crate::ai::Kind::System && !system_runs_locally()) {
            Ok(lock) => Some(lock),
            Err(_) if r.auto && quitting() => {
                withdraw(&calls::by_stamp(&call_stamp).unwrap_or_else(|| dir.to_path_buf()));
                return Ok(dir.to_path_buf());
            }
            Err(e) => return Err(e),
        }
    } else {
        None
    };
    // Retitled while it waited: carry on in the folder as it is named now.
    let moved = calls::by_stamp(&call_stamp).filter(|d| d != dir);
    let dir: &Path = moved.as_deref().unwrap_or(dir);
    // An automatic run may have waited long (a sign-in, the local model, a call): it starts only if
    // the call is still due now — the switch, its boundary, the user's notes — else it is withdrawn
    // and the app takes the call up again at its new time.
    if r.auto {
        let meta = calls::lock_meta(dir);
        if let Err(e) = &meta {
            eprintln!("minutes: {}: {e:#} — taken up again at the next tick", dir.display());
        }
        if meta.is_err() || quitting() || Config::load().map_or(true, |cfg| auto_due(dir, &cfg, Local::now(), Some(&r)) != Due::Now) {
            withdraw(dir);
            return Ok(dir.to_path_buf());
        }
    }
    let fail = |r: &mut Run, msg: String| -> Result<PathBuf> {
        r.state = "failed".into();
        r.message = Some(msg);
        r.finished_at = Some(Local::now());
        save(dir, r);
        release_claim(dir);
        Ok(dir.to_path_buf())
    };
    // `system` runs where it is set up (that folder's `.claude/settings.json` may route it), never
    // elsewhere; the others load no project settings.
    let cwd = if profile.kind == crate::ai::Kind::System {
        match crate::ai::system_folder() {
            Ok(d) => d,
            Err(e) => return fail(&mut r, format!("{e:#}")),
        }
    } else {
        cfg.notes_in.clone().filter(|d| d.is_dir()).unwrap_or_else(|| home.clone())
    };
    // The guard must run (`guard.rs`): not under a policy that switches settings-file hooks off.
    let checked = match crate::ai::hooks_conflict(profile, &cwd) {
        Ok(f) => f,
        Err(why) => return fail(&mut r, why),
    };
    // One spelling of the call for the prompt, the rules, the guard and the CLI (`resolve_call`
    // inside the run gives the resolved path).
    let resolved = dunce::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let dir: &Path = &resolved;
    // The previous minutes of the same meeting: the one file of another call the run may read.
    let previous = calls::previous_subject(dir).and_then(|s| calls::previous_with_notes(&s, Some(dir))).map(|d| d.join("notes.md")).filter(|f| f.is_file());
    let run_id = r.run_id.clone().unwrap_or_else(run_id);
    let settings = match run_settings(&run_id) {
        Ok(s) => s,
        Err(e) => return fail(&mut r, format!("couldn't write the run's settings: {e:#}")),
    };
    let mut log = match fs::File::create(dir.join(LOG)) {
        Ok(f) => f,
        Err(e) => return fail(&mut r, format!("couldn't create claude.log: {e}")),
    };
    r.state = "running".into();
    r.message = None;
    r.started_at = Some(Local::now());
    let started = Instant::now();
    let started_wall = std::time::SystemTime::now();
    let _ = writeln!(log, "{}  Echo starts Claude Code to write the minutes{}", stamp(), if r.auto { " (automatically)" } else { "" });
    let _ = writeln!(log, "          call:   {}", dir.display());
    let _ = writeln!(log, "          model:  {label}");
    let _ = writeln!(log, "          folder: {}", cwd.display());
    let _ = writeln!(log, "          plugin: {}", plugin.display());
    let _ = writeln!(log, "          guard:  every tool call is checked: this call's folder{}, and Echo's minutes commands on this call only", if previous.is_some() { " and the previous minutes" } else { "" });
    let _ = writeln!(log, "          hooks:  allowed by the Claude Code policy in {}", checked.iter().map(|f| f.display().to_string()).collect::<Vec<_>>().join(", "));

    crate::kept_minutes::keep(dir);
    let intent = if r.auto { Intent::Auto } else { Intent::Explicit };
    let mut child = match admit_child(|| {
        let a = args(dir, &plugin, model.as_deref(), intent, &run_id, previous.as_deref(), &settings.0);
        let mut c = crate::ai::prepare_claude(profile, &a, &cwd, false)?;
        c.env(calls::RUN_ID, &run_id).env(crate::guard::RUN_CALL, dir).env(crate::guard::RUN_CLIS, run_clis(&plugin).join("\n"));
        if let Some(p) = &previous {
            c.env(crate::guard::RUN_PREVIOUS, p);
        }
        Ok(c.spawn()?)
    }) {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("couldn't start Claude Code: {e:#}");
            let _ = writeln!(log, "{}  {msg}", stamp());
            return fail(&mut r, msg);
        }
    };
    r.child_pid = Some(child.id()); // `pid` stays this process: the owner of the run
    save(dir, &r);

    // stderr: kept whole in the log (sign-in errors and shell noise land here).
    let stderr = child.stderr.take().map(|e| {
        std::thread::spawn(move || {
            let mut s = String::new();
            for line in BufReader::new(e).lines().map_while(Result::ok) {
                s.push_str(&line);
                s.push('\n');
            }
            s
        })
    });
    // The watchdog: stop a run that hangs.
    let pid = child.id();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let done = done.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            while t.elapsed() < TIMEOUT {
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            kill(pid);
        });
    }

    let mut result: Option<Value> = None;
    let mut seen = String::new(); // non-JSON output, for the sign-in check
    let mut stopped: Option<String> = None; // stopped by Echo before it began: why
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            match serde_json::from_str::<Value>(&line) {
                Ok(ev) => {
                    if ev["type"] == "system" && ev["subtype"] == "init" {
                        r.session_id = ev["session_id"].as_str().map(str::to_owned);
                        save(dir, &r);
                        // The guard must block when it fails (`onFailure`): an older Claude Code is
                        // stopped before its model's first turn.
                        let v = ev["claude_code_version"].as_str().unwrap_or("");
                        if stopped.is_none() && !crate::guard::new_enough(v) {
                            let [a, b, c] = crate::guard::MIN_CLAUDE;
                            stopped = Some(format!(
                                "Claude Code {} is too old for minutes runs — update it (`claude update`) to {a}.{b}.{c} or later, then click Try again",
                                if v.is_empty() { "(of unknown version)" } else { v }
                            ));
                            kill(pid);
                        }
                    }
                    for l in describe(&ev) {
                        let _ = writeln!(log, "{}  {l}", stamp());
                    }
                    if ev["type"] == "result" {
                        result = Some(ev);
                    }
                }
                Err(_) if !line.trim().is_empty() => {
                    let _ = writeln!(log, "{}  {line}", stamp());
                    seen.push_str(&line);
                    seen.push('\n');
                }
                Err(_) => {}
            }
        }
    }
    let status_code = child.wait().ok();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    CHILDREN.lock().unwrap_or_else(|e| e.into_inner()).pids.retain(|p| *p != pid);
    // The call may have been retitled meanwhile (by the user, or an older app's run whose `title`
    // renamed at once): finish in the folder as it is named now.
    let renamed = calls::by_stamp(&call_stamp).filter(|d| d != dir);
    let dir: &Path = renamed.as_deref().unwrap_or(dir);
    let err_text = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    for l in err_text.lines().filter(|l| !l.trim().is_empty()) {
        let _ = writeln!(log, "{}  stderr: {l}", stamp());
    }

    let result_text = result.as_ref().and_then(|v| v["result"].as_str()).unwrap_or("").to_string();
    let is_error = result.as_ref().is_some_and(|v| v["is_error"].as_bool().unwrap_or(false)) || result.is_none();
    let refused: Vec<String> = result.as_ref()
        .and_then(|v| v["permission_denials"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|d| d["tool_name"].as_str().map(str::to_owned))
        .collect();
    r.finished_at = Some(Local::now());
    let written_since = |f: &str| fs::metadata(dir.join(f)).and_then(|m| m.modified()).is_ok_and(|t| t >= started_wall);
    let wrote_notes = written_since("notes.md");
    if r.auto && quitting() && !wrote_notes {
        // Echo stopped it on its way out: taken up again at the next start, as if never tried.
        let _ = writeln!(log, "{}  Echo quit before the minutes were written: it starts them again next time.", stamp());
        withdraw(dir);
        return Ok(dir.to_path_buf());
    }
    let applied = if stopped.is_none() && !is_error && wrote_notes { finish_actions(dir, &run_id, started_wall, &mut log) } else { Ok(false) };
    let wrote_actions = matches!(applied, Ok(true));
    let ended = Ended {
        wrote_notes,
        wrote_actions,
        is_error,
        result_text: &result_text,
        err_text: &err_text,
        seen: &seen,
        timed_out: started.elapsed() >= TIMEOUT,
        refused: &refused,
        exit: status_code.and_then(|s| s.code()),
        actions_error: applied.as_ref().err().map(String::as_str),
        stopped: stopped.as_deref(),
    };
    conclude(&mut r, profile, &ended, &label, &cfg.notes_model_label());
    crate::kept_minutes::settle(dir, r.state == "done", &mut r.message, &run_id, &mut log);
    if r.state == "done" {
        let sha256 = crate::models::sha256_file(&dir.join("notes.md")).unwrap_or_default();
        r.minutes = Some(Written { model: label.clone(), at: Local::now(), sha256, template: template.clone() });
    }
    let _ = writeln!(log, "{}  {} ({}s)", stamp(), match r.state.as_str() {
        "done" => "Minutes written.".to_string(),
        s => format!("{s}: {}", r.message.as_deref().unwrap_or("")),
    }, started.elapsed().as_secs());
    if r.state != "done" {
        // Leave the call to an open session or a retry.
        let _ = fs::remove_file(dir.join(calls::CLAIMED));
    }
    drop(_local_turn); // the local model is free for the next job
    save(dir, &r);
    // What the run's `frames --keep` moved to the trash expires here, outside the run.
    calls::purge_trash();
    // A title set during the run: its folder follows only now (a run's `title` leaves the rename to
    // its end, and Windows couldn't rename the folder while the log or the place was open).
    drop(log);
    drop(place.take());
    let dir = match calls::rename_to_title(dir) {
        Ok(d) => d,
        Err(e) => {
            if let Ok(mut log) = fs::OpenOptions::new().append(true).open(dir.join(LOG)) {
                let _ = writeln!(log, "{}  The folder wasn't renamed to the call's title: {e:#}", stamp());
            }
            dir.to_path_buf()
        }
    };
    // The minutes page (opt-in), under the call's final title.
    if r.state == "done" && cfg.minutes_page {
        let line = match crate::minutes::write(&dir) {
            Ok(page) => format!("Minutes page: {}", page.display()),
            Err(e) => format!("No minutes page: {e:#}"),
        };
        if let Ok(mut log) = fs::OpenOptions::new().append(true).open(dir.join(LOG)) {
            let _ = writeln!(log, "{}  {line}", stamp());
        }
    }
    Ok(dir)
}

/// The run's own settings file (`guard::settings`) in `<config dir>/minutes-run/`, removed when
/// the run ends (it drops); one a run left when Echo died goes after a day, with the next run.
struct RunSettings(PathBuf);

impl Drop for RunSettings {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn run_settings(run_id: &str) -> Result<RunSettings> {
    let folder = paths::config_dir().join("minutes-run");
    fs::create_dir_all(&folder)?;
    for e in fs::read_dir(&folder)?.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().is_ok_and(|age| age > Duration::from_secs(24 * 3600)));
        if old {
            let _ = fs::remove_file(e.path());
        }
    }
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    let f = folder.join(format!("{run_id}.json"));
    fs::write(&f, serde_json::to_vec_pretty(&crate::guard::settings(&exe, &fwd(&paths::calls_dir())))?)?;
    Ok(RunSettings(f))
}

fn stamp() -> String {
    Local::now().format("%H:%M:%S").to_string()
}

fn first_line(s: &str) -> Option<String> {
    s.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| l.chars().take(300).collect())
}

/// Did this run write the call's action items? Yes when `echoctl actions --apply` recorded its
/// id; else, when the run left its list unapplied — a weaker model stops one step short — the app
/// applies it as this run's. Only the run's own file (`actions.new-<run>.json`, named in its
/// prompt) counts: the skill's usual `actions.new.json` could be a session's, written meanwhile.
/// Err(why): the run left its list, but it can't be applied.
fn finish_actions(dir: &Path, run_id: &str, started: std::time::SystemTime, log: &mut impl Write) -> Result<bool, String> {
    if calls::actions_written_by(dir).as_deref() == Some(run_id) {
        return Ok(true);
    }
    let name = calls::actions_new_of(run_id);
    if !fs::metadata(dir.join(&name)).and_then(|m| m.modified()).is_ok_and(|t| t >= started) {
        return Ok(false);
    }
    match calls::apply_actions_as(dir, &dir.join(&name), run_id) {
        Ok(n) => {
            let _ = writeln!(log, "{}  Echo applied the {n} action items the run left in {name}", stamp());
            Ok(true)
        }
        Err(e) => {
            let _ = writeln!(log, "{}  The action items the run left in {name} can't be applied: {e:#}", stamp());
            Err(format!("{e:#}"))
        }
    }
}

/// Who wrote the minutes `notes.md` holds now (`notes-run.json`'s `minutes`, while the file still
/// has the hash that run left): an update that replaced them and then failed, a session's rewrite
/// or a hand edit leaves no model to name.
pub fn minutes_model(dir: &Path) -> Option<String> {
    written(dir).map(|w| w.model)
}

/// The template the minutes shown were written with — while they are as that run left them.
pub fn minutes_template(dir: &Path) -> Option<String> {
    written(dir)?.template
}

fn written(dir: &Path) -> Option<Written> {
    let w = raw(dir)?.minutes?;
    (crate::models::sha256_file(&dir.join("notes.md")).ok()? == w.sha256).then_some(w)
}

/// How Claude Code left a minutes run.
struct Ended<'a> {
    /// `notes.md` written during the run.
    wrote_notes: bool,
    /// The action items applied by this run (`.actions-generation` names its id).
    wrote_actions: bool,
    is_error: bool,
    result_text: &'a str,
    err_text: &'a str,
    /// Its output that isn't stream JSON.
    seen: &'a str,
    timed_out: bool,
    /// Tools it was refused.
    refused: &'a [String],
    /// Its exit code (None: a signal).
    exit: Option<i32>,
    /// Why the action list the run left couldn't be applied (`finish_actions`).
    actions_error: Option<&'a str>,
    /// Echo stopped it before it began, and why (an old Claude Code).
    stopped: Option<&'a str>,
}

/// The run's final state and message. A `claude` profile's sign-in is marked refused when a failed
/// run says it isn't signed in (`ai::auth_failed`); notes written count as a job that worked.
fn conclude(r: &mut Run, profile: &crate::ai::Profile, e: &Ended, label: &str, notes_model_label: &str) {
    use crate::ai::Kind;
    let all = format!("{}\n{}\n{}", e.seen, e.err_text, e.result_text);
    let (state, message) = if let Some(why) = e.stopped {
        ("failed", Some(why.to_string()))
    } else if e.wrote_notes && e.wrote_actions && !e.is_error {
        crate::ai::job_worked(&profile.id);
        ("done", None)
    } else if e.wrote_notes && !e.is_error {
        ("failed", Some(match e.actions_error {
            Some(why) => format!("The minutes were written, but the action items the run left can't be applied ({why}) — see the log, then Try again."),
            None => "The minutes were written but not the action items — see the log, then Try again.".to_string(),
        }))
    } else if e.is_error && needs_login(&all) && profile.kind == Kind::Local {
        // Its token is the profile's: /login in a terminal would only sign in the user's own Claude Code.
        ("failed", Some(format!(
            "{} refused the profile's token — set the right one: `echoctl ai remove {id}`, then `echoctl ai add-local {id} … --token …`",
            profile.name,
            id = profile.id
        )))
    } else if e.is_error && needs_login(&all) {
        // Only a failed run: a skipped one's report may quote what was said ("not logged in").
        ("needs_login", Some(if profile.kind == Kind::Claude {
            crate::ai::auth_failed(profile);
            format!("{} isn't signed in — Sign in…, then click Try again.", profile.name)
        } else {
            "Claude Code isn't signed in. Open Terminal, run claude, type /login, then click Try again.".into()
        }))
    } else if model_rejected(e.is_error, &all) {
        ("failed", Some(if profile.kind == Kind::System {
            format!(
                "Claude Code can't use the model \"{notes_model_label}\". Change it under menu → AI for calls → Minutes model (while {} is the profile new calls use), or `echoctl ai set {} --notes-model <name>` (\"default\" uses Claude Code's own setting).",
                profile.name, profile.id
            )
        } else {
            format!(
                "Claude Code can't use the model of {} ({label}). Change it under menu → AI for calls → Minutes model (while it is the profile new calls use), or `echoctl ai set {} --notes-model <name>`.",
                profile.name, profile.id
            )
        }))
    } else if e.timed_out {
        ("failed", Some(format!("stopped after {} minutes without finishing", TIMEOUT.as_secs() / 60)))
    } else if !e.refused.is_empty() {
        ("failed", Some(format!("Claude Code was refused a tool it needed ({}); see the log", e.refused.join(", "))))
    } else if !e.is_error {
        // Claude finished but wrote no minutes: the call was too short or thin (the skill's gate).
        ("skipped", Some(first_line(e.result_text).unwrap_or_else(|| "Claude Code wrote no minutes for this call".into())))
    } else {
        ("failed", Some(
            first_line(e.result_text)
                .or_else(|| first_line(e.err_text))
                .or_else(|| first_line(e.seen))
                .unwrap_or_else(|| format!("Claude Code exited with {}", e.exit.map_or("a signal".into(), |c| c.to_string()))),
        ))
    };
    r.state = state.into();
    r.message = message;
}

/// Claude Code's own words when it can't use the account.
pub(crate) fn needs_login(text: &str) -> bool {
    let t = text.to_lowercase();
    ["not logged in", "please run /login", "invalid api key", "oauth token has expired", "authentication_error", "credit balance is too low"]
        .iter()
        .any(|p| t.contains(p))
}

/// Claude Code refused the notes model. Only when the run failed: it warns `unrecognized_model`
/// for any model it doesn't know (a local one through Ollama, a gateway's) even when it works, and
/// a call too short for notes would otherwise be reported as a model problem.
fn model_rejected(is_error: bool, text: &str) -> bool {
    is_error && (text.contains("unrecognized_model") || text.contains("issue with the selected model"))
}

/// A path as Claude Code's permission rules compare it. On Windows that is POSIX-style with a
/// lower-case drive: `C:\Users\me` → `/c/Users/me` (measured: `/C:\…` and `/C:/…` never match,
/// so a run in don't-ask mode was refused every Write). Elsewhere the path as it is.
pub(crate) fn rule_path(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    match s.as_bytes() {
        [d, b':', rest @ ..] if d.is_ascii_alphabetic() => format!("/{}{}", (*d as char).to_ascii_lowercase(), String::from_utf8_lossy(rest)),
        _ => s,
    }
}

/// The Echo commands a run may use: what process-call needs, no more. A run follows a transcript
/// (others' words) unattended, so it can't change Echo's settings, read the calendar or chats, or
/// quit Echo — nor work on another call (`calls::resolve_call` inside a run). The guard
/// (`guard.rs`) holds every Bash call to these, as plain commands.
pub const RUN_COMMANDS: [&str; 9] = ["context", "actions", "speakers", "title", "type", "vocab add", "frames", "transcribe", "memo threads"];

/// The only tools a run has: no web, no agents, and with `--strict-mcp-config` no MCP server.
const RUN_TOOLS: &str = "Bash,Read,Glob,Grep,Edit,Write,Skill";

/// Forward slashes: Bash (Git Bash on Windows) drops the backslashes of a `C:\…` path.
fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// The names a run may use for the CLI: the plugin's script, the same by forward slashes (the
/// launcher the Windows prompt names), and `echoctl` on PATH.
pub(crate) fn run_clis(plugin: &Path) -> Vec<String> {
    let mut clis = vec![format!("{}/scripts/echoctl", plugin.display()), format!("{}/scripts/echoctl", fwd(plugin)), "echoctl".to_string()];
    clis.dedup();
    clis
}

/// The arguments for `claude`. `previous`: the previous minutes of the same meeting (`context`
/// names them), the one file of another call the run may read; `settings`: the run's own
/// settings file (`run_settings`: the guard).
fn args(dir: &Path, plugin: &Path, model: Option<&str>, intent: Intent, run_id: &str, previous: Option<&Path>, settings: &Path) -> Vec<String> {
    let calls = paths::calls_dir();
    let abs = |p: &Path| format!("/{}", rule_path(p)); // permission rules spell absolute paths `//…`
    // Windows: Claude Code's Bash doesn't see the PATH this run gets, so the plugin's launcher
    // by its full path (forward slashes, as allowed below).
    let launcher = format!("{}/scripts/echoctl", fwd(plugin));
    let cli = if cfg!(windows) {
        format!("`{launcher} …` (the plugin's launcher, by this exact path)")
    } else {
        "plain `echoctl …` (it is on PATH)".to_string()
    };
    let prompt = format!(
        "/echo:process-call {}\n\n\
         This run was started from the Echo app and nobody is watching it: do every step without \
         asking anything. Echo has already claimed this call for this run, so don't claim it again. \
         Run the Echo CLI as {cli}: one command per Bash call, with no \
         variables, `cd`, pipes, redirects or `&&`; put words with spaces or punctuation in single \
         quotes, and never use `$` or backticks. Write and edit files with the Write and Edit \
         tools, never through the shell. Read and write only in this call's folder; the only other \
         file you may read is the previous minutes `context` names. `echoctl title` keeps the \
         folder's path until the run ends: go on using it. In step 5 write the new action list to \
         `{}` (this run's own file) and apply that one. Skip the follow-up offers at the end; finish \
         with a two-line report.{}",
        fwd(dir),
        fwd(&dir.join(calls::actions_new_of(run_id))),
        if intent == Intent::Explicit {
            "\n\nThe user asked for these minutes explicitly: write them even if `context` says \
             `summarize: false`."
        } else {
            ""
        }
    );
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    if let Some(m) = model {
        a.extend(["--model".into(), m.into()]);
    }
    a.extend(["--plugin-dir".into(), plugin.display().to_string()]);
    // The run's own settings: the guard before every tool call, whatever the user's own settings
    // and permissions allow. It is the boundary; the rules below are a second layer.
    a.extend(["--settings".into(), settings.display().to_string()]);
    a.extend(["--tools".into(), RUN_TOOLS.into(), "--strict-mcp-config".into()]);
    // Only the tools below, whatever the user's own default mode is (auto / bypass would let a
    // transcript talk the run into anything): anything else is refused without asking.
    a.extend(["--permission-mode".into(), "dontAsk".into()]);
    a.push("--allowedTools".into());
    // This call's folder; Write too, though Claude Code (2.1.310, measured) decides Write by the
    // Edit rule. No `--add-dir`: the rules allow it outside the working folder.
    let call = abs(dir);
    a.extend(["Read", "Glob", "Grep", "Edit", "Write"].map(|t| format!("{t}({call}/**)")));
    if let Some(p) = previous {
        a.push(format!("Read({})", abs(p)));
    }
    a.push("Skill(echo:process-call)".into());
    // The CLI by any of the names the run may use for it, `RUN_COMMANDS` only.
    a.extend(run_clis(plugin).iter().flat_map(|cli| RUN_COMMANDS.iter().map(move |c| format!("Bash({cli} {c}:*)"))));
    // The user's own notes are never changed, and the action items are written only through
    // `echoctl actions --apply` (it keeps the ticks made meanwhile): deny rules win over the
    // allowlist, and match every call folder (a retitle doesn't escape them).
    a.push("--disallowedTools".into());
    a.extend([format!("Edit({}/**/{})", abs(&calls), calls::USER_NOTES), format!("Edit({}/**/{})", abs(&calls), calls::ACTIONS)]);
    a
}

/// Stop a run and what it started (its whole group; the run's thread reaps the leader).
fn kill(pid: u32) {
    crate::ai::stop_group(pid);
}

/// One stream-json event as log lines a person can follow.
pub(crate) fn describe(ev: &Value) -> Vec<String> {
    let clip = |s: &str, n: usize| -> String {
        let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if one.chars().count() > n { format!("{}…", one.chars().take(n).collect::<String>()) } else { one }
    };
    match ev["type"].as_str() {
        Some("system") if ev["subtype"] == "init" => vec![format!(
            "Claude Code started · model {} · session {}",
            ev["model"].as_str().unwrap_or("?"),
            ev["session_id"].as_str().unwrap_or("?")
        )],
        Some("assistant") => ev["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| match c["type"].as_str() {
                Some("text") => c["text"].as_str().filter(|t| !t.trim().is_empty()).map(|t| format!("Claude: {}", clip(t, 400))),
                Some("tool_use") => {
                    let i = &c["input"];
                    let what = i["command"].as_str().or(i["file_path"].as_str()).or(i["skill"].as_str()).or(i["pattern"].as_str()).unwrap_or("");
                    Some(format!("→ {} {}", c["name"].as_str().unwrap_or("tool"), clip(what, 200)))
                }
                _ => None,
            })
            .collect(),
        Some("user") => ev["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "tool_result" && c["is_error"].as_bool() == Some(true))
            .map(|c| {
                let text = c["content"].as_str().map(str::to_owned).unwrap_or_else(|| {
                    c["content"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ")
                });
                format!("  ✗ {}", clip(&text, 300))
            })
            .collect(),
        Some("result") => {
            let mut out = vec![format!(
                "Finished: {} · {:.0}s · ${:.2}",
                if ev["is_error"].as_bool() == Some(true) { "error" } else { "ok" },
                ev["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0,
                ev["total_cost_usd"].as_f64().unwrap_or(0.0)
            )];
            if let Some(t) = ev["result"].as_str().filter(|t| !t.trim().is_empty()) {
                out.push(format!("Result: {}", clip(t, 600)));
            }
            for d in ev["permission_denials"].as_array().into_iter().flatten() {
                out.push(format!("Not allowed: {} {}", d["tool_name"].as_str().unwrap_or("?"), clip(&d["tool_input"].to_string(), 200)));
            }
            out
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A live session's claim: this test's own process holds it, now.
    fn live_claim() -> String {
        format!("{}\n", json!({"at": Local::now().to_rfc3339(), "pid": std::process::id(), "session": "s1", "by": "session"}))
    }

    #[test]
    fn only_a_live_session_working_on_the_call_holds_it() {
        let (_env, root) = crate::paths::testenv::folders("notes-ghost-claim");
        let dir = root.join("home/calls/2026-10-23_100000-call");
        fs::create_dir_all(&dir).unwrap();
        // An older plugin's claim (a bare time, #43): it holds nothing.
        fs::write(dir.join(calls::CLAIMED), format!("{}\n", Local::now().to_rfc3339())).unwrap();
        assert!(!session_claim(&dir));
        // A session that went away: nothing.
        fs::write(dir.join(calls::CLAIMED), json!({"at": Local::now().to_rfc3339(), "pid": 999_999_u32, "session": "s1"}).to_string()).unwrap();
        assert!(!session_claim(&dir));
        // A live one: it holds the call, and the window can say who.
        fs::write(dir.join(calls::CLAIMED), live_claim()).unwrap();
        assert!(session_claim(&dir));
        assert_eq!(claim_holder(&dir).unwrap()["pid"], std::process::id());
        // Quiet for longer than a session takes to start writing, with nothing changed: it holds nothing; a newer
        // minutes file since: it holds.
        let old = (Local::now() - chrono::Duration::minutes(20)).to_rfc3339();
        fs::write(dir.join(calls::CLAIMED), json!({"at": old, "pid": std::process::id(), "session": "s1"}).to_string()).unwrap();
        let set_age = |f: &std::path::Path, mins: u64| fs::File::options().write(true).open(f).unwrap().set_modified(std::time::SystemTime::now() - Duration::from_secs(mins * 60)).unwrap();
        set_age(&dir.join(calls::CLAIMED), 20);
        assert!(!session_claim(&dir), "quiet for 20 minutes");
        fs::write(dir.join("notes.md"), "# Minutes").unwrap();
        assert!(session_claim(&dir), "it wrote the minutes since");
        fs::remove_file(dir.join("notes.md")).unwrap();
        // No process known (where it can't be told): by time and work only.
        fs::write(dir.join(calls::CLAIMED), json!({"at": Local::now().to_rfc3339(), "pid": null, "session": "s1"}).to_string()).unwrap();
        assert!(session_claim(&dir));
        // And a claim without a session id is an older plugin's: refused.
        assert!(calls::claim(&dir.display().to_string(), None, None).unwrap_err().to_string().contains("too old"));
    }

    /// The pid of a process that has just exited (any fixed number could be a live process).
    fn dead_pid() -> u32 {
        let mut c = std::process::Command::new(if cfg!(windows) { "cmd" } else { "true" });
        if cfg!(windows) {
            c.args(["/c", "exit"]);
        }
        let mut child = c.spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn a_session_never_starts_minutes_automatically() {
        let (_env, root) = crate::paths::testenv::folders("notes-auto");
        let dir = root.join("home/calls/2026-10-23_120000-x");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), "{}").unwrap();
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        for on in [true, false] {
            let mut cfg = Config { notes_in: Some(root.join("home")), ..Default::default() };
            cfg.set_auto_minutes(on);
            cfg.save().unwrap();
            request_auto(&dir.display().to_string()).unwrap();
            assert!(!dir.join(calls::CLAIMED).exists() && !dir.join(RUN).exists(), "the app writes them, or nobody automatically");
        }
    }

    #[test]
    fn a_run_state_saved_makes_the_inbox_read_it_again() {
        // #121: the Inbox kept a failed run's old message for 20 s after Try again ended with a new one.
        let dir = std::env::temp_dir().join(format!("echo-run-stale-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        crate::attention::cache_for_test(serde_json::json!({"open": []}));
        save(&dir, &Run { state: "failed".into(), message: Some("new reason".into()), ..Default::default() });
        assert!(!crate::attention::cached(), "the Inbox's cached list is dropped");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_run_per_call_and_interrupted_runs_free_it() {
        let dir = std::env::temp_dir().join(format!("echo-place-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let place = admit(&dir, Intent::Explicit).unwrap().unwrap();
        assert_eq!(status(&dir).unwrap().state, "queued", "admission says so at once");
        assert!(admit(&dir, Intent::Explicit).is_err(), "one run at a time (even in this process)");
        // While the place is held, an "interrupted" look at the state frees nothing.
        let gone = dead_pid();
        save(&dir, &Run { state: "queued".into(), pid: Some(gone), ..Default::default() });
        fs::write(dir.join(calls::CLAIMED), "2026-10-23T01:00:00+02:00 echo notes\n").unwrap();
        assert_eq!(status(&dir).unwrap().state, "failed");
        assert!(dir.join(calls::CLAIMED).exists(), "the place is held: a new run owns the claim");
        drop(place); // its owner is gone: the OS releases the place
        // A queued run whose owner died: interrupted, and its claim is released.
        crate::paths::testenv::eventually("the place is free", || {
            let _ = status(&dir);
            !dir.join(calls::CLAIMED).exists()
        });
        assert_eq!(status(&dir).unwrap().state, "failed");
        // A session's claim is not ours to release.
        save(&dir, &Run { state: "running".into(), pid: Some(gone), ..Default::default() });
        fs::write(dir.join(calls::CLAIMED), live_claim()).unwrap();
        assert_eq!(status(&dir).unwrap().state, "failed");
        assert!(dir.join(calls::CLAIMED).exists());
        // Claude Code from a run whose owner died still writes: still the only writer.
        #[cfg(unix)]
        {
            let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
            save(&dir, &Run { state: "running".into(), pid: Some(gone), child_pid: Some(child.id()), ..Default::default() });
            assert_eq!(status(&dir).unwrap().state, "running");
            assert!(admit(&dir, Intent::Explicit).is_err());
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_admission_touches_neither_claim_nor_state() {
        let dir = std::env::temp_dir().join(format!("echo-admit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let other = crate::app::FileLock::try_take(&dir.join(PLACE)).unwrap().unwrap(); // another run holds the place
        fs::write(dir.join(calls::CLAIMED), "2026-10-23T01:00:00+02:00 echo notes\n").unwrap();
        save(&dir, &Run { state: "running".into(), pid: Some(std::process::id()), ..Default::default() });
        assert!(admit(&dir, Intent::Explicit).is_err());
        assert_eq!(fs::read_to_string(dir.join(calls::CLAIMED)).unwrap(), "2026-10-23T01:00:00+02:00 echo notes\n");
        assert_eq!(status(&dir).unwrap().state, "running");
        drop(other);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn quitting_stops_every_child_and_starts_none() {
        let _q = QUITTING.lock().unwrap_or_else(|e| e.into_inner());
        let child = admit_child(|| {
            let mut c = std::process::Command::new("sleep");
            c.arg("30");
            crate::ai::detach(&mut c);
            Ok(c.spawn()?)
        })
        .unwrap();
        let pid = child.id();
        let reaper = std::thread::spawn(move || {
            let mut child = child;
            child.wait().unwrap()
        });
        shutdown();
        assert!(!reaper.join().unwrap().success(), "the running child was stopped");
        let called = std::sync::atomic::AtomicBool::new(false);
        assert!(admit_child(|| {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            bail!("started")
        })
        .is_err());
        assert!(!called.load(std::sync::atomic::Ordering::SeqCst), "none starts after");
        assert!(quitting());
        CHILDREN.lock().unwrap().stopping = false; // other tests in this process
        CHILDREN.lock().unwrap().pids.retain(|p| *p != pid);
    }

    #[test]
    fn a_session_working_on_the_call_keeps_it() {
        let dir = std::env::temp_dir().join(format!("echo-claim-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(calls::CLAIMED), live_claim()).unwrap(); // a session's, fresh
        save(&dir, &Run { state: "failed".into(), ..Default::default() }); // an earlier Echo run left its state
        assert!(claim(&dir, Intent::Auto).is_err(), "automatic minutes don't overwrite a session's fresh claim");
        claim(&dir, Intent::Explicit).unwrap(); // the user's Write minutes does
        assert!(fs::read_to_string(dir.join(calls::CLAIMED)).unwrap().contains("echo notes"));
        fs::write(dir.join(calls::CLAIMED), live_claim()).unwrap();
        fs::write(dir.join("notes.md"), "# done").unwrap();
        claim(&dir, Intent::Auto).unwrap(); // its notes exist: a re-run may take over
        assert!(fs::read_to_string(dir.join(calls::CLAIMED)).unwrap().contains("echo notes"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn permission_rule_paths_are_posix_with_a_lower_case_drive() {
        assert_eq!(rule_path(Path::new(r"C:\Users\User\Echo\calls")), "/c/Users/User/Echo/calls");
        assert_eq!(rule_path(Path::new("D:/data/echo")), "/d/data/echo");
        assert_eq!(rule_path(Path::new("/Users/me/Echo")), "/Users/me/Echo");
    }

    /// Tests that set or depend on `quitting()` (process-wide).
    static QUITTING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(unix)]
    #[test]
    fn notes_wait_for_a_sign_in_going_on() {
        let _q = QUITTING.lock().unwrap_or_else(|e| e.into_inner());
        let (_env, root) = crate::paths::testenv::folders("notes-wait-sign-in");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("w", None, &crate::ai::fake::signed_in("pat@example.com"));
        let d = root.join("home/calls/2026-10-23_170000-x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("meta.json"), r#"{"ai_profile": "w"}"#).unwrap();
        let held = crate::ai::sign_in_lease(&p).unwrap().unwrap();
        let renamed = root.join("home/calls/2026-10-23_170000-retitled");
        let (d2, r2) = (d.clone(), renamed.clone());
        // Once the notes say they wait for the sign-in, the call is retitled; then the sign-in ends.
        let signing_in = std::thread::spawn(move || {
            let t = Instant::now();
            while !status(&d2).is_some_and(|r| r.message.is_some_and(|m| m.contains("the minutes wait for it"))) {
                assert!(t.elapsed() < Duration::from_secs(20), "the notes never said they wait");
                std::thread::sleep(Duration::from_millis(20));
            }
            fs::rename(&d2, &r2).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });
        let mut r = Run::default();
        let (now, ready) = ready_for_notes(&calls::stamp_of(&d), &d, &mut r);
        signing_in.join().unwrap();
        assert_eq!(ready.unwrap().profile().id, "w");
        assert_eq!(now, renamed, "carries on in the folder as it is named now");
        assert!(r.message.unwrap().contains("the minutes wait for it"));
        assert!(status(&renamed).is_some(), "its state is where the call is");
        // Retitled during the check itself (it takes a second here), which then fails: the
        // failure goes where the call is.
        fs::write(crate::ai::fake::file(&p, "status.json"), crate::ai::fake::SIGNED_OUT).unwrap();
        fs::write(crate::ai::fake::file(&p, "slow"), "").unwrap();
        crate::ai::forget_check("w"); // (the first check is cached for 30 s)
        let again = root.join("home/calls/2026-10-23_170000-again");
        let (r1, r2) = (renamed.clone(), again.clone());
        let retitle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            fs::rename(&r1, &r2).unwrap();
        });
        let mut r = Run::default();
        let (now, ready) = ready_for_notes(&calls::stamp_of(&renamed), &renamed, &mut r);
        assert!(ready.is_err());
        assert_eq!(now, again);
        retitle.join().unwrap();
        // One that never ends in time: the run says so and names the profile.
        let mut r = Run::default();
        not_ready(&mut r, &anyhow::Error::new(crate::ai::SigningIn { profile: "w".into(), name: "Work".into() }));
        assert_eq!((r.state.as_str(), r.profile.as_deref()), ("needs_login", Some("w")));
    }

    #[test]
    fn a_profile_that_needs_a_sign_in_is_named_in_the_run() {
        let mut r = Run::default();
        let e = anyhow::Error::new(crate::ai::SignInNeeded { profile: "w".into(), name: "Work".into(), now: None, expected: None });
        not_ready(&mut r, &e.context("checking"));
        assert_eq!((r.state.as_str(), r.profile.as_deref()), ("needs_login", Some("w")), "found through a context too");
        assert!(r.message.unwrap().contains("isn't signed in"));
        let mut r = Run::default();
        not_ready(&mut r, &anyhow::anyhow!("the AI settings can't be read"));
        assert_eq!((r.state.as_str(), r.profile), ("failed", None));
    }

    #[test]
    fn sign_in_errors_are_recognised() {
        assert!(needs_login("Not logged in · Please run /login"));
        assert!(needs_login("API Error: 401 {\"type\":\"error\",\"error\":{\"type\":\"authentication_error\"}}"));
        assert!(!needs_login("Notes written to notes.md"));
    }

    #[test]
    fn minutes_on_a_local_model_wait_their_turn() {
        let _q = QUITTING.lock().unwrap_or_else(|e| e.into_inner());
        let (_env, root) = crate::paths::testenv::folders("notes-local-turn");
        let d = root.join("home/calls/2026-10-23_220000-x");
        fs::create_dir_all(&d).unwrap();
        let busy = crate::app::FileLock::try_take(&crate::livenotes::local_lock_path()).unwrap().unwrap();
        // Another job holds the model: the run says so, and goes on once it is free.
        let d2 = d.clone();
        let other = std::thread::spawn(move || {
            let t = Instant::now();
            while !status(&d2).is_some_and(|r| r.message.is_some_and(|m| m.contains("waits for the model"))) {
                assert!(t.elapsed() < Duration::from_secs(20), "the run never said it waits");
                std::thread::sleep(Duration::from_millis(20));
            }
            drop(busy);
        });
        let mut r = Run::default();
        let turn = wait_for_local_model(&calls::stamp_of(&d), &d, &mut r, "Qwen", false);
        other.join().unwrap();
        assert!(turn.is_ok());
        assert_eq!(r.message.as_deref(), Some("Qwen waits for the model to finish another job"));
    }

    #[test]
    fn minutes_waiting_for_another_call_name_it_and_may_start_anyway() {
        let _q = QUITTING.lock().unwrap_or_else(|e| e.into_inner());
        let (_env, root) = crate::paths::testenv::folders("notes-other-call");
        let own = root.join("home/calls/2026-11-02_180000-planning");
        let other = root.join("home/calls/2026-11-03_100100-weekly-sync");
        for d in [&own, &other] {
            fs::create_dir_all(d).unwrap();
        }
        fs::write(other.join("meta.json"), r#"{"title": "Weekly sync", "started_at": "2026-11-03T10:01:00+00:00"}"#).unwrap();
        // The other call is being transcribed (this process holds its marker).
        fs::write(other.join(crate::transcribe_marker()), std::process::id().to_string()).unwrap();
        let (why, is_own) = busy_call(&own).unwrap();
        assert!(!is_own);
        let at = DateTime::parse_from_rfc3339("2026-11-03T10:01:00+00:00").unwrap().with_timezone(&Local).format("%H:%M").to_string();
        assert_eq!(why, format!("waits for the transcript of “Weekly sync” (started {at}) to be written"));
        // Its own transcript is never skipped; another call's is, on the user's say-so.
        assert_eq!(busy_call(&other), Some(("waits for its transcript".into(), true)));
        assert!(matches!(local_model_turn(&own, false), Ok(Err((_, "call")))));
        assert!(matches!(local_model_turn(&other, true), Ok(Err((_, "own")))));
        assert!(matches!(local_model_turn(&own, true), Ok(Ok(_))));
        // Write now reaches a waiting run of Claude Code as configured, and is used up when it starts.
        let (o, stamp) = (own.clone(), calls::stamp_of(&own));
        let now = std::thread::spawn(move || {
            let t = Instant::now();
            while status(&o).is_none_or(|r| !(r.wait.as_deref() == Some("call") && r.may_start)) {
                assert!(t.elapsed() < Duration::from_secs(20), "the run never said it waits for a call");
                std::thread::sleep(Duration::from_millis(20));
            }
            fs::write(o.join(NOW_REQUEST), b"").unwrap();
        });
        let mut r = Run::default();
        let turn = wait_for_local_model(&stamp, &own, &mut r, "Claude Code", true);
        now.join().unwrap();
        assert!(turn.is_ok());
        assert!(!own.join(NOW_REQUEST).exists());
        assert_eq!(r.wait, None);
    }

    #[test]
    fn a_run_is_judged_by_how_it_ended() {
        use crate::ai::Kind;
        let (_env, _root) = crate::paths::testenv::folders("notes-conclude");
        let profile = |kind| crate::ai::Profile { id: "judged".into(), name: "J".into(), kind, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
        let ended = |wrote_notes, is_error, result_text| Ended { wrote_notes, wrote_actions: wrote_notes, is_error, result_text, err_text: "", seen: "", timed_out: false, refused: &[], exit: Some(1), actions_error: None, stopped: None };
        let judge = |kind, e: &Ended| {
            let mut r = Run::default();
            conclude(&mut r, &profile(kind), e, "J · m", "sonnet");
            (r.state, r.message.unwrap_or_default())
        };
        assert_eq!(judge(Kind::System, &ended(true, false, "")).0, "done");
        // Fresh minutes without this run's action items (an action ticked meanwhile doesn't count).
        let (state, message) = judge(Kind::System, &Ended { wrote_actions: false, ..ended(true, false, "") });
        assert_eq!(state, "failed");
        assert!(message.contains("action items"), "{message}");
        // A call skipped as too short, whose report quotes what was said, needs no sign-in.
        let (state, message) = judge(Kind::Claude, &ended(false, false, "Too short for notes: they said they're not logged in."));
        assert_eq!((state.as_str(), message.as_str()), ("skipped", "Too short for notes: they said they're not logged in."));
        // A failed run that says so does.
        assert_eq!(judge(Kind::System, &ended(false, true, "Not logged in · Please run /login")).0, "needs_login");
        let (state, message) = judge(Kind::Claude, &ended(false, true, "Not logged in · Please run /login"));
        assert_eq!(state, "needs_login");
        assert!(message.contains("Sign in…"), "{message}");
        // A local server refusing the profile's token: the fix is the token, not /login.
        let (state, message) = judge(Kind::Local, &ended(false, true, "Invalid API key · Please run /login"));
        assert_eq!(state, "failed");
        assert!(message.contains("token") && !message.contains("/login"), "{message}");
        assert_eq!(judge(Kind::System, &ended(false, true, "")).1, "Claude Code exited with 1");
    }

    #[test]
    fn an_unknown_model_fails_only_a_failed_run() {
        let warn = "[claude-code:unrecognized_model] {\"model\":\"qwen3.6:35b-a3b\",\"query_source\":\"sdk\"}";
        // A local model that worked, on a call too short for notes: skipped, not a model problem.
        assert!(!model_rejected(false, warn));
        assert!(model_rejected(true, warn));
        assert!(model_rejected(true, "There's an issue with the selected model (x)."));
        assert!(!model_rejected(true, "Not logged in · Please run /login"));
    }

    #[test]
    fn log_lines_from_stream_events() {
        let tool = serde_json::json!({ "type": "assistant", "message": { "content": [
            { "type": "text", "text": "Reading the context." },
            { "type": "tool_use", "name": "Bash", "input": { "command": "echoctl context /x" } } ] } });
        assert_eq!(describe(&tool), vec!["Claude: Reading the context.", "→ Bash echoctl context /x"]);
        let res = serde_json::json!({ "type": "result", "is_error": false, "duration_ms": 61000.0, "total_cost_usd": 0.4, "result": "Done.",
            "permission_denials": [{ "tool_name": "WebFetch", "tool_input": { "url": "https://x" } }] });
        let lines = describe(&res);
        assert_eq!(lines[0], "Finished: ok · 61s · $0.40");
        assert!(lines[2].starts_with("Not allowed: WebFetch"));
    }

    #[test]
    fn model_setting() {
        let mut c = Config::default();
        assert_eq!(c.notes_model_arg().as_deref(), Some("sonnet"));
        c.notes_model = Some("default".into());
        assert_eq!(c.notes_model_arg(), None);
        c.notes_model = Some("claude-opus-5-5".into());
        assert_eq!(c.notes_model_arg().as_deref(), Some("claude-opus-5-5"));
    }

    /// A closed, transcribed call worth minutes, ended `ago` before now.
    fn closed_call(root: &Path, name: &str, ago: chrono::Duration) -> PathBuf {
        let d = root.join("home/calls").join(name);
        fs::create_dir_all(&d).unwrap();
        let end = Local::now() - ago;
        let meta = json!({ "started_at": (end - chrono::Duration::minutes(30)).to_rfc3339(), "ended_at": end.to_rfc3339() });
        fs::write(d.join("meta.json"), meta.to_string()).unwrap();
        fs::write(d.join("transcript.md"), "# t\n").unwrap();
        fs::write(d.join("segments.json"), json!([{ "text": "x".repeat(2000) }]).to_string()).unwrap();
        d
    }

    fn auto_on(since: chrono::Duration) -> Config {
        let cfg = Config { auto_minutes_since: Some(chrono::Utc::now() - since), ..Default::default() };
        cfg.save().unwrap();
        cfg
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_debug_build_takes_the_test_delay_in_seconds_and_nothing_else() {
        assert_eq!(test_delay(Some("90")), Some(Duration::from_secs(90)));
        assert_eq!(test_delay(Some(" 5 ")), Some(Duration::from_secs(5)));
        // Unset or not whole seconds: the real 15 minutes.
        assert_eq!(test_delay(None), None);
        assert_eq!(test_delay(Some("")), None);
        assert_eq!(test_delay(Some("1.5")), None);
        assert_eq!(test_delay(Some("-3")), None);
        // The tests never set the variable: auto_due here runs on the real delay.
        if std::env::var_os(TEST_AUTO_DELAY).is_none() {
            assert_eq!(auto_delay(), AUTO_DELAY);
        }
    }

    #[test]
    fn minutes_come_15_minutes_after_the_call_or_the_last_note() {
        let (_env, root) = crate::paths::testenv::folders("notes-auto-due");
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        let cfg = auto_on(chrono::Duration::days(1));
        let now = Local::now();
        let d = closed_call(&root, "2026-10-23_090000-a", chrono::Duration::minutes(20));
        assert_eq!(auto_due(&d, &cfg, now, None), Due::Now);
        // A note 10 minutes ago moves it to 5 minutes from now.
        fs::write(d.join(calls::USER_NOTES), "mine").unwrap();
        fs::File::options().write(true).open(d.join(calls::USER_NOTES)).unwrap().set_modified((now - chrono::Duration::minutes(10)).into()).unwrap();
        let Due::At(t) = auto_due(&d, &cfg, now, None) else { panic!("not due yet") };
        assert!((t - (now + chrono::Duration::minutes(5))).num_seconds().abs() <= 1);
        // Just ended: 15 minutes after its end.
        let fresh = closed_call(&root, "2026-10-23_100000-b", chrono::Duration::minutes(1));
        assert!(matches!(auto_due(&fresh, &cfg, now, None), Due::At(_)));
        // Not yet (#42): later than that, or not until the user says; cleared, as before.
        let until = now + chrono::Duration::minutes(40);
        fs::write(fresh.join(LATER), json!({"until": until.to_rfc3339()}).to_string()).unwrap();
        assert_eq!(auto_due(&fresh, &cfg, now, None), Due::At(until));
        fs::write(fresh.join(LATER), r#"{"manual": true}"#).unwrap();
        assert_eq!(auto_due(&fresh, &cfg, now, None), Due::Not(NotDue::Held));
        set_later(&fresh, Some(0)).unwrap();
        assert!(matches!(auto_due(&fresh, &cfg, now, None), Due::At(_)));
        // Off, or ended before it was turned on.
        assert_eq!(auto_due(&d, &Config { auto_minutes: false, ..Config::default() }, now, None), Due::Not(NotDue::Off));
        let old = closed_call(&root, "2026-10-21_090000-c", chrono::Duration::days(2));
        assert_eq!(auto_due(&old, &cfg, now, None), Due::Not(NotDue::Before));
        // Made from a Teams transcript just now: it counts from its import, so an old meeting gets minutes.
        let imported = closed_call(&root, "2026-10-21_100000-k", chrono::Duration::days(2));
        let mut meta: serde_json::Value = serde_json::from_slice(&fs::read(imported.join("meta.json")).unwrap()).unwrap();
        meta["imported_at"] = json!((now - chrono::Duration::minutes(1)).to_rfc3339());
        fs::write(imported.join("meta.json"), meta.to_string()).unwrap();
        assert!(matches!(auto_due(&imported, &cfg, now, None), Due::At(_)));
        // Not closed, not transcribed, or being transcribed.
        let open = closed_call(&root, "2026-10-23_110000-d", chrono::Duration::minutes(30));
        fs::write(open.join("meta.json"), "{}").unwrap();
        assert_eq!(auto_due(&open, &cfg, now, None), Due::Not(NotDue::NotReady));
        let untranscribed = closed_call(&root, "2026-10-23_120000-e", chrono::Duration::minutes(30));
        fs::remove_file(untranscribed.join("transcript.md")).unwrap();
        assert_eq!(auto_due(&untranscribed, &cfg, now, None), Due::Not(NotDue::NotReady));
        let busy = closed_call(&root, "2026-10-23_130000-f", chrono::Duration::minutes(30));
        fs::write(busy.join(crate::transcribe_marker()), "1").unwrap();
        assert_eq!(auto_due(&busy, &cfg, now, None), Due::Not(NotDue::NotReady));
        // Too thin: why.
        let short = closed_call(&root, "2026-10-23_140000-g", chrono::Duration::minutes(30));
        fs::write(short.join("segments.json"), "[]").unwrap();
        assert_eq!(auto_due(&short, &cfg, now, None), Due::Not(NotDue::Gate("less than 1,500 characters of transcript")));
        // Tried: minutes exist, a run was recorded, or a session is writing them now.
        let done = closed_call(&root, "2026-10-23_150000-h", chrono::Duration::minutes(30));
        fs::write(done.join("notes.md"), "# m").unwrap();
        assert_eq!(auto_due(&done, &cfg, now, None), Due::Not(NotDue::Tried));
        let failed = closed_call(&root, "2026-10-23_160000-i", chrono::Duration::minutes(30));
        save(&failed, &Run { state: "failed".into(), ..Default::default() });
        assert_eq!(auto_due(&failed, &cfg, now, None), Due::Not(NotDue::Tried));
        let session = closed_call(&root, "2026-10-23_170000-j", chrono::Duration::minutes(30));
        fs::write(session.join(calls::CLAIMED), live_claim()).unwrap();
        assert_eq!(auto_due(&session, &cfg, now, None), Due::Not(NotDue::Tried));
        // What a pane is told: wait for the app (where it can write them), or not.
        let ask = |d: &Path, cfg: &Config| auto_answer_from(d, None, true, || auto_due(d, cfg, Local::now(), None));
        assert_eq!(ask(&fresh, &cfg)["waiting"], true);
        assert_eq!(ask(&open, &cfg)["waiting"], true);
        for not in [&done, &short, &old, &failed] {
            assert_eq!(ask(not, &cfg)["waiting"], false);
        }
        // Without Claude Code nothing comes; with Echo quit they come once it runs again.
        let none = auto_answer_from(&fresh, Some("Claude Code isn't installed"), true, || auto_due(&fresh, &cfg, Local::now(), None));
        assert_eq!((none["waiting"].as_bool(), none["reason"].as_str()), (Some(false), Some("no automatic minutes here: Claude Code isn't installed")));
        assert_eq!(auto_answer_from(&open, Some("x"), true, || Due::Not(NotDue::NotReady))["waiting"], false);
        let quit = auto_answer_from(&fresh, None, false, || Due::Now);
        assert_eq!((quit["waiting"].as_bool(), quit["reason"].as_str()), (Some(true), Some("Echo isn't running: it writes the minutes once it starts again")));
        // Notes that can't be read: the minutes wait for them, and the pane says why.
        let unreadable = closed_call(&root, "2026-10-23_210000-n", chrono::Duration::minutes(30));
        fs::write(unreadable.join(calls::USER_NOTES), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(auto_due(&unreadable, &cfg, now, None), Due::Not(NotDue::NotReady));
        let why = ask(&unreadable, &cfg);
        assert!(why["waiting"] == true && why["reason"].as_str().unwrap().starts_with("your notes (my-notes.md) can't be read"), "{why}");
        // Tried, but on their way: a run admitted (this process owns it) or a session writing them.
        let admitted = closed_call(&root, "2026-10-23_200000-m", chrono::Duration::minutes(30));
        save(&admitted, &Run { state: "queued".into(), pid: Some(std::process::id()), ..Default::default() });
        assert_eq!(auto_due(&admitted, &cfg, now, None), Due::Not(NotDue::Tried));
        assert_eq!(ask(&admitted, &cfg)["waiting"], true);
        assert_eq!(ask(&session, &cfg)["waiting"], true);
        // …whatever the computer: what is being written comes.
        assert_eq!(auto_answer_from(&session, Some("x"), false, || Due::Not(NotDue::Tried))["waiting"], true);
        // The scheduler moving meanwhile. Admitted between the pane's look and `auto_due`'s:
        let racing = closed_call(&root, "2026-10-23_190000-l", chrono::Duration::minutes(30));
        let admit_then_tried = || {
            save(&racing, &Run { state: "queued".into(), pid: Some(std::process::id()), ..Default::default() });
            Due::Not(NotDue::Tried)
        };
        assert_eq!(auto_answer_from(&racing, None, true, admit_then_tried)["waiting"], true);
        // Withdrawn after `auto_due` saw it (a later note): looked at again, due later.
        fs::remove_file(racing.join(RUN)).unwrap();
        let mut seen = vec![Due::Not(NotDue::Tried), Due::At(Local::now() + chrono::Duration::minutes(15))].into_iter();
        let answer = auto_answer_from(&racing, None, true, || seen.next().unwrap());
        assert_eq!(answer["waiting"], true, "{answer}");
        // Abandoned after `auto_due` saw it (Echo died while it waited): never tried, due again.
        save(&racing, &Run { state: "queued".into(), auto: true, pid: Some(dead_pid()), ..Default::default() });
        let mut seen = vec![Due::Not(NotDue::Tried), Due::At(Local::now() + chrono::Duration::minutes(15))].into_iter();
        assert_eq!(auto_answer_from(&racing, None, true, || seen.next().unwrap())["waiting"], true);
        // Ended after `auto_due` saw it running: final.
        save(&racing, &Run { state: "failed".into(), ..Default::default() });
        assert_eq!(auto_answer_from(&racing, None, true, || Due::Not(NotDue::Tried))["waiting"], false);
        // The switch off (or the call before it): what is being written is still on its way.
        let off = Config { auto_minutes: false, ..Config::default() };
        assert_eq!(ask(&admitted, &off)["waiting"], true);
        assert_eq!(ask(&session, &off)["waiting"], true);
        assert_eq!(auto_answer_from(&admitted, None, true, || Due::Not(NotDue::Before))["waiting"], true);
        assert_eq!(auto_answer(&fresh, &Config { auto_minutes: false, ..Config::default() })["waiting"], false);
        // An automatic run that was still waiting when its owner died never started: due again.
        let abandoned = closed_call(&root, "2026-10-23_180000-k", chrono::Duration::minutes(30));
        save(&abandoned, &Run { state: "queued".into(), auto: true, pid: Some(dead_pid()), ..Default::default() });
        assert_eq!(auto_due(&abandoned, &cfg, now, None), Due::Now);
        assert!(status(&abandoned).is_none(), "shown as never tried");
        // Nor one whose Claude Code stopped with it before writing anything (Echo quit, crashed).
        save(&abandoned, &Run { state: "running".into(), auto: true, pid: Some(dead_pid()), child_pid: Some(dead_pid()), ..Default::default() });
        assert_eq!(auto_due(&abandoned, &cfg, now, None), Due::Now);
        assert!(status(&abandoned).is_none());
        // …but not one whose Claude Code still writes, nor one that wrote the minutes.
        save(&abandoned, &Run { state: "running".into(), auto: true, pid: Some(dead_pid()), child_pid: Some(std::process::id()), ..Default::default() });
        assert_eq!(auto_due(&abandoned, &cfg, now, None), Due::Not(NotDue::Tried));
        save(&abandoned, &Run { state: "running".into(), auto: true, pid: Some(dead_pid()), ..Default::default() });
        fs::write(abandoned.join("notes.md"), "# Minutes\n").unwrap();
        assert_eq!(auto_due(&abandoned, &cfg, now, None), Due::Not(NotDue::Tried));
        assert_eq!(status(&abandoned).unwrap().state, "failed", "shown as a run that stopped");
    }

    #[test]
    fn an_automatic_run_starts_only_while_the_call_is_still_due() {
        let (_env, root) = crate::paths::testenv::folders("notes-auto-admit");
        fs::create_dir_all(root.join("home/.echo")).unwrap();
        auto_on(chrono::Duration::days(1));
        let d = closed_call(&root, "2026-10-23_090000-a", chrono::Duration::minutes(20));
        // A note saved between the scheduler's look and the admission: not admitted, nothing written.
        fs::write(d.join(calls::USER_NOTES), "just now").unwrap();
        assert!(admit(&d, Intent::Auto).unwrap().is_none());
        assert!(!d.join(RUN).exists() && !d.join(calls::CLAIMED).exists());
        fs::File::options().write(true).open(d.join(calls::USER_NOTES)).unwrap().set_modified((Local::now() - chrono::Duration::minutes(16)).into()).unwrap();
        // Due: admitted once, the second look finds it tried.
        let (place, r) = admit(&d, Intent::Auto).unwrap().unwrap();
        assert!(r.auto && r.run_id.is_some() && !r.had_minutes && status(&d).unwrap().state == "queued");
        drop(place);
        assert!(admit(&d, Intent::Auto).unwrap().is_none(), "ticked twice: admitted once");
        // Right before Claude Code would start: still due for this run (its own record is no "tried")…
        let cfg = Config::load().unwrap();
        assert_eq!(auto_due(&d, &cfg, Local::now(), Some(&r)), Due::Now);
        // …not after a note, nor once the switch went off and on again past the call.
        fs::write(d.join(calls::USER_NOTES), "one more").unwrap();
        assert!(matches!(auto_due(&d, &cfg, Local::now(), Some(&r)), Due::At(_)));
        let mut again = cfg;
        again.set_auto_minutes(false);
        again.set_auto_minutes(true);
        assert_eq!(auto_due(&d, &again, Local::now(), Some(&r)), Due::Not(NotDue::Before));
        // Withdrawn: never tried again, the app takes it up later.
        withdraw(&d);
        assert!(!d.join(RUN).exists() && !d.join(calls::CLAIMED).exists());
        // Explicit runs ignore all of that; an update says the call had minutes (the window keeps them).
        fs::write(d.join("notes.md"), "# m").unwrap();
        let (_place, r) = admit(&d, Intent::Explicit).unwrap().unwrap();
        assert!(r.had_minutes && !r.auto);
    }

    #[test]
    fn the_model_is_named_only_for_the_minutes_it_wrote() {
        let dir = std::env::temp_dir().join(format!("echo-minutes-model-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.md"), "# Minutes\n").unwrap();
        let sha256 = crate::models::sha256_file(&dir.join("notes.md")).unwrap();
        let written = Written { model: "Work · sonnet".into(), at: Local::now(), sha256, template: None };
        save(&dir, &Run { state: "done".into(), minutes: Some(written.clone()), ..Default::default() });
        assert_eq!(minutes_model(&dir).as_deref(), Some("Work · sonnet"));
        // An update queued, running, failed with another model: still the one that wrote them.
        save(&dir, &Run { state: "failed".into(), model: Some("Home · opus".into()), minutes: Some(written), ..Default::default() });
        assert_eq!(minutes_model(&dir).as_deref(), Some("Work · sonnet"));
        let rev = crate::minutes::revision(&dir);
        save(&dir, &Run { state: "queued".into(), model: None, minutes: raw(&dir).unwrap().minutes, ..Default::default() });
        assert_eq!(crate::minutes::revision(&dir), rev, "the page doesn't reload for a run's state");
        // An update refused before it started (no Claude Code, a profile that can't be pinned).
        save(&dir, &Run { state: "failed".into(), minutes: raw(&dir).unwrap().minutes, ..Default::default() });
        refuse(&dir, "Claude Code isn't installed");
        assert_eq!(status(&dir).unwrap().message.as_deref(), Some("Claude Code isn't installed"));
        assert_eq!(minutes_model(&dir).as_deref(), Some("Work · sonnet"), "a refused update keeps the model");
        // Rewritten since (an update that failed after writing, a session, a hand edit): no model.
        fs::write(dir.join("notes.md"), "# Minutes, rewritten\n").unwrap();
        assert_eq!(minutes_model(&dir), None);
        // A record from before: no hash, no model.
        save(&dir, &Run { state: "done".into(), model: Some("old".into()), ..Default::default() });
        assert_eq!(minutes_model(&dir), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_run_may_not_change_settings_or_keep_audio_through_its_commands() {
        assert_eq!(refused_in_run(false, false, false), None);
        assert!(refused_in_run(true, false, false).is_some_and(|w| w.contains("always-on")));
        for (keep, model) in [(true, false), (false, true), (true, true)] {
            assert!(refused_in_run(false, keep, model).is_some_and(|w| w.contains("audio")), "{keep} {model}");
        }
    }

    #[test]
    fn explicit_runs_may_override_the_gate_and_keep_the_users_files() {
        let (_env, _root) = crate::paths::testenv::folders("notes-args");
        let calls = paths::calls_dir();
        let s = Path::new("/tmp/s.json");
        let a = args(Path::new("/tmp/c"), Path::new("/tmp/p"), None, Intent::Explicit, "r1", None, s).join("\n");
        assert!(a.contains("asked for these minutes explicitly") && a.contains("/tmp/c/actions.new-r1.json"));
        let auto = args(Path::new("/tmp/c"), Path::new("/tmp/p"), None, Intent::Auto, "r1", None, s).join("\n");
        assert!(!auto.contains("explicitly"));
        let i = auto.lines().position(|l| l == "--disallowedTools").expect("deny rules");
        let denied: Vec<&str> = auto.lines().skip(i + 1).collect();
        let base = format!("/{}", rule_path(&calls));
        assert_eq!(denied, [format!("Edit({base}/**/my-notes.md)"), format!("Edit({base}/**/actions.json)")]);
        // The CLI only for what the skill does: not settings, the calendar, chats or quitting.
        let cli: Vec<&str> = auto.lines().filter(|l| l.starts_with("Bash(")).collect();
        for c in RUN_COMMANDS {
            assert!(cli.contains(&format!("Bash(echoctl {c}:*)").as_str()) && cli.contains(&format!("Bash(/tmp/p/scripts/echoctl {c}:*)").as_str()), "{c}");
        }
        assert_eq!(cli.len(), RUN_COMMANDS.len() * 2, "{cli:?}"); // the plugin's script = the launcher here
        // A Windows plugin folder: its script by the backslash path too, and the launcher with slashes.
        let win = args(Path::new("/tmp/c"), Path::new(r"C:\p"), None, Intent::Auto, "r1", None, s).join("\n");
        let names = |c: &str| win.lines().filter(|l| l.starts_with("Bash(") && l.ends_with(&format!(" {c}:*)"))).count();
        assert!(RUN_COMMANDS.iter().all(|c| names(c) == 3) && win.contains("Bash(C:/p/scripts/echoctl context:*)"), "{win}");
        assert!(!auto.contains("echoctl:*") && !["server", "config", "calendar", "app", "ai ", "home"].iter().any(|w| cli.iter().any(|l| l.contains(&format!("echoctl {w}")))));
    }

    #[test]
    fn a_run_reads_and_writes_only_its_call_and_runs_with_its_guard() {
        let (_env, _root) = crate::paths::testenv::folders("notes-args-scope");
        let s = Path::new("/tmp/s.json");
        let prev = Path::new("/tmp/calls/2026-10-18_100000-sync/notes.md");
        let a = args(Path::new("/tmp/calls/2026-10-25_100000-sync"), Path::new("/tmp/p"), None, Intent::Auto, "r1", Some(prev), s);
        let after = |flag: &str| a.iter().position(|x| x == flag).map(|i| a[i + 1].clone());
        assert_eq!(after("--settings").as_deref(), Some("/tmp/s.json"));
        assert_eq!(after("--tools").as_deref(), Some(RUN_TOOLS));
        assert!(a.contains(&"--strict-mcp-config".to_string()) && !a.contains(&"--add-dir".to_string()));
        // Files: this call only, and the previous minutes to read; the deny rules still cover every call.
        let files: Vec<&String> = a.iter().filter(|x| ["Read(", "Glob(", "Grep(", "Edit(", "Write("].iter().any(|t| x.starts_with(t))).collect();
        let call = "//tmp/calls/2026-10-25_100000-sync/**";
        let mut want: Vec<String> = ["Read", "Glob", "Grep", "Edit", "Write"].iter().map(|t| format!("{t}({call})")).collect();
        want.push("Read(//tmp/calls/2026-10-18_100000-sync/notes.md)".into());
        let base = format!("/{}", rule_path(&paths::calls_dir()));
        want.extend([format!("Edit({base}/**/my-notes.md)"), format!("Edit({base}/**/actions.json)")]);
        assert_eq!(files, want.iter().collect::<Vec<_>>());
        // No previous minutes: no other file at all.
        let none = args(Path::new("/tmp/calls/2026-10-25_100000-sync"), Path::new("/tmp/p"), None, Intent::Auto, "r1", None, s);
        assert!(!none.iter().any(|x| x.contains("2026-10-18")));
        // The prompt keeps the run in the folder.
        assert!(a[1].contains("only in this call's folder") && a[1].contains("keeps the folder's path until the run ends"));
    }

    #[test]
    fn the_run_settings_carry_the_guard_and_go_with_the_run() {
        let (_env, root) = crate::paths::testenv::folders("notes-run-settings");
        let stale = root.join("config/minutes-run/old.json");
        fs::create_dir_all(stale.parent().unwrap()).unwrap();
        fs::write(&stale, "{}").unwrap();
        let old = std::time::SystemTime::now() - Duration::from_secs(2 * 24 * 3600);
        fs::File::options().write(true).open(&stale).unwrap().set_modified(old).unwrap();
        let s = run_settings("r9").unwrap();
        let v: Value = serde_json::from_slice(&fs::read(&s.0).unwrap()).unwrap();
        assert!(v["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().ends_with(" run-guard"));
        assert_eq!(v["hooks"]["PreToolUse"][0]["hooks"][0]["onFailure"], "block");
        assert_eq!(v["claudeMdExcludes"][0], format!("{}/**", fwd(&paths::calls_dir())));
        assert!(!stale.exists(), "a file a run left a day ago goes");
        let f = s.0.clone();
        drop(s);
        assert!(!f.exists(), "removed when the run ends");
    }

    #[test]
    fn a_run_stopped_for_an_old_claude_code_says_why() {
        let e = Ended {
            wrote_notes: false,
            wrote_actions: false,
            is_error: true,
            result_text: "",
            err_text: "",
            seen: "",
            timed_out: false,
            refused: &[],
            exit: None,
            actions_error: None,
            stopped: Some("Claude Code 2.1.200 is too old for minutes runs"),
        };
        let mut r = Run::default();
        let p = crate::ai::Profile { id: "c".into(), name: "C".into(), kind: crate::ai::Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false };
        conclude(&mut r, &p, &e, "C · sonnet", "sonnet");
        assert_eq!(r.state, "failed");
        assert!(r.message.unwrap().contains("too old"));
    }

    #[test]
    fn a_list_the_run_wrote_but_didnt_apply_is_applied_for_it() {
        let (_env, root) = crate::paths::testenv::folders("notes-finish-actions");
        let d = root.join("home/calls/2026-10-23_090000-x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("meta.json"), "{}").unwrap();
        let start = std::time::SystemTime::now() - Duration::from_secs(60);
        let applied = |run: &str| serde_json::from_slice::<Value>(&fs::read(d.join(calls::ACTIONS_GENERATION)).unwrap()).unwrap()["run"] == run;
        let mut log = Vec::new();
        // Nothing left: no actions.
        assert_eq!(finish_actions(&d, "run-1", start, &mut log), Ok(false));
        // Its own file: applied as this run's.
        fs::write(d.join(calls::actions_new_of("run-1")), r#"[{"task": "Send the report", "owner": "Me"}]"#).unwrap();
        assert_eq!(finish_actions(&d, "run-1", start, &mut log), Ok(true));
        assert!(applied("run-1") && !d.join(calls::actions_new_of("run-1")).exists());
        assert!(String::from_utf8(log).unwrap().contains("applied the 1 action items"));
        // Applied by the run itself: nothing to do.
        assert_eq!(finish_actions(&d, "run-1", start, &mut Vec::new()), Ok(true));
        // The skill's usual file is never taken: it could be a session's, written meanwhile.
        fs::write(d.join("actions.new.json"), r#"[{"task": "From a session", "owner": null}]"#).unwrap();
        assert_eq!(finish_actions(&d, "run-2", start, &mut Vec::new()), Ok(false));
        assert!(d.join("actions.new.json").exists() && applied("run-1"));
        // Its own file, but from before this run started (a clock gone back): not this run's.
        fs::write(d.join(calls::actions_new_of("run-3")), "[]").unwrap();
        fs::File::options().write(true).open(d.join(calls::actions_new_of("run-3"))).unwrap().set_modified(start - Duration::from_secs(60)).unwrap();
        assert_eq!(finish_actions(&d, "run-3", start, &mut Vec::new()), Ok(false));
        // A malformed one: not applied, said in the log and to the run (its message says why).
        fs::write(d.join(calls::actions_new_of("run-4")), "{").unwrap();
        let mut log = Vec::new();
        assert!(matches!(finish_actions(&d, "run-4", start, &mut log), Err(why) if why.contains("isn't JSON")));
        assert!(String::from_utf8(log).unwrap().contains("can't be applied"));
    }
}

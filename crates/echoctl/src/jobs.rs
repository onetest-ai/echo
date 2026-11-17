//! Echo's background jobs (#669): what each one is, the settings the user may tune (how often, the night pause, the
//! model), and when each last ran. The settings are `<config dir>/jobs.json`, changed only through `set` (its lock,
//! re-read inside it), never inside a run Echo started; a job reads its own on every tick, so a change applies at
//! the next one. What a job did is kept as a routine's runs are: one line a run in `<config dir>/jobs-runs/<job>.jsonl`
//! (when, how it ended, its model, tokens and cost — `begin` … `Run::done`, or `record`), the newest 500 kept. Every job
//! runs on its defaults when the file is missing or a value in it can't be read.

use crate::paths;
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local, NaiveTime};
use serde_json::{json, Map, Value};

const FILE: &str = "jobs.json";
const RUNS: &str = "jobs-runs";
const NEXT: &str = "jobs-next.json";
/// Runs kept a job: those of the last `KEEP_DAYS` days, at most the newest `KEEP` (trimmed once a file passes
/// `KEEP * 2` lines or holds an older one).
const KEEP: usize = 5000;
const KEEP_DAYS: i64 = 31;

/// A setting a job has: its key, what it means, its unit and default.
pub struct Field {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: FieldKind,
}

pub enum FieldKind {
    /// A whole number between `min` and `max` (its unit in the label), `default` when not set.
    Number { min: u64, max: u64, default: u64 },
    /// Paused at night (`NIGHT`): on or off.
    Night { default: bool },
    /// One of Claude's names (haiku, sonnet, opus), which each profile maps to its provider's model; empty: the job's
    /// default.
    Model { default: &'static str },
}

pub struct Job {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
    /// Where what it does shows.
    pub to: &'static str,
    pub fields: &'static [Field],
}

const fn num(key: &'static str, label: &'static str, min: u64, max: u64, default: u64) -> Field {
    Field { key, label, kind: FieldKind::Number { min, max, default } }
}

const fn night(default: bool) -> Field {
    Field { key: "night", label: "Paused at night", kind: FieldKind::Night { default } }
}

const fn model_field(default: &'static str) -> Field {
    Field { key: "model", label: "Model", kind: FieldKind::Model { default } }
}

/// Every job Echo runs by itself, in the order the user sees them.
pub const JOBS: &[Job] = &[
    Job {
        id: "needs",
        name: "Needs you now",
        what: "Reads your Teams and mail of the last day for what waits on you, and chats that are burning",
        to: "Needs you now",
        fields: &[num("every", "Every (minutes)", 1, 240, 5), night(true), model_field("the profile's live model")],
    },
    Job {
        id: "thread_sync",
        name: "Thread sync",
        what: "Reads new calls, mail and chat into each thread's work and people",
        to: "Threads' work and people",
        fields: &[num("batch_wait", "Mail and chat wait at most (minutes)", 5, 1440, 120), num("batch_items", "…or until this many wait", 1, 50, 5), night(true), model_field("the thread's, else the profile's minutes model")],
    },
    Job {
        id: "fetch",
        name: "Threads' mail and meeting chats",
        what: "Fetches the mail and meeting chats threads follow (no model: what the thread sync reads)",
        to: "Threads' sources",
        fields: &[num("every", "Every (minutes)", 5, 1440, 30), night(true)],
    },
    Job {
        id: "minutes",
        name: "Minutes",
        what: "Writes a call's minutes: by itself after the call when automatic minutes are on, or when you ask",
        to: "The call's minutes",
        fields: &[num("delay", "Automatic: after the call (minutes)", 1, 240, 15), model_field("the profile's minutes model")],
    },
    Job {
        id: "prep",
        name: "Meeting prep",
        what: "Prepares today's meetings of threads, types meetings by their invites, and sorts your tasks by who can do them",
        to: "Meeting prep, meeting types, task modes",
        fields: &[num("every", "Every (minutes)", 1, 240, 5), night(false), model_field("the profile's minutes model")],
    },
    Job {
        id: "brain_reader",
        name: "Recall",
        what: "Indexes new calls, mail, channel posts and folders for Recall, reading mail and posts into plain facts",
        to: "Recall",
        fields: &[num("every", "Every (minutes)", 1, 1440, 5), num("per_pass", "Documents a pass", 1, 200, 30), night(false), model_field("the profile's minutes model")],
    },
    Job {
        id: "live_notes",
        name: "Live notes",
        what: "Keeps a running summary during a call",
        to: "The call's live notes",
        fields: &[num("every_s", "Summary every (seconds)", 30, 900, 90), model_field("the profile's live model")],
    },
];

/// The night jobs pause in (those with "Paused at night" on): `from` to `to`, local time.
pub const NIGHT: (&str, &str) = ("22:30", "07:00");

fn job(id: &str) -> Result<&'static Job> {
    JOBS.iter().find(|j| j.id == id).ok_or_else(|| anyhow!("no job “{id}”: {}", JOBS.iter().map(|j| j.id).collect::<Vec<_>>().join(", ")))
}

fn load_from(file: &str) -> Map<String, Value> {
    std::fs::read(paths::config_dir().join(file)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

/// The user's settings (only those set; indexing a missing one gives null).
pub fn settings() -> Value {
    Value::Object(load_from(FILE))
}

/// A number setting of a job: the user's when it is in range, else its default (1 for a key no job has: never a
/// loop without a pause).
pub fn number(id: &str, key: &str) -> u64 {
    let Some(Field { kind: FieldKind::Number { min, max, default }, .. }) = JOBS.iter().find(|j| j.id == id).and_then(|j| j.fields.iter().find(|f| f.key == key)) else { return 1 };
    settings()[id][key].as_u64().filter(|n| (*min..=*max).contains(n)).unwrap_or(*default)
}

/// Is `id` paused now: its night setting on (or its default) and `now` within the night (`night_window`)?
pub fn paused(id: &str, now: NaiveTime) -> bool {
    let Some(Field { kind: FieldKind::Night { default }, .. }) = JOBS.iter().find(|j| j.id == id).and_then(|j| j.fields.iter().find(|f| f.key == "night")) else { return false };
    settings()[id]["night"].as_bool().unwrap_or(*default) && in_night(now)
}

/// The night window: the user's `night.from` / `night.to` ("HH:MM"), else `NIGHT`.
pub fn night_window() -> (NaiveTime, NaiveTime) {
    let s = settings();
    let at = |k: &str, d: &str| s["night"][k].as_str().and_then(|t| NaiveTime::parse_from_str(t, "%H:%M").ok()).unwrap_or_else(|| NaiveTime::parse_from_str(d, "%H:%M").unwrap_or_default());
    (at("from", NIGHT.0), at("to", NIGHT.1))
}

/// `t` within the night: from `from` up to (not including) `to`, across midnight when `from` is later.
pub fn in_night(t: NaiveTime) -> bool {
    let (from, to) = night_window();
    if from <= to {
        t >= from && t < to
    } else {
        t >= from || t < to
    }
}

/// When the night that holds `at` ends (`at` itself when it isn't night).
pub fn after_night(at: DateTime<Local>) -> DateTime<Local> {
    if !in_night(at.time()) {
        return at;
    }
    let (from, to) = night_window();
    let day = if at.time() >= to && from > to { at.date_naive() + chrono::Days::new(1) } else { at.date_naive() };
    day.and_time(to).and_local_timezone(Local).earliest().unwrap_or(at)
}

/// The model the user set for a job, if any (else the job asks for its own default): one of Claude's names, mapped by
/// the profile where it reaches the provider (`Profile::model`).
pub fn model(id: &str) -> Option<String> {
    settings()[id]["model"].as_str().map(|m| m.trim().to_ascii_lowercase()).filter(|m| matches!(m.as_str(), "haiku" | "sonnet" | "opus"))
}

/// The model job `id` asks `p` for: the user's setting when `p` can run it — any profile but a local one that has no
/// model of its own for that name (its server knows none of Claude's) — else None (the job's own default).
pub fn model_for(id: &str, p: &crate::ai::Profile) -> Option<String> {
    model(id).filter(|m| p.kind != crate::ai::Kind::Local || p.model(m) != *m)
}

// ── runs ─────────────────────────────────────────────────────────────────────────────────────

thread_local! {
    /// What the job running on this thread used so far (`begin` … `Run::done`): its model calls' tokens and cost.
    static CURRENT: std::cell::RefCell<Option<Used>> = const { std::cell::RefCell::new(None) };
}

#[derive(Default, Clone)]
struct Used {
    calls: u64,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write: u64,
    cost_usd: f64,
    models: Vec<String>,
}

impl Used {
    fn value(&self) -> Value {
        json!({"calls": self.calls, "input": self.input, "output": self.output, "cache_read": self.cache_read, "cache_write": self.cache_write,
               "cost_usd": (self.cost_usd * 1e6).round() / 1e6, "model": self.models.join(", ")})
    }
}

/// A job's run on this thread: its model calls count towards it (`add_usage`) until it is `done` or `failed`.
pub struct Run {
    job: &'static str,
    started: DateTime<Local>,
    ended: bool,
}

/// Start a run of `job` on this thread.
pub fn begin(job: &'static str) -> Run {
    CURRENT.with(|c| *c.borrow_mut() = Some(Used::default()));
    Run { job, started: Local::now(), ended: false }
}

impl Run {
    /// It ended well. Recorded when it said what it did (`summary`) or called a model; a pass that found nothing to
    /// do leaves no run (a job looks every few minutes).
    pub fn done(mut self, summary: Option<String>) {
        self.ended = true;
        let used = CURRENT.with(|c| c.borrow_mut().take()).unwrap_or_default();
        if summary.is_some() || used.calls > 0 {
            write_run(self.job, self.started, Ok(summary.unwrap_or_default()), used.value());
        }
    }

    /// A minutes run of `dir` concluded (`notes`): its Claude Code result's usage, and done or why not.
    pub fn minutes(self, dir: &std::path::Path, result: Option<&Value>, model: Option<&str>, state: &str, message: Option<&str>) {
        if let Some(v) = result {
            add_usage(model.unwrap_or(""), v);
        }
        let call = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
        if state == "done" {
            self.done(Some(format!("minutes of {call}")));
        } else {
            self.failed(&format!("{call}: {}", message.unwrap_or(state)));
        }
    }

    /// It failed: always recorded, with why.
    pub fn failed(mut self, error: &str) {
        self.ended = true;
        let used = CURRENT.with(|c| c.borrow_mut().take()).unwrap_or_default();
        write_run(self.job, self.started, Err(error.to_string()), used.value());
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if !self.ended {
            let used = CURRENT.with(|c| c.borrow_mut().take()).unwrap_or_default();
            write_run(self.job, self.started, Err("it stopped before it ended".into()), used.value());
        }
    }
}

/// A model call on this thread used this (a Claude Code result event's `usage` and `total_cost_usd`, or a Messages API
/// answer's `usage`): counted towards the job running here, if any.
pub fn add_usage(model: &str, v: &Value) {
    CURRENT.with(|c| {
        if let Some(u) = c.borrow_mut().as_mut() {
            let x = &v["usage"];
            let n = |k: &str| x[k].as_u64().unwrap_or(0);
            u.calls += 1;
            u.input += n("input_tokens");
            u.output += n("output_tokens");
            u.cache_read += n("cache_read_input_tokens");
            u.cache_write += n("cache_creation_input_tokens");
            u.cost_usd += v["total_cost_usd"].as_f64().unwrap_or(0.0);
            if !model.is_empty() && !u.models.iter().any(|m| m == model) {
                u.models.push(model.to_string());
            }
        }
    });
}

/// A run that happened elsewhere (its own process or session): `used` as `{calls, input, output, cache_read,
/// cache_write, cost_usd, model}`, any of them absent.
pub fn record(job: &'static str, started: DateTime<Local>, result: Result<String, String>, used: Value) {
    write_run(job, started, result, used);
}

/// When a timed job looks next (its loop, after a pass or a paused tick): kept for the list, any process.
pub fn next_at(job: &str, at: DateTime<Local>) {
    let Ok(_lock) = crate::plugins::wait_lock_for(&paths::config_dir().join(".jobs-next.lock"), "Echo's jobs", std::time::Duration::from_secs(5)) else { return };
    let mut m = load_from(NEXT);
    m.insert(job.to_string(), json!(at.to_rfc3339()));
    let _ = std::fs::create_dir_all(paths::config_dir());
    let _ = paths::write_atomic(&paths::config_dir().join(NEXT), serde_json::to_string_pretty(&Value::Object(m)).unwrap_or_default().as_bytes());
}

/// After a timed job's pass: it looks again in `minutes` (its `every`), or at the night's end when that falls in a
/// night it pauses in. The seconds to sleep.
pub fn after_pass(job: &str, minutes: u64) -> u64 {
    let now = Local::now();
    let mut at = now + chrono::Duration::minutes(minutes as i64);
    if paused(job, at.time()) {
        at = after_night(at);
    }
    next_at(job, at);
    minutes * 60
}

/// What a job's next run waits for, in words, when it isn't a time (a job that runs on events).
fn next_words(id: &str) -> Option<String> {
    Some(match id {
        "thread_sync" => {
            let n = number(id, "batch_items");
            format!("after a call, or when mail and chat wait {} min or {n} item{}", number(id, "batch_wait"), if n == 1 { "" } else { "s" })
        }
        "fetch" => format!("each followed source every {} min", number(id, "every")),
        "minutes" => format!("{} min after a call", number(id, "delay")),
        "live_notes" => format!("during a call, every {} s", number(id, "every_s")),
        _ => return None,
    })
}

fn runs_file(job: &str) -> std::path::PathBuf {
    paths::config_dir().join(RUNS).join(format!("{job}.jsonl"))
}

fn write_run(job: &str, started: DateTime<Local>, result: Result<String, String>, used: Value) {
    let ended = Local::now();
    let mut line = json!({"started": started.to_rfc3339(), "ended": ended.to_rfc3339(), "duration_ms": (ended - started).num_milliseconds().max(0), "ok": result.is_ok()});
    match result {
        Ok(s) if !s.is_empty() => line["summary"] = json!(s.chars().take(300).collect::<String>()),
        Ok(_) => {}
        Err(e) => line["error"] = json!(e.chars().take(500).collect::<String>()),
    }
    for k in ["calls", "input", "output", "cache_read", "cache_write", "cost_usd", "model"] {
        if !used[k].is_null() && used[k] != json!("") {
            line[k] = used[k].clone();
        }
    }
    if let Some(c) = line["cost_usd"].as_f64() {
        line["cost_usd"] = json!((c * 1e6).round() / 1e6);
    }
    let Ok(_lock) = crate::plugins::wait_lock_for(&paths::config_dir().join(".jobs-runs.lock"), "Echo's jobs", std::time::Duration::from_secs(5)) else { return };
    let f = runs_file(job);
    let _ = std::fs::create_dir_all(f.parent().expect("a folder"));
    let mut lines: Vec<String> = std::fs::read_to_string(&f).unwrap_or_default().lines().filter(|l| !l.trim().is_empty()).map(str::to_owned).collect();
    lines.push(line.to_string());
    let oldest = (Local::now() - chrono::Duration::days(KEEP_DAYS)).to_rfc3339();
    let old = lines.first().and_then(|l| serde_json::from_str::<Value>(l).ok()).and_then(|v| v["started"].as_str().map(str::to_owned)).is_some_and(|t| t < oldest);
    if lines.len() > KEEP * 2 || old {
        lines.retain(|l| serde_json::from_str::<Value>(l).ok().and_then(|v| v["started"].as_str().map(str::to_owned)).is_some_and(|t| t >= oldest));
        if lines.len() > KEEP {
            lines.drain(..lines.len() - KEEP);
        }
    }
    let _ = paths::write_atomic(&f, (lines.join("\n") + "\n").as_bytes());
}

/// A job's runs, newest first, at most `limit`.
pub fn runs(job: &str, limit: usize) -> Result<Vec<Value>> {
    self::job(job)?;
    Ok(read_runs(job).into_iter().rev().take(limit).collect())
}

fn read_runs(job: &str) -> Vec<Value> {
    std::fs::read_to_string(runs_file(job)).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
}

/// `echoctl jobs`: every job with its settings (each with its value and default), last run and runs today, and the night.
pub fn list() -> Value {
    let s = settings();
    let next_runs = Value::Object(load_from(NEXT));
    let today = Local::now().format("%Y-%m-%d").to_string();
    let (from, to) = night_window();
    let jobs: Vec<Value> = JOBS
        .iter()
        .map(|j| {
            let fields: Vec<Value> = j
                .fields
                .iter()
                .map(|f| {
                    let (default, value, unit) = match &f.kind {
                        FieldKind::Number { min, max, default } => (json!(default), json!(number(j.id, f.key)), json!({"min": min, "max": max})),
                        FieldKind::Night { default } => (json!(default), json!(s[j.id]["night"].as_bool().unwrap_or(*default)), Value::Null),
                        FieldKind::Model { default } => (json!(default), s[j.id]["model"].as_str().map_or(json!(""), |m| json!(m)), Value::Null),
                    };
                    let kind = match f.kind {
                        FieldKind::Number { .. } => "number",
                        FieldKind::Night { .. } => "night",
                        FieldKind::Model { .. } => "model",
                    };
                    json!({"key": f.key, "label": f.label, "kind": kind, "value": value, "default": default, "range": unit, "set": s[j.id].get(f.key).is_some()})
                })
                .collect();
            let all = read_runs(j.id);
            let last = all.last().cloned().unwrap_or(Value::Null);
            let todays: Vec<&Value> = all.iter().filter(|r| r["started"].as_str().is_some_and(|t| t.starts_with(&today))).collect();
            let cost_today: f64 = todays.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
            let tokens_today: u64 = todays.iter().map(|r| ["input", "output", "cache_read", "cache_write"].iter().filter_map(|k| r[*k].as_u64()).sum::<u64>()).sum();
            // The last 30 days, a day each (oldest first), as a routine's month.
            let days: Vec<String> = (0..30).rev().map(|d| (Local::now().date_naive() - chrono::Days::new(d)).format("%Y-%m-%d").to_string()).collect();
            let daily: Vec<Value> = days
                .iter()
                .map(|d| {
                    let of: Vec<&Value> = all.iter().filter(|r| r["started"].as_str().is_some_and(|t| t.starts_with(d.as_str()))).collect();
                    let cost: f64 = of.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
                    json!({"day": d, "runs": of.len(), "failed": of.iter().filter(|r| r["ok"] == false).count(), "cost": (cost * 1e6).round() / 1e6})
                })
                .collect();
            let month = json!({"runs": daily.iter().map(|d| d["runs"].as_u64().unwrap_or(0)).sum::<u64>(),
                               "failed": daily.iter().map(|d| d["failed"].as_u64().unwrap_or(0)).sum::<u64>(),
                               "cost": (daily.iter().map(|d| d["cost"].as_f64().unwrap_or(0.0)).sum::<f64>() * 1e6).round() / 1e6, "daily": daily});
            let next = next_words(j.id);
            let next_run = if next.is_some() { Value::Null } else { next_runs[j.id].clone() };
            json!({"id": j.id, "name": j.name, "what": j.what, "to": j.to, "fields": fields, "last_run": last["started"], "last": last,
                   "next_run": next_run, "next": next, "month": month,
                   "runs_today": todays.len(), "failed_today": todays.iter().filter(|r| r["ok"] == false).count(),
                   "cost_today": (cost_today * 1e6).round() / 1e6, "tokens_today": tokens_today})
        })
        .collect();
    json!({"jobs": jobs, "night": {"from": from.format("%H:%M").to_string(), "to": to.format("%H:%M").to_string()}, "night_default": {"from": NIGHT.0, "to": NIGHT.1}})
}

/// `echoctl jobs set <job> key=value …` (`key=` back to the default), and `night from=HH:MM to=HH:MM`. All checked,
/// then written at once; never inside a run Echo started (how often Echo's jobs run and on which model is the user's).
pub fn set(id: &str, assignments: &[String]) -> Result<Value> {
    set_in(id, assignments, crate::notes::headless())
}

fn set_in(id: &str, assignments: &[String], in_run: bool) -> Result<Value> {
    if in_run {
        bail!("refused inside a run Echo started: how Echo's own jobs run is the user's to choose");
    }
    if assignments.is_empty() {
        bail!("nothing to set: key=value (key= for its default)");
    }
    let mut want: Vec<(String, Option<Value>)> = vec![];
    for a in assignments {
        let (k, v) = a.split_once('=').ok_or_else(|| anyhow!("“{a}”: key=value"))?;
        let (k, v) = (k.trim().to_string(), v.trim());
        if v.is_empty() {
            want.push((k, None));
            continue;
        }
        let value = if id == "night" {
            if k != "from" && k != "to" {
                bail!("the night has from and to (HH:MM), not “{k}”");
            }
            NaiveTime::parse_from_str(v, "%H:%M").map_err(|_| anyhow!("night {k}: “{v}” isn't a time (HH:MM)"))?;
            json!(v)
        } else {
            let f = job(id)?.fields.iter().find(|f| f.key == k).ok_or_else(|| anyhow!("{id} has {}, not “{k}”", job(id).map(|j| j.fields.iter().map(|f| f.key).collect::<Vec<_>>().join(", ")).unwrap_or_default()))?;
            match f.kind {
                FieldKind::Number { min, max, .. } => {
                    let n: u64 = v.parse().map_err(|_| anyhow!("{id} {k}: “{v}” isn't a whole number"))?;
                    if !(min..=max).contains(&n) {
                        bail!("{id} {k}: {n} is outside {min}–{max}");
                    }
                    json!(n)
                }
                FieldKind::Night { .. } => match v {
                    "on" | "true" => json!(true),
                    "off" | "false" => json!(false),
                    _ => bail!("{id} night: on or off, not “{v}”"),
                },
                FieldKind::Model { .. } => {
                    // One of Claude's names, never a provider's own: the job runs on every profile, and each maps it.
                    let v = v.to_ascii_lowercase();
                    if !matches!(v.as_str(), "haiku" | "sonnet" | "opus") {
                        bail!("{id} model: haiku, sonnet or opus (each profile runs its own model for them), not “{v}”");
                    }
                    json!(v)
                }
            }
        };
        want.push((k, Some(value)));
    }
    if id != "night" {
        job(id)?;
    }
    let _lock = crate::plugins::wait_lock_for(&paths::config_dir().join(".jobs.lock"), "Echo's jobs", std::time::Duration::from_secs(10))?;
    let mut s = load_from(FILE);
    let e = s.entry(id.to_string()).or_insert_with(|| json!({}));
    if !e.is_object() {
        *e = json!({});
    }
    for (k, v) in want {
        match (v, e.as_object_mut()) {
            (Some(v), Some(o)) => {
                o.insert(k, v);
            }
            (None, Some(o)) => {
                o.remove(&k);
            }
            _ => {}
        }
    }
    if e.as_object().is_some_and(Map::is_empty) {
        s.remove(id);
    }
    std::fs::create_dir_all(paths::config_dir())?;
    paths::write_atomic(&paths::config_dir().join(FILE), serde_json::to_string_pretty(&Value::Object(s))?.as_bytes())?;
    Ok(list())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn a_job_runs_on_its_defaults_until_the_user_sets_one_and_back_again() {
        let (_env, _) = paths::testenv::folders("jobs-defaults");
        assert_eq!((number("needs", "every"), number("thread_sync", "batch_wait"), number("live_notes", "every_s")), (5, 120, 90));
        assert_eq!(model("needs"), None);
        assert!(paused("needs", t(23, 0)) && !paused("needs", t(12, 0)) && !paused("minutes", t(23, 0)), "night on by default; a job with no night never pauses");
        assert!(!paused("prep", t(23, 0)) && !paused("brain_reader", t(23, 0)), "prep and Recall run at night until the user pauses them");
        assert_eq!(number("needs", "nope"), 1, "a key no job has: never 0");
        let set = |id: &str, a: &[&str]| set_in(id, &a.iter().map(|s| s.to_string()).collect::<Vec<_>>(), false);
        set("needs", &["every=15", "night=off", "model=Sonnet"]).unwrap();
        assert_eq!((number("needs", "every"), paused("needs", t(23, 0)), model("needs")), (15, false, Some("sonnet".into())));
        let l = list();
        let needs = l["jobs"].as_array().unwrap().iter().find(|j| j["id"] == "needs").unwrap();
        assert_eq!((needs["fields"][0]["value"].clone(), needs["fields"][0]["default"].clone(), needs["fields"][0]["set"].clone()), (json!(15), json!(5), json!(true)));
        set("needs", &["every=", "model="]).unwrap();
        assert_eq!((number("needs", "every"), model("needs")), (5, None), "cleared: the default again");
        // A value out of range in the file (edited by hand): the default, never a panic.
        std::fs::write(paths::config_dir().join(FILE), r#"{"needs": {"every": 0}, "fetch": "x"}"#).unwrap();
        assert_eq!((number("needs", "every"), number("fetch", "every")), (5, 30));
    }

    #[test]
    fn bad_settings_are_refused_all_or_nothing_and_never_inside_a_run() {
        let (_env, _) = paths::testenv::folders("jobs-refused");
        let set = |id: &str, a: &[&str], run: bool| set_in(id, &a.iter().map(|s| s.to_string()).collect::<Vec<_>>(), run);
        for (id, a) in [("needs", "every=0"), ("needs", "every=x"), ("needs", "night=maybe"), ("needs", "model=two words"), ("needs", "model=--x"), ("needs", "model=qwen3.8:27b"), ("needs", "batch_wait=5"), ("nope", "every=5"), ("night", "from=25:00"), ("night", "at=22:00")] {
            assert!(set(id, &["every=10", a], false).is_err() || set(id, &[a], false).is_err(), "{id} {a}");
        }
        assert!(set("needs", &["every=10", "every=0"], false).is_err());
        assert_eq!(number("needs", "every"), 5, "nothing written");
        assert!(set("needs", &["every=10"], true).unwrap_err().to_string().contains("inside a run"));
        assert_eq!(number("needs", "every"), 5);
    }

    #[test]
    fn the_night_is_the_users_window_across_midnight_or_not() {
        let (_env, _) = paths::testenv::folders("jobs-night");
        assert!(in_night(t(22, 30)) && in_night(t(3, 0)) && !in_night(t(7, 0)) && !in_night(t(22, 29)));
        let at = |h, m| Local::now().date_naive().and_hms_opt(h, m, 0).unwrap().and_local_timezone(Local).earliest().unwrap();
        assert_eq!(after_night(at(23, 0)), at(7, 0) + chrono::Duration::days(1));
        assert_eq!(after_night(at(3, 0)), at(7, 0));
        assert_eq!(after_night(at(15, 0)), at(15, 0));
        set_in("night", &["from=01:00".into(), "to=05:00".into()], false).unwrap();
        assert!(in_night(t(1, 0)) && in_night(t(4, 59)) && !in_night(t(23, 0)) && !in_night(t(5, 0)));
        assert_eq!(after_night(at(2, 0)), at(5, 0));
    }

    #[test]
    fn a_jobs_model_reaches_a_local_profile_only_where_it_maps_that_name() {
        let (_env, _) = paths::testenv::folders("jobs-model-for");
        set_in("prep", &["model=opus".into()], false).unwrap();
        let mut local = crate::ai::Profile { id: "q".into(), name: "Q".into(), kind: crate::ai::Kind::Local, base_url: None, token: None, live_model: Some("m".into()), notes_model: None, account: None, email: None, sso: false, models: Default::default(), wrapper: None };
        assert_eq!(model_for("prep", &local), None, "its server knows no opus: the job's own default");
        local.models.opus = Some("big-m".into());
        assert_eq!(model_for("prep", &local), Some("opus".into()));
        let claude = crate::ai::Profile { kind: crate::ai::Kind::Claude, ..local.clone() };
        assert_eq!(model_for("prep", &crate::ai::Profile { models: Default::default(), ..claude }), Some("opus".into()));
    }

    #[test]
    fn a_run_is_kept_with_its_model_calls_and_one_with_nothing_done_is_not() {
        let (_env, _) = paths::testenv::folders("jobs-runs");
        let r = begin("needs");
        add_usage("haiku", &json!({"usage": {"input_tokens": 100, "output_tokens": 20, "cache_read_input_tokens": 5}, "total_cost_usd": 0.0012}));
        add_usage("haiku", &json!({"usage": {"input_tokens": 50, "output_tokens": 10}, "total_cost_usd": 0.0008}));
        r.done(Some("3 waiting on you".into()));
        begin("needs").done(None);
        begin("needs").failed("Teams: unreachable");
        // Outside a run, a model call counts towards nothing.
        add_usage("sonnet", &json!({"usage": {"input_tokens": 9}}));
        let got = runs("needs", 10).unwrap();
        assert_eq!(got.len(), 2, "the pass with nothing done left no run: {got:?}");
        assert_eq!((got[0]["ok"].clone(), got[0]["error"].clone()), (json!(false), json!("Teams: unreachable")));
        assert_eq!((got[1]["summary"].clone(), got[1]["calls"].clone(), got[1]["input"].clone(), got[1]["cost_usd"].clone(), got[1]["model"].clone()), (json!("3 waiting on you"), json!(2), json!(150), json!(0.002), json!("haiku")));
        {
            let _dropped = begin("prep");
        }
        assert_eq!(runs("prep", 1).unwrap()[0]["error"], "it stopped before it ended");
        record("thread_sync", Local::now(), Ok("1 item read".into()), json!({"calls": 3, "input": 10, "cost_usd": 0.5, "model": "sonnet"}));
        let l = list();
        let job = |id: &str| l["jobs"].as_array().unwrap().iter().find(|j| j["id"] == id).unwrap().clone();
        assert_eq!((job("needs")["runs_today"].clone(), job("needs")["failed_today"].clone(), job("needs")["cost_today"].clone(), job("needs")["tokens_today"].clone()), (json!(2), json!(1), json!(0.002), json!(185)));
        assert_eq!((job("thread_sync")["last"]["summary"].clone(), job("thread_sync")["cost_today"].clone()), (json!("1 item read"), json!(0.5)));
        assert!(runs("nope", 1).is_err());
    }

    #[test]
    fn a_jobs_runs_older_than_a_month_go_and_the_month_is_counted_by_day() {
        let (_env, _) = paths::testenv::folders("jobs-runs-trim");
        let old = Local::now() - chrono::Duration::days(40);
        let f = runs_file("fetch");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, format!("{}\n", json!({"started": old.to_rfc3339(), "ok": true, "summary": "old"}))).unwrap();
        record("fetch", Local::now() - chrono::Duration::days(1), Ok("yesterday".into()), json!({"cost_usd": 0.25}));
        record("fetch", Local::now(), Err("today".into()), json!({"cost_usd": 0.5}));
        let got = runs("fetch", usize::MAX).unwrap();
        assert_eq!(got.iter().map(|r| r["summary"].as_str().or(r["error"].as_str()).unwrap()).collect::<Vec<_>>(), ["today", "yesterday"], "the run of 40 days ago is gone");
        let l = list();
        let fetch = l["jobs"].as_array().unwrap().iter().find(|j| j["id"] == "fetch").unwrap().clone();
        let daily = fetch["month"]["daily"].as_array().unwrap();
        assert_eq!(daily.len(), 30);
        assert_eq!((daily[29]["runs"].clone(), daily[29]["failed"].clone(), daily[28]["cost"].clone()), (json!(1), json!(1), json!(0.25)));
        assert_eq!((fetch["month"]["runs"].clone(), fetch["month"]["cost"].clone()), (json!(2), json!(0.75)));
        assert!(fetch["last"]["duration_ms"].is_number());
        assert_eq!((fetch["next_run"].clone(), fetch["next"].clone()), (Value::Null, json!("each followed source every 30 min")));
        assert_eq!(l["night_default"], json!({"from": "22:30", "to": "07:00"}));
    }

    #[test]
    fn a_timed_job_says_when_it_looks_next_after_a_pass_or_the_nights_end() {
        let (_env, _) = paths::testenv::folders("jobs-next");
        assert_eq!(after_pass("needs", 5), 300);
        let l = list();
        let needs = l["jobs"].as_array().unwrap().iter().find(|j| j["id"] == "needs").unwrap().clone();
        let at = chrono::DateTime::parse_from_rfc3339(needs["next_run"].as_str().unwrap()).unwrap();
        let want = if paused("needs", (Local::now() + chrono::Duration::minutes(5)).time()) { after_night(Local::now() + chrono::Duration::minutes(5)) } else { Local::now() + chrono::Duration::minutes(5) };
        assert!((at.timestamp() - want.timestamp()).abs() < 5, "{at} vs {want}");
        assert!(needs["next"].is_null() && needs["to"] == "Needs you now");
    }
}

//! Echo's background jobs (#669): what each one is, the settings the user may tune (how often, the night pause, the
//! model), and when each last ran. The settings are `<config dir>/jobs.json`, changed only through `set` (its lock,
//! re-read inside it), never inside a run Echo started; a job reads its own on every tick, so a change applies at
//! the next one. What a job did is `<config dir>/jobs-state.json` (`ran`). Every job runs on its defaults when the
//! file is missing or a value in it can't be read.

use crate::paths;
use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local, NaiveTime};
use serde_json::{json, Map, Value};

const FILE: &str = "jobs.json";
const STATE: &str = "jobs-state.json";

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
    /// A model name (one of Claude's names, mapped by the profile, or the provider's own); empty: the job's default.
    Model { default: &'static str },
}

pub struct Job {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
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
        fields: &[num("every", "Every (minutes)", 1, 240, 5), night(true), model_field("the profile's live model")],
    },
    Job {
        id: "thread_sync",
        name: "Thread sync",
        what: "Reads new calls, mail and chat into each thread's work and people",
        fields: &[num("batch_wait", "Mail and chat wait at most (minutes)", 5, 1440, 120), num("batch_items", "…or until this many wait", 1, 50, 5), night(true), model_field("the thread's, else the profile's minutes model")],
    },
    Job {
        id: "fetch",
        name: "Threads' mail and meeting chats",
        what: "Fetches the mail and meeting chats threads follow (no model: what the thread sync reads)",
        fields: &[num("every", "Every (minutes)", 5, 1440, 30), night(true)],
    },
    Job {
        id: "minutes",
        name: "Minutes",
        what: "Writes a call's minutes: by itself after the call when automatic minutes are on, or when you ask",
        fields: &[num("delay", "Automatic: after the call (minutes)", 1, 240, 15), model_field("the profile's minutes model")],
    },
    Job {
        id: "prep",
        name: "Meeting prep",
        what: "Prepares today's meetings of threads, types meetings by their invites, and sorts your tasks by who can do them",
        fields: &[num("every", "Every (minutes)", 1, 240, 5), night(true), model_field("the profile's minutes model")],
    },
    Job {
        id: "brain_reader",
        name: "Recall reader",
        what: "Reads new mail and channel posts into plain facts for Recall",
        fields: &[num("every", "Every (minutes)", 1, 1440, 5), num("per_pass", "Documents a pass", 1, 200, 30), night(true), model_field("the profile's minutes model")],
    },
    Job {
        id: "live_notes",
        name: "Live notes",
        what: "Keeps a running summary during a call",
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

/// A number setting of a job: the user's when it is in range, else its default.
pub fn number(id: &str, key: &str) -> u64 {
    let Some(Field { kind: FieldKind::Number { min, max, default }, .. }) = JOBS.iter().find(|j| j.id == id).and_then(|j| j.fields.iter().find(|f| f.key == key)) else { return 0 };
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

/// The model the user set for a job, if any (else the job asks for its own default). Not yet mapped: the profile maps
/// it (`Profile::model`) where it reaches the provider.
pub fn model(id: &str) -> Option<String> {
    settings()[id]["model"].as_str().map(str::trim).filter(|m| !m.is_empty()).map(str::to_owned)
}

/// Record that `id` did its work now (the list's last run and runs today).
pub fn ran(id: &str) {
    let path = paths::config_dir().join(STATE);
    let Ok(_lock) = crate::plugins::wait_lock_for(&paths::config_dir().join(".jobs-state.lock"), "Echo's jobs", std::time::Duration::from_secs(5)) else { return };
    let mut st = load_from(STATE);
    let now = Local::now();
    let today = now.format("%Y-%m-%d").to_string();
    let e = st.entry(id.to_string()).or_insert_with(|| json!({}));
    let runs = if e["day"].as_str() == Some(today.as_str()) { e["runs_today"].as_u64().unwrap_or(0) + 1 } else { 1 };
    *e = json!({"last_run": now.to_rfc3339(), "day": today, "runs_today": runs});
    let _ = paths::write_atomic(&path, serde_json::to_string_pretty(&Value::Object(st)).unwrap_or_default().as_bytes());
}

/// `echoctl jobs`: every job with its settings (each with its value and default), last run and runs today, and the night.
pub fn list() -> Value {
    let s = settings();
    let st = Value::Object(load_from(STATE));
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
            let e = &st[j.id];
            let runs_today = if e["day"].as_str() == Some(today.as_str()) { e["runs_today"].as_u64().unwrap_or(0) } else { 0 };
            json!({"id": j.id, "name": j.name, "what": j.what, "fields": fields, "last_run": e["last_run"], "runs_today": runs_today})
        })
        .collect();
    json!({"jobs": jobs, "night": {"from": from.format("%H:%M").to_string(), "to": to.format("%H:%M").to_string()}})
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
                    if !(v.len() <= 200 && !v.starts_with('-') && !v.chars().any(|c| c.is_control() || c.is_whitespace())) {
                        bail!("{id} model: “{v}” isn't a model name (one word, not starting with “-”)");
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
        let set = |id: &str, a: &[&str]| set_in(id, &a.iter().map(|s| s.to_string()).collect::<Vec<_>>(), false);
        set("needs", &["every=15", "night=off", "model=sonnet"]).unwrap();
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
        for (id, a) in [("needs", "every=0"), ("needs", "every=x"), ("needs", "night=maybe"), ("needs", "model=two words"), ("needs", "model=--x"), ("needs", "batch_wait=5"), ("nope", "every=5"), ("night", "from=25:00"), ("night", "at=22:00")] {
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
    fn a_run_is_counted_for_today() {
        let (_env, _) = paths::testenv::folders("jobs-ran");
        ran("needs");
        ran("needs");
        let l = list();
        let needs = l["jobs"].as_array().unwrap().iter().find(|j| j["id"] == "needs").unwrap();
        assert_eq!(needs["runs_today"], 2);
        assert!(needs["last_run"].is_string());
    }
}

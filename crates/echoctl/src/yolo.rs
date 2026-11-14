//! Yolo runs (#603): every run Claude did on its own from My work's Claude dialog — on a task, or on an item of Needs
//! you now — as one list, newest first (`/api/yolo`).
//!
//! A run's folder is `<config>/yolo/<id>/`. Besides what `claudejob::run` leaves there (`permissions.json`,
//! `actions.jsonl`, `drafts.jsonl`, `log.txt`), Yolo keeps the run's record in it, `run.json`: what it was on, when it
//! started and ended, its state and Claude's one line. A run of an older Echo has no record: its outcome comes from
//! `task-asks.json` while that still holds it. Runs older than `KEEP_DAYS` drop off the list, and their folders go when
//! the next run starts (`prune`).

// The list is the window's (`tray`); the record is written by Yolo (`js`).
#![cfg_attr(not(all(feature = "tray", feature = "js")), allow(dead_code))]

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local, TimeZone};
use serde_json::{json, Value};

use crate::paths;

/// How long a run stays on the list.
pub const KEEP_DAYS: i64 = 90;

/// A run still "working" this long after it started has ended without a record (Echo stopped during it): its limit is
/// 20 minutes.
const STALE_MINUTES: i64 = 25;

/// Where the runs' folders are.
pub fn dir() -> PathBuf {
    paths::config_dir().join("yolo")
}

/// A run's id names its folder: `ask-<ms>` as the page makes it (letters, digits and dashes).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Merge `v`'s fields into the run's record (`run.json`), making it when the run starts.
pub fn record(id: &str, v: &Value) {
    let d = dir().join(id);
    let _ = fs::create_dir_all(&d);
    let mut rec = read(&d.join("run.json"));
    if !rec.is_object() {
        rec = json!({});
    }
    for (k, x) in v.as_object().into_iter().flatten() {
        rec[k] = x.clone();
    }
    let _ = paths::write_atomic(&d.join("run.json"), &serde_json::to_vec_pretty(&rec).unwrap_or_default());
}

fn read(f: &Path) -> Value {
    fs::read(f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

fn time(v: &Value) -> Option<DateTime<Local>> {
    v.as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Local))
}

/// When a run started: its record's, else the time in its id (`ask-<ms>`), else its folder's.
fn started(d: &Path, rec: &Value) -> Option<DateTime<Local>> {
    time(&rec["started"])
        .or_else(|| d.file_name()?.to_str()?.strip_prefix("ask-")?.parse::<i64>().ok().and_then(|ms| Local.timestamp_millis_opt(ms).single()))
        .or_else(|| fs::metadata(d).and_then(|m| m.modified()).ok().map(DateTime::<Local>::from))
}

/// Drop the folders of runs older than `KEEP_DAYS`.
pub fn prune() {
    let keep = Local::now() - Duration::days(KEEP_DAYS);
    for d in fs::read_dir(dir()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        if started(&d, &read(&d.join("run.json"))).is_some_and(|t| t < keep) {
            let _ = fs::remove_dir_all(&d);
        }
    }
}

/// A change or send that went through (a draft is listed with the drafts; a refused or failed one changed nothing).
fn took_effect(a: &Value) -> bool {
    matches!(a["kind"].as_str(), Some("change" | "send")) && !a["outcome"].as_str().is_some_and(|o| o.starts_with("failed"))
}

/// One run as the list shows it: what it was on (a task, or an item of Needs you now), its thread, when, how long, its
/// state — working, done, didn't do it (`nothing`: it ended without changing, sending or drafting anything), failed —,
/// Claude's line, what it changed and sent, its drafts, and the permissions it ran with (`acts`: on its own; `asks`:
/// prepared and asked).
fn row(d: &Path, asks: &[Value]) -> Option<Value> {
    let id = d.file_name()?.to_str()?.to_string();
    if !valid_id(&id) {
        return None;
    }
    let rec = read(&d.join("run.json"));
    let start = started(d, &rec)?;
    if start < Local::now() - Duration::days(KEEP_DAYS) {
        return None;
    }
    // An older Echo's run: its outcome while task-asks.json holds it, else what its folder says.
    let old = (!rec.is_object()).then(|| asks.iter().find(|a| a["id"] == id.as_str()).cloned().unwrap_or(Value::Null));
    let rec = match old {
        None => rec,
        Some(a) if a.is_object() => json!({"title": a["title"], "state": a["state"], "say": a["say"], "ended": a["at"], "drafts": a["drafts"], "item": a["item"]}),
        Some(_) => {
            let answer = fs::read_to_string(d.join("answer.md")).ok();
            json!({"state": if answer.is_some() { "done" } else { "failed" }, "say": answer.map(|a| a.trim().chars().take(400).collect::<String>())})
        }
    };
    let actions = crate::notes::run_actions(&d.join("actions.jsonl"));
    let drafts = rec["drafts"].as_array().cloned().unwrap_or_else(|| crate::drafts::of_run(&d.join("drafts.jsonl"), |_| None));
    let changed = actions.iter().any(took_effect);
    let mut state = rec["state"].as_str().unwrap_or("failed").to_string();
    let mut say = rec["say"].clone();
    if state == "working" && start < Local::now() - Duration::minutes(STALE_MINUTES) {
        state = "failed".into();
        say = json!("It didn't finish: Echo stopped during the run.");
    }
    if state == "done" && !changed && drafts.is_empty() {
        state = "nothing".into();
    }
    let ended = time(&rec["ended"]).filter(|_| state != "working");
    let used = read(&d.join("permissions.json"))["used"].as_str().unwrap_or("").to_string();
    let permissions = if used.contains("acts on its own") { "acts" } else if used.contains("prepares and asks") { "asks" } else { "" };
    let thread = rec["thread"].as_str().filter(|t| !t.is_empty());
    let title = rec["title"].as_str().map(str::to_owned).or_else(|| log_title(d)).unwrap_or_else(|| "A Yolo run".into());
    Some(json!({
        "id": id, "kind": if rec["item"].is_string() { "needs" } else { "task" }, "title": title,
        "project": rec["project"], "node": rec["node"], "item": rec["item"],
        "thread": thread, "thread_name": thread.and_then(|t| crate::projects::load(t).ok()).map(|p| p.name),
        "started": start.to_rfc3339(), "ended": ended.map(|t| t.to_rfc3339()), "took": ended.map(|e| (e - start).num_seconds().max(0)),
        "state": state, "say": say, "changes": actions, "changed": changed, "drafts": drafts, "permissions": permissions,
        "log": d.join("log.txt").is_file(),
    }))
}

/// An older run's title, from its log's first line ("Echo runs “Yolo: <title>” on …").
fn log_title(d: &Path) -> Option<String> {
    let log = fs::read_to_string(d.join("log.txt")).ok()?;
    let t = log.lines().next()?.split_once("“Yolo: ")?.1.split_once('”')?.0;
    Some(t.to_string())
}

/// Every run on the list, newest first.
pub fn runs() -> Vec<Value> {
    let asks: Vec<Value> = fs::read(paths::state_dir().join("task-asks.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let mut out: Vec<Value> = fs::read_dir(dir()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).filter_map(|d| row(&d, &asks)).collect();
    out.sort_by(|a, b| b["started"].as_str().cmp(&a["started"].as_str()));
    out
}

/// A run's log, as it is (the page shows it as text).
pub fn log(id: &str) -> Option<String> {
    valid_id(id).then(|| fs::read_to_string(dir().join(id).join("log.txt")).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: &str, rec: Value, actions: &[Value], drafts: &[Value]) {
        record(id, &rec);
        let d = dir().join(id);
        let lines = |v: &[Value]| v.iter().map(|x| format!("{x}\n")).collect::<String>();
        fs::write(d.join("actions.jsonl"), lines(actions)).unwrap();
        fs::write(d.join("drafts.jsonl"), lines(drafts)).unwrap();
        fs::write(d.join("log.txt"), "12:00:00  Echo runs “Yolo: x” on Work\n").unwrap();
    }

    fn ago(minutes: i64) -> String {
        (Local::now() - Duration::minutes(minutes)).to_rfc3339()
    }

    #[test]
    fn every_run_newest_first_with_its_state_its_changes_and_its_drafts() {
        let (_env, _root) = paths::testenv::folders("yolo-runs");
        let change = json!({"kind": "change", "command": "project work acme '{}'"});
        run("ask-1", json!({"title": "Book the room", "project": "acme", "node": "T1", "thread": "acme", "started": ago(30), "ended": ago(29), "state": "done", "say": "Booked it."}), &[change.clone()], &[]);
        run("ask-2", json!({"title": "Collect the list", "project": "acme", "node": "T2", "started": ago(20), "ended": ago(20), "state": "done", "say": "Nobody sent it yet."}), &[], &[]);
        let refused = json!({"kind": "change", "command": "project create X", "outcome": "failed: refused"});
        run("ask-3", json!({"title": "Mail to you", "item": "mail:x", "started": ago(10), "ended": ago(9), "state": "done", "say": "Drafted a reply."}), &[refused, json!({"kind": "draft", "command": "compose --to kim@acme.example"})], &[json!({"where": "mail-app", "to": ["kim@acme.example"], "subject": "Re: x"})]);
        run("ask-4", json!({"title": "Plan it", "project": "acme", "node": "T3", "started": ago(2), "state": "working"}), &[], &[]);
        run("ask-5", json!({"title": "Stuck", "project": "acme", "node": "T4", "started": ago(40), "state": "working"}), &[], &[]);
        fs::write(dir().join("ask-1/permissions.json"), json!({"used": "the folder session's: acts on its own (every permission prompt skipped)"}).to_string()).unwrap();
        fs::write(dir().join("ask-3/permissions.json"), json!({"used": "the folder session's: prepares and asks (…)"}).to_string()).unwrap();
        let all = runs();
        let ids: Vec<&str> = all.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["ask-4", "ask-3", "ask-2", "ask-1", "ask-5"], "newest first");
        let by = |id: &str| all.iter().find(|r| r["id"] == id).unwrap().clone();
        let (one, two, three, four, five) = (by("ask-1"), by("ask-2"), by("ask-3"), by("ask-4"), by("ask-5"));
        assert_eq!((one["state"].as_str(), one["changed"].as_bool(), one["took"].as_i64(), one["permissions"].as_str(), one["kind"].as_str()), (Some("done"), Some(true), Some(60), Some("acts"), Some("task")));
        assert_eq!(two["state"], "nothing", "it ended changing, sending and drafting nothing: it didn't do it");
        assert_eq!((three["state"].as_str(), three["changed"].as_bool(), three["kind"].as_str(), three["permissions"].as_str()), (Some("done"), Some(false), Some("needs"), Some("asks")), "a draft is done; a refused change changed nothing");
        assert_eq!(three["drafts"][0]["subject"], "Re: x");
        assert_eq!((four["state"].as_str(), four["took"].is_null()), (Some("working"), true));
        assert_eq!(five["state"], "failed", "still working past its limit: Echo stopped during it");
        assert_eq!(log("ask-1").unwrap().lines().next(), Some("12:00:00  Echo runs “Yolo: x” on Work"));
        assert!(log("../ask-1").is_none() && log("").is_none());
    }

    #[test]
    fn a_run_older_than_ninety_days_drops_off_and_its_folder_goes_with_the_next_run() {
        let (_env, _root) = paths::testenv::folders("yolo-keep");
        run("ask-old", json!({"title": "Old", "started": (Local::now() - Duration::days(KEEP_DAYS + 1)).to_rfc3339(), "ended": ago(1), "state": "done"}), &[], &[]);
        run("ask-new", json!({"title": "New", "started": ago(5), "ended": ago(4), "state": "done"}), &[], &[]);
        assert_eq!(runs().iter().map(|r| r["id"].as_str().unwrap()).collect::<Vec<_>>(), ["ask-new"]);
        assert!(dir().join("ask-old").exists());
        prune();
        assert!(!dir().join("ask-old").exists() && dir().join("ask-new").exists());
    }

    #[test]
    fn an_older_echos_run_without_a_record_reads_its_outcome_from_task_asks() {
        let (_env, _root) = paths::testenv::folders("yolo-older");
        let ms = (Local::now() - Duration::minutes(10)).timestamp_millis();
        let (a, b) = (format!("ask-{ms}"), format!("ask-{}", ms + 1000));
        for id in [&a, &b] {
            fs::create_dir_all(dir().join(id)).unwrap();
            fs::write(dir().join(id).join("log.txt"), "12:00:00  Echo runs “Yolo: Book the room” on Work\n").unwrap();
        }
        fs::write(dir().join(&b).join("answer.md"), "Booked.\n").unwrap();
        fs::write(dir().join(&b).join("actions.jsonl"), format!("{}\n", json!({"kind": "change", "command": "project work acme"}))).unwrap();
        fs::create_dir_all(paths::state_dir()).unwrap();
        fs::write(paths::state_dir().join("task-asks.json"), json!([{"id": a, "state": "failed", "title": "From the asks", "say": "It broke", "at": ago(9)}]).to_string()).unwrap();
        let all = runs();
        assert_eq!((all[1]["title"].as_str(), all[1]["state"].as_str(), all[1]["say"].as_str(), all[1]["took"].as_i64()), (Some("From the asks"), Some("failed"), Some("It broke"), Some(60)));
        assert_eq!((all[0]["title"].as_str(), all[0]["state"].as_str(), all[0]["say"].as_str()), (Some("Book the room"), Some("done"), Some("Booked.")), "no outcome left: what its folder says");
    }
}

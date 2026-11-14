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

use std::collections::HashMap;
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

/// Whether a run marked working still runs: the Echo that started it is alive (`pid`), and — without a pid — it is
/// within its limit, or its log was written lately (a laptop asleep in the middle of a run wakes and goes on).
fn still_working(d: &Path, rec: &Value, start: DateTime<Local>) -> bool {
    if let Some(pid) = rec["pid"].as_u64().and_then(|p| u32::try_from(p).ok()) {
        return crate::app::process_alive(pid);
    }
    let now = Local::now();
    let logged = fs::metadata(d.join("log.txt")).and_then(|m| m.modified()).ok().map(DateTime::<Local>::from);
    start > now - Duration::minutes(STALE_MINUTES) || logged.is_some_and(|t| t > now - Duration::minutes(5))
}

/// One run as the list shows it: what it was on (a task, or an item of Needs you now), its thread, when, how long, its
/// state as its outcome says it (working, done, failed), Claude's line, how many changes and sends it made through
/// Echo's CLI (`through`: the commands themselves come with its log, `details`), its drafts, and the permissions it
/// ran with (`acts`: on its own; `asks`: prepared and asked). `tracked`: Echo noted what it did through its CLI (a run
/// of an older Echo may not have), so "nothing through Echo" can be said.
fn row(d: &Path, asks: &[Value], names: &mut HashMap<String, Option<String>>) -> Option<Value> {
    let id = d.file_name()?.to_str()?.to_string();
    if !valid_id(&id) {
        return None;
    }
    let rec = read(&d.join("run.json"));
    let start = started(d, &rec)?;
    if start < Local::now() - Duration::days(KEEP_DAYS) {
        return None;
    }
    let tracked = rec.is_object() || d.join("actions.jsonl").is_file();
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
    let count = |k: &str| actions.iter().filter(|a| a["kind"] == k && took_effect(a)).count();
    let tried = actions.iter().filter(|a| matches!(a["kind"].as_str(), Some("change" | "send")) && !took_effect(a)).count();
    let (changes, sends) = (count("change"), count("send"));
    let mut state = rec["state"].as_str().unwrap_or("failed").to_string();
    let mut say = rec["say"].clone();
    if state == "working" && !still_working(d, &rec, start) {
        state = "failed".into();
        say = json!("It didn't finish: Echo stopped during the run.");
    }
    let ended = time(&rec["ended"]).filter(|_| state != "working");
    let used = read(&d.join("permissions.json"))["used"].as_str().unwrap_or("").to_string();
    let permissions = if used.contains("acts on its own") { "acts" } else if used.contains("prepares and asks") { "asks" } else { "" };
    let thread = rec["thread"].as_str().filter(|t| !t.is_empty());
    let thread_name = thread.and_then(|t| names.entry(t.to_string()).or_insert_with(|| crate::projects::load(t).ok().map(|p| p.name)).clone());
    let title = rec["title"].as_str().map(str::to_owned).or_else(|| log_title(d)).unwrap_or_else(|| "A Yolo run".into());
    // An item of Needs you now by what it is about too: "Mail to you" alone says little.
    let title = match rec["subject"].as_str().filter(|s| !s.trim().is_empty()) {
        Some(s) => format!("{title} · {}", s.trim()),
        None => title,
    };
    Some(json!({
        "id": id, "kind": if rec["item"].is_string() { "needs" } else { "task" }, "title": title,
        "project": rec["project"], "node": rec["node"], "item": rec["item"],
        "thread": thread, "thread_name": thread_name,
        "started": start.to_rfc3339(), "ended": ended.map(|t| t.to_rfc3339()), "took": ended.map(|e| (e - start).num_seconds().max(0)),
        "state": state, "say": say, "through": {"changes": changes, "sends": sends, "tried": tried}, "changed": changes + sends > 0,
        "tracked": tracked, "drafts": drafts, "permissions": permissions, "log": d.join("log.txt").is_file(),
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
    let mut names = HashMap::new();
    let mut out: Vec<Value> = fs::read_dir(dir()).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).filter_map(|d| row(&d, &asks, &mut names)).collect();
    out.sort_by_key(|r| std::cmp::Reverse(time(&r["started"])));
    out
}

/// At most this much of a log is sent to the page: its end.
const LOG_MAX: usize = 200 * 1024;

/// A run's details: its log as it is (the page shows it as text; only its last `LOG_MAX` bytes, `cut` when longer)
/// and every change and send it made through Echo's CLI.
pub fn details(id: &str) -> Option<Value> {
    if !valid_id(id) {
        return None;
    }
    let d = dir().join(id);
    let log = fs::read_to_string(d.join("log.txt")).ok()?;
    let cut = log.len() > LOG_MAX;
    let mut from = log.len().saturating_sub(LOG_MAX);
    while !log.is_char_boundary(from) {
        from += 1;
    }
    let changes: Vec<Value> = crate::notes::run_actions(&d.join("actions.jsonl")).into_iter().filter(|a| matches!(a["kind"].as_str(), Some("change" | "send"))).collect();
    Some(json!({"log": &log[from..], "cut": cut, "changes": changes}))
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
    fn every_run_newest_first_with_its_outcome_what_it_did_through_echo_and_its_drafts() {
        let (_env, _root) = paths::testenv::folders("yolo-runs");
        let change = json!({"kind": "change", "command": "project work acme '{}'"});
        run("ask-1", json!({"title": "Book the room", "project": "acme", "node": "T1", "thread": "acme", "started": ago(30), "ended": ago(29), "state": "done", "say": "Booked it."}), &[change.clone()], &[]);
        run("ask-2", json!({"title": "Collect the list", "project": "acme", "node": "T2", "started": ago(20), "ended": ago(20), "state": "done", "say": "Nobody sent it yet."}), &[], &[]);
        let refused = json!({"kind": "change", "command": "project create X", "outcome": "failed: refused"});
        run("ask-3", json!({"title": "Mail to you", "subject": "Contract redlines", "item": "mail:x", "started": ago(10), "ended": ago(9), "state": "done", "say": "Drafted a reply."}), &[refused, json!({"kind": "draft", "command": "compose --to kim@acme.example"})], &[json!({"where": "mail-app", "to": ["kim@acme.example"], "subject": "Re: x"})]);
        run("ask-4", json!({"title": "Plan it", "project": "acme", "node": "T3", "started": ago(2), "state": "working"}), &[], &[]);
        fs::write(dir().join("ask-1/permissions.json"), json!({"used": "the folder session's: acts on its own (every permission prompt skipped)"}).to_string()).unwrap();
        fs::write(dir().join("ask-3/permissions.json"), json!({"used": "the folder session's: prepares and asks (…)"}).to_string()).unwrap();
        let all = runs();
        let ids: Vec<&str> = all.iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["ask-4", "ask-3", "ask-2", "ask-1"], "newest first");
        let by = |id: &str| all.iter().find(|r| r["id"] == id).unwrap().clone();
        let (one, two, three, four) = (by("ask-1"), by("ask-2"), by("ask-3"), by("ask-4"));
        assert_eq!((one["state"].as_str(), one["changed"].as_bool(), one["took"].as_i64(), one["permissions"].as_str(), one["kind"].as_str()), (Some("done"), Some(true), Some(60), Some("acts"), Some("task")));
        assert_eq!(one["through"], json!({"changes": 1, "sends": 0, "tried": 0}));
        assert!(one.get("changes").is_none(), "the commands come with the run's details, not every poll");
        assert_eq!((two["state"].as_str(), two["changed"].as_bool(), two["tracked"].as_bool()), (Some("done"), Some(false), Some(true)), "done as its outcome says: nothing through Echo is a note, not a verdict");
        assert_eq!((three["state"].as_str(), three["changed"].as_bool(), three["kind"].as_str(), three["permissions"].as_str()), (Some("done"), Some(false), Some("needs"), Some("asks")), "a refused change changed nothing");
        assert_eq!((three["title"].as_str(), three["through"]["tried"].as_u64(), three["drafts"][0]["subject"].as_str()), (Some("Mail to you · Contract redlines"), Some(1), Some("Re: x")));
        assert_eq!((four["state"].as_str(), four["took"].is_null()), (Some("working"), true));
        let d = details("ask-3").unwrap();
        assert_eq!(d["log"].as_str().unwrap().lines().next(), Some("12:00:00  Echo runs “Yolo: x” on Work"));
        assert_eq!((d["cut"].as_bool(), d["changes"].as_array().unwrap().len()), (Some(false), 1), "its change and send commands; the draft is in its drafts");
        assert!(details("../ask-1").is_none() && details("").is_none() && details(&"a".repeat(65)).is_none());
    }

    #[test]
    fn a_run_still_working_is_failed_only_once_its_echo_is_gone_or_it_is_past_its_limit_and_quiet() {
        let (_env, _root) = paths::testenv::folders("yolo-stale");
        // Its Echo is alive: working, however long ago it started.
        run("ask-alive", json!({"title": "a", "started": ago(60), "state": "working", "pid": std::process::id()}), &[], &[]);
        // Its Echo is gone (a pid no process has): failed at once.
        run("ask-gone", json!({"title": "b", "started": ago(1), "state": "working", "pid": i32::MAX as u32}), &[], &[]);
        // No pid (an older record): past its limit but its log written just now (a laptop that slept), still working;
        run("ask-slept", json!({"title": "c", "started": ago(40), "state": "working"}), &[], &[]);
        // past its limit and its log quiet for longer than 5 minutes: failed.
        run("ask-quiet", json!({"title": "d", "started": ago(40), "state": "working"}), &[], &[]);
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(10 * 60);
        fs::File::options().write(true).open(dir().join("ask-quiet/log.txt")).unwrap().set_modified(old).unwrap();
        let all = runs();
        let state = |id: &str| all.iter().find(|r| r["id"] == id).unwrap()["state"].as_str().unwrap().to_string();
        assert_eq!([state("ask-alive"), state("ask-gone"), state("ask-slept"), state("ask-quiet")], ["working", "failed", "working", "failed"]);
    }

    #[test]
    fn a_long_log_comes_as_its_end_and_says_it_is_cut() {
        let (_env, _root) = paths::testenv::folders("yolo-log");
        run("ask-long", json!({"title": "x", "started": ago(1), "state": "done"}), &[], &[]);
        let long = format!("{}\nthe last line\n", "é".repeat(LOG_MAX));
        fs::write(dir().join("ask-long/log.txt"), &long).unwrap();
        let d = details("ask-long").unwrap();
        let log = d["log"].as_str().unwrap();
        assert_eq!(d["cut"], true);
        assert!(log.len() <= LOG_MAX && log.ends_with("the last line\n"));
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
        assert_eq!(all[0]["tracked"], true, "it has actions.jsonl");
    }

    #[test]
    fn an_older_run_with_neither_a_record_nor_its_actions_keeps_its_outcome_and_says_nothing_of_echo() {
        let (_env, _root) = paths::testenv::folders("yolo-untracked");
        let id = format!("ask-{}", (Local::now() - Duration::minutes(10)).timestamp_millis());
        fs::create_dir_all(dir().join(&id)).unwrap();
        fs::write(dir().join(&id).join("log.txt"), "12:00:00  Echo runs “Yolo: Send the deck” on Work\n").unwrap();
        fs::write(dir().join(&id).join("answer.md"), "Sent the deck.\n").unwrap();
        let r = runs().remove(0);
        assert_eq!((r["state"].as_str(), r["tracked"].as_bool(), r["changed"].as_bool()), (Some("done"), Some(false), Some(false)), "Echo didn't note what it did then: done, nothing claimed");
    }
}

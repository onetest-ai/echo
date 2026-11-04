//! Memos: the user alone, recorded by hand — no call, no thread to pick (`Recording::start_memo`, `echoctl memo
//! start`, the window's Record ▾, the menu bar). Processed like a call; their threads are found from what was said:
//! the minutes run gets the open threads (`open_threads`, in `context`), splits the memo into parts by thread and
//! hands them to `echoctl memo threads <call> --file …` (`set_threads`). Echo checks them, pins the memo to each thread
//! and keeps the parts in `<call>/memo-threads.json`; each thread's sync reads only its part (`parts_for`). "Not this"
//! on a chip is the thread's exclusion: a later split never puts the memo back there.

use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{calls, paths, projects};

const FILE: &str = "memo-threads.json";

/// A memo's part about one thread: `[from, to)` seconds into the memo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Part {
    pub thread: String,
    pub from: f64,
    pub to: f64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    parts: Vec<Part>,
    /// Threads this split pinned the memo to: a later split unpins only these (never the user's own pins).
    #[serde(default)]
    pinned: Vec<String>,
    #[serde(default)]
    at: String,
}

/// Is this call a memo (`meta.json` `kind`)?
pub fn is_memo(dir: &Path) -> bool {
    calls::read_json(&dir.join("meta.json"))["kind"] == "memo"
}

/// The threads a memo may be about, for its minutes run: every open one, with what tells them apart.
pub fn open_threads(dir: &Path) -> Value {
    let stamp = calls::stamp_of(dir);
    let clients = projects::clients().unwrap_or_default();
    json!(projects::list()
        .into_iter()
        .filter(|p| !p.stage.closed() && !p.exclusions.contains(&stamp))
        .map(|p| {
            let brief: String = projects::brief(&p.id).unwrap_or_default().chars().take(400).collect();
            json!({
                "id": p.id, "name": p.name,
                "client": p.client.as_ref().and_then(|c| clients.iter().find(|x| &x.id == c)).map(|c| c.name.clone()),
                "brief": brief,
                "terms": p.terms.iter().map(|t| t.term.clone()).take(30).collect::<Vec<_>>(),
                "people": p.people.iter().map(|m| m.name.clone()).take(15).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>())
}

/// The memo's parts as saved (none yet: empty).
pub fn parts(dir: &Path) -> Vec<Part> {
    load(dir).parts
}

fn load(dir: &Path) -> Saved {
    std::fs::read(dir.join(FILE)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// One thread's spans of a memo (seconds), or None when the call isn't a memo split by thread — then the whole call
/// is the thread's, as for any call.
pub fn parts_for(dir: &Path, thread: &str) -> Option<Vec<(f64, f64)>> {
    if !is_memo(dir) {
        return None;
    }
    let saved = load(dir);
    if saved.parts.is_empty() {
        return None;
    }
    Some(saved.parts.iter().filter(|p| p.thread == thread).map(|p| (p.from, p.to)).collect())
}

/// A time as the run writes it: seconds, or "mm:ss" / "h:mm:ss".
fn seconds(v: &Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    let s = v.as_str()?.trim().trim_matches(|c| c == '[' || c == ']');
    s.split(':').try_fold(0f64, |acc, x| x.trim().parse::<f64>().ok().map(|n| acc * 60.0 + n))
}

/// `echoctl memo threads <call> --file <json>`: the minutes run's split — `[{thread, from, to}]` — checked and applied.
/// Unknown or closed threads, ones the user took the memo out of, and spans outside the memo are dropped (each said);
/// the memo is pinned to the threads that remain, and unpinned from those an earlier split put it in and this one
/// doesn't name. Returns what was kept and what was dropped.
pub fn set_threads(call: &str, file: &Path) -> Result<Value> {
    let dir = calls::resolve_call(call)?;
    if !is_memo(&dir) {
        bail!("{} isn't a memo", dir.display());
    }
    let raw: Value = serde_json::from_slice(&std::fs::read(file).with_context(|| format!("reading {}", file.display()))?)
        .with_context(|| format!("{} isn't JSON", file.display()))?;
    let items = raw.as_array().or_else(|| raw["parts"].as_array()).cloned().unwrap_or_default();
    let stamp = calls::stamp_of(&dir);
    let all = projects::list();
    let length = memo_length(&dir);
    let mut parts: Vec<Part> = vec![];
    let mut dropped = vec![];
    for it in &items {
        let id = it["thread"].as_str().unwrap_or("").trim();
        let (Some(from), Some(to)) = (seconds(&it["from"]), seconds(&it["to"])) else {
            dropped.push(json!({"item": it, "why": "from and to must be times"}));
            continue;
        };
        let Some(p) = all.iter().find(|p| p.id == id) else {
            dropped.push(json!({"item": it, "why": "no such thread"}));
            continue;
        };
        if p.stage.closed() {
            dropped.push(json!({"item": it, "why": "a closed thread"}));
            continue;
        }
        if p.exclusions.contains(&stamp) {
            dropped.push(json!({"item": it, "why": "the user took this memo out of that thread"}));
            continue;
        }
        let (from, to) = (from.max(0.0), length.map_or(to, |l| to.min(l + 1.0)));
        if to <= from {
            dropped.push(json!({"item": it, "why": "the span is empty or outside the memo"}));
            continue;
        }
        parts.push(Part { thread: p.id.clone(), from, to });
    }
    parts.sort_by(|a, b| a.from.total_cmp(&b.from));
    let before = load(&dir);
    let threads: Vec<String> = parts.iter().map(|p| p.thread.clone()).fold(vec![], |mut v, t| {
        if !v.contains(&t) {
            v.push(t);
        }
        v
    });
    // Pins this split adds, and the earlier split's that it no longer names (the user's own pins stay).
    let mut pinned = vec![];
    for t in &threads {
        let p = all.iter().find(|p| &p.id == t).expect("checked above");
        if !p.pins.contains(&stamp) {
            projects::set_membership(t, &dir, "include")?;
            pinned.push(t.clone());
        } else if before.pinned.contains(t) {
            pinned.push(t.clone());
        }
    }
    for t in before.pinned.iter().filter(|t| !threads.contains(t)) {
        if all.iter().any(|p| &p.id == t && p.pins.contains(&stamp)) {
            projects::set_membership(t, &dir, "rules")?;
        }
    }
    let saved = Saved { parts: parts.clone(), pinned, at: chrono::Local::now().to_rfc3339() };
    paths::write_atomic(&dir.join(FILE), &serde_json::to_vec_pretty(&saved)?)?;
    Ok(json!({"parts": parts, "dropped": dropped}))
}

/// The memo's length in seconds (its last segment's end), when it is transcribed.
fn memo_length(dir: &Path) -> Option<f64> {
    calls::read_json(&dir.join("segments.json")).as_array()?.iter().filter_map(|s| s["end"].as_f64()).reduce(f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_split_pins_its_threads_and_respects_not_this() {
        let (_env, root) = paths::testenv::folders("memo-split");
        let dir = root.join("home/calls/2026-11-04_140000-memo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("meta.json"), r#"{"title":"Memo","kind":"memo","started_at":"2026-11-04T14:00:00+02:00","ended_at":"2026-11-04T14:00:40+02:00"}"#).unwrap();
        std::fs::write(dir.join("segments.json"), r#"[{"speaker":"me","start":0.0,"end":12.0,"text":"Acme"},{"speaker":"me","start":14.0,"end":21.0,"text":"Globex"}]"#).unwrap();
        let acme = projects::create("Acme rollout", None, None).unwrap();
        let globex = projects::create("Globex discovery", None, None).unwrap();
        let done = projects::create("Old pilot", None, Some(projects::Stage::Done)).unwrap();
        let call = dir.display().to_string();
        let file = root.join("split.json");
        let write = |v: Value| std::fs::write(&file, v.to_string()).unwrap();

        write(json!([{"thread": acme.id, "from": "00:00", "to": "00:13"}, {"thread": globex.id, "from": 13, "to": 99},
                     {"thread": done.id, "from": 0, "to": 5}, {"thread": "nope", "from": 0, "to": 5}]));
        let out = set_threads(&call, &file).unwrap();
        assert_eq!(out["parts"].as_array().unwrap().len(), 2);
        assert_eq!(out["dropped"].as_array().unwrap().len(), 2, "a closed thread and an unknown one are dropped: {out}");
        assert_eq!(parts_for(&dir, &globex.id), Some(vec![(13.0, 22.0)]), "clamped to the memo's end");
        let stamp = calls::stamp_of(&dir);
        assert!(projects::load(&acme.id).unwrap().pins.contains(&stamp) && projects::load(&globex.id).unwrap().pins.contains(&stamp));

        // Not this on Globex, then a new split naming both: Globex stays out; Acme's pin goes when it isn't named.
        projects::set_membership(&globex.id, &dir, "exclude").unwrap();
        write(json!([{"thread": globex.id, "from": 0, "to": 20}]));
        let out = set_threads(&call, &file).unwrap();
        assert!(out["parts"].as_array().unwrap().is_empty(), "{out}");
        assert!(!projects::load(&acme.id).unwrap().pins.contains(&stamp), "the earlier split's pin is taken back");
        assert!(projects::load(&globex.id).unwrap().exclusions.contains(&stamp));

        // A pin the user made by hand is never taken back by a split.
        projects::set_membership(&acme.id, &dir, "include").unwrap();
        write(json!([]));
        set_threads(&call, &file).unwrap();
        assert!(projects::load(&acme.id).unwrap().pins.contains(&stamp));
    }

    #[test]
    fn times_read_as_seconds_or_clock() {
        assert_eq!(seconds(&json!(42.5)), Some(42.5));
        assert_eq!(seconds(&json!("02:40")), Some(160.0));
        assert_eq!(seconds(&json!("[1:02:03]")), Some(3723.0));
        assert_eq!(seconds(&json!("soon")), None);
    }
}

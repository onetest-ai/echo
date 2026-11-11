//! Threads at a glance and Done this week (Today's Tasks widgets, #315): each open thread's tasks as done this week /
//! to do / waiting, and the tasks done this week across them. Read from the threads' work (`ledger::fold_linked`); no
//! model call.

use chrono::{Datelike, Local, NaiveDate};
use serde_json::{json, Value};

use crate::ledger::{self, Node};
use crate::projects;

/// The day a task was done: its last change to done (a call's status, or the user's edit), when it is done now.
pub(crate) fn done_on(n: &Node) -> Option<NaiveDate> {
    if n.state != "done" {
        return None;
    }
    n.history
        .iter()
        .rev()
        .filter(|h| h["applied"] != false)
        // A call's status (done, or held: "the workshop happened") or the user's edit to done.
        .find(|h| h["effect"] == "done" || h["effect"] == "held" || h["set"]["state"] == "done" || h["op"] == "done")
        .and_then(|h| ["at", "ts", "date"].iter().find_map(|k| h[*k].as_str().filter(|s| s.len() >= 10)))
        .and_then(|s| NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d").ok())
}

/// Monday of `today`'s week.
fn week_start(today: NaiveDate) -> NaiveDate {
    today - chrono::Duration::days(i64::from(today.weekday().num_days_from_monday()))
}

/// Each open thread with tasks: done in the period (this week from Monday, or `month`: from the 1st), to do, waiting
/// (on someone, or blocked), the total; busiest first. And the tasks done in the period across them (`done`: their
/// titles, newest first, with their thread).
pub fn view(today: NaiveDate, period: &str) -> Value {
    let from = if period == "month" { today.with_day(1).unwrap_or(today) } else { week_start(today) };
    let mut threads = vec![];
    let mut done: Vec<(NaiveDate, Value)> = vec![];
    for p in projects::list().iter().filter(|p| !p.stage.closed()) {
        let Ok(t) = ledger::fold_linked(&p.id) else { continue };
        let (mut d, mut todo, mut waiting) = (0, 0, 0);
        // A mirror counts once, in its master's thread (as Tasks shows it), unless that thread is closed.
        let counted = |n: &&Node| n.kind == "task" && n.merged_into.is_none() && n.mirror_of.as_ref().is_none_or(|m| m["closed_thread"] == true);
        for n in t.nodes.values().filter(counted) {
            match n.state.as_str() {
                "done" => {
                    if let Some(day) = done_on(n).filter(|day| *day >= from && *day <= today) {
                        d += 1;
                        done.push((day, json!({"title": n.title, "project": p.id, "thread": p.name, "id": n.id, "on": day.to_string()})));
                    }
                }
                "blocked" => waiting += 1,
                "open" | "in_progress" if n.waiting_for.as_ref().is_some_and(|w| !w.is_null()) => waiting += 1,
                "open" | "in_progress" => todo += 1,
                _ => {}
            }
        }
        if d + todo + waiting > 0 {
            threads.push(json!({"id": p.id, "name": p.name, "done": d, "todo": todo, "waiting": waiting, "total": d + todo + waiting}));
        }
    }
    threads.sort_by_key(|t| std::cmp::Reverse(t["total"].as_u64().unwrap_or(0)));
    done.sort_by_key(|d| std::cmp::Reverse(d.0));
    json!({"since": from.to_string(), "period": if period == "month" { "month" } else { "week" }, "threads": threads, "done": done.into_iter().map(|(_, v)| v).collect::<Vec<_>>()})
}

/// Today's view for a period (`view` for the local date), read again at most every 30 s a period: Today asks for it on
/// each refresh, and each thread's work is folded for it.
pub fn today(period: &str) -> Value {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Vec<(String, Instant, Value)>> = Mutex::new(Vec::new());
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, _, v)) = last.iter().find(|(p, at, _)| p == period && at.elapsed() < Duration::from_secs(30)) {
        return v.clone();
    }
    let v = view(Local::now().date_naive(), period);
    last.retain(|(p, _, _)| p != period);
    last.push((period.to_string(), Instant::now(), v.clone()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t_node(project: &str, id: &str) -> Node {
        ledger::fold_linked(project).unwrap().nodes[id].clone()
    }

    #[test]
    fn a_thread_counts_done_this_week_to_do_and_waiting_and_done_lists_this_weeks() {
        let (_env, _) = crate::paths::testenv::folders("glance");
        let id = projects::create("Acme rollout", None, None).unwrap().id;
        ledger::ensure_first_epic(&ledger::Ledger::of(&id).unwrap()).unwrap();
        ledger::human_event(&id, &json!({"op": "create", "kind": "story", "parent": "E01", "title": "Launch"})).unwrap();
        let mk = |title: &str| ledger::human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": title})).unwrap()["id"].as_str().unwrap().to_string();
        let (a, b, c, _d) = (mk("Book the room"), mk("Send the deck"), mk("Map the groups"), mk("Write the FAQ"));
        ledger::human_event(&id, &json!({"op": "edit", "node": a, "set": {"state": "done"}})).unwrap();
        ledger::human_event(&id, &json!({"op": "edit", "node": b, "set": {"state": "blocked"}})).unwrap();
        ledger::human_event(&id, &json!({"op": "edit", "node": c, "set": {"waiting_for": {"who": "Dana", "what": "the list"}}})).unwrap();
        let today = Local::now().date_naive();
        let v = view(today, "week");
        let t = &v["threads"][0];
        assert_eq!((t["done"].as_u64(), t["todo"].as_u64(), t["waiting"].as_u64(), t["total"].as_u64()), (Some(1), Some(1), Some(2), Some(4)), "{v}");
        assert_eq!(v["done"][0]["title"], "Book the room");
        // Done because a call said it happened (held) counts as done this week too.
        let mut held = t_node(&id, &c);
        held.state = "done".into();
        held.history = vec![json!({"effect": "held", "date": today.to_string(), "applied": true})];
        assert_eq!(done_on(&held), Some(today));
        // This month counts from the 1st: today's done is in it.
        let m = view(today, "month");
        assert_eq!((m["period"].as_str(), m["done"].as_array().map(Vec::len)), (Some("month"), Some(1)));
        assert_eq!(m["since"].as_str().unwrap().get(8..), Some("01"));
        // Next week, that one was done last week: not counted, not listed.
        let v = view(today + chrono::Duration::days(7), "week");
        assert_eq!((v["threads"][0]["done"].as_u64(), v["done"].as_array().map(Vec::len)), (Some(0), Some(0)), "{v}");
    }
}

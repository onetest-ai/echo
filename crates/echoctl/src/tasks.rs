//! The Tasks activity: every open project's open tasks in four piles by who has to move — I owe ·
//! Owed to me · Waiting for others · Nobody named — and what to ask each person the next time you
//! meet. Computed from the ledgers (and the calendar for "next time you meet"), never a model's guess.

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::collections::BTreeMap;


use crate::ledger::{self, Node};
use crate::meeting::{MeetingContext, Person};
use crate::projects::{self, Project};

/// The piles, in the order the window shows them.
pub const PILES: [&str; 4] = ["mine", "owed", "waiting", "nobody"];
/// Asks: at most this many, and this many per story (taska's rule).
const ASKS_MAX: usize = 15;
const ASKS_PER_STORY: usize = 2;
/// An owed task with no word for this long is worth asking about.
const QUIET_DAYS: i64 = 5;
/// Or one due within this many days.
const DUE_SOON: i64 = 3;

/// Who "I" am in a ledger: the transcripts' "Me", and the user's name, first name and email.
pub struct Me(Vec<String>);

impl Me {
    pub fn of(p: Option<&Person>) -> Me {
        let mut v: Vec<String> = ["me", "i", "myself"].map(String::from).to_vec();
        if let Some(p) = p {
            let name = p.name.trim().to_lowercase();
            if !name.is_empty() {
                v.extend(name.split_whitespace().next().map(str::to_owned));
                v.push(name);
            }
            if let Some(e) = p.email.as_deref().map(str::to_lowercase) {
                v.extend(e.split('@').next().map(str::to_owned));
                v.push(e);
            }
        }
        Me(v)
    }
    pub fn is(&self, who: &str) -> bool {
        let w = who.trim().to_lowercase();
        !w.is_empty() && self.0.contains(&w)
    }
}

fn open(state: &str) -> bool {
    matches!(state, "open" | "in_progress" | "blocked")
}

fn days(from: &str, to: NaiveDate) -> Option<i64> {
    NaiveDate::parse_from_str(from.get(..10)?, "%Y-%m-%d").ok().map(|d| (to - d).num_days())
}

/// One open task as a card: where it is, who has it, when it's due, what it waits for, how long
/// nobody said a word about it.
fn card(p: &Project, n: &Node, nodes: &BTreeMap<String, Node>, ev: &BTreeMap<String, Value>, today: NaiveDate, me: &Me) -> (String, Value) {
    let story = n.parent.as_ref().and_then(|s| nodes.get(s)).filter(|s| s.kind == "story");
    // The client's owner first when the client owes it.
    let owner = if n.client_owed { n.client_owner.clone().or(n.internal_owner.clone()) } else { n.internal_owner.clone().or(n.client_owner.clone()) };
    let client = owner.is_some() && owner == n.client_owner && n.internal_owner.as_deref() != owner.as_deref();
    let last = n.history.iter().rev().find(|h| h["date"].is_string());
    let quiet = last.and_then(|h| days(h["date"].as_str()?, today));
    let last_call = n.history.iter().rev().filter_map(|h| ev.get(h["evidence"].as_str()?)).next();
    let after: Vec<Value> = n.waiting_on.iter().filter_map(|d| nodes.get(d)).map(|d| json!({"id": d.id, "title": d.title})).collect();
    let waiting = n.state == "blocked" || n.waiting_for.is_some() || !after.is_empty();
    let pile = if waiting {
        "waiting"
    } else if owner.as_deref().is_some_and(|o| me.is(o)) {
        "mine"
    } else if owner.is_some() {
        "owed"
    } else {
        "nobody"
    };
    let late = n.due.as_deref().and_then(|d| days(d, today)).is_some_and(|d| d > 0);
    (
        pile.into(),
        json!({
            "project": p.id, "id": n.id, "title": n.title, "type": n.task_type, "state": n.state,
            "story": story.map(|s| json!({"id": s.id, "title": s.title})),
            "owner": owner, "mine": owner.as_deref().is_some_and(|o| me.is(o)), "client": client,
            "due": n.due, "late": late, "waiting_for": n.waiting_for, "after": after,
            "quiet": quiet, "last": last_call.map(|e| json!({"date": e["date"], "call": e["call"], "title": e["title"]})),
        }),
    )
}

/// Overdue first, then the nearest date, then undated ones that went quiet longest.
fn rank(a: &Value, b: &Value) -> std::cmp::Ordering {
    let k = |x: &Value| (!x["late"].as_bool().unwrap_or(false), x["due"].as_str().unwrap_or("9999").to_owned(), -x["quiet"].as_i64().unwrap_or(0));
    k(a).cmp(&k(b))
}

/// A person's email from a project's people (by name, or a one-word name as the first name).
fn email_of(p: &Project, who: &str) -> Option<String> {
    let w = who.trim().to_lowercase();
    p.people.iter().find(|m| {
        let n = m.name.to_lowercase();
        n == w || (!w.contains(' ') && n.split_whitespace().next() == Some(w.as_str()))
    })?.email.clone()
}

fn at(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&chrono::Utc))
}

/// The first meeting from now on with this person in it.
fn next_meeting<'a>(meetings: &'a [MeetingContext], name: &str, email: Option<&str>, now: &str) -> Option<&'a MeetingContext> {
    let n = name.trim().to_lowercase();
    let one_word = !n.contains(' ');
    let hit = |a: &Person| {
        email.is_some_and(|e| a.email.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(e)))
            || a.name.to_lowercase() == n
            || (one_word && a.name.to_lowercase().split_whitespace().next() == Some(n.as_str()))
    };
    let mut ms: Vec<&MeetingContext> = meetings
        .iter()
        .filter(|m| m.end.as_deref().or(m.start.as_deref()).and_then(at).zip(at(now)).is_some_and(|(e, n)| e > n))
        .filter(|m| m.attendees.iter().chain(m.organizer.as_ref()).any(hit))
        .collect();
    ms.sort_by_key(|m| m.start.as_deref().and_then(at));
    ms.into_iter().next()
}

/// Everything the Tasks activity shows. `meetings`: the calendar from now on (for "next time you
/// meet"); `now`: RFC 3339.
pub fn across(meetings: &[MeetingContext], today: NaiveDate, now: &str) -> Result<Value> {
    let me = Me::of(crate::config::Config::load().ok().and_then(|c| c.me).as_ref());
    across_with(&projects::list(), &me, meetings, today, now)
}

fn across_with(list: &[Project], me: &Me, meetings: &[MeetingContext], today: NaiveDate, now: &str) -> Result<Value> {
    let mut piles: BTreeMap<String, Vec<Value>> = PILES.iter().map(|p| (p.to_string(), vec![])).collect();
    let mut shown = vec![];
    let mut errors = vec![];
    // Asks: per person, the items to raise, with the project they're in.
    let mut asks: Vec<(String, Option<String>, Value)> = vec![];
    for p in list.iter().filter(|p| !p.stage.closed()) {
        let t = match ledger::Ledger::of(&p.id).and_then(|l| ledger::fold(&l)) {
            Ok(t) => t,
            Err(e) => {
                errors.push(json!({"project": p.id, "error": format!("{e:#}")}));
                continue;
            }
        };
        let mut count = 0;
        for n in t.nodes.values().filter(|n| n.kind == "task" && open(&n.state) && n.merged_into.is_none()) {
            let (pile, c) = card(p, n, &t.nodes, &t.evidence, today, me);
            count += 1;
            // Something to ask: what they owe that is late, due soon or quiet; what we wait on them for.
            let due_in = c["due"].as_str().and_then(|d| days(d, today)).map(|d| -d);
            let ask = match pile.as_str() {
                "owed" => {
                    let why = if c["late"] == true {
                        Some("late")
                    } else if due_in.is_some_and(|d| d <= DUE_SOON) {
                        Some("due")
                    } else if c["quiet"].as_i64().is_some_and(|q| q >= QUIET_DAYS) {
                        Some("quiet")
                    } else {
                        None
                    };
                    why.map(|w| (c["owner"].as_str().unwrap_or_default().to_owned(), w))
                }
                "waiting" => n.waiting_for.as_ref().and_then(|w| w["who"].as_str()).filter(|w| !me.is(w)).map(|w| (w.to_owned(), "waiting")),
                _ => None,
            };
            if let Some((who, why)) = ask {
                let email = email_of(p, &who);
                let mut a = c.clone();
                a["why"] = json!(why);
                asks.push((who, email, a));
            }
            if let Some(v) = piles.get_mut(&pile) {
                v.push(c);
            }
        }
        shown.push(json!({"id": p.id, "name": p.name, "tasks": count}));
    }
    for v in piles.values_mut() {
        v.sort_by(rank);
    }
    // Most urgent first; at most two per story, fifteen in all; then grouped by person.
    asks.sort_by(|a, b| rank(&a.2, &b.2));
    let mut per_story: BTreeMap<String, usize> = BTreeMap::new();
    let mut people: Vec<(String, Option<String>, Vec<Value>)> = vec![];
    for (who, email, a) in asks {
        let story = format!("{}/{}", a["project"].as_str().unwrap_or(""), a["story"]["id"].as_str().unwrap_or(a["id"].as_str().unwrap_or("")));
        let n = per_story.entry(story).or_default();
        if *n >= ASKS_PER_STORY || people.iter().map(|p| p.2.len()).sum::<usize>() >= ASKS_MAX {
            continue;
        }
        *n += 1;
        match people.iter_mut().find(|p| p.0.to_lowercase() == who.to_lowercase()) {
            Some(p) => {
                p.1 = p.1.take().or(email);
                p.2.push(a);
            }
            None => people.push((who, email, vec![a])),
        }
    }
    let asks: Vec<Value> = people
        .into_iter()
        .map(|(who, email, items)| {
            let m = next_meeting(meetings, &who, email.as_deref(), now);
            json!({"who": who, "email": email, "items": items,
                   "next": m.map(|m| json!({"subject": m.subject, "start": m.start, "people": m.attendees.len()}))})
        })
        .collect();
    Ok(json!({"piles": piles, "projects": shown, "asks": asks, "errors": errors}))
}

/// A client's page: its description, each of its threads with a summary of its work (open tasks
/// by pile, late ones; the latest decisions; open risks and questions), every call of its threads
/// (newest first, with the threads it is in), and the Tasks piles for its open threads.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn client(id: &str, meetings: &[MeetingContext], today: NaiveDate, now: &str) -> Result<Value> {
    let c = projects::clients()?.into_iter().find(|c| c.id == id).ok_or_else(|| anyhow::anyhow!("no client {id:?}"))?;
    let brief = projects::client_brief(id)?;
    let mine: Vec<Project> = projects::list().into_iter().filter(|p| p.client.as_deref() == Some(id)).collect();
    let me = Me::of(crate::config::Config::load().ok().and_then(|c| c.me).as_ref());
    let work = across_with(&mine, &me, meetings, today, now)?;
    let mut calls: BTreeMap<String, (Value, Vec<String>)> = BTreeMap::new();
    let mut threads = vec![];
    for p in &mine {
        let mut v = projects::view(p, false)?;
        for (d, why) in projects::calls_of(p)? {
            let row = projects::call_row(&d, why);
            let k = row["name"].as_str().unwrap_or("").to_owned();
            calls.entry(k).or_insert_with(|| (row, vec![])).1.push(p.id.clone());
        }
        let mut piles = serde_json::Map::new();
        let mut late = 0;
        for k in PILES {
            let n: Vec<&Value> = work["piles"][k].as_array().into_iter().flatten().filter(|c| c["project"] == p.id.as_str()).collect();
            late += n.iter().filter(|c| c["late"] == true).count();
            piles.insert(k.into(), json!(n.len()));
        }
        let (mut decisions, mut risks, mut questions, mut stories) = (vec![], vec![], vec![], (0, 0));
        if let Ok(t) = ledger::Ledger::of(&p.id).and_then(|l| ledger::fold(&l)) {
            let rec = |n: &Node| json!({"id": n.id, "title": n.title, "date": n.created});
            let mut ds: Vec<&Node> = t.nodes.values().filter(|n| n.kind == "decision" && n.state != "merged" && n.state != "deleted").collect();
            ds.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
            decisions = ds.into_iter().take(3).map(rec).collect();
            risks = t.nodes.values().filter(|n| n.kind == "risk" && open(&n.state)).map(rec).collect();
            questions = t.nodes.values().filter(|n| n.kind == "question" && open(&n.state)).map(rec).collect();
            let st: Vec<&Node> = t.nodes.values().filter(|n| n.kind == "story" && n.state != "merged" && n.state != "deleted" && n.state != "cancelled").collect();
            stories = (st.iter().filter(|s| s.state == "done").count(), st.len());
        }
        v["summary"] = json!({"piles": piles, "late": late, "decisions": decisions, "risks": risks, "questions": questions,
                              "stories_done": stories.0, "stories": stories.1});
        threads.push(v);
    }
    let mut calls: Vec<Value> = calls.into_values().map(|(mut r, ps)| {
        r["threads"] = json!(ps);
        r
    }).collect();
    calls.sort_by(|a, b| b["started_at"].as_str().unwrap_or("").cmp(a["started_at"].as_str().unwrap_or("")));
    Ok(json!({"client": c, "brief": brief, "brief_html": crate::minutes::markdown_html(&brief), "threads": threads, "calls": calls, "work": work}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn me_is_the_speaker_label_my_name_and_email() {
        let me = Me::of(Some(&Person { name: "Artem Rozumenko".into(), email: Some("artem@example.com".into()) }));
        assert!(me.is("Me") && me.is("artem") && me.is("Artem Rozumenko") && me.is("artem@example.com"));
        assert!(!me.is("Dana Kowal") && !me.is(""));
        assert!(Me::of(None).is("me"));
    }

    #[test]
    fn tasks_fall_into_piles_and_asks() {
        let (_env, _root) = crate::paths::testenv::folders("tasks-across");
        let p = projects::create("Aurora", None, None).unwrap();
        projects::update(&p.id, |p| {
            p.people = vec![projects::Member { name: "Dana Kowal".into(), email: Some("dana@northwind.example".into()), role: None, ..Default::default() }];
            Ok(())
        })
        .unwrap();
        let l = ledger::Ledger::of(&p.id).unwrap();
        std::fs::create_dir_all(l.dir.join("evidence")).unwrap();
        let item = json!({"id": "2026-10-20_110000", "kind": "meeting", "source": "call", "at": "2026-10-20T11:00:00+02:00", "date": "2026-10-20", "title": "Aurora weekly",
            "turns": [
                {"ts": "00:01:00", "speaker": "Me", "text": "I can have the SAML config ready by Friday."},
                {"ts": "00:02:00", "speaker": "Dana Kowal", "text": "I will send the pilot user list on Monday."},
                {"ts": "00:03:00", "speaker": "Ravi Mehta", "text": "Somebody has to run the load test for ingest at some point."},
                {"ts": "00:04:00", "speaker": "Ravi Mehta", "text": "The security review needs the data-flow diagram first."},
            ], "text": ""});
        std::fs::write(l.dir.join("evidence/2026-10-20_110000.json"), serde_json::to_vec(&item).unwrap()).unwrap();
        ledger::ensure_first_epic(&l).unwrap();
        let ev = "2026-10-20_110000";
        let r = ledger::apply(&p.id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "Pilot", "ac": ["Users can sign in", "Load holds"], "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "due_text": "Friday", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Pilot user list", "type": "dependency", "client_owed": true, "client_owner": "Dana Kowal", "due_text": "Monday", "quote": "I will send the pilot user list on Monday"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Load test for ingest", "type": "session", "quote": "Somebody has to run the load test for ingest"}),
        ]).unwrap();
        assert_eq!(r["events"], 4, "{r}");
        ledger::mark_processed(&p.id, ev, "").ok();
        let meetings = vec![MeetingContext {
            subject: Some("Aurora weekly".into()),
            start: Some("2026-10-29T09:00:00Z".into()),
            end: Some("2026-10-29T09:30:00Z".into()),
            attendees: vec![Person { name: "Dana Kowal".into(), email: Some("DANA@northwind.example".into()) }],
            ..Default::default()
        }];
        let me = Me::of(None);
        let v = across_with(&projects::list(), &me, &meetings, NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(), "2026-10-28T12:00:00Z").unwrap();
        let titles = |k: &str| v["piles"][k].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        assert_eq!(titles("mine"), ["SAML config"]);
        assert_eq!(titles("owed"), ["Pilot user list"]);
        assert_eq!(titles("nobody"), ["Load test for ingest"]);
        let pl = &v["piles"]["owed"][0];
        assert_eq!(pl["client"], true);
        assert_eq!(pl["late"], true, "Monday the 26th is before the 28th");
        assert_eq!(pl["quiet"], 8);
        assert_eq!(v["asks"][0]["who"], "Dana Kowal");
        assert_eq!(v["asks"][0]["items"][0]["why"], "late");
        assert_eq!(v["asks"][0]["next"]["subject"], "Aurora weekly", "matched by email, any case");
        assert_eq!(v["projects"][0]["tasks"], 3);
    }

    #[test]
    fn a_client_page_sums_up_its_threads() {
        let (_env, _root) = crate::paths::testenv::folders("tasks-client");
        projects::add_client("Northwind", &["northwind.example".into()], false).unwrap();
        let p = projects::create("Aurora", Some("northwind"), None).unwrap();
        projects::create("Elsewhere", None, None).unwrap();
        projects::set_client_brief("northwind", "Retail group; **Dana** runs IT.").unwrap();
        assert!(projects::set_client_brief("nobody", "x").is_err());
        let v = client("northwind", &[], NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(), "2026-10-28T12:00:00Z").unwrap();
        assert_eq!(v["brief"], "Retail group; **Dana** runs IT.");
        assert!(v["brief_html"].as_str().unwrap().contains("<strong>Dana</strong>"));
        assert_eq!(v["threads"].as_array().unwrap().len(), 1, "only its own threads");
        assert_eq!(v["threads"][0]["id"], p.id.as_str());
        assert_eq!(v["threads"][0]["summary"]["piles"]["mine"], 0);
        assert!(client("nobody", &[], NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(), "x").is_err());
    }

    #[test]
    fn ranking_puts_late_then_nearest_then_quiet() {
        let mut v = [
            json!({"title": "a", "due": null, "quiet": 2, "late": false}),
            json!({"title": "b", "due": "2026-11-05", "quiet": 0, "late": false}),
            json!({"title": "c", "due": "2026-10-01", "quiet": 0, "late": true}),
            json!({"title": "d", "due": null, "quiet": 9, "late": false}),
        ];
        v.sort_by(rank);
        assert_eq!(v.iter().map(|x| x["title"].as_str().unwrap()).collect::<String>(), "cbda");
    }
}


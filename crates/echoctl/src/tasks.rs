//! The Tasks activity: every open project's open tasks in piles by the user's rules (2026-11-02) —
//! I owe (what I do), Owed to me (what I handed over and check), Waiting for others (what stops me
//! doing what I owe: the work and the people my tasks wait on), Nobody named, and Others (other
//! people's work I neither delegated nor wait on) — and what to ask each person the next time you
//! meet. Computed from the ledgers (and the calendar for "next time you meet"), never a model's guess.

use anyhow::Result;
use chrono::NaiveDate;
use serde_json::{json, Value};
use std::collections::BTreeMap;


use crate::ledger::{self, Node};
use crate::meeting::{MeetingContext, Person};
use crate::projects::{self, Project};

/// The piles, in the order the window shows them.
pub const PILES: [&str; 5] = ["mine", "owed", "waiting", "nobody", "others"];
/// Asks: at most this many, and this many per story (taska's rule).
const ASKS_MAX: usize = 15;
const ASKS_PER_STORY: usize = 2;
/// An owed task with no word for this long is worth asking about.
const QUIET_DAYS: i64 = 5;
/// Mail sent from a task with no reply for this long: it waits for that reply (Waiting for others, #92).
pub const MAIL_REPLY_DAYS: i64 = 3;
/// Or one due within this many days.
const DUE_SOON: i64 = 3;

/// Whose a task is: the client's owner first when the client owes it, else ours (the Tasks piles, What moved).
pub fn owner_of(n: &Node) -> Option<&String> {
    if n.client_owed { n.client_owner.as_ref().or(n.internal_owner.as_ref()) } else { n.internal_owner.as_ref().or(n.client_owner.as_ref()) }
}

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

/// The names the user goes by among a ledger's owners ("Me", their name…), sorted: the Work tab's
/// Owner filter shows them as one entry.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn my_names(me: &Me, nodes: &Value) -> Vec<String> {
    let mut v: Vec<String> = nodes.as_object().into_iter().flat_map(|m| m.values())
        .flat_map(|n| [&n["internal_owner"], &n["client_owner"]])
        .filter_map(|o| o.as_str()).filter(|o| me.is(o)).map(str::to_owned).collect();
    v.sort();
    v.dedup();
    v
}

fn open(state: &str) -> bool {
    matches!(state, "open" | "in_progress" | "blocked")
}

fn days(from: &str, to: NaiveDate) -> Option<i64> {
    NaiveDate::parse_from_str(from.get(..10)?, "%Y-%m-%d").ok().map(|d| (to - d).num_days())
}

/// One open task as a card: where it is, who has it, when it's due, what it waits for, how long
/// nobody said a word about it.
#[allow(clippy::too_many_arguments)]
fn card(p: &Project, n: &Node, nodes: &BTreeMap<String, Node>, ev: &BTreeMap<String, Value>, today: NaiveDate, me: &Me, modes: &serde_json::Map<String, Value>) -> (String, Value) {
    let story = n.parent.as_ref().and_then(|s| nodes.get(s)).filter(|s| s.kind == "story");
    let owner = owner_of(n).cloned();
    let client = owner.is_some() && owner == n.client_owner && n.internal_owner.as_deref() != owner.as_deref();
    let last = n.history.iter().rev().find(|h| h["date"].is_string());
    let quiet = last.and_then(|h| days(h["date"].as_str()?, today));
    let last_call = n.history.iter().rev().filter_map(|h| ev.get(h["evidence"].as_str()?)).next();
    let mut after: Vec<Value> = n.waiting_on.iter().filter_map(|d| nodes.get(d)).filter(|d| open(&d.state)).map(|d| json!({"id": d.id, "title": d.title})).collect();
    // Open work in other threads it waits on (#20).
    after.extend(n.xwaits.iter().map(|w| json!({"id": w["node"], "title": w["title"], "thread": w["thread"]})));
    let blocked = n.state == "blocked" || n.waiting_for.as_ref().is_some_and(|w| w["what"].is_string() || w["who"].is_string()) || !after.is_empty();
    // Who handed it over: as recorded, else whoever's words created it. Not "made by hand": a session
    // filing the user's mail makes tasks by hand that nobody handed over (2026-11-02).
    let delegated = n.delegated_by.clone().or(n.creator.clone());
    let pile = if owner.as_deref().is_some_and(|o| me.is(o)) {
        "mine"
    } else if owner.is_none() {
        "nobody"
    } else if delegated.as_deref().is_some_and(|d| me.is(d)) {
        "owed"
    } else {
        "others"
    };
    // A guessed due (#463) is never late; where it came from says why it is there.
    let late = !n.due_guessed && n.due.as_deref().and_then(|d| days(d, today)).is_some_and(|d| d > 0);
    let due_from = n.due_guessed.then(|| n.due_history.last().and_then(|h| h["evidence"].as_str()).and_then(|i| ev.get(i)).map(|e| json!({"kind": e["kind"], "date": e["date"]}))).flatten();
    (
        pile.into(),
        json!({
            "project": p.id, "id": n.id, "title": n.title, "type": n.task_type, "state": n.state,
            "story": story.map(|s| json!({"id": s.id, "title": s.title})),
            "owner": owner, "mine": owner.as_deref().is_some_and(|o| me.is(o)), "client": client, "delegated_by": delegated,
            "due": n.due, "late": late, "due_guessed": n.due_guessed, "due_from": due_from, "looks_done": n.looks_done,
            "waiting_for": n.waiting_for, "after": after, "blocked": blocked,
            "quiet": quiet, "last": last_call.map(|e| json!({"date": e["date"], "call": e["call"], "title": e["title"]})),
            // One piece of work raised in several threads (#20): this card stands for all of them.
            "also_in": n.also_in,
            // Who does it: the user's choice, else Echo's guess (`taskmodes`), else null.
            "mode": if pile == "mine" { crate::taskmodes::of(modes, &p.id, n) } else { Value::Null },
        }),
    )
}

/// A task's sent mails with no reply yet, sent at least `MAIL_REPLY_DAYS` before `now` (RFC 3339).
fn unanswered<'a>(n: &'a Node, now: &str) -> Vec<&'a Value> {
    let Some(now) = at(now) else { return vec![] };
    n.sent.iter().filter(|m| m["reply_at"].is_null() && m["at"].as_str().and_then(at).is_some_and(|t| now - t >= chrono::Duration::days(MAIL_REPLY_DAYS))).collect()
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
    let modes = crate::taskmodes::guesses();
    for p in list.iter().filter(|p| !p.stage.closed()) {
        let t = match ledger::fold_linked(&p.id) {
            Ok(t) => t,
            Err(e) => {
                errors.push(json!({"project": p.id, "error": format!("{e:#}")}));
                continue;
            }
        };
        // A mirror (#20) is shown once, as its master's card in the master's thread.
        let tasks: Vec<&Node> = t.nodes.values().filter(|n| n.kind == "task" && open(&n.state) && n.merged_into.is_none() && n.mirror_of.as_ref().is_none_or(|m| m["closed_thread"] == true)).collect();
        let mut cards: BTreeMap<String, (String, Value)> = tasks.iter().map(|n| (n.id.clone(), card(p, n, &t.nodes, &t.evidence, today, me, &modes))).collect();
        // Waiting for others = what stops me doing what I owe: the open work my tasks wait on (not
        // mine; it leaves Others / Nobody named for here) and the people they wait on (a card each).
        let mut waits: Vec<Value> = vec![];
        for n in &tasks {
            if cards[&n.id].0 != "mine" {
                continue;
            }
            let mine = json!({"id": n.id, "title": n.title});
            for d in &n.waiting_on {
                if let Some((pile, c)) = cards.get_mut(d) {
                    if pile == "mine" {
                        continue;
                    }
                    if pile != "owed" {
                        *pile = "waiting".into();
                    }
                    if let Some(b) = c["blocks"].as_array_mut() {
                        b.push(mine.clone());
                    } else {
                        c["blocks"] = json!([mine]);
                    }
                }
            }
            // What it waits on in another thread (#20): that work, a card here.
            for w in &n.xwaits {
                let owner = w["owner"].as_str();
                if owner.is_some_and(|o| me.is(o)) {
                    continue;
                }
                // Several of my tasks waiting on it: one card, every one of them in what it stops.
                if let Some(c) = waits.iter_mut().find(|c| c["project"] == w["thread"] && c["id"] == w["node"]) {
                    if let Some(b) = c["blocks"].as_array_mut() {
                        b.push(mine.clone());
                    }
                    continue;
                }
                let c = &cards[&n.id].1;
                waits.push(json!({"project": w["thread"], "id": w["node"], "title": w["title"], "owner": w["owner"], "state": w["state"], "other_thread": true,
                                  "after": [], "blocks": [mine], "due": null, "late": false, "quiet": c["quiet"], "story": null, "last": null}));
            }
            if let Some(w) = n.waiting_for.as_ref().filter(|w| w["what"].is_string() || w["who"].is_string()) {
                if w["who"].as_str().is_some_and(|x| me.is(x)) {
                    continue;
                }
                let c = &cards[&n.id].1;
                waits.push(json!({"project": p.id, "id": n.id, "wait": true, "title": w["what"].as_str().unwrap_or("an answer"), "who": w["who"], "owner": w["who"],
                                  "waiting_for": w, "after": [], "blocks": [mine], "due": null, "late": false, "quiet": c["quiet"], "story": c["story"], "last": c["last"]}));
            }
        }
        // Mail the user sent from a task, unanswered for `MAIL_REPLY_DAYS`: they wait for that reply, whoever's task it is.
        for n in &tasks {
            for m in unanswered(n, now) {
                let who = m["to"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).filter(|w| !w.is_empty());
                let c = &cards[&n.id].1;
                waits.push(json!({"project": p.id, "id": n.id, "wait": true, "mail": m, "title": format!("a reply to “{}”", m["subject"].as_str().unwrap_or("your mail")),
                                  "who": who, "owner": who, "waiting_for": {"who": who, "what": "a reply to your mail", "since": m["at"]},
                                  "after": [], "blocks": [{"id": n.id, "title": n.title}], "due": null, "late": false, "quiet": c["quiet"], "story": c["story"], "last": c["last"]}));
            }
        }
        // An owed task my work waits on is in both: I check it, and it stops me.
        let owed_blockers: Vec<Value> = cards.values().filter(|(pile, c)| pile == "owed" && c["blocks"].is_array()).map(|(_, c)| {
            let mut c = c.clone();
            c["also"] = json!("owed");
            c
        }).collect();
        let count = tasks.len();
        for (pile, c) in cards.into_values().chain(owed_blockers.into_iter().map(|c| ("waiting".to_string(), c))).chain(waits.into_iter().map(|c| ("waiting".to_string(), c))) {
            // Something to ask: what they owe me that is late, due soon or quiet; what my work waits on them for.
            let due_in = c["due"].as_str().filter(|_| c["due_guessed"] != true).and_then(|d| days(d, today)).map(|d| -d);
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
                "waiting" if c["owner"].as_str().is_some_and(|o| !me.is(o)) => {
                    Some((c["owner"].as_str().unwrap_or_default().to_owned(), "waiting"))
                }
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
    // Work of another thread my tasks wait on (#20) is Waiting for others, not Others / Nobody named there.
    let waited: Vec<(Value, Value)> = piles["waiting"].iter().filter(|c| c["other_thread"] == true).map(|c| (c["project"].clone(), c["id"].clone())).collect();
    for k in ["others", "nobody"] {
        if let Some(v) = piles.get_mut(k) {
            v.retain(|c| !waited.iter().any(|(p, i)| c["project"] == *p && c["id"] == *i));
        }
    }
    for v in piles.values_mut() {
        v.sort_by(rank);
    }
    // Most urgent first; at most two per story, fifteen in all; then grouped by person.
    asks.sort_by(|a, b| rank(&a.2, &b.2));
    // An owed task that also stops me is asked about once.
    let mut asked = std::collections::BTreeSet::new();
    asks.retain(|a| a.2["wait"] == true || asked.insert(format!("{}/{}", a.2["project"], a.2["id"])));
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
            // Tasks, each once: not a person waited on, not an owed task shown again as a blocker.
            let n: Vec<&Value> = work["piles"][k].as_array().into_iter().flatten().filter(|c| c["project"] == p.id.as_str() && c["wait"] != true && c["also"].is_null()).collect();
            late += n.iter().filter(|c| c["late"] == true).count();
            piles.insert(k.into(), json!(n.len()));
        }
        let (mut decisions, mut risks, mut questions, mut stories) = (vec![], vec![], vec![], (0, 0));
        if let Ok(t) = ledger::Ledger::of(&p.id).and_then(|l| ledger::fold(&l)) {
            // A record with what it holds: a decision's content and who took it, a risk's response.
            let rec = |n: &Node| {
                let what = n.description.as_deref().and_then(|d| d.lines().map(str::trim).find(|l| !l.is_empty())).map(|l| l.chars().take(160).collect::<String>());
                json!({"id": n.id, "title": n.title, "date": n.created, "what": what, "by": n.decided_by.as_ref().or(n.internal_owner.as_ref()),
                       "likelihood": n.likelihood, "impact": n.impact})
            };
            let mut ds: Vec<&Node> = t.nodes.values().filter(|n| n.kind == "decision" && n.state != "merged" && n.state != "deleted").collect();
            ds.sort_by(|a, b| b.created.cmp(&a.created).then(b.id.cmp(&a.id)));
            decisions = ds.into_iter().take(3).map(rec).collect();
            // Open risks with no response first: they are the ones nobody handles.
            let mut rs: Vec<&Node> = t.nodes.values().filter(|n| n.kind == "risk" && ledger::record_open(n)).collect();
            rs.sort_by_key(|n| n.description.is_some());
            risks = rs.into_iter().map(rec).collect();
            questions = t.nodes.values().filter(|n| n.kind == "question" && ledger::record_open(n)).map(rec).collect();
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
        let me = Me::of(Some(&Person { name: "Alex Morgan".into(), email: Some("alex@example.com".into()) }));
        assert!(me.is("Me") && me.is("alex") && me.is("Alex Morgan") && me.is("alex@example.com"));
        assert!(!me.is("Dana Kowal") && !me.is(""));
        assert!(Me::of(None).is("me"));
    }

    #[test]
    fn my_names_are_the_owners_of_a_ledger_that_are_me() {
        // #114: the Work tab's Owner filter lists the user once, whatever name an item was recorded under.
        let me = Me::of(Some(&Person { name: "Alex Morgan".into(), email: None }));
        let nodes = json!({
            "T001": {"internal_owner": "Me"}, "T002": {"internal_owner": "Alex Morgan"}, "T003": {"client_owner": "Dana Kowal"},
            "T004": {"internal_owner": "Alex Morgan"}, "T005": {}
        });
        assert_eq!(my_names(&me, &nodes), vec!["Alex Morgan".to_string(), "Me".to_string()]);
        assert!(my_names(&me, &json!({})).is_empty());
    }

    #[test]
    fn a_mail_sent_from_a_task_waits_for_its_reply_after_three_days() {
        let (_env, _root) = crate::paths::testenv::folders("tasks-mail-sent");
        let id = projects::create("Acme platform", None, None).unwrap().id;
        let l = ledger::Ledger::of(&id).unwrap();
        ledger::ensure_first_epic(&l).unwrap();
        ledger::human_event(&id, &json!({"op": "create", "kind": "story", "parent": "E01", "title": "Cut-over"})).unwrap();
        let t = ledger::human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "Agree the cut-over date", "internal_owner": "Me"})).unwrap()["id"].as_str().unwrap().to_string();
        ledger::mail_sent(&id, &t, "conv-1", "Cut-over date", &["dana@acme.example".into()]).unwrap();
        let me = Me::of(None);
        let today = chrono::Local::now().date_naive();
        let waiting = |days: i64| {
            let now = (chrono::Local::now() + chrono::Duration::days(days) + chrono::Duration::minutes(1)).to_rfc3339();
            across_with(&projects::list(), &me, &[], today, &now).unwrap()["piles"]["waiting"].as_array().unwrap().iter().filter(|c| c["mail"].is_object()).cloned().collect::<Vec<Value>>()
        };
        assert!(waiting(2).is_empty(), "two days: not yet");
        let w = waiting(MAIL_REPLY_DAYS);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!((w[0]["id"].as_str(), w[0]["title"].as_str(), w[0]["who"].as_str()), (Some(t.as_str()), Some("a reply to “Cut-over date”"), Some("dana@acme.example")));
        // Answered: it waits no more.
        std::fs::create_dir_all(l.dir.join("evidence")).unwrap();
        let reply = json!({"id": "mail-r", "kind": "mail", "conversation": "conv-1", "at": (chrono::Local::now() + chrono::Duration::hours(1)).to_rfc3339(), "date": "2026-11-11",
                           "title": "RE: Cut-over date", "turns": [{"ts": "10:00:00", "speaker": "Dana Doe", "text": "The 24th works."}]});
        std::fs::write(l.dir.join("evidence/mail-r.json"), reply.to_string()).unwrap();
        assert!(waiting(MAIL_REPLY_DAYS).is_empty());
    }

    #[test]
    fn one_piece_of_work_in_two_threads() {
        // #20's case, with fictional threads: a practice thread hears the pitch its pursuit thread already has.
        let (_env, _root) = crate::paths::testenv::folders("tasks-xref");
        let practice = projects::create("Acme practice", None, None).unwrap().id;
        let pursuit = projects::create("Acme pursuit", None, None).unwrap().id;
        for id in [&practice, &pursuit] {
            ledger::ensure_first_epic(&ledger::Ledger::of(id).unwrap()).unwrap();
            ledger::human_event(id, &json!({"op": "create", "kind": "story", "parent": "E01", "title": "Proposal"})).unwrap();
        }
        let mk = |id: &str, title: &str| ledger::human_event(id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": title, "internal_owner": "Me"})).unwrap()["id"].as_str().unwrap().to_string();
        let pitch = mk(&practice, "Pitch the reworked Acme proposal to the CIO");
        let demo = mk(&pursuit, "Acme demo, Fri");
        let waits = mk(&practice, "Send the deck to the CIO's office");
        let prep = mk(&pursuit, "Prepare the demo data");
        ledger::human_event(&pursuit, &json!({"op": "edit", "node": prep, "set": {"internal_owner": "Sam Ortiz"}})).unwrap();
        // Found by words of its title in the other threads, not this one.
        let found = ledger::search_open("acme demo", Some(&practice));
        assert_eq!(found.iter().map(|x| (x["thread"].as_str().unwrap(), x["id"].as_str().unwrap())).collect::<Vec<_>>(), vec![(pursuit.as_str(), demo.as_str())]);
        assert!(ledger::search_open("pitch", Some(&practice)).is_empty(), "this thread's own items aren't offered");
        // 1. The same work, its master in the pursuit thread.
        let link = ledger::human_event(&practice, &json!({"op": "xref", "node": pitch, "thread": pursuit, "target": demo, "kind": "same", "master": "there"})).unwrap();
        // An edit of the mirror's owner, due or state is the master's: made there.
        ledger::human_event(&practice, &json!({"op": "edit", "node": pitch, "set": {"internal_owner": "Lee Park", "title": "Pitch to the CIO"}})).unwrap();
        assert_eq!(ledger::fold_linked(&pursuit).unwrap().nodes[&demo].internal_owner.as_deref(), Some("Lee Park"));
        assert_eq!(ledger::fold_linked(&practice).unwrap().nodes[&pitch].title, "Pitch to the CIO", "its title stays its own");
        // 2. The master changes; the mirror shows it, and Tasks shows the work once, with both threads.
        ledger::human_event(&pursuit, &json!({"op": "edit", "node": demo, "set": {"internal_owner": "Dana Doe", "due": "2026-11-06"}})).unwrap();
        let t = ledger::fold_linked(&practice).unwrap();
        let m = &t.nodes[&pitch];
        assert_eq!((m.internal_owner.as_deref(), m.due.as_deref()), (Some("Dana Doe"), Some("2026-11-06")));
        assert_eq!(m.mirror_of.as_ref().unwrap()["node"], demo.as_str());
        let me = Me::of(None);
        let list = projects::list();
        let today = NaiveDate::from_ymd_opt(2026, 11, 3).unwrap();
        let work = across_with(&list, &me, &[], today, "2026-11-03T09:00:00Z").unwrap();
        let cards: Vec<Value> = PILES.iter().flat_map(|k| work["piles"][*k].as_array().cloned().unwrap_or_default()).filter(|c| c["title"].as_str().is_some_and(|t| t.contains("Acme demo") || t.contains("Pitch"))).collect();
        assert_eq!(cards.len(), 1, "one card: {cards:?}");
        assert_eq!(cards[0]["also_in"][0]["thread"], practice.as_str());
        ledger::human_event(&pursuit, &json!({"op": "edit", "node": demo, "set": {"state": "done"}})).unwrap();
        assert_eq!(ledger::fold_linked(&practice).unwrap().nodes[&pitch].state, "done");
        // 3. Undo the link (from either side): both are on their own again, with their own last values.
        ledger::human_event(&practice, &json!({"op": "undo", "target": link["mid"]})).unwrap();
        let t = ledger::fold_linked(&practice).unwrap();
        assert!(t.nodes[&pitch].mirror_of.is_none() && t.nodes[&pitch].state == "open" && t.nodes[&pitch].internal_owner.as_deref() == Some("Me"));
        assert!(ledger::fold_linked(&pursuit).unwrap().nodes[&demo].xrefs.is_empty(), "taken away on both sides");
        // Undoing the unlink links them again, as they were.
        let unlink = ledger::Ledger::of(&practice).unwrap().events().unwrap().into_iter().rev().find(|e| e["op"] == "unxref").unwrap();
        ledger::human_event(&practice, &json!({"op": "undo", "target": unlink["mid"]})).unwrap_err(); // the pitch's master is done: can't link a closed item
        // 4. Across threads, a wait: my task waits on someone's open task in the other thread.
        ledger::human_event(&practice, &json!({"op": "xref", "node": waits, "thread": pursuit, "target": prep, "kind": "depends_on"})).unwrap();
        let w = &ledger::fold_linked(&practice).unwrap().nodes[&waits];
        assert_eq!(w.xwaits[0]["node"], prep.as_str());
        let work = across_with(&projects::list(), &me, &[], today, "2026-11-03T09:00:00Z").unwrap();
        assert!(work["piles"]["waiting"].as_array().unwrap().iter().any(|c| c["id"] == prep.as_str() && c["project"] == pursuit.as_str()), "{}", work["piles"]["waiting"]);
        // Unlinked, then the unlink undone: the wait is back, as it was.
        let u = ledger::human_event(&practice, &json!({"op": "unxref", "node": waits, "thread": pursuit, "target": prep})).unwrap();
        assert!(ledger::fold_linked(&practice).unwrap().nodes[&waits].xwaits.is_empty());
        ledger::human_event(&practice, &json!({"op": "undo", "target": u["mid"]})).unwrap();
        assert_eq!(ledger::fold_linked(&practice).unwrap().nodes[&waits].xwaits.len(), 1);
        // Same thread, unknown item, closed item: refused, nothing written.
        assert!(ledger::human_event(&practice, &json!({"op": "xref", "node": waits, "thread": practice, "target": pitch, "kind": "same"})).is_err());
        assert!(ledger::human_event(&practice, &json!({"op": "xref", "node": waits, "thread": pursuit, "target": "T999", "kind": "same"})).is_err());
        assert!(ledger::human_event(&practice, &json!({"op": "xref", "node": waits, "thread": pursuit, "target": demo, "kind": "same"})).is_err(), "the demo is done");
    }

    #[test]
    fn tasks_fall_into_piles_and_asks() {
        let (_env, _root) = crate::paths::testenv::folders("tasks-across");
        projects::add_client("Northwind", &["northwind.example".into()], false).unwrap();
        let p = projects::create("Aurora", Some("northwind"), None).unwrap();
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
                {"ts": "00:04:00", "speaker": "Ravi Mehta", "text": "I will run the security review of the vendor contract next week."},
                {"ts": "00:05:00", "speaker": "Me", "text": "Dana, please draft the rollout email for the pilot users by Thursday."},
                {"ts": "00:06:00", "speaker": "Me", "text": "The SAML config can't start until Sam sends us the signing certificate."},
            ], "text": ""});
        std::fs::write(l.dir.join("evidence/2026-10-20_110000.json"), serde_json::to_vec(&item).unwrap()).unwrap();
        ledger::ensure_first_epic(&l).unwrap();
        let ev = "2026-10-20_110000";
        let r = ledger::apply(&p.id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "Pilot", "ac": ["Users can sign in", "Load holds"], "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "ref": "t1", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "due_text": "Friday", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "ref": "t2", "kind": "task", "parent": "s1", "title": "Pilot user list", "type": "dependency", "client_owed": true, "client_owner": "Dana Kowal", "due_text": "Monday", "quote": "I will send the pilot user list on Monday"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Load test for ingest", "type": "session", "quote": "Somebody has to run the load test for ingest"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Security review of the vendor contract", "type": "action", "internal_owner": "Ravi Mehta", "due_text": "next week", "quote": "I will run the security review of the vendor contract next week"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Rollout email", "type": "action", "internal_owner": "Dana Kowal", "due_text": "Thursday", "quote": "Dana, please draft the rollout email for the pilot users by Thursday"}),
            json!({"op": "link", "node": "t1", "depends_on": "t2", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "waiting", "node": "t1", "who": "Sam", "what": "the signing certificate", "quote": "can't start until Sam sends us the signing certificate"}),
        ]).unwrap();
        assert_eq!(r["rejected"].as_array().map(|a| a.len()), Some(0), "{r}");
        let claims: Vec<Value> = ledger::verify_prep(&p.id, false).unwrap().iter().map(|c| json!({"mid": c["mid"], "verdict": "supports"})).collect();
        ledger::verify_apply(&p.id, &claims).unwrap();
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
        // Mine stays mine while it waits; what it waits on — work and a person — is in Waiting.
        assert_eq!(titles("mine"), ["SAML config"]);
        assert_eq!(v["piles"]["mine"][0]["blocked"], true);
        assert_eq!(titles("owed"), ["Rollout email"], "my words handed it to Dana");
        let mut w = titles("waiting");
        w.sort();
        assert_eq!(w, ["Pilot user list", "the signing certificate"]);
        let cert = v["piles"]["waiting"].as_array().unwrap().iter().find(|c| c["wait"] == true).unwrap();
        assert_eq!((cert["who"].as_str(), cert["blocks"][0]["title"].as_str()), (Some("Sam"), Some("SAML config")));
        assert_eq!(titles("others"), ["Security review of the vendor contract"], "Ravi's own work, not mine to chase");
        assert_eq!(titles("nobody"), ["Load test for ingest"]);
        let pl = v["piles"]["waiting"].as_array().unwrap().iter().find(|c| c["title"] == "Pilot user list").unwrap();
        assert_eq!((pl["client"].as_bool(), pl["late"].as_bool(), pl["quiet"].as_i64()), (Some(true), Some(true), Some(8)), "Monday the 26th is before the 28th");
        let dana = v["asks"].as_array().unwrap().iter().find(|a| a["who"] == "Dana Kowal").unwrap();
        assert_eq!(dana["next"]["subject"], "Aurora weekly", "matched by email, any case");
        let whys: Vec<&str> = dana["items"].as_array().unwrap().iter().map(|i| i["why"].as_str().unwrap()).collect();
        assert_eq!(whys, ["late", "waiting"], "the rollout email (Thursday the 22nd), then the list my work waits on");
        // Sam's certificate would be next, but a story gets two asks at most.
        assert!(!v["asks"].as_array().unwrap().iter().any(|a| a["who"] == "Ravi Mehta"), "others' work isn't chased");
        assert_eq!(v["projects"][0]["tasks"], 5);
        // Handed over by hand: the user says who delegated it.
        ledger::human_event(&p.id, &json!({"op": "edit", "node": "T004", "set": {"delegated_by": "Me"}})).unwrap();
        let v = across_with(&projects::list(), &me, &meetings, NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(), "2026-10-28T12:00:00Z").unwrap();
        assert!(v["piles"]["owed"].as_array().unwrap().iter().any(|c| c["title"] == "Security review of the vendor contract"));
        // The client's summary counts tasks, each once: Sam's certificate is a person waited on, not a task.
        let c = client("northwind", &meetings, NaiveDate::from_ymd_opt(2026, 10, 28).unwrap(), "2026-10-28T12:00:00Z").unwrap();
        let sum = &c["threads"][0]["summary"];
        assert_eq!(sum["piles"], json!({"mine": 1, "owed": 2, "waiting": 1, "nobody": 1, "others": 0}));
        let late = ["mine", "owed", "waiting", "nobody", "others"].iter().flat_map(|k| c["work"]["piles"][*k].as_array().unwrap().iter()).filter(|x| x["wait"] != true && x["also"].is_null() && x["late"] == true).count();
        assert_eq!(sum["late"], late);
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


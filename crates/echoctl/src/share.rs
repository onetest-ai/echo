//! What Echo shares with a plugin that asked for it at install (`uses.echo.read` / `write`, `ctx.echo`): its sources
//! — a call's minutes and transcript, a mail conversation's or channel post's facts from Recall —, the list of them,
//! people, the dictionary; and the dictionary's one write (names added, never removed). Plus the events that say a
//! source is new or changed (`minutes_written`, `sources_changed`). A source's id: `call:<folder>`, `mail:<Recall
//! source>` (`mail/…`, `channel/…`).

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::path::Path;

/// A call's source id and link.
fn call_id(dir: &Path) -> (String, String) {
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    (format!("call:{name}"), format!("echo://call/{name}"))
}

/// A thread in a Recall document's refs: `{id, …}` (or a bare id, older).
#[cfg(feature = "brain")]
fn ref_thread(t: &Value) -> Option<&str> {
    t["id"].as_str().or(t.as_str())
}

/// A thread as a source names it: `{id, name, client, client_name}` (`client` the client's id, `client_name` its name).
fn thread_ref(p: &crate::projects::Project, clients: &[crate::projects::Client]) -> Value {
    let client_name = p.client.as_deref().and_then(|c| clients.iter().find(|x| x.id == c)).map(|c| c.name.clone());
    json!({"id": p.id, "name": p.name, "client": p.client, "client_name": client_name})
}

/// A call's threads (`thread_ref`), as Echo put it in them.
fn threads_of_call(dir: &Path) -> Vec<Value> {
    let clients = crate::projects::clients().unwrap_or_default();
    crate::projects::of_call(dir).unwrap_or_default().into_iter().map(|(p, _)| thread_ref(&p, &clients)).collect()
}

/// A source's client: `{id, name}` when its threads have one client between them, else null (never a guess between two).
fn client_of(threads: &[Value]) -> Value {
    let ids: std::collections::BTreeSet<&str> = threads.iter().filter_map(|t| t["client"].as_str()).collect();
    match ids.into_iter().collect::<Vec<_>>()[..] {
        [one] => json!({"id": one, "name": threads.iter().find(|t| t["client"] == one).map(|t| t["client_name"].clone())}),
        _ => Value::Null,
    }
}

/// At most this many of each list a source or an event carries (decisions, questions, actions, people, messages).
const LIST_MAX: usize = 50;
/// A line of the minutes is cut to this many characters, their summary to `SUMMARY_MAX`.
const LINE_MAX: usize = 500;
const SUMMARY_MAX: usize = 4000;

fn cut(s: &str, n: usize) -> String {
    let s = s.trim();
    match s.char_indices().nth(n) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

/// A call's facts: `{title, started_at, ended_at, duration_min, app, memo}` (`duration_min` from its start and end).
fn call_facts(meta: &Value) -> serde_json::Map<String, Value> {
    let at = |k: &str| meta[k].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
    let minutes = at("started_at").zip(at("ended_at")).map(|(a, b)| ((b - a).num_seconds() as f64 / 60.0).round() as i64).filter(|m| *m >= 0);
    let v = json!({"title": meta["title"], "started_at": meta["started_at"], "ended_at": meta["ended_at"], "duration_min": minutes, "app": meta["app"], "memo": meta["kind"] == "memo"});
    v.as_object().cloned().unwrap_or_default()
}

/// Everyone `echo.read('people')` lists: People's, without those seen only in closed threads (`archived`).
fn known_people() -> Vec<Value> {
    let everyone = crate::people::everyone(chrono::Local::now().date_naive()).unwrap_or_default();
    everyone["people"].as_array().into_iter().flatten().filter(|x| x["archived"].is_null()).cloned().collect()
}

/// Who took part as the invite names them (`attendees`), each with the person Echo knows them as: `person` their key in
/// `echo.read('people')` (`known_people`) by address, by name only when the invite gives no address; null when Echo
/// doesn't know them. At most `LIST_MAX`.
fn participants(dir: &Path, people: &[Value]) -> Vec<Value> {
    let invited = attendees(&crate::calls::read_json(&dir.join("meeting.json")));
    let same = |x: &Value, e: &str| x.as_str().is_some_and(|x| x.eq_ignore_ascii_case(e));
    let person = |name: &str, email: Option<&str>| -> Value {
        let found = match email.filter(|e| !e.is_empty()) {
            Some(e) => people.iter().find(|p| same(&p["email"], e) || p["also"].as_array().is_some_and(|a| a.iter().any(|x| same(x, e)))),
            None => {
                let n = crate::people::norm(name);
                (!n.is_empty()).then(|| people.iter().find(|p| p["name"].as_str().is_some_and(|x| crate::people::norm(x) == n))).flatten()
            }
        };
        found.map(|p| p["key"].clone()).unwrap_or(Value::Null)
    };
    invited.into_iter().take(LIST_MAX).map(|p| json!({"name": p["name"], "email": p["email"], "person": person(p["name"].as_str().unwrap_or(""), p["email"].as_str())})).collect()
}

/// What a minutes heading has in it (lowercase) to open a part: its words in English, Ukrainian or Russian.
const SUMMARY_WORDS: &[&str] = &["summary", "підсум", "резюм", "итог", "сводк"];
const DECISION_WORDS: &[&str] = &["decision", "рішен", "решени", "решено"];
const QUESTION_WORDS: &[&str] = &["question", "питан", "вопрос"];

/// A call's minutes by section, from what Echo keeps: `{summary, decisions, actions, questions, revision}` — `summary`
/// the text under a heading with one of `SUMMARY_WORDS` in it, `decisions` and `questions` the bullets under headings
/// with one of `DECISION_WORDS` / `QUESTION_WORDS`, `actions` the call's action items as Echo keeps them
/// (`actions.json`: `{id, text, owner, due, done}`), `revision` a hash of the minutes that changes when they do. With
/// `body`, also the minutes' Markdown from their first `## ` section on (without the header: title, date, who). None
/// without minutes.
fn minutes_parts(dir: &Path, body: bool) -> Option<Value> {
    let md = std::fs::read_to_string(dir.join("notes.md")).ok()?;
    let (mut summary, mut decisions, mut questions) = (vec![], vec![], vec![]);
    let mut section = String::new();
    for line in md.lines() {
        let l = line.trim_start();
        if let Some(h) = l.strip_prefix("## ") {
            section = h.to_lowercase();
            continue;
        }
        let bullet = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")).map(|b| b.trim_start_matches("[ ] ").trim_start_matches("[x] ")).filter(|b| !b.trim().is_empty());
        let under = |words: &[&str]| words.iter().any(|w| section.contains(w));
        if under(SUMMARY_WORDS) {
            summary.push(line);
        } else if let Some(b) = bullet {
            if under(DECISION_WORDS) && decisions.len() < LIST_MAX {
                decisions.push(cut(b, LINE_MAX));
            } else if under(QUESTION_WORDS) && questions.len() < LIST_MAX {
                questions.push(cut(b, LINE_MAX));
            }
        }
    }
    let actions: Vec<Value> = crate::calls::read_json(&dir.join(crate::calls::ACTIONS))
        .as_array()
        .into_iter()
        .flatten()
        .take(LIST_MAX)
        .map(|a| json!({"id": a["id"], "text": cut(a["task"].as_str().unwrap_or(""), LINE_MAX), "owner": a["owner"], "due": a["due"], "done": a["status"] == "done"}))
        .collect();
    let summary = summary.join("\n");
    let mut out = json!({"summary": (!summary.trim().is_empty()).then(|| cut(&summary, SUMMARY_MAX)), "decisions": decisions, "actions": actions, "questions": questions, "revision": revision(&md)});
    if body {
        let at = if md.starts_with("## ") { 0 } else { md.find("\n## ").map_or(md.len(), |i| i + 1) };
        out["body"] = json!(md[at..].trim_end());
    }
    Some(out)
}

/// A short hash of the minutes: the same text, the same revision.
fn revision(md: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(md.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Who a call's invite names: its organizer, then its attendees, each once — `{name, email}`.
fn attendees(meeting: &Value) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for p in std::iter::once(&meeting["organizer"]).chain(meeting["attendees"].as_array().into_iter().flatten()) {
        let (name, email) = (p["name"].as_str().unwrap_or(""), p["email"].as_str());
        if name.is_empty() && email.is_none() || out.iter().any(|x| x["name"] == name && x["email"].as_str() == email) {
            continue;
        }
        out.push(json!({"name": name, "email": email}));
    }
    out
}

/// One source in the shape a plugin's ingest takes: `{id, kind, title, date, text, link, threads, client, profile}` —
/// `threads` Echo's routing (`thread_ref`), `client` theirs when they have one (`client_of`), `profile` the AI profile
/// its text may go to (strict routing: a call's own, a mail's threads'). A call's also has its facts (`call_facts`, and
/// `platform`), `attendees` (`{name, email}`, the invite's), `participants` (the same with `person`), `minutes`
/// (`minutes_parts` with `body`), `transcript` and `chat` (its meeting chat: `[{at, from, text, files}]`, `[]` when
/// none) — `opts` `{transcript: false}` / `{chat: false}` leave those two out. A mail's or post's: `mail_source`.
pub fn source(id: &str, opts: &Value) -> Result<Value> {
    if let Some(name) = id.strip_prefix("call:") {
        anyhow::ensure!(crate::links::Link::parse(&format!("echo://call/{name}")).is_some(), "not a call: {id}");
        let dir = crate::calls::resolve_call(name).map_err(|_| anyhow!("no call {name:?}"))?;
        let meta = crate::calls::read_json(&dir.join("meta.json"));
        let text = std::fs::read_to_string(dir.join("notes.md")).map_err(|_| anyhow!("{name}: no minutes yet"))?;
        let (id, link) = call_id(&dir);
        let chat: Vec<Value> = crate::calls::read_json(&dir.join("chat.json"))
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| json!({"at": m["at"], "from": m["from"], "text": m["text"], "files": m["files"].as_array().cloned().unwrap_or_default()}))
            .collect();
        let threads = threads_of_call(&dir);
        let mut out = json!({
            "id": id, "kind": if meta["kind"] == "memo" { "memo" } else { "call" },
            "date": meta["started_at"].as_str().and_then(|s| s.get(..10)),
            "text": text, "link": link, "client": client_of(&threads), "threads": threads,
            "profile": crate::ai::call_profile_id(&dir).ok(), "platform": meta["platform"],
            "attendees": attendees(&crate::calls::read_json(&dir.join("meeting.json"))), "participants": participants(&dir, &known_people()),
            "minutes": minutes_parts(&dir, true),
        });
        for (k, v) in call_facts(&meta) {
            out[k] = v;
        }
        if opts["transcript"] != false {
            out["transcript"] = json!(std::fs::read_to_string(dir.join("transcript.md")).ok());
        }
        if opts["chat"] != false {
            out["chat"] = json!(chat);
        }
        return Ok(out);
    }
    if let Some(src) = id.strip_prefix("mail:") {
        return mail_source(src);
    }
    bail!("not a source id Echo knows: {id} (call:<folder> | mail:<source>)")
}

/// A mail conversation or channel post as Recall indexed it, by its section (one a message): `{from, at, text, link}` —
/// `at` the message's day and time, `text` the facts Recall's reader kept of it (never the message's own words), in order.
#[cfg(feature = "brain")]
fn mail_messages(c: &rusqlite::Connection, src: &str) -> Result<Vec<Value>> {
    let rows: Vec<[Option<String>; 4]> = c
        .prepare("SELECT title, speaker, text, link FROM chunks WHERE source = ?1 ORDER BY ord")?
        .query_map([src], |r| Ok([r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?]))?
        .collect::<Result<_, _>>()?;
    let mut out: Vec<(String, Value)> = vec![];
    for [title, speaker, text, link] in rows {
        let (title, text) = (title.unwrap_or_default(), text.unwrap_or_default());
        // A long message's facts are several chunks of one section.
        if let Some((_, last)) = out.last_mut().filter(|(t, _)| *t == title) {
            last["text"] = json!(format!("{}\n{}", last["text"].as_str().unwrap_or(""), text.trim()));
            continue;
        }
        // The section's heading: "YYYY-MM-DD HH:MM <who>" (`brain::sources::part`).
        let at = title.get(..16).filter(|a| a.as_bytes().get(4) == Some(&b'-')).map(str::to_owned);
        out.push((title.clone(), json!({"from": speaker, "at": at, "text": text.trim(), "link": link})));
    }
    Ok(out.into_iter().map(|(_, m)| m).take(LIST_MAX).collect())
}

/// What `mail_item` routes a mail by, read once for a list or an event: the threads, the clients, the default AI profile.
#[cfg(feature = "brain")]
struct Routing {
    threads: Vec<crate::projects::Project>,
    clients: Vec<crate::projects::Client>,
    default: String,
}

#[cfg(feature = "brain")]
impl Routing {
    fn load() -> Self {
        Routing { threads: crate::projects::list(), clients: crate::projects::clients().unwrap_or_default(), default: crate::ai::default_id() }
    }
}

/// What Recall decided of a mail conversation or channel post when it indexed it: `{id, kind, title, subject, at,
/// threads, client, profile, from, people, trust, link, url}` — `threads` the threads whose sources matched it,
/// `client` theirs (`client_of`), `profile` their AI profile when they share one, `from` who wrote the messages it
/// holds, `people` the conversation's addresses but the user's (mail), `trust` "trusted": Recall indexes only the
/// messages of senders the user trusts (a sender waiting for the user's decision, ignored, or unauthenticated stays
/// out of it); `at` its last message's day.
#[cfg(feature = "brain")]
fn mail_item(c: &rusqlite::Connection, src: &str, routing: &Routing) -> Result<Value> {
    let (kind, title, date, refs): (String, Option<String>, Option<String>, Option<String>) = c
        .query_row("SELECT kind, title, event_date, refs FROM documents WHERE source = ?1", [src], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|_| anyhow!("no {src:?} in Recall"))?;
    let refs: Value = refs.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let Routing { threads: all, clients, default } = routing;
    let mine: Vec<&crate::projects::Project> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| all.iter().find(|p| Some(p.id.as_str()) == ref_thread(t))).collect();
    let threads: Vec<Value> = mine.iter().map(|p| thread_ref(p, clients)).collect();
    // Its threads' profile when they share one (strict routing: never a guess between two).
    let profiles: std::collections::BTreeSet<String> = mine.iter().map(|p| p.ai_profile.clone().unwrap_or_else(|| default.clone())).collect();
    let mut from: Vec<String> = vec![];
    for s in c.prepare("SELECT speaker FROM chunks WHERE source = ?1 AND speaker IS NOT NULL ORDER BY ord")?.query_map([src], |r| r.get::<_, String>(0))?.flatten() {
        if !from.contains(&s) && from.len() < LIST_MAX {
            from.push(s);
        }
    }
    let people: Vec<Value> = refs["people"].as_array().into_iter().flatten().take(LIST_MAX).cloned().collect();
    Ok(json!({
        "id": format!("mail:{src}"), "kind": kind, "title": title, "subject": refs["subjects"][0].as_str().or(refs["subject"].as_str()).or(title.as_deref()),
        "at": date, "client": client_of(&threads), "threads": threads,
        "profile": if profiles.len() == 1 { profiles.into_iter().next() } else { None },
        "from": from, "people": people, "trust": "trusted", "link": format!("echo://mail/{src}"), "url": refs["url"],
    }))
}

/// A mail conversation's or channel post's source: `mail_item`, plus `date` (= `at`), `text` (the facts Recall keeps
/// of it, section by section — Recall keeps no message's own words; those are read from Microsoft 365, `ctx.m365`) and
/// `messages` (`mail_messages`).
#[cfg(feature = "brain")]
fn mail_source(src: &str) -> Result<Value> {
    anyhow::ensure!(matches!(crate::links::Link::parse(&format!("echo://mail/{src}")), Some(crate::links::Link::Mail(_))), "not a mail or post: {src}");
    let c = crate::brain::store::open(&crate::brain::store_path())?;
    let mut out = mail_item(&c, src, &Routing::load())?;
    let text: Vec<String> = c.prepare("SELECT text FROM chunks WHERE source = ?1 ORDER BY ord")?.query_map([src], |r| r.get(0))?.collect::<Result<_, _>>()?;
    out["date"] = out["at"].clone();
    out["text"] = json!(text.join("\n\n"));
    out["messages"] = json!(mail_messages(&c, src)?);
    Ok(out)
}

#[cfg(not(feature = "brain"))]
fn mail_source(src: &str) -> Result<Value> {
    bail!("{src}: this build has no Recall")
}

/// Every word of `q` (lowercase) in `hay`.
fn has_words(words: &[String], hay: &str) -> bool {
    let hay = hay.to_lowercase();
    words.iter().all(|w| hay.contains(w.as_str()))
}

fn words(opts: &Value) -> Vec<String> {
    opts["q"].as_str().unwrap_or("").to_lowercase().split_whitespace().map(str::to_owned).collect()
}

/// The sources, newest first: `{id, kind, title, date, threads, client}` (`threads` their ids, `client` as `source`
/// says it) — calls with minutes, and Recall's mail and channel posts, those with what Recall decided of each
/// (`mail_item`: `subject`, `at`, `from`, `people`, `trust`, `profile`, …), never a text. `opts`: `kind` (call | mail |
/// channel), `thread`, `client`, `q` (every word in its title, a mail's subject or senders), `since` (YYYY-MM-DD),
/// `limit` (≤ 1000, default 200).
pub fn sources(opts: &Value) -> Result<Value> {
    let kind = opts["kind"].as_str().unwrap_or("");
    let thread = opts["thread"].as_str().filter(|t| !t.is_empty());
    let client = opts["client"].as_str().filter(|t| !t.is_empty());
    let words = words(opts);
    let since = opts["since"].as_str().unwrap_or("");
    let limit = opts["limit"].as_u64().unwrap_or(200).clamp(1, 1000) as usize;
    let routed = |threads: &[Value], client_v: &Value| thread.is_none_or(|t| threads.iter().any(|x| x["id"] == t)) && client.is_none_or(|c| client_v["id"] == c);
    let mut out: Vec<Value> = vec![];
    if kind.is_empty() || kind == "call" {
        for dir in crate::calls::all_calls() {
            if !dir.join("notes.md").exists() {
                continue;
            }
            let meta = crate::calls::read_json(&dir.join("meta.json"));
            let date = meta["started_at"].as_str().and_then(|s| s.get(..10)).unwrap_or("").to_string();
            if date.as_str() < since || !has_words(&words, meta["title"].as_str().unwrap_or("")) {
                continue;
            }
            let threads = threads_of_call(&dir);
            let client_v = client_of(&threads);
            if !routed(&threads, &client_v) {
                continue;
            }
            out.push(json!({"id": call_id(&dir).0, "kind": "call", "title": meta["title"], "date": date, "threads": threads.iter().map(|t| t["id"].clone()).collect::<Vec<_>>(), "client": client_v}));
        }
    }
    #[cfg(feature = "brain")]
    if kind.is_empty() || kind == "mail" || kind == "channel" {
        if let Ok(c) = crate::brain::store::open(&crate::brain::store_path()) {
            let routing = Routing::load();
            let rows: Vec<[String; 2]> = c
                .prepare("SELECT source, kind FROM documents WHERE kind IN ('mail', 'channel') ORDER BY event_date DESC")?
                .query_map([], |r| Ok([r.get(0)?, r.get(1)?]))?
                .collect::<Result<_, _>>()?;
            for [src, k] in rows {
                if !kind.is_empty() && k != kind {
                    continue;
                }
                let Ok(mut it) = mail_item(&c, &src, &routing) else { continue };
                let hay = format!("{} {} {}", it["title"].as_str().unwrap_or(""), it["subject"].as_str().unwrap_or(""), it["from"]);
                if it["at"].as_str().unwrap_or("") < since || !has_words(&words, &hay) || !routed(it["threads"].as_array().map_or(&[], |v| v), &it["client"]) {
                    continue;
                }
                it["date"] = it["at"].clone();
                it["threads"] = json!(it["threads"].as_array().into_iter().flatten().map(|t| t["id"].clone()).collect::<Vec<_>>());
                out.push(it);
            }
        }
    }
    out.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
    out.truncate(limit);
    Ok(json!(out))
}

/// The calls, newest first, with or without minutes: `{id, title, date, started_at, ended_at, duration_min, app, memo,
/// minutes, transcript, chat, people, participants, threads, client, profile}` — `minutes`, `transcript`, `chat` say
/// what it has (its text is `source`'s); `people` the invite's names, `participants` as `source` gives them. `opts`:
/// `q` (every word in its title, folder, subject or people), `since` / `until` (YYYY-MM-DD, both days in), `thread`,
/// `client`, `limit` (≤ 500, default 50).
pub fn calls(opts: &Value) -> Result<Value> {
    let words = words(opts);
    let since = opts["since"].as_str().unwrap_or("");
    let until = opts["until"].as_str().filter(|u| !u.is_empty());
    let thread = opts["thread"].as_str().filter(|t| !t.is_empty());
    let client = opts["client"].as_str().filter(|t| !t.is_empty());
    let limit = opts["limit"].as_u64().unwrap_or(50).clamp(1, 500) as usize;
    let known = known_people();
    let mut out = vec![];
    for dir in crate::calls::all_calls().into_iter().rev() {
        if out.len() >= limit {
            break;
        }
        let meta = crate::calls::read_json(&dir.join("meta.json"));
        let date = meta["started_at"].as_str().and_then(|s| s.get(..10)).unwrap_or("").to_string();
        if date.as_str() < since || until.is_some_and(|u| date.as_str() > u) {
            continue;
        }
        let meeting = crate::calls::read_json(&dir.join("meeting.json"));
        let people: Vec<Value> = attendees(&meeting).into_iter().map(|p| p["name"].clone()).filter(|n| n.as_str().is_some_and(|n| !n.is_empty())).collect();
        let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
        let hay = format!("{} {} {} {}", meta["title"].as_str().unwrap_or(""), name.replace('-', " "), meeting["subject"].as_str().unwrap_or(""), people.iter().filter_map(|p| p.as_str()).collect::<Vec<_>>().join(" "));
        if !has_words(&words, &hay) {
            continue;
        }
        let threads = threads_of_call(&dir);
        let client_v = client_of(&threads);
        if thread.is_some_and(|t| !threads.iter().any(|x| x["id"] == t)) || client.is_some_and(|c| client_v["id"] != c) {
            continue;
        }
        let mut row = json!({
            "id": call_id(&dir).0, "date": date, "minutes": dir.join("notes.md").is_file(), "transcript": dir.join("transcript.md").is_file(),
            "chat": dir.join("chat.json").is_file(), "people": people, "participants": participants(&dir, &known), "client": client_v, "threads": threads,
            "profile": crate::ai::call_profile_id(&dir).ok(),
        });
        for (k, v) in call_facts(&meta) {
            row[k] = v;
        }
        out.push(row);
    }
    Ok(json!(out))
}

/// The meetings Echo last read from the calendar (its agenda — no new request to Microsoft 365), by start: `{id,
/// subject, start, end, organizer, attendees, join_url, response, threads, client}` (`threads`: the ids of the threads
/// Echo puts it in; `client` `{id, name}` when they have one between them). `opts`: `from` / `to` (YYYY-MM-DD, both days
/// in, by the user's local day of its start, as Today's).
pub fn calendar(opts: &Value) -> Result<Value> {
    calendar_in(opts, &chrono::Local)
}

/// `calendar`, its days in the time zone `tz`.
fn calendar_in<Tz: chrono::TimeZone>(opts: &Value, tz: &Tz) -> Result<Value> {
    let (from, to) = (opts["from"].as_str().unwrap_or(""), opts["to"].as_str().filter(|t| !t.is_empty()));
    let mut agenda = crate::watch::agenda();
    agenda.sort_by_key(crate::watch::meeting_start);
    let clients = crate::projects::clients().unwrap_or_default();
    let all = crate::projects::list();
    Ok(json!(agenda
        .iter()
        .filter(|m| {
            let day = crate::watch::meeting_start(m).map(|s| s.with_timezone(tz).date_naive().to_string()).unwrap_or_default();
            day.as_str() >= from && to.is_none_or(|t| day.as_str() <= t)
        })
        .map(|m| {
            let threads: Vec<Value> = crate::projects::for_event(m).iter().filter_map(|c| all.iter().find(|p| c["id"] == p.id.as_str())).map(|p| thread_ref(p, &clients)).collect();
            json!({
                "id": m.id, "subject": m.subject, "start": m.start, "end": m.end, "organizer": m.organizer, "attendees": m.attendees,
                "join_url": m.join_url, "response": m.response, "client": client_of(&threads),
                "threads": threads.iter().map(|t| t["id"].clone()).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>()))
}

/// Who the user is to Echo: `{name, email}` — About me's, the email else Microsoft 365's account; null when unknown.
pub fn me() -> Result<Value> {
    let me = crate::config::Config::load().ok().and_then(|c| c.me);
    let name = me.as_ref().map(|m| m.name.clone()).filter(|n| !n.is_empty());
    let email = me.and_then(|m| m.email).filter(|e| !e.is_empty()).or_else(crate::graph::account);
    Ok(json!({"name": name, "email": email}))
}

/// The parts of About me a plugin may ask for (`calls::me_parts`); all but `patterns` when it names none — as the
/// Brain's readers, never how the user works unless asked.
const ME_KINDS: &[&str] = &["identity", "people", "terms", "style", "patterns"];

/// About me for a plugin with `uses.me` (`ctx.me({kinds})`): null when it is missing, empty or still the template;
/// else `{text, name, parts, hash}` — the blocks of the kinds asked in About me's order, the name (its `Name:` line,
/// else Echo's) when `identity` is asked, each kind's blocks, and a hash of `text` that changes when the user edits it.
pub fn about_me(opts: &Value) -> Result<Value> {
    let mut kinds: Vec<&str> = vec![];
    for k in opts["kinds"].as_array().into_iter().flatten() {
        match k.as_str().and_then(|k| ME_KINDS.iter().find(|x| **x == k)) {
            Some(k) => kinds.push(k),
            None => bail!("About me has no part “{}” ({})", k.as_str().map(str::to_owned).unwrap_or_else(|| k.to_string()), ME_KINDS.join(", ")),
        }
    }
    if kinds.is_empty() {
        kinds = ME_KINDS.iter().copied().filter(|k| *k != "patterns").collect();
    }
    let Some(about) = crate::calls::me_profile() else { return Ok(Value::Null) };
    const MAX: usize = 20_000;
    let text = crate::calls::me_parts(&about, &kinds, MAX).unwrap_or_default();
    let parts: serde_json::Map<String, Value> = kinds.iter().filter_map(|k| crate::calls::me_parts(&about, &[k], MAX).map(|p| (k.to_string(), json!(p)))).collect();
    let name = kinds.contains(&"identity").then(|| {
        let line = about.lines().find_map(|l| l.trim().strip_prefix("Name:").map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned));
        line.or_else(|| me().ok().and_then(|v| v["name"].as_str().map(str::to_owned)))
    });
    Ok(json!({"text": text, "name": name.flatten(), "parts": parts, "hash": crate::calls::me_hash(Some(&text))}))
}

/// People across the threads: `{key, name, aliases, email, also, organisation, client, title, threads, link}` (People's
/// own; roles stay there) — `aliases` the other names the user's directory knows them by, `also` their other addresses,
/// `client` their organisation's id; `known_people`: never someone seen only in closed threads.
pub fn people() -> Result<Value> {
    let directory = crate::people::directory().unwrap_or_default();
    Ok(json!(known_people()
        .iter()
        .map(|x| {
            let key = x["key"].as_str().unwrap_or("");
            let entry = directory.iter().find(|p| p.id == key || x["email"].as_str().is_some_and(|e| p.emails.iter().any(|m| m == e)));
            json!({"key": key, "name": x["name"], "aliases": entry.map(|p| p.aliases.clone()).unwrap_or_default(), "email": x["email"], "also": x["also"],
                   "organisation": x["org"]["name"], "client": x["org"]["id"], "title": x["title"], "threads": x["threads"],
                   "link": format!("echo://person/{}", key.replace('%', "%25").replace(' ', "%20"))})
        })
        .collect::<Vec<_>>()))
}

/// The dictionary: `{terms: [{name, synonyms, spellings, kind, group, client}]}` — `client` none = the practice's.
#[cfg(feature = "brain")]
pub fn dictionary() -> Result<Value> {
    let t = crate::brain::taxonomy::current().unwrap_or_default();
    Ok(json!({"terms": t.terms.iter().map(|x| json!({
        "name": x.name, "synonyms": x.synonyms, "spellings": x.variants, "kind": x.kind,
        "group": if x.group.is_empty() { crate::brain::taxonomy::group_of(&x.kind).to_string() } else { x.group.clone() },
        "client": (x.layer != "shared").then_some(&x.layer),
    })).collect::<Vec<_>>()}))
}

#[cfg(not(feature = "brain"))]
pub fn dictionary() -> Result<Value> {
    Ok(json!({"terms": []}))
}

/// Names a plugin adds to the dictionary: `{name, synonyms?, spellings?, kind?, client?}`. Merged into the term any
/// of its names already is (its other names added; nothing renamed or removed), else a new term in the client's
/// layer (or the practice's). Answers `{term, added: [names new to it], new}`.
#[cfg(feature = "brain")]
pub fn dictionary_add(plugin: &str, v: &Value) -> Result<Value> {
    let name = v["name"].as_str().map(str::trim).filter(|n| !n.is_empty() && n.chars().count() <= 80).ok_or_else(|| anyhow!("a term needs a name (≤ 80 characters)"))?;
    let words = |k: &str| -> Vec<String> {
        v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty() && x.chars().count() <= 80).map(str::to_owned).take(20).collect()
    };
    let client = v["client"].as_str().filter(|c| crate::projects::clients().ok().is_some_and(|cs| cs.iter().any(|x| x.id == *c)));
    crate::brain::taxonomy::dictionary_add(name, &words("synonyms"), &words("spellings"), v["kind"].as_str(), client, &format!("plugin:{plugin}"))
}

#[cfg(not(feature = "brain"))]
pub fn dictionary_add(_plugin: &str, _v: &Value) -> Result<Value> {
    bail!("this build has no dictionary")
}

/// Tell the plugins a call's minutes are written (the app's run or a session's).
pub fn minutes_written(dir: &Path) {
    crate::jshost::event("minutes_written", &minutes_payload(dir));
}

/// The revision of a call's minutes last told to the plugins (`minutes_written`): told again, they were rewritten.
const TOLD: &str = ".minutes-told";

/// `minutes_written` as Echo tells it: `{id, link, call, title, started_at, ended_at, duration_min, app, memo,
/// revision, rewritten}` to every plugin that hears it, and to one that reads sources (`plugins::as_heard`) also
/// `threads`, `client`, `profile`, `participants` and `minutes` (`minutes_parts`, without the body) — what Echo
/// decided of the call, never its transcript. `rewritten`: its minutes were told before (written again).
pub(crate) fn minutes_payload(dir: &Path) -> Value {
    let (id, link) = call_id(dir);
    let call = id.strip_prefix("call:").unwrap_or(&id).to_string();
    let meta = crate::calls::read_json(&dir.join("meta.json"));
    let minutes = minutes_parts(dir, false);
    let revision = minutes.as_ref().map(|m| m["revision"].clone()).unwrap_or(Value::Null);
    let rewritten = dir.join(TOLD).exists();
    if let Some(r) = revision.as_str() {
        let _ = crate::paths::write_atomic(&dir.join(TOLD), r.as_bytes());
    }
    let threads = threads_of_call(dir);
    let mut out = json!({
        "id": id, "link": link, "call": call, "revision": revision, "rewritten": rewritten,
        "client": client_of(&threads), "threads": threads, "profile": crate::ai::call_profile_id(dir).ok(),
        "participants": participants(dir, &known_people()), "minutes": minutes,
    });
    for (k, v) in call_facts(&meta) {
        out[k] = v;
    }
    out
}

/// At most this many `items` in a `sources_changed` (the rest by their ids in `changed`, read with `echo.read('source')`).
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
const ITEMS_MAX: usize = 100;

/// Tell the plugins which of Recall's mail and channel posts are new, changed or gone after a sync.
pub fn sources_changed(touched: &[String], gone: &[String]) {
    if let Some(p) = sources_payload(touched, gone) {
        crate::jshost::event("sources_changed", &p);
    }
}

/// `sources_changed` as Echo tells it: `{changed, removed}` (ids, `mail:<source>`) to every plugin that hears it, and to
/// one that reads sources also `items`: what Recall decided of each changed one (`mail_item`), at most `ITEMS_MAX`.
/// None when no mail or post changed.
pub(crate) fn sources_payload(touched: &[String], gone: &[String]) -> Option<Value> {
    let mine = |v: &[String]| v.iter().filter(|s| s.starts_with("mail/") || s.starts_with("channel/")).cloned().collect::<Vec<_>>();
    let (changed, removed) = (mine(touched), mine(gone));
    if changed.is_empty() && removed.is_empty() {
        return None;
    }
    #[cfg(feature = "brain")]
    let items: Vec<Value> = crate::brain::store::open(&crate::brain::store_path())
        .map(|c| {
            let routing = Routing::load();
            changed.iter().take(ITEMS_MAX).filter_map(|s| mail_item(&c, s, &routing).ok()).collect()
        })
        .unwrap_or_default();
    #[cfg(not(feature = "brain"))]
    let items: Vec<Value> = vec![];
    let ids = |v: Vec<String>| v.into_iter().map(|s| format!("mail:{s}")).collect::<Vec<_>>();
    Some(json!({"changed": ids(changed), "removed": ids(removed), "items": items}))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Acme with a thread "Acme pilot" that takes calls whose subject says "pilot", Jane Doe in it.
    fn acme_pilot() -> String {
        crate::projects::add_client("Acme", &["acme.example".into()], false).unwrap();
        let pilot = crate::projects::create("Acme pilot", Some("acme"), None).unwrap().id;
        crate::projects::update(&pilot, |p| {
            p.rules.push(crate::projects::Rule::Subject { contains: "pilot".into() });
            Ok(())
        })
        .unwrap();
        crate::people::place(&pilot, "Jane Doe", Some("jane@acme.example"), &json!({"role": "Sponsor"})).unwrap();
        pilot
    }

    #[test]
    fn minutes_written_carries_the_calls_routing_people_and_minutes_by_section() {
        let (_env, _root) = crate::paths::testenv::folders("share-minutes-event");
        let pilot = acme_pilot();
        let d = crate::paths::calls_dir().join("2026-11-10_100000-acme-pilot");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), r#"{"title": "Acme pilot sync", "started_at": "2026-11-10T10:00:00+01:00", "ended_at": "2026-11-10T10:44:40+01:00", "app": "teams", "ai_profile": "work"}"#).unwrap();
        std::fs::write(d.join("meeting.json"), r#"{"subject": "Acme pilot sync", "organizer": {"name": "Jane Doe", "email": "Jane@acme.example"}, "attendees": [{"name": "Kim Ito", "email": "kim@acme.example"}]}"#).unwrap();
        std::fs::write(
            d.join("notes.md"),
            "# Acme pilot sync\n\n**When:** 10 Nov · 45 min\n\n## Summary\nThe pilot's dates.\n- **Dates:** moved [12:05]\n\n## Decisions\n- Pilot moves to 24 Nov [12:05]\n\n## Action items\n- [ ] Me — send the plan\n\n## Open questions\n- Who signs off?\n",
        )
        .unwrap();
        std::fs::write(d.join(crate::calls::ACTIONS), r#"[{"id": "a1", "task": "Send the plan", "owner": "Me", "due": "2026-11-14", "status": "todo"}]"#).unwrap();
        std::fs::write(d.join("transcript.md"), "**Jane Doe** [12:05]\nThe pilot moves.\n").unwrap();
        let p = minutes_payload(&d);
        assert_eq!((p["id"].as_str(), p["call"].as_str(), p["title"].as_str()), (Some("call:2026-11-10_100000-acme-pilot"), Some("2026-11-10_100000-acme-pilot"), Some("Acme pilot sync")));
        assert_eq!((p["duration_min"].as_i64(), p["app"].as_str(), p["memo"].as_bool(), p["profile"].as_str()), (Some(45), Some("teams"), Some(false), Some("work")));
        assert_eq!(p["threads"], json!([{"id": pilot, "name": "Acme pilot", "client": "acme", "client_name": "Acme"}]));
        assert_eq!(p["client"], json!({"id": "acme", "name": "Acme"}));
        assert_eq!(p["participants"][0], json!({"name": "Jane Doe", "email": "Jane@acme.example", "person": "jane@acme.example"}), "known by address, any case");
        assert_eq!(p["participants"][1]["person"], "kim@acme.example", "seen in the thread's calls");
        let lunch = crate::paths::calls_dir().join("2026-11-10_130000-lunch");
        std::fs::create_dir_all(&lunch).unwrap();
        std::fs::write(lunch.join("meta.json"), r#"{"title": "Lunch"}"#).unwrap();
        std::fs::write(lunch.join("meeting.json"), r#"{"subject": "Lunch", "attendees": [{"name": "Bob Stone", "email": "bob@else.example"}]}"#).unwrap();
        let l = minutes_payload(&lunch);
        assert_eq!((l["participants"][0]["person"].clone(), l["threads"].clone(), l["client"].clone(), l["minutes"].clone()), (Value::Null, json!([]), Value::Null, Value::Null), "{l}");
        let m = &p["minutes"];
        assert_eq!(m["summary"], "The pilot's dates.\n- **Dates:** moved [12:05]");
        assert_eq!((m["decisions"].clone(), m["questions"].clone()), (json!(["Pilot moves to 24 Nov [12:05]"]), json!(["Who signs off?"])));
        assert_eq!(m["actions"], json!([{"id": "a1", "text": "Send the plan", "owner": "Me", "due": "2026-11-14", "done": false}]));
        assert!(m.get("body").is_none() && !p.to_string().contains("The pilot moves."), "no body, never the transcript: {p}");
        // Told once: written again, the event says so, and its revision changes with the minutes.
        assert_eq!(p["rewritten"], false);
        let again = minutes_payload(&d);
        assert_eq!((again["rewritten"].as_bool(), again["revision"] == p["revision"]), (Some(true), true));
        std::fs::write(d.join("notes.md"), "# Acme pilot sync\n\n## Decisions\n- Pilot stays\n").unwrap();
        assert_ne!(minutes_payload(&d)["revision"], p["revision"]);
        // The read says the same, with the minutes' body (no header) — and leaves out the transcript and chat when asked.
        std::fs::write(d.join("notes.md"), "# Acme pilot sync\n\n**When:** 10 Nov\n\n## Decisions\n- Pilot stays\n").unwrap();
        let s = source("call:2026-11-10_100000-acme-pilot", &json!({})).unwrap();
        for k in ["threads", "client", "profile", "participants", "duration_min", "app", "memo", "title"] {
            assert_eq!(s[k], minutes_payload(&d)[k], "{k}");
        }
        assert_eq!((s["minutes"]["body"].as_str(), s["minutes"]["decisions"].clone()), (Some("## Decisions\n- Pilot stays"), json!(["Pilot stays"])));
        assert!(s["transcript"].is_string() && s["chat"].is_array());
        let lean = source("call:2026-11-10_100000-acme-pilot", &json!({"transcript": false, "chat": false})).unwrap();
        assert!(lean.get("transcript").is_none() && lean.get("chat").is_none(), "{lean}");
    }

    #[test]
    fn the_lists_carry_echos_routing_and_filter_by_it() {
        let (_env, _root) = crate::paths::testenv::folders("share-lists");
        let pilot = acme_pilot();
        crate::people::merge("n:j doe", "J. Doe", &[], "jane@acme.example", "Jane Doe", Some("jane@acme.example")).unwrap();
        for (folder, subject) in [("2026-11-10_100000-acme-pilot", "Acme pilot sync"), ("2026-11-12_100000-lunch", "Lunch")] {
            let d = crate::paths::calls_dir().join(folder);
            std::fs::create_dir_all(&d).unwrap();
            let day = &folder[..10];
            std::fs::write(d.join("meta.json"), json!({"title": subject, "started_at": format!("{day}T10:00:00+01:00"), "ended_at": format!("{day}T10:30:00+01:00"), "app": "teams"}).to_string()).unwrap();
            std::fs::write(d.join("meeting.json"), json!({"subject": subject, "attendees": [{"name": "Jane Doe", "email": "jane@acme.example"}]}).to_string()).unwrap();
            std::fs::write(d.join("notes.md"), format!("# {subject}\n")).unwrap();
        }
        // Calls: a thread's or a client's, within days; each with its facts, client and participants.
        let ids = |v: Value| v.as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(ids(calls(&json!({"thread": pilot})).unwrap()), ["call:2026-11-10_100000-acme-pilot"]);
        assert_eq!(ids(calls(&json!({"client": "acme"})).unwrap()), ["call:2026-11-10_100000-acme-pilot"]);
        assert_eq!(ids(calls(&json!({"until": "2026-11-11"})).unwrap()), ["call:2026-11-10_100000-acme-pilot"]);
        assert_eq!(ids(calls(&json!({"since": "2026-11-11", "until": "2026-11-12"})).unwrap()), ["call:2026-11-12_100000-lunch"]);
        let c = &calls(&json!({"thread": pilot})).unwrap()[0];
        assert_eq!((c["duration_min"].as_i64(), c["app"].as_str(), c["client"].clone()), (Some(30), Some("teams"), json!({"id": "acme", "name": "Acme"})));
        assert_eq!(c["participants"], json!([{"name": "Jane Doe", "email": "jane@acme.example", "person": "jane@acme.example"}]));
        // Sources: by client and words, with the client.
        assert_eq!(ids(sources(&json!({"client": "acme"})).unwrap()), ["call:2026-11-10_100000-acme-pilot"]);
        assert_eq!(ids(sources(&json!({"q": "lunch"})).unwrap()), ["call:2026-11-12_100000-lunch"]);
        assert_eq!(sources(&json!({"q": "lunch"})).unwrap()[0]["client"], Value::Null);
        // The calendar: by days, with the client its threads have.
        std::fs::create_dir_all(crate::paths::state_dir()).unwrap();
        std::fs::write(
            crate::paths::state_dir().join("agenda.json"),
            r#"[{"id": "m1", "subject": "Acme pilot weekly", "start": "2026-11-20T10:00:00Z"}, {"id": "m2", "subject": "Lunch", "start": "2026-11-25T12:00:00Z"}]"#,
        )
        .unwrap();
        let cal = calendar(&json!({"from": "2026-11-20", "to": "2026-11-21"})).unwrap();
        assert_eq!(ids(cal.clone()), ["m1"]);
        assert_eq!((cal[0]["threads"].clone(), cal[0]["client"].clone()), (json!([pilot]), json!({"id": "acme", "name": "Acme"})));
        assert_eq!(ids(calendar(&json!({})).unwrap()), ["m1", "m2"]);
        // People: their addresses, other names and client.
        let ppl = people().unwrap();
        let jane = ppl.as_array().unwrap().iter().find(|p| p["key"] == "jane@acme.example").unwrap();
        assert_eq!((jane["email"].as_str(), jane["client"].as_str(), jane["aliases"].clone()), (Some("jane@acme.example"), Some("acme"), json!(["J. Doe"])), "{jane}");
    }

    #[test]
    fn the_calendar_by_days_is_the_users_local_day() {
        let (_env, _root) = crate::paths::testenv::folders("share-calendar-local-day");
        std::fs::create_dir_all(crate::paths::state_dir()).unwrap();
        // 00:30 on the 20th in UTC+2 is 22:30 on the 19th in UTC; Graph's naive UTC too.
        std::fs::write(
            crate::paths::state_dir().join("agenda.json"),
            r#"[{"id": "early", "subject": "Early sync", "start": "2026-11-19T22:30:00Z"}, {"id": "naive", "subject": "Late", "start": "2026-11-20T21:30:00.0000000"}]"#,
        )
        .unwrap();
        let plus2 = chrono::FixedOffset::east_opt(2 * 3600).unwrap();
        let ids = |v: Value| v.as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(ids(calendar_in(&json!({"from": "2026-11-20", "to": "2026-11-20"}), &plus2).unwrap()), ["early", "naive"]);
        assert!(ids(calendar_in(&json!({"to": "2026-11-19"}), &plus2).unwrap()).is_empty(), "not the UTC day");
        assert!(ids(calendar_in(&json!({"from": "2026-11-21"}), &plus2).unwrap()).is_empty(), "23:30 in UTC+2 is still the 20th");
        assert_eq!(ids(calendar_in(&json!({"from": "2026-11-19", "to": "2026-11-19"}), &chrono::Utc).unwrap()), ["early"]);
    }

    #[test]
    fn russian_and_ukrainian_minutes_give_their_summary_decisions_and_questions() {
        let (_env, _root) = crate::paths::testenv::folders("share-minutes-languages");
        let call = |folder: &str, md: &str| {
            let d = crate::paths::calls_dir().join(folder);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("notes.md"), md).unwrap();
            minutes_parts(&d, false).unwrap()
        };
        let ru = call(
            "2026-11-10_100000-acme-ru",
            "# Синк по пилоту Acme\n\n## Итоги\nСроки пилота сдвинулись.\n\n## Решения\n- Пилот переносится на 24 ноября\n\n## РЕШЕНО ПОЗЖЕ\n- Площадка — офис Acme\n\n## Открытые вопросы\n- Кто подписывает?\n",
        );
        assert_eq!(ru["summary"], "Сроки пилота сдвинулись.");
        assert_eq!((ru["decisions"].clone(), ru["questions"].clone()), (json!(["Пилот переносится на 24 ноября", "Площадка — офис Acme"]), json!(["Кто подписывает?"])));
        let ru2 = call("2026-11-10_110000-acme-ru", "# Acme\n\n## Сводка\nКоротко.\n\n## Вопросы\n- Когда старт?\n");
        assert_eq!((ru2["summary"].as_str(), ru2["questions"].clone()), (Some("Коротко."), json!(["Когда старт?"])));
        let uk = call(
            "2026-11-11_100000-acme-uk",
            "# Синк по пілоту Acme\n\n## Підсумки\nДати пілоту змінилися.\n\n## Рішення\n- Пілот переноситься на 24 листопада\n\n## Відкриті питання\n- Хто погоджує?\n",
        );
        assert_eq!(uk["summary"], "Дати пілоту змінилися.");
        assert_eq!((uk["decisions"].clone(), uk["questions"].clone()), (json!(["Пілот переноситься на 24 листопада"]), json!(["Хто погоджує?"])));
        let uk2 = call("2026-11-11_110000-acme-uk", "# Acme\n\n## Резюме\nКоротко.\n\n## Інше\n- не рішення\n");
        assert_eq!((uk2["summary"].as_str(), uk2["decisions"].clone()), (Some("Коротко."), json!([])), "only a heading's words count");
    }

    #[test]
    fn a_participant_is_a_listed_person_by_address_and_by_name_only_without_one() {
        let (_env, _root) = crate::paths::testenv::folders("share-participants");
        acme_pilot();
        let old = crate::projects::create("Acme pilot 2025", Some("acme"), None).unwrap().id;
        crate::people::place(&old, "Old Timer", Some("old@acme.example"), &json!({"role": "Sponsor"})).unwrap();
        assert!(people().unwrap().as_array().unwrap().iter().any(|p| p["key"] == "old@acme.example"), "listed while their thread is open");
        crate::projects::set_stage(&old, crate::projects::Stage::Done, None).unwrap();
        let d = crate::paths::calls_dir().join("2026-11-12_100000-acme-sync");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("meeting.json"),
            r#"{"subject": "Acme sync", "attendees": [{"name": "Jane Doe", "email": "jane.doe@other.example"}, {"name": "Jane Doe"}, {"name": "Old Timer", "email": "old@acme.example"}, {"name": "Old Timer"}]}"#,
        )
        .unwrap();
        let person = |i: usize| participants(&d, &known_people())[i]["person"].clone();
        assert_eq!(person(0), Value::Null, "an address Echo doesn't know: never matched by name");
        assert_eq!(person(1), "jane@acme.example", "no address: by name");
        assert_eq!((person(2), person(3)), (Value::Null, Value::Null), "seen only in a closed thread: not a person the people read lists");
        let listed: Vec<Value> = people().unwrap().as_array().unwrap().iter().map(|p| p["key"].clone()).collect();
        assert!(listed.contains(&json!("jane@acme.example")) && !listed.contains(&json!("old@acme.example")), "{listed:?}");
    }

    #[test]
    fn a_plugin_that_doesnt_read_sources_hears_no_routing_people_minutes_or_mail() {
        let u = |v: Value| v.as_object().cloned().unwrap();
        let readers = u(json!({"echo": {"read": ["source"]}}));
        let listener = u(json!({"events": {"on": ["minutes_written", "sources_changed"]}}));
        assert!(crate::plugins::reads_sources(&readers) && crate::plugins::reads_sources(&u(json!({"echo": {"read": ["sources"]}}))));
        assert!(!crate::plugins::reads_sources(&listener) && !crate::plugins::reads_sources(&u(json!({"echo": {"read": ["calls", "people"]}}))));
        let full = json!({"id": "call:x", "title": "Acme pilot sync", "revision": "ab", "threads": [{"id": "p"}], "client": {"id": "acme"}, "profile": "work",
                          "participants": [{"name": "Jane Doe"}], "minutes": {"decisions": ["x"]}});
        let mut heard = full.clone();
        crate::plugins::as_heard("minutes_written", &mut heard, false);
        assert_eq!(heard, json!({"id": "call:x", "title": "Acme pilot sync", "revision": "ab"}));
        let mut whole = full.clone();
        crate::plugins::as_heard("minutes_written", &mut whole, true);
        assert_eq!(whole, full);
        let mut mail = json!({"changed": ["mail:mail/1"], "removed": [], "items": [{"subject": "Pilot dates", "from": ["Jane Doe"]}]});
        crate::plugins::as_heard("sources_changed", &mut mail, false);
        assert_eq!(mail, json!({"changed": ["mail:mail/1"], "removed": []}));
        let mut other = json!({"call": "x", "title": "t"});
        crate::plugins::as_heard("recording_started", &mut other, false);
        assert_eq!(other, json!({"call": "x", "title": "t"}));
    }

    #[cfg(feature = "brain")]
    #[test]
    fn the_sources_list_and_event_give_each_mail_as_recall_decided_it() {
        let (_env, _root) = crate::paths::testenv::folders("share-sources-golden");
        let pilot = acme_pilot();
        crate::projects::update(&pilot, |p| {
            p.ai_profile = Some("work".into());
            Ok(())
        })
        .unwrap();
        crate::projects::add_client("Globex", &["globex.example".into()], false).unwrap();
        let globex = crate::projects::create("Globex pilot", Some("globex"), None).unwrap().id;
        let mut inputs = vec![];
        for (src, title, day, threads) in [("mail/one", "Pilot dates", "2026-11-11", json!([{"id": pilot}])), ("mail/two", "Both pilots", "2026-11-12", json!([{"id": pilot}, {"id": globex}])), ("channel/three", "Standup", "2026-11-10", json!([{"id": globex}]))] {
            let kind = src.split('/').next().unwrap();
            let md = format!("# {title}\n\n### {day} 09:12 Jane Doe\n\n- {title}, a fact\n");
            let refs = json!({"kind": kind, "people": ["jane@acme.example"], "url": format!("https://outlook.example/{src}"), "subjects": [title], "threads": threads});
            inputs.push(crate::brain::store::Input {
                source: src.into(), kind: kind.into(), title: title.into(), event_date: Some(day.into()), md, refs,
                link: Box::new(|_| "https://outlook.example/x".into()), speaker: Box::new(|c| c.title.splitn(3, ' ').nth(2).map(str::to_owned)),
            });
        }
        crate::brain::store::sync_kinds(&crate::brain::store_path(), inputs, None, &[]).unwrap();
        // Each mail as it was given before its threads and profiles were read once per list: one client and profile,
        // two threads of two clients (no client, no profile), the default profile.
        let (acme, globex_t) = (json!({"client": "acme", "client_name": "Acme", "id": "acme-pilot", "name": "Acme pilot"}), json!({"client": "globex", "client_name": "Globex", "id": "globex-pilot", "name": "Globex pilot"}));
        let item = |src: &str, kind: &str, title: &str, at: &str, client: Value, profile: Value, threads: Value| {
            json!({"at": at, "client": client, "from": ["Jane Doe"], "id": format!("mail:{src}"), "kind": kind, "link": format!("echo://mail/{src}"), "people": ["jane@acme.example"],
                   "profile": profile, "subject": title, "threads": threads, "title": title, "trust": "trusted", "url": format!("https://outlook.example/{src}")})
        };
        let one = item("mail/one", "mail", "Pilot dates", "2026-11-11", json!({"id": "acme", "name": "Acme"}), json!("work"), json!([acme]));
        let two = item("mail/two", "mail", "Both pilots", "2026-11-12", Value::Null, Value::Null, json!([acme, globex_t]));
        let three = item("channel/three", "channel", "Standup", "2026-11-10", json!({"id": "globex", "name": "Globex"}), json!("system"), json!([globex_t]));
        let event = sources_payload(&["mail/one".into(), "mail/two".into(), "channel/three".into()], &[]).unwrap();
        assert_eq!(event["items"], json!([one, two, three]));
        let row = |mut v: Value| {
            v["date"] = v["at"].clone();
            v["threads"] = json!(v["threads"].as_array().unwrap().iter().map(|t| t["id"].clone()).collect::<Vec<_>>());
            v
        };
        assert_eq!(sources(&json!({"kind": "mail"})).unwrap(), json!([row(two), row(one)]));
        assert_eq!(sources(&json!({"kind": "channel"})).unwrap(), json!([row(three.clone())]));
        let s = source("mail:channel/three", &json!({})).unwrap();
        for (k, v) in three.as_object().unwrap() {
            assert_eq!(&s[k], v, "{k}");
        }
    }

    #[cfg(feature = "brain")]
    #[test]
    fn sources_changed_carries_what_recall_decided_and_the_read_says_the_same() {
        let (_env, _root) = crate::paths::testenv::folders("share-sources-event");
        let pilot = acme_pilot();
        let md = "# Pilot dates\n\n### 2026-11-10 09:12 Jane Doe\n\n- The pilot moves to 24 Nov — “moves to the 24th”\n\n### 2026-11-11 08:00 Kim Ito\n\n- Kim confirms the room\n";
        let refs = json!({"kind": "mail", "conversation": "c1", "people": ["jane@acme.example", "kim@acme.example"], "url": "https://outlook.example/1", "subjects": ["Pilot dates", "RE: Pilot dates"], "threads": [{"id": pilot}]});
        let input = crate::brain::store::Input {
            source: "mail/abc".into(), kind: "mail".into(), title: "Pilot dates".into(), event_date: Some("2026-11-11".into()), md: md.into(), refs,
            link: Box::new(|_| "https://outlook.example/1".into()), speaker: Box::new(|c| c.title.splitn(3, ' ').nth(2).map(str::to_owned)),
        };
        crate::brain::store::sync_kinds(&crate::brain::store_path(), vec![input], None, &[]).unwrap();
        assert!(sources_payload(&["call/x".into()], &[]).is_none(), "calls aren't Recall's mail");
        let p = sources_payload(&["mail/abc".into(), "call/x".into()], &["channel/gone".into()]).unwrap();
        assert_eq!((p["changed"].clone(), p["removed"].clone()), (json!(["mail:mail/abc"]), json!(["mail:channel/gone"])));
        let it = &p["items"][0];
        assert_eq!((it["id"].as_str(), it["kind"].as_str(), it["subject"].as_str(), it["at"].as_str(), it["trust"].as_str()), (Some("mail:mail/abc"), Some("mail"), Some("Pilot dates"), Some("2026-11-11"), Some("trusted")));
        assert_eq!(it["threads"], json!([{"id": pilot, "name": "Acme pilot", "client": "acme", "client_name": "Acme"}]));
        assert_eq!((it["client"].clone(), it["from"].clone()), (json!({"id": "acme", "name": "Acme"}), json!(["Jane Doe", "Kim Ito"])));
        assert_eq!(it["people"], json!(["jane@acme.example", "kim@acme.example"]));
        assert!(it.get("text").is_none() && it.get("messages").is_none(), "no content in the event: {it}");
        // The read: the same routing, with Recall's facts message by message.
        let s = source("mail:mail/abc", &json!({})).unwrap();
        for k in ["threads", "client", "profile", "from", "people", "trust", "subject", "at"] {
            assert_eq!(s[k], it[k], "{k}");
        }
        assert_eq!(s["messages"][0]["from"], "Jane Doe");
        assert_eq!(s["messages"][0]["at"], "2026-11-10 09:12");
        assert!(s["messages"][0]["text"].as_str().unwrap().contains("The pilot moves to 24 Nov"), "{s}");
        assert_eq!(s["messages"][1]["from"], "Kim Ito");
        assert!(s["text"].as_str().unwrap().contains("Kim confirms the room"));
        // The list: the same decisions, its threads by id, no text; found by a sender's name and by client.
        let rows = sources(&json!({"kind": "mail", "q": "kim", "client": "acme"})).unwrap();
        let row = &rows[0];
        assert_eq!((row["id"].as_str(), row["date"].as_str(), row["threads"].clone(), row["trust"].as_str()), (Some("mail:mail/abc"), Some("2026-11-11"), json!([pilot]), Some("trusted")));
        assert_eq!((row["from"].clone(), row["client"].clone()), (it["from"].clone(), it["client"].clone()));
        assert!(row.get("text").is_none() && row.get("messages").is_none());
        assert!(sources(&json!({"kind": "mail", "q": "nobody"})).unwrap().as_array().unwrap().is_empty());
        assert!(sources(&json!({"kind": "mail", "client": "globex"})).unwrap().as_array().unwrap().is_empty());
    }

    #[test]
    fn a_calls_source_is_its_minutes_with_its_transcript() {
        let (_env, _root) = crate::paths::testenv::folders("share-source");
        let d = crate::paths::calls_dir().join("2026-10-28_110000-aurora");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), r#"{"title": "Aurora weekly", "started_at": "2026-10-28T11:00:00+02:00"}"#).unwrap();
        assert!(source("call:2026-10-28_110000-aurora", &json!({})).is_err(), "no minutes yet");
        std::fs::write(d.join("notes.md"), "# Aurora weekly\n- SSO first [12:05]\n").unwrap();
        std::fs::write(d.join("transcript.md"), "**Lena Varga** [12:05]\nSSO has to land first.\n").unwrap();
        let s = source("call:2026-10-28_110000-aurora", &json!({})).unwrap();
        assert_eq!((s["kind"].as_str(), s["date"].as_str(), s["link"].as_str()), (Some("call"), Some("2026-10-28"), Some("echo://call/2026-10-28_110000-aurora")));
        assert!(s["text"].as_str().unwrap().contains("SSO first") && s["transcript"].as_str().unwrap().contains("Lena Varga"));
        let list = sources(&json!({})).unwrap();
        assert_eq!(list[0]["id"], "call:2026-10-28_110000-aurora");
        assert!(sources(&json!({"since": "2026-11-01"})).unwrap().as_array().unwrap().is_empty());
        for bad in ["call:../x", "mail:../x", "nope:x"] {
            assert!(source(bad, &json!({})).is_err(), "{bad}");
        }
    }

    #[cfg(feature = "brain")]
    #[test]
    fn plugins_add_names_never_take_them() {
        let (_env, _root) = crate::paths::testenv::folders("share-dict");
        let r = dictionary_add("obsidian", &json!({"name": "Signal Grid", "synonyms": ["SG"], "kind": "product"})).unwrap();
        assert_eq!(r["new"], true);
        // Another name of the same thing: merged, nothing renamed.
        let r = dictionary_add("obsidian", &json!({"name": "SG", "synonyms": ["SignalGrid"], "spellings": ["signal greed"]})).unwrap();
        assert_eq!((r["term"].as_str(), r["new"].as_bool()), (Some("Signal Grid"), Some(false)), "{r}");
        let d = dictionary().unwrap();
        let t = d["terms"].as_array().unwrap().iter().find(|t| t["name"] == "Signal Grid").unwrap().clone();
        assert_eq!(t["synonyms"], json!(["SG", "SignalGrid"]));
        assert_eq!(t["spellings"], json!(["signal greed"]));
        assert!(dictionary_add("obsidian", &json!({"name": ""})).is_err());
    }
}

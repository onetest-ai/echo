//! `echo://` links: how anything outside Echo's window — a note in Echo's vault, a mail, a chat — points at what Echo
//! keeps: a call (at a moment), a thread, a client, a person, a mail conversation. The system opens Echo at them (the
//! scheme is registered: macOS `Info.plist`, Windows `HKCU\Software\Classes\echo`); the window routes them
//! (`window.echo.openLink`); `echoctl read <link>` gives Claude the text behind one, read-only.

use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Link {
    /// A call by its folder name, at a moment (seconds) or not.
    Call { name: String, t: Option<u64> },
    Thread(String),
    Client(String),
    /// A person by People's key (an address, or `n:<name>`).
    Person(String),
    /// A mail conversation or Teams post by the Brain's source id (`mail/…`, `channel/…`).
    Mail(String),
    /// A work item of a thread's ledger: a story, a task, a decision, a risk, a question (`T043`).
    Task { thread: String, id: String },
    /// A meeting's prep (`prep.rs`) by its id.
    Prep(String),
}

/// Percent-decoding of a link's parts (UTF-8).
fn decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let h = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A name that may go into a path or a route: no separators beyond the kind's own, no `..`, no control characters.
fn clean(s: &str, slash_ok: bool) -> Option<String> {
    let ok = !s.is_empty()
        && s.len() <= 300
        && !s.contains("..")
        && !s.starts_with('/')
        && s.chars().all(|c| !c.is_control() && c != '\\' && (slash_ok || c != '/'));
    ok.then(|| s.to_string())
}

impl Link {
    pub fn parse(s: &str) -> Option<Link> {
        let rest = s.trim().strip_prefix("echo://")?;
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (kind, id) = path.split_once('/')?;
        let id = decode(id.trim_end_matches('/'))?;
        let t = query.split('&').find_map(|kv| kv.strip_prefix("t=")).and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= 0.0).map(|v| v as u64);
        Some(match kind {
            "call" => Link::Call { name: clean(&id, false)?, t },
            "thread" | "project" => Link::Thread(clean(&id, false)?),
            "client" => Link::Client(clean(&id, false)?),
            "person" => Link::Person(clean(&id, false)?),
            // A Brain source id has one slash or two (`mail/<id>`, `channel/<key>/<id>`).
            "task" => {
                let (thread, item) = id.split_once('/')?;
                let ok = !item.is_empty() && item.len() <= 40 && item.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
                Link::Task { thread: clean(thread, false)?, id: ok.then(|| item.to_string())? }
            }
            "prep" => Link::Prep(Some(id).filter(|x| !x.is_empty() && x.len() <= 32 && x.bytes().all(|b| b.is_ascii_hexdigit()))?),
            "mail" | "post" => Link::Mail(clean(&id, true).filter(|x| x.starts_with("mail/") || x.starts_with("channel/"))?),
            _ => return None,
        })
    }
}

fn clock(s: u64) -> String {
    if s >= 3600 { format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

/// The call folder a link names (as `calls::resolve_call` finds it: a renamed folder by its stamp).
fn call_dir(name: &str) -> Result<PathBuf> {
    crate::calls::resolve_call(name).map_err(|_| anyhow!("no call {name:?}"))
}

/// `echoctl read <link>`: what Echo holds behind a link, as text for Claude — a call's minutes (and its turns around
/// the moment), a thread (client, stage, people with roles, calls), a client (threads), a person (People's card), a
/// mail's facts. Nothing is written.
pub fn read(link: &str) -> Result<String> {
    let l = Link::parse(link).ok_or_else(|| anyhow!("not an echo:// link Echo knows: {link}"))?;
    let mut out = String::new();
    match l {
        Link::Call { name, t } => {
            let dir = call_dir(&name)?;
            let meta = crate::calls::read_json(&dir.join("meta.json"));
            out.push_str(&format!("# Call: {} — {}\n\n", meta["title"].as_str().unwrap_or(&name), meta["started_at"].as_str().unwrap_or("").get(..16).unwrap_or("").replace('T', " ")));
            if let Some(t) = t {
                // The turns within 90 s of the moment, from the call's segments (speaker names as the transcript has them).
                let segs = crate::calls::read_json(&dir.join("segments.json"));
                let near: Vec<String> = segs
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|s| s["start"].as_f64().is_some_and(|a| (a - t as f64).abs() <= 90.0))
                    .map(|s| {
                        let who = s["name"].as_str().map(crate::people::display).unwrap_or_else(|| if s["speaker"] == "mic" || s["speaker"] == "me" { "Me".into() } else { "Remote".into() });
                        format!("[{}] {who}: {}", clock(s["start"].as_f64().unwrap_or(0.0) as u64), s["text"].as_str().unwrap_or("").trim())
                    })
                    .take(60)
                    .collect();
                out.push_str(&format!("## Said around {} (±90 s)\n{}\n\n", clock(t), if near.is_empty() { "(no transcript there)".to_string() } else { near.join("\n") }));
            }
            match std::fs::read_to_string(dir.join("notes.md")) {
                Ok(n) => out.push_str(&format!("## Minutes\n{}\n", n.chars().take(if t.is_some() { 4000 } else { 16000 }).collect::<String>())),
                Err(_) => out.push_str("(no minutes yet)\n"),
            }
        }
        Link::Thread(id) => {
            let p = crate::projects::load(&id).map_err(|_| anyhow!("no thread {id:?}"))?;
            let client = p.client.as_deref().and_then(|c| crate::projects::orgs().ok()?.into_iter().find(|o| o.id == c).map(|o| o.name)).unwrap_or_else(|| "no client".into());
            out.push_str(&format!("# Thread {} — client {client} (echo://client/{}), stage {:?}\n\n", p.name, p.client.as_deref().unwrap_or("-"), p.stage));
            let brief = std::fs::read_to_string(crate::projects::dir().join(&id).join("brief.md")).unwrap_or_default();
            if !brief.trim().is_empty() {
                out.push_str(&format!("{}\n\n", brief.chars().take(3000).collect::<String>()));
            }
            let v = crate::people::of_thread(&id, chrono::Local::now().date_naive())?;
            out.push_str("## People\n");
            for x in v["people"].as_array().into_iter().flatten().filter(|x| x["in_thread"] == true || x["calls"].as_u64().unwrap_or(0) > 0) {
                let role = x["places"].as_array().into_iter().flatten().find(|pl| pl["thread"] == id.as_str()).and_then(|pl| pl["role"].as_str()).unwrap_or("");
                out.push_str(&format!("- [{}](echo://person/{}){}{}\n", x["name"].as_str().unwrap_or(""), x["key"].as_str().unwrap_or(""), x["org"]["name"].as_str().map(|o| format!(" ({o})")).unwrap_or_default(), if role.is_empty() { String::new() } else { format!(": {role}") }));
            }
            out.push_str("\n## Calls\n");
            for (d, _) in crate::projects::calls_of(&p)?.into_iter().rev().take(40) {
                let meta = crate::calls::read_json(&d.join("meta.json"));
                let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
                out.push_str(&format!("- [{} {}](echo://call/{name})\n", meta["title"].as_str().unwrap_or(&name), meta["started_at"].as_str().and_then(|s| s.get(..10)).unwrap_or("")));
            }
        }
        Link::Client(id) => {
            let o = crate::projects::orgs()?.into_iter().find(|o| o.id == id).ok_or_else(|| anyhow!("no client {id:?}"))?;
            out.push_str(&format!("# {} — {}{}\n\n## Threads\n", o.name, if o.account { "client" } else { "organisation" }, if o.domains.is_empty() { String::new() } else { format!(", {}", o.domains.join(", ")) }));
            for p in crate::projects::list().into_iter().filter(|p| p.client.as_deref() == Some(id.as_str())) {
                out.push_str(&format!("- [{}](echo://thread/{}) — {:?}\n", p.name, p.id, p.stage));
            }
            let desc = std::fs::read_to_string(crate::projects::dir().join("clients").join(format!("{id}.md"))).unwrap_or_default();
            if !desc.trim().is_empty() {
                out.push_str(&format!("\n## About\n{}\n", desc.chars().take(3000).collect::<String>()));
            }
        }
        Link::Person(key) => {
            let threads: Vec<String> = crate::projects::list().into_iter().map(|p| p.id).collect();
            let v = crate::people::card(&threads, &key, 40)?;
            if v["status"] == "not_found" || v.get("error").is_some() {
                bail!("no person {key:?} in People");
            }
            out.push_str(&person_text(&v));
        }
        Link::Mail(src) => out.push_str(&mail_text(&src)?),
        Link::Task { thread, id } => out.push_str(&task_text(&thread, &id)?),
        Link::Prep(id) => out.push_str(&format!("# Prep\n\n{}", crate::prep::as_text(&crate::prep::load(&id).ok_or_else(|| anyhow!("no prep {id:?}"))?))),
    }
    Ok(out)
}

/// A work item as text: its kind, title, state, owners, due, description, and where it was said (links).
fn task_text(thread: &str, id: &str) -> Result<String> {
    let t = crate::ledger::tree(thread).map_err(|_| anyhow!("no thread {thread:?}"))?;
    let n = t["nodes"].get(id).filter(|n| n.is_object()).ok_or_else(|| anyhow!("no item {id} in {thread}"))?;
    let mut out = format!("# {} — {} ({})\nThread: echo://thread/{thread}\n", n["title"].as_str().unwrap_or(""), n["kind"].as_str().unwrap_or(""), n["state"].as_str().unwrap_or(""));
    for (k, l) in [("internal_owner", "Owner"), ("client_owner", "Client owner"), ("due", "Due")] {
        if let Some(v) = n[k].as_str() {
            out.push_str(&format!("{l}: {v}\n"));
        }
    }
    if let Some(d) = n["description"].as_str() {
        out.push_str(&format!("\n{d}\n"));
    }
    let ev = &t["evidence"];
    let said: Vec<String> = n["history"].as_array().into_iter().flatten().filter_map(|h| {
        let e = &ev[h["evidence"].as_str()?];
        let quote = h["quote"].as_str().filter(|q| !q.is_empty())?;
        // A call's moment: the turn's time (hh:mm:ss) on its link.
        let secs = h["ts"].as_str().map(|t| t.split(':').fold(0u64, |a, x| a * 60 + x.parse::<u64>().unwrap_or(0)));
        let link = match (e["link"].as_str(), secs) {
            (Some(l), Some(t)) => Some(format!("{l}?t={t}")),
            (Some(l), None) => Some(l.to_string()),
            _ => e["url"].as_str().map(str::to_owned),
        };
        Some(format!("- {}{} — “{quote}” ({}, {}){}", h["op"].as_str().unwrap_or(""), h["effect"].as_str().map(|x| format!(" {x}")).unwrap_or_default(), e["title"].as_str().unwrap_or(""), e["date"].as_str().unwrap_or(""), link.map(|l| format!(" {l}")).unwrap_or_default()))
    }).take(20).collect();
    if !said.is_empty() {
        out.push_str(&format!("\n## Where it was said\n{}\n", said.join("\n")));
    }
    // Who does it (`taskmodes`): the user's choice, else Echo's guess — said which, and on one line: a guess is model
    // output from others' words, a suggestion, never the user's instruction.
    let m = &n["who_does"];
    let line = |v: &Value| v.as_str().map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty());
    let whose = |by: &Value| if by == "you" { "the user's" } else { "Echo's guess, a suggestion — not the user's words" };
    match m["mode"].as_str() {
        Some("claude") => out.push_str(&format!(
            "\n## Who does it\nClaude can ({}){}{}\n",
            whose(&m["by"]),
            line(&m["prompt"]).map(|p| format!("; to hand back ({}): {p}", whose(&m["prompt_by"]))).unwrap_or_default(),
            m["skill"].as_str().map(|k| format!("; skill ({}): /my-skills:{k}", whose(&m["skill_by"]))).unwrap_or_default()
        )),
        Some(_) => out.push_str(&format!("\n## Who does it\nThe user, by hand ({}){}\n", whose(&m["by"]), line(&m["how"]).map(|h| format!("; how ({}): {h}", whose(&m["how_by"]))).unwrap_or_default())),
        None => {}
    }
    // The context its work starts from: the thread's last calls, and the last of the meeting series it was raised in
    // (by the invite's subject, as "Since last time" goes) that the thread's don't already list.
    // Only calls on the thread's own AI profile: a call's text goes nowhere else (strict routing).
    let profile = crate::projects::load(thread).ok().and_then(|p| p.ai_profile).unwrap_or_else(crate::ai::default_id);
    let ours = |d: &std::path::Path| crate::ai::call_profile_id(d).is_ok_and(|p| p == profile);
    let recent: Vec<std::path::PathBuf> = recent_calls(thread, &ours, 3);
    if !recent.is_empty() {
        out.push_str(&format!("\n## The thread's last calls\n{}\n", recent.iter().map(|d| call_line(d)).collect::<Vec<_>>().join("\n")));
    }
    let raised_in = n["history"].as_array().into_iter().flatten().find_map(|h| ev[h["evidence"].as_str()?]["call"].as_str().map(str::to_owned));
    if let Some((subject, series)) = raised_in.and_then(|c| series_calls(&crate::paths::calls_dir().join(c), &|d| ours(d) && !recent.iter().any(|r| r == d), 3)) {
        let more: Vec<String> = series.iter().map(|d| call_line(d)).collect();
        if !more.is_empty() {
            out.push_str(&format!("\n## The last calls of its series ({subject})\n{}\n", more.join("\n")));
        }
    }
    Ok(out)
}

/// A call as one line: its title, date and link (the reader opens the ones it needs).
fn call_line(d: &std::path::Path) -> String {
    let name = d.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    let meta = crate::calls::read_json(&d.join("meta.json"));
    format!("- {} ({}) echo://call/{name}", meta["title"].as_str().unwrap_or(&name), name.get(..10).unwrap_or(""))
}

/// A thread's last `n` calls with minutes that `keep` keeps, newest first.
fn recent_calls(thread: &str, keep: &dyn Fn(&std::path::Path) -> bool, n: usize) -> Vec<std::path::PathBuf> {
    let Ok(p) = crate::projects::load(thread) else { return vec![] };
    let mut calls: Vec<std::path::PathBuf> = crate::projects::calls_of(&p).unwrap_or_default().into_iter().map(|(d, _)| d).filter(|d| d.join("notes.md").is_file()).collect();
    calls.sort();
    calls.into_iter().rev().filter(|d| keep(d)).take(n).collect()
}

/// The last `n` calls with minutes of `dir`'s meeting series — the same invite subject; unlike "Since last time", a
/// call with no invite (an ad-hoc one, its title only) has no series here: a generic title would gather strangers —
/// newest first among the last 300 calls, with that subject.
fn series_calls(dir: &std::path::Path, keep: &dyn Fn(&std::path::Path) -> bool, n: usize) -> Option<(String, Vec<std::path::PathBuf>)> {
    let invite = |d: &std::path::Path| crate::meeting::MeetingContext::load(d).ok().flatten().and_then(|m| m.subject).filter(|s| !s.trim().is_empty());
    let subject = invite(dir)?;
    let want = subject.trim().to_lowercase();
    let calls = crate::calls::all_calls()
        .into_iter()
        .rev()
        .take(300)
        .filter(|d| d.join("notes.md").is_file() && invite(d).is_some_and(|s| s.trim().to_lowercase() == want) && keep(d))
        .take(n)
        .collect();
    Some((subject, calls))
}

fn person_text(v: &Value) -> String {
    let mut out = format!("# {}\n", v["name"].as_str().unwrap_or(""));
    let line: Vec<String> = ["title", "organisation", "email"].iter().filter_map(|k| v[*k].as_str().map(str::to_owned)).collect();
    out.push_str(&format!("{}\n", line.join(" · ")));
    if let Some(a) = v["about"].as_str() {
        out.push_str(&format!("\n{a}\n"));
    }
    out.push_str("\n## Threads\n");
    for p in v["places"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}{}\n", p["thread"].as_str().unwrap_or(""), p["role"].as_str().map(|r| format!(": {r}")).unwrap_or_default()));
    }
    for t in v["threads"].as_array().into_iter().flatten().filter(|t| !v["places"].as_array().into_iter().flatten().any(|p| p["thread"] == t["name"])) {
        out.push_str(&format!("- {} (seen)\n", t["name"].as_str().unwrap_or("")));
    }
    let with: Vec<String> = v["often_with"].as_array().into_iter().flatten().filter_map(|y| y["name"].as_str().map(str::to_owned)).collect();
    if !with.is_empty() {
        out.push_str(&format!("\nOften in calls with: {}\n", with.join(", ")));
    }
    let facts = v["facts"].as_array().cloned().unwrap_or_default();
    if !facts.is_empty() {
        out.push_str("\n## What was said about them\n");
        for f in facts {
            out.push_str(&format!("- {} — “{}” ({}, {}){}\n", f["fact"].as_str().unwrap_or(""), f["quote"].as_str().unwrap_or(""), f["source"].as_str().unwrap_or(""), f["date"].as_str().unwrap_or(""), f["link"].as_str().map(|l| format!(" {l}")).unwrap_or_default()));
        }
    }
    out
}

#[cfg(feature = "brain")]
fn mail_text(src: &str) -> Result<String> {
    let c = crate::brain::store::open(&crate::brain::store_path())?;
    let (title, date, refs): (Option<String>, Option<String>, Option<String>) = c
        .query_row("SELECT title, event_date, refs FROM documents WHERE source = ?1", [src], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(|_| anyhow!("no mail {src:?}"))?;
    let url = refs.as_deref().and_then(|r| serde_json::from_str::<Value>(r).ok()).and_then(|r| r["url"].as_str().map(str::to_owned)).unwrap_or_default();
    let mut out = format!("# {} — {}\nOpen: {url}\n\n", title.unwrap_or_default(), date.unwrap_or_default());
    let mut st = c.prepare("SELECT text FROM chunks WHERE source = ?1 ORDER BY ord")?;
    for t in st.query_map([src], |r| r.get::<_, String>(0))? {
        out.push_str(&t?);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(not(feature = "brain"))]
fn mail_text(src: &str) -> Result<String> {
    bail!("mail {src:?}: this build reads no mail")
}

/// Where the window goes for a link: a route (`#…`) with the moment to scroll to and the person whose panel opens, or
/// an https link to open outside Echo (a mail in Outlook). An error says why it can't go there.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn target(link: &str) -> Result<Value> {
    let l = Link::parse(link).ok_or_else(|| anyhow!("not an echo:// link Echo knows"))?;
    Ok(match l {
        Link::Call { name, t } => {
            let dir = call_dir(&name)?;
            let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
            serde_json::json!({"hash": format!("call/{name}/{}", if t.is_some() { "transcript" } else { "minutes" }), "jump": t})
        }
        Link::Thread(id) => {
            crate::projects::load(&id).map_err(|_| anyhow!("no thread {id:?}"))?;
            serde_json::json!({"hash": format!("project/{id}/brief")})
        }
        Link::Client(id) => {
            crate::projects::orgs()?.into_iter().find(|o| o.id == id).ok_or_else(|| anyhow!("no client {id:?}"))?;
            serde_json::json!({"hash": format!("client/{id}/overview")})
        }
        Link::Person(key) => {
            let t = person_thread(&key).ok_or_else(|| anyhow!("{key:?} is in no thread"))?;
            serde_json::json!({"hash": format!("project/{t}/people"), "person": key})
        }
        Link::Mail(src) => serde_json::json!({"url": mail_url(&src).ok_or_else(|| anyhow!("Echo has no link to that mail"))?}),
        Link::Task { thread, id } => {
            crate::projects::load(&thread).map_err(|_| anyhow!("no thread {thread:?}"))?;
            serde_json::json!({"hash": format!("project/{thread}/work"), "node": id})
        }
        // A recorded meeting: its call's Prep tab; one still to come: Calls → Coming up today.
        Link::Prep(id) => {
            crate::prep::load(&id).ok_or_else(|| anyhow!("no prep {id:?}"))?;
            match crate::calls::all_calls().into_iter().find(|d| crate::prep::of_call(d).is_some_and(|p| p["id"] == id)) {
                Some(d) => serde_json::json!({"hash": format!("call/{}/prep", d.file_name().unwrap_or_default().to_string_lossy())}),
                None => serde_json::json!({"hash": format!("upcoming/{id}")}),
            }
        }
    })
}

/// Where a mail link opens outside Echo: its message in Outlook or Teams, when Echo knows it.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
#[cfg(feature = "brain")]
pub fn mail_url(src: &str) -> Option<String> {
    let c = crate::brain::store::open(&crate::brain::store_path()).ok()?;
    let refs: Option<String> = c.query_row("SELECT refs FROM documents WHERE source = ?1", [src], |r| r.get(0)).ok()?;
    serde_json::from_str::<Value>(&refs?).ok()?["url"].as_str().filter(|u| u.starts_with("https://")).map(str::to_owned)
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
#[cfg(not(feature = "brain"))]
pub fn mail_url(_src: &str) -> Option<String> {
    None
}

/// The thread a person is placed in or was seen in first (their People panel opens there), else none.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn person_thread(key: &str) -> Option<String> {
    let v = crate::people::everyone(chrono::Local::now().date_naive()).ok()?;
    let x = v["people"].as_array()?.iter().find(|x| x["key"] == key || x["email"].as_str().is_some_and(|e| e.eq_ignore_ascii_case(key)))?;
    x["places"][0]["thread"].as_str().or_else(|| x["threads"][0].as_str()).map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_series_is_its_calls_with_minutes_by_the_invites_subject() {
        let (_env, _) = crate::paths::testenv::folders("links-series");
        let mk = |name: &str, title: &str, notes: bool| {
            let d = crate::paths::calls_dir().join(name);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("meta.json"), "{}").unwrap();
            std::fs::write(d.join("meeting.json"), serde_json::json!({"subject": title}).to_string()).unwrap();
            if notes {
                std::fs::write(d.join("notes.md"), "# x").unwrap();
            }
            d
        };
        let first = mk("2026-10-01_100000-acme-weekly", "Acme weekly", true);
        mk("2026-10-08_100000-acme-weekly", "Acme weekly", true);
        mk("2026-10-09_100000-other", "Other", true);
        mk("2026-10-15_100000-acme-weekly", "acme weekly", false);
        mk("2026-10-22_100000-acme-weekly", "Acme Weekly", true);
        let adhoc = crate::paths::calls_dir().join("2026-10-23_100000-adhoc");
        std::fs::create_dir_all(&adhoc).unwrap();
        std::fs::write(adhoc.join("meta.json"), serde_json::json!({"title": "Acme weekly"}).to_string()).unwrap();
        assert!(series_calls(&adhoc, &|_| true, 3).is_none(), "no invite, no series");
        let (subject, calls) = series_calls(&first, &|_| true, 3).unwrap();
        assert_eq!(subject, "Acme weekly");
        let names: Vec<String> = calls.iter().map(|d| d.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["2026-10-22_100000-acme-weekly", "2026-10-08_100000-acme-weekly", "2026-10-01_100000-acme-weekly"], "newest first, with minutes, any case");
        // Those the thread lists already are passed over before counting: the older ones come in.
        let (_, older) = series_calls(&first, &|d| !d.ends_with("2026-10-22_100000-acme-weekly"), 1).unwrap();
        assert!(older[0].ends_with("2026-10-08_100000-acme-weekly"));
    }

    #[test]
    fn a_task_says_who_does_it() {
        let (_env, _) = crate::paths::testenv::folders("links-task-mode");
        let id = crate::projects::create("Aurora", None, None).unwrap().id;
        let st = crate::ledger::human_event(&id, &serde_json::json!({"op": "create", "kind": "story", "parent": "E01", "title": "Pilot"})).unwrap();
        let t = crate::ledger::human_event(&id, &serde_json::json!({"op": "create", "kind": "task", "parent": st["id"], "title": "Book the room"})).unwrap();
        let tid = t["id"].as_str().unwrap();
        crate::ledger::human_event(&id, &serde_json::json!({"op": "edit", "node": tid, "set": {"mode": "manual", "how": "Call the venue"}})).unwrap();
        let text = task_text(&id, tid).unwrap();
        assert!(text.contains("## Who does it\nThe user, by hand (the user's); how (the user's): Call the venue"), "{text}");
    }

    #[test]
    fn links_parse_and_refuse_paths() {
        assert_eq!(Link::parse("echo://call/2026-11-03_180000-acme-sync?t=725"), Some(Link::Call { name: "2026-11-03_180000-acme-sync".into(), t: Some(725) }));
        assert_eq!(Link::parse("echo://call/2026-11-03_180000-acme-sync"), Some(Link::Call { name: "2026-11-03_180000-acme-sync".into(), t: None }));
        assert_eq!(Link::parse("echo://thread/acme-pilot"), Some(Link::Thread("acme-pilot".into())));
        assert_eq!(Link::parse("echo://person/jane%40acme.example"), Some(Link::Person("jane@acme.example".into())));
        assert_eq!(Link::parse("echo://mail/mail/22c3f6bf0128d167"), Some(Link::Mail("mail/22c3f6bf0128d167".into())));
        assert_eq!(Link::parse("echo://mail/channel/abc/123"), Some(Link::Mail("channel/abc/123".into())));
        assert_eq!(Link::parse("echo://task/acme-pilot/T043"), Some(Link::Task { thread: "acme-pilot".into(), id: "T043".into() }));
        assert_eq!(Link::parse("echo://task/acme-pilot"), None);
        assert_eq!(Link::parse("echo://task/acme-pilot/T0/../x"), None);
        for bad in ["echo://call/../../etc", "echo://call/a/b", "echo://thread/", "echo://mail/../x", "echo://nope/x", "https://call/x", "echo://person/%2e%2e"] {
            assert_eq!(Link::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn markdown_keeps_echo_links_echo_knows() {
        let h = crate::minutes::markdown_html("[the call](echo://call/2026-10-28_110000-aurora?t=725), [bad](echo://call/../x), [web](https://example.com), [js](javascript:alert(1))");
        assert!(h.contains("<a href=\"echo://call/2026-10-28_110000-aurora?t=725\" data-echo-link>the call</a>"), "{h}");
        assert!(!h.contains("echo://call/../x") && !h.contains("javascript:"), "{h}");
        assert!(h.contains("href=\"https://example.com\""));
    }

    #[test]
    fn links_say_where_the_window_goes() {
        let (_env, _root) = crate::paths::testenv::folders("links-target");
        crate::projects::add_client("Acme", &["acme.example".into()], false).unwrap();
        let pilot = crate::projects::create("Acme pilot", Some("acme"), None).unwrap().id;
        crate::people::place(&pilot, "Jane Doe", Some("jane@acme.example"), &serde_json::json!({"role": "Sponsor"})).unwrap();
        let d = crate::paths::calls_dir().join("2026-10-28_110000-aurora");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), r#"{"title": "Aurora"}"#).unwrap();
        assert_eq!(target("echo://call/2026-10-28_110000-aurora?t=725").unwrap(), serde_json::json!({"hash": "call/2026-10-28_110000-aurora/transcript", "jump": 725}));
        assert_eq!(target(&format!("echo://thread/{pilot}")).unwrap()["hash"], format!("project/{pilot}/brief"));
        assert_eq!(target("echo://client/acme").unwrap()["hash"], "client/acme/overview");
        let p = target("echo://person/jane%40acme.example").unwrap();
        assert_eq!((p["hash"].as_str(), p["person"].as_str()), (Some(format!("project/{pilot}/people").as_str()), Some("jane@acme.example")), "{p}");
        assert!(target("echo://thread/nope").is_err() && target("echo://client/nope").is_err());
    }

    #[test]
    fn a_call_link_reads_its_minutes_and_the_moment() {
        let (_env, _root) = crate::paths::testenv::folders("links-read");
        let d = crate::paths::calls_dir().join("2026-10-28_110000-aurora");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), serde_json::json!({"title": "Aurora weekly", "started_at": "2026-10-28T11:00:00+02:00"}).to_string()).unwrap();
        std::fs::write(d.join("notes.md"), "# Aurora weekly\n\n## Decisions\n- SSO lands first [12:05].\n").unwrap();
        std::fs::write(d.join("segments.json"), serde_json::json!([
            {"speaker": "remote", "name": "Varga, Lena (Partner)", "start": 725.0, "text": "Then SSO has to land first."},
            {"speaker": "remote", "name": "Varga, Lena (Partner)", "start": 2000.0, "text": "Later talk."}
        ]).to_string()).unwrap();
        let t = read("echo://call/2026-10-28_110000-aurora?t=720").unwrap();
        assert!(t.contains("Aurora weekly") && t.contains("[12:05] Lena Varga: Then SSO has to land first.") && !t.contains("Later talk"), "{t}");
        assert!(t.contains("SSO lands first"), "the minutes too: {t}");
        assert!(read("echo://call/2026-10-28_110000-nope").is_err());
    }
}

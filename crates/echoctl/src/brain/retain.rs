//! Retention (intake-and-search R7): the index is working memory, not an archive. A section leaves it past its kind's
//! horizon, or when every thread it is in is closed; a call's card stays always, and a closed thread's document keeps
//! a card (title, path, headings) in place of its text. Only index records go: the call folders, the thread folders'
//! files and Intake's facts are never touched, and a thread opened again gets its sections back at the next sync.

use anyhow::Result;
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::store;

/// Transcript turns and the meeting chat: kept this many days.
pub const TALK_DAYS: i64 = 45;
/// A call's minutes sections.
pub const MINUTES_DAYS: i64 = 90;
/// Mail and channel facts (each message's own date).
pub const FACTS_DAYS: i64 = 90;
/// What a search past those horizons says.
pub const OLDER: &str = "older than the index keeps: see the call folders or Knowledge";

/// How many days a section kind is kept (none: while its thread is open — documents — or always — cards).
pub fn horizon(section: &str) -> Option<i64> {
    match section {
        "transcript" | "chat" => Some(TALK_DAYS),
        "summary" | "decisions" | "action_items" | "open_questions" | "minutes" => Some(MINUTES_DAYS),
        "mail" | "channel" => Some(FACTS_DAYS),
        _ => None,
    }
}

/// A section's day: a mail or channel fact's own (its heading starts "YYYY-MM-DD HH:MM"), else its document's.
fn day_of(section: &str, title: &str, event_date: Option<&str>) -> Option<NaiveDate> {
    let own = matches!(section, "mail" | "channel").then(|| title.get(..10)).flatten();
    own.or_else(|| event_date.and_then(|d| d.get(..10))).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
}

/// A document's mark while its sections are out for its closed threads: those threads.
fn closed_mark(refs: &Value, closed: &BTreeMap<String, Value>) -> String {
    let ids: BTreeSet<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).filter(|t| closed.contains_key(*t)).collect();
    format!("closed:{}", ids.into_iter().collect::<Vec<_>>().join(","))
}

/// Before a sync: a document whose sections went for threads that are no longer all closed (one opened again) leaves
/// the index whole, so the sync reads it again — a call from its folder, a document from its file, mail and posts from
/// their Intake facts at the next read. Returns how many.
pub fn restore(path: &Path, closed: &BTreeMap<String, Value>) -> Result<usize> {
    let c = store::open(path)?;
    let docs: Vec<(String, Option<String>, String)> = c
        .prepare("SELECT source, refs, retained FROM documents WHERE retained LIKE 'closed:%'")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let tx = c.unchecked_transaction()?;
    let mut n = 0;
    for (src, refs, mark) in docs {
        let refs: Value = refs.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
        if closed_mark(&refs, closed) == mark {
            continue;
        }
        let ids: Vec<i64> = tx.prepare("SELECT id FROM chunks WHERE source = ?1")?.query_map([&src], |r| r.get(0))?.collect::<Result<_, _>>()?;
        store::delete_chunks(&tx, &ids)?;
        tx.execute("DELETE FROM documents WHERE source = ?1", [&src])?;
        // A folder's file is read again (its registry says it isn't indexed).
        if let Some(file) = refs["file"].as_str() {
            tx.execute("UPDATE files SET indexed_sha = NULL WHERE id = ?1", [file]).ok();
        }
        n += 1;
    }
    tx.commit()?;
    Ok(n)
}

/// After a sync: every section past its horizon on `today`, and every section only in closed threads, leaves the index
/// (cards stay; a closed thread's document gets its card first). Returns how many sections left.
pub fn apply(path: &Path, today: NaiveDate, closed: &BTreeMap<String, Value>) -> Result<usize> {
    let c = store::open(path)?;
    let docs: Vec<(String, String, Option<String>, Option<String>)> = c
        .prepare("SELECT source, coalesce(title, ''), refs, event_date FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let tx = c.unchecked_transaction()?;
    let mut gone = 0;
    for (src, title, refs, date) in docs {
        let refs: Value = refs.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
        gone += trim(&tx, &src, &title, &refs, date.as_deref(), today, closed)?;
    }
    tx.commit()?;
    Ok(gone)
}

/// One document's sections past their horizon or only in closed threads out (see `apply`).
fn trim(tx: &Connection, src: &str, title: &str, refs: &Value, date: Option<&str>, today: NaiveDate, closed: &BTreeMap<String, Value>) -> Result<usize> {
    let rows: Vec<(i64, String, String, Option<String>)> = tx
        .prepare("SELECT id, coalesce(section, ''), coalesce(title, ''), event_date FROM chunks WHERE source = ?1 ORDER BY ord")?
        .query_map([src], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
    let archived = store::archived_of(tx, &ids, closed)?;
    let (mut out, mut for_closed) = (vec![], false);
    for (id, section, head, day) in &rows {
        if section == "card" {
            continue;
        }
        if archived.contains_key(id) {
            out.push(*id);
            for_closed = true;
        } else if let (Some(days), Some(d)) = (horizon(section), day_of(section, head, day.as_deref())) {
            if (today - d).num_days() > days {
                out.push(*id);
            }
        }
    }
    if out.is_empty() {
        return Ok(0);
    }
    // A closed thread's document: its card (title, path, threads, headings) before its text goes.
    if for_closed && src.starts_with("file/") {
        let mut heads: Vec<String> = vec![];
        for (_, _, h, _) in &rows {
            let h = h.split(" (part ").next().unwrap_or(h).trim().to_string();
            if !h.is_empty() && !heads.contains(&h) && heads.len() < CARD_HEADINGS {
                heads.push(h);
            }
        }
        let threads: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().or(t["id"].as_str())).collect();
        let path = refs["path"].as_str().or(refs["rel"].as_str()).unwrap_or("");
        let text = format!("Document: {title}\nPath: {path}\nThreads: {}\nHeadings: {}", threads.join(", "), heads.join(" · "));
        let link = format!("echo://file/{}", refs["file"].as_str().unwrap_or(""));
        store::put_card(tx, src, title, &text, date, &link, refs)?;
    }
    store::delete_chunks(tx, &out)?;
    if for_closed {
        tx.execute("UPDATE documents SET retained = ?2 WHERE source = ?1", params![src, closed_mark(refs, closed)])?;
    }
    store::doc_facts(tx, src)?;
    Ok(out.len())
}

/// The most headings a document's card lists.
const CARD_HEADINGS: usize = 40;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::{self, facts, store};
    use crate::projects::{self, Stage};
    use serde_json::json;

    /// A call `days` ago, in `thread` when given: minutes (summary, decisions), one transcript turn, its card.
    fn call(name: &str, days: i64, thread: Option<&str>) -> std::path::PathBuf {
        let start = chrono::Local::now() - chrono::Duration::days(days);
        let d = crate::paths::calls_dir().join(format!("{}-{name}", start.format("%Y-%m-%d_%H%M%S")));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), json!({"title": format!("{name} weekly"), "started_at": start.to_rfc3339()}).to_string()).unwrap();
        std::fs::write(d.join("notes.md"), format!("# {name} weekly\n\n## Summary\nThe {name} gateway goes first.\n\n## Decisions\n- Ship the {name} gateway.\n")).unwrap();
        std::fs::write(d.join("segments.json"), json!([{"speaker": "remote", "name": "Dana Ruiz", "start": 5.0, "text": format!("The gateway for {name} is ready.")}]).to_string()).unwrap();
        if let Some(t) = thread {
            projects::set_membership(t, &d, "include").unwrap();
        }
        d
    }

    fn source_of(dir: &std::path::Path) -> String {
        format!("call/{}", crate::calls::stamp_of(dir))
    }

    /// A source's sections in the index: (id, kind), in order.
    fn sections(src: &str) -> Vec<(i64, String)> {
        let c = store::open(&brain::store_path()).unwrap();
        let mut st = c.prepare("SELECT id, section FROM chunks WHERE source = ?1 ORDER BY ord").unwrap();
        st.query_map([src], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|x| x.unwrap()).collect()
    }

    fn kinds(src: &str) -> Vec<String> {
        sections(src).into_iter().map(|s| s.1).collect()
    }

    /// Every file under `dir` but the index's own, with its bytes.
    fn files(dir: &std::path::Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().is_dir() {
                out.extend(files(&e.path()));
            } else if !e.file_name().to_string_lossy().starts_with("knowledge.sqlite") {
                out.insert(e.path(), std::fs::read(e.path()).unwrap());
            }
        }
        out
    }

    fn on() {
        brain::save_settings(&brain::embed::Settings { enabled: Some(true), ..Default::default() }).unwrap();
    }

    #[test]
    fn past_their_horizons_only_index_records_go_and_every_call_keeps_its_card() {
        let (_env, _root) = crate::paths::testenv::folders("recall-retain-ages");
        on();
        let acme = projects::create("Acme rollout", None, None).unwrap().id;
        let old = call("old", 100, Some(&acme));
        let mid = call("mid", 60, Some(&acme));
        let new = call("new", 2, Some(&acme));
        brain::sync().unwrap();
        // Mail: one message 100 days old, one 10 days old.
        let day = |n: i64| (chrono::Local::now() - chrono::Duration::days(n)).format("%Y-%m-%d").to_string();
        let section = |n: i64, what: &str| facts::Section { head: format!("{} 09:00 Jane Doe", day(n)), from: Some("Jane Doe".into()), facts: vec![brain::digest::Fact { text: what.into(), quote: None }], ..Default::default() };
        let mail = facts::Facts::new("mail/acme-gateway".into(), "mail", "Gateway".into(), Some(day(10)), json!({"kind": "mail", "threads": [{"id": acme, "client": null}]}),
                                     vec![section(100, "The old gateway plan."), section(10, "The new gateway plan.")]);
        facts::Writer::default().write(&mail).unwrap();
        store::sync_kinds(&brain::store_path(), vec![mail.input()], None, &[]).unwrap();
        let before = files(&crate::paths::home());
        let r = brain::sync().unwrap();
        assert!(r.retired >= 1, "{r:?}");
        assert_eq!(kinds(&source_of(&old)), ["card"], "past 90 days: only its card");
        assert_eq!(kinds(&source_of(&mid)), ["summary", "decisions", "card"], "past 45 days: its minutes and card");
        assert_eq!(kinds(&source_of(&new)), ["summary", "decisions", "transcript", "card"]);
        assert_eq!(sections("mail/acme-gateway").len(), 1, "only the newer message's facts");
        // Nothing on disk changed: the call folders and Intake's facts are as they were.
        assert_eq!(files(&crate::paths::home()), before);
        assert!(old.join("segments.json").is_file() && facts::path_of("mail/acme-gateway").unwrap().is_file());
        // The card says what the call was.
        let c = store::open(&brain::store_path()).unwrap();
        let card: String = c.query_row("SELECT text FROM chunks WHERE source = ?1 AND section = 'card'", [source_of(&old)], |r| r.get(0)).unwrap();
        assert!(card.contains("Call: old weekly") && card.contains("Threads: Acme rollout") && card.contains("People: Dana Ruiz") && card.contains("Summary: The old gateway goes first."), "{card}");
        // Again: nothing more leaves, nothing is read again.
        let r = brain::sync().unwrap();
        assert_eq!((r.retired, r.added, r.changed), (0, 0, 0), "{r:?}");
    }

    #[test]
    fn a_closed_threads_records_leave_but_its_cards_and_come_back_with_the_same_ids() {
        let (_env, root) = crate::paths::testenv::folders("recall-retain-closed");
        on();
        let hooli = projects::create("Hooli workshop", None, None).unwrap().id;
        let open_t = projects::create("Acme rollout", None, None).unwrap().id;
        let shut = call("hooli", 2, Some(&hooli));
        let kept = call("acme", 3, Some(&open_t));
        // A folder of the closed thread: its document.
        let folder = root.join("hooli-docs");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("plan.md"), "# Plan\n\n## Gateway rollout\nThe gateway goes in first.\n\n## Budget\nApproved by finance.\n").unwrap();
        projects::update(&hooli, |p| {
            p.sources.push(projects::Source { kind: "folder".into(), id: folder.to_string_lossy().into(), ..Default::default() });
            Ok(())
        })
        .unwrap();
        brain::sync().unwrap();
        let doc: String = store::open(&brain::store_path()).unwrap().query_row("SELECT source FROM documents WHERE source LIKE 'file/%'", [], |r| r.get(0)).unwrap();
        let (call_ids, doc_ids) = (sections(&source_of(&shut)), sections(&doc));
        assert!(!doc_ids.is_empty() && doc_ids.iter().all(|s| s.1 == "document"), "{doc_ids:?}");

        projects::set_stage(&hooli, Stage::Done, Some("wrapped up")).unwrap();
        brain::sync().unwrap();
        assert_eq!(kinds(&source_of(&shut)), ["card"], "a closed thread's call: its card only");
        assert_eq!(sections(&source_of(&shut))[0], *call_ids.last().unwrap(), "the card keeps its id");
        assert_eq!(kinds(&source_of(&kept)).len(), 4, "an open thread's call is whole");
        assert_eq!(sections(&doc), [(crate::brain::chunk::chunk_id(&doc, store::CARD_ORD), "card".to_string())], "a closed thread's document: its card");
        let h = store::search_in(&brain::store_path(), "gateway rollout budget", 5, None, None, None).unwrap();
        let card = h["hits"].as_array().unwrap().iter().find(|x| x["source"] == doc.as_str()).expect("the card is found").clone();
        let text = card["text"].as_str().unwrap();
        assert!(text.contains("Headings: Gateway rollout · Budget") && text.contains("plan.md") && text.contains("Hooli workshop") && card["closed"].is_array(), "{card}");
        assert!(folder.join("plan.md").is_file(), "the file stays");

        // Opened again: everything back, with the ids it had (R10).
        projects::set_stage(&hooli, Stage::Active, Some("back")).unwrap();
        brain::sync().unwrap();
        assert_eq!(sections(&source_of(&shut)), call_ids);
        assert_eq!(sections(&doc), doc_ids);
        let r = brain::sync().unwrap();
        assert_eq!((r.retired, r.added, r.changed), (0, 0, 0), "{r:?}");
    }
}

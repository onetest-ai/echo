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

/// Whether a section of `section` kind dated `day` (YYYY-MM-DD; none: never guessed) is within its horizon on `today`.
pub fn kept(section: &str, day: Option<&str>, today: NaiveDate) -> bool {
    let d = day.and_then(|d| d.get(..10)).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    match (horizon(section), d) {
        (Some(days), Some(d)) => (today - d).num_days() <= days,
        _ => true,
    }
}

/// A document's mark while its sections are out for its closed threads: those threads.
pub(super) fn closed_mark(refs: &Value, closed: &BTreeMap<String, Value>) -> String {
    let ids: BTreeSet<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).filter(|t| closed.contains_key(*t)).collect();
    format!("closed:{}", ids.into_iter().collect::<Vec<_>>().join(","))
}

/// What the last sweep was for: its day and the closed threads (`meta.retained_for`). The same again: no sweep.
fn sweep_key(today: NaiveDate, closed: &BTreeMap<String, Value>) -> String {
    format!("{today}|{}", closed.keys().cloned().collect::<Vec<_>>().join(","))
}

/// Whether a sweep is due: once a day, and when a thread closed or opened again since the last.
pub fn due(path: &Path, today: NaiveDate, closed: &BTreeMap<String, Value>) -> Result<bool> {
    Ok(store::meta(&store::open(path)?, "retained_for").as_deref() != Some(sweep_key(today, closed).as_str()))
}

/// Before a sync: a document whose sections went for threads that are no longer all closed (one opened again) leaves
/// the index whole, so the sync reads it again — a call from its folder, a document from its file, mail and posts from
/// their Intake facts (`sources::feed_missing`). Returns how many.
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

/// A section as the sweep reads it: id, source, kind, title, day.
type Row = (i64, String, String, String, Option<String>);

/// The sweep, when `due`: every section past its horizon on `today`, and every section only in closed threads, leaves
/// the index (cards stay; a closed thread's document gets its card first). Every section and its threads are read in
/// one pass each. Returns how many sections left.
pub fn apply(path: &Path, today: NaiveDate, closed: &BTreeMap<String, Value>) -> Result<usize> {
    let c = store::open(path)?;
    let docs: BTreeMap<String, (String, Option<String>, Option<String>)> = c
        .prepare("SELECT source, coalesce(title, ''), refs, event_date FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?
        .collect::<Result<_, _>>()?;
    let mut threads: BTreeMap<i64, Vec<String>> = BTreeMap::new();
    for row in c.prepare("SELECT chunk_id, id FROM scopes WHERE kind = 'thread' AND id != '-'")?.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (id, t) = row?;
        threads.entry(id).or_default().push(t);
    }
    let mut by_doc: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for row in c
        .prepare("SELECT id, source, coalesce(section, ''), coalesce(title, ''), event_date FROM chunks ORDER BY source, ord")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
    {
        let row: Row = row?;
        by_doc.entry(row.1.clone()).or_default().push(row);
    }
    let only_closed = |id: &i64| threads.get(id).is_some_and(|ts| ts.iter().all(|t| closed.contains_key(t)));
    let tx = c.unchecked_transaction()?;
    let mut gone = 0;
    for (src, rows) in &by_doc {
        let Some((title, refs, date)) = docs.get(src) else { continue };
        let (mut out, mut for_closed) = (vec![], false);
        for (id, _, section, _, day) in rows.iter().filter(|r| r.2 != "card") {
            if only_closed(id) {
                out.push(*id);
                for_closed = true;
            } else if !kept(section, day.as_deref(), today) {
                out.push(*id);
            }
        }
        if out.is_empty() {
            continue;
        }
        let refs: Value = refs.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
        if for_closed && src.starts_with("file/") {
            card(&tx, src, title, &refs, date.as_deref(), rows)?;
        }
        store::delete_chunks(&tx, &out)?;
        if for_closed {
            tx.execute("UPDATE documents SET retained = ?2 WHERE source = ?1", params![src, closed_mark(&refs, closed)])?;
        }
        store::doc_facts(&tx, src)?;
        gone += out.len();
    }
    store::set_meta(&tx, "retained_for", &sweep_key(today, closed))?;
    tx.commit()?;
    Ok(gone)
}

/// A closed thread's document's card (title, path, threads, headings), before its text goes.
fn card(tx: &Connection, src: &str, title: &str, refs: &Value, date: Option<&str>, rows: &[Row]) -> Result<()> {
    let mut heads: Vec<String> = vec![];
    for (_, _, _, h, _) in rows {
        let h = h.split(" (part ").next().unwrap_or(h).trim().to_string();
        if !h.is_empty() && !heads.contains(&h) && heads.len() < CARD_HEADINGS {
            heads.push(h);
        }
    }
    let threads: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().or(t["id"].as_str())).collect();
    let path = refs["path"].as_str().or(refs["rel"].as_str()).unwrap_or("");
    let text = format!("Document: {title}\nPath: {path}\nThreads: {}\nHeadings: {}", threads.join(", "), heads.join(" · "));
    let link = format!("echo://file/{}", refs["file"].as_str().unwrap_or(""));
    store::put_card(tx, src, title, &text, date, &link, refs)
}

/// The most headings a document's card lists.
const CARD_HEADINGS: usize = 40;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intake::{self, facts, store};
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
        let c = store::open(&intake::store_path()).unwrap();
        let mut st = c.prepare("SELECT id, section FROM chunks WHERE source = ?1 ORDER BY ord").unwrap();
        st.query_map([src], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|x| x.unwrap()).collect()
    }

    fn kinds(src: &str) -> Vec<String> {
        sections(src).into_iter().map(|s| s.1).collect()
    }

    /// Every file under `dir` but the index's own and the facts move's done mark, with its bytes.
    fn files(dir: &std::path::Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                out.extend(files(&e.path()));
            } else if !name.starts_with("knowledge.sqlite") && name != "migrated.json" {
                out.insert(e.path(), std::fs::read(e.path()).unwrap());
            }
        }
        out
    }

    fn on() {
        intake::save_settings(&intake::embed::Settings { enabled: Some(true), ..Default::default() }).unwrap();
    }

    /// A mail conversation's fact file under `thread`: one message `days` ago per entry.
    fn mail(source: &str, thread: &str, days: &[i64]) -> facts::Facts {
        let at = |n: i64| (chrono::Utc::now() - chrono::Duration::days(n)).format("%Y-%m-%d %H:%M").to_string();
        let sections = days
            .iter()
            .map(|n| facts::Section { head: format!("{} Jane Doe", at(*n)), from: Some("Jane Doe".into()), facts: vec![intake::digest::Fact { text: format!("The gateway plan, {n} days ago."), quote: None }], ..Default::default() })
            .collect();
        let f = facts::Facts::new(source.into(), "mail", "Gateway".into(), Some(at(0)[..10].to_string()), json!({"kind": "mail", "threads": [{"id": thread, "client": null}]}), sections);
        facts::Writer::default().write(&f).unwrap();
        f
    }

    #[test]
    fn past_their_horizons_sections_are_never_written_and_leave_later_and_every_call_keeps_its_card() {
        let (_env, root) = crate::paths::testenv::folders("recall-retain-ages");
        on();
        let acme = projects::create("Acme rollout", None, None).unwrap().id;
        let old = call("old", 100, Some(&acme));
        let mid = call("mid", 60, Some(&acme));
        let new = call("new", 2, Some(&acme));
        mail("mail/acme-gateway", &acme, &[100, 10]);
        // A dictionary entry and a thread folder outside the data folder: checked byte for byte too.
        intake::taxonomy::dictionary_add("Hooli", &[], &[], Some("org"), None, "test").unwrap();
        let outside = root.join("outside-docs");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("plan.md"), "# Plan\n\n## Gateway rollout\nThe gateway goes in first.\n").unwrap();
        projects::update(&acme, |p| {
            p.sources.push(projects::Source { kind: "folder".into(), id: outside.to_string_lossy().into(), ..Default::default() });
            Ok(())
        })
        .unwrap();
        let (home_before, outside_before) = (files(&crate::paths::home()), files(&outside));
        assert!(home_before.keys().any(|p| p.to_string_lossy().contains("taxonomy")), "{:?}", home_before.keys());
        let r = intake::sync().unwrap();
        assert_eq!(kinds(&source_of(&old)), ["card"], "past 90 days: only its card is written");
        assert_eq!(kinds(&source_of(&mid)), ["summary", "decisions", "card"], "past 45 days: its minutes and card");
        assert_eq!(kinds(&source_of(&new)), ["summary", "decisions", "transcript", "card"]);
        assert_eq!(sections("mail/acme-gateway").len(), 1, "only the newer message's facts, from the file with no sign-in");
        assert_eq!(r.retired, 0, "nothing past a horizon was written, so nothing leaves: {r:?}");
        // The call folders, Intake's facts, the dictionary and the outside folder are as they were.
        assert_eq!(files(&crate::paths::home()), home_before);
        assert_eq!(files(&outside), outside_before);
        // The card says what the call was.
        let c = store::open(&intake::store_path()).unwrap();
        let card: String = c.query_row("SELECT text FROM chunks WHERE source = ?1 AND section = 'card'", [source_of(&old)], |r| r.get(0)).unwrap();
        assert!(card.contains("Call: old weekly") && card.contains("Threads: Acme rollout") && card.contains("People: Dana Ruiz") && card.contains("Summary: The old gateway goes first."), "{card}");
        // Again the same day: nothing is read again and no sweep runs.
        let today = chrono::Local::now().date_naive();
        assert!(!due(&intake::store_path(), today, &projects::closed_threads()).unwrap());
        let r = intake::sync().unwrap();
        assert_eq!((r.retired, r.added, r.changed, r.embedded), (0, 0, 0, 0), "{r:?}");
        // 85 days on, the next day's sweep takes what aged since; the cards stay.
        let later = today + chrono::Duration::days(85);
        assert!(due(&intake::store_path(), later, &projects::closed_threads()).unwrap());
        assert!(apply(&intake::store_path(), later, &projects::closed_threads()).unwrap() >= 2);
        assert_eq!(kinds(&source_of(&new)), ["summary", "decisions", "card"]);
        assert_eq!(kinds(&source_of(&mid)), ["card"]);
        assert!(sections("mail/acme-gateway").is_empty());
        assert_eq!(files(&crate::paths::home()), home_before);
        assert_eq!(files(&outside), outside_before);
    }

    #[test]
    fn a_second_sync_with_nothing_changed_counts_every_document_unchanged() {
        let (_env, root) = crate::paths::testenv::folders("recall-sync-unchanged");
        on();
        let acme = projects::create("Acme rollout", None, None).unwrap().id;
        let one = call("one", 2, Some(&acme));
        let two = call("two", 3, None);
        mail("mail/acme-gateway", &acme, &[1]);
        let folder = root.join("acme-docs");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("plan.md"), "# Plan\n\n## Gateway rollout\nThe gateway goes in first.\n").unwrap();
        projects::update(&acme, |p| {
            p.sources.push(projects::Source { kind: "folder".into(), id: folder.to_string_lossy().into(), ..Default::default() });
            Ok(())
        })
        .unwrap();
        let docs = || -> Vec<(String, String, String)> {
            let c = store::open(&intake::store_path()).unwrap();
            let mut st = c.prepare("SELECT source, content_hash, indexed_at FROM documents ORDER BY source").unwrap();
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().map(|x| x.unwrap()).collect()
        };
        let first = intake::sync().unwrap();
        assert_eq!(first.added, 4, "two calls, the mail, the folder's file: {first:?}");
        let before = docs();
        let r = intake::sync().unwrap();
        assert_eq!((r.added, r.changed, r.removed, r.unchanged, r.chunks, r.embedded), (0, 0, 0, 4, 0, 0), "{r:?}");
        assert_eq!(docs(), before, "nothing written again");
        // One call changed, the other deleted: neither is counted unchanged.
        std::fs::write(one.join("notes.md"), "# one weekly\n\n## Summary\nThe one gateway slips a week.\n").unwrap();
        std::fs::remove_dir_all(&two).unwrap();
        let r = intake::sync().unwrap();
        assert_eq!((r.added, r.changed, r.removed, r.unchanged), (0, 1, 1, 2), "{r:?}");
    }

    #[test]
    fn a_closed_threads_records_leave_but_its_cards_and_come_back_with_the_same_ids() {
        let (_env, root) = crate::paths::testenv::folders("recall-retain-closed");
        on();
        let hooli = projects::create("Hooli workshop", None, None).unwrap().id;
        let open_t = projects::create("Acme rollout", None, None).unwrap().id;
        let shut = call("hooli", 2, Some(&hooli));
        let kept = call("acme", 3, Some(&open_t));
        mail("mail/hooli-plan", &hooli, &[1]);
        // A folder of the closed thread: its document.
        let folder = root.join("hooli-docs");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("plan.md"), "# Plan\n\n## Gateway rollout\nThe gateway goes in first.\n\n## Budget\nApproved by finance.\n").unwrap();
        projects::update(&hooli, |p| {
            p.sources.push(projects::Source { kind: "folder".into(), id: folder.to_string_lossy().into(), ..Default::default() });
            Ok(())
        })
        .unwrap();
        intake::sync().unwrap();
        let doc: String = store::open(&intake::store_path()).unwrap().query_row("SELECT source FROM documents WHERE source LIKE 'file/%'", [], |r| r.get(0)).unwrap();
        let (call_ids, doc_ids, mail_ids) = (sections(&source_of(&shut)), sections(&doc), sections("mail/hooli-plan"));
        assert!(!doc_ids.is_empty() && doc_ids.iter().all(|s| s.1 == "document"), "{doc_ids:?}");
        assert_eq!(mail_ids.len(), 1);

        projects::set_stage(&hooli, Stage::Done, Some("wrapped up")).unwrap();
        intake::sync().unwrap();
        assert_eq!(kinds(&source_of(&shut)), ["card"], "a closed thread's call: its card only");
        assert_eq!(sections(&source_of(&shut))[0], *call_ids.last().unwrap(), "the card keeps its id");
        assert_eq!(kinds(&source_of(&kept)).len(), 4, "an open thread's call is whole");
        assert!(sections("mail/hooli-plan").is_empty(), "a closed thread's mail leaves");
        assert_eq!(sections(&doc), [(crate::intake::chunk::chunk_id(&doc, store::CARD_ORD), "card".to_string())], "a closed thread's document: its card");
        let h = store::search_in(&intake::store_path(), "gateway rollout budget", 5, None, None, None).unwrap();
        let card = h["hits"].as_array().unwrap().iter().find(|x| x["source"] == doc.as_str()).expect("the card is found").clone();
        let text = card["text"].as_str().unwrap();
        assert!(text.contains("Headings: Gateway rollout · Budget") && text.contains("plan.md") && text.contains("Hooli workshop") && card["closed"].is_array(), "{card}");
        assert!(folder.join("plan.md").is_file() && facts::path_of("mail/hooli-plan").unwrap().is_file(), "the file and the facts stay");

        // Opened again, with no Microsoft 365 sign-in: everything back, with the ids it had (R10).
        assert!(crate::graph::account().is_none());
        projects::set_stage(&hooli, Stage::Active, Some("back")).unwrap();
        intake::sync().unwrap();
        assert_eq!(sections(&source_of(&shut)), call_ids);
        assert_eq!(sections(&doc), doc_ids);
        assert_eq!(sections("mail/hooli-plan"), mail_ids, "the mail from its fact file");
        let r = intake::sync().unwrap();
        assert_eq!((r.retired, r.added, r.changed), (0, 0, 0), "{r:?}");
    }

    #[test]
    fn a_transcription_in_progress_holds_the_sweep_back_and_battery_never_does() {
        let (_env, _root) = crate::paths::testenv::folders("recall-retain-busy");
        on();
        let hooli = projects::create("Hooli workshop", None, None).unwrap().id;
        let shut = call("hooli", 2, Some(&hooli));
        let other = call("other", 3, None);
        intake::sync().unwrap();
        assert_eq!(kinds(&source_of(&shut)).len(), 4);
        projects::set_stage(&hooli, Stage::Done, Some("wrapped up")).unwrap();
        // Another call is being transcribed (its marker holds a live process: this one).
        let marker = other.join(crate::transcribe_marker());
        std::fs::write(&marker, std::process::id().to_string()).unwrap();
        let r = intake::sync().unwrap();
        assert!(r.sweep_waits && r.retired == 0, "{r:?}");
        assert_eq!(kinds(&source_of(&shut)).len(), 4, "the closed thread's call waits for the sweep");
        assert!(due(&intake::store_path(), chrono::Local::now().date_naive(), &projects::closed_threads()).unwrap(), "still due");
        // A marker left by a process that is gone holds nothing back.
        std::fs::write(&marker, "999999999").unwrap();
        let r = intake::sync().unwrap();
        assert!(!r.sweep_waits && r.retired == 3, "{r:?}");
        assert_eq!(kinds(&source_of(&shut)), ["card"]);
    }
}

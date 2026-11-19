//! The Brain (`docs/brain-design.md`): cross-call intelligence as Echo's own — one per user, facts
//! only (calls now; Teams channels, mail and thread folders next), nothing copied: every section
//! links back to the moment, message, mail or file it came from. Brain's store and rules
//! (onetest-ai/applied_skills `bundles/brain`), run by the app; no model inside Echo — embeddings
//! come from the endpoint the user sets (Ollama or OpenAI-compatible).

pub mod access;
pub mod browse;
pub mod advert;
pub mod calls;
pub mod chunk;
pub mod digest;
pub mod embed;
pub mod facts;
pub mod files;
pub mod mcp;
pub mod retain;
pub mod senders;
pub mod sources;
pub mod store;
pub mod taxonomy;
pub mod views;
pub mod when;

use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::paths;

/// Recall's folder, `<home>/recall` (Echo before 0.8.0 called it the Brain: `<home>/brain`, moved here the first time
/// it is asked for).
pub fn dir() -> PathBuf {
    let new = paths::home().join("recall");
    if !new.exists() {
        let old = paths::home().join("brain");
        if old.is_dir() {
            if let Err(e) = std::fs::rename(&old, &new) {
                eprintln!("recall: can't move {} to {}: {e}", old.display(), new.display());
                return old;
            }
        }
    }
    new
}

/// Echo is quiet enough for background reading: nothing records; on a Mac, on power.
pub(crate) fn quiet() -> bool {
    if crate::watch::running_watcher().and_then(|w| w.recording).is_some() {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("pmset").args(["-g", "batt"]).output();
        if let Ok(o) = out {
            if !String::from_utf8_lossy(&o.stdout).contains("AC Power") {
                return false;
            }
        }
    }
    true
}

/// `<home>/recall/knowledge.sqlite`: the user's one index of what was said, beside their calls.
pub fn store_path() -> PathBuf {
    dir().join("knowledge.sqlite")
}

/// Settings → Brain, per computer: `<config dir>/brain.json` (0600: it may hold a key).
fn settings_path() -> PathBuf {
    paths::config_dir().join("brain.json")
}

pub fn settings() -> embed::Settings {
    std::fs::read(settings_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_settings(s: &embed::Settings) -> Result<()> {
    anyhow::ensure!(s.api.is_empty() || s.api == "ollama" || s.api == "openai", "the API is ollama or openai");
    std::fs::create_dir_all(paths::config_dir())?;
    // Keys inside: written 0600 from the start (never readable by others, even for a moment), then
    // renamed into place.
    let p = settings_path();
    let tmp = p.with_extension(format!("json.{}.tmp", std::process::id()));
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        let mut f = opts.open(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(s)?)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &p)?;
    Ok(())
}

/// The configured embedder, if one is set.
/// Whether the Brain is on (Settings → Brain; `echoctl intake set --enabled`).
pub fn enabled() -> bool {
    settings().enabled()
}

pub fn embedder() -> Option<embed::Http> {
    let s = settings();
    s.ready().then_some(embed::Http(s))
}

/// Every call with a transcript, as documents.
fn call_inputs() -> Vec<store::Input> {
    crate::calls::all_calls()
        .into_iter()
        .filter_map(|d| calls::document(&d))
        .map(|doc| {
            let name = doc.call.clone();
            store::Input {
                source: doc.source,
                kind: "call".into(),
                title: doc.title,
                event_date: doc.event_date,
                md: doc.md,
                refs: doc.refs,
                link: Box::new(move |c| match calls::title_time(&c.title) {
                    Some(t) => format!("echo://call/{name}?t={}", t as u64),
                    None => format!("echo://call/{name}"),
                }),
                speaker: Box::new(|c| calls::title_time(&c.title).map(|_| c.title.split_once("] ").map(|x| x.1.to_string()).unwrap_or_default())),
            }
        })
        .collect()
}

/// Whether Echo records or transcribes a call now (the retention sweep waits for it): the recorder's state, or a call's
/// `.transcribing` marker whose process is alive. On battery or not.
fn recording_or_transcribing() -> bool {
    crate::watch::running_watcher().and_then(|w| w.recording).is_some()
        || crate::calls::all_calls().iter().any(|d| {
            std::fs::read_to_string(d.join(crate::transcribe_marker())).ok().and_then(|p| p.trim().parse::<u32>().ok()).is_some_and(crate::app::process_alive)
        })
}

/// Bring the Brain up to date with the sources (one sync at a time on this computer).
pub fn sync() -> Result<store::Report> {
    let lock = paths::config_dir().join("brain.lock");
    anyhow::ensure!(enabled(), "Intake is off (Settings → Intake)");
    let _one = crate::app::FileLock::try_take(&lock)?.ok_or_else(|| anyhow::anyhow!("the Brain is being updated already"))?;
    // Intake's facts out of the index into their files, once, before a sync writes any.
    // Failed, it says so in Settings → Intake; mail and channels wait for it (`sources::sync`).
    if let Err(e) = facts::migrate_locked() {
        eprintln!("intake: {e:#}");
        facts::set_status(Some(&format!("moving the facts out of the index failed: {e:#}")), &[]);
    }
    let emb = embedder();
    // A thread opened again: its documents leave the index whole first, so this sync reads them back (`retain`).
    let closed = crate::projects::closed_threads();
    match retain::restore(&store_path(), &closed) {
        Ok(n) if n > 0 => eprintln!("recall: {n} sources of threads opened again read back"),
        Ok(_) => {}
        Err(e) => eprintln!("recall: retention: {e:#}"),
    }
    let keep = store::Keep::now();
    let before = indexed_sources()?;
    // Folders' documents read again: not among `r.touched` (plugins aren't told of them), but not unchanged.
    let mut folder_touched: Vec<String> = vec![];
    let mut r = store::sync_kept(&store_path(), call_inputs(), emb.as_ref().map(|e| e as &dyn embed::Embedder), &["call"], Some(&keep))?;
    // Mail and posts whose facts the index lacks (a thread opened again, an index rebuilt): from their files, with or
    // without a Microsoft 365 sign-in.
    match sources::feed_missing(emb.as_ref().map(|e| e as &dyn embed::Embedder), &keep) {
        Ok(s) => {
            r.touched.extend(s.touched);
            r.added += s.added;
            r.chunks += s.chunks;
            r.embedded += s.embedded;
        }
        Err(e) => eprintln!("intake: feeding the index from the fact files: {e:#}"),
    }
    // Thread sources (Teams channels, mail): only with a Microsoft 365 sign-in; their failures are
    // kept per source (`brain/sources.json`) and never stop the calls' sync.
    if crate::graph::account().is_some() {
        match sources::sync(emb.as_ref().map(|e| e as &dyn embed::Embedder)) {
            Ok((read, s)) => {
                r.touched.extend(s.touched);
                r.gone.extend(s.gone);
                r.added += s.added;
                r.changed += s.changed;
                r.removed += s.removed + read.removed;
                r.chunks += s.chunks;
                r.embedded += s.embedded;
                r.embed_error = r.embed_error.or(s.embed_error);
                for e in read.errors {
                    eprintln!("brain: sources: {e}");
                }
            }
            Err(e) => eprintln!("brain: sources: {e:#}"),
        }
    }
    // Thread folders: read what changed, each page from its text.
    match files::folder::sync(emb.as_ref().map(|e| e as &dyn embed::Embedder)) {
        Ok(reps) => {
            for f in reps {
                r.removed += f.removed;
                r.files_read += f.read;
                if let Some(s) = &f.store {
                    folder_touched.extend(s.touched.iter().cloned());
                    r.added += s.added;
                    r.changed += s.changed;
                    r.chunks += s.chunks;
                    r.embedded += s.embedded;
                    r.embed_error = r.embed_error.clone().or(s.embed_error.clone());
                }
                for e in f.errors.iter().take(5) {
                    eprintln!("brain: folder {}: {e}", f.root);
                }
            }
        }
        Err(e) => eprintln!("brain: folders: {e:#}"),
    }
    // History: documents, posts and mail dated before every one of their threads' "history before"
    // date are superseded (kept; searched only when history is asked for). Calls aren't: a thread's
    // "since" decides which calls it reads.
    let cutoffs: std::collections::BTreeMap<String, Option<String>> = crate::projects::list().into_iter().map(|p| (p.id, p.history_before)).collect();
    let cutoff = |src: &str, refs: &serde_json::Value| -> Option<String> {
        if src.starts_with("call/") {
            return None;
        }
        let ts: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
        let cs: Vec<String> = ts.iter().map(|t| cutoffs.get(*t).cloned().flatten()).collect::<Option<Vec<_>>>()?;
        cs.into_iter().min()
    };
    match store::supersede(&store_path(), &cutoff) {
        Ok((m, rs)) if m + rs > 0 => eprintln!("brain: history: {m} sections marked superseded, {rs} brought back"),
        Ok(_) => {}
        Err(e) => eprintln!("brain: history: {e:#}"),
    }
    // Retention: sections past their horizon, and closed threads' (their cards stay), leave the index.
    // Once a day and when a thread closed or opened again; never while Echo records or transcribes (it waits, and the
    // app's next pass tries again).
    if retain::due(&store_path(), keep.today, &closed)? {
        if recording_or_transcribing() {
            r.sweep_waits = true;
        } else {
            r.retired = retain::apply(&store_path(), keep.today, &closed)?;
        }
    }
    // Which sections mention which terms: no model; the sections written or moved to other threads since they
    // were last read (`term_scan`, cleared by every write of a section). A store from before Brain's meta keys
    // and entity-kind nodes gets its graph rebuilt once.
    taxonomy::mentions_after_sync(&store::open(&store_path())?)?;
    r.unchanged = unchanged(&before, &indexed_sources()?, r.touched.iter().chain(&folder_touched)).len();
    #[cfg(feature = "js")]
    crate::share::sources_changed(&r.touched, &r.gone);
    Ok(r)
}

/// Every source the index has a document of.
fn indexed_sources() -> Result<BTreeSet<String>> {
    let c = store::open(&store_path())?;
    let mut st = c.prepare("SELECT source FROM documents")?;
    let rows = st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(rows)
}

/// The sources a sync left as they were: indexed before and after it (not removed), and neither added nor changed.
fn unchanged<'a>(before: &BTreeSet<String>, after: &BTreeSet<String>, touched: impl Iterator<Item = &'a String>) -> BTreeSet<String> {
    let mut same: BTreeSet<String> = before.intersection(after).cloned().collect();
    for t in touched {
        same.remove(t);
    }
    same
}

/// What the calls look like on disk (their files' times and sizes): a sync runs only when it changed.
fn fingerprint() -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    // Channels and mail change without any file here: with a source to read, every half hour.
    if crate::projects::list().iter().any(|p| !p.stage.closed() && p.sources.iter().any(|s| s.kind == "teams_channel" || s.kind == "mail")) {
        (chrono::Utc::now().timestamp() / 1800).hash(&mut h);
        serde_json::to_string(&crate::projects::list().iter().map(|p| &p.sources).collect::<Vec<_>>()).unwrap_or_default().hash(&mut h);
    }
    // Folders: every file's size and time.
    for p in crate::projects::list().iter().filter(|p| !p.stage.closed()) {
        for s in p.sources.iter().filter(|s| s.kind == "folder") {
            (s.id.as_str(), &s.exclude).hash(&mut h);
            let mut stack = vec![std::path::PathBuf::from(&s.id)];
            let mut n = 0;
            while let Some(d) = stack.pop() {
                for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                    n += 1;
                    if n > 5000 {
                        break;
                    }
                    let Ok(m) = e.metadata() else { continue };
                    if m.is_dir() {
                        if !e.file_name().to_string_lossy().starts_with('.') {
                            stack.push(e.path());
                        }
                    } else {
                        (e.path(), m.len(), m.modified().ok()).hash(&mut h);
                    }
                }
            }
        }
    }
    // Each thread's history date (supersession) and whether it is closed (retention), and the day (horizons).
    for p in crate::projects::list() {
        (p.id, p.history_before, p.stage.closed()).hash(&mut h);
    }
    chrono::Local::now().date_naive().hash(&mut h);
    for d in crate::calls::all_calls() {
        d.hash(&mut h);
        for f in ["meta.json", "segments.json", "notes.md", "chat.json", "meeting.json"] {
            if let Ok(m) = std::fs::metadata(d.join(f)) {
                m.len().hash(&mut h);
                m.modified().ok().hash(&mut h);
            }
        }
    }
    // Threads decide a call's references (its threads, its client).
    for p in crate::projects::list() {
        serde_json::to_string(&(&p.id, &p.name, &p.client, &p.rules, &p.pins, &p.exclusions, &p.events, &p.since)).unwrap_or_default().hash(&mut h);
    }
    settings().model.hash(&mut h);
    h.finish()
}

/// Keep the Brain current in the app: every 5 minutes, a sync when the calls (or the threads, or the
/// model) changed since the last one; a failure is retried at the next change or after an hour.
/// Once, after the update that turned the Brain into Recall: say what went and why (only to someone who had topics).
fn tell_recall() {
    let mark = dir().join("recall-notice");
    if mark.exists() || taxonomy::current().is_none_or(|t| t.topics.is_empty()) {
        return;
    }
    crate::app::notify(
        "The Brain is now Recall",
        "Topics, the graph, insights and numbers didn't survive our tests, so they're switched off. Recall keeps your sources, search and the dictionary; lasting facts go to Knowledge (the Obsidian plugin).",
    );
    let _ = std::fs::write(mark, chrono::Local::now().to_rfc3339());
}

pub fn start_scheduler() {
    tell_recall();
    // Intake's facts out of the index into their files, once after the update (Intake on or off).
    std::thread::spawn(|| match facts::migrate() {
        Ok(Some(r)) => eprintln!("intake: {} mail and posts moved into fact files", r["written"]),
        Ok(None) => {}
        Err(e) => {
            eprintln!("intake: {e:#}");
            facts::set_status(Some(&format!("moving the facts out of the index failed: {e:#}")), &[]);
        }
    });
    // The old learned vocabulary moves into the taxonomy once (transcription reads both meanwhile).
    std::thread::spawn(|| {
        if let Ok(mut cfg) = crate::config::Config::load() {
            match crate::vocab::migrate_learned(&mut cfg) {
                Ok(0) => {}
                Ok(n) => eprintln!("vocabulary: {n} learned terms moved into the taxonomy"),
                Err(e) => eprintln!("vocabulary: moving the learned terms: {e:#}"),
            }
        }
    });
    // What the page reader and the snapshots left behind, once: Intake's facts, the dictionary and the
    // senders stay.
    std::thread::spawn(drop_old_folders);
    std::thread::spawn(|| {
        let mut last: Option<u64> = None;
        let mut failed_at: Option<std::time::Instant> = None;
        loop {
            // As often as Recall's job settings say (5 minutes by default; a minute after start), never in
            // the night they pause it in.
            std::thread::sleep(std::time::Duration::from_secs(if last.is_none() { 60 } else { crate::jobs::after_pass("brain_reader", crate::jobs::number("brain_reader", "every")) }));
            if crate::notes::quitting() || !settings().enabled() || !settings().auto() || crate::jobs::paused("brain_reader", chrono::Local::now().time()) {
                continue;
            }
            let fp = fingerprint();
            let retry = failed_at.is_some_and(|t| t.elapsed() > std::time::Duration::from_secs(3600));
            if last == Some(fp) && !retry {
                continue;
            }
            let run = crate::jobs::begin("brain_reader");
            match sync() {
                Ok(r) => {
                    // A run when it indexed something or read mail into facts (its model calls).
                    run.done((r.chunks > 0).then(|| format!("{} new section{} indexed", r.chunks, if r.chunks == 1 { "" } else { "s" })));
                    failed_at = r.embed_error.as_ref().map(|_| std::time::Instant::now());
                    if let Some(e) = &r.embed_error {
                        eprintln!("brain: embeddings: {e}");
                    }
                    // A sweep held back by a recording or transcription: the next pass (a minute on) runs it.
                    last = (!r.sweep_waits).then_some(fp);
                }
                Err(e) => {
                    eprintln!("brain: {e:#}");
                    run.failed("Recall couldn't read its sources (Echo's log says why)");
                    failed_at = Some(std::time::Instant::now());
                    last = Some(fp);
                }
            }
        }
    });
}

/// `recall/snapshots/` (copies of the index: it is rebuilt from its sources) and `recall/assets/` (the page reader's
/// page images and text), removed when they are there; nothing else under `recall/` is touched.
fn drop_old_folders() {
    for name in ["snapshots", "assets"] {
        let d = dir().join(name);
        if std::fs::symlink_metadata(&d).is_ok() {
            match std::fs::remove_dir_all(&d) {
                Ok(()) => eprintln!("recall: {name}/ removed (no longer used)"),
                Err(e) => eprintln!("recall: can't remove {}: {e}", d.display()),
            }
        }
    }
}

pub fn search(q: &str, limit: usize, within: Option<&str>, scope: store::Scope, filters: &store::Filters) -> Result<Value> {
    let emb = embedder();
    store::search_at(&store_path(), q, limit, within, scope, emb.as_ref().map(|e| e as &dyn embed::Embedder), store::History::Latest, None, filters)
}

pub fn status() -> Result<Value> {
    let mut v = store::status(&store_path())?;
    let s = settings();
    v["settings"] = serde_json::json!({"endpoint": s.endpoint, "api": s.api, "model": s.model, "key": s.key.as_deref().is_some_and(|k| !k.is_empty())});
    v["path"] = serde_json::json!(store_path());
    Ok(v)
}

#[cfg(test)]
mod tests {
    #[test]
    fn unchanged_is_what_was_indexed_and_stays_neither_added_changed_nor_removed() {
        let set = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<std::collections::BTreeSet<String>>();
        // Before: a call, two mail facts, a channel post, a folder's file.
        let before = set(&["call/a", "mail/kept", "mail/edited", "channel/gone", "file/f"]);
        // The sources sync changes one fact and removes the post; a fact fed from its file is then changed again too.
        let after = set(&["call/a", "mail/kept", "mail/edited", "file/f", "mail/fed"]);
        let touched = ["mail/fed".to_string(), "mail/edited".to_string(), "mail/fed".to_string()];
        assert_eq!(super::unchanged(&before, &after, touched.iter()), set(&["call/a", "mail/kept", "file/f"]));
        // Nothing touched, nothing gone: everything indexed is unchanged.
        assert_eq!(super::unchanged(&before, &before, [].iter()), before);
    }

    #[test]
    fn the_brains_folder_becomes_recalls() {
        let (_env, root) = crate::paths::testenv::folders("recall-dir");
        let old = crate::paths::home().join("brain");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("knowledge.sqlite"), b"x").unwrap();
        assert_eq!(super::store_path(), crate::paths::home().join("recall").join("knowledge.sqlite"));
        assert!(!old.exists() && super::store_path().exists(), "moved, not copied");
        assert!(root.exists());
    }

    #[test]
    fn the_snapshots_and_page_images_go_and_intakes_files_stay() {
        let (_env, _root) = crate::paths::testenv::folders("recall-old-folders");
        let d = super::dir();
        let keep = ["intake/facts/mail/a.json", "taxonomy/v1.json", "senders.json", "sources.json", "knowledge.sqlite"];
        for f in keep.iter().chain(["snapshots/knowledge-1.sqlite", "assets/ab12/p01.png"].iter()) {
            std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
            std::fs::write(d.join(f), b"x").unwrap();
        }
        super::drop_old_folders();
        assert!(!d.join("snapshots").exists() && !d.join("assets").exists());
        for f in keep {
            assert!(d.join(f).is_file(), "{f} stays");
        }
        // Again, with nothing to remove: nothing changes.
        super::drop_old_folders();
        assert!(keep.iter().all(|f| d.join(f).is_file()));
    }
}

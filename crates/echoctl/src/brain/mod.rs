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
pub mod files;
pub mod insights;
pub mod mcp;
pub mod numbers;
pub mod senders;
pub mod sources;
pub mod store;
pub mod taxonomy;
pub mod views;

use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

use crate::paths;

/// `<home>/brain/knowledge.sqlite`: the user's one Brain, beside their calls.
pub fn store_path() -> PathBuf {
    paths::home().join("brain").join("knowledge.sqlite")
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
/// Whether the Brain is on (Settings → Brain; `echoctl brain set --enabled`).
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

/// Bring the Brain up to date with the sources (one sync at a time on this computer).
pub fn sync() -> Result<store::Report> {
    let lock = paths::config_dir().join("brain.lock");
    anyhow::ensure!(enabled(), "the Brain is off (Settings → Brain)");
    let _one = crate::app::FileLock::try_take(&lock)?.ok_or_else(|| anyhow::anyhow!("the Brain is being updated already"))?;
    let emb = embedder();
    let mut r = store::sync(&store_path(), call_inputs(), emb.as_ref().map(|e| e as &dyn embed::Embedder))?;
    // Thread sources (Teams channels, mail): only with a Microsoft 365 sign-in; their failures are
    // kept per source (`brain/sources.json`) and never stop the calls' sync.
    if crate::graph::account().is_some() {
        match sources::sync(emb.as_ref().map(|e| e as &dyn embed::Embedder)) {
            Ok((read, s)) => {
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
    // Thread folders: read what changed; pages flagged visual go to the page reader only while
    // nothing records (and, on a Mac, on power), within a budget per run.
    match files::folder::sync(emb.as_ref().map(|e| e as &dyn embed::Embedder), insights::quiet()) {
        Ok(reps) => {
            for f in reps {
                r.removed += f.removed;
                r.files_read += f.read;
                r.pages_read += f.pages_read;
                r.pages_pending += f.pages_pending;
                if let Some(s) = &f.store {
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
    // The numbers lane: clients with accepted families, built again when their workbooks changed.
    for (client, r) in numbers::refresh() {
        match r {
            Ok(v) => eprintln!("brain: numbers {client}: {} facts", v["summary"]["facts"]),
            Err(e) => eprintln!("brain: numbers {client}: {e:#}"),
        }
    }
    match store::supersede(&store_path(), &cutoff) {
        Ok((m, rs)) if m + rs > 0 => eprintln!("brain: history: {m} sections marked superseded, {rs} brought back"),
        Ok(_) => {}
        Err(e) => eprintln!("brain: history: {e:#}"),
    }
    // Which sections mention which terms: no model; the sections written or moved to other threads since they
    // were last read (`term_scan`, cleared by every write of a section). A store from before Brain's meta keys
    // and entity-kind nodes gets its graph rebuilt once.
    taxonomy::mentions_after_sync(&store::open(&store_path())?)?;
    Ok(r)
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
    // Folders: every file's size and time; with pages waiting for the page reader, every 10 minutes.
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
    // Each thread's history date (supersession).
    for p in crate::projects::list() {
        (p.id, p.history_before).hash(&mut h);
    }
    if files::folder::pending() > 0 {
        (chrono::Utc::now().timestamp() / 600).hash(&mut h);
    }
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
pub fn start_scheduler() {
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
    // What the Brain's screens count over every section (seconds on millions), counted once now so the first look
    // doesn't wait; later changes are counted again in the background (`store::kept`).
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(20));
        if enabled() {
            let _ = views::overview(None);
            let _ = browse::graph_topics(None);
        }
    });
    std::thread::spawn(|| {
        let mut last: Option<u64> = None;
        let mut failed_at: Option<std::time::Instant> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(if last.is_none() { 60 } else { 300 }));
            if crate::notes::quitting() || !settings().enabled() || !settings().auto() {
                continue;
            }
            let fp = fingerprint();
            let retry = failed_at.is_some_and(|t| t.elapsed() > std::time::Duration::from_secs(3600));
            if last == Some(fp) && !retry {
                continue;
            }
            match sync() {
                Ok(r) => {
                    // New sections get their topics once a taxonomy exists (Claude, on the default profile).
                    if r.chunks > 0 && taxonomy::current().is_some() {
                        if let Err(e) = taxonomy::classify(None, false, &|_| {}) {
                            eprintln!("brain: tagging: {e:#}");
                        }
                    }
                    // Insights for scopes whose sections changed (at most daily each, a daily cap, only when quiet).
                    if let Err(e) = insights::tick() {
                        eprintln!("brain: insights: {e:#}");
                    }
                    failed_at = r.embed_error.as_ref().map(|_| std::time::Instant::now());
                    if let Some(e) = &r.embed_error {
                        eprintln!("brain: embeddings: {e}");
                    }
                    last = Some(fp);
                }
                Err(e) => {
                    eprintln!("brain: {e:#}");
                    failed_at = Some(std::time::Instant::now());
                    last = Some(fp);
                }
            }
        }
    });
}

pub fn search(q: &str, limit: usize, within: Option<&str>, scope: store::Scope) -> Result<Value> {
    let emb = embedder();
    store::search_in(&store_path(), q, limit, within, scope, emb.as_ref().map(|e| e as &dyn embed::Embedder))
}

pub fn status() -> Result<Value> {
    let mut v = store::status(&store_path())?;
    let s = settings();
    v["settings"] = serde_json::json!({"endpoint": s.endpoint, "api": s.api, "model": s.model, "key": s.key.as_deref().is_some_and(|k| !k.is_empty()),
        "pages": s.pages(), "vision_endpoint": s.vision_endpoint, "vision_api": s.vision_api, "vision_model": s.vision_model,
        "vision_key": s.vision_key.as_deref().is_some_and(|k| !k.is_empty())});
    v["path"] = serde_json::json!(store_path());
    Ok(v)
}

//! The store (`<home>/brain/knowledge.sqlite`), Brain's schema: `chunks` (+ Echo's `link` and `refs`),
//! `chunks_fts` (FTS5, any script), `chunks_vec` (a plain table of unit vectors, searched in Rust),
//! `documents` (the per-document skip cache), `related`, the taxonomy tables (filled in phase 2),
//! `meta`. A sync rebuilds only documents whose text changed and embeds only chunks whose text
//! changed; it snapshots the file first and writes in one transaction.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::chunk::{self, CHUNKER_VERSION, MAX_CHARS};
use super::embed::{Embedder, BATCH};

/// `related`: the k nearest sections of other documents, at least this similar.
const RELATED_K: usize = 6;
const RELATED_MIN: f32 = 0.55;
/// Search: the keyword top 30 are the candidates; with an embedding server they are re-ranked by weighted reciprocal
/// rank (k = 60) of their keyword rank (0.4) and their vector rank among themselves (0.6).
const RRF_K: f32 = 60.0;
const W_FTS: f32 = 0.4;
const W_VEC: f32 = 0.6;
const CANDIDATES: usize = 30;
/// What `chunks_fts` holds of a section (`fts_text`); another version writes it again from `chunks` at `open`.
const FTS_TEXT_VERSION: &str = "2";
/// Snapshots kept.
const SNAPSHOTS: usize = 3;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let c = Connection::open(path).with_context(|| format!("opening {}", path.display()))?;
    c.busy_timeout(std::time::Duration::from_secs(10))?;
    c.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS chunks(id INTEGER PRIMARY KEY, source TEXT NOT NULL, ord INTEGER NOT NULL, title TEXT, text TEXT,
            sha TEXT, image TEXT, embedding_content_hash TEXT, created_at TEXT, event_date TEXT, valid_from TEXT, valid_to TEXT,
            status TEXT DEFAULT 'ACTIVE', parent_heading TEXT, breadcrumb_path TEXT, speaker TEXT, link TEXT, refs TEXT);
         CREATE INDEX IF NOT EXISTS chunks_source ON chunks(source);
         CREATE INDEX IF NOT EXISTS chunks_status ON chunks(status);
         CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(text, tokenize = 'unicode61 remove_diacritics 2');
         CREATE TABLE IF NOT EXISTS chunks_vec(rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL);
         CREATE TABLE IF NOT EXISTS documents(source TEXT PRIMARY KEY, content_hash TEXT, indexed_at TEXT, title TEXT, kind TEXT, event_date TEXT, refs TEXT);
         CREATE TABLE IF NOT EXISTS related(chunk_id INTEGER, related_id INTEGER, score REAL, edge_type TEXT DEFAULT 'SIMILAR', directed INTEGER DEFAULT 0,
            PRIMARY KEY(chunk_id, related_id));
         CREATE TABLE IF NOT EXISTS graph_nodes(id TEXT PRIMARY KEY, label TEXT, kind TEXT, parent TEXT, description TEXT);
         CREATE TABLE IF NOT EXISTS graph_edges(source TEXT, target TEXT, rel TEXT);
         CREATE TABLE IF NOT EXISTS graph_aliases(alias_id TEXT PRIMARY KEY, node_id TEXT);
         CREATE TABLE IF NOT EXISTS chunk_topics(chunk_id INTEGER, category_id TEXT, category_label TEXT, kind TEXT);
         CREATE TABLE IF NOT EXISTS chunk_verdicts(chunk_id INTEGER PRIMARY KEY, verdict TEXT, taxonomy_version TEXT);
         CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT);
         CREATE TABLE IF NOT EXISTS scopes(chunk_id INTEGER NOT NULL, kind TEXT NOT NULL, id TEXT NOT NULL, PRIMARY KEY(chunk_id, kind, id));
         CREATE TABLE IF NOT EXISTS term_scan(chunk_id INTEGER PRIMARY KEY, sha TEXT NOT NULL);
         CREATE INDEX IF NOT EXISTS scopes_by ON scopes(kind, id);
         CREATE INDEX IF NOT EXISTS chunk_topics_cat ON chunk_topics(category_id, chunk_id);
         CREATE INDEX IF NOT EXISTS chunk_topics_chunk ON chunk_topics(chunk_id);
         CREATE TABLE IF NOT EXISTS brain_schema(component TEXT PRIMARY KEY, version TEXT, migrated_at TEXT);
         INSERT OR IGNORE INTO brain_schema VALUES ('echo', '1', datetime('now'));",
    )?;
    // What a list of sources reads without touching its sections (the window pages tens of thousands): how many
    // sections, its client (`-`: none) and its folder — kept by `doc_facts` at every write, filled once for an older store.
    // One writer at a time, each column on its own: two processes opening an older store, or one stopped half way,
    // leave it whole.
    let missing = |c: &Connection| -> Result<Vec<&'static str>> {
        let mut out = vec![];
        for col in ["sections", "client", "root"] {
            if c.query_row("SELECT count(*) FROM pragma_table_info('documents') WHERE name = ?1", [col], |r| r.get::<_, i64>(0))? == 0 {
                out.push(col);
            }
        }
        Ok(out)
    };
    if !missing(&c)?.is_empty() {
        c.execute_batch("BEGIN IMMEDIATE")?;
        let done = (|| -> Result<()> {
            let cols = missing(&c)?;
            for col in &cols {
                c.execute_batch(&format!("ALTER TABLE documents ADD COLUMN {col} {}", if *col == "sections" { "INTEGER" } else { "TEXT" }))?;
            }
            if !cols.is_empty() {
                c.execute(&format!("UPDATE documents SET {DOC_FACTS}"), [])?;
            }
            Ok(())
        })();
        c.execute_batch(if done.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        done?;
    }
    c.execute_batch("CREATE INDEX IF NOT EXISTS documents_list ON documents(client, kind, event_date); CREATE INDEX IF NOT EXISTS documents_root ON documents(root, event_date);")?;
    // A store from before scopes: fill them from each section's references, once.
    let empty: bool = c.query_row("SELECT NOT EXISTS (SELECT 1 FROM scopes) AND EXISTS (SELECT 1 FROM chunks)", [], |r| r.get(0))?;
    if empty {
        let rows: Vec<(i64, Option<String>)> = c.prepare("SELECT id, refs FROM chunks")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let tx = c.unchecked_transaction()?;
        for (id, refs) in rows {
            let refs = refs.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or(Value::Null);
            write_scopes(&tx, id, &refs)?;
        }
        tx.commit()?;
    }
    if meta(&c, "fts_text").as_deref() != Some(FTS_TEXT_VERSION) {
        rewrite_fts(&c)?;
    }
    Ok(c)
}

/// Sections whose keyword text one transaction of `rewrite_fts` writes.
const FTS_BATCH: i64 = 500;

/// Write every section's keyword text again (`fts_text`) from what `chunks` keeps, once per `FTS_TEXT_VERSION`: in
/// short transactions by id (`meta.fts_text_next`: the version and the last id done), each taken only when no one else
/// writes — busy, it stops, and the old text is searched until a later open goes on. No source is read again and no
/// vector made again.
fn rewrite_fts(c: &Connection) -> Result<()> {
    c.busy_timeout(std::time::Duration::ZERO)?;
    let r = (|| -> Result<()> {
        loop {
            if let Err(e) = c.execute_batch("BEGIN IMMEDIATE") {
                return if matches!(e.sqlite_error_code(), Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)) { Ok(()) } else { Err(e.into()) };
            }
            let step = (|| -> Result<bool> {
                if meta(c, "fts_text").as_deref() == Some(FTS_TEXT_VERSION) {
                    return Ok(true);
                }
                let after: i64 = meta(c, "fts_text_next")
                    .and_then(|v| v.strip_prefix(&format!("{FTS_TEXT_VERSION}:")).and_then(|id| id.parse().ok()))
                    .unwrap_or(i64::MIN);
                let mut st = c.prepare("SELECT id, coalesce(breadcrumb_path, ''), coalesce(speaker, ''), coalesce(title, ''), coalesce(text, '') FROM chunks WHERE id > ?1 ORDER BY id LIMIT ?2")?;
                let mut del = c.prepare("DELETE FROM chunks_fts WHERE rowid = ?1")?;
                let mut ins = c.prepare("INSERT INTO chunks_fts(rowid, text) VALUES (?1, ?2)")?;
                let (mut n, mut last) = (0, after);
                let mut rows = st.query(params![after, FTS_BATCH])?;
                while let Some(r) = rows.next()? {
                    last = r.get(0)?;
                    del.execute([last])?;
                    ins.execute(params![last, fts_text(&r.get::<_, String>(1)?, &r.get::<_, String>(2)?, &r.get::<_, String>(3)?, &r.get::<_, String>(4)?)])?;
                    n += 1;
                }
                if n < FTS_BATCH {
                    set_meta(c, "fts_text", FTS_TEXT_VERSION)?;
                    c.execute("DELETE FROM meta WHERE key = 'fts_text_next'", [])?;
                    return Ok(true);
                }
                set_meta(c, "fts_text_next", &format!("{FTS_TEXT_VERSION}:{last}"))?;
                Ok(false)
            })();
            c.execute_batch(if step.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
            if step? {
                return Ok(());
            }
        }
    })();
    c.busy_timeout(std::time::Duration::from_secs(10))?;
    r
}

/// A section's keyword text: its breadcrumb, speaker, title and body, without the minutes' header lines that link
/// people and threads (`**Who:** [Dana](echo://…)`, repeated in every section of a call; what such a line says besides
/// its label and links stays) and with any other echo:// link reduced to its label.
fn fts_text(breadcrumb: &str, speaker: &str, title: &str, body: &str) -> String {
    let all = format!("{breadcrumb}\n{speaker}\n{title}\n{body}");
    let kept: Vec<String> = all
        .lines()
        .filter_map(|l| match header_rest(l) {
            None => Some(l.to_string()),
            Some(rest) => rest.chars().any(char::is_alphanumeric).then_some(rest),
        })
        .collect();
    strip_echo_links(&kept.join("\n"), true)
}

/// A `Who:`, `Thread:`, `Threads:` or `Client:` line (bold or not) with an echo:// link: what it says besides its label
/// and its links (none: not such a line).
fn header_rest(line: &str) -> Option<String> {
    let t = line.trim_start();
    let t = t.strip_prefix("**").unwrap_or(t);
    ["threads", "thread", "who", "client"].iter().find_map(|w| {
        let rest = t.get(..w.len()).filter(|p| p.eq_ignore_ascii_case(w)).map(|_| &t[w.len()..])?;
        let after = rest.trim_start().trim_start_matches("**").strip_prefix(':')?.trim_start_matches("**");
        rest.contains("echo://").then(|| strip_echo_links(after, false))
    })
}

/// `[label](echo://…)` → `label` (with `keep_label`, else nothing); a bare `echo://…` (in backticks or not) → a space.
/// A link never reaches past its line.
fn strip_echo_links(s: &str, keep_label: bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("echo://") {
        let (before, after) = rest.split_at(i);
        let line_end = after.find('\n').unwrap_or(after.len());
        if let Some(head) = before.strip_suffix("](") {
            if let Some(o) = head.rfind('[').filter(|o| !head[o + 1..].contains([']', '\n'])) {
                out.push_str(&head[..o]);
                if keep_label {
                    out.push_str(&head[o + 1..]);
                }
                rest = after[..line_end].find(')').map_or(&after[line_end..], |e| &after[e + 1..]);
                continue;
            }
        }
        out.push_str(before.strip_suffix('`').unwrap_or(before));
        out.push(' ');
        let end = after.find(|c: char| c.is_whitespace() || c == ')' || c == '`').unwrap_or(after.len());
        rest = after[end..].strip_prefix('`').unwrap_or(&after[end..]);
    }
    out.push_str(rest);
    out
}

/// An answer counted from the store, kept while `fp` (the store's `changed_stamp`, plus whatever else it reads) is
/// unchanged. Changed since: the last answer is given while it is counted again in the background — the fresh one when
/// it comes within 250 ms (a small store: always). Aggregates over millions of sections take seconds; a window waiting
/// for each of them after every sync would not do.
pub fn kept<T: Send + Sync + 'static>(key: String, fp: String, make: impl FnOnce() -> Result<T> + Send + 'static) -> Result<std::sync::Arc<T>> {
    let r = kept_inner(key, fp, make);
    if let Ok((_, stale)) = &r {
        if *stale {
            STALE.with(|s| s.set(true));
        }
    }
    r.map(|x| x.0)
}

thread_local! {
    /// Set when this thread was given a stale `kept` answer: what is built from it mustn't be kept as fresh.
    static STALE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f`; whether it was given any stale `kept` answer on the way.
pub fn noting_stale<R>(f: impl FnOnce() -> R) -> (R, bool) {
    let before = STALE.with(|s| s.replace(false));
    let r = f();
    let stale = STALE.with(|s| s.replace(before));
    if stale {
        STALE.with(|s| s.set(true));
    }
    (r, stale)
}

fn kept_inner<T: Send + Sync + 'static>(key: String, fp: String, make: impl FnOnce() -> Result<T> + Send + 'static) -> Result<(std::sync::Arc<T>, bool)> {
    use std::any::Any;
    use std::sync::{Arc, Condvar, Mutex};
    type Val = Arc<dyn Any + Send + Sync>;
    struct Entry {
        fp: String,
        val: Val,
        running: bool,
    }
    static CACHE: Mutex<Option<std::collections::HashMap<String, Entry>>> = Mutex::new(None);
    static DONE: Condvar = Condvar::new();
    let lock = || CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let typed = |v: &Val| v.clone().downcast::<T>().map_err(|_| anyhow::anyhow!("kept: another type under {key}"));
    let mut g = lock();
    let Some(e) = g.get_or_insert_with(Default::default).get_mut(&key) else {
        drop(g);
        let v: Arc<T> = Arc::new(make()?);
        lock().get_or_insert_with(Default::default).insert(key, Entry { fp, val: v.clone(), running: false });
        return Ok((v, false));
    };
    if e.fp == fp {
        return Ok((typed(&e.val)?, false));
    }
    if !e.running {
        e.running = true;
        let (k, f) = (key.clone(), fp.clone());
        std::thread::spawn(move || {
            // A panic counting still frees the key (else it is never counted again).
            let v = std::panic::catch_unwind(std::panic::AssertUnwindSafe(make)).unwrap_or_else(|_| Err(anyhow::anyhow!("panicked")));
            let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(e) = g.get_or_insert_with(Default::default).get_mut(&k) {
                e.running = false;
                match v {
                    Ok(v) => {
                        e.val = Arc::new(v);
                        e.fp = f;
                    }
                    Err(err) => eprintln!("brain: counting {k} again failed: {err:#}"),
                }
            }
            DONE.notify_all();
        });
    }
    let until = std::time::Instant::now() + std::time::Duration::from_millis(250);
    loop {
        let e = g.as_ref().and_then(|m| m.get(&key)).ok_or_else(|| anyhow::anyhow!("kept: {key} went"))?;
        let left = until.saturating_duration_since(std::time::Instant::now());
        if e.fp == fp || !e.running || left.is_zero() {
            return Ok((typed(&e.val)?, e.fp != fp));
        }
        g = DONE.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner()).0;
    }
}

/// What changes when the store is written: the file, and its write-ahead log while it holds writes not yet
/// checkpointed (an open connection makes an empty one: not a change).
pub fn changed_stamp(path: &Path) -> String {
    let stamp = |p: &Path| std::fs::metadata(p).ok().filter(|m| m.len() > 0).map(|m| format!("{}:{:?};", m.len(), m.modified().ok())).unwrap_or_default();
    stamp(path) + &stamp(&path.with_extension("sqlite-wal"))
}

/// A document's list facts from its sections and references (the client: its threads' first, else its own).
const DOC_FACTS: &str = "sections = (SELECT count(*) FROM chunks c WHERE c.source = documents.source),
    client = coalesce((SELECT json_extract(t.value, '$.client') FROM json_each(documents.refs, '$.threads') t WHERE json_extract(t.value, '$.client') IS NOT NULL LIMIT 1),
                      json_extract(documents.refs, '$.client'), '-'),
    root = json_extract(documents.refs, '$.root')";

/// Keep one document's list facts current after its sections or references changed.
fn doc_facts(tx: &Connection, source: &str) -> Result<()> {
    tx.execute(&format!("UPDATE documents SET {DOC_FACTS} WHERE source = ?1"), [source])?;
    Ok(())
}

/// A scope: `client` or `thread` and its id (`-`: in none).
pub type Scope<'a> = Option<(&'a str, &'a str)>;

/// The scopes of a section from its references: its client, its threads (or `-` when it is in none).
fn scopes_of(refs: &Value) -> Vec<(&'static str, String)> {
    let mut out = vec![];
    let threads: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    for t in &threads {
        out.push(("thread", t.to_string()));
    }
    if threads.is_empty() {
        out.push(("thread", "-".to_string()));
    }
    let mut clients: Vec<String> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["client"].as_str().map(str::to_owned)).collect();
    clients.extend(refs["client"].as_str().map(str::to_owned));
    clients.sort();
    clients.dedup();
    if clients.is_empty() {
        clients.push("-".into());
    }
    out.extend(clients.into_iter().map(|c| ("client", c)));
    out
}

fn write_scopes(tx: &Connection, id: i64, refs: &Value) -> Result<()> {
    tx.execute("DELETE FROM scopes WHERE chunk_id = ?1", [id])?;
    // Its clients decide whose terms it may mention, and a write may have dropped its tags: read for terms again
    // (`taxonomy::mentions_changed`).
    tx.execute("DELETE FROM term_scan WHERE chunk_id = ?1", [id])?;
    // A memo split by thread: a transcript section is only its part's thread's (its time says which part).
    let narrowed = if refs["memo_parts"].as_array().is_some_and(|p| !p.is_empty()) {
        let title: Option<String> = tx.query_row("SELECT title FROM chunks WHERE id = ?1", [id], |r| r.get(0)).ok().flatten();
        memo_section_refs(refs, title.as_deref().unwrap_or(""))
    } else {
        None
    };
    for (k, v) in scopes_of(narrowed.as_ref().unwrap_or(refs)) {
        tx.execute("INSERT OR IGNORE INTO scopes(chunk_id, kind, id) VALUES (?1, ?2, ?3)", params![id, k, v])?;
    }
    Ok(())
}

/// A memo's transcript section scoped to the thread of the part it falls in (`[mm:ss]` at the start of its title), and
/// a minutes section headed by a thread's name to that thread: what was said about another thread is never found within
/// this one. A transcript section about none: no thread. Any other section (the summary): every thread of the memo.
fn memo_section_refs(refs: &Value, title: &str) -> Option<Value> {
    let parts = refs["memo_parts"].as_array()?;
    let ids: Vec<&str> = match super::calls::title_time(title) {
        Some(t) => parts.iter().filter(|p| p["from"].as_f64().is_some_and(|a| t >= a) && p["to"].as_f64().is_some_and(|b| t < b)).filter_map(|p| p["thread"].as_str()).collect(),
        // A minutes section headed by a thread's name (the memo template's "### <thread> (00:00–02:40)"): that thread's.
        None => {
            let low = title.trim().to_lowercase();
            let named: Vec<&str> = refs["threads"].as_array().into_iter().flatten()
                .filter(|x| x["name"].as_str().is_some_and(|n| !n.is_empty() && low.starts_with(&n.to_lowercase())))
                .filter_map(|x| x["id"].as_str()).collect();
            if named.is_empty() {
                return None;
            }
            named
        }
    };
    let threads: Vec<Value> = refs["threads"].as_array().into_iter().flatten().filter(|x| x["id"].as_str().is_some_and(|i| ids.contains(&i))).cloned().collect();
    let client = threads.iter().find_map(|x| x["client"].as_str().map(str::to_owned));
    Some(json!({"kind": refs["kind"], "call": refs["call"], "threads": threads, "client": client}))
}

/// The sections in a scope (all when none).
fn in_scope(c: &Connection, scope: Scope) -> Result<Option<BTreeSet<i64>>> {
    let Some((k, v)) = scope else { return Ok(None) };
    let ids: BTreeSet<i64> = c.prepare("SELECT chunk_id FROM scopes WHERE kind = ?1 AND id = ?2")?.query_map(params![k, v], |r| r.get(0))?.collect::<Result<_, _>>()?;
    Ok(Some(ids))
}

fn sha(s: &str) -> String {
    Sha256::digest(s.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn floats(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|&c| f32::from_le_bytes(c)).collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

pub fn meta(c: &Connection, key: &str) -> Option<String> {
    c.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional().ok().flatten()
}

pub(crate) fn set_meta(c: &Connection, key: &str, value: &str) -> Result<()> {
    c.execute("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
    Ok(())
}

/// What a chunk says of itself to the store: its link, or its speakers.
pub type PerChunk<T> = Box<dyn Fn(&chunk::Chunk) -> T>;

/// A document to index (from any source): its id, title, Markdown, date, what it refers to, and how
/// to link a chunk back to the moment it came from.
pub struct Input {
    pub source: String,
    pub kind: String,
    pub title: String,
    pub event_date: Option<String>,
    pub md: String,
    pub refs: Value,
    pub link: PerChunk<String>,
    pub speaker: PerChunk<Option<String>>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Report {
    /// Sources added or changed, and gone (for plugins: `sources_changed`).
    #[serde(skip)]
    pub touched: Vec<String>,
    #[serde(skip)]
    pub gone: Vec<String>,
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub chunks: usize,
    pub embedded: usize,
    pub embed_error: Option<String>,
    /// Thread folders: files read, pages read by the page reader, pages still waiting for it.
    #[serde(skip_serializing_if = "is_zero")]
    pub files_read: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub pages_read: usize,
    #[serde(skip_serializing_if = "is_zero")]
    pub pages_pending: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn snapshot(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let dir = path.parent().unwrap().join("snapshots");
    std::fs::create_dir_all(&dir)?;
    let to = dir.join(format!("knowledge-{}.sqlite", chrono::Local::now().format("%Y%m%d-%H%M%S%3f")));
    // VACUUM INTO gives a consistent copy while others may read.
    let c = Connection::open(path)?;
    c.execute("VACUUM INTO ?1", [to.to_string_lossy()])?;
    let mut all: Vec<_> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).collect();
    all.sort();
    while all.len() > SNAPSHOTS {
        let _ = std::fs::remove_file(all.remove(0));
    }
    Ok(())
}

fn delete_chunks(tx: &Connection, ids: &[i64]) -> Result<()> {
    for id in ids {
        tx.execute("DELETE FROM chunks WHERE id = ?1", [id])?;
        tx.execute("DELETE FROM chunks_fts WHERE rowid = ?1", [id])?;
        tx.execute("DELETE FROM chunks_vec WHERE rowid = ?1", [id])?;
        tx.execute("DELETE FROM related WHERE chunk_id = ?1 OR related_id = ?1", [id])?;
        tx.execute("DELETE FROM chunk_topics WHERE chunk_id = ?1", [id])?;
        tx.execute("DELETE FROM chunk_verdicts WHERE chunk_id = ?1", [id])?;
        tx.execute("DELETE FROM graph_edges WHERE source = ?1", [format!("chunk:{id}")])?;
        tx.execute("DELETE FROM scopes WHERE chunk_id = ?1", [id])?;
        tx.execute("DELETE FROM term_scan WHERE chunk_id = ?1", [id])?;
    }
    Ok(())
}

/// Bring the store up to date with `inputs` (every document of the sources it covers; a document
/// missing from them is removed). Embeddings are asked for chunks whose text changed and chunks
/// without a vector yet; without an embedder (none configured) or when the server fails, the text
/// is indexed anyway and the vectors come at a later sync.
pub fn sync(path: &Path, inputs: Vec<Input>, emb: Option<&dyn Embedder>) -> Result<Report> {
    sync_kinds(path, inputs, emb, &["call"])
}

/// As `sync`, removing only documents of `kinds` that aren't among the inputs (`&[]`: none — channels
/// and mail add and update their documents; a source taken off a thread is removed with `remove`).
pub fn sync_kinds(path: &Path, inputs: Vec<Input>, emb: Option<&dyn Embedder>, kinds: &[&str]) -> Result<Report> {
    let mut c = open(path)?;
    let model = emb.map(|e| e.model()).unwrap_or_default();
    let mut rep = Report::default();
    let known_kinds: BTreeMap<String, (String, String)> = {
        let mut st = c.prepare("SELECT source, content_hash, coalesce(kind, 'call') FROM documents")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, (r.get::<_, String>(1)?, r.get::<_, String>(2)?))))?;
        rows.collect::<Result<_, _>>()?
    };
    let known: BTreeMap<String, String> = known_kinds.iter().map(|(k, v)| (k.clone(), v.0.clone())).collect();
    let present: BTreeSet<String> = inputs.iter().map(|i| i.source.clone()).collect();
    let gone: Vec<String> = known_kinds.iter().filter(|(s, (_, k))| !present.contains(*s) && kinds.contains(&k.as_str())).map(|(s, _)| s.clone()).collect();
    // Which documents changed (their text, the chunker, or the references a chunk carries).
    let mut todo: Vec<(Input, String, Vec<chunk::Chunk>)> = vec![];
    for i in inputs {
        let h = sha(&format!("{CHUNKER_VERSION}\0{MAX_CHARS}\0{}\0{}", i.md, i.refs));
        if known.get(&i.source) == Some(&h) {
            rep.unchanged += 1;
            continue;
        }
        if known.contains_key(&i.source) {
            rep.changed += 1;
        } else {
            rep.added += 1;
        }
        rep.touched.push(i.source.clone());
        let cs = chunk::chunk(&i.md, MAX_CHARS);
        todo.push((i, h, cs));
    }
    let model_changed = !model.is_empty() && meta(&c, "embed_model").is_some_and(|m| m != model);
    if todo.is_empty() && gone.is_empty() && !model_changed && emb.is_none() {
        return Ok(rep);
    }
    if !todo.is_empty() || !gone.is_empty() || model_changed {
        snapshot(path)?;
    }
    let tx = c.transaction()?;
    if model_changed {
        // Another model: every vector is from the old one.
        tx.execute("DELETE FROM chunks_vec", [])?;
        tx.execute("UPDATE chunks SET embedding_content_hash = NULL", [])?;
        tx.execute("DELETE FROM related", [])?;
    }
    for s in &gone {
        let ids: Vec<i64> = tx.prepare("SELECT id FROM chunks WHERE source = ?1")?.query_map([s], |r| r.get(0))?.collect::<Result<_, _>>()?;
        delete_chunks(&tx, &ids)?;
        tx.execute("DELETE FROM documents WHERE source = ?1", [s])?;
        rep.removed += 1;
        rep.gone.push(s.clone());
    }
    let now = chrono::Utc::now().to_rfc3339();
    for (i, h, cs) in &todo {
        let old: Vec<i64> = tx.prepare("SELECT id FROM chunks WHERE source = ?1")?.query_map([&i.source], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let new: BTreeSet<i64> = cs.iter().map(|x| chunk::chunk_id(&i.source, x.ord)).collect();
        delete_chunks(&tx, &old.iter().copied().filter(|id| !new.contains(id)).collect::<Vec<_>>())?;
        for x in cs {
            let id = chunk::chunk_id(&i.source, x.ord);
            let speaker = (i.speaker)(x);
            let ehash = sha(&format!("{}\0{}\0{}\0{}", x.title, x.body, x.breadcrumb, speaker.as_deref().unwrap_or("")));
            let prev: Option<Option<String>> = tx.query_row("SELECT embedding_content_hash FROM chunks WHERE id = ?1", [id], |r| r.get(0)).optional()?;
            let keep_vec = prev.clone().flatten().as_deref() == Some(ehash.as_str());
            if !keep_vec {
                tx.execute("DELETE FROM chunks_vec WHERE rowid = ?1", [id])?;
                // New text at this position: its topic and term tags, verdict and graph edges were the
                // old text's — gone, so the classifier tags it again.
                if prev.is_some() {
                    tx.execute("DELETE FROM chunk_topics WHERE chunk_id = ?1", [id])?;
                    tx.execute("DELETE FROM term_scan WHERE chunk_id = ?1", [id])?;
                    tx.execute("DELETE FROM chunk_verdicts WHERE chunk_id = ?1", [id])?;
                    tx.execute("DELETE FROM graph_edges WHERE source = ?1", [format!("chunk:{id}")])?;
                }
            }
            tx.execute(
                "INSERT INTO chunks(id, source, ord, title, text, sha, created_at, event_date, status, parent_heading, breadcrumb_path, speaker, link, refs, embedding_content_hash)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'ACTIVE',?9,?10,?11,?12,?13,?14)
                 ON CONFLICT(id) DO UPDATE SET source=excluded.source, ord=excluded.ord, title=excluded.title, text=excluded.text, sha=excluded.sha,
                   event_date=excluded.event_date, parent_heading=excluded.parent_heading, breadcrumb_path=excluded.breadcrumb_path, speaker=excluded.speaker,
                   link=excluded.link, refs=excluded.refs, embedding_content_hash=CASE WHEN ?15 THEN chunks.embedding_content_hash ELSE NULL END",
                // `sha` = the text the vector is made from; `embedding_content_hash` = the one it was made from.
                params![id, i.source, x.ord as i64, x.title, x.body, ehash, now, i.event_date, x.parent_heading, x.breadcrumb, speaker, (i.link)(x), i.refs.to_string(), Option::<String>::None, keep_vec],
            )?;
            write_scopes(&tx, id, &i.refs)?;
            tx.execute("DELETE FROM chunks_fts WHERE rowid = ?1", [id])?;
            tx.execute("INSERT INTO chunks_fts(rowid, text) VALUES (?1, ?2)", params![id, fts_text(&x.breadcrumb, speaker.as_deref().unwrap_or(""), &x.title, &x.body)])?;
            rep.chunks += 1;
        }
        tx.execute(
            "INSERT INTO documents(source, content_hash, indexed_at, title, kind, event_date, refs) VALUES (?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(source) DO UPDATE SET content_hash=excluded.content_hash, indexed_at=excluded.indexed_at, title=excluded.title,
               kind=excluded.kind, event_date=excluded.event_date, refs=excluded.refs",
            params![i.source, h, now, i.title, i.kind, i.event_date, i.refs.to_string()],
        )?;
        doc_facts(&tx, &i.source)?;
    }
    set_meta(&tx, "built_at", &now)?;
    set_meta(&tx, "chunker_version", CHUNKER_VERSION)?;
    tx.commit()?;
    // Vectors for every chunk without one, in batches (outside the transaction: the network is slow).
    if let Some(e) = emb {
        let missing: Vec<(i64, String, String)> = c
            .prepare("SELECT c.id, c.title || char(10) || char(10) || c.text, c.sha FROM chunks c LEFT JOIN chunks_vec v ON v.rowid = c.id WHERE v.rowid IS NULL")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        let mut changed_ids = vec![];
        for batch in missing.chunks(BATCH) {
            let texts: Vec<String> = batch.iter().map(|b| b.1.chars().take(8000).collect()).collect();
            match e.embed(&texts) {
                Ok(vs) => {
                    let tx = c.transaction()?;
                    for ((id, _, h), v) in batch.iter().zip(vs) {
                        tx.execute("INSERT OR REPLACE INTO chunks_vec(rowid, embedding) VALUES (?1, ?2)", params![id, blob(&v)])?;
                        tx.execute("UPDATE chunks SET embedding_content_hash = ?2 WHERE id = ?1", params![id, h])?;
                        changed_ids.push(*id);
                    }
                    set_meta(&tx, "embed_model", &model)?;
                    tx.commit()?;
                    rep.embedded += batch.len();
                }
                Err(err) => {
                    rep.embed_error = Some(format!("{err:#}"));
                    break;
                }
            }
        }
        if let Some(d) = c.query_row("SELECT length(embedding) / 4 FROM chunks_vec LIMIT 1", [], |r| r.get::<_, i64>(0)).optional()? {
            set_meta(&c, "embed_dim", &d.to_string())?;
        }
        if !changed_ids.is_empty() {
            build_related(&c, &changed_ids)?;
        }
    }
    Ok(rep)
}

/// Remove every document whose source starts with `prefix` (a source taken off a thread).
pub fn remove(path: &Path, prefix: &str) -> Result<usize> {
    let c = open(path)?;
    let docs: Vec<String> = c.prepare("SELECT source FROM documents WHERE source LIKE ?1")?.query_map([format!("{prefix}%")], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let tx = c.unchecked_transaction()?;
    tx.execute("DELETE FROM date_superseded WHERE source LIKE ?1", [format!("{prefix}%")]).ok();
    for s in &docs {
        let ids: Vec<i64> = tx.prepare("SELECT id FROM chunks WHERE source = ?1")?.query_map([s], |r| r.get(0))?.collect::<Result<_, _>>()?;
        delete_chunks(&tx, &ids)?;
        tx.execute("DELETE FROM documents WHERE source = ?1", [s])?;
    }
    tx.commit()?;
    Ok(docs.len())
}

/// Put a document under other threads / another client without reading it again: its refs and
/// its sections' scopes. Its hash is cleared, so the next read of it re-chunks (texts unchanged:
/// nothing is embedded again).
pub fn rescope(path: &Path, source: &str, refs: &Value) -> Result<()> {
    let c = open(path)?;
    let tx = c.unchecked_transaction()?;
    tx.execute("UPDATE documents SET refs = ?2, content_hash = '' WHERE source = ?1", params![source, refs.to_string()])?;
    tx.execute("UPDATE chunks SET refs = ?2 WHERE source = ?1", params![source, refs.to_string()])?;
    let ids: Vec<i64> = tx.prepare("SELECT id FROM chunks WHERE source = ?1")?.query_map([source], |r| r.get(0))?.collect::<Result<_, _>>()?;
    for id in ids {
        write_scopes(&tx, id, refs)?;
    }
    doc_facts(&tx, source)?;
    tx.commit()?;
    Ok(())
}

/// Every vector, by chunk id, with its source.
fn vectors(c: &Connection) -> Result<Vec<(i64, String, Vec<f32>)>> {
    let mut st = c.prepare("SELECT v.rowid, c.source, v.embedding FROM chunks_vec v JOIN chunks c ON c.id = v.rowid")?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, floats(&r.get::<_, Vec<u8>>(2)?))))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The nearest sections of other documents for these chunks (and these chunks among others'
/// neighbours): Brain's `build_related` (k = 6, ≥ 0.55, across documents).
fn build_related(c: &Connection, ids: &[i64]) -> Result<()> {
    let all = vectors(c)?;
    let by_id: BTreeMap<i64, usize> = all.iter().enumerate().map(|(i, x)| (x.0, i)).collect();
    let tx = c.unchecked_transaction()?;
    for id in ids {
        let Some(&i) = by_id.get(id) else { continue };
        let (_, src, v) = &all[i];
        let mut near: Vec<(f32, i64)> = all.iter().filter(|o| &o.1 != src).map(|o| (dot(v, &o.2), o.0)).filter(|(s, _)| *s >= RELATED_MIN).collect();
        near.sort_by(|a, b| b.0.total_cmp(&a.0));
        tx.execute("DELETE FROM related WHERE chunk_id = ?1", [id])?;
        for (s, o) in near.into_iter().take(RELATED_K) {
            tx.execute("INSERT OR REPLACE INTO related(chunk_id, related_id, score) VALUES (?1, ?2, ?3)", params![id, o, s])?;
            // The other side learns of it too when it is among its nearest.
            let worst: Option<f64> = tx.query_row("SELECT min(score) FROM related WHERE chunk_id = ?1 HAVING count(*) >= ?2", params![o, RELATED_K as i64], |r| r.get(0)).optional()?.flatten();
            if worst.is_none_or(|w| (s as f64) > w) {
                tx.execute("INSERT OR REPLACE INTO related(chunk_id, related_id, score) VALUES (?1, ?2, ?3)", params![o, id, s])?;
                tx.execute(
                    "DELETE FROM related WHERE chunk_id = ?1 AND related_id NOT IN (SELECT related_id FROM related WHERE chunk_id = ?1 ORDER BY score DESC LIMIT ?2)",
                    params![o, RELATED_K as i64],
                )?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// Words a query is never matched on: English, Ukrainian and Russian function words.
const STOPWORDS: &str = "
a an the and or but if of to in on at by for from with without into onto over under about as is are was were be been being
am do does did done doing have has had having i me my we our you your he she its they them their this that these those
there here what which who whom whose when where why how all any some not nor only so than too very could would
shall should might must just also more most other such own same again then once both each few s t don now up down out
off further while until because between through during before after above below vs via per let lets get got please
що як де коли чому хто який яка яке які це цей ця ці той та те ті і й а але або чи не ні так в у на з із зі до від по за
для про при над під між через без є був була було були буде будуть я ти ви ми він вона воно вони його її їх мене мені нам вам
їм свій своя своє свої все всі весь вся також ще вже лише тільки ж же би б якщо щоб бо то ось там тут
что как где когда почему кто какой какая какое какие это этот эта эти тот то те и но или ли ни да во с со к ко
от под между есть был была было были будет будут ты вы мы он она оно они ее её их
меня мне им его свой свое свои всё также еще ещё уже лишь только бы если чтобы потому вот";

fn stopword(w: &str) -> bool {
    static SET: std::sync::OnceLock<BTreeSet<&'static str>> = std::sync::OnceLock::new();
    SET.get_or_init(|| STOPWORDS.split_whitespace().collect()).contains(w)
}

/// An FTS query from free text: every word of any script (two letters or more) but the stopwords, OR-ed, quoted.
fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 2)
        .map(str::to_lowercase)
        .filter(|w| !stopword(w))
        .map(|w| format!("\"{w}\""))
        .collect();
    (!words.is_empty()).then(|| words.join(" OR "))
}

fn hit(c: &Connection, id: i64, score: f32) -> Result<Value> {
    c.query_row(
        "SELECT source, title, breadcrumb_path, text, event_date, speaker, link, refs, status FROM chunks WHERE id = ?1",
        [id],
        |r| {
            Ok(json!({"chunk_id": id.to_string(), "source": r.get::<_, String>(0)?, "section": r.get::<_, Option<String>>(1)?,
                      "breadcrumb_path": r.get::<_, Option<String>>(2)?, "text": r.get::<_, Option<String>>(3)?, "event_date": r.get::<_, Option<String>>(4)?,
                      "speaker": r.get::<_, Option<String>>(5)?, "link": r.get::<_, Option<String>>(6)?,
                      "refs": r.get::<_, Option<String>>(7)?.and_then(|s| serde_json::from_str::<Value>(&s).ok()), "status": r.get::<_, Option<String>>(8)?,
                      "score": (score * 10000.0).round() / 10000.0}))
        },
    )
    .map_err(Into::into)
}

/// Search: BM25 over every script for the candidates, re-ranked by the query's vector among them (`rerank`).
/// `source_contains` narrows to sources whose id contains it; without an embedder, the keyword order.
#[cfg(test)]
pub fn search(path: &Path, q: &str, limit: usize, source_contains: Option<&str>, emb: Option<&dyn Embedder>) -> Result<Value> {
    search_in(path, q, limit, source_contains, None, emb)
}

/// Search within a client or a thread (`scope`), as `search`.
/// Which sections a search sees (Brain's `latest_only` / `as_of`): the current ones (superseded
/// sections hidden), all of them, or what was current on a day (sections dated after it, or
/// superseded on or before it, hidden).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum History<'a> {
    #[default]
    Latest,
    All,
    AsOf(&'a str),
}

fn hidden(c: &Connection, h: History) -> Result<BTreeSet<i64>> {
    let ids = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Result<BTreeSet<i64>> { Ok(c.prepare(sql)?.query_map(p, |r| r.get(0))?.collect::<Result<_, _>>()?) };
    match h {
        History::Latest => ids("SELECT id FROM chunks WHERE status = 'SUPERSEDED'", &[]),
        History::All => Ok(BTreeSet::new()),
        History::AsOf(d) => ids("SELECT id FROM chunks WHERE (valid_to IS NOT NULL AND valid_to <= ?1) OR (event_date IS NOT NULL AND event_date > ?1)", &[&d]),
    }
}

/// Mark sections of documents dated before their cutoff `SUPERSEDED` (`valid_to` = the cutoff; each
/// recorded in `date_superseded`, so only Echo's own marks are ever undone), and bring back the ones
/// whose cutoff moved or went. `cutoff(source, refs)` says a document's cutoff, if any. Undated
/// documents are never superseded (a date is never guessed). Returns (marked, restored) sections.
pub fn supersede(path: &Path, cutoff: &dyn Fn(&str, &Value) -> Option<String>) -> Result<(usize, usize)> {
    let c = open(path)?;
    c.execute_batch("CREATE TABLE IF NOT EXISTS date_superseded(chunk_id INTEGER PRIMARY KEY, source TEXT, cutoff TEXT);")?;
    let docs: Vec<(String, Option<String>, Option<String>)> =
        c.prepare("SELECT source, event_date, refs FROM documents")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut want: BTreeMap<i64, (String, String)> = BTreeMap::new();
    for (src, date, refs) in &docs {
        let refs: Value = refs.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
        let (Some(date), Some(cut)) = (date.as_deref(), cutoff(src, &refs)) else { continue };
        if date < cut.as_str() {
            let ids: Vec<i64> = c.prepare("SELECT id FROM chunks WHERE source = ?1")?.query_map([src], |r| r.get(0))?.collect::<Result<_, _>>()?;
            for id in ids {
                want.insert(id, (src.clone(), cut.clone()));
            }
        }
    }
    let had: BTreeMap<i64, String> = c.prepare("SELECT chunk_id, cutoff FROM date_superseded")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let tx = c.unchecked_transaction()?;
    let (mut marked, mut restored) = (0, 0);
    for (id, cut) in &had {
        if want.get(id).map(|w| &w.1) != Some(cut) {
            tx.execute("UPDATE chunks SET status = 'ACTIVE', valid_to = NULL WHERE id = ?1 AND status = 'SUPERSEDED'", [id])?;
            tx.execute("DELETE FROM date_superseded WHERE chunk_id = ?1", [id])?;
            restored += usize::from(!want.contains_key(id));
        }
    }
    for (id, (src, cut)) in &want {
        if had.get(id) != Some(cut) {
            tx.execute("UPDATE chunks SET status = 'SUPERSEDED', valid_to = ?2 WHERE id = ?1", params![id, cut])?;
            tx.execute("INSERT OR REPLACE INTO date_superseded(chunk_id, source, cutoff) VALUES (?1, ?2, ?3)", params![id, src, cut])?;
            marked += 1;
        }
    }
    tx.commit()?;
    Ok((marked, restored))
}

pub fn search_in(path: &Path, q: &str, limit: usize, source_contains: Option<&str>, scope: Scope, emb: Option<&dyn Embedder>) -> Result<Value> {
    search_at(path, q, limit, source_contains, scope, emb, History::Latest, None)
}

/// A search: `only` — the sections a reader may see (its AI profile's, `access.rs`), filtered
/// before the cut so others never crowd them out.
#[allow(clippy::too_many_arguments)]
pub fn search_at(path: &Path, q: &str, limit: usize, source_contains: Option<&str>, scope: Scope, emb: Option<&dyn Embedder>, history: History, only: Option<&BTreeSet<i64>>) -> Result<Value> {
    let c = open(path)?;
    let allowed = in_scope(&c, scope)?;
    let hide = hidden(&c, history)?;
    let ok = |id: &i64| allowed.as_ref().is_none_or(|a| a.contains(id)) && !hide.contains(id) && only.is_none_or(|o| o.contains(id));
    let like = source_contains.map(|s| format!("%{s}%")).unwrap_or_else(|| "%".into());
    // The candidates, in keyword order: matches read best first until enough of them may be seen (history and the
    // reader's profile are checked here, so sections it can't see never crowd out the ones it can).
    let want = CANDIDATES.max(limit);
    let mut cands: Vec<i64> = vec![];
    let mut note = None;
    match fts_query(q) {
        Some(fq) => {
            let mut st = c.prepare(
                "SELECT f.rowid FROM chunks_fts f JOIN chunks c ON c.id = f.rowid WHERE chunks_fts MATCH ?1 AND c.source LIKE ?2
                 AND (?3 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = c.id AND s.kind = ?3 AND s.id = ?4)) ORDER BY bm25(chunks_fts)",
            )?;
            let mut rows = st.query(params![fq, like, scope.map(|s| s.0), scope.map(|s| s.1)])?;
            while cands.len() < want {
                let Some(r) = rows.next()? else { break };
                let id: i64 = r.get(0)?;
                if ok(&id) {
                    cands.push(id);
                }
            }
        }
        None => note = Some("only common words; nothing to search".to_string()),
    }
    let mut ranked: Vec<(i64, f32)> = cands.iter().enumerate().map(|(rank, id)| (*id, W_FTS / (RRF_K + rank as f32 + 1.0))).collect();
    match emb {
        Some(e) if !ranked.is_empty() => match e.embed(&[q.to_string()]) {
            Ok(v) => rerank(&c, &mut ranked, &v[0])?,
            Err(err) => note = Some(format!("keywords only: {err:#}")),
        },
        Some(_) => {}
        None if note.is_none() => note = Some("keywords only: no embedding server is set (Settings → Recall)".into()),
        None => {}
    }
    // A closed thread's sections are its archive (#55): found, marked, ranked under what is current — unless the search
    // is within that thread.
    let closed = crate::projects::closed_threads();
    let archived = archived_of(&c, &cands, &closed)?;
    let within_closed = scope.is_some_and(|s| s.0 == "thread" && closed.contains_key(s.1));
    for (id, s) in ranked.iter_mut() {
        if archived.contains_key(id) && !within_closed {
            *s *= ARCHIVE_WEIGHT;
        }
    }
    // Stable: a tie keeps the keyword order.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let hits: Vec<Value> = ranked
        .into_iter()
        .take(limit)
        .map(|(id, s)| {
            let mut h = hit(&c, id, s)?;
            if let Some(t) = archived.get(&id) {
                h["closed"] = json!(t);
            }
            Ok(h)
        })
        .collect::<Result<_>>()?;
    let history = match history {
        History::Latest => json!("latest"),
        History::All => json!("all"),
        History::AsOf(d) => json!({"as_of": d}),
    };
    Ok(json!({"query": q, "scope": scope.map(|s| json!({"kind": s.0, "id": s.1})), "history": history, "hits": hits, "count": hits.len(), "note": note}))
}

/// Re-rank the keyword candidates (`ranked`: id and keyword score, in keyword order) by their vectors' nearness to the
/// query's, among themselves: a candidate without a vector keeps its keyword score; nothing is added.
fn rerank(c: &Connection, ranked: &mut [(i64, f32)], qv: &[f32]) -> Result<()> {
    let mut st = c.prepare("SELECT embedding FROM chunks_vec WHERE rowid = ?1")?;
    let mut near: Vec<(f32, i64)> = vec![];
    for (id, _) in ranked.iter() {
        if let Some(v) = st.query_row([id], |r| r.get::<_, Vec<u8>>(0)).optional()?.map(|b| floats(&b)).filter(|v| v.len() == qv.len()) {
            near.push((dot(qv, &v), *id));
        }
    }
    near.sort_by(|a, b| b.0.total_cmp(&a.0));
    let rank: BTreeMap<i64, usize> = near.iter().enumerate().map(|(r, (_, id))| (*id, r)).collect();
    for (id, s) in ranked.iter_mut() {
        if let Some(r) = rank.get(id) {
            *s += W_VEC / (RRF_K + *r as f32 + 1.0);
        }
    }
    Ok(())
}

/// How much a closed thread's section weighs in a search against a current one.
const ARCHIVE_WEIGHT: f32 = 0.5;

/// The sections that belong only to closed threads (in a thread, and every thread of theirs closed): id → those
/// threads (`closed_threads`' entries).
pub fn archived_of(c: &Connection, ids: &[i64], closed: &BTreeMap<String, Value>) -> Result<BTreeMap<i64, Vec<Value>>> {
    let mut out = BTreeMap::new();
    if closed.is_empty() || ids.is_empty() {
        return Ok(out);
    }
    let mut st = c.prepare("SELECT id FROM scopes WHERE chunk_id = ?1 AND kind = 'thread' AND id != '-'")?;
    for id in ids {
        let threads: Vec<String> = st.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        if !threads.is_empty() && threads.iter().all(|t| closed.contains_key(t)) {
            out.insert(*id, threads.iter().filter_map(|t| closed.get(t).cloned()).collect());
        }
    }
    Ok(out)
}

/// The sections nearest to a chunk, from other documents.
pub fn related(path: &Path, id: i64, limit: usize) -> Result<Value> {
    related_in(path, id, limit, None)
}

/// The nearest sections within a client or a thread.
pub fn related_in(path: &Path, id: i64, limit: usize, scope: Scope) -> Result<Value> {
    let c = open(path)?;
    let anchor = hit(&c, id, 1.0)?;
    let rows: Vec<(i64, f32)> = c
        .prepare("SELECT r.related_id, r.score FROM related r WHERE r.chunk_id = ?1
                  AND (?3 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = r.related_id AND s.kind = ?3 AND s.id = ?4)) ORDER BY r.score DESC LIMIT ?2")?
        .query_map(params![id, limit as i64, scope.map(|s| s.0), scope.map(|s| s.1)], |r| Ok((r.get(0)?, r.get::<_, f64>(1)? as f32)))?
        .collect::<Result<_, _>>()?;
    let related: Vec<Value> = rows.into_iter().map(|(o, s)| hit(&c, o, s)).collect::<Result<_>>()?;
    Ok(json!({"anchor": anchor, "related": related}))
}

/// The clients and threads in the Brain: each with its documents and sections (names from Echo's own
/// files, so a renamed thread shows its new name).
pub fn scopes(path: &Path) -> Result<Value> {
    // Counted over every section (`kept`: seconds on millions); the names are read fresh.
    let p = path.to_path_buf();
    let rows = kept(format!("scopes|{}", path.display()), changed_stamp(path), move || -> Result<Vec<(String, String, i64, i64)>> {
        Ok(open(&p)?
            .prepare("SELECT s.kind, s.id, count(DISTINCT ch.source), count(*) FROM scopes s JOIN chunks ch ON ch.id = s.chunk_id GROUP BY s.kind, s.id")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?)
    })?;
    let clients = crate::projects::clients().unwrap_or_default();
    let threads = crate::projects::list();
    let name = |k: &str, id: &str| -> String {
        if id == "-" {
            return if k == "client" { "No client".into() } else { "In no thread".into() };
        }
        if k == "client" {
            clients.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_else(|| id.to_string())
        } else {
            threads.iter().find(|p| p.id == id).map(|p| p.name.clone()).unwrap_or_else(|| id.to_string())
        }
    };
    let pick = |k: &str| -> Vec<Value> {
        rows.iter()
            .filter(|r| r.0 == k)
            .map(|r| {
                let client = (k == "thread").then(|| threads.iter().find(|p| p.id == r.1).and_then(|p| p.client.clone())).flatten();
                json!({"id": r.1, "name": name(k, &r.1), "documents": r.2, "sections": r.3, "client": client})
            })
            .collect()
    };
    Ok(json!({"clients": pick("client"), "threads": pick("thread")}))
}

/// What the store holds.
pub fn status(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({"exists": false}));
    }
    let c = open(path)?;
    let n = |sql: &str| -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    let kinds: BTreeMap<String, i64> = c
        .prepare("SELECT kind, count(*) FROM documents GROUP BY kind")?
        .query_map([], |r| Ok((r.get::<_, Option<String>>(0)?.unwrap_or_default(), r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "exists": true, "documents": n("SELECT count(*) FROM documents"), "by_kind": kinds,
        "chunks": n("SELECT count(*) FROM chunks"), "embedded": n("SELECT count(*) FROM chunks_vec"),
        "related": n("SELECT count(*) FROM related"), "graph_nodes": n("SELECT count(*) FROM graph_nodes"),
        "chunk_topics": n("SELECT count(*) FROM chunk_topics"),
        "embed_model": meta(&c, "embed_model"), "embed_dim": meta(&c, "embed_dim"), "built_at": meta(&c, "built_at"),
        "bytes": std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    }))
}

#[cfg(test)]
mod tests {
    use super::super::embed::Words;
    use super::*;

    #[test]
    fn a_memo_section_is_its_part_thread_only() {
        let refs = json!({"kind": "call", "call": "m", "client": "acme",
            "threads": [{"id": "acme-rollout", "name": "Acme rollout", "client": "acme"}, {"id": "globex", "name": "Globex discovery", "client": "glx"}],
            "memo_parts": [{"thread": "acme-rollout", "from": 0.0, "to": 13.0}, {"thread": "globex", "from": 13.0, "to": 21.0}]});
        let ids = |r: Option<Value>| r.map(|r| scopes_of(&r).into_iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>());
        assert_eq!(ids(memo_section_refs(&refs, "[00:14] Me")), Some(vec!["thread:globex".into(), "client:glx".into()]));
        assert_eq!(ids(memo_section_refs(&refs, "[00:02] Me")), Some(vec!["thread:acme-rollout".into(), "client:acme".into()]));
        assert_eq!(ids(memo_section_refs(&refs, "[00:30] Me")), Some(vec!["thread:-".into(), "client:-".into()]), "about no thread");
        assert_eq!(ids(memo_section_refs(&refs, "Globex discovery (00:13–00:21)")), Some(vec!["thread:globex".into(), "client:glx".into()]));
        assert!(memo_section_refs(&refs, "Summary").is_none(), "the summary stays the memo's");
    }

    fn input(source: &str, md: &str) -> Input {
        let s = source.to_string();
        Input {
            source: s.clone(),
            kind: "call".into(),
            title: s.clone(),
            event_date: Some("2026-10-28".into()),
            md: md.into(),
            refs: json!({"threads": [{"id": "aurora"}]}),
            link: Box::new(move |c| format!("echo://call/{s}?ord={}", c.ord)),
            speaker: Box::new(|_| None),
        }
    }

    #[test]
    fn kept_answers_at_once_and_counts_again_behind() {
        let key = format!("test-kept-{}", std::process::id());
        assert_eq!(*kept(key.clone(), "v1".into(), || Ok(1)).unwrap(), 1);
        assert_eq!(*kept(key.clone(), "v1".into(), || -> Result<i32> { panic!("unchanged: never counted again") }).unwrap(), 1);
        // Changed, and slow to count: the last answer comes back within the short wait, the new one later.
        let t = std::time::Instant::now();
        let (v, stale) = noting_stale(|| {
            kept(key.clone(), "v2".into(), || {
                std::thread::sleep(std::time::Duration::from_millis(700));
                Ok(2)
            })
            .unwrap()
        });
        assert_eq!((*v, stale, t.elapsed() < std::time::Duration::from_millis(600)), (1, true, true), "the last answer, marked stale");
        std::thread::sleep(std::time::Duration::from_millis(900));
        assert_eq!(*kept(key.clone(), "v2".into(), || Ok(3)).unwrap(), 2);
        // Changed and quick: the fresh answer.
        assert_eq!(*kept(key, "v3".into(), || Ok(4)).unwrap(), 4);
    }

    #[test]
    fn sync_search_related_and_deltas() {
        let dir = std::env::temp_dir().join(format!("echo-brain-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("knowledge.sqlite");
        let a = "# Aurora\n## SSO\nSingle sign-on for the Berlin pilot lands before the security review.\n## Budget\nДана подтвердила бюджет на пилот в Берлине.\n";
        let b = "# Helios\n## Tenants\nTenant admin consent for the Berlin pilot and single sign-on.\n";
        let r = sync(&db, vec![input("call/a", a), input("call/b", b)], Some(&Words)).unwrap();
        assert_eq!((r.added, r.chunks, r.embedded), (2, 3, 3), "{r:?}");
        // Nothing changed: nothing done.
        let r = sync(&db, vec![input("call/a", a), input("call/b", b)], Some(&Words)).unwrap();
        assert_eq!((r.unchanged, r.chunks, r.embedded), (2, 0, 0));
        // Keywords in any script.
        let h = search(&db, "бюджет", 5, None, None).unwrap();
        assert_eq!(h["hits"][0]["section"], "Budget", "{h}");
        assert_eq!(h["hits"][0]["link"], "echo://call/call/a?ord=1");
        // Fused: both lanes agree on the SSO sections.
        let h = search(&db, "single sign-on pilot", 5, None, Some(&Words)).unwrap();
        let top: Vec<&str> = h["hits"].as_array().unwrap().iter().take(2).map(|x| x["section"].as_str().unwrap()).collect();
        assert!(top.contains(&"SSO") && top.contains(&"Tenants"), "{h}");
        assert_eq!(search(&db, "single sign-on", 5, Some("call/b"), Some(&Words)).unwrap()["count"], 1);
        // Related across documents.
        let sso = h["hits"].as_array().unwrap().iter().find(|x| x["section"] == "SSO").unwrap()["chunk_id"].as_str().unwrap().parse::<i64>().unwrap();
        let rel = related(&db, sso, 6).unwrap();
        assert_eq!(rel["related"][0]["section"], "Tenants", "{rel}");
        // One section changed: only it is embedded again; a document gone is removed.
        let a2 = a.replace("before the security review", "after the security review");
        let r = sync(&db, vec![input("call/a", &a2)], Some(&Words)).unwrap();
        assert_eq!((r.changed, r.removed, r.chunks, r.embedded), (1, 1, 2, 1), "{r:?}");
        let st = status(&db).unwrap();
        assert_eq!((st["documents"].as_i64(), st["chunks"].as_i64(), st["embedded"].as_i64()), (Some(1), Some(2), Some(2)), "{st}");
        assert_eq!(search(&db, "tenant", 5, None, None).unwrap()["count"], 0);
        assert!(dir.join("snapshots").read_dir().unwrap().count() >= 1);
        // Without an embedder the text is still indexed; vectors come later.
        let r = sync(&db, vec![input("call/a", &a2), input("call/c", "# C\n## X\nСпринт закрыт.\n")], None).unwrap();
        assert_eq!((r.added, r.embedded), (1, 0));
        assert_eq!(status(&db).unwrap()["embedded"], 2);
        let r = sync(&db, vec![input("call/a", &a2), input("call/c", "# C\n## X\nСпринт закрыт.\n")], Some(&Words)).unwrap();
        assert_eq!(r.embedded, 1, "the missing vector is made now");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_closed_threads_sections_are_its_archive() {
        let (_env, root) = crate::paths::testenv::folders("brain-archive");
        let db = root.join("knowledge.sqlite");
        let open_t = crate::projects::create("Agents pilot", None, None).unwrap().id;
        let closed_t = crate::projects::create("Hooli workshop", None, None).unwrap().id;
        crate::projects::set_stage(&closed_t, crate::projects::Stage::Done, Some("wrapped up")).unwrap();
        let mut a = input("call/a", "# A\n## Gateway\nThe gateway goes in before the pilot.\n");
        a.refs = json!({"threads": [{"id": open_t}]});
        let mut b = input("call/b", "# B\n## Gateway\nThe gateway pattern for the procurement bots, the gateway again.\n");
        b.refs = json!({"threads": [{"id": closed_t}]});
        let mut both = input("call/c", "# C\n## Gateway\nA gateway shared by both.\n");
        both.refs = json!({"threads": [{"id": open_t}, {"id": closed_t}]});
        sync(&db, vec![a, b, both], Some(&Words)).unwrap();
        let h = search_in(&db, "gateway", 10, None, None, None).unwrap();
        let hits = h["hits"].as_array().unwrap().clone();
        assert_eq!(hits.len(), 3, "found, all of it");
        let closed_hit = hits.iter().find(|x| x["source"] == "call/b").unwrap();
        assert_eq!(closed_hit["closed"][0]["name"], "Hooli workshop", "marked");
        assert!(closed_hit["closed"][0]["closed_at"].is_string());
        assert!(hits.iter().filter(|x| x["source"] != "call/b").all(|x| x["closed"].is_null()), "one open thread keeps it current");
        assert_eq!(hits.last().unwrap()["source"], "call/b", "ranked under current ones");
        // Within that thread it isn't pushed down; reopened, it is current again (nothing read again).
        let within = search_in(&db, "gateway", 10, None, Some(("thread", closed_t.as_str())), None).unwrap();
        let own = within["hits"].as_array().unwrap().iter().find(|x| x["source"] == "call/b").unwrap();
        let outside = h["hits"].as_array().unwrap().iter().find(|x| x["source"] == "call/b").unwrap();
        assert!(own["closed"].is_array() && own["score"].as_f64() > outside["score"].as_f64(), "marked, not weighed down: {within}");
        crate::projects::set_stage(&closed_t, crate::projects::Stage::Active, Some("back")).unwrap();
        let h2 = search_in(&db, "gateway", 10, None, None, None).unwrap();
        assert!(h2["hits"].as_array().unwrap().iter().all(|x| x["closed"].is_null()));
    }

    #[test]
    fn client_and_thread_are_dimensions() {
        let dir = std::env::temp_dir().join(format!("echo-brain-scopes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let db = dir.join("knowledge.sqlite");
        let mut a = input("call/a", "# A\n## Budget\nThe budget for next year is not approved.\n");
        a.refs = json!({"threads": [{"id": "aurora", "client": "northwind"}]});
        let mut b = input("call/b", "# B\n## Budget\nThe budget for the tenants is approved.\n");
        b.refs = json!({"threads": [{"id": "helios", "client": "contoso"}]});
        let mut c = input("call/c", "# C\n## Budget\nA budget chat with nobody in particular.\n");
        c.refs = json!({"threads": []});
        sync(&db, vec![a, b, c], Some(&Words)).unwrap();
        assert_eq!(search_in(&db, "budget", 10, None, None, None).unwrap()["count"], 3);
        let h = search_in(&db, "budget", 10, None, Some(("client", "northwind")), Some(&Words)).unwrap();
        assert_eq!((h["count"].as_u64(), h["hits"][0]["source"].as_str()), (Some(1), Some("call/a")), "{h}");
        assert_eq!(search_in(&db, "budget", 10, None, Some(("thread", "helios")), Some(&Words)).unwrap()["hits"][0]["source"], "call/b");
        // History: a document dated before its cutoff is hidden from the latest view, seen with all
        // or as of a day before the cutoff; a cutoff taken away brings it back.
        let old = |src: &str, _: &Value| (src == "call/a").then(|| "2026-12-31".to_string());
        let (m, _) = supersede(&db, &old).unwrap();
        assert!(m > 0);
        let srcs = |h| search_at(&db, "budget", 10, None, None, None, h, None).unwrap()["hits"].as_array().unwrap().iter().map(|x| x["source"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert!(!srcs(History::Latest).contains(&"call/a".to_string()));
        assert!(srcs(History::All).contains(&"call/a".to_string()));
        let (_, back) = supersede(&db, &|_, _| None).unwrap();
        assert!(back > 0 && srcs(History::Latest).contains(&"call/a".to_string()));
        assert_eq!(search_in(&db, "budget", 10, None, Some(("thread", "-")), None).unwrap()["hits"][0]["source"], "call/c", "a call in no thread");
        let s = scopes(&db).unwrap();
        assert_eq!(s["clients"].as_array().unwrap().len(), 3, "{s}");
        // A call moved to another thread: its sections follow (the refs are part of its hash).
        let mut a2 = input("call/a", "# A\n## Budget\nThe budget for next year is not approved.\n");
        a2.refs = json!({"threads": [{"id": "helios", "client": "contoso"}]});
        sync(&db, vec![a2], None).unwrap();
        assert_eq!(search_in(&db, "budget", 10, None, Some(("client", "northwind")), None).unwrap()["count"], 0);
        assert_eq!(search_in(&db, "next year", 10, None, Some(("thread", "helios")), None).unwrap()["count"], 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stopwords_are_never_matched_in_english_ukrainian_or_russian() {
        assert_eq!(fts_query("What did we decide about the Gateway?").as_deref(), Some(r#""decide" OR "gateway""#));
        assert_eq!(fts_query("що ми вирішили про бюджет").as_deref(), Some(r#""вирішили" OR "бюджет""#));
        assert_eq!(fts_query("что мы решили по бюджету").as_deref(), Some(r#""решили" OR "бюджету""#));
        assert_eq!(fts_query("who is this, and when?"), None, "nothing but stopwords: no keyword search");
        assert_eq!(fts_query("can it go live, or will they say no?").as_deref(), Some(r#""can" OR "it" OR "go" OR "live" OR "will" OR "say" OR "no""#), "words that carry meaning in a question stay");
        let (_env, root) = crate::paths::testenv::folders("brain-stopwords");
        let db = root.join("knowledge.sqlite");
        sync(&db, vec![input("call/a", "# A\n## Notes\nWho is this and when?\n")], None).unwrap();
        let h = search_in(&db, "who is this, and when?", 5, None, None, None).unwrap();
        assert_eq!((h["count"].as_u64(), h["note"].as_str()), (Some(0), Some("only common words; nothing to search")), "{h}");
    }

    #[test]
    fn minutes_header_lines_and_echo_links_are_not_indexed() {
        let body = "**Who:** [Dana Ruiz](echo://person/dana@acme.example), Lee Park (Me)\n**Thread:** [Acme rollout](echo://thread/acme-rollout) · [Billing](echo://thread/billing)\nThreads: [Globex](echo://thread/globex)\nWho owns the gateway? Dana does.\nSee [the kickoff](echo://call/2026-10-01_090000?t=95) and `echo://call/x?t=5` too.";
        let t = fts_text("Acme kickoff > Summary", "", "Summary", body);
        assert!(!t.contains("echo://") && !t.contains("Ruiz") && !t.contains("rollout") && !t.contains("Globex"), "{t}");
        assert!(t.contains("Who owns the gateway? Dana does.") && t.contains("See the kickoff and   too."), "{t}");
        assert!(t.contains("Lee Park (Me)") && !t.contains("Who:"), "what a header line says besides its links stays: {t}");
        assert_eq!(header_rest("  **Client**: [Acme](echo://client/acme)").as_deref(), Some(" "));
        assert_eq!((header_rest("Whoever links echo://call/x"), header_rest("Who: Dana")), (None, None), "not header lines");
        // An unclosed link stops at its line's end.
        let t = fts_text("", "", "", "See [the kickoff](echo://call/x?t=95 unclosed\nThe gateway (phase two) slips.");
        assert!(t.contains("The gateway (phase two) slips."), "{t}");

        // A store written before: its keyword text is written again once, from the sections it keeps.
        let (_env, root) = crate::paths::testenv::folders("brain-fts-text");
        let db = root.join("knowledge.sqlite");
        sync(&db, vec![input("call/a", &format!("# A\n## Summary\n{body}\n")), input("call/b", "# B\n## Notes\nRuiz sent the gateway estimate.\n")], None).unwrap();
        let srcs = |q| search_in(&db, q, 5, None, None, None).unwrap()["hits"].as_array().unwrap().iter().map(|h| h["source"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(srcs("Ruiz"), ["call/b"], "a name only in the header isn't a match");
        let c = open(&db).unwrap();
        let id: i64 = c.query_row("SELECT id FROM chunks WHERE source = 'call/a'", [], |r| r.get(0)).unwrap();
        c.execute("UPDATE chunks_fts SET text = ?2 WHERE rowid = ?1", params![id, body]).unwrap();
        set_meta(&c, "fts_text", "1").unwrap();
        // Another process writing: the rewrite neither waits nor fails, and leaves it for a later open.
        let other = Connection::open(&db).unwrap();
        other.execute_batch("BEGIN IMMEDIATE").unwrap();
        let t0 = std::time::Instant::now();
        rewrite_fts(&c).unwrap();
        assert!(t0.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(meta(&c, "fts_text").as_deref(), Some("1"));
        other.execute_batch("COMMIT").unwrap();
        drop(c);
        assert_eq!(srcs("Ruiz"), ["call/b"], "written again at a later open");
        assert_eq!(meta(&open(&db).unwrap(), "fts_text").as_deref(), Some(FTS_TEXT_VERSION));
    }

    #[test]
    fn reranking_never_adds_a_section_the_keywords_did_not_find() {
        let (_env, root) = crate::paths::testenv::folders("brain-rerank");
        let db = root.join("knowledge.sqlite");
        let docs = [
            ("call/a", "# A\n## Budget\nThe budget for the pilot is approved.\n"),
            ("call/b", "# B\n## Budget\nBudget, budget and the budget again: the budget slipped.\n"),
            // Nearest to the query's vector (its words, stopwords too), with none of its keywords.
            ("call/c", "# C\n## Plan\nThe pilot for the plan and the rollout of the pilot.\n"),
        ];
        sync(&db, docs.iter().map(|(s, m)| input(s, m)).collect(), Some(&Words)).unwrap();
        let srcs = |emb: Option<&dyn Embedder>| -> BTreeSet<String> {
            search_in(&db, "the budget", 10, None, None, emb).unwrap()["hits"].as_array().unwrap().iter().map(|h| h["source"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(srcs(None), BTreeSet::from(["call/a".to_string(), "call/b".to_string()]));
        assert_eq!(srcs(Some(&Words)), srcs(None), "the same sections, re-ranked");
        assert_eq!(search_in(&db, "rollout of the plan", 10, None, None, Some(&Words)).unwrap()["hits"][0]["source"], "call/c");
        assert_eq!(search_in(&db, "zeppelin", 10, None, None, Some(&Words)).unwrap()["count"], 0, "no keyword match: nothing, whatever is nearest");
    }

    /// An embedder that puts the query "budget" and every text about the gateway in one place, everything else in another.
    struct Gateway;
    impl Embedder for Gateway {
        fn model(&self) -> String {
            "test-gateway".into()
        }
        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts.iter().map(|t| if t == "budget" || t.contains("gateway") { vec![1.0, 0.0] } else { vec![0.0, 1.0] }).collect())
        }
    }

    #[test]
    fn the_vectors_reorder_the_keyword_candidates() {
        let (_env, root) = crate::paths::testenv::folders("brain-reorder");
        let db = root.join("knowledge.sqlite");
        let docs = [("call/a", "# A\n## Notes\nBudget: the budget, the budget.\n"), ("call/b", "# B\n## Notes\nThe budget for the new gateway goes to the board next month with the rest of the plan.\n")];
        sync(&db, docs.iter().map(|(s, m)| input(s, m)).collect(), Some(&Gateway)).unwrap();
        let order = |emb: Option<&dyn Embedder>| -> Vec<String> {
            search_in(&db, "budget", 5, None, None, emb).unwrap()["hits"].as_array().unwrap().iter().map(|h| h["source"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(order(None), ["call/a", "call/b"], "keyword order");
        assert_eq!(order(Some(&Gateway)), ["call/b", "call/a"], "the nearer one first");
    }

    #[test]
    fn sections_a_reader_may_not_see_never_crowd_out_the_ones_it_may() {
        let (_env, root) = crate::paths::testenv::folders("brain-only");
        let db = root.join("knowledge.sqlite");
        // 1,300 short sections that match better than the one the reader may see (more than any cap would read).
        let mut md = String::from("# Other\n");
        for i in 0..1300 {
            md.push_str(&format!("## S{i}\nGateway.\n"));
        }
        sync(&db, vec![input("call/other", &md), input("call/mine", "# Mine\n## Notes\nThe gateway goes in after the long security review of the pilot.\n")], None).unwrap();
        let c = open(&db).unwrap();
        let mine: BTreeSet<i64> = c.prepare("SELECT id FROM chunks WHERE source = 'call/mine'").unwrap().query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
        let h = search_at(&db, "gateway", 5, None, None, None, History::Latest, Some(&mine)).unwrap();
        assert_eq!((h["count"].as_u64(), h["hits"][0]["source"].as_str()), (Some(1), Some("call/mine")), "{h}");
        // Asked for more than 30: as many as asked.
        assert_eq!(search_in(&db, "gateway", 50, None, None, None).unwrap()["count"], 50);
        // A rewrite of the keyword text in several batches ends with every section's.
        set_meta(&c, "fts_text", "1").unwrap();
        drop(c);
        let c = open(&db).unwrap();
        assert_eq!((meta(&c, "fts_text").as_deref(), meta(&c, "fts_text_next")), (Some(FTS_TEXT_VERSION), None));
        assert_eq!(c.query_row("SELECT count(*) FROM chunks_fts", [], |r| r.get::<_, i64>(0)).unwrap(), 1301);
        assert_eq!(search_at(&db, "gateway", 5, None, None, None, History::Latest, Some(&mine)).unwrap()["count"], 1);
    }

    /// The made-up regression set (intake-and-search R11): a small store of acme.example calls and mail, each query with
    /// the source it must find first, with and without an embedding server.
    #[test]
    fn the_made_up_regression_set_finds_its_sources_first() {
        let (_env, root) = crate::paths::testenv::folders("brain-golden");
        let db = root.join("knowledge.sqlite");
        let head = "**Who:** [Dana Ruiz](echo://person/dana@acme.example), [Lee Park](echo://person/lee@acme.example), [Priya Shah](echo://person/priya@acme.example)\n**Thread:** [Acme rollout](echo://thread/acme-rollout)\n";
        let docs: Vec<(&str, &str, String)> = vec![
            ("call/2026-10-01_090000", "acme", format!("# Acme kickoff\n{head}\n## Summary\nThe team met to plan the first quarter.\n\n## Decisions\n- We decided the billing gateway goes first, before the portal.\n")),
            ("call/2026-10-08_090000", "acme", format!("# Acme weekly\n{head}\n## Summary\nThe pilot go-live date moved to 24 November after the load test.\n\n## Action items\n- Priya owns the security review and sends the findings by Friday.\n")),
            ("call/2026-10-15_090000", "acme", format!("# Acme status\n{head}\n## Summary\nThe rollout status is amber: two stores are blocked on network access.\n")),
            ("call/2026-10-20_090000", "acme", format!("# Acme sync\n{head}\n## Transcript\nDana: I can take the vendor contract renewal myself.\n")),
            ("call/2026-10-22_090000", "acme", format!("# Acme Kyiv\n{head}\n## Підсумок\nОбговорили розподіл бюджету на пілот у Києві.\n")),
            ("call/2026-10-23_090000", "acme", format!("# Acme Moscow desk\n{head}\n## Итоги\nСтатус миграции данных: перенос завершён на восемьдесят процентов.\n")),
            ("call/2026-10-24_090000", "globex", "# Globex review\n**Thread:** [Globex discovery](echo://thread/globex)\n\n## Summary\nThe budget for the discovery phase is approved by finance.\n".to_string()),
            ("call/2026-10-27_090000", "acme", format!("# Acme budget\n{head}\n## Summary\nThe acme budget for next year waits on the board.\n")),
            ("mail/acme-procurement", "acme", "# Invoice matching\n\n## Mail\nThe procurement bots match each invoice to its purchase order before payment.\n".to_string()),
            ("call/2026-10-29_090000", "acme", format!("# Helios tenants\n{head}\n## Summary\nTenant admin consent is needed before single sign-on works.\n")),
            ("mail/globex-scope", "globex", "# Scope\n\n## Mail\nThe Globex discovery scope covers the three warehouses and the returns flow.\n".to_string()),
            // Distractors: short sections of stopwords, and of header lines around another subject.
            ("call/2026-10-30_090000", "globex", "# Globex lunch\n**Thread:** [Globex discovery](echo://thread/globex)\n**Who:** [Dana Ruiz](echo://person/dana@acme.example)\n\n## Notes\nLunch moved.\n".to_string()),
            ("call/2026-10-31_090000", "acme", "# Hallway\n## Notes\nWhat is it, who is it for, and when? What did we say we would do about it?\n".to_string()),
            ("call/2026-11-01_090000", "acme", "# Чат\n## Нотатки\nЩо з цим? Що ми про це казали, що з того?\n".to_string()),
            ("call/2026-11-02_090000", "acme", "# Болтовня\n## Заметки\nКакой там статус, какой он был и какой будет?\n".to_string()),
        ];
        let inputs = docs
            .iter()
            .map(|(s, client, md)| {
                let mut i = input(s, md);
                i.refs = json!({"threads": [{"id": format!("{client}-thread"), "client": client}]});
                i
            })
            .collect();
        sync(&db, inputs, Some(&Words)).unwrap();
        let golden: [(&str, Scope, &str); 12] = [
            ("what did we decide about the billing gateway", None, "call/2026-10-01_090000"),
            ("when is the pilot go-live date", None, "call/2026-10-08_090000"),
            ("who owns the security review", None, "call/2026-10-08_090000"),
            ("what is the rollout status", None, "call/2026-10-15_090000"),
            ("what did Dana take on", None, "call/2026-10-20_090000"),
            ("що з бюджету на пілот", None, "call/2026-10-22_090000"),
            ("какой статус миграции данных", None, "call/2026-10-23_090000"),
            ("budget approved", Some(("client", "globex")), "call/2026-10-24_090000"),
            ("how do the procurement bots match an invoice", None, "mail/acme-procurement"),
            ("tenant consent for single sign-on", None, "call/2026-10-29_090000"),
            ("which warehouses are in the Globex discovery scope", None, "mail/globex-scope"),
            ("what is Dana Ruiz doing about this", None, "call/2026-10-20_090000"),
        ];
        for emb in [None, Some(&Words as &dyn Embedder)] {
            for (q, scope, want) in golden {
                let h = search_in(&db, q, 5, None, scope, emb).unwrap();
                assert_eq!(h["hits"][0]["source"], want, "{q} (embeddings: {}): {h}", emb.is_some());
            }
        }
    }
}

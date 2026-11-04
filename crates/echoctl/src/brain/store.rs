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
/// Fusion: Brain's weighted reciprocal rank (k = 60), 0.4 keywords, 0.6 vectors; 30 from each.
const RRF_K: f32 = 60.0;
const W_FTS: f32 = 0.4;
const W_VEC: f32 = 0.6;
const CANDIDATES: usize = 30;
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
    Ok(c)
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
    b.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect()
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
            tx.execute("INSERT INTO chunks_fts(rowid, text) VALUES (?1, ?2)", params![id, format!("{}\n{}\n{}\n{}", x.breadcrumb, speaker.unwrap_or_default(), x.title, x.body)])?;
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

/// An FTS query from free text: every word of any script (two letters or more), OR-ed, quoted.
fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 2).map(|w| format!("\"{w}\"")).collect();
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

/// Hybrid search: BM25 over every script and the query's vector, fused by weighted reciprocal rank.
/// `source_contains` narrows to sources whose id contains it; without an embedder, keywords only.
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

fn total_chunks(c: &Connection) -> usize {
    c.query_row("SELECT count(*) FROM chunks", [], |r| r.get::<_, i64>(0)).unwrap_or(0) as usize
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
    let mut score: BTreeMap<i64, f32> = BTreeMap::new();
    if let Some(fq) = fts_query(q) {
        let mut st = c.prepare(
            "SELECT f.rowid FROM chunks_fts f JOIN chunks c ON c.id = f.rowid WHERE chunks_fts MATCH ?1 AND c.source LIKE ?2
             AND (?4 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = c.id AND s.kind = ?4 AND s.id = ?5)) ORDER BY bm25(chunks_fts) LIMIT ?3",
        )?;
        // Hidden sections (history) are dropped after the query: ask for as many more as could be hidden.
        let not_seen = hide.len() + only.map(|o| total_chunks(&c).saturating_sub(o.len())).unwrap_or(0);
        let want = (CANDIDATES + not_seen).min(CANDIDATES * 40) as i64;
        let ids: Vec<i64> = st.query_map(params![fq, like, want, scope.map(|s| s.0), scope.map(|s| s.1)], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for (rank, id) in ids.iter().filter(|i| ok(i)).enumerate() {
            *score.entry(*id).or_default() += W_FTS / (RRF_K + rank as f32 + 1.0);
        }
    }
    let mut note = None;
    if let Some(e) = emb {
        match e.embed(&[q.to_string()]) {
            Ok(v) => {
                let qv = &v[0];
                let mut near: Vec<(f32, i64)> = vectors(&c)?
                    .into_iter()
                    .filter(|x| source_contains.is_none_or(|s| x.1.contains(s)) && ok(&x.0))
                    .filter(|x| x.2.len() == qv.len())
                    .map(|x| (dot(qv, &x.2), x.0))
                    .collect();
                near.sort_by(|a, b| b.0.total_cmp(&a.0));
                for (rank, (_, id)) in near.into_iter().take(CANDIDATES).enumerate() {
                    *score.entry(id).or_default() += W_VEC / (RRF_K + rank as f32 + 1.0);
                }
            }
            Err(err) => note = Some(format!("keywords only: {err:#}")),
        }
    } else {
        note = Some("keywords only: no embedding server is set (Settings → Brain)".into());
    }
    // A closed thread's sections are its archive (#55): found, marked, ranked under what is current — unless the search
    // is within that thread.
    let closed = crate::projects::closed_threads();
    let ids: Vec<i64> = score.keys().copied().collect();
    let archived = archived_of(&c, &ids, &closed)?;
    let within_closed = scope.is_some_and(|s| s.0 == "thread" && closed.contains_key(s.1));
    for (id, s) in score.iter_mut() {
        if archived.contains_key(id) && !within_closed {
            *s *= ARCHIVE_WEIGHT;
        }
    }
    let mut ranked: Vec<(i64, f32)> = score.into_iter().collect();
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
}

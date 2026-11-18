//! A thread's folders in the Brain (Brain's `source_registry` + `parse_corpus` + `visual-parse`,
//! ported). The registry (`files` in the store) holds what was seen — path, hash, size, time, how it was
//! read, why it wasn't — never the file. Each sync walks the folders: a file is hashed only when its
//! size or time changed; the same bytes at a new path are a move (its document keeps its id); bytes
//! changed are read again; gone → removed with its sections. Of twins (one name, several formats) the
//! editable one is read. Every page is indexed from its text.

use crate::brain::store::{self, Input};
use crate::projects;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Bump when a reader changes what it makes of a file: every file is read again.
pub const READER_VERSION: i64 = 3;
const MAX_FILES: usize = 5000;
const MAX_BYTES: u64 = 200_000_000;

pub fn ensure_schema(c: &Connection) -> Result<()> {
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS files(id TEXT PRIMARY KEY, root TEXT NOT NULL, rel TEXT NOT NULL, sha256 TEXT, size INTEGER, mtime INTEGER,
            state TEXT NOT NULL DEFAULT 'active', status TEXT, reason TEXT, method TEXT, fidelity TEXT, event_date TEXT, date_from TEXT,
            pages INTEGER, flagged INTEGER, read INTEGER, reader_v INTEGER, indexed_sha TEXT, seen_at TEXT, UNIQUE(root, rel));
         CREATE INDEX IF NOT EXISTS files_root ON files(root, state);",
    )?;
    Ok(())
}

fn new_id(root: &str, rel: &str) -> String {
    let h = Sha256::digest(format!("{root}\0{rel}").as_bytes());
    h[..8].iter().map(|b| format!("{b:02x}")).collect()
}

fn sha_file(p: &Path) -> Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(p)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// A file found in a folder.
struct Seen {
    rel: String,
    abs: PathBuf,
    size: u64,
    mtime: i64,
    skip: Option<(String, String)>,
}

/// Every file under `root` (links not followed; hidden folders not entered), with its skip, if any;
/// and whether the walk stopped at `MAX_FILES` (then what wasn't seen isn't gone).
fn walk(root: &Path, excludes: &[String]) -> (Vec<Seen>, bool) {
    let mut out = vec![];
    let mut full = false;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let Ok(ft) = e.file_type() else { continue };
            let abs = e.path();
            let Ok(rel) = abs.strip_prefix(root) else { continue };
            let rel = rel.to_string_lossy().replace('\\', "/");
            if ft.is_dir() {
                if !e.file_name().to_string_lossy().starts_with('.') && super::skip_reason(&format!("{rel}/x"), excludes).is_none() {
                    stack.push(abs);
                }
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            if out.len() >= MAX_FILES {
                full = true;
                continue;
            }
            let md = e.metadata().ok();
            let size = md.as_ref().map(|m| m.len()).unwrap_or(0);
            let mtime = md.and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            let ext = super::ext(&abs);
            let skip = if let Some(why) = super::skip_reason(&rel, excludes) {
                Some(("skipped".to_string(), why))
            } else if !super::FORMATS.contains(&ext.as_str()) {
                Some(("unsupported".to_string(), if ext.is_empty() { "no extension".into() } else { format!(".{ext} isn't read") }))
            } else if size > MAX_BYTES {
                Some(("skipped".to_string(), format!("too big ({} MB)", size / 1_000_000)))
            } else if (ext == "html" || ext == "htm") && echo_page(&abs) {
                Some(("skipped".to_string(), "Echo's own minutes page".to_string()))
            } else {
                None
            };
            out.push(Seen { rel, abs, size, mtime, skip });
        }
    }
    (out, full)
}

/// A minutes page Echo wrote (its generator tag in the first bytes): never read back as evidence.
fn echo_page(p: &Path) -> bool {
    use std::io::Read;
    let mut buf = vec![0u8; 4096];
    let n = std::fs::File::open(p).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).contains(r#"<meta name="generator" content="Echo minutes">"#)
}

/// Which files are twins of another (same folder, same name, another format): rel → the one read.
fn twins(seen: &[Seen]) -> BTreeMap<String, String> {
    let mut groups: BTreeMap<String, Vec<&Seen>> = BTreeMap::new();
    for s in seen.iter().filter(|s| s.skip.is_none()) {
        let stem = s.rel.rsplit_once('.').map(|(a, _)| a).unwrap_or(&s.rel).to_lowercase();
        groups.entry(stem).or_default().push(s);
    }
    let rank = |s: &Seen| super::TWIN_ORDER.iter().position(|e| *e == super::ext(&s.abs)).unwrap_or(99);
    let mut out = BTreeMap::new();
    for (_, mut g) in groups {
        if g.len() < 2 {
            continue;
        }
        g.sort_by_key(|s| rank(s));
        for t in &g[1..] {
            out.insert(t.rel.clone(), g[0].rel.clone());
        }
    }
    out
}

#[derive(Debug, Default, serde::Serialize)]
pub struct Report {
    pub root: String,
    pub files: usize,
    pub read: usize,
    pub unchanged: usize,
    pub moved: usize,
    pub removed: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
    /// What the store did with this folder's documents.
    pub store: Option<store::Report>,
}

/// The folders open threads watch: root → (its excludes, those threads). A folder an open thread shares with a closed
/// one is current: read under the open threads only, so nothing read after a close joins the closed thread.
fn watched() -> BTreeMap<String, (Vec<String>, Vec<Value>)> {
    folders_of(projects::list().into_iter().filter(|p| !p.stage.closed()))
}

/// Every thread's folders, closed threads' too: a folder only closed threads have isn't read, and what was read stays
/// (their archive, #55).
fn with_folders() -> BTreeMap<String, (Vec<String>, Vec<Value>)> {
    folders_of(projects::list().into_iter())
}

fn folders_of(ps: impl Iterator<Item = projects::Project>) -> BTreeMap<String, (Vec<String>, Vec<Value>)> {
    let mut out: BTreeMap<String, (Vec<String>, Vec<Value>)> = BTreeMap::new();
    for p in ps {
        for s in p.sources.iter().filter(|s| s.kind == "folder") {
            let e = out.entry(s.id.clone()).or_default();
            for x in &s.exclude {
                if !e.0.contains(x) {
                    e.0.push(x.clone());
                }
            }
            e.1.push(json!({"id": p.id, "name": p.name, "client": p.client}));
        }
    }
    out
}

/// Sync every watched folder.
pub fn sync(emb: Option<&dyn crate::brain::embed::Embedder>) -> Result<Vec<Report>> {
    let path = crate::brain::store_path();
    let c = store::open(&path)?;
    ensure_schema(&c)?;
    let w = watched();
    let mut out = vec![];
    for (root, (excludes, threads)) in &w {
        let r = sync_root(&c, &path, Path::new(root), excludes, threads, emb);
        out.push(r.unwrap_or_else(|e| Report { root: root.clone(), errors: vec![format!("{e:#}")], ..Default::default() }));
    }
    // Folders no thread has any more: their files leave the Brain (the registry keeps them removed). A closed thread's
    // folder isn't read, and what was read stays: its archive (#55).
    let kept = with_folders();
    let roots: Vec<String> = c.prepare("SELECT DISTINCT root FROM files WHERE state = 'active'")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
    for root in roots.into_iter().filter(|r| !kept.contains_key(r)) {
        let ids: Vec<String> = c.prepare("SELECT id FROM files WHERE root = ?1 AND state = 'active'")?.query_map([&root], |r| r.get(0))?.collect::<Result<_, _>>()?;
        for id in &ids {
            store::remove(&path, &format!("file/{id}"))?;
        }
        c.execute("UPDATE files SET state = 'removed', status = 'removed', reason = 'no thread reads this folder now', indexed_sha = NULL WHERE root = ?1", [&root])?;
        out.push(Report { root: root.clone(), removed: ids.len(), ..Default::default() });
    }
    Ok(out)
}

struct Row {
    id: String,
    rel: String,
    sha: Option<String>,
    size: i64,
    mtime: i64,
    state: String,
    reader_v: Option<i64>,
    indexed_sha: Option<String>,
}

fn sync_root(c: &Connection, path: &Path, root: &Path, excludes: &[String], threads: &[Value], emb: Option<&dyn crate::brain::embed::Embedder>) -> Result<Report> {
    let root_s = root.to_string_lossy().to_string();
    let mut rep = Report { root: root_s.clone(), ..Default::default() };
    if !root.is_dir() {
        return Err(anyhow!("the folder isn't there (moved, or a drive not connected)"));
    }
    let (seen, full) = walk(root, excludes);
    rep.files = seen.len();
    if full {
        rep.errors.push(format!("more than {MAX_FILES} files: only the first are read (exclude folders to choose which)"));
    }
    let rows: Vec<Row> = c
        .prepare("SELECT id, rel, sha256, size, mtime, state, reader_v, indexed_sha FROM files WHERE root = ?1")?
        .query_map([&root_s], |r| {
            Ok(Row { id: r.get(0)?, rel: r.get(1)?, sha: r.get(2)?, size: r.get(3)?, mtime: r.get(4)?, state: r.get(5)?, reader_v: r.get(6)?, indexed_sha: r.get(7)? })
        })?
        .collect::<Result<_, _>>()?;
    let by_rel: BTreeMap<&str, &Row> = rows.iter().map(|r| (r.rel.as_str(), r)).collect();
    let now = chrono::Utc::now().to_rfc3339();
    // Hash only what changed size or time.
    let mut sha: BTreeMap<String, String> = BTreeMap::new();
    for s in seen.iter().filter(|s| s.skip.is_none()) {
        let known = by_rel.get(s.rel.as_str()).filter(|r| r.size == s.size as i64 && r.mtime == s.mtime).and_then(|r| r.sha.clone());
        match known.map(Ok).unwrap_or_else(|| sha_file(&s.abs)) {
            Ok(h) => {
                sha.insert(s.rel.clone(), h);
            }
            Err(e) => rep.errors.push(format!("{}: {e}", s.rel)),
        }
    }
    // Moves: an active row whose path is gone, and a new path with its bytes.
    let present: BTreeSet<&str> = seen.iter().map(|s| s.rel.as_str()).collect();
    let mut moved_to: BTreeMap<String, String> = BTreeMap::new(); // new rel → id
    // A walk cut short at the cap saw only part of the folder: what it didn't see may still be there.
    for r in rows.iter().filter(|r| r.state == "active" && !present.contains(r.rel.as_str()) && !full) {
        let target = seen.iter().find(|s| !by_rel.contains_key(s.rel.as_str()) && !moved_to.contains_key(&s.rel) && r.sha.is_some() && sha.get(&s.rel) == r.sha.as_ref());
        if let Some(t) = target {
            c.execute("UPDATE files SET rel = ?2, indexed_sha = NULL WHERE id = ?1", params![r.id, t.rel])?;
            moved_to.insert(t.rel.clone(), r.id.clone());
            rep.moved += 1;
        } else {
            store::remove(path, &format!("file/{}", r.id))?;
            c.execute("UPDATE files SET state = 'removed', status = 'removed', reason = 'gone from the folder', indexed_sha = NULL, seen_at = ?2 WHERE id = ?1", params![r.id, now])?;
            rep.removed += 1;
        }
    }
    let twin_of = twins(&seen);
    let mut inputs: Vec<Input> = vec![];
    for s in &seen {
        let id = moved_to.get(&s.rel).cloned().or_else(|| by_rel.get(s.rel.as_str()).map(|r| r.id.clone())).unwrap_or_else(|| new_id(&root_s, &s.rel));
        let row = by_rel.get(s.rel.as_str());
        let h = sha.get(&s.rel).cloned();
        let upsert = |status: &str, reason: Option<&str>| -> Result<()> {
            c.execute(
                "INSERT INTO files(id, root, rel, sha256, size, mtime, state, status, reason, seen_at) VALUES (?1,?2,?3,?4,?5,?6,'active',?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET rel=excluded.rel, sha256=excluded.sha256, size=excluded.size, mtime=excluded.mtime, state='active',
                   status=excluded.status, reason=excluded.reason, seen_at=excluded.seen_at",
                params![id, root_s, s.rel, h, s.size as i64, s.mtime, status, reason, now],
            )?;
            Ok(())
        };
        let not_read = s.skip.clone().or_else(|| twin_of.get(&s.rel).map(|t| ("twin".to_string(), format!("a twin of {t} (read instead)"))));
        if let Some((status, reason)) = not_read {
            // Read before (a file excluded since, a new twin — also one just moved here): its sections go.
            if row.is_some_and(|r| r.indexed_sha.is_some()) || moved_to.contains_key(&s.rel) {
                store::remove(path, &format!("file/{id}"))?;
                c.execute("UPDATE files SET indexed_sha = NULL WHERE id = ?1", [&id])?;
            }
            upsert(&status, Some(&reason))?;
            rep.skipped += 1;
            continue;
        }
        let Some(h) = h.clone() else { continue };
        let fresh = row.is_some_and(|r| r.indexed_sha.as_deref() == Some(h.as_str()) && r.reader_v == Some(READER_VERSION))
            && !moved_to.contains_key(&s.rel);
        if fresh {
            c.execute("UPDATE files SET seen_at = ?2, size = ?3, mtime = ?4 WHERE id = ?1", params![id, now, s.size as i64, s.mtime])?;
            rep.unchanged += 1;
            continue;
        }
        upsert("reading", None)?;
        match read_file(&id, &root_s, s, threads) {
            Ok((input, meta)) => {
                rep.read += 1;
                c.execute(
                    "UPDATE files SET status = 'indexed', reason = ?2, method = ?3, fidelity = ?4, event_date = ?5, date_from = ?6, pages = ?7, flagged = 0, read = 0,
                       reader_v = ?8, indexed_sha = ?9 WHERE id = ?1",
                    params![id, meta.reason, meta.method, meta.fidelity, meta.date, meta.date_from, meta.pages as i64, READER_VERSION, h],
                )?;
                match input {
                    Some(i) => inputs.push(i),
                    None => {
                        store::remove(path, &format!("file/{id}"))?;
                    }
                }
            }
            Err(e) => {
                let msg = format!("{e:#}");
                rep.errors.push(format!("{}: {msg}", s.rel));
                c.execute("UPDATE files SET status = 'error', reason = ?2, reader_v = ?3, indexed_sha = ?4 WHERE id = ?1", params![id, msg, READER_VERSION, h])?;
            }
        }
    }
    if !inputs.is_empty() {
        rep.store = Some(store::sync_kept(path, inputs, emb, &[], Some(&store::Keep::now()))?);
    }
    // Every document of this folder under the threads that read it now (a thread added or taken off).
    let want = json!(threads);
    let docs: Vec<(String, String)> = c
        .prepare("SELECT d.source, d.refs FROM documents d JOIN files f ON d.source = 'file/' || f.id WHERE f.root = ?1")?
        .query_map([&root_s], |r| Ok((r.get(0)?, r.get::<_, Option<String>>(1)?.unwrap_or_default())))?
        .collect::<Result<_, _>>()?;
    for (src, refs) in docs {
        let mut refs: Value = serde_json::from_str(&refs).unwrap_or(json!({}));
        if refs["threads"] != want {
            refs["threads"] = want.clone();
            refs["client"] = json!(threads.iter().find_map(|t| t["client"].as_str()));
            store::rescope(path, &src, &refs)?;
        }
    }
    Ok(rep)
}

struct Meta {
    method: String,
    fidelity: String,
    reason: Option<String>,
    date: Option<String>,
    date_from: String,
    pages: usize,
}

/// A page as indexed: its number, title, and Markdown (its text and tables).
struct Out {
    n: usize,
    title: String,
    md: String,
}

fn read_file(id: &str, root: &str, s: &Seen, threads: &[Value]) -> Result<(Option<Input>, Meta)> {
    let parsed = super::read(&s.abs)?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut meta = Meta { method: parsed.method.to_string(), fidelity: parsed.fidelity.to_string(), reason: parsed.skip.clone(), date: None, date_from: "none".into(), pages: 0 };
    if parsed.skip.is_some() {
        return Ok((None, meta));
    }
    if parsed.method == "spreadsheet" && parsed.fidelity == "partial" {
        meta.reason = Some("a reporting book: its schema and first rows (its figures are for the numbers lane)".into());
    }
    // Paged formats (PDF, decks): each page's text and tables; flowing ones (Word, sheets, text): their Markdown.
    let mut out: Vec<Out> = vec![];
    let mut body = String::new();
    if !parsed.pages.is_empty() {
        meta.pages = parsed.pages.len();
        for p in &parsed.pages {
            let mut m = p.text.clone();
            for t in &p.tables {
                m.push_str(&format!("\n\n{t}"));
            }
            if !m.trim().is_empty() {
                out.push(Out { n: p.n, title: p.title.clone().unwrap_or_default(), md: m });
            }
        }
    } else {
        body = parsed.md.clone().unwrap_or_default();
        if parsed.figures > 0 {
            meta.reason = Some(format!("{} figures not read (their text only)", parsed.figures));
        }
    }
    // The document: a heading per page (`## p03 · Title`), so each section knows its page.
    let name = s.rel.rsplit('/').next().unwrap_or(&s.rel).to_string();
    let mut md = format!("# {name}\n\n");
    let text_for_date = if out.is_empty() { body.clone() } else { out.iter().map(|o| o.md.clone()).collect::<Vec<_>>().join("\n") };
    let (date, from) = super::exact_date(&text_for_date, &s.abs, &today);
    meta.date = date.clone();
    meta.date_from = from.to_string();
    if out.is_empty() {
        md.push_str(&body);
    } else {
        for o in &out {
            md.push_str(&format!("## p{:02} · {}\n\n{}\n\n", o.n, o.title.trim(), o.md.trim()));
        }
    }
    if md.trim().len() <= name.len() + 2 {
        meta.reason.get_or_insert("no text in it".into());
        return Ok((None, meta));
    }
    let file_id = id.to_string();
    let input = Input {
        source: format!("file/{id}"),
        kind: "file".into(),
        title: name,
        event_date: date,
        md,
        refs: json!({"kind": "file", "file": id, "root": root, "rel": s.rel, "path": s.abs, "method": meta.method, "fidelity": meta.fidelity,
                     "threads": threads, "client": threads.iter().find_map(|t| t["client"].as_str())}),
        link: Box::new(move |ch| match page_of(&ch.title).or_else(|| page_of(&ch.parent_heading)) {
            Some(n) => format!("echo://file/{file_id}?page={n}"),
            None => format!("echo://file/{file_id}"),
        }),
        speaker: Box::new(|_| None),
    };
    Ok((Some(input), meta))
}

/// "p03 · Title" → 3.
pub fn page_of(title: &str) -> Option<usize> {
    let t = title.trim().strip_prefix('p')?;
    let n: String = t.chars().take_while(char::is_ascii_digit).collect();
    (!n.is_empty() && t[n.len()..].trim_start().starts_with('·')).then(|| n.parse().ok()).flatten()
}

/// The registry for the window: per watched folder, its files with what happened to each.
pub fn status() -> Result<Value> {
    let c = store::open(&crate::brain::store_path())?;
    ensure_schema(&c)?;
    // Counts only: a folder may hold thousands of files.
    let mut out = serde_json::Map::new();
    for root in watched().keys() {
        let mut by = serde_json::Map::new();
        let mut n = 0i64;
        let mut st = c.prepare("SELECT coalesce(status, '?'), count(*) FROM files WHERE root = ?1 AND state = 'active' GROUP BY 1")?;
        for row in st.query_map([root], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (s, k) = row?;
            n += k;
            by.insert(s, json!(k));
        }
        out.insert(root.clone(), json!({"n": n, "by_status": by}));
    }
    Ok(Value::Object(out))
}

/// Every folder's files with what happened to each (`echoctl brain files`).
pub fn list_all() -> Result<Value> {
    let c = store::open(&crate::brain::store_path())?;
    ensure_schema(&c)?;
    let mut out = serde_json::Map::new();
    for root in watched().keys() {
        let rows: Vec<Value> = c
            .prepare("SELECT id, rel, status, reason, method, fidelity, event_date, pages, flagged, read, size FROM files WHERE root = ?1 AND state = 'active' ORDER BY rel")?
            .query_map([root], |r| {
                Ok(json!({"id": r.get::<_, String>(0)?, "rel": r.get::<_, String>(1)?, "status": r.get::<_, Option<String>>(2)?, "reason": r.get::<_, Option<String>>(3)?,
                          "method": r.get::<_, Option<String>>(4)?, "fidelity": r.get::<_, Option<String>>(5)?, "date": r.get::<_, Option<String>>(6)?,
                          "pages": r.get::<_, Option<i64>>(7)?, "flagged": r.get::<_, Option<i64>>(8)?, "read": r.get::<_, Option<i64>>(9)?, "size": r.get::<_, Option<i64>>(10)?}))
            })?
            .collect::<Result<_, _>>()?;
        out.insert(root.clone(), json!(rows));
    }
    Ok(Value::Object(out))
}

/// A registered file's path (the window opens it; nothing else may be opened this way).
pub fn path_of(id: &str) -> Option<PathBuf> {
    let c = store::open(&crate::brain::store_path()).ok()?;
    // Only a document Echo reads (never a file it skipped or doesn't read — a script, an app).
    let (root, rel): (String, String) = c
        .query_row("SELECT root, rel FROM files WHERE id = ?1 AND state = 'active' AND coalesce(status, '') NOT IN ('skipped', 'unsupported')", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .ok()??;
    let p = Path::new(&root).join(rel);
    p.is_file().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_titles_and_twins() {
        assert_eq!(page_of("p03 · Roadmap"), Some(3));
        assert_eq!(page_of("p12 ·"), Some(12));
        assert_eq!(page_of("plan · x"), None);
        let s = |rel: &str| Seen { rel: rel.into(), abs: PathBuf::from(rel), size: 1, mtime: 0, skip: None };
        let t = twins(&[s("a/Program Overview.pdf"), s("a/Program Overview.pptx"), s("a/Other.pdf"), s("b/Program Overview.pdf")]);
        assert_eq!(t.get("a/Program Overview.pdf").map(String::as_str), Some("a/Program Overview.pptx"));
        assert_eq!(t.len(), 1);
    }
}

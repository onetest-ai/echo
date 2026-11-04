//! A thread's folders in the Brain (Brain's `source_registry` + `parse_corpus` + `visual-parse`,
//! ported). The registry (`files` in the store) holds what was seen — path, hash, size, time, how it was
//! read, why it wasn't — never the file. Each sync walks the folders: a file is hashed only when its
//! size or time changed; the same bytes at a new path are a move (its document keeps its id); bytes
//! changed are read again; gone → removed with its sections. Of twins (one name, several formats) the
//! editable one is read. Pages flagged visual are read by the page reader within a budget per run and
//! only while nothing records; the others wait (`pending`) and are read on later runs — meanwhile
//! those pages are indexed from their text.

use super::{pdf, render, route, vision, Page};
use crate::brain::store::{self, Input};
use crate::projects;
use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Bump when a reader changes what it makes of a file: every file is read again (page readings stay
/// cached by image).
pub const READER_VERSION: i64 = 2;
/// Pages the page reader reads in one run (the rest wait for the next).
pub const PAGES_PER_RUN: usize = 48;
const MAX_FILES: usize = 5000;
const MAX_BYTES: u64 = 200_000_000;
/// Page images for the reader: this wide at most.
const RENDER_WIDTH: i32 = 1400;

pub fn ensure_schema(c: &Connection) -> Result<()> {
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS files(id TEXT PRIMARY KEY, root TEXT NOT NULL, rel TEXT NOT NULL, sha256 TEXT, size INTEGER, mtime INTEGER,
            state TEXT NOT NULL DEFAULT 'active', status TEXT, reason TEXT, method TEXT, fidelity TEXT, event_date TEXT, date_from TEXT,
            pages INTEGER, flagged INTEGER, read INTEGER, reader_v INTEGER, indexed_sha TEXT, seen_at TEXT, UNIQUE(root, rel));
         CREATE INDEX IF NOT EXISTS files_root ON files(root, state);",
    )?;
    vision::ensure_schema(c)
}

pub fn assets_dir(id: &str) -> PathBuf {
    crate::brain::dir().join("assets").join(id)
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
    pub pages_read: usize,
    pub pages_pending: usize,
    pub reader: Option<String>,
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

/// Sync every watched folder. `read_pages`: the page reader may run (quiet, and asked to).
pub fn sync(emb: Option<&dyn crate::brain::embed::Embedder>, read_pages: bool) -> Result<Vec<Report>> {
    let path = crate::brain::store_path();
    let c = store::open(&path)?;
    ensure_schema(&c)?;
    let w = watched();
    let mut budget = if read_pages { PAGES_PER_RUN } else { 0 };
    let mut reader = if read_pages { vision::reader().map(Some) } else { Ok(None) };
    let mut out = vec![];
    for (root, (excludes, threads)) in &w {
        let r = sync_root(&c, &path, Path::new(root), excludes, threads, emb, &mut budget, &mut reader);
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
    status: Option<String>,
    reader_v: Option<i64>,
    indexed_sha: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn sync_root(
    c: &Connection,
    path: &Path,
    root: &Path,
    excludes: &[String],
    threads: &[Value],
    emb: Option<&dyn crate::brain::embed::Embedder>,
    budget: &mut usize,
    reader: &mut std::result::Result<Option<vision::Reader>, String>,
) -> Result<Report> {
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
        .prepare("SELECT id, rel, sha256, size, mtime, state, status, reader_v, indexed_sha FROM files WHERE root = ?1")?
        .query_map([&root_s], |r| {
            Ok(Row { id: r.get(0)?, rel: r.get(1)?, sha: r.get(2)?, size: r.get(3)?, mtime: r.get(4)?, state: r.get(5)?, status: r.get(6)?, reader_v: r.get(7)?, indexed_sha: r.get(8)? })
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
    // Strict routing: the page reader runs on the default profile (or a vision endpoint); a folder whose
    // threads all run on other profiles has its pages indexed from their text, never sent to it.
    let default = crate::ai::default_id();
    let thread_profiles = crate::brain::access::thread_profiles(&default);
    let profiles: BTreeSet<String> = threads.iter().filter_map(|t| t["id"].as_str()).map(|t| thread_profiles.get(t).cloned().unwrap_or_else(|| default.clone())).collect();
    let mut own_reader: std::result::Result<Option<vision::Reader>, String> = Err(format!("this folder's threads run on another AI profile ({}): its pages are indexed from their text", profiles.iter().cloned().collect::<Vec<_>>().join(", ")));
    let reader: &mut std::result::Result<Option<vision::Reader>, String> = if profiles.is_empty() || profiles.contains(&default) { reader } else { &mut own_reader };
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
        let fresh = row.is_some_and(|r| r.indexed_sha.as_deref() == Some(h.as_str()) && r.reader_v == Some(READER_VERSION) && r.status.as_deref() != Some("pending"))
            && !moved_to.contains_key(&s.rel);
        if fresh {
            c.execute("UPDATE files SET seen_at = ?2, size = ?3, mtime = ?4 WHERE id = ?1", params![id, now, s.size as i64, s.mtime])?;
            rep.unchanged += 1;
            continue;
        }
        upsert("reading", None)?;
        match read_file(c, &id, &root_s, s, &h, threads, budget, reader) {
            Ok((input, meta)) => {
                rep.read += 1;
                rep.pages_read += meta.read_now;
                rep.pages_pending += meta.flagged.saturating_sub(meta.read);
                // Pages wait for the reader only when it may still come (budget spent, a failure this
                // run); a reader that is off, unset or not for this folder's profile won't: indexed.
                let may_come = match &*reader {
                    Ok(_) => true,
                    Err(e) => e.starts_with("the page reader failed"),
                };
                let status = if meta.flagged > meta.read && may_come { "pending" } else { "indexed" };
                c.execute(
                    "UPDATE files SET status = ?2, reason = ?3, method = ?4, fidelity = ?5, event_date = ?6, date_from = ?7, pages = ?8, flagged = ?9, read = ?10,
                       reader_v = ?11, indexed_sha = ?12 WHERE id = ?1",
                    params![id, status, meta.reason, meta.method, meta.fidelity, meta.date, meta.date_from, meta.pages as i64, meta.flagged as i64, meta.read as i64, READER_VERSION, h],
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
    rep.reader = match reader {
        Ok(Some(r)) => Some(r.name()),
        Ok(None) => None,
        Err(e) => Some(format!("none: {e}")),
    };
    if !inputs.is_empty() {
        rep.store = Some(store::sync_kinds(path, inputs, emb, &[])?);
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
    flagged: usize,
    read: usize,
    read_now: usize,
}

/// A page as indexed: its number, title, and Markdown (the reading, or its text and tables).
struct Out {
    n: usize,
    title: String,
    md: String,
}

#[allow(clippy::too_many_arguments)]
fn read_file(
    c: &Connection,
    id: &str,
    root: &str,
    s: &Seen,
    sha: &str,
    threads: &[Value],
    budget: &mut usize,
    reader: &mut std::result::Result<Option<vision::Reader>, String>,
) -> Result<(Option<Input>, Meta)> {
    let parsed = super::read(&s.abs)?;
    let assets = assets_dir(id);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut meta = Meta { method: parsed.method.to_string(), fidelity: parsed.fidelity.to_string(), reason: parsed.skip.clone(), date: None, date_from: "none".into(), pages: 0, flagged: 0, read: 0, read_now: 0 };
    if parsed.skip.is_some() {
        return Ok((None, meta));
    }
    if parsed.method == "spreadsheet" && parsed.fidelity == "partial" {
        meta.reason = Some("a reporting book: its schema and first rows (its figures are for the numbers lane)".into());
    }
    let ext = super::ext(&s.abs);
    // The paged lane: PDF; decks always; Word files with figures. Their layout comes from a PDF.
    let mut layout: Option<(PathBuf, Vec<Page>)> = None;
    let mut xml_pages: Vec<Page> = parsed.pages.clone();
    if ext == "pdf" {
        layout = Some((s.abs.clone(), parsed.pages.clone()));
    } else if ext == "pptx" || (ext == "docx" && parsed.figures > 0) {
        std::fs::create_dir_all(&assets)?;
        let out = assets.join(format!("layout-{}.pdf", &sha[..12]));
        // Older layouts of this file go.
        if let Ok(rd) = std::fs::read_dir(&assets) {
            for e in rd.flatten() {
                let n = e.file_name().to_string_lossy().to_string();
                if n.starts_with("layout-") && e.path() != out {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        match render::to_pdf(&s.abs, &out).and_then(|p| pdf::read(&p).map(|r| (p, r.pages))) {
            Ok((p, pages)) if ext == "docx" || pages.len() == xml_pages.len() => layout = Some((p, pages)),
            Ok((_, pages)) => {
                meta.fidelity = "degraded".into();
                meta.reason = Some(format!("LibreOffice laid out {} pages for {} slides: slides indexed from their text", pages.len(), xml_pages.len()));
            }
            Err(e) => {
                meta.fidelity = "degraded".into();
                meta.reason = Some(format!("{e:#}: indexed from its text"));
            }
        }
    }
    let mut out: Vec<Out> = vec![];
    let mut body = String::new();
    if let Some((pdf_path, lay)) = &layout {
        // Slides keep their XML text and tables (exact); a PDF or a Word file its layout's text.
        if ext == "docx" {
            xml_pages = lay.clone();
        }
        if ext == "pdf" {
            xml_pages = lay.clone();
        }
        std::fs::create_dir_all(&assets)?;
        meta.pages = xml_pages.len();
        let mut want: Vec<(usize, String, Vec<u8>)> = vec![]; // (index, img sha, png) to read
        let mut readings: BTreeMap<usize, String> = BTreeMap::new();
        for (i, p) in xml_pages.iter().enumerate() {
            let measured = Page { text: lay.get(i).map(|l| l.text.clone()).unwrap_or_default(), tables: p.tables.clone(), drawings: lay.get(i).map(|l| l.drawings).unwrap_or(0), ..Default::default() };
            let _ = std::fs::write(assets.join(format!("p{:02}.txt", p.n)), &p.text);
            if !p.tables.is_empty() {
                let _ = std::fs::write(assets.join(format!("p{:02}.tables.md", p.n)), p.tables.join("\n\n"));
            }
            if route(&measured).is_none() {
                continue;
            }
            meta.flagged += 1;
            let png = match pdf::render(pdf_path, p.n, RENDER_WIDTH) {
                Ok(b) => b,
                Err(e) => {
                    meta.reason.get_or_insert(format!("page {}: {e:#}", p.n));
                    continue;
                }
            };
            let _ = std::fs::write(assets.join(format!("p{:02}.png", p.n)), &png);
            let img: String = Sha256::digest(&png).iter().map(|b| format!("{b:02x}")).collect();
            if let Some(md) = vision::cached(c, &img)? {
                readings.insert(i, md);
            } else {
                want.push((i, img, png));
            }
        }
        // Read what isn't cached, within the run's budget.
        let name = s.rel.rsplit('/').next().unwrap_or(&s.rel).to_string();
        for batch in want.chunks(vision::PER_REQUEST) {
            let r = match reader {
                Ok(Some(r)) if *budget >= batch.len() => r,
                Ok(Some(_)) | Ok(None) => break,
                Err(e) => {
                    meta.reason.get_or_insert(e.clone());
                    break;
                }
            };
            let items: Vec<vision::Item> = batch
                .iter()
                .map(|(i, _, png)| vision::Item { png: png.clone(), hint: format!("{name} p{}", xml_pages[*i].n), tables: xml_pages[*i].tables.join("\n\n") })
                .collect();
            match vision::read(r, &items) {
                Ok(mds) => {
                    *budget -= batch.len();
                    for ((i, img, _), md) in batch.iter().zip(mds) {
                        vision::save(c, img, &md, &r.name())?;
                        readings.insert(*i, md);
                        meta.read_now += 1;
                    }
                }
                Err(e) => {
                    // A reader that fails stops for this run (signed out, endpoint down); pages wait.
                    *reader = Err(format!("the page reader failed: {e:#}"));
                    break;
                }
            }
        }
        meta.read = readings.len();
        for (i, p) in xml_pages.iter().enumerate() {
            let (title, md) = match readings.get(&i) {
                Some(r) => {
                    let t = r.lines().find_map(|l| l.trim().strip_prefix("# ")).map(str::to_owned);
                    (t.or_else(|| p.title.clone()), vision::demote(r))
                }
                None => {
                    let mut m = p.text.clone();
                    for t in &p.tables {
                        m.push_str(&format!("\n\n{t}"));
                    }
                    (p.title.clone(), m)
                }
            };
            if md.trim().is_empty() {
                continue;
            }
            out.push(Out { n: p.n, title: title.unwrap_or_default(), md });
        }
    } else if !xml_pages.is_empty() {
        // A deck without a layout (no LibreOffice): its slides' text.
        meta.pages = xml_pages.len();
        for p in &xml_pages {
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
        if parsed.figures > 0 && layout.is_none() && ext == "docx" && meta.reason.is_none() {
            meta.reason = Some(format!("{} figures not read (no layout)", parsed.figures));
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
    // Counts only: a folder may hold thousands of files (its files a page at a time: `browse::folder_files`).
    let mut out = serde_json::Map::new();
    for root in watched().keys() {
        let mut by = serde_json::Map::new();
        let (mut n, mut pages) = (0i64, 0i64);
        let mut st = c.prepare("SELECT coalesce(status, '?'), count(*), coalesce(sum(CASE WHEN status = 'pending' THEN max(coalesce(flagged, 0) - coalesce(read, 0), 0) END), 0) FROM files WHERE root = ?1 AND state = 'active' GROUP BY 1")?;
        for row in st.query_map([root], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)))? {
            let (s, k, p) = row?;
            n += k;
            pages += p;
            by.insert(s, json!(k));
        }
        out.insert(root.clone(), json!({"n": n, "by_status": by, "pending_pages": pages}));
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

/// The factual content of a page (`get_evidence`): its text layer, its table grids, its image's path.
pub fn evidence(id: &str, page: usize) -> Value {
    let dir = assets_dir(id);
    let read = |f: String| std::fs::read_to_string(dir.join(f)).ok();
    let img = dir.join(format!("p{page:02}.png"));
    json!({"text_layer": read(format!("p{page:02}.txt")), "tables": read(format!("p{page:02}.tables.md")), "image": img.is_file().then(|| img.to_string_lossy().to_string()),
           "file": path_of(id).map(|p| p.to_string_lossy().to_string())})
}

/// Pages still waiting for the page reader (the scheduler keeps syncing while there are some).
pub fn pending() -> usize {
    store::open(&crate::brain::store_path())
        .ok()
        .and_then(|c| ensure_schema(&c).ok().and_then(|_| c.query_row("SELECT coalesce(sum(flagged - read), 0) FROM files WHERE state = 'active' AND status = 'pending'", [], |r| r.get::<_, i64>(0)).ok()))
        .unwrap_or(0) as usize
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

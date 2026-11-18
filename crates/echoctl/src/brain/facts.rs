//! Intake's facts: what the quarantine reader (`digest`) kept of a mail conversation or a channel post, one JSON file
//! per source under `<home>/recall/intake/facts/<source>.json` (`mail/<short>`, `channel/<short>/<post>`). A message
//! past the reading window can't be read again, so these files are the only copy: the index (`store`) is fed from
//! them and stays derived. Before a sync changes any of them the folder is copied to a dated generation under
//! `facts.bak/` (`backup`). A source read again is merged into its file, never put in its place (`merge`).
//! The facts the index held before these files are moved here once (`migrate`).

use super::{chunk, digest, store};
use crate::paths;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The file format; a change to it stays compatible (fields only added, with a default).
pub const VERSION: u32 = 1;

fn is_false(b: &bool) -> bool {
    !*b
}

/// One message's facts: its heading (`YYYY-MM-DD HH:MM <who>`), its author, its time (RFC 3339), its link in Outlook
/// or Teams, its sender as Who may speak names them (a mail's address, a Teams author's id), whether it was read
/// unauthenticated ("Read anyway"), and whether its facts are the index's text as it was (`raw`, from the move).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub head: String,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub at: String,
    #[serde(default)]
    pub link: String,
    #[serde(default)]
    pub facts: Vec<digest::Fact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unverified: bool,
    /// The message's Graph id: what says two sections are the same message (`merge`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub raw: bool,
}

/// A source's facts: what `share` and `links` serve and what the index is fed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    #[serde(default = "version")]
    pub version: u32,
    pub source: String,
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub event_date: Option<String>,
    /// threads, client, people, url, subjects, and a mail's `conversation` and `message` ids.
    #[serde(default)]
    pub refs: Value,
    #[serde(default)]
    pub sections: Vec<Section>,
    /// Every thread it is under is closed (its archive).
    #[serde(default)]
    pub archived: bool,
    /// Moved from the index as its text was, its facts not given back exactly (`migrate`).
    #[serde(default, skip_serializing_if = "is_false")]
    pub migrated_raw: bool,
}

fn version() -> u32 {
    VERSION
}

/// A section's author from its heading, as the index takes it.
fn speaker(title: &str) -> Option<String> {
    title.splitn(3, ' ').nth(2).map(str::to_owned)
}

/// A piece of a source as the index holds it: its section's title, text, author and link.
pub struct Piece {
    pub title: String,
    pub text: String,
    pub speaker: Option<String>,
    pub link: String,
}

impl Facts {
    pub fn new(source: String, kind: &str, title: String, event_date: Option<String>, refs: Value, sections: Vec<Section>) -> Facts {
        Facts { version: VERSION, source, kind: kind.into(), title, event_date, refs, sections, archived: false, migrated_raw: false }
    }

    /// The Markdown the index reads: the title, then a section per message with its facts (and their checked quotes);
    /// a raw section's text as it was.
    pub fn md(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);
        for s in self.sections.iter().filter(|s| !s.facts.is_empty()) {
            md.push_str(&format!("### {}\n\n", s.head));
            for f in &s.facts {
                match &f.quote {
                    _ if s.raw => md.push_str(&format!("{}\n", f.text)),
                    Some(q) => md.push_str(&format!("- {} — “{q}”\n", f.text)),
                    None => md.push_str(&format!("- {}\n", f.text)),
                }
            }
            md.push('\n');
        }
        md
    }

    /// Take out the sections of a sender the user ignored, or only those read unauthenticated when they kept that out —
    /// only sections that recorded that sender's id. A section from before senders were kept, by the same author's name
    /// only, stays (a name can be shared or forged). Whether any went, and how many by that name stayed.
    pub fn drop_sender(&mut self, key: &str, name: &str, only_unverified: bool) -> (bool, usize) {
        let n = self.sections.len();
        let by_name = self.sections.iter().filter(|s| s.sender.is_none() && !name.is_empty() && s.from.as_deref() == Some(name)).count();
        self.sections.retain(|s| !(s.sender.as_deref() == Some(key) && (!only_unverified || s.unverified)));
        (self.sections.len() != n, by_name)
    }

    fn links(&self) -> BTreeMap<String, String> {
        let mut links = BTreeMap::new();
        for s in self.sections.iter().filter(|s| !s.facts.is_empty()) {
            links.entry(s.head.clone()).or_insert_with(|| s.link.clone());
        }
        links
    }

    fn url(&self) -> String {
        self.refs["url"].as_str().unwrap_or("").to_string()
    }

    /// The pieces the index makes of it, in order.
    pub fn pieces(&self) -> Vec<Piece> {
        let (links, url) = (self.links(), self.url());
        chunk::chunk(&self.md(), chunk::MAX_CHARS)
            .into_iter()
            .map(|c| Piece { link: links.get(&c.title).cloned().unwrap_or_else(|| url.clone()), speaker: speaker(&c.title), title: c.title, text: c.body })
            .collect()
    }

    /// The index's document of it.
    pub fn input(&self) -> store::Input {
        let (links, url) = (self.links(), self.url());
        store::Input {
            source: self.source.clone(),
            kind: self.kind.clone(),
            title: self.title.clone(),
            event_date: self.event_date.clone(),
            md: self.md(),
            refs: self.refs.clone(),
            link: Box::new(move |c| links.get(&c.title).cloned().unwrap_or_else(|| url.clone())),
            speaker: Box::new(|c| speaker(&c.title)),
        }
    }
}

/// A source read again, merged into what was kept of it: every section kept, one read again (the same message: its
/// Graph id; a section from before ids were kept, its heading and link) replaced, new ones added, in time order — a
/// message the new read lacks (deleted, or past what Microsoft 365 keeps) never goes, and two sections of one read
/// never become one. Its title, refs and date are the new read's, its people and subjects both reads'.
pub fn merge(old: Facts, mut new: Facts) -> Facts {
    let same = |a: &Section, b: &Section| match (&a.id, &b.id) {
        (Some(x), Some(y)) => x == y,
        _ => a.head == b.head && a.link == b.link,
    };
    let mut sections = old.sections;
    let mut replaced = vec![false; sections.len()];
    for s in std::mem::take(&mut new.sections) {
        match (0..replaced.len()).find(|&i| !replaced[i] && same(&sections[i], &s)) {
            Some(i) => {
                sections[i] = s;
                replaced[i] = true;
            }
            None => sections.push(s),
        }
    }
    sections.sort_by(|a, b| a.head.get(..16).cmp(&b.head.get(..16)));
    new.sections = sections;
    for k in ["people", "subjects"] {
        let mut all: Vec<Value> = old.refs[k].as_array().cloned().unwrap_or_default();
        for v in new.refs[k].as_array().cloned().unwrap_or_default() {
            if !all.contains(&v) {
                all.push(v);
            }
        }
        if !all.is_empty() {
            new.refs[k] = json!(all);
        }
    }
    new.event_date = new.event_date.max(old.event_date);
    new.migrated_raw = old.migrated_raw && new.sections.iter().any(|s| s.raw);
    new
}

/// Whether every thread in these refs is closed (and there is one).
pub fn archived(refs: &Value, closed: &BTreeMap<String, Value>) -> bool {
    let ts: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    !ts.is_empty() && ts.iter().all(|t| closed.contains_key(*t))
}

/// Whether the facts the index held were moved into their files (`migrate`): nothing is read before.
pub fn migrated() -> bool {
    intake_dir().join("migrated.json").exists()
}

/// `<home>/recall/intake`.
pub fn intake_dir() -> PathBuf {
    super::dir().join("intake")
}

/// `<home>/recall/intake/facts`.
pub fn dir() -> PathBuf {
    intake_dir().join("facts")
}

/// Where the generations of the facts are kept (`backup`).
pub fn backup_dir() -> PathBuf {
    intake_dir().join("facts.bak")
}

/// A source's file: `facts/<source>.json`, only for a mail's or a post's id made of plain parts.
pub fn path_of(source: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = source.split('/').collect();
    if !matches!(parts.first(), Some(&"mail") | Some(&"channel")) || parts.len() < 2 {
        return None;
    }
    let plain = |p: &str| !p.is_empty() && !p.starts_with('.') && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.=+".contains(c));
    if !parts.iter().all(|p| plain(p)) {
        return None;
    }
    let mut p = dir();
    for x in &parts[..parts.len() - 1] {
        p.push(x);
    }
    p.push(format!("{}.json", parts[parts.len() - 1]));
    Some(p)
}

/// A source's facts.
pub fn read(source: &str) -> Option<Facts> {
    let b = std::fs::read(path_of(source)?).ok()?;
    serde_json::from_slice(&b).ok()
}

fn files(d: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(d) else { return };
    for e in rd.flatten() {
        let p = e.path();
        // `write_atomic`'s files on their way in start with a dot.
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// Every source's facts, by source.
pub fn all() -> Vec<Facts> {
    let mut paths = vec![];
    files(&dir(), &mut paths);
    let mut out: Vec<Facts> = paths.iter().filter_map(|p| std::fs::read(p).ok()).filter_map(|b| serde_json::from_slice(&b).ok()).collect();
    out.sort_by(|a, b| a.source.cmp(&b.source));
    out
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let (p, name) = (e.path(), e.file_name());
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if p.is_dir() {
            copy_dir(&p, &to.join(&name))?;
        } else {
            std::fs::copy(&p, to.join(&name))?;
        }
    }
    Ok(())
}

/// Generations of the facts kept: the newest days' (and always the fullest one).
const GENERATIONS: usize = 7;

fn count_files(d: &Path) -> usize {
    let mut v = vec![];
    files(d, &mut v);
    v.len()
}

/// The generations under `facts.bak/`, oldest first: `YYYY-MM-DD`, and `YYYY-MM-DD-shrunk` for the copy kept beside its
/// day's (it had less than half its files).
pub fn generations() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(backup_dir())
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.len() >= 10 && n.as_bytes()[0].is_ascii_digit()).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// Copy the facts to today's generation under `facts.bak/` (made whole beside it under a name of its own first). A copy
/// with less than half the files of today's earlier one never takes its place: it is the day's shrunk copy. The newest
/// `GENERATIONS` days are kept, and always the generation with the most files.
pub fn backup() -> Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let from = dir();
    if !from.is_dir() {
        return Ok(());
    }
    let root = backup_dir();
    std::fs::create_dir_all(&root)?;
    let tmp = root.join(format!(".copy.{}.{}.tmp", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&tmp);
    if let Err(e) = copy_dir(&from, &tmp) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(e.into());
    }
    let n = count_files(&tmp);
    let now = chrono::Local::now();
    let day = root.join(now.format("%Y-%m-%d").to_string());
    // Smaller than half of today's copy: kept beside it as the day's one shrunk copy (replacing an earlier one).
    let to = if day.exists() && n * 2 < count_files(&day) { root.join(format!("{}-shrunk", now.format("%Y-%m-%d"))) } else { day };
    if to.exists() {
        std::fs::remove_dir_all(&to)?;
    }
    std::fs::rename(&tmp, &to)?;
    // Prune: generations of days past the newest GENERATIONS go, but never the fullest one.
    let gens = generations();
    let mut days: Vec<&str> = gens.iter().map(|g| &g[..10]).collect();
    days.dedup();
    let keep_days: Vec<&str> = days.iter().rev().take(GENERATIONS).copied().collect();
    let fullest = gens.iter().max_by_key(|g| count_files(&root.join(g))).cloned();
    for g in &gens {
        if !keep_days.contains(&&g[..10]) && Some(g) != fullest.as_ref() {
            let _ = std::fs::remove_dir_all(root.join(g));
        }
    }
    Ok(())
}

/// A sync's changes to the facts: the folder is backed up once, before the first of them.
#[derive(Default)]
pub struct Writer {
    backed_up: bool,
}

impl Writer {
    fn before_change(&mut self) -> Result<()> {
        if !self.backed_up {
            backup()?;
            self.backed_up = true;
        }
        Ok(())
    }

    /// Write a source's facts (nothing when the file holds them already). A file there that can't be read is kept as
    /// `<name>.broken-<time>.json`, never written over.
    pub fn write(&mut self, f: &Facts) -> Result<()> {
        let Some(p) = path_of(&f.source) else { bail!("not a mail's or a post's id: {}", f.source) };
        let had = read(&f.source);
        if had.as_ref() == Some(f) {
            return Ok(());
        }
        self.before_change()?;
        if had.is_none() && p.exists() {
            let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();
            let to = p.with_file_name(format!("{stem}.broken-{}.json", chrono::Utc::now().format("%Y%m%dT%H%M%S%3f")));
            std::fs::rename(&p, &to)?;
            eprintln!("intake: {} couldn't be read: kept as {}", p.display(), to.display());
        }
        std::fs::create_dir_all(p.parent().expect("a folder"))?;
        paths::write_atomic(&p, serde_json::to_string_pretty(f)?.as_bytes())?;
        Ok(())
    }

    /// Remove a source's facts: only for a source that left its threads or the user took out (whether there was a file).
    pub fn remove(&mut self, source: &str) -> Result<bool> {
        let Some(p) = path_of(source).filter(|p| p.exists()) else { return Ok(false) };
        self.before_change()?;
        std::fs::remove_file(p)?;
        Ok(true)
    }
}

/// A section's facts back from the index's text (`- fact — “quote”` lines, as `Facts::md` writes them).
fn parse_facts(text: &str) -> Vec<digest::Fact> {
    let mut lines: Vec<String> = vec![];
    for l in text.lines() {
        match (l.strip_prefix("- "), lines.last_mut()) {
            (Some(r), _) => lines.push(r.to_string()),
            (None, Some(last)) => {
                last.push('\n');
                last.push_str(l);
            }
            (None, None) => lines.push(l.to_string()),
        }
    }
    lines
        .into_iter()
        .map(|s| match s.strip_suffix('”').and_then(|x| x.rsplit_once(" — “")) {
            Some((t, q)) => digest::Fact { text: t.to_string(), quote: Some(q.to_string()) },
            None => digest::Fact { text: s, quote: None },
        })
        .collect()
}

/// A chunk's title without the chunker's " (part n)".
fn base_title(t: &str) -> &str {
    match t.rsplit_once(" (part ") {
        Some((b, n)) if n.strip_suffix(')').is_some_and(|n| n.parse::<u32>().is_ok()) => b,
        _ => t,
    }
}

/// A document of the index: source, kind, title, date, refs.
type DocRow = (String, String, Option<String>, Option<String>, Option<String>);

/// Move the mail and channel facts the index (`knowledge.sqlite`) held into fact files, once, never while a sync runs
/// (it takes the sync's lock, `brain.lock`). `None`: done before, or a sync holds the lock (it moves them itself).
pub fn migrate() -> Result<Option<Value>> {
    if intake_dir().join("migrated.json").exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(paths::config_dir())?;
    let Some(_sync) = crate::app::FileLock::try_take(&paths::config_dir().join("brain.lock"))? else { return Ok(None) };
    migrate_locked()
}

/// `migrate`, its caller holding the sync's lock. A source that has a file keeps it. Each document is written from its
/// facts when they give the index's sections back text for text, else as its sections' text was (`migrated_raw`); one
/// that can't have a file (its id) is listed as skipped, with why; a failed write stops the move, to run again. It is
/// marked done (`intake/migrated.json`, with the counts)
/// once every document is accounted for, so the index is read once. The index is only read.
pub fn migrate_locked() -> Result<Option<Value>> {
    let marker = intake_dir().join("migrated.json");
    if marker.exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(intake_dir())?;
    let store = super::store_path();
    let (mut docs, mut written, mut had) = (0usize, 0usize, 0usize);
    let (mut raw, mut skipped): (Vec<String>, Vec<Value>) = (vec![], vec![]);
    if store.exists() {
        let c = rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        c.busy_timeout(std::time::Duration::from_secs(10))?;
        let rows: Vec<DocRow> = c
            .prepare("SELECT source, kind, title, event_date, refs FROM documents WHERE kind IN ('mail', 'channel') ORDER BY source")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<Result<_, _>>()?;
        let closed = crate::projects::closed_threads();
        let mut w = Writer::default();
        for (source, kind, title, date, refs) in rows {
            docs += 1;
            if path_of(&source).is_none() {
                skipped.push(json!({"source": source, "why": "not a name a file can have"}));
                continue;
            }
            if read(&source).is_some() {
                had += 1;
                continue;
            }
            let chunks: Vec<(String, String, Option<String>, Option<String>)> = c
                .prepare("SELECT coalesce(title, ''), coalesce(text, ''), speaker, link FROM chunks WHERE source = ?1 ORDER BY ord")?
                .query_map([&source], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<_, _>>()?;
            // A section's parts ("(part n)") back into one.
            let mut sections: Vec<(String, String, Option<String>, String)> = vec![];
            for (t, text, sp, link) in &chunks {
                let head = base_title(t).to_string();
                match sections.last_mut().filter(|s| s.0 == head && t != &head) {
                    Some(s) => {
                        s.1.push_str("\n\n");
                        s.1.push_str(text);
                    }
                    None => sections.push((head, text.clone(), sp.clone(), link.clone().unwrap_or_default())),
                }
            }
            let refs: Value = refs.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_else(|| json!({}));
            let make = |raw: bool| -> Vec<Section> {
                sections
                    .iter()
                    .map(|(head, text, sp, link)| Section {
                        at: rfc3339_of(head),
                        from: sp.clone(),
                        link: link.clone(),
                        facts: if raw { vec![digest::Fact { text: text.clone(), quote: None }] } else { parse_facts(text) },
                        head: head.clone(),
                        raw,
                        ..Default::default()
                    })
                    .collect()
            };
            let mut f = Facts::new(source.clone(), &kind, title.unwrap_or_default(), date, refs, make(false));
            f.archived = archived(&f.refs, &closed);
            let again = f.pieces();
            if again.len() != chunks.len() || again.iter().zip(&chunks).any(|(p, c)| p.text != c.1) {
                // Its facts wouldn't give the index's text back: kept as that text, flagged.
                f.sections = make(true);
                f.migrated_raw = true;
                raw.push(source.clone());
            }
            // A write that fails (the backup, the disk) isn't a document that can't move: the move stops and runs again.
            w.write(&f)?;
            written += 1;
        }
    }
    anyhow::ensure!(written + had + skipped.len() == docs, "Intake's facts: {written} written, {had} kept, {} skipped of {docs} documents", skipped.len());
    if !skipped.is_empty() {
        eprintln!("intake: {} of {docs} documents couldn't be moved into fact files (intake/migrated.json says why)", skipped.len());
    }
    let report = json!({"at": chrono::Utc::now().to_rfc3339(), "documents": docs, "written": written, "kept": had, "raw": raw, "skipped": skipped});
    paths::write_atomic(&marker, serde_json::to_string_pretty(&report)?.as_bytes())?;
    Ok(Some(report))
}

/// A section heading's time as RFC 3339 ("YYYY-MM-DD HH:MM …" is UTC, as the reader wrote it); empty without one.
fn rfc3339_of(head: &str) -> String {
    match (head.get(..10), head.get(11..16)) {
        (Some(d), Some(t)) if d.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(2) == Some(&b':') => format!("{d}T{t}:00Z"),
        _ => String::new(),
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn sec(head: &str, link: &str, fact: &str, sender: &str) -> Section {
        Section {
            head: head.into(),
            from: speaker(head),
            at: rfc3339_of(head),
            link: link.into(),
            facts: vec![digest::Fact { text: fact.into(), quote: None }],
            sender: Some(sender.into()),
            ..Default::default()
        }
    }

    fn sample(source: &str, threads: Value) -> Facts {
        let mut first = sec("2026-11-10 09:12 Jane Doe", "https://outlook.example/1", "The pilot moves to 24 Nov", "jane@acme.example");
        first.facts[0].quote = Some("moves to the 24th".into());
        Facts::new(
            source.into(),
            source.split('/').next().unwrap(),
            "Pilot dates".into(),
            Some("2026-11-11".into()),
            json!({"kind": "mail", "conversation": "c1", "message": "AAMk-1", "people": ["jane@acme.example"], "url": "https://outlook.example/1", "subjects": ["Pilot dates"], "threads": threads}),
            vec![first, sec("2026-11-11 08:00 Kim Ito", "https://outlook.example/2", "Kim confirms the room", "kim@acme.example")],
        )
    }

    #[test]
    fn a_sources_facts_round_trip_through_their_file() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-roundtrip");
        let f = sample("mail/0a1b2c3d4e5f6071", json!([{"id": "acme-pilot"}]));
        let mut w = Writer::default();
        w.write(&f).unwrap();
        assert_eq!(path_of(&f.source).unwrap(), dir().join("mail").join("0a1b2c3d4e5f6071.json"));
        assert_eq!(read(&f.source).as_ref(), Some(&f));
        assert_eq!(all(), vec![f.clone()]);
        // A post's id has the channel and the post: a folder per channel.
        let p = sample("channel/abcd/1700000000000", json!([]));
        w.write(&p).unwrap();
        assert!(dir().join("channel/abcd/1700000000000.json").exists());
        // What the index reads of it, section by section, with each message's link and author.
        let pieces = f.pieces();
        assert_eq!(pieces.len(), 2);
        assert_eq!((pieces[0].title.as_str(), pieces[0].speaker.as_deref(), pieces[0].link.as_str()), ("2026-11-10 09:12 Jane Doe", Some("Jane Doe"), "https://outlook.example/1"));
        assert_eq!(pieces[0].text, "- The pilot moves to 24 Nov — “moves to the 24th”");
        assert_eq!(pieces[1].link, "https://outlook.example/2");
        // An older file without the newer fields still reads.
        std::fs::write(path_of("mail/old").unwrap(), r#"{"source": "mail/old", "kind": "mail", "sections": [{"head": "2026-11-01 10:00 Ann"}]}"#).unwrap();
        let old = read("mail/old").unwrap();
        assert_eq!((old.version, old.sections[0].sender.clone(), old.sections[0].unverified), (VERSION, None, false));
        // Ids that aren't plain never become paths.
        for bad in ["mail/../x", "mail/.x", "call/x", "mail", "mail/a\\b", "mail/"] {
            assert!(path_of(bad).is_none(), "{bad}");
        }
    }

    fn gen_files(g: &str) -> usize {
        count_files(&backup_dir().join(g))
    }

    #[test]
    fn the_backup_keeps_dated_generations_and_a_shrunk_copy_never_replaces_a_fuller_one() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-backup");
        let mut w = Writer::default();
        for i in 0..4 {
            w.write(&sample(&format!("mail/m{i}"), json!([]))).unwrap();
        }
        assert!(generations().is_empty(), "nothing to back up before the first file");
        // The next sync: its first change copies the facts as they were, once, into today's generation.
        let mut w = Writer::default();
        let mut g = read("mail/m0").unwrap();
        g.title = "Pilot dates moved".into();
        w.write(&g).unwrap();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(generations(), vec![today.clone()]);
        assert_eq!(gen_files(&today), 4);
        let before: Facts = serde_json::from_slice(&std::fs::read(backup_dir().join(&today).join("mail/m0.json")).unwrap()).unwrap();
        assert_eq!(before.title, "Pilot dates");
        w.write(&sample("mail/m4", json!([]))).unwrap();
        assert_eq!(gen_files(&today), 4, "backed up once a sync, before its first change");
        // Nothing changed: no new copy.
        let mut w = Writer::default();
        w.write(&g).unwrap();
        assert_eq!(gen_files(&today), 4);
        // A later sync replaces today's copy with the folder as it is now (5 files).
        assert!(w.remove("mail/m4").unwrap());
        assert_eq!(gen_files(&today), 5);
        // The facts wiped (a bug, a bad list): the copy of 1 file is kept beside today's, never in its place.
        let mut w = Writer::default();
        for i in 0..3 {
            w.remove(&format!("mail/m{i}")).unwrap();
        }
        let mut w = Writer::default();
        w.write(&sample("mail/m9", json!([]))).unwrap();
        let gens = generations();
        assert_eq!(gens.len(), 2, "{gens:?}");
        assert_eq!(gen_files(&today), 4, "the fuller copy stays");
        assert!(gens[1] == format!("{today}-shrunk") && gen_files(&gens[1]) == 1);
        // Older days past seven go, but never the fullest generation.
        for d in 1..=9 {
            let day = (chrono::Local::now() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string();
            std::fs::create_dir_all(backup_dir().join(&day).join("mail")).unwrap();
            if d == 9 {
                for i in 0..20 {
                    std::fs::write(backup_dir().join(&day).join(format!("mail/x{i}.json")), "{}").unwrap();
                }
            }
        }
        let fullest = (chrono::Local::now() - chrono::Duration::days(9)).format("%Y-%m-%d").to_string();
        let mut w = Writer::default();
        w.write(&sample("mail/m10", json!([]))).unwrap();
        let gens = generations();
        let days: BTreeSet<&str> = gens.iter().map(|g| &g[..10]).collect();
        assert_eq!(days.len(), 8, "seven days and the fullest: {gens:?}");
        assert_eq!(gens.iter().filter(|g| g.ends_with("-shrunk")).count(), 1, "one shrunk copy a day, the latest");
        assert_eq!((gen_files(&today), gen_files(&format!("{today}-shrunk"))), (2, 1), "half of today's is no shrink: it replaces today's");
        assert!(gens.contains(&fullest));
        assert!(!std::fs::read_dir(backup_dir()).unwrap().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn a_source_read_again_keeps_every_section_the_new_read_lacks() {
        let old = sample("mail/m", json!([{"id": "acme-pilot"}]));
        // The new read lacks Jane's message (deleted, or past what Microsoft 365 keeps), says Kim's again in other
        // words, and brings a new one.
        let mut new = sample("mail/m", json!([{"id": "acme-pilot"}, {"id": "acme-ops"}]));
        new.title = "Pilot dates and rooms".into();
        new.event_date = Some("2026-11-12".into());
        new.refs["people"] = json!(["kim@acme.example", "lee@acme.example"]);
        new.sections = vec![
            sec("2026-11-11 08:00 Kim Ito", "https://outlook.example/2", "Kim confirms room 4", "kim@acme.example"),
            sec("2026-11-12 10:00 Lee Park", "https://outlook.example/3", "Lee books lunch", "lee@acme.example"),
        ];
        let m = merge(old, new);
        assert_eq!(m.sections.iter().map(|s| s.head.as_str()).collect::<Vec<_>>(), ["2026-11-10 09:12 Jane Doe", "2026-11-11 08:00 Kim Ito", "2026-11-12 10:00 Lee Park"], "kept, in time order");
        assert_eq!(m.sections[0].facts[0].text, "The pilot moves to 24 Nov", "the section the new read lacks stays");
        assert_eq!(m.sections[1].facts[0].text, "Kim confirms room 4", "the same message read again is replaced");
        assert_eq!((m.title.as_str(), m.event_date.as_deref()), ("Pilot dates and rooms", Some("2026-11-12")));
        assert_eq!(m.refs["people"], json!(["jane@acme.example", "kim@acme.example", "lee@acme.example"]));
        assert_eq!(m.refs["threads"].as_array().unwrap().len(), 2);
        // A date never goes back.
        let mut older = sample("mail/m", json!([]));
        older.event_date = Some("2026-01-01".into());
        assert_eq!(merge(m.clone(), older).event_date.as_deref(), Some("2026-11-12"));
    }

    #[test]
    fn ignoring_a_sender_takes_out_only_their_sections_and_keep_out_only_their_unauthenticated_ones() {
        let mut f = sample("mail/m", json!([]));
        let mut unver = sec("2026-11-12 07:00 Kim Ito", "https://outlook.example/4", "Kim forwards a digest", "kim@acme.example");
        unver.unverified = true;
        f.sections.push(unver);
        // From before senders were kept: by the author's name.
        let mut old = sec("2026-11-09 07:00 Kim Ito", "https://outlook.example/0", "Kim says hello", "x");
        old.sender = None;
        f.sections.insert(0, old);
        let mut keep_out = f.clone();
        assert_eq!(keep_out.drop_sender("kim@acme.example", "Kim Ito", true), (true, 1));
        assert_eq!(keep_out.sections.len(), 3, "only the unauthenticated one: {:?}", keep_out.sections);
        assert!(!keep_out.sections.iter().any(|s| s.unverified));
        assert_eq!(f.drop_sender("kim@acme.example", "Kim Ito", false), (true, 1), "the one only named is counted");
        // Only by the recorded id: the section only named "Kim Ito" stays (a name can be shared or forged), and Jane's.
        assert_eq!(f.sections.iter().map(|s| s.head.as_str()).collect::<Vec<_>>(), ["2026-11-09 07:00 Kim Ito", "2026-11-10 09:12 Jane Doe"]);
        assert_eq!(f.drop_sender("nobody@acme.example", "Nobody", false), (false, 0));
    }

    /// A made-up index with two mails (one of a long section split in parts), a post and a call.
    fn old_index() {
        let fact_lines = (0..40).map(|i| format!("- Fact number {i} about the Acme pilot, its dates and rooms")).collect::<Vec<_>>().join("\n");
        let docs = [
            ("mail/aaaa", "mail", "# Pilot dates\n\n### 2026-11-10 09:12 Jane Doe\n\n- The pilot moves to 24 Nov — “moves to the 24th”\n- Line one\ncontinued\n\n### 2026-11-11 08:00 Kim Ito\n\n- Kim confirms the room\n\n".to_string()),
            ("mail/bbbb", "mail", format!("# Long\n\n### 2026-11-09 10:00 Omar Haddad\n\n{fact_lines}\n\n{fact_lines}\n\n")),
            ("channel/cccc/17000", "channel", "# Standup\n\n### 2026-11-08 09:00 Dana Lee\n\n- Dana is out on Friday\n\n".to_string()),
        ];
        let mut inputs: Vec<store::Input> = docs
            .iter()
            .map(|(s, k, md)| store::Input {
                source: s.to_string(), kind: k.to_string(), title: md.lines().next().unwrap()[2..].to_string(), event_date: Some("2026-11-11".into()), md: md.clone(),
                refs: json!({"kind": k, "url": format!("https://outlook.example/{s}"), "threads": [{"id": "acme-pilot", "client": "acme"}]}),
                link: Box::new(|c| format!("https://outlook.example/{}", c.ord)), speaker: Box::new(|c| speaker(&c.title)),
            })
            .collect();
        inputs.push(store::Input { source: "call/x".into(), kind: "call".into(), title: "A call".into(), event_date: None, md: "# A call\n\n## Summary\n\nWords.\n".into(), refs: json!({}), link: Box::new(|_| String::new()), speaker: Box::new(|_| None) });
        store::sync_kinds(&super::super::store_path(), inputs, None, &[]).unwrap();
    }

    #[test]
    fn the_indexs_mail_and_posts_move_into_fact_files_once_with_their_counts_checked() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-migrate");
        // Nothing to move: done at once.
        assert_eq!(migrate().unwrap().unwrap()["documents"], 0);
        std::fs::remove_file(intake_dir().join("migrated.json")).unwrap();
        old_index();
        let store = super::super::store_path();
        let before = std::fs::read(&store).unwrap();
        let r = migrate().unwrap().unwrap();
        assert_eq!((r["documents"].as_u64(), r["written"].as_u64(), r["raw"].as_array().unwrap().len()), (Some(3), Some(3), 0));
        let all = all();
        assert_eq!(all.iter().map(|f| f.source.as_str()).collect::<Vec<_>>(), ["channel/cccc/17000", "mail/aaaa", "mail/bbbb"], "mail and posts only");
        // Each file gives the index's sections back, text for text.
        let c = store::open(&store).unwrap();
        for f in &all {
            let texts: Vec<String> = c.prepare("SELECT text FROM chunks WHERE source = ?1 ORDER BY ord").unwrap().query_map([&f.source], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect();
            assert_eq!(f.pieces().into_iter().map(|p| p.text).collect::<Vec<_>>(), texts, "{}", f.source);
        }
        let a = &all[1];
        assert_eq!(a.sections[0].facts[0], digest::Fact { text: "The pilot moves to 24 Nov".into(), quote: Some("moves to the 24th".into()) });
        assert_eq!(a.sections[0].facts[1].text, "Line one\ncontinued");
        assert_eq!((a.sections[0].from.as_deref(), a.sections[0].at.as_str(), a.sections[0].link.as_str()), (Some("Jane Doe"), "2026-11-10T09:12:00Z", "https://outlook.example/0"));
        assert_eq!(a.refs["threads"][0]["id"], "acme-pilot");
        assert_eq!(all[2].sections.len(), 1, "a section split in parts is one again");
        assert_eq!(std::fs::read(&store).unwrap(), before, "the index is only read");
        // Once: a second start does nothing, and a file changed since stays as it is.
        let mut w = Writer::default();
        let mut changed = all[1].clone();
        changed.title = "Changed since".into();
        w.write(&changed).unwrap();
        assert!(migrate().unwrap().is_none());
        std::fs::remove_file(intake_dir().join("migrated.json")).unwrap();
        let again = migrate().unwrap().unwrap();
        assert_eq!((again["documents"].as_u64(), again["written"].as_u64(), again["kept"].as_u64()), (Some(3), Some(0), Some(3)), "idempotent: what has a file keeps it");
        assert_eq!(read("mail/aaaa").unwrap().title, "Changed since");
    }

    #[test]
    fn a_document_whose_facts_cant_give_its_text_back_is_moved_as_its_text_and_the_move_completes() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-migrate-raw");
        old_index();
        let c = store::open(&super::super::store_path()).unwrap();
        // A section whose text isn't the reader's lines (written by hand), and a document no file can be named for.
        c.execute("UPDATE chunks SET text = 'A note\n\nwith a blank line' WHERE source = 'channel/cccc/17000'", []).unwrap();
        c.execute("INSERT INTO documents(source, kind, title) VALUES ('mail/../evil', 'mail', 'x')", []).unwrap();
        drop(c);
        let r = migrate().unwrap().unwrap();
        assert_eq!((r["documents"].as_u64(), r["written"].as_u64()), (Some(4), Some(3)));
        assert_eq!(r["raw"], json!(["channel/cccc/17000"]));
        assert_eq!(r["skipped"][0]["source"], "mail/../evil");
        let post = read("channel/cccc/17000").unwrap();
        assert!(post.migrated_raw && post.sections[0].raw);
        assert_eq!(post.pieces()[0].text, "A note\n\nwith a blank line", "its text as the index had it");
        assert!(intake_dir().join("migrated.json").exists(), "done: the index isn't scanned at every sync");
    }

    #[test]
    fn the_move_and_a_sync_never_run_together() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-migrate-lock");
        old_index();
        std::fs::create_dir_all(crate::paths::config_dir()).unwrap();
        // A sync holds its lock: the move waits for it (the sync moves them itself, `migrate_locked`).
        let sync = crate::app::FileLock::try_take(&crate::paths::config_dir().join("brain.lock")).unwrap().unwrap();
        assert!(migrate().unwrap().is_none());
        assert!(all().is_empty() && !intake_dir().join("migrated.json").exists());
        drop(sync);
        assert_eq!(migrate().unwrap().unwrap()["written"], 3);
    }

    #[test]
    fn a_message_is_the_same_by_its_id_and_two_of_one_read_never_become_one() {
        let base = |sections: Vec<Section>| Facts::new("channel/c/1".into(), "channel", "Standup".into(), Some("2026-11-11".into()), json!({"url": "https://teams.example/post"}), sections);
        let with_id = |id: &str, fact: &str| Section { id: Some(id.into()), ..sec("2026-11-11 09:00 Dana Lee", "https://teams.example/post", fact, "dana") };
        // Two replies of Dana's in the same minute, both falling back to the post's link: two messages by their ids.
        let old = base(vec![with_id("r1", "Dana is out Friday")]);
        let m = merge(old, base(vec![with_id("r1", "Dana is out Friday and Monday"), with_id("r2", "Dana asks Kim to cover")]));
        assert_eq!(m.sections.iter().map(|s| s.facts[0].text.as_str()).collect::<Vec<_>>(), ["Dana is out Friday and Monday", "Dana asks Kim to cover"]);
        // A section from before ids (moved from the index) is the same message by its heading and link; a second one of
        // the new read like it is added, never folded into it.
        let old = base(vec![sec("2026-11-11 09:00 Dana Lee", "https://teams.example/post", "Dana is out Friday", "dana")]);
        let m = merge(old, base(vec![with_id("r1", "Dana is out Friday"), with_id("r2", "Dana asks Kim to cover")]));
        assert_eq!(m.sections.len(), 2);
        assert_eq!(m.sections.iter().filter_map(|s| s.id.as_deref()).collect::<Vec<_>>(), ["r1", "r2"]);
    }

    #[test]
    fn a_file_that_cant_be_read_is_kept_aside_never_written_over() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-broken");
        let f = sample("mail/m", json!([]));
        let p = path_of(&f.source).unwrap();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "{ half a file").unwrap();
        Writer::default().write(&f).unwrap();
        assert_eq!(read("mail/m").as_ref(), Some(&f));
        let aside: Vec<String> = std::fs::read_dir(p.parent().unwrap()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("m.broken-")).collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(std::fs::read_to_string(p.parent().unwrap().join(&aside[0])).unwrap(), "{ half a file");
        assert_eq!(all(), vec![f], "the broken one isn't read as facts");
    }

    #[test]
    fn a_move_that_cant_write_isnt_marked_done_and_runs_again() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-migrate-io");
        old_index();
        // The facts folder can't be made (a file in its way): nothing is skipped, the move stops and isn't done.
        std::fs::create_dir_all(intake_dir()).unwrap();
        std::fs::write(dir(), "in the way").unwrap();
        assert!(migrate().is_err());
        assert!(!migrated());
        std::fs::remove_file(dir()).unwrap();
        assert_eq!(migrate().unwrap().unwrap()["written"], 3);
        assert!(migrated());
    }
}

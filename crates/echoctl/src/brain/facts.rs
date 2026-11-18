//! Intake's facts: what the quarantine reader (`digest`) kept of a mail conversation or a channel post, one JSON file
//! per source under `<home>/recall/intake/facts/<source>.json` (`mail/<short>`, `channel/<short>/<post>`). A message
//! past the reading window can't be read again, so these files are the only copy: the index (`store`) is fed from
//! them and stays derived. Before a sync changes any of them the folder is copied to `facts.bak/` (one rolling copy).
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

/// One message's facts: its heading (`YYYY-MM-DD HH:MM <who>`), its author, its time, its link in Outlook or Teams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
        Facts { version: VERSION, source, kind: kind.into(), title, event_date, refs, sections, archived: false }
    }

    /// The Markdown the index reads: the title, then a section per message with its facts (and their checked quotes).
    pub fn md(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);
        for s in self.sections.iter().filter(|s| !s.facts.is_empty()) {
            md.push_str(&format!("### {}\n\n", s.head));
            for f in &s.facts {
                match &f.quote {
                    Some(q) => md.push_str(&format!("- {} — “{q}”\n", f.text)),
                    None => md.push_str(&format!("- {}\n", f.text)),
                }
            }
            md.push('\n');
        }
        md
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

/// Whether every thread in these refs is closed (and there is one).
pub fn archived(refs: &Value, closed: &BTreeMap<String, Value>) -> bool {
    let ts: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    !ts.is_empty() && ts.iter().all(|t| closed.contains_key(*t))
}

/// `<home>/recall/intake`.
pub fn intake_dir() -> PathBuf {
    super::dir().join("intake")
}

/// `<home>/recall/intake/facts`.
pub fn dir() -> PathBuf {
    intake_dir().join("facts")
}

/// The rolling copy taken before a sync changes the facts.
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

/// Copy the facts to `facts.bak/`, replacing the copy before: made whole beside it first, then put in its place.
pub fn backup() -> Result<()> {
    let from = dir();
    if !from.is_dir() {
        return Ok(());
    }
    let tmp = intake_dir().join(".facts.bak.tmp");
    let _ = std::fs::remove_dir_all(&tmp);
    copy_dir(&from, &tmp)?;
    let to = backup_dir();
    if to.exists() {
        std::fs::remove_dir_all(&to)?;
    }
    std::fs::rename(&tmp, &to)?;
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

    /// Write a source's facts (nothing when the file holds them already).
    pub fn write(&mut self, f: &Facts) -> Result<()> {
        let Some(p) = path_of(&f.source) else { bail!("not a mail's or a post's id: {}", f.source) };
        if read(&f.source).as_ref() == Some(f) {
            return Ok(());
        }
        self.before_change()?;
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

/// Move the mail and channel facts the index (`knowledge.sqlite`) held into fact files, once: a source that has a file
/// keeps it; every document must then have its file, each written one giving the index's sections again, before it is
/// marked done (`intake/migrated.json`). The index is only read. `None`: done before, or another process is at it.
pub fn migrate() -> Result<Option<Value>> {
    let marker = intake_dir().join("migrated.json");
    if marker.exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(intake_dir())?;
    let Some(_one) = crate::app::FileLock::try_take(&intake_dir().join(".migrate.lock"))? else { return Ok(None) };
    if marker.exists() {
        return Ok(None);
    }
    let store = super::store_path();
    let (mut docs, mut written, mut errors) = (0usize, 0usize, vec![]);
    if store.exists() {
        let c = rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        c.busy_timeout(std::time::Duration::from_secs(10))?;
        let rows: Vec<DocRow> = c
            .prepare("SELECT source, kind, title, event_date, refs FROM documents WHERE kind IN ('mail', 'channel') ORDER BY source")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .collect::<Result<_, _>>()?;
        let closed = crate::projects::closed_threads();
        let mut w = Writer::default();
        let sources: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
        for (source, kind, title, date, refs) in rows {
            docs += 1;
            let chunks: Vec<(String, String, Option<String>, Option<String>)> = c
                .prepare("SELECT coalesce(title, ''), coalesce(text, ''), speaker, link FROM chunks WHERE source = ?1 ORDER BY ord")?
                .query_map([&source], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<_, _>>()?;
            if read(&source).is_some() {
                continue;
            }
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
            let mut f = Facts::new(
                source.clone(),
                &kind,
                title.unwrap_or_default(),
                date,
                refs,
                sections
                    .into_iter()
                    .map(|(head, text, sp, link)| Section { at: head.get(..16).filter(|a| a.as_bytes().get(4) == Some(&b'-')).unwrap_or("").to_string(), from: sp, link, facts: parse_facts(&text), head })
                    .collect(),
            );
            f.archived = archived(&f.refs, &closed);
            if path_of(&source).is_none() {
                errors.push(format!("{source}: not a name a file can have"));
                continue;
            }
            let again = f.pieces();
            // Facts that wouldn't give the index's sections back stay only in the index (untouched) until it is fixed.
            if again.len() != chunks.len() || again.iter().zip(&chunks).any(|(p, c)| p.text != c.1) {
                errors.push(format!("{source}: {} sections in the index, {} from its facts", chunks.len(), again.len()));
                continue;
            }
            w.write(&f)?;
            written += 1;
        }
        drop(c);
        let have = sources.iter().filter(|s| read(s).is_some()).count();
        if errors.is_empty() && have != docs {
            errors.push(format!("{have} fact files for {docs} documents"));
        }
    }
    if !errors.is_empty() {
        bail!("Intake's facts weren't all moved from the index ({} of {docs}): {}", errors.len(), errors.join("; "));
    }
    let report = json!({"at": chrono::Utc::now().to_rfc3339(), "documents": docs, "written": written});
    paths::write_atomic(&marker, serde_json::to_string_pretty(&report)?.as_bytes())?;
    Ok(Some(report))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(source: &str, threads: Value) -> Facts {
        Facts::new(
            source.into(),
            source.split('/').next().unwrap(),
            "Pilot dates".into(),
            Some("2026-11-11".into()),
            json!({"kind": "mail", "conversation": "c1", "message": "AAMk-1", "people": ["jane@acme.example"], "url": "https://outlook.example/1", "subjects": ["Pilot dates"], "threads": threads}),
            vec![
                Section { head: "2026-11-10 09:12 Jane Doe".into(), from: Some("Jane Doe".into()), at: "2026-11-10T09:12:00Z".into(), link: "https://outlook.example/1".into(),
                          facts: vec![digest::Fact { text: "The pilot moves to 24 Nov".into(), quote: Some("moves to the 24th".into()) }] },
                Section { head: "2026-11-11 08:00 Kim Ito".into(), from: Some("Kim Ito".into()), at: "2026-11-11T08:00:00Z".into(), link: "https://outlook.example/2".into(),
                          facts: vec![digest::Fact { text: "Kim confirms the room".into(), quote: None }] },
            ],
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
        std::fs::write(path_of("mail/old").unwrap(), r#"{"source": "mail/old", "kind": "mail"}"#).unwrap();
        assert_eq!(read("mail/old").unwrap().version, VERSION);
        // Ids that aren't plain never become paths.
        for bad in ["mail/../x", "mail/.x", "call/x", "mail", "mail/a\\b", "mail/"] {
            assert!(path_of(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn a_change_backs_the_facts_up_once_and_the_copy_rolls() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-backup");
        let f = sample("mail/one", json!([]));
        let mut w = Writer::default();
        w.write(&f).unwrap();
        assert!(!backup_dir().exists(), "nothing to back up before the first file");
        // The next sync: its first change copies the facts as they were, once.
        let mut w = Writer::default();
        let mut g = f.clone();
        g.title = "Pilot dates moved".into();
        w.write(&g).unwrap();
        let before: Facts = serde_json::from_slice(&std::fs::read(backup_dir().join("mail/one.json")).unwrap()).unwrap();
        assert_eq!(before.title, "Pilot dates");
        w.write(&sample("mail/two", json!([]))).unwrap();
        assert!(!backup_dir().join("mail/two.json").exists(), "backed up once a sync, before its first change");
        // Nothing changed: no new copy.
        let mut w = Writer::default();
        w.write(&g).unwrap();
        let still: Facts = serde_json::from_slice(&std::fs::read(backup_dir().join("mail/one.json")).unwrap()).unwrap();
        assert_eq!(still.title, "Pilot dates");
        // A removal backs up too: the copy rolls to what the folder held.
        assert!(w.remove("mail/two").unwrap());
        assert!(backup_dir().join("mail/two.json").exists() && read("mail/two").is_none());
        assert!(!w.remove("mail/two").unwrap());
        assert!(!intake_dir().join(".facts.bak.tmp").exists());
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
        assert_eq!((r["documents"].as_u64(), r["written"].as_u64()), (Some(3), Some(3)));
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
        assert_eq!((a.sections[0].from.as_deref(), a.sections[0].at.as_str(), a.sections[0].link.as_str()), (Some("Jane Doe"), "2026-11-10 09:12", "https://outlook.example/0"));
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
        assert_eq!((again["documents"].as_u64(), again["written"].as_u64()), (Some(3), Some(0)), "idempotent: what has a file keeps it");
        assert_eq!(read("mail/aaaa").unwrap().title, "Changed since");
    }

    #[test]
    fn a_move_that_cant_give_the_sections_back_isnt_marked_done() {
        let (_env, _root) = crate::paths::testenv::folders("intake-facts-migrate-bad");
        old_index();
        // A section's text the facts can't give back (rows changed by hand): the move says so and runs again next time.
        let c = store::open(&super::super::store_path()).unwrap();
        c.execute("INSERT INTO chunks(id, source, ord, title, text) VALUES (1, 'mail/aaaa', 9, '', '')", []).unwrap();
        drop(c);
        assert!(migrate().is_err());
        assert!(!intake_dir().join("migrated.json").exists());
        assert!(read("mail/aaaa").is_none() && read("mail/bbbb").is_some(), "only what gives its sections back gets a file");
    }
}

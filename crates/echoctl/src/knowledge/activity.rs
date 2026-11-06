//! The activity log: one line for every change Echo makes in a vault or the user approves — Home's "What's new"
//! (spec D27, D35). JSON lines in `<cache>/activity.jsonl`, appended under `<cache>/activity.lock`, the last
//! [`KEEP`] lines kept.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::Vault;

/// Lines kept.
pub const KEEP: usize = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// When (RFC 3339).
    pub at: String,
    /// The vault-relative path.
    pub path: String,
    /// `new` or `edit`.
    pub kind: String,
    /// Who: `minutes`, `ingest`, `ask`, `health`, `editor`, `new-page`, `index-rows`, `source` or `starter`.
    pub by: String,
    /// What it came from (a call, a source, a version), if anything.
    #[serde(default)]
    pub r#ref: Option<String>,
    pub plus: usize,
    pub minus: usize,
    /// The commit's short id, when one was made.
    #[serde(default)]
    pub commit: Option<String>,
}

fn dir(v: &Vault) -> PathBuf {
    super::cache_dir(&v.id)
}

fn file(v: &Vault) -> PathBuf {
    dir(v).join("activity.jsonl")
}

/// Append a line (lock held; older lines past [`KEEP`] dropped).
pub fn record(v: &Vault, e: Entry) -> Result<()> {
    let d = dir(v);
    std::fs::create_dir_all(&d)?;
    let t = std::time::Instant::now();
    let _held = loop {
        if let Some(l) = crate::app::FileLock::try_take(&d.join("activity.lock"))? {
            break l;
        }
        if t.elapsed() > std::time::Duration::from_secs(20) {
            anyhow::bail!("the activity log is busy");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let old = std::fs::read_to_string(file(v)).unwrap_or_default();
    let mut lines: Vec<&str> = old.lines().filter(|l| !l.trim().is_empty()).collect();
    let line = serde_json::to_string(&e)?;
    lines.push(&line);
    let from = lines.len().saturating_sub(KEEP);
    let mut text = lines[from..].join("\n");
    text.push('\n');
    Ok(crate::paths::write_atomic(&file(v), text.as_bytes())?)
}

#[allow(dead_code)] // read by Home's "What's new" (a later task)
/// Lines at or after `since` (RFC 3339), newest first. A line that doesn't read is skipped.
pub fn recent(v: &Vault, since: &str) -> Vec<Entry> {
    let since = chrono::DateTime::parse_from_rfc3339(since).ok();
    let text = std::fs::read_to_string(file(v)).unwrap_or_default();
    let mut out: Vec<(chrono::DateTime<chrono::FixedOffset>, Entry)> = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Entry>(l).ok())
        .filter_map(|e| chrono::DateTime::parse_from_rfc3339(&e.at).ok().map(|t| (t, e)))
        .filter(|(t, _)| since.is_none_or(|s| *t >= s))
        .collect();
    out.sort_by_key(|(t, _)| std::cmp::Reverse(*t));
    out.into_iter().map(|(_, e)| e).collect()
}

/// A commit label as the savers keep it (`<id>`, `off`, `pending`, `skipped: …`) → the id, if there is one.
pub fn commit_id(label: &str) -> Option<String> {
    (!label.is_empty() && label != "off" && label != "pending" && !label.starts_with("skipped")).then(|| label.to_string())
}

/// Record that `path` went from `old` to `new` (`None` = it didn't exist) — after the write succeeded. A failure to
/// record never fails the write the user asked for: it is logged and the caller goes on. A change that changed nothing
/// leaves no line.
pub fn changed(v: &Vault, path: &str, by: &str, r#ref: Option<&str>, old: Option<&str>, new: &str, commit: Option<String>) {
    let (plus, minus) = super::diff::counts(&super::diff::lines(old.unwrap_or(""), new));
    if old.is_some() && plus == 0 && minus == 0 {
        return;
    }
    let e = Entry { at: chrono::Local::now().to_rfc3339(), path: path.into(), kind: if old.is_some() { "edit" } else { "new" }.into(), by: by.into(), r#ref: r#ref.map(str::to_owned), plus, minus, commit };
    if let Err(err) = record(v, e) {
        eprintln!("knowledge: activity of {} in {}: {err:#}", path, v.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{fixture, AddOpts};

    fn vault(root: &std::path::Path) -> Vault {
        let t = fixture::twin(&root.join("twin"));
        crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
    }

    fn entry(n: usize) -> Entry {
        Entry { at: chrono::Local::now().to_rfc3339(), path: format!("p{n}.md"), kind: "edit".into(), by: "editor".into(), r#ref: None, plus: n, minus: 0, commit: None }
    }

    #[test]
    fn activity_appends_under_a_lock() {
        let (_env, root) = crate::paths::testenv::folders("kn-act-lock");
        let v = vault(&root);
        let threads: Vec<_> = (0..2).map(|t| {
            let v = v.clone();
            std::thread::spawn(move || (0..200).for_each(|n| record(&v, entry(t * 1000 + n)).unwrap()))
        }).collect();
        threads.into_iter().for_each(|t| t.join().unwrap());
        let text = std::fs::read_to_string(file(&v)).unwrap();
        assert_eq!(text.lines().count(), 400);
        assert!(text.lines().all(|l| serde_json::from_str::<Entry>(l).is_ok()), "every line is whole");
        assert_eq!(recent(&v, "2000-01-01T00:00:00+00:00").len(), 400);
        assert!(recent(&v, "2999-01-01T00:00:00+00:00").is_empty());
    }

    #[test]
    fn activity_keeps_the_last_2000() {
        let (_env, root) = crate::paths::testenv::folders("kn-act-keep");
        let v = vault(&root);
        std::fs::create_dir_all(dir(&v)).unwrap();
        let seed: String = (0..KEEP).map(|n| serde_json::to_string(&entry(n)).unwrap() + "\n").collect();
        std::fs::write(file(&v), seed).unwrap();
        record(&v, entry(9999)).unwrap();
        let text = std::fs::read_to_string(file(&v)).unwrap();
        assert_eq!(text.lines().count(), KEEP);
        let first: Entry = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(first.plus, 1, "the oldest line went");
        let last: Entry = serde_json::from_str(text.lines().last().unwrap()).unwrap();
        assert_eq!(last.plus, 9999);
        assert!(text.contains("\"ref\":null"), "`ref` is written under its plain name");
    }
}

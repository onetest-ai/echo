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
    /// The vault-relative path. The starter's line has an empty path: "the whole vault" (Home shows "The vault").
    pub path: String,
    /// `new` or `edit`; `commit` for a line that only names a commit Echo made of changes its lines already name
    /// ([`committed`]).
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
    /// The other files the same write wrote — an index row, a log line (vault-relative): Echo's too, never a change of
    /// their own (Home tells them from the user's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<String>,
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
    // Only a log that isn't there is empty; any other read error stops here, so the file is never rewritten from nothing.
    let old = match std::fs::read(file(v)) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e.into()),
    };
    // A line that isn't UTF-8 is dropped; it doesn't cost the others.
    let old: Vec<&str> = old.split(|b| *b == b'\n').filter_map(|l| std::str::from_utf8(l).ok()).collect();
    let mut lines: Vec<&str> = old.into_iter().filter(|l| !l.trim().is_empty()).collect();
    let line = serde_json::to_string(&e)?;
    lines.push(&line);
    let from = lines.len().saturating_sub(KEEP);
    let mut text = lines[from..].join("\n");
    text.push('\n');
    Ok(crate::paths::write_atomic(&file(v), text.as_bytes())?)
}

/// Lines at or after `since` (RFC 3339), newest first. A line that doesn't read is skipped.
pub fn recent(v: &Vault, since: &str) -> Vec<Entry> {
    let since = chrono::DateTime::parse_from_rfc3339(since).ok();
    let bytes = std::fs::read(file(v)).unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
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
    changed_with(v, path, by, r#ref, old, new, commit, &[]);
}

/// [`changed`], with the other files the same write wrote (absolute, under the vault's root): an index row, a log line.
#[allow(clippy::too_many_arguments)]
pub fn changed_with(v: &Vault, path: &str, by: &str, r#ref: Option<&str>, old: Option<&str>, new: &str, commit: Option<String>, also: &[std::path::PathBuf]) {
    let (plus, minus) = super::diff::counts(&super::diff::lines(old.unwrap_or(""), new));
    if old.is_some() && plus == 0 && minus == 0 {
        return;
    }
    let also = also.iter().filter_map(|p| p.strip_prefix(&v.root).ok()).map(|r| r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")).filter(|r| r != path).collect();
    let e = Entry { at: chrono::Local::now().to_rfc3339(), path: path.into(), kind: if old.is_some() { "edit" } else { "new" }.into(), by: by.into(), r#ref: r#ref.map(str::to_owned), plus, minus, commit, also };
    if let Err(err) = record(v, e) {
        eprintln!("knowledge: activity of {} in {}: {err:#}", path, v.id);
    }
}

/// Echo made commit `id` of changes its lines already name, after them (a Curate turn's end, *Save past minutes*): a
/// line naming the commit, so it is never taken for the user's (`kind: commit`, no change of its own).
pub fn committed(v: &Vault, by: &str, id: &str) {
    let e = Entry { at: chrono::Local::now().to_rfc3339(), path: String::new(), kind: "commit".into(), by: by.into(), r#ref: None, plus: 0, minus: 0, commit: Some(id.into()), also: vec![] };
    if let Err(err) = record(v, e) {
        eprintln!("knowledge: activity of commit {id} in {}: {err:#}", v.id);
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
        Entry { at: chrono::Local::now().to_rfc3339(), path: format!("p{n}.md"), kind: "edit".into(), by: "editor".into(), r#ref: None, plus: n, minus: 0, commit: None, also: vec![] }
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
    fn record_waits_for_the_lock() {
        let (_env, root) = crate::paths::testenv::folders("kn-act-wait");
        let v = vault(&root);
        std::fs::create_dir_all(dir(&v)).unwrap();
        let held = crate::app::FileLock::try_take(&dir(&v).join("activity.lock")).unwrap().expect("the lock is free");
        let w = { let v = v.clone(); std::thread::spawn(move || record(&v, entry(1)).unwrap()) };
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(!file(&v).exists(), "nothing is written while another holder has the lock");
        drop(held);
        w.join().unwrap();
        assert_eq!(std::fs::read_to_string(file(&v)).unwrap().lines().count(), 1);
    }

    #[test]
    fn a_bad_line_does_not_cost_the_good_ones() {
        let (_env, root) = crate::paths::testenv::folders("kn-act-bad");
        let v = vault(&root);
        std::fs::create_dir_all(dir(&v)).unwrap();
        let mut seed = serde_json::to_string(&entry(1)).unwrap().into_bytes();
        seed.extend(b"\n\xff\xfe not utf-8\n");
        seed.extend(serde_json::to_string(&entry(2)).unwrap().into_bytes());
        seed.push(b'\n');
        std::fs::write(file(&v), seed).unwrap();
        record(&v, entry(3)).unwrap();
        let text = std::fs::read_to_string(file(&v)).unwrap();
        let plus: Vec<usize> = text.lines().map(|l| serde_json::from_str::<Entry>(l).unwrap().plus).collect();
        assert_eq!(plus, [1, 2, 3], "the unreadable line is dropped, the others stay");
    }

    #[test]
    fn an_unreadable_log_is_never_overwritten() {
        let (_env, root) = crate::paths::testenv::folders("kn-act-unreadable");
        let v = vault(&root);
        // A directory where the log should be: reading it fails, and that is not "empty".
        std::fs::create_dir_all(file(&v)).unwrap();
        assert!(record(&v, entry(1)).is_err());
        assert!(file(&v).is_dir(), "left as it was");
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

//! What's new: after an update, the window shows what the versions since the last one seen bring, in plain words
//! (`whats-new.json`, compiled in; how to write an entry: `WHATS-NEW.md` beside it). The last version shown is kept in
//! the config folder (`whats-new-seen.json`), so each version's entry shows once; a fresh install starts seen.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::paths;

/// The file every release adds its entry to (its version-bump PR).
const SOURCE: &str = include_str!("../whats-new.json");
#[cfg(test)]
const FILE: &str = "crates/echoctl/whats-new.json";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The icons the window draws (`WN_ICONS` in the window script).
#[cfg(test)]
const ICONS: &[&str] = &["sparkle", "filter", "export", "layout", "link", "plugin", "shield", "thread", "calendar", "record", "fix"];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    version: String,
    items: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Item {
    icon: String,
    title: String,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    open: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Seen {
    version: String,
}

/// Every entry up to the running version, newest first as the file has them (a later one, written ahead, waits for its
/// release).
fn entries() -> Vec<Entry> {
    let mut all: Vec<Entry> = serde_json::from_str(SOURCE).unwrap_or_default();
    all.retain(|e| !crate::update::newer(&e.version, VERSION));
    all
}

/// The entries newer than `seen`, newest first.
fn since(all: &[Entry], seen: &str) -> Vec<Entry> {
    all.iter().filter(|e| crate::update::newer(&e.version, seen)).cloned().collect()
}

fn seen_file() -> std::path::PathBuf {
    paths::config_dir().join("whats-new-seen.json")
}

fn seen() -> Option<String> {
    serde_json::from_slice::<Seen>(&std::fs::read(seen_file()).ok()?).ok().map(|s| s.version)
}

/// The running version counts as seen: shown, or a fresh install's (`app::first_run`).
pub fn mark_seen() -> Result<()> {
    std::fs::create_dir_all(paths::config_dir())?;
    paths::write_atomic(&seen_file(), &serde_json::to_vec_pretty(&Seen { version: VERSION.into() })?)?;
    Ok(())
}

/// What the window shows now (`{version, entries}`), or null. Nothing seen yet: with calls, Echo was in use before What's
/// new existed, and the running version's entry shows; without any, there is nothing to compare with (a fresh install
/// starts seen, `app::first_run`).
pub fn pending() -> Value {
    let all = entries();
    let show = match seen() {
        Some(v) => since(&all, &v),
        None if crate::calls::all_calls().is_empty() => vec![],
        None => all.into_iter().take_while(|e| e.version == VERSION).collect(),
    };
    if show.is_empty() {
        return Value::Null;
    }
    json!({ "version": VERSION, "entries": show })
}

/// Every version's entry, newest first (the window's "What's new" from ⌘K and Settings).
pub fn all() -> Value {
    json!({ "version": VERSION, "entries": entries() })
}

/// What's wrong with the entries (`WHATS-NEW.md`'s rules), one line each.
#[cfg(test)]
fn problems(all: &[Entry]) -> Vec<String> {
    let mut out = vec![];
    for e in all {
        let at = |what: &str| format!("{FILE}: {}: {what}", e.version);
        if !(1..=6).contains(&e.items.len()) {
            out.push(at(&format!("{} items; 1 to 6", e.items.len())));
        }
        for i in &e.items {
            let t = &i.title;
            if !ICONS.contains(&i.icon.as_str()) {
                out.push(at(&format!("\"{t}\": icon \"{}\" isn't one of {ICONS:?}", i.icon)));
            }
            if t.trim().is_empty() || t.chars().count() > 40 || t.ends_with('.') {
                out.push(at(&format!("\"{t}\": a title is a few words, at most 40 characters, no full stop")));
            }
            if !one_sentence(&i.text) || i.text.chars().count() > 160 {
                out.push(at(&format!("\"{t}\": text is one sentence of at most 160 characters, ending with a full stop")));
            }
            if let Some(o) = &i.open {
                if !place(o) {
                    out.push(at(&format!("\"{t}\": open \"{o}\" is no place in Echo (a window route or an echo:// link)")));
                }
            }
        }
    }
    out
}

/// One sentence: it ends with a full stop and has no other sentence end (". ", "! ", "? ") inside.
#[cfg(test)]
fn one_sentence(s: &str) -> bool {
    let s = s.trim();
    s.ends_with('.') && ![". ", "! ", "? "].iter().any(|end| s[..s.len() - 1].contains(end))
}

/// A place "Show me" can open: a window route the window routes (`route` in the window script), or an echo:// link.
#[cfg(test)]
fn place(open: &str) -> bool {
    if open.starts_with("echo://") {
        return crate::links::Link::parse(open).is_some();
    }
    let mut parts = open.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some("today" | "calls" | "tasks" | "projects" | "found" | "calendar" | "settings"), None, None) => true,
        (Some("calendar"), Some("day" | "week"), None) => true,
        (Some("settings"), Some(s), None) => settings_sections().contains(&s.to_string()),
        _ => false,
    }
}

/// Settings' sections, as the window lists them (`SETTINGS_SECTIONS`).
#[cfg(test)]
fn settings_sections() -> Vec<String> {
    let js = include_str!("../ui/app/30-settings.js");
    let list = js.split("const SETTINGS_SECTIONS = [").nth(1).and_then(|r| r.split("\n]").next()).unwrap_or("");
    list.split("['").skip(1).filter_map(|p| p.split('\'').next()).map(str::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_written() -> Vec<Entry> {
        serde_json::from_str(SOURCE).unwrap_or_else(|e| panic!("{FILE} isn't a list of {{version, items: [{{icon, title, text, open?}}]}}: {e}"))
    }

    #[test]
    fn the_running_version_has_its_whats_new_entry() {
        assert!(
            all_written().iter().any(|e| e.version == VERSION),
            "{FILE} has no entry for Echo {VERSION}: a release adds its own, newest first: \
             {{\"version\": \"{VERSION}\", \"items\": [{{\"icon\": \"sparkle\", \"title\": \"A few words\", \"text\": \"One plain sentence.\", \"open\": \"today\"}}]}} \
             (how to write it: crates/echoctl/WHATS-NEW.md)"
        );
    }

    #[test]
    fn every_whats_new_entry_is_short_plain_and_opens_a_real_place() {
        let all = all_written();
        let p = problems(&all);
        assert!(p.is_empty(), "What's new entries break WHATS-NEW.md's rules:\n{}", p.join("\n"));
        let mut versions: Vec<&str> = all.iter().map(|e| e.version.as_str()).collect();
        versions.dedup();
        assert_eq!(versions.len(), all.len(), "{FILE}: one entry per version");
        // The checks themselves.
        let bad = |icon: &str, title: &str, text: &str, open: Option<&str>| {
            problems(&[Entry { version: "1.0.0".into(), items: vec![Item { icon: icon.into(), title: title.into(), text: text.into(), open: open.map(Into::into) }] }])
        };
        assert!(bad("filter", "Filters", "Set it once.", Some("settings/plugins")).is_empty());
        assert!(bad("filter", "Filters", "Open it.", Some("echo://thread/acme-rollout")).is_empty());
        assert_eq!(bad("rocket", "Filters", "Set it once.", None).len(), 1, "an unknown icon");
        assert_eq!(bad("filter", &"x".repeat(41), "Set it once.", None).len(), 1, "a long title");
        assert_eq!(bad("filter", "Filters", "Set it once. Then go.", None).len(), 1, "two sentences");
        assert_eq!(bad("filter", "Filters", "No full stop", None).len(), 1, "no sentence end");
        assert_eq!(bad("filter", "Filters", &format!("{}.", "x".repeat(160)), None).len(), 1, "a long text");
        assert_eq!(bad("filter", "Filters", "Set it once.", Some("settings/nope")).len(), 1, "no such section");
        assert_eq!(bad("filter", "Filters", "Set it once.", Some("https://example.com")).len(), 1, "not a place in Echo");
        assert!(bad("filter", "Filters", "Version 1.2 is out.", None).is_empty(), "a dot inside a word ends nothing");
        assert_eq!(problems(&[Entry { version: "1.0.0".into(), items: vec![] }]).len(), 1, "no items");
    }

    #[test]
    fn every_icon_is_drawn_by_the_window() {
        let js = include_str!("../ui/app/19-whats-new.js");
        for i in ICONS {
            assert!(js.contains(&format!("  {i}: ")), "the window's WN_ICONS draws no \"{i}\"");
        }
    }

    #[test]
    fn an_update_shows_each_version_since_the_last_seen_newest_first() {
        let e = |v: &str| Entry { version: v.into(), items: vec![] };
        let all = vec![e("0.10.0"), e("0.9.9"), e("0.9.8")];
        let v = |l: Vec<Entry>| l.into_iter().map(|e| e.version).collect::<Vec<_>>();
        assert_eq!(v(since(&all, "0.9.8")), ["0.10.0", "0.9.9"], "0.10 is newer than 0.9.9");
        assert_eq!(v(since(&all, "0.9.7")), ["0.10.0", "0.9.9", "0.9.8"]);
        assert!(since(&all, "0.10.0").is_empty());
        assert!(all_written().windows(2).all(|w| crate::update::newer(&w[0].version, &w[1].version)), "{FILE}: newest first");
        assert!(entries().iter().all(|e| !crate::update::newer(&e.version, VERSION)), "never a version after this one");
    }

    #[test]
    fn whats_new_shows_once_per_version_and_never_on_a_fresh_install() {
        let (_env, root) = crate::paths::testenv::folders("whats-new");
        // Nothing seen and no calls: nothing to compare with, and asking writes nothing (the window asks every few seconds).
        assert!(pending().is_null());
        assert_eq!(seen(), None);
        // A fresh install starts seen (`app::first_run`): nothing.
        mark_seen().unwrap();
        assert_eq!(seen().as_deref(), Some(VERSION));
        assert!(pending().is_null());
        // An update: the versions since the last seen; once shown (seen), nothing.
        let older = entries().last().map(|e| e.version.clone()).unwrap();
        std::fs::write(seen_file(), r#"{"version": "0.0.1"}"#).unwrap();
        let p = pending();
        assert_eq!(p["entries"].as_array().map(Vec::len), Some(entries().len()));
        assert_eq!(p["entries"][0]["version"], VERSION);
        assert_eq!(p["entries"].as_array().unwrap().last().unwrap()["version"], older.as_str());
        mark_seen().unwrap();
        assert!(pending().is_null(), "seen: never again");
        // Nothing seen, but calls: Echo was in use before What's new; only this version's entry.
        std::fs::remove_file(seen_file()).unwrap();
        let call = root.join("home/calls/2026-11-01_100000_acme-sync");
        std::fs::create_dir_all(&call).unwrap();
        std::fs::write(call.join("meta.json"), "{}").unwrap();
        let p = pending();
        assert_eq!(p["entries"].as_array().map(Vec::len), Some(1));
        assert_eq!(p["entries"][0]["version"], VERSION);
        assert_eq!(all()["entries"].as_array().map(Vec::len), Some(entries().len()), "⌘K and Settings: every version");
    }
}

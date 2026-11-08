//! Which terms help transcribe a call. Two lists in the data folder's config:
//!
//! - `vocabulary`: terms the user wants in every call (`echoctl vocab add --always Contoso`);
//! - `learned`: terms the notes run picked up from a call (`echoctl vocab add --call <dir> …`),
//!   each with the meeting subjects and people it came up with. A learned term is used again only
//!   in a call that shares one of those subjects, has mostly those people, or names it in its
//!   invite.
//!
//! With the Brain built in, learned terms live in its taxonomy (`brain::taxonomy::learn`: in the call's
//! client's layer, pinned to its threads, with the same scope); the old `learned` list is moved there
//! once (`migrate_learned`, a copy kept in `.echo/learned-before-taxonomy.json`) and read meanwhile.
//! It is transcription's, so it works whether or not the Brain is switched on.
//!
//! One list for every call is what went wrong first: terms learned in one project were boosted
//! into all the others, and Nemotron wrote "Flow Studio" and "ServiceNow" dozens of times into a
//! call that never mentioned them (2026-10-13).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::config::{valid_term, Config};
use crate::meeting::{MeetingContext, Person};
use crate::paths;

/// Learned terms kept (the least recently learned go first).
const LEARNED_MAX: usize = 100;

/// One learned term and where it came up.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Learned {
    pub term: String,
    /// Meeting subjects (normalised) of the calls it was learned from.
    #[serde(default)]
    pub subjects: Vec<String>,
    /// The other people on those calls (email, else name; lowercase).
    #[serde(default)]
    pub people: Vec<String>,
    /// The call folders, for `vocab list` (folders can be renamed; not used to match).
    #[serde(default)]
    pub calls: Vec<String>,
}

/// "Weekly sync #12" ~ "weekly sync": case, digits and punctuation don't make another series.
pub(crate) fn norm_subject(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphabetic() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn person_key(p: &Person) -> String {
    p.email.as_deref().filter(|e| !e.is_empty()).unwrap_or(&p.name).trim().to_lowercase()
}

fn is_me(p: &Person, me: Option<&Person>) -> bool {
    me.is_some_and(|m| match (&m.email, &p.email) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => !m.name.is_empty() && m.name.eq_ignore_ascii_case(&p.name),
    })
}

/// The other people of a meeting, as keys.
pub(crate) fn others(m: &MeetingContext, me: Option<&Person>) -> BTreeSet<String> {
    m.organizer.iter().chain(m.attendees.iter()).filter(|p| !is_me(p, me)).map(person_key).filter(|k| !k.is_empty()).collect()
}

/// Does a learned term belong in this call?
fn applies(l: &Learned, m: &MeetingContext, me: Option<&Person>) -> bool {
    let term = l.term.to_lowercase();
    if m.vocabulary.iter().any(|v| v.eq_ignore_ascii_case(&l.term)) || m.subject.as_deref().is_some_and(|s| s.to_lowercase().contains(&term)) {
        return true;
    }
    if m.subject.as_deref().map(norm_subject).is_some_and(|s| !s.is_empty() && l.subjects.contains(&s)) {
        return true;
    }
    let people = others(m, me);
    let shared = people.iter().filter(|p| l.people.contains(p)).count();
    shared > 0 && shared * 2 >= people.len()
}

/// Every learned term: the old list's, then the taxonomy's.
pub fn learned_all(cfg: &Config) -> Vec<Learned> {
    let out: Vec<Learned> = cfg.learned.clone().unwrap_or_default();
    #[cfg(feature = "brain")]
    let out = {
        let mut out = out;
        for t in crate::brain::taxonomy::learned() {
            if out.iter().any(|l| l.term.eq_ignore_ascii_case(&t.name)) {
                continue;
            }
            let s = t.learned.unwrap_or_default();
            out.push(Learned { term: t.name, subjects: s.subjects, people: s.people, calls: s.calls });
        }
        out
    };
    out
}

/// Terms for a call: the always-on list, then the learned terms that belong to this meeting.
/// Without a meeting (an ad-hoc call before its people are known) only the always-on list.
pub fn for_meeting(cfg: &Config, meeting: Option<&MeetingContext>) -> Vec<String> {
    let mut out: Vec<String> = cfg.vocabulary.clone();
    if let Some(m) = meeting {
        for l in learned_all(cfg).iter().filter(|l| applies(l, m, cfg.me.as_ref())) {
            if !out.iter().any(|t| t.eq_ignore_ascii_case(&l.term)) {
                out.push(l.term.clone());
            }
        }
    }
    out
}

/// Terms for a call: the always-on list, the glossaries of the projects the meeting belongs to, then
/// the learned terms that belong to it.
pub fn for_call(cfg: &Config, meeting: Option<&MeetingContext>) -> Vec<String> {
    let mut out: Vec<String> = cfg.vocabulary.clone();
    for t in meeting.map(crate::projects::terms_for_meeting).unwrap_or_default().into_iter().chain(for_meeting(cfg, meeting)) {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out
}

/// Learn terms from a call: each remembers the call's subject and people (re-learning adds
/// them and makes the term the newest). Returns the terms new to the list.
pub fn learn(cfg: &mut Config, dir: &Path, terms: &[String]) -> Vec<String> {
    let (subject, people, call) = scope_of(cfg, dir);
    #[cfg(feature = "brain")]
    {
        let (layer, threads) = place_of(dir);
        let items: Vec<crate::brain::taxonomy::Learn> = terms
            .iter()
            .map(|t| t.trim())
            .filter(|t| valid_term(t))
            .map(|t| crate::brain::taxonomy::Learn {
                term: t.to_string(),
                layer: layer.clone(),
                threads: threads.clone(),
                scope: crate::brain::taxonomy::LearnedScope { subjects: subject.iter().cloned().collect(), people: people.to_vec(), calls: if call.is_empty() { vec![] } else { vec![call.clone()] } },
            })
            .collect();
        match crate::brain::taxonomy::learn(&items, "minutes") {
            Ok(added) => return added,
            Err(e) => eprintln!("warning: the taxonomy couldn't learn them ({e:#}); kept in the old list"),
        }
    }
    learn_into_list(cfg, terms, subject, &people, &call)
}

/// A call's subject (normalised), its other people and its folder name.
fn scope_of(cfg: &Config, dir: &Path) -> (Option<String>, Vec<String>, String) {
    let meeting = MeetingContext::load(dir).ok().flatten();
    let subject = meeting
        .as_ref()
        .and_then(|m| m.subject.clone())
        .or_else(|| fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()).and_then(|v| v["title"].as_str().map(str::to_owned)))
        .map(|s| norm_subject(&s))
        .filter(|s| !s.is_empty());
    let people = meeting.as_ref().map(|m| others(m, cfg.me.as_ref())).unwrap_or_default();
    let call = dir.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    (subject, people.into_iter().collect(), call)
}

/// The old `learned` list in the config. Reads nothing else: `migrate` runs inside `Config::load`, and
/// the taxonomy's way (`place_of` → the projects → `Config::load`) recursed there until the stack ran out.
fn learn_into_list(cfg: &mut Config, terms: &[String], subject: Option<String>, people: &[String], call: &str) -> Vec<String> {
    let list = cfg.learned.get_or_insert_with(Vec::new);
    let mut added = vec![];
    for t in terms.iter().map(|t| t.trim()) {
        if !valid_term(t) {
            continue;
        }
        let mut entry = match list.iter().position(|l| l.term.eq_ignore_ascii_case(t)) {
            Some(i) => list.remove(i), // keeps its spelling
            None => {
                added.push(t.to_string());
                Learned { term: t.to_string(), ..Default::default() }
            }
        };
        if let Some(s) = &subject {
            if !entry.subjects.contains(s) {
                entry.subjects.push(s.clone());
            }
        }
        for p in people {
            if !entry.people.contains(p) {
                entry.people.push(p.clone());
            }
        }
        if !call.is_empty() && !entry.calls.iter().any(|c| c == call) {
            entry.calls.push(call.to_string());
        }
        list.push(entry);
    }
    let excess = list.len().saturating_sub(LEARNED_MAX);
    list.drain(..excess);
    added
}

/// Where a call's learned terms go: its client's layer (the first of its threads with a client),
/// else the shared one; pinned to its threads.
#[cfg(feature = "brain")]
fn place_of(dir: &Path) -> (String, Vec<String>) {
    let ps = crate::projects::of_call(dir).unwrap_or_default();
    let layer = ps.iter().find_map(|(p, _)| p.client.clone()).unwrap_or_else(|| crate::brain::taxonomy::SHARED.to_string());
    (layer, ps.into_iter().map(|(p, _)| p.id).collect())
}

/// Move the old `learned` list into the taxonomy, once: each term in the layer of the calls it came
/// from (their client), pinned to their threads, its scope kept; the list saved beside the config,
/// then emptied. Returns how many terms moved.
#[cfg(feature = "brain")]
pub fn migrate_learned(cfg: &mut Config) -> Result<usize> {
    let Some(list) = cfg.learned.clone().filter(|l| !l.is_empty()) else { return Ok(0) };
    let items: Vec<crate::brain::taxonomy::Learn> = list
        .iter()
        .map(|l| {
            let dirs: Vec<std::path::PathBuf> = l.calls.iter().filter_map(|c| crate::calls::resolve_call(c).ok()).collect();
            let places: Vec<(String, Vec<String>)> = dirs.iter().map(|d| place_of(d)).collect();
            let layer = places.iter().map(|p| p.0.clone()).find(|l| l != crate::brain::taxonomy::SHARED).unwrap_or_else(|| crate::brain::taxonomy::SHARED.to_string());
            let mut threads: Vec<String> = places.into_iter().flat_map(|p| p.1).collect();
            threads.sort();
            threads.dedup();
            crate::brain::taxonomy::Learn {
                term: l.term.clone(),
                layer,
                threads,
                scope: crate::brain::taxonomy::LearnedScope { subjects: l.subjects.clone(), people: l.people.clone(), calls: l.calls.clone() },
            }
        })
        .collect();
    paths::write_atomic(&paths::state_dir().join("learned-before-taxonomy.json"), serde_json::to_string_pretty(&list)?.as_bytes())?;
    crate::brain::taxonomy::learn(&items, "migration")?;
    // `Some(empty)`, never None: an empty `learned` beside a non-empty always-on list is what the older
    // migration (`migrate`) takes for a config from before learned terms, and would scope the list away.
    // The config is read again right before saving, so a change made meanwhile isn't overwritten.
    let mut fresh = Config::load()?;
    fresh.learned = Some(vec![]);
    fresh.save()?;
    cfg.learned = Some(vec![]);
    Ok(items.len())
}

/// The call a notes run just wrote (`vocab add` without `--call`): the newest notes.md, if it
/// was written in the last three hours.
fn latest_noted_call() -> Option<std::path::PathBuf> {
    let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    for e in fs::read_dir(paths::calls_dir()).ok()?.flatten() {
        let Ok(t) = e.path().join("notes.md").metadata().and_then(|m| m.modified()) else { continue };
        if best.as_ref().is_none_or(|b| t > b.0) {
            best = Some((t, e.path()));
        }
    }
    let (t, dir) = best?;
    (t.elapsed().unwrap_or_default() < std::time::Duration::from_secs(3 * 3600)).then_some(dir)
}

/// Before learned terms had a scope, every term was in `vocabulary` and used in every call.
/// Once: each such term is scoped to the calls whose notes mention it (notes, not transcripts:
/// a transcript can hold the very insertions this undoes); the always-on list starts empty.
pub fn migrate(cfg: &mut Config) -> bool {
    if cfg.learned.is_some() || cfg.vocabulary.is_empty() {
        return false;
    }
    // Learned terms already moved to the taxonomy (a config saved with `learned: None` by an earlier
    // build): this config is not from before learned terms, and its always-on list stays as it is.
    #[cfg(feature = "brain")]
    if !crate::brain::taxonomy::learned().is_empty() {
        cfg.learned = Some(vec![]);
        return false;
    }
    let old = std::mem::take(&mut cfg.vocabulary);
    cfg.learned = Some(vec![]);
    let calls: Vec<std::path::PathBuf> = fs::read_dir(paths::calls_dir()).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    let notes: Vec<(std::path::PathBuf, String)> = calls.into_iter().filter_map(|d| Some((d.clone(), fs::read_to_string(d.join("notes.md")).ok()?.to_lowercase()))).collect();
    for term in old {
        let t = term.to_lowercase();
        let found: Vec<&std::path::PathBuf> = notes.iter().filter(|(_, n)| n.contains(&t)).map(|(d, _)| d).collect();
        if found.is_empty() {
            cfg.learned.get_or_insert_with(Vec::new).push(Learned { term: term.clone(), ..Default::default() });
        }
        // Into the old list only (`migrate_learned` moves it into the taxonomy later, outside a load).
        for d in found {
            let (subject, people, call) = scope_of(cfg, d);
            learn_into_list(cfg, std::slice::from_ref(&term), subject, &people, &call);
        }
    }
    true
}

/// `echoctl vocab …`.
pub fn add(cfg: &mut Config, terms: &[String], call: Option<&str>, always: bool) -> Result<String> {
    if always {
        let mut added = vec![];
        for t in terms.iter().map(|t| t.trim()).filter(|t| valid_term(t)) {
            if !cfg.vocabulary.iter().any(|v| v.eq_ignore_ascii_case(t)) {
                cfg.vocabulary.push(t.to_string());
                added.push(t.to_string());
            }
        }
        return Ok(format!("always: {}", if added.is_empty() { "nothing new".into() } else { added.join(", ") }));
    }
    let dir = match call {
        Some(c) => crate::calls::resolve_call(c)?,
        // A minutes run learns for its own call, never the newest one.
        None if crate::notes::headless() => crate::calls::resolve_call(&std::env::var(crate::guard::RUN_CALL).unwrap_or_default())?,
        None => match latest_noted_call() {
            Some(d) => d,
            None => bail!("which call are these terms from? Pass --call <dir> (or --always for every call)"),
        },
    };
    let added = learn(cfg, &dir, terms);
    // A project's call: its glossary learns them too — unless the taxonomy did (it pins them to the
    // call's threads, and it superseded the glossaries).
    #[cfg(feature = "brain")]
    let glossary = false;
    #[cfg(not(feature = "brain"))]
    let glossary = true;
    match crate::projects::of_call(&dir).map(|p| if glossary { p } else { vec![] }) {
        Ok(projects) => {
            for (p, _) in projects {
                let r = crate::projects::update(&p.id, |p| {
                    for t in terms.iter().map(|t| t.trim()).filter(|t| valid_term(t)) {
                        if !p.terms.iter().any(|x| x.term.eq_ignore_ascii_case(t)) {
                            p.terms.push(crate::projects::Term { term: t.to_string(), kind: None, note: Some("learned from a call".into()) });
                        }
                    }
                    Ok(())
                });
                if let Err(e) = r {
                    eprintln!("warning: project {}: {e:#}", p.id);
                }
            }
        }
        Err(e) => eprintln!("warning: projects: {e:#}"),
    }
    Ok(format!(
        "learned from {}: {}",
        dir.file_name().unwrap_or_default().to_string_lossy(),
        if added.is_empty() { "nothing new".into() } else { added.join(", ") }
    ))
}

/// Forget a learned term only (the always-on list keeps it if it is there).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn forget_learned(cfg: &mut Config, term: &str) {
    if let Some(l) = cfg.learned.as_mut() {
        l.retain(|x| !x.term.eq_ignore_ascii_case(term));
    }
    #[cfg(feature = "brain")]
    if let Err(e) = crate::brain::taxonomy::forget_learned(term) {
        eprintln!("warning: the taxonomy: {e:#}");
    }
}

pub fn remove(cfg: &mut Config, terms: &[String]) {
    let gone = |t: &str| terms.iter().any(|x| x.trim().eq_ignore_ascii_case(t));
    cfg.vocabulary.retain(|v| !gone(v));
    if let Some(l) = cfg.learned.as_mut() {
        l.retain(|x| !gone(&x.term));
    }
    #[cfg(feature = "brain")]
    for t in terms {
        if let Err(e) = crate::brain::taxonomy::forget_learned(t.trim()) {
            eprintln!("warning: the taxonomy: {e:#}");
        }
    }
}

pub fn list(cfg: &Config) -> String {
    let mut out = format!("always ({}): {}\n", cfg.vocabulary.len(), cfg.vocabulary.join(", "));
    let learned = learned_all(cfg);
    out.push_str(&format!("learned ({}):\n", learned.len()));
    for l in &learned {
        let scope = if l.subjects.is_empty() && l.people.is_empty() { "only when an invite names it".to_string() } else { format!("{} · {}", l.subjects.join(" | "), l.people.join(", ")) };
        out.push_str(&format!("  {:<22} {scope}\n", l.term));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, email: &str) -> Person {
        Person { name: name.into(), email: Some(email.into()) }
    }

    #[test]
    fn learned_terms_come_back_only_with_their_meeting() {
        // Learning writes the data folder's taxonomy: a folder of the test's own.
        let (_env, _root) = crate::paths::testenv::folders("vocab-learned");
        let me = p("Jane Doe", "jane@x.com");
        let dial9 = MeetingContext { subject: Some("Analyse dial9".into()), attendees: vec![me.clone(), p("Karen Ortiz", "karen@y.com")], ..Default::default() };
        let other = MeetingContext { subject: None, attendees: vec![me.clone(), p("Kate Lin", "kate@x.com")], ..Default::default() };
        let mut cfg = Config { me: Some(me.clone()), ..Default::default() };
        let dir = std::env::temp_dir().join(format!("echo-vocab-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(crate::meeting::FILE), serde_json::to_vec(&dial9).unwrap()).unwrap();
        assert_eq!(learn(&mut cfg, &dir, &["Flow Studio".into(), "x".into()]), ["Flow Studio"]);
        cfg.vocabulary = vec!["Contoso".into()];
        // The same meeting again, or Karen in a call of her own: the term comes back.
        assert_eq!(for_meeting(&cfg, Some(&dial9)), ["Contoso", "Flow Studio"]);
        let karen = MeetingContext { attendees: vec![p("Karen Ortiz", "Karen@y.com")], ..Default::default() };
        assert_eq!(for_meeting(&cfg, Some(&karen)), ["Contoso", "Flow Studio"]);
        // Someone else: only the always-on list.
        assert_eq!(for_meeting(&cfg, Some(&other)), ["Contoso"]);
        assert_eq!(for_meeting(&cfg, None), ["Contoso"]);
        // An invite that names the term brings it in.
        let named = MeetingContext { subject: Some("Flow Studio review".into()), ..Default::default() };
        assert_eq!(for_meeting(&cfg, Some(&named)), ["Contoso", "Flow Studio"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(feature = "brain")]
    #[test]
    fn the_old_learned_list_moves_into_the_taxonomy() {
        let (_env, _root) = crate::paths::testenv::folders("vocab-migrate");
        let me = p("Jane Doe", "jane@x.com");
        let mut cfg = Config { me: Some(me.clone()), ..Default::default() };
        cfg.learned = Some(vec![Learned { term: "Flow Studio".into(), subjects: vec!["analyse dial".into()], people: vec!["karen@y.com".into()], calls: vec![] }]);
        cfg.save().unwrap();
        cfg.vocabulary = vec!["Contoso".into()];
        cfg.save().unwrap();
        assert_eq!(migrate_learned(&mut cfg).unwrap(), 1);
        assert_eq!(cfg.learned, Some(vec![]));
        // The always-on list stays always-on across loads (the old migration mustn't scope it away).
        let again = Config::load().unwrap();
        assert_eq!((again.vocabulary.clone(), again.learned.clone()), (vec!["Contoso".to_string()], Some(vec![])));
        assert!(crate::paths::state_dir().join("learned-before-taxonomy.json").exists());
        let t = crate::brain::taxonomy::learned();
        assert_eq!((t[0].name.as_str(), t[0].layer.as_str(), t[0].kind.as_str()), ("Flow Studio", "shared", "learned"));
        // Its scope still decides: Karen's call gets it, someone else's doesn't.
        let karen = MeetingContext { attendees: vec![me.clone(), p("Karen Ortiz", "karen@y.com")], ..Default::default() };
        let other = MeetingContext { attendees: vec![me, p("Kate Lin", "kate@x.com")], ..Default::default() };
        assert_eq!(for_meeting(&cfg, Some(&karen)), ["Contoso", "Flow Studio"]);
        assert_eq!(for_meeting(&cfg, Some(&other)), ["Contoso"]);
        // Nothing left to move; forgetting it removes the learned-only term.
        assert_eq!(migrate_learned(&mut cfg).unwrap(), 0);
        forget_learned(&mut cfg, "flow studio");
        assert!(crate::brain::taxonomy::learned().is_empty());
    }

    #[test]
    fn every_call_terms_stay_every_call_across_loads() {
        let (_env, _root) = crate::paths::testenv::folders("vocab-always");
        // A new data folder: the first term is added for every call, then the config is loaded again.
        let mut cfg = Config::load().unwrap();
        add(&mut cfg, &["Kubernetes".into()], None, true).unwrap();
        cfg.save().unwrap();
        let again = Config::load().unwrap();
        assert_eq!((again.vocabulary, again.learned), (vec!["Kubernetes".to_string()], Some(vec![])));
        // A config from before learned terms (no `learned` key) is still scoped, once.
        fs::write(paths::state_dir().join("config.json"), r#"{"vocabulary": ["Contoso"]}"#).unwrap();
        let old = Config::load().unwrap();
        assert!(old.vocabulary.is_empty());
        assert_eq!(old.learned.unwrap().iter().map(|l| l.term.as_str()).collect::<Vec<_>>(), ["Contoso"]);
    }

    #[test]
    fn subjects_match_without_numbers_or_punctuation() {
        assert_eq!(norm_subject("Weekly Sync #12 (EMEA)"), "weekly sync emea");
    }
}

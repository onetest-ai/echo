//! The Brain's taxonomy (`docs/brain-design.md`, "The taxonomy supersedes the glossaries"), in **two
//! layers**: a **shared** layer (the user's own practice, from calls with no client) and **one layer per
//! client** (that client's goal and audience, its own term kinds, from its calls) — client is a real
//! differentiator: the same name in two clients is two topics. Each layer has **topics** (two levels;
//! sections tagged with 0–3 from the shared layer and their own client's) and **terms** (names, systems,
//! acronyms… with spellings and variants: what thread glossaries were). Versions are files
//! (`<home>/brain/taxonomy/vN.json`, `current.json`, `decisions.jsonl`); the store's graph and tags follow
//! the current version. Claude proposes (a layer's build, classification); renames, merges and removals
//! are the user's and carry the tags with them (Brain's migrations).

use anyhow::{anyhow, bail, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use super::store;

mod drift;
mod induce;
mod mentions;
mod view;
pub use drift::*;
pub use induce::*;
pub use mentions::*;
pub use view::*;

/// Sections per classification request, and the characters of each shown.
const CLASSIFY_BATCH: usize = 40;
const PREVIEW: usize = 400;
/// Calls per map request in a build.
const MAP_BATCH: usize = 4;
/// Names this alike are one candidate (Brain's `consolidate`: SequenceMatcher ratio ≥ 0.86).
const SAME: f64 = 0.86;
pub const NO_TOPIC: &str = "__no_topic__";
/// The layer of the user's own practice (calls with no client).
pub const SHARED: &str = "shared";

fn shared() -> String {
    SHARED.into()
}

fn is_shared(s: &String) -> bool {
    s == SHARED
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Topic {
    /// `shared` or a client's id.
    #[serde(default = "shared", skip_serializing_if = "is_shared")]
    pub layer: String,
    pub label: String,
    /// 1 or 2.
    pub level: u8,
    /// Its level-1 topic: one of its own layer's, else (a client's topic) one of the practice's — one
    /// tree for the whole Brain, a client's topics placed under the practice's where they fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub description: String,
    /// Other names for it: its old labels (renames, merges, a rebuild's wording) and the user's. A
    /// rebuild keeps a topic that one of them names; tagging takes them too.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub synonyms: Vec<String>,
}

/// A topic's level-1 parent: its own layer's of that label, else the practice's (a client topic under it).
pub fn parent_of<'a>(t: &'a Taxonomy, x: &Topic) -> Option<&'a Topic> {
    let p = nid(x.parent.as_deref()?);
    t.topics.iter().find(|y| y.layer == x.layer && y.level == 1 && nid(&y.label) == p)
        .or_else(|| t.topics.iter().find(|y| y.layer == SHARED && y.level == 1 && nid(&y.label) == p))
}

/// Does this name one topic: its label or one of its synonyms.
fn names(x: &Topic, name: &str) -> bool {
    let k = nid(name);
    nid(&x.label) == k || x.synonyms.iter().any(|s| nid(s) == k)
}

fn add_synonym(x: &mut Topic, name: &str) {
    if !name.trim().is_empty() && nid(name) != nid(&x.label) && !x.synonyms.iter().any(|s| nid(s) == nid(name)) {
        x.synonyms.push(name.trim().to_string());
    }
}

/// The dictionary's groups (a term's `kind` is its subtype within one): a short fixed list, so the
/// same sort of thing isn't "product" in one layer and "platform" in another.
pub const GROUPS: [(&str, &str); 8] = [
    ("organisation", "Organisations"), ("product", "Products & systems"), ("initiative", "Initiatives & projects"), ("document", "Documents"),
    ("metric", "Metrics"), ("method", "Methods & processes"), ("concept", "Concepts & acronyms"), ("other", "Other"),
];

/// The group a kind falls in (the model's open words, mapped; unknown ones are "other").
pub fn group_of(kind: &str) -> &'static str {
    match kind.trim().to_lowercase().as_str() {
        "organisation" | "organization" | "company" | "client" | "vendor" | "partner" | "team" | "department" | "region" | "branch" => "organisation",
        "product" | "system" | "tool" | "platform" | "server" | "model" | "framework" | "driver" | "protocol" | "application" | "app" | "service" | "database" | "library" => "product",
        "initiative" | "project" | "programme" | "program" | "offering" | "milestone" | "workstream" | "pilot" | "engagement" => "initiative",
        "document" | "report" | "template" | "contract" | "policy" | "deck" => "document",
        "metric" | "kpi" | "measure" => "metric",
        "method" | "process" | "governance" | "discipline" | "practice" | "methodology" | "ceremony" => "method",
        "acronym" | "concept" | "feature" | "requirement" | "term" | "capability" => "concept",
        _ => "other",
    }
}

impl Term {
    /// Its dictionary group: set, else from its kind.
    pub fn group(&self) -> &str {
        if self.group.is_empty() { group_of(&self.kind) } else { &self.group }
    }
    /// Every name it goes by: its name, synonyms and spellings.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        std::iter::once(&self.name).chain(&self.synonyms).chain(&self.variants)
    }
    fn is(&self, name: &str) -> bool {
        self.names().any(|n| nid(n) == nid(name))
    }
}

/// A term written before synonyms: its `variants` held both. An abbreviation, or a name nothing like
/// its own, is another name (a synonym); a near spelling stays a spelling. Same answer every load.
fn split_variants(x: &mut Term) {
    if !x.synonyms.is_empty() {
        return;
    }
    let abbrev = |v: &str| { let c: Vec<char> = v.chars().filter(|c| !c.is_whitespace()).collect(); (2..=6).contains(&c.len()) && c.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '&') };
    // Another script than its name's is how a transcription heard it ("Эко" for Echo): a spelling.
    let cyr = |v: &str| v.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c));
    let (syn, spell): (Vec<String>, Vec<String>) = std::mem::take(&mut x.variants).into_iter().partition(|v| cyr(v) == cyr(&x.name) && (abbrev(v) || ratio(v, &x.name) < 0.75));
    x.synonyms = syn;
    x.variants = spell;
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Term {
    #[serde(default = "shared", skip_serializing_if = "is_shared")]
    pub layer: String,
    pub name: String,
    /// The layer's own word for what it is (system, initiative, branch, acronym…): open — its subtype
    /// within its `group`.
    #[serde(default)]
    pub kind: String,
    /// How it is written or misheard (the transcription hint and finding its mentions).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<String>,
    /// Its other names: abbreviations, older or informal names, translations ("SG" for Signal Grid)
    /// — what search, the Brain and Claude take for it. Renames and merges keep old names here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub synonyms: Vec<String>,
    /// Its group in the dictionary (one of `GROUPS`); empty: read from its kind (`group_of`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    #[serde(default)]
    pub description: String,
    /// Threads it is pinned to (else it belongs where it is mentioned).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub threads: Vec<String>,
    /// Hint every call's transcription.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub always: bool,
    /// Learned from calls by the minutes (it superseded `config.learned`): used again in a call that
    /// shares one of these meeting subjects, has mostly these people, or whose invite names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned: Option<LearnedScope>,
}

/// Where a learned term came up (subjects normalised; people by email, else name; call folders).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LearnedScope {
    #[serde(default)]
    pub subjects: Vec<String>,
    #[serde(default)]
    pub people: Vec<String>,
    #[serde(default)]
    pub calls: Vec<String>,
}

/// A term to learn: its layer (the call's client's, else shared), the threads the call is in, and
/// the scope it came up in.
#[derive(Debug, Clone, Default)]
pub struct Learn {
    pub term: String,
    pub layer: String,
    pub threads: Vec<String>,
    pub scope: LearnedScope,
}

fn push_new<T: PartialEq + Clone>(v: &mut Vec<T>, xs: &[T]) {
    for x in xs {
        if !v.contains(x) {
            v.push(x.clone());
        }
    }
}

/// One term's learned scope added to another's.
fn merge_learned(into: &mut Term, from: &Term) {
    let Some(l) = &from.learned else { return };
    let x = into.learned.get_or_insert_with(LearnedScope::default);
    push_new(&mut x.subjects, &l.subjects);
    push_new(&mut x.people, &l.people);
    push_new(&mut x.calls, &l.calls);
}

/// A layer rebuilt: what the user and the minutes put on its terms carries over to the new ones of the
/// same name or spelling — pinned threads, always, the learned scope, other spellings, a description —
/// and a term the model didn't name again is kept when it carries any of that (learned vocabulary, a
/// pin, always). A rebuild never loses them.
fn carry_over(old_terms: &[Term], lterms: &mut Vec<Term>) {
    let same_term = |a: &Term, b: &Term| a.is(&b.name) || b.is(&a.name);
    for x in lterms.iter_mut() {
        let matches: Vec<&Term> = old_terms.iter().filter(|o| same_term(o, x)).collect();
        for old in matches {
            push_new(&mut x.threads, &old.threads);
            x.always |= old.always;
            if x.learned.is_none() {
                x.learned = old.learned.clone();
            }
            push_new(&mut x.variants, &old.variants);
            push_new(&mut x.synonyms, &old.synonyms);
            if x.group.is_empty() {
                x.group = old.group.clone();
            }
            if x.description.is_empty() {
                x.description = old.description.clone();
            }
        }
    }
    for old in old_terms {
        let signals = old.learned.is_some() || old.always || !old.threads.is_empty() || old.kind == "learned";
        if signals && !lterms.iter().any(|x| same_term(x, old)) {
            lterms.push(old.clone());
        }
    }
}

/// Learn terms (from a minutes run, or the old list's migration): merged into the term of that name
/// (or variant) in its layer — its scope and threads added —, else a new term of kind `learned`.
/// One new version. Returns the terms that were new.
pub fn learn(items: &[Learn], by: &str) -> Result<Vec<String>> {
    if items.is_empty() {
        return Ok(vec![]);
    }
    let _lock = write_lock()?;
    let mut t = current().unwrap_or_default();
    t.layers.entry(SHARED.into()).or_insert_with(|| Layer { name: "Your practice".into(), ..Default::default() });
    let mut added = vec![];
    for it in items {
        if !t.layers.contains_key(&it.layer) {
            let name = crate::projects::clients().ok().and_then(|cs| cs.into_iter().find(|c| c.id == it.layer).map(|c| c.name)).unwrap_or_else(|| it.layer.clone());
            t.layers.insert(it.layer.clone(), Layer { name, ..Default::default() });
        }
        let pos = t.terms.iter().position(|x| x.layer == it.layer && (nid(&x.name) == nid(&it.term) || x.variants.iter().any(|v| nid(v) == nid(&it.term))));
        match pos {
            Some(i) => {
                let x = &mut t.terms[i];
                push_new(&mut x.threads, &it.threads);
                let s = x.learned.get_or_insert_with(LearnedScope::default);
                push_new(&mut s.subjects, &it.scope.subjects);
                push_new(&mut s.people, &it.scope.people);
                push_new(&mut s.calls, &it.scope.calls);
            }
            None => {
                added.push(it.term.clone());
                t.terms.push(Term { layer: it.layer.clone(), name: it.term.clone(), kind: "learned".into(), threads: it.threads.clone(), learned: Some(it.scope.clone()), ..Default::default() });
            }
        }
    }
    save(t, json!({"op": "learn", "by": by, "terms": items.iter().map(|i| &i.term).collect::<Vec<_>>()}))?;
    Ok(added)
}

/// A plugin's names for one thing (`share::dictionary_add`): merged into the term any of them already is — the
/// client's layer first, then the practice's, then any — its other names added as synonyms or spellings; else a new
/// term in `client`'s layer (or the practice's). Nothing is renamed or removed. One new version when anything changed.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn dictionary_add(name: &str, synonyms: &[String], spellings: &[String], kind: Option<&str>, client: Option<&str>, by: &str) -> Result<Value> {
    let _lock = write_lock()?;
    let mut t = current().unwrap_or_default();
    t.layers.entry(SHARED.into()).or_insert_with(|| Layer { name: "Your practice".into(), ..Default::default() });
    let names: Vec<String> = std::iter::once(name.to_string()).chain(synonyms.iter().cloned()).collect();
    let keys: BTreeSet<String> = names.iter().chain(spellings).map(|n| nid(n)).filter(|k| !k.is_empty()).collect();
    let is_it = |x: &Term| std::iter::once(&x.name).chain(&x.synonyms).chain(&x.variants).any(|n| keys.contains(&nid(n)));
    let layer = client.unwrap_or(SHARED);
    let pos = t.terms.iter().position(|x| x.layer == layer && is_it(x)).or_else(|| t.terms.iter().position(|x| x.layer == SHARED && is_it(x))).or_else(|| t.terms.iter().position(is_it));
    let (term, added, new) = match pos {
        Some(i) => {
            let x = &mut t.terms[i];
            let mut added = vec![];
            let known = |x: &Term, n: &str| std::iter::once(&x.name).chain(&x.synonyms).chain(&x.variants).any(|k| nid(k) == nid(n));
            for n in &names {
                if !known(x, n) {
                    x.synonyms.push(n.clone());
                    added.push(n.clone());
                }
            }
            for n in spellings {
                if !known(x, n) {
                    x.variants.push(n.clone());
                    added.push(n.clone());
                }
            }
            (x.name.clone(), added, false)
        }
        None => {
            if !t.layers.contains_key(layer) {
                let n = crate::projects::clients().ok().and_then(|cs| cs.into_iter().find(|c| c.id == layer).map(|c| c.name)).unwrap_or_else(|| layer.to_string());
                t.layers.insert(layer.to_string(), Layer { name: n, ..Default::default() });
            }
            let mut syn: Vec<String> = vec![];
            push_new(&mut syn, synonyms);
            t.terms.push(Term { layer: layer.to_string(), name: name.to_string(), kind: kind.unwrap_or("other").to_string(), synonyms: syn, variants: spellings.to_vec(), ..Default::default() });
            (name.to_string(), names.iter().chain(spellings).cloned().collect(), true)
        }
    };
    if !added.is_empty() {
        save(t, json!({"op": "dictionary_add", "by": by, "term": term, "added": added}))?;
    }
    Ok(json!({"term": term, "added": added, "new": new}))
}

/// Every learned term (with its scope).
pub fn learned() -> Vec<Term> {
    current().map(|t| t.terms.into_iter().filter(|x| x.learned.is_some()).collect()).unwrap_or_default()
}

/// Forget a learned term: a term that is only that (kind `learned`, no description) goes; one the
/// taxonomy also knows keeps its other facts and loses the learned scope.
pub fn forget_learned(term: &str) -> Result<bool> {
    let _lock = write_lock()?;
    let Some(mut t) = current() else { return Ok(false) };
    let before = t.terms.len();
    let mut changed = false;
    t.terms.retain_mut(|x| {
        if x.learned.is_none() || nid(&x.name) != nid(term) {
            return true;
        }
        changed = true;
        if x.kind == "learned" && x.description.is_empty() && !x.always {
            return false;
        }
        x.learned = None;
        true
    });
    if changed {
        save(t, json!({"op": "forget_learned", "by": "you", "name": term, "removed": before > 0}))?;
    }
    Ok(changed)
}

/// A layer's own facts (Brain's per-Brain ones: goal, audience, summary, demoted).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub audience: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    /// Candidates its build left out, with how many calls named them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub demoted: Vec<(String, usize)>,
    /// Built by Claude and not reviewed by the user yet.
    #[serde(default)]
    pub provisional: bool,
    #[serde(default)]
    pub built_from: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Taxonomy {
    pub version: u32,
    #[serde(default)]
    pub layers: BTreeMap<String, Layer>,
    #[serde(default)]
    pub topics: Vec<Topic>,
    #[serde(default)]
    pub terms: Vec<Term>,
    /// What a reviewer should look at (Brain's `review_flags`): near-duplicates, per layer.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub review_flags: Value,
    /// Old node id → current node id (renames, merges).
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
    #[serde(default)]
    pub history: Vec<Value>,
    // A version from before layers (one global list): its facts become the shared layer's.
    #[serde(default, skip_serializing)]
    goal: String,
    #[serde(default, skip_serializing)]
    summary: String,
    #[serde(default, skip_serializing)]
    provisional: bool,
}

impl Taxonomy {
    pub fn provisional(&self) -> bool {
        self.layers.values().any(|l| l.provisional)
    }
}

/// Brain's node id: lower case, every run of other characters one `_`.
pub fn nid(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.to_lowercase().chars() {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('_');
            }
            out.push(c);
            gap = false;
        } else {
            gap = true;
        }
    }
    if out.is_empty() {
        "n".into()
    } else {
        out
    }
}

/// A topic's node id: Brain's slug in the shared layer, prefixed by the client's id in a client's.
pub fn topic_id(layer: &str, label: &str) -> String {
    if layer == SHARED {
        nid(label)
    } else {
        format!("{}__{}", nid(layer), nid(label))
    }
}

pub fn term_id(layer: &str, name: &str) -> String {
    format!("term__{}", topic_id(layer, name))
}

fn dir() -> PathBuf {
    super::store_path().with_file_name("taxonomy")
}

/// One writer at a time: every change reads `current.json`, changes it and saves a version — the app,
/// a minutes run's `vocab add` and the CLI can be at it together. Waits up to 30 s for another writer.
fn write_lock() -> Result<crate::app::FileLock> {
    std::fs::create_dir_all(dir())?;
    let path = dir().join(".lock");
    let start = std::time::Instant::now();
    loop {
        if let Some(l) = crate::app::FileLock::try_take(&path)? {
            return Ok(l);
        }
        if start.elapsed() > std::time::Duration::from_secs(30) {
            bail!("the taxonomy is being changed by another process");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

pub fn current() -> Option<Taxonomy> {
    let mut t: Taxonomy = std::fs::read(dir().join("current.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())?;
    if t.layers.is_empty() && !t.topics.is_empty() {
        t.layers.insert(SHARED.into(), Layer { name: "Your practice".into(), goal: t.goal.clone(), summary: t.summary.clone(), provisional: t.provisional, ..Default::default() });
    }
    t.terms.iter_mut().for_each(split_variants);
    Some(t)
}

/// Save a new version (immutable `vN.json`, and `current.json`) and bring the store's graph and tags
/// in line with it.
fn save(mut t: Taxonomy, what: Value) -> Result<Taxonomy> {
    let before = current();
    t.version = before.as_ref().map(|c| c.version).unwrap_or(0) + 1;
    t.review_flags = review_flags(&t);
    let mut h = what.clone();
    h["version"] = json!(t.version);
    h["at"] = json!(chrono::Utc::now().to_rfc3339());
    t.history.push(h.clone());
    std::fs::create_dir_all(dir())?;
    let bytes = serde_json::to_vec_pretty(&t)?;
    crate::paths::write_atomic(&dir().join(format!("v{}.json", t.version)), &bytes)?;
    crate::paths::write_atomic(&dir().join("current.json"), &bytes)?;
    use std::io::Write;
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(dir().join("decisions.jsonl"))?;
    writeln!(log, "{h}")?;
    build_graph(&store::open(&super::store_path())?, &t, before.as_ref())?;
    Ok(t)
}

/// Near-duplicate topics (same layer and level, names this alike) and terms: a reviewer's first look.
fn review_flags(t: &Taxonomy) -> Value {
    let mut topics = vec![];
    for (i, a) in t.topics.iter().enumerate() {
        for b in t.topics.iter().skip(i + 1) {
            if a.layer == b.layer && a.level == b.level && ratio(&a.label, &b.label) >= 0.75 {
                topics.push(json!({"layer": a.layer, "pair": [a.label, b.label]}));
            }
        }
    }
    // Terms (tens of thousands): each compared with its neighbours in A–Z order within its layer (near-duplicates
    // share a beginning), and a spelling that is another term's name found through a map — never each with each.
    let mut terms = vec![];
    let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut by_layer: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, x) in t.terms.iter().enumerate() {
        by_layer.entry(x.layer.as_str()).or_default().push(i);
    }
    let mut pair = |a: usize, b: usize, terms: &mut Vec<Value>| {
        let (a, b) = (a.min(b), a.max(b));
        if a != b && seen.insert((a, b)) {
            terms.push(json!({"layer": t.terms[a].layer, "pair": [t.terms[a].name, t.terms[b].name]}));
        }
    };
    for (_, mut ids) in by_layer {
        let by_name: HashMap<String, usize> = ids.iter().map(|i| (nid(&t.terms[*i].name), *i)).collect();
        // Neighbours by beginning, then by ending (names that differ in their first letters).
        for key in [|n: &str| nid(n), |n: &str| nid(n).chars().rev().collect::<String>()] {
            ids.sort_by_key(|i| key(&t.terms[*i].name));
            for (k, a) in ids.iter().enumerate() {
                for b in ids.iter().skip(k + 1).take(12) {
                    if ratio(&t.terms[*a].name, &t.terms[*b].name) >= 0.8 {
                        pair(*a, *b, &mut terms);
                    }
                }
            }
        }
        for a in &ids {
            for v in &t.terms[*a].variants {
                if let Some(b) = by_name.get(&nid(v)) {
                    pair(*a, *b, &mut terms);
                }
            }
        }
    }
    json!({"near_duplicate_topics": topics, "near_duplicate_terms": terms})
}

/// Build every layer that has calls: the shared one and each client's.
pub fn induce_all(progress: &dyn Fn(&str)) -> Result<Value> {
    let mut out = vec![];
    for l in buildable()? {
        out.push(induce(&l, progress).unwrap_or_else(|e| json!({"layer": l, "error": format!("{e:#}")})));
    }
    Ok(json!({"layers": out}))
}

/// Record that `old` is now `new` (a rename, merge, fold, move): what pointed at `old` points at `new` too,
/// and `new` itself is no alias any more — so aliases never chain or loop (a fold, then a move back down,
/// would otherwise send tags in a circle and lose them).
fn set_alias(aliases: &mut BTreeMap<String, String>, old: String, new: String) {
    aliases.remove(&new);
    if old == new {
        return;
    }
    for v in aliases.values_mut() {
        if *v == old {
            *v = new.clone();
        }
    }
    aliases.insert(old, new);
}

/// A client layer never repeats the practice (#32, #33): a client topic that names a practice topic (its
/// label or a synonym, or nearly: ratio ≥ 0.86) isn't made — its subtopics go under the practice's topic,
/// its own tags move there (an alias) — and a client term that is a practice term by any of its names
/// isn't made either (its thread pins move to the practice's term). Returns what was folded, for the log.
fn fold_into_practice(t: &mut Taxonomy, layer: &str) -> Vec<String> {
    if layer == SHARED {
        return vec![];
    }
    let practice: Vec<Topic> = t.topics.iter().filter(|x| x.layer == SHARED).cloned().collect();
    let same_topic = |p: &Topic, x: &Topic| names(p, &x.label) || x.synonyms.iter().any(|s| names(p, s)) || ratio(&p.label, &x.label) >= SAME;
    let mut folded = vec![];
    let mut gone: Vec<(String, Topic)> = vec![];
    for x in t.topics.iter().filter(|x| x.layer == layer) {
        if let Some(p) = practice.iter().find(|p| same_topic(p, x)) {
            gone.push((x.label.clone(), p.clone()));
        }
    }
    for (label, p) in &gone {
        // Its subtopics sit under the practice's level-1 topic (the matched one, or the one it sits under).
        let home = if p.level == 1 { p.label.clone() } else { p.parent.clone().unwrap_or_else(|| p.label.clone()) };
        for k in t.topics.iter_mut().filter(|k| k.layer == layer && k.level == 2 && k.parent.as_deref().is_some_and(|q| nid(q) == nid(label))) {
            k.parent = Some(home.clone());
        }
        t.topics.retain(|x| !(x.layer == layer && nid(&x.label) == nid(label)));
        set_alias(&mut t.aliases, topic_id(layer, label), topic_id(SHARED, &p.label));
        folded.push(format!("topic “{label}” → the practice's “{}”", p.label));
    }
    let practice_terms: Vec<Term> = t.terms.iter().filter(|x| x.layer == SHARED).cloned().collect();
    let mut moved: Vec<(Term, String)> = vec![];
    t.terms.retain(|m| {
        if m.layer != layer {
            return true;
        }
        match practice_terms.iter().find(|pt| m.names().any(|n| pt.is(n))) {
            Some(pt) => {
                moved.push((m.clone(), pt.name.clone()));
                false
            }
            None => true,
        }
    });
    for (m, into) in moved {
        // Merged as `edit` merge does: its names become the practice term's other names, its spellings
        // (transcription hints) its spellings; pins, always, what was learned and a description carry over.
        if let Some(pt) = t.terms.iter_mut().find(|x| x.layer == SHARED && x.name == into) {
            for n in std::iter::once(m.name.clone()).chain(m.synonyms.clone()) {
                if !pt.is(&n) {
                    pt.synonyms.push(n);
                }
            }
            for v in &m.variants {
                if !pt.is(v) {
                    pt.variants.push(v.clone());
                }
            }
            for th in &m.threads {
                if !pt.threads.contains(th) {
                    pt.threads.push(th.clone());
                }
            }
            pt.always |= m.always;
            if pt.description.trim().is_empty() {
                pt.description = m.description.clone();
            }
            merge_learned(pt, &m);
        }
        set_alias(&mut t.aliases, term_id(layer, &m.name), term_id(SHARED, &into));
        folded.push(format!("term “{}” → the practice's “{into}”", m.name));
    }
    folded
}

/// A rebuild keeps what it still has: a new topic that an old one's label or synonym names (or nearly:
/// the names' ratio ≥ 0.86) takes the old label — so its id, tags, insights and history go on —, the
/// new wording kept as a synonym, the old synonyms (and its description, when the new one has none) kept.
fn keep_identity(old: &[Topic], new: &mut [Topic]) {
    let mut taken = BTreeSet::new();
    let label_of = |n: &Topic| -> Option<usize> {
        old.iter().position(|o| names(o, &n.label) || n.synonyms.iter().any(|s| names(o, s))).or_else(|| old.iter().position(|o| ratio(&o.label, &n.label) >= SAME))
    };
    let mut renamed: Vec<(String, String)> = vec![];
    for n in new.iter_mut() {
        let Some(i) = label_of(n).filter(|i| taken.insert(*i)) else { continue };
        let o = &old[i];
        if nid(&o.label) != nid(&n.label) {
            renamed.push((n.label.clone(), o.label.clone()));
            let wording = std::mem::replace(&mut n.label, o.label.clone());
            add_synonym(n, &wording);
        }
        for s in &o.synonyms {
            add_synonym(n, s);
        }
        if n.description.trim().is_empty() {
            n.description = o.description.clone();
        }
    }
    // Their children follow the label their parent kept.
    for n in new.iter_mut() {
        if let Some((_, to)) = n.parent.as_deref().and_then(|p| renamed.iter().find(|(from, _)| nid(from) == nid(p))) {
            n.parent = Some(to.clone());
        }
    }
}

#[cfg(test)]
fn parse_layer(v: &Value, layer: &str) -> Result<(Vec<Topic>, Vec<Term>)> {
    parse_layer_under(v, layer, &BTreeSet::new())
}

/// As `parse_layer`; a client layer's level-2 topics may sit under one of the practice's level-1
/// topics (`practice`: their labels' ids).
fn parse_layer_under(v: &Value, layer: &str, practice: &BTreeSet<String>) -> Result<(Vec<Topic>, Vec<Term>)> {
    let mut topics: Vec<Topic> = vec![];
    let mut seen = BTreeSet::new();
    for tp in v["topics"].as_array().into_iter().flatten() {
        let label = tp["label"].as_str().unwrap_or("").trim().to_string();
        if label.is_empty() || !seen.insert(nid(&label)) {
            continue;
        }
        let level = if tp["level"].as_u64() == Some(2) { 2 } else { 1 };
        let synonyms = tp["synonyms"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty() && nid(s) != nid(&label)).collect();
        topics.push(Topic { layer: layer.into(), label, level, parent: tp["parent"].as_str().map(|s| s.trim().to_string()).filter(|_| level == 2), description: tp["description"].as_str().unwrap_or("").to_string(), synonyms });
    }
    // A level-2 topic whose parent isn't a level-1 one becomes level 1.
    let mut l1: BTreeSet<String> = topics.iter().filter(|x| x.level == 1).map(|x| nid(&x.label)).collect();
    if layer != SHARED {
        l1.extend(practice.iter().cloned());
    }
    for x in topics.iter_mut().filter(|x| x.level == 2) {
        if !x.parent.as_deref().is_some_and(|p| l1.contains(&nid(p))) {
            x.level = 1;
            x.parent = None;
        }
    }
    let mut terms = vec![];
    let mut tn = BTreeSet::new();
    for tm in v["terms"].as_array().into_iter().flatten() {
        let name = tm["name"].as_str().unwrap_or("").trim().to_string();
        if name.is_empty() || !tn.insert(nid(&name)) {
            continue;
        }
        let kind = tm["kind"].as_str().map(|k| k.trim().to_lowercase()).filter(|k| !k.is_empty() && k.len() <= 24).unwrap_or_else(|| "other".into());
        let variants = tm["variants"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty() && nid(s) != nid(&name)).collect();
        let synonyms = tm["synonyms"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty() && nid(s) != nid(&name)).collect();
        let group = tm["group"].as_str().map(|g| g.trim().to_lowercase()).filter(|g| GROUPS.iter().any(|(k, _)| k == g)).unwrap_or_default();
        terms.push(Term { layer: layer.into(), name, kind, variants, synonyms, group, description: tm["description"].as_str().unwrap_or("").to_string(), ..Default::default() });
    }
    if topics.is_empty() {
        bail!("the model proposed no topics");
    }
    Ok((topics, terms))
}

/// The vocabulary a section is tagged from: the shared layer and its clients' layers, as text for the
/// model, labels qualified by their layer when two layers share one.
fn vocab_text(t: &Taxonomy, layers: &[String]) -> String {
    let mut out = String::new();
    let name = |l: &str| t.layers.get(l).map(|x| x.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| l.to_string());
    let line = |x: &Topic, indent: &str| {
        let who = if x.layer == SHARED { String::new() } else { format!(" ({})", name(&x.layer)) };
        let also = if x.synonyms.is_empty() { String::new() } else { format!(" [also: {}]", x.synonyms.join(", ")) };
        format!("{indent}- {}{who}: {}{also}\n", x.label, x.description)
    };
    let shown = |x: &Topic| layers.contains(&x.layer);
    // One tree: the practice's topics, each with its own subtopics and the clients' topics placed under it;
    // then each client's topics of its own.
    for l1 in t.topics.iter().filter(|x| x.level == 1 && x.layer == SHARED && shown(x)) {
        out.push_str(&line(l1, ""));
        for k in t.topics.iter().filter(|x| x.level == 2 && shown(x) && parent_of(t, x).is_some_and(|p| std::ptr::eq(p, l1))) {
            out.push_str(&line(k, "  "));
        }
    }
    for l in layers.iter().filter(|l| l.as_str() != SHARED) {
        let own: Vec<&Topic> = t.topics.iter().filter(|x| &x.layer == l && x.level == 1).collect();
        if own.is_empty() {
            continue;
        }
        out.push_str(&format!("[{} only]\n", name(l)));
        for l1 in own {
            out.push_str(&line(l1, ""));
            for k in t.topics.iter().filter(|x| x.level == 2 && &x.layer == l && parent_of(t, x).is_some_and(|p| std::ptr::eq(p, l1))) {
                out.push_str(&line(k, "  "));
            }
        }
    }
    out
}

/// A label the model gave, among these layers: a client's topic first (it is the more specific).
fn resolve<'a>(t: &'a Taxonomy, layers: &[String], label: &str) -> Option<&'a Topic> {
    // The vocabulary marks a client's topic "(Client)": the label is what precedes it.
    let label = label.rsplit_once(" (").filter(|(_, r)| r.ends_with(')')).map(|(l, _)| l).unwrap_or(label);
    layers.iter().rev().find_map(|l| {
        let id = topic_id(l, label);
        let id = t.aliases.get(&id).cloned().unwrap_or(id);
        t.topics.iter().find(|x| &x.layer == l && (names(x, label) || topic_id(&x.layer, &x.label) == id))
    })
}

/// Tag sections with topics (0–3 each, from the shared layer and their client's; an L2 brings its L1):
/// those not judged yet — or, with `again`, every section of `layer` (all with no layer).
pub fn classify(layer: Option<&str>, again: bool, progress: &dyn Fn(&str)) -> Result<usize> {
    classify_in(layer, again, None, progress)
}

/// As `classify`, limited to the sections in `only` when given (a split's).
fn classify_in(layer: Option<&str>, again: bool, only: Option<&BTreeSet<i64>>, progress: &dyn Fn(&str)) -> Result<usize> {
    let t = current().ok_or_else(|| anyhow!("there is no taxonomy yet: build it first"))?;
    let c = store::open(&super::store_path())?;
    let clients = chunk_clients(&c)?;
    let in_layer = |id: &i64| -> bool {
        if only.is_some_and(|o| !o.contains(id)) {
            return false;
        }
        match layer {
            None => true,
            Some(SHARED) => clients.get(id).is_none_or(|s| s.contains("-")),
            Some(l) => clients.get(id).is_some_and(|s| s.contains(l)),
        }
    };
    if again {
        let ids: Vec<i64> = c.prepare("SELECT id FROM chunks")?.query_map([], |r| r.get(0))?.collect::<Result<Vec<i64>, _>>()?.into_iter().filter(|i| in_layer(i)).collect();
        let tx = c.unchecked_transaction()?;
        for id in &ids {
            tx.execute("DELETE FROM chunk_topics WHERE chunk_id = ?1 AND kind IN ('intent_l1', 'intent_l2')", [id])?;
            tx.execute("DELETE FROM chunk_verdicts WHERE chunk_id = ?1", [id])?;
            tx.execute("DELETE FROM graph_edges WHERE source = ?1 AND rel = 'about'", [format!("chunk:{id}")])?;
        }
        tx.commit()?;
    }
    let todo: Vec<(i64, String, String)> = c
        .prepare("SELECT c.id, coalesce(c.breadcrumb_path, c.title), c.text FROM chunks c WHERE c.status = 'ACTIVE'
                  AND NOT EXISTS (SELECT 1 FROM chunk_verdicts v WHERE v.chunk_id = c.id)
                  AND NOT EXISTS (SELECT 1 FROM chunk_topics k WHERE k.chunk_id = c.id AND k.kind IN ('intent_l1', 'intent_l2'))")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|x| in_layer(&x.0))
        .collect();
    // Tagging runs on the default profile: only its sources' sections (strict routing).
    let ok = super::access::Access::default_profile().chunks(&c)?;
    let todo: Vec<(i64, String, String)> = todo.into_iter().filter(|x| ok.contains(&x.0)).collect();
    // One vocabulary per set of layers: sections grouped by the layers they may be tagged from.
    let mut groups: BTreeMap<Vec<String>, Vec<&(i64, String, String)>> = BTreeMap::new();
    for x in &todo {
        groups.entry(layers_for(&t, clients.get(&x.0))).or_default().push(x);
    }
    let mut done = 0;
    for (layers, items) in groups {
        let vocab = vocab_text(&t, &layers);
        for batch in items.chunks(CLASSIFY_BATCH) {
            progress(&format!("tagging sections {}–{} of {}", done + 1, done + batch.len(), todo.len()));
            let list: Vec<Value> = batch.iter().map(|(id, title, text)| json!({"id": id.to_string(), "where": title, "text": text.chars().take(PREVIEW).collect::<String>()})).collect();
            let user = format!(
                "Vocabulary (by layer; level 1, with level 2 under it):\n{vocab}\nFor each section give 0–3 labels exactly as written above (prefer the most specific; a client's topic over a general one when both fit). [\"{NO_TOPIC}\"] for small talk or filler; [] when it is on topic but nothing fits.\nAnswer {{\"<id>\": [labels], …}} for every id.\n\n{}",
                serde_json::to_string(&list)?
            );
            let v = ask(CLASSIFY_SYSTEM, &user)?;
            let tx = c.unchecked_transaction()?;
            for (id, ..) in batch.iter() {
                let labels: Vec<String> = v[id.to_string()].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect();
                tx.execute("DELETE FROM chunk_topics WHERE chunk_id = ?1 AND kind IN ('intent_l1', 'intent_l2')", [id])?;
                tx.execute("DELETE FROM graph_edges WHERE source = ?1 AND rel = 'about'", [format!("chunk:{id}")])?;
                if labels.iter().any(|l| l == NO_TOPIC) {
                    tx.execute("INSERT OR REPLACE INTO chunk_verdicts(chunk_id, verdict, taxonomy_version) VALUES (?1, 'no_topic', ?2)", params![id, t.version.to_string()])?;
                    done += 1;
                    continue;
                }
                let mut tags: BTreeMap<String, &Topic> = BTreeMap::new();
                for l in labels.iter().take(3) {
                    if let Some(tp) = resolve(&t, &layers, l) {
                        tags.insert(topic_id(&tp.layer, &tp.label), tp);
                        if let Some(p) = parent_of(&t, tp) {
                            tags.insert(topic_id(&p.layer, &p.label), p);
                        }
                    }
                }
                for (k, tp) in &tags {
                    tx.execute("INSERT INTO chunk_topics(chunk_id, category_id, category_label, kind) VALUES (?1, ?2, ?3, ?4)", params![id, k, tp.label, if tp.level == 1 { "intent_l1" } else { "intent_l2" }])?;
                    tx.execute("INSERT INTO graph_edges(source, target, rel) VALUES (?1, ?2, 'about')", params![format!("chunk:{id}"), k])?;
                }
                tx.execute("INSERT OR REPLACE INTO chunk_verdicts(chunk_id, verdict, taxonomy_version) VALUES (?1, ?2, ?3)", params![id, if tags.is_empty() { "untagged" } else { "tagged" }, t.version.to_string()])?;
                done += 1;
            }
            tx.commit()?;
        }
    }
    Ok(done)
}

/// Brain's coverage: sections tagged, judged filler, judged with nothing fitting, not judged yet.
pub fn coverage(c: &Connection) -> Result<Value> {
    let n = |sql: &str| -> i64 { c.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };
    Ok(json!({
        "total": n("SELECT count(*) FROM chunks WHERE status = 'ACTIVE'"),
        "tagged": n("SELECT count(DISTINCT chunk_id) FROM chunk_topics WHERE kind IN ('intent_l1', 'intent_l2')"),
        "no_topic": n("SELECT count(*) FROM chunk_verdicts WHERE verdict = 'no_topic'"),
        "nothing_fits": n("SELECT count(*) FROM chunk_verdicts WHERE verdict = 'untagged'"),
        "not_judged": n("SELECT count(*) FROM chunks c WHERE status = 'ACTIVE' AND NOT EXISTS (SELECT 1 FROM chunk_verdicts v WHERE v.chunk_id = c.id)"),
    }))
}

/// What the tags add up to, per topic or term (in a scope): its sections, threads, clients (with sections each) and
/// the topics terms come up in. SQL aggregates over every tag — seconds on millions of sections — so kept per store
/// state (`stats`), never per taxonomy version: an edit shows at once with the counts it had.
#[derive(Default)]
pub struct Stats {
    sections: HashMap<String, i64>,
    threads: HashMap<String, BTreeSet<String>>,
    clients: HashMap<String, BTreeMap<String, i64>>,
    together: Vec<(String, String, i64)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rebuild_keeps_learned_and_pinned_terms() {
        let t = |name: &str| Term { layer: "acme".into(), name: name.into(), kind: "system".into(), ..Default::default() };
        let old = vec![
            Term { learned: Some(LearnedScope { subjects: vec!["weekly".into()], ..Default::default() }), variants: vec!["Dial 9".into()], ..t("Dial9") },
            Term { kind: "learned".into(), learned: Some(LearnedScope::default()), ..t("Flow Studio") },
            Term { threads: vec!["p1".into()], ..t("Stargate") },
            t("Something the model said once"),
        ];
        let mut new = vec![Term { description: "contact centre platform".into(), ..t("dial9") }];
        carry_over(&old, &mut new);
        let names: Vec<&str> = new.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["dial9", "Flow Studio", "Stargate"]);
        assert!(new[0].learned.is_some() && new[0].variants == ["Dial 9"] && new[0].description == "contact centre platform");
    }

    #[test]
    fn ids_names_and_clusters() {
        assert_eq!(nid("AI Agent Hub — MVP"), "ai_agent_hub_mvp");
        assert_eq!(nid("Москва Сити"), "москва_сити");
        assert_eq!(topic_id(SHARED, "Budget"), "budget");
        assert_eq!(topic_id("globex", "Budget"), "globex__budget", "the same name in a client is another node");
        assert_eq!(term_id("acme", "Dial9"), "term__acme__dial9");
        assert!(ratio("Agent Hub", "agent-hub") > 0.99);
        assert!(ratio("Data Studio", "Data Stuido") >= SAME);
        assert!(ratio("Budget", "Security") < SAME);
        let c = cluster(&[("Agent Hub".into(), 3), ("agent hub".into(), 1), ("Budget".into(), 2)]);
        assert_eq!(c[0], ("Agent Hub".to_string(), 4, vec!["Agent Hub".to_string(), "agent hub".to_string()]));
        assert!(word_in("we use SSO here", "SSO") && !word_in("ssoft", "sso") && word_in("Пилот в Берлине", "берлине"));
        assert_eq!(json_in("Sure:\n```json\n{\"a\": 1}\n```").unwrap()["a"], 1);
    }

    #[test]
    fn layers_keep_clients_apart() {
        let (_env, _root) = crate::paths::testenv::folders("brain-layers");
        let db = super::super::store_path();
        let input = |src: &str, client: &str, md: &str| store::Input {
            source: src.into(), kind: "call".into(), title: src.into(), event_date: None, md: md.into(),
            refs: json!({"threads": [{"id": format!("{client}-thread"), "client": client}]}), link: Box::new(|_| String::new()), speaker: Box::new(|_| None),
        };
        store::sync(&db, vec![
            input("call/a", "northwind", "# A\n## SSO\nSingle sign-on through Okta for the pilot.\n"),
            input("call/b", "contoso", "# B\n## Tenants\nOkta tenants and the budget.\n"),
        ], None).unwrap();
        let (mut ta, mut ma) = parse_layer(&json!({"topics": [{"label": "Identity", "level": 1}, {"label": "Pilot", "level": 2, "parent": "Identity"}, {"label": "Orphan", "level": 2, "parent": "Nowhere"}],
                                                   "terms": [{"name": "Okta", "kind": "Product", "variants": ["octa"]}, {"name": "okta"}]}), "northwind").unwrap();
        assert_eq!(ta.iter().find(|x| x.label == "Orphan").unwrap().level, 1, "no such parent: level 1");
        assert_eq!((ma.len(), ma[0].kind.as_str()), (1, "product"), "one term once; kinds are the layer's own words, lower case");
        let (mut tb, mut mb) = parse_layer(&json!({"topics": [{"label": "Identity", "level": 1}], "terms": [{"name": "Okta", "kind": "system"}]}), "contoso").unwrap();
        let mut t = Taxonomy::default();
        t.topics.append(&mut ta);
        t.topics.append(&mut tb);
        t.terms.append(&mut ma);
        t.terms.append(&mut mb);
        t.topics.push(Topic { layer: SHARED.into(), label: "Budget".into(), level: 1, ..Default::default() });
        for l in ["northwind", "contoso", SHARED] {
            t.layers.insert(l.into(), Layer { name: l.into(), provisional: true, ..Default::default() });
        }
        let t = save(t, json!({"op": "test"})).unwrap();
        let c = store::open(&db).unwrap();
        // A section's vocabulary: the shared layer and its own client's — never another client's.
        let clients = chunk_clients(&c).unwrap();
        let a: i64 = c.query_row("SELECT id FROM chunks WHERE source = 'call/a'", [], |r| r.get(0)).unwrap();
        let ls = layers_for(&t, clients.get(&a));
        assert_eq!(ls, vec![SHARED.to_string(), "northwind".to_string()]);
        assert!(vocab_text(&t, &ls).contains("Pilot") && !vocab_text(&t, &ls).contains("[contoso]"));
        assert_eq!(resolve(&t, &ls, "Identity").unwrap().layer, "northwind", "the client's own Identity, not contoso's");
        assert!(resolve(&t, &ls, "Budget").is_some_and(|x| x.layer == SHARED));
        // Terms are found only in their client's sections.
        let n = |id: &str| -> i64 { c.query_row("SELECT count(*) FROM chunk_topics WHERE category_id = ?1", [id], |r| r.get(0)).unwrap() };
        assert_eq!((n("term__northwind__okta"), n("term__contoso__okta")), (1, 1));
        // A client's view: the shared layer and its own.
        let v = view_in(Some(("client", "northwind"))).unwrap();
        let layers: BTreeSet<&str> = v["topics"].as_array().unwrap().iter().map(|x| x["layer"].as_str().unwrap()).collect();
        assert_eq!(layers, [SHARED, "northwind"].into());
        // Edits name their layer when a name is in two.
        assert!(edit(&json!({"op": "rename", "label": "Identity", "to": "Access"})).unwrap_err().to_string().contains("more than one layer"));
        c.execute("INSERT INTO chunk_topics VALUES (?1, 'northwind__identity', 'Identity', 'intent_l1')", [a]).unwrap();
        edit(&json!({"op": "rename", "label": "Identity", "to": "Access", "layer": "northwind"})).unwrap();
        assert_eq!(n("northwind__access"), 1, "the tag moved with the rename");
        assert!(view().unwrap()["topics"].as_array().unwrap().iter().any(|x| x["layer"] == "contoso" && x["label"] == "Identity"), "the other client's untouched");
        let t2 = edit(&json!({"op": "accept", "layer": "northwind"})).unwrap();
        assert!(!t2.layers["northwind"].provisional && t2.layers["contoso"].provisional);
        edit(&json!({"op": "add", "term": true, "name": "Foundry", "layer": "northwind", "threads": ["northwind-thread"]})).unwrap();
        let mut _p = crate::projects::create("NW thread", None, None).unwrap();
        assert!(terms_for_threads(&["northwind-thread".into()]).contains(&"Foundry".to_string()), "pinned");
    }

    fn t_echo_spellings(t: &Taxonomy) -> Vec<String> {
        t.terms.iter().find(|x| x.name == "Echo").map(|x| x.variants.clone()).unwrap_or_default()
    }

    #[test]
    fn an_old_taxonomy_reads_as_the_new_one() {
        let (_env, _root) = crate::paths::testenv::folders("brain-old-taxonomy");
        // As written before synonyms, groups and one tree: two clients' layers, spellings and other
        // names mixed in `variants`, free kinds, a learned term, the same product in both clients.
        let old = json!({"version": 7, "layers": {"shared": {"name": "Your practice", "built_from": 4}, "globex": {"name": "Globex", "built_from": 2}, "acme": {"name": "Acme", "built_from": 3}},
            "topics": [{"label": "Delivery", "level": 1, "description": "How work is delivered."}, {"layer": "globex", "label": "Data platform", "level": 1}, {"layer": "globex", "label": "Ingestion", "level": 2, "parent": "Data platform"}],
            "terms": [{"layer": "globex", "name": "Signal Grid", "kind": "platform", "variants": ["SG", "Signal Gird", "the grid layer"]},
                      {"name": "Echo", "kind": "product", "variants": ["Эко"]},
                      {"layer": "globex", "name": "Databricks", "kind": "product", "variants": ["Data bricks"]},
                      {"layer": "acme", "name": "Databricks", "kind": "tool"},
                      {"layer": "acme", "name": "Pathfinder", "kind": "learned", "learned": {"subjects": ["weekly"], "people": [], "calls": []}},
                      {"layer": "acme", "name": "SLA", "kind": "metric"}],
            "history": [{"op": "build", "layer": "globex", "version": 5}]});
        std::fs::create_dir_all(dir()).unwrap();
        std::fs::write(dir().join("current.json"), old.to_string()).unwrap();
        let t = current().unwrap();
        let sg = t.terms.iter().find(|x| x.name == "Signal Grid").unwrap();
        assert_eq!((sg.synonyms.clone(), sg.variants.clone()), (vec!["SG".to_string(), "the grid layer".to_string()], vec!["Signal Gird".to_string()]), "an abbreviation or a name nothing like it is another name; a near spelling stays a spelling");
        assert_eq!(sg.group(), "product");
        assert_eq!(t.terms.iter().find(|x| x.name == "SLA").unwrap().group(), "metric");
        assert_eq!(t.terms.iter().find(|x| x.name == "Pathfinder").unwrap().group(), "other", "learned is a source, not a group");
        assert!(t.topics.iter().all(|x| x.synonyms.is_empty()) && parent_of(&t, &t.topics[2]).is_some_and(|p| p.label == "Data platform"));
        // The view reads it: groups, other names, nothing lost.
        let v = view().unwrap();
        assert_eq!(v["terms"].as_array().unwrap().len(), 6);
        assert_eq!(t_echo_spellings(&current().unwrap()), vec!["Эко".to_string()], "a transcription's other script is a spelling");
        assert!(v["terms"].as_array().unwrap().iter().any(|x| x["name"] == "Signal Grid" && x["synonyms"][0] == "SG" && x["group"] == "product"));
        // The same product in two clients: one thing of the practice; done, it is one term.
        let m = moves().unwrap();
        let up = m.iter().find(|x| x["what"] == "term").unwrap();
        assert_eq!(up["label"], "Databricks");
        apply_move(up["key"].as_str().unwrap()).unwrap();
        let t = current().unwrap();
        let dbx: Vec<&Term> = t.terms.iter().filter(|x| x.name == "Databricks").collect();
        assert_eq!((dbx.len(), dbx[0].layer.as_str()), (1, SHARED));
        assert!(dbx[0].variants.contains(&"Data bricks".to_string()));
        // Its first save keeps the split: written as synonyms now, not again on the next load.
        let raw: Value = serde_json::from_slice(&std::fs::read(dir().join("current.json")).unwrap()).unwrap();
        assert!(raw["terms"].as_array().unwrap().iter().any(|x| x["name"] == "Signal Grid" && x["synonyms"][0] == "SG"));
        // Split a name off, rename (the old name kept), regroup.
        edit(&json!({"op": "split", "term": true, "name": "Signal Grid", "layer": "globex", "alias": "the grid layer"})).unwrap();
        edit(&json!({"op": "rename", "term": true, "name": "Signal Grid", "layer": "globex", "to": "SGrid"})).unwrap();
        let t = edit(&json!({"op": "set", "term": true, "name": "SGrid", "layer": "globex", "group": "initiative"})).unwrap();
        let k = t.terms.iter().find(|x| x.name == "SGrid").unwrap();
        assert!(k.synonyms.contains(&"Signal Grid".to_string()) && k.group() == "initiative");
        assert!(t.terms.iter().any(|x| x.name == "the grid layer" && x.layer == "globex"));
        assert!(edit(&json!({"op": "set", "term": true, "name": "SGrid", "layer": "globex", "group": "nonsense"})).is_err());
    }

    #[test]
    fn one_tree_with_topics_that_move_up_and_down() {
        let (_env, _root) = crate::paths::testenv::folders("brain-one-tree");
        let tp = |layer: &str, label: &str, level: u8, parent: Option<&str>| Topic { layer: layer.into(), label: label.into(), level, parent: parent.map(str::to_owned), ..Default::default() };
        // A rebuild keeps what it has: a new wording of an old topic (or one of its synonyms) is the old one.
        let old = vec![Topic { synonyms: vec!["Roadmap".into()], description: "Plans and dates.".into(), ..tp("globex", "Plan", 1, None) }, tp("globex", "Milestones", 2, Some("Plan"))];
        let mut new = vec![tp("globex", "Roadmap", 1, None), tp("globex", "Delivery plan", 2, Some("Roadmap"))];
        keep_identity(&old, &mut new);
        assert_eq!((new[0].label.as_str(), new[0].description.as_str()), ("Plan", "Plans and dates."));
        assert_eq!(new[1].parent.as_deref(), Some("Plan"), "children follow the label their parent kept");
        // A client's topic may sit under one of the practice's level-1 topics.
        let practice: BTreeSet<String> = [nid("Commercials")].into();
        let (ts, _) = parse_layer_under(&json!({"topics": [{"label": "Pricing", "level": 2, "parent": "Commercials", "synonyms": ["Rate card"]}]}), "globex", &practice).unwrap();
        assert_eq!((ts[0].level, ts[0].parent.as_deref(), ts[0].synonyms.len()), (2, Some("Commercials"), 1));
        let mut t = Taxonomy { topics: vec![tp(SHARED, "Commercials", 1, None), ts[0].clone(), tp("globex", "Plan", 1, None), tp("acme", "Plan", 1, None), tp(SHARED, "Data", 1, None)], ..Default::default() };
        for l in [SHARED, "globex", "acme"] {
            t.layers.insert(l.into(), Layer { name: l.to_uppercase(), ..Default::default() });
        }
        assert_eq!(parent_of(&t, &t.topics[1]).map(|p| p.layer.as_str()), Some(SHARED));
        // One vocabulary: the practice's tree with the client's topics placed in it, then the client's own.
        let v = vocab_text(&t, &[SHARED.into(), "globex".into()]);
        assert!(v.contains("- Commercials: \n  - Pricing (GLOBEX): ") && v.contains("[also: Rate card]") && v.contains("[GLOBEX only]\n- Plan (GLOBEX)") && !v.contains("ACME"), "{v}");
        assert_eq!(resolve(&t, &[SHARED.into(), "globex".into()], "Pricing (GLOBEX)").map(|x| x.label.as_str()), Some("Pricing"));
        assert_eq!(resolve(&t, &[SHARED.into(), "globex".into()], "rate card").map(|x| x.label.as_str()), Some("Pricing"), "a synonym names it");
        // Moves: two clients' "Plan" → up; the practice's "Data" only Globex tags → down.
        let c = store::open(&super::super::store_path()).unwrap();
        for i in 1..=12 {
            c.execute("INSERT INTO chunks(id, source, ord, status, title, text) VALUES (?1, 'call/g', ?1, 'ACTIVE', '', '')", [i]).unwrap();
            c.execute("INSERT INTO scopes(chunk_id, kind, id) VALUES (?1, 'client', 'globex')", [i]).unwrap();
            c.execute("INSERT INTO chunk_topics VALUES (?1, 'data', 'Data', 'intent_l1')", [i]).unwrap();
        }
        let m = moves_in(&stats_of(&c, None).unwrap(), &t, &BTreeSet::new());
        let up = m.iter().find(|x| x["dir"] == "up").unwrap();
        assert_eq!(up["topics"].as_array().unwrap().len(), 2);
        let down = m.iter().find(|x| x["dir"] == "down").unwrap();
        assert_eq!((down["label"].as_str(), down["to"].as_str()), (Some("Data"), Some("globex")));
        assert!(moves_in(&stats_of(&c, None).unwrap(), &t, &[down["key"].as_str().unwrap().to_string()].into()).iter().all(|x| x["dir"] != "down"), "kept where it is");
        // Done: up — one practice topic, the other merged in as a synonym of it; down — Globex's, its tags kept.
        save(t, json!({"op": "test"})).unwrap();
        apply_move(up["key"].as_str().unwrap()).unwrap();
        apply_move(down["key"].as_str().unwrap()).unwrap();
        let t = current().unwrap();
        assert!(t.topics.iter().any(|x| x.layer == SHARED && x.label == "Plan") && !t.topics.iter().any(|x| x.layer != SHARED && x.label == "Plan"));
        assert!(t.topics.iter().any(|x| x.layer == "globex" && x.label == "Data"));
        let n = |id: &str| -> i64 { c.query_row("SELECT count(*) FROM chunk_topics WHERE category_id = ?1", [id], |r| r.get(0)).unwrap() };
        assert_eq!(n("globex__data"), 12, "the tags followed it down");
        // A client's topic never tags another client's section.
        c.execute("INSERT INTO chunks(id, source, ord, status, title, text) VALUES (99, 'call/a', 99, 'ACTIVE', '', '')", []).unwrap();
        c.execute("INSERT INTO scopes(chunk_id, kind, id) VALUES (99, 'client', 'acme')", []).unwrap();
        c.execute("INSERT INTO chunk_topics VALUES (99, 'globex__data', 'Data', 'intent_l1')", []).unwrap();
        build_graph(&c, &t, None).unwrap();
        assert_eq!(n("globex__data"), 12);
        // Renamed: the old label stays a synonym.
        let t = edit(&json!({"op": "rename", "label": "Commercials", "layer": SHARED, "to": "Commercial terms"})).unwrap();
        assert!(t.topics.iter().any(|x| x.label == "Commercial terms" && x.synonyms == ["Commercials"]));
    }

    #[test]
    fn mentions_follow_edits_and_syncs_as_a_full_recount_would() {
        let (_env, _root) = crate::paths::testenv::folders("brain-mentions");
        let path = super::super::store_path();
        let doc = |source: &str, client: &str, md: &str| -> store::Input {
            let s = source.to_string();
            store::Input { source: s.clone(), kind: "call".into(), title: s.clone(), event_date: Some("2026-10-28".into()), md: md.into(),
                refs: json!({"threads": [{"id": format!("{client}-t"), "client": client}]}), link: Box::new(move |_| s.clone()), speaker: Box::new(|_| None) }
        };
        let docs = |four: &str| vec![
            doc("call/1", "acme", "## A\nThe merchant portal launch is on Friday.\n"),
            doc("call/2", "globex", "## B\nPilot desk review with the ops team.\n"),
            doc("call/3", "acme", "## C\nThe pilot desk and the (MERCHANT PORTAL) share a login.\n"),
            doc("call/4", "acme", four),
        ];
        store::sync(&path, docs("## D\nPortal merchants, not the product.\n"), None).unwrap();
        let term = |layer: &str, name: &str, variants: &[&str]| Term { layer: layer.into(), name: name.into(), kind: "product".into(), variants: variants.iter().map(|v| v.to_string()).collect(), ..Default::default() };
        let mut t = Taxonomy::default();
        for l in [SHARED, "acme", "globex"] {
            t.layers.insert(l.into(), Layer { name: l.into(), ..Default::default() });
        }
        t.terms = vec![term(SHARED, "Merchant portal", &[]), term("acme", "Pilot desk", &[]), term("globex", "Pilot desk", &[]), term(SHARED, "Ledger", &[])];
        save(t, json!({"op": "test"})).unwrap();
        let c = store::open(&path).unwrap();
        let tags = || -> BTreeSet<(String, String)> {
            c.prepare("SELECT ch.source, k.category_id FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE k.kind = 'term'").unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|x| x.unwrap()).collect()
        };
        let full = || -> BTreeSet<(String, String)> {
            let before = tags();
            mentions(&c, &current().unwrap()).unwrap();
            let all = tags();
            assert_eq!(before, all, "what was kept up as it went = a full recount");
            all
        };
        let s = |a: &str, b: &str| (a.to_string(), b.to_string());
        // Whole words, any case and punctuation; a client's term only in its client's sections.
        assert_eq!(full(), [s("call/1", "term__merchant_portal"), s("call/3", "term__merchant_portal"), s("call/3", "term__acme__pilot_desk"), s("call/2", "term__globex__pilot_desk")].into());
        // An edit: a new spelling finds more; only that term is read again.
        edit(&json!({"op": "set", "term": true, "name": "Ledger", "layer": SHARED, "variants": ["login"]})).unwrap();
        assert!(full().contains(&s("call/3", "term__ledger")));
        // A merge: the merged term's mentions are the other's now.
        edit(&json!({"op": "merge", "term": true, "name": "Ledger", "layer": SHARED, "into": "Merchant portal"})).unwrap();
        let after = full();
        assert!(!after.iter().any(|x| x.1 == "term__ledger") && after.contains(&s("call/3", "term__merchant_portal")));
        // A sync that changes one section: only it is read again.
        store::sync(&path, docs("## D\nThe merchant portal again.\n"), None).unwrap();
        assert_eq!(mentions_changed(&c, &current().unwrap()).unwrap(), 1, "only the changed section");
        assert!(full().contains(&s("call/4", "term__merchant_portal")));
        // A document written again (no embedder: every section's tags were dropped) whose first section didn't
        // change: it keeps its mentions.
        let two = |second: &str| doc("call/5", "acme", &format!("## E\nThe merchant portal pilot desk.\n## F\n{second}\n"));
        let mut all = docs("## D\nThe merchant portal again.\n");
        all.push(two("First text."));
        store::sync(&path, all, None).unwrap();
        mentions_changed(&c, &current().unwrap()).unwrap();
        let mut all = docs("## D\nThe merchant portal again.\n");
        all.push(two("Second text."));
        store::sync(&path, all, None).unwrap();
        mentions_changed(&c, &current().unwrap()).unwrap();
        assert!(full().contains(&s("call/5", "term__merchant_portal")), "the unchanged section kept its mention");
        // Moved to another client's thread, text unchanged: its client's terms follow.
        store::rescope(&path, "call/2", &json!({"threads": [{"id": "acme-t", "client": "acme"}]})).unwrap();
        mentions_changed(&c, &current().unwrap()).unwrap();
        let moved = full();
        assert!(moved.contains(&s("call/2", "term__acme__pilot_desk")) && !moved.contains(&s("call/2", "term__globex__pilot_desk")));
        // A term removed takes its mentions with it.
        store::rescope(&path, "call/2", &json!({"threads": [{"id": "globex-t", "client": "globex"}]})).unwrap();
        mentions_changed(&c, &current().unwrap()).unwrap();
        edit(&json!({"op": "remove", "term": true, "name": "Pilot desk", "layer": "globex"})).unwrap();
        assert!(!full().iter().any(|x| x.1 == "term__globex__pilot_desk"));
        // An edit that stopped half way (its version saved, its mentions not): the next sync's step redoes them.
        let good = full();
        assert_eq!(mentions_after_sync(&c).unwrap(), 0, "in step: nothing to read");
        c.execute("DELETE FROM chunk_topics WHERE category_id = 'term__merchant_portal'", []).unwrap();
        store::set_meta(&c, "mentions_for", "an older version").unwrap();
        mentions_after_sync(&c).unwrap();
        assert_eq!(full(), good);
    }

    #[test]
    fn a_layer_drifts_when_its_topics_stop_fitting() {
        let (_env, _root) = crate::paths::testenv::folders("brain-drift");
        let c = store::open(&super::super::store_path()).unwrap();
        let mut id = 0;
        // A section of a client, judged in a taxonomy version, with its level-1 topic when tagged.
        let mut add = |client: &str, source: &str, verdict: &str, version: u32, topic: Option<&str>| {
            id += 1;
            c.execute("INSERT INTO chunks(id, source, ord, status) VALUES (?1, ?2, ?1, 'ACTIVE')", params![id, source]).unwrap();
            c.execute("INSERT INTO scopes(chunk_id, kind, id) VALUES (?1, 'client', ?2)", params![id, client]).unwrap();
            c.execute("INSERT INTO chunk_verdicts(chunk_id, verdict, taxonomy_version) VALUES (?1, ?2, ?3)", params![id, verdict, version.to_string()]).unwrap();
            if let Some(t) = topic {
                c.execute("INSERT INTO chunk_topics VALUES (?1, ?2, 'x', 'intent_l1')", params![id, t]).unwrap();
            }
        };
        // Globex: built in version 3 from 2 calls. Before it, 50 sections fitting nothing (not counted);
        // since, 40: 6 fit nothing (15%), 20 of the 34 tagged went to one topic (59%), over 7 calls.
        for i in 0..50 {
            add("globex", &format!("call/old{}", i % 2), "untagged", 1, None);
        }
        for i in 0..40 {
            let (v, t) = if i < 6 { ("untagged", None) } else if i < 26 { ("tagged", Some("globex__plan")) } else { ("tagged", Some("globex__data")) };
            add("globex", &format!("call/new{}", i % 7), v, 3, t);
        }
        // Acme: 20 sections fitting nothing — too few to judge — from the 2 calls it was built from.
        for i in 0..20 {
            add("acme", &format!("call/acme{}", i % 2), "untagged", 4, None);
        }
        let mut t = Taxonomy::default();
        t.layers.insert("globex".into(), Layer { name: "Globex".into(), built_from: 2, ..Default::default() });
        t.layers.insert("acme".into(), Layer { name: "Acme".into(), built_from: 2, ..Default::default() });
        t.topics.push(Topic { layer: "globex".into(), label: "Plan".into(), level: 1, ..Default::default() });
        t.history = vec![json!({"op": "build", "layer": "globex", "version": 3}), json!({"op": "build", "layer": "acme", "version": 4})];
        let d = drift_in(&c, &t, &json!({})).unwrap();
        let of = |l: &str| d.iter().find(|x| x["layer"] == l).unwrap().clone();
        let g = of("globex");
        assert_eq!((g["sections"].as_i64(), g["untagged"].as_i64(), g["calls"].as_i64()), (Some(40), Some(6), Some(9)), "{g}");
        let rules: Vec<&str> = g["reasons"].as_array().unwrap().iter().map(|r| r["rule"].as_str().unwrap()).collect();
        // Built from 2 calls: how its sections fit isn't judged (#18), only that it outgrew them.
        assert_eq!(rules, ["outgrown"], "{g}");
        let mut t6 = t.clone();
        t6.layers.get_mut("globex").unwrap().built_from = 0; // unknown: judged
        let g0 = drift_in(&c, &t6, &json!({})).unwrap().into_iter().find(|x| x["layer"] == "globex").unwrap();
        let rules: Vec<&str> = g0["reasons"].as_array().unwrap().iter().map(|r| r["rule"].as_str().unwrap()).collect();
        assert_eq!(rules, ["nothing_fits", "catch_all"], "{g0}");
        assert_eq!((g["stale"].as_bool(), g["top"]["label"].as_str()), (Some(true), Some("Plan")));
        let a = of("acme");
        assert_eq!((a["stale"].as_bool(), a["reasons"].as_array().unwrap().len()), (Some(false), 0), "under 30 sections: not judged; 2 calls of 2");
        // Not now: held until its sections (or calls) grow by half.
        let snoozed = json!({"globex": {"sections": 40, "calls": 9}});
        let g = drift_in(&c, &t, &snoozed).unwrap().into_iter().find(|x| x["layer"] == "globex").unwrap();
        assert_eq!((g["stale"].as_bool(), g["snoozed"].as_bool()), (Some(false), Some(true)));
        for i in 0..20 {
            add("globex", &format!("call/new{}", i % 7), "untagged", 3, None);
        }
        assert_eq!(drift_in(&c, &t, &snoozed).unwrap().into_iter().find(|x| x["layer"] == "globex").unwrap()["stale"], true, "60 sections: half again");
    }

    #[test]
    fn a_rebuild_isnt_judged_on_its_own_sections() {
        let (_env, _root) = crate::paths::testenv::folders("brain-drift-rebuild");
        let c = store::open(&super::super::store_path()).unwrap();
        let mut id = 0;
        // A section of a client ("-": the practice), indexed at `created`, tagged in version 5 with a
        // topic (and its level-1 parent, as `classify` does), or fitting nothing.
        let mut add = |client: &str, source: &str, created: &str, l1: Option<&str>, l2: Option<&str>| {
            id += 1;
            c.execute("INSERT INTO chunks(id, source, ord, status, created_at) VALUES (?1, ?2, ?1, 'ACTIVE', ?3)", params![id, source, created]).unwrap();
            c.execute("INSERT INTO scopes(chunk_id, kind, id) VALUES (?1, 'client', ?2)", params![id, client]).unwrap();
            c.execute("INSERT INTO chunk_verdicts(chunk_id, verdict, taxonomy_version) VALUES (?1, ?2, '5')", params![id, if l1.is_some() { "tagged" } else { "untagged" }]).unwrap();
            for (t, kind) in [(l1, "intent_l1"), (l2, "intent_l2")] {
                if let Some(t) = t {
                    c.execute("INSERT INTO chunk_topics VALUES (?1, ?2, 'x', ?3)", params![id, t, kind]).unwrap();
                }
            }
        };
        let (before, after) = ("2026-10-20T10:00:00+00:00", "2026-11-02T10:00:00+00:00");
        let topic = |layer: &str, label: &str, parent: Option<&str>| Topic { layer: layer.into(), label: label.into(), level: if parent.is_some() { 2 } else { 1 }, parent: parent.map(str::to_owned), ..Default::default() };
        let mut t = Taxonomy::default();
        t.layers.insert(SHARED.into(), Layer { name: "Your practice".into(), built_from: 3, ..Default::default() });
        t.topics = vec![topic(SHARED, "A", None), topic(SHARED, "A1", Some("A")), topic(SHARED, "A2", Some("A")), topic(SHARED, "A3", Some("A")), topic(SHARED, "B", None), topic(SHARED, "C", None)];
        t.history = vec![json!({"op": "build", "layer": SHARED, "version": 5, "at": "2026-11-01T10:00:00+00:00"}), json!({"op": "accept", "version": 6})];
        let (a, b, cc) = (topic_id(SHARED, "A"), topic_id(SHARED, "B"), topic_id(SHARED, "C"));
        let sub = |i: usize| topic_id(SHARED, &format!("A{}", i % 3 + 1));
        // Built from 3 calls with 66 sections, re-tagged by the rebuild: 38 (58%) under A.
        for i in 0..66 {
            let (l1, l2) = if i < 38 { (Some(a.as_str()), Some(sub(i))) } else if i < 52 { (Some(b.as_str()), None) } else { (Some(cc.as_str()), None) };
            add("-", &format!("call/p{}", i % 3), before, l1, l2.as_deref());
        }
        let shared = |t: &Taxonomy| drift_in(&c, t, &json!({})).unwrap().into_iter().find(|x| x["layer"] == SHARED).unwrap();
        let s = shared(&t);
        assert_eq!((s["stale"].as_bool(), s["sections"].as_i64()), (Some(false), Some(0)), "the build's own sections: {s}");
        // 40 sections from 4 new calls, 70% under A, split between its subtopics.
        for i in 0..40 {
            let (l1, l2) = if i < 28 { (Some(a.as_str()), Some(sub(i))) } else { (Some(b.as_str()), None) };
            add("-", &format!("call/n{}", i % 4), after, l1, l2.as_deref());
        }
        let s = shared(&t);
        assert_eq!((s["stale"].as_bool(), s["sections"].as_i64(), s["calls"].as_i64()), (Some(false), Some(40), Some(7)), "{s}");
        assert_eq!(s["top"]["label"], "A");
        // Built from 6 calls, so judged: A's subtopics split it (none over 40%)…
        let mut t6 = t.clone();
        t6.layers.get_mut(SHARED).unwrap().built_from = 6;
        assert_eq!(shared(&t6)["stale"], false, "{}", shared(&t6));
        // …until one subtopic takes most of them.
        for i in 0..30 {
            add("-", &format!("call/n{}", i % 4), after, Some(&a), Some(&topic_id(SHARED, "A1")));
        }
        let s = shared(&t6);
        assert_eq!(s["stale"], true, "{s}");
        assert!(s["reasons"][0]["text"].as_str().unwrap().contains("“A1”"), "{s}");
        // It names the topic to split (#31).
        assert_eq!(s["reasons"][0]["topic"], json!({"layer": SHARED, "label": "A1"}), "{s}");
        // Sections tagged with a level-1 topic alone, none of its subtopics, are a catch-all too.
        t.layers.insert("beta".into(), Layer { name: "Beta".into(), built_from: 6, ..Default::default() });
        t.topics.extend([topic("beta", "P", None), topic("beta", "P1", Some("P")), topic("beta", "P2", Some("P")), topic("beta", "Q", None)]);
        t.history.push(json!({"op": "build", "layer": "beta", "version": 5, "at": "2026-11-01T10:00:00+00:00"}));
        let (pp, q) = (topic_id("beta", "P"), topic_id("beta", "Q"));
        for i in 0..40 {
            let (l1, l2) = if i < 28 { (&pp, None) } else if i < 32 { (&pp, Some(topic_id("beta", "P1"))) } else { (&q, None) };
            add("beta", &format!("call/b{}", i % 6), after, Some(l1), l2.as_deref());
        }
        let d = drift_in(&c, &t, &json!({})).unwrap().into_iter().find(|x| x["layer"] == "beta").unwrap();
        assert_eq!(d["stale"], true, "{d}");
        assert!(d["reasons"][0]["text"].as_str().unwrap().contains("P (in none of its subtopics)"), "{d}");
        // A client built from 10 calls: half its new sections under one topic with no subtopics.
        t.layers.insert("acme".into(), Layer { name: "Acme".into(), built_from: 10, ..Default::default() });
        t.topics.extend([topic("acme", "X", None), topic("acme", "Y", None), topic("acme", "Z", None)]);
        t.history.push(json!({"op": "build", "layer": "acme", "version": 5, "at": "2026-11-01T10:00:00+00:00"}));
        let (x, y, z) = (topic_id("acme", "X"), topic_id("acme", "Y"), topic_id("acme", "Z"));
        for i in 0..40 {
            let l1 = if i < 20 { &x } else if i < 30 { &y } else { &z };
            add("acme", &format!("call/a{}", i % 10), after, Some(l1), None);
        }
        let d = drift_in(&c, &t, &json!({})).unwrap().into_iter().find(|x| x["layer"] == "acme").unwrap();
        let rules: Vec<&str> = d["reasons"].as_array().unwrap().iter().map(|r| r["rule"].as_str().unwrap()).collect();
        assert_eq!((d["stale"].as_bool(), rules), (Some(true), vec!["catch_all"]), "{d}");
    }

    #[test]
    fn a_client_build_never_repeats_the_practice() {
        let (_env, _root) = crate::paths::testenv::folders("taxonomy-fold");
        let topic = |layer: &str, label: &str, parent: Option<&str>| Topic { layer: layer.into(), label: label.into(), level: if parent.is_some() { 2 } else { 1 }, parent: parent.map(str::to_owned), ..Default::default() };
        let mut t = Taxonomy {
            topics: vec![
            Topic { synonyms: vec!["Proposals".into()], ..topic(SHARED, "Offerings and proposals", None) },
            topic(SHARED, "Agentic delivery", None),
            topic(SHARED, "Evaluations", Some("Agentic delivery")),
            // What a client build came back with: a practice level-1 label again (#33), one by a synonym,
            // one as a level-2 topic of the practice, and one of its own.
            topic("acme", "Offerings and proposals", None),
            topic("acme", "Acme RFP answers", Some("Offerings and proposals")),
            topic("acme", "Proposals", None),
            topic("acme", "Evaluation", None),
            topic("acme", "Rollout waves", None),
        ],
            terms: vec![
            Term { layer: SHARED.into(), name: "Widgetfactory".into(), synonyms: vec!["WF".into()], ..Default::default() },
            Term { layer: "acme".into(), name: "WF".into(), threads: vec!["pilot".into()], variants: vec!["Double-U Eff".into()], ..Default::default() },
            Term { layer: "acme".into(), name: "Acme portal".into(), ..Default::default() },
        ],
            ..Default::default()
        };
        let folded = fold_into_practice(&mut t, "acme");
        let acme: Vec<(&str, u8, Option<&str>)> = t.topics.iter().filter(|x| x.layer == "acme").map(|x| (x.label.as_str(), x.level, x.parent.as_deref())).collect();
        assert_eq!(acme, vec![("Acme RFP answers", 2, Some("Offerings and proposals")), ("Rollout waves", 1, None)], "{folded:?}");
        assert_eq!(t.aliases[&topic_id("acme", "Evaluation")], topic_id(SHARED, "Evaluations"));
        assert_eq!(t.aliases[&topic_id("acme", "Proposals")], topic_id(SHARED, "Offerings and proposals"));
        // A term moved up earlier (#32): the client's copy goes, its pins go to the practice's term.
        assert_eq!(t.terms.iter().filter(|x| x.layer == "acme").map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["Acme portal"]);
        assert_eq!(t.terms[0].threads, vec!["pilot".to_string()]);
        assert!(t.terms[0].variants.contains(&"Double-U Eff".to_string()), "its spellings stay transcription hints");
        assert_eq!(t.aliases[&term_id("acme", "WF")], term_id(SHARED, "Widgetfactory"));
        // The shared layer never folds into itself.
        assert!(fold_into_practice(&mut t, SHARED).is_empty());
    }

    #[test]
    fn aliases_never_chain_or_loop() {
        let mut a = BTreeMap::new();
        // Folded into the practice's x, then y merged into x too.
        set_alias(&mut a, "acme__x".into(), "x".into());
        set_alias(&mut a, "y".into(), "x".into());
        assert_eq!((a["acme__x"].as_str(), a["y"].as_str()), ("x", "x"));
        // Moved back down: x is acme's again — nothing points in a circle.
        set_alias(&mut a, "x".into(), "acme__x".into());
        assert!(!a.contains_key("acme__x"), "{a:?}");
        assert_eq!((a["x"].as_str(), a["y"].as_str()), ("acme__x", "acme__x"));
    }

    #[test]
    fn a_split_makes_subtopics_under_the_topic_or_its_parent() {
        let topic = |label: &str, parent: Option<&str>| Topic { layer: "acme".into(), label: label.into(), level: if parent.is_some() { 2 } else { 1 }, parent: parent.map(str::to_owned), ..Default::default() };
        let t = Taxonomy { topics: vec![topic("Presales staffing", None), topic("Rollout", None), topic("Wave planning", Some("Rollout"))], ..Default::default() };
        let v = json!({"subtopics": [{"label": "Expert requests", "description": "who is asked for"}, {"label": "Presales staffing"}, {"label": "Rollout"},
                                     {"label": "Bench availability", "synonyms": ["bench", "Bench availability"]}, {"label": "expert requests"}]});
        let subs = subtopics_of(&t, &t.topics[0].clone(), &v).unwrap();
        assert_eq!(subs.iter().map(|x| (x.label.as_str(), x.parent.as_deref())).collect::<Vec<_>>(), vec![("Expert requests", Some("Presales staffing")), ("Bench availability", Some("Presales staffing"))]);
        assert_eq!(subs[1].synonyms, vec!["bench".to_string()]);
        // A level-2 topic's subtopics sit beside it, under its parent.
        let subs = subtopics_of(&t, &t.topics[2].clone(), &json!({"subtopics": [{"label": "Wave 1"}, {"label": "Wave 2"}]})).unwrap();
        assert!(subs.iter().all(|x| x.parent.as_deref() == Some("Rollout") && x.level == 2));
        assert!(subtopics_of(&t, &t.topics[0].clone(), &json!({"subtopics": [{"label": "Only one"}]})).is_err(), "one isn't a split");
    }

    #[test]
    fn a_client_topic_the_practice_already_has_is_offered_up() {
        let (_env, _root) = crate::paths::testenv::folders("taxonomy-fold-moves");
        let c = store::open(&super::super::store_path()).unwrap();
        let mut t = Taxonomy::default();
        t.layers.insert("acme".into(), Layer { name: "Acme".into(), ..Default::default() });
        t.topics = vec![Topic { layer: SHARED.into(), label: "Agentic delivery".into(), level: 1, ..Default::default() }, Topic { layer: "acme".into(), label: "Agentic delivery".into(), level: 1, ..Default::default() }];
        let m = moves_in(&stats_of(&c, None).unwrap(), &t, &BTreeSet::new());
        let up = m.iter().find(|x| x["dir"] == "up").expect("offered");
        assert!(up["why"].as_str().unwrap().starts_with("your practice has it already"), "{up}");
    }

    #[test]
    fn an_old_single_list_reads_as_the_shared_layer() {
        let t: Taxonomy = serde_json::from_value(json!({"version": 1, "provisional": true, "goal": "g", "topics": [{"label": "Budget", "level": 1}], "terms": []})).unwrap();
        assert_eq!(t.topics[0].layer, SHARED);
        assert!(t.provisional && t.layers.is_empty());
    }
}

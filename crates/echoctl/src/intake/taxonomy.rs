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

mod mentions;
mod view;
pub use mentions::*;
pub use view::*;

/// The layer of the user's own practice (calls with no client).
pub const SHARED: &str = "shared";

/// Brain's SequenceMatcher-like ratio on normalised names: 2·matches / total, matches from the
/// longest common subsequence of characters.
fn ratio(a: &str, b: &str) -> f64 {
    let (a, b): (Vec<char>, Vec<char>) = (nid(a).chars().collect(), nid(b).chars().collect());
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            dp[i][j] = if a[i - 1] == b[j - 1] { dp[i - 1][j - 1] + 1 } else { dp[i - 1][j].max(dp[i][j - 1]) };
        }
    }
    2.0 * dp[a.len()][b.len()] as f64 / (a.len() + b.len()) as f64
}

/// The shared layer's goal: the user's own practice (their threads with no client).
pub(super) fn goal() -> String {
    "What one person discusses across their own practice: methods, tools, offerings, internal work — not any one client's matters.".to_string()
}

/// A layer's name, goal and audience: the shared one's, or a client's from its name, description and threads.
pub(super) fn layer_facts(layer: &str) -> (String, String, String) {
    if layer == SHARED {
        return ("Your practice".into(), goal(), "the user".into());
    }
    let c = crate::projects::clients().unwrap_or_default().into_iter().find(|c| c.id == layer);
    let name = c.as_ref().map(|c| c.name.clone()).unwrap_or_else(|| layer.to_string());
    let brief = crate::projects::client_brief(layer).unwrap_or_default();
    let threads: Vec<String> = crate::projects::list().into_iter().filter(|p| p.client.as_deref() == Some(layer)).map(|p| p.name).collect();
    let goal = format!(
        "What is said with and about {name}{}{}: their goals, decisions, commitments, risks, systems and people.",
        if threads.is_empty() { String::new() } else { format!(" in the engagements {}", threads.join(", ")) },
        if brief.trim().is_empty() { String::new() } else { format!(". About them: {}", brief.trim().chars().take(600).collect::<String>()) }
    );
    (name.clone(), goal, format!("the user's team working with {name}"))
}

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
    fn ids_names_and_clusters() {
        assert_eq!(nid("AI Agent Hub — MVP"), "ai_agent_hub_mvp");
        assert_eq!(nid("Москва Сити"), "москва_сити");
        assert_eq!(topic_id(SHARED, "Budget"), "budget");
        assert_eq!(topic_id("globex", "Budget"), "globex__budget", "the same name in a client is another node");
        assert_eq!(term_id("acme", "Dial9"), "term__acme__dial9");
        assert!(ratio("Agent Hub", "agent-hub") > 0.99);
        assert!(word_in("we use SSO here", "SSO") && !word_in("ssoft", "sso") && word_in("Пилот в Берлине", "берлине"));
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
        // Its first save keeps the split: written as synonyms now, not again on the next load.
        save(current().unwrap(), json!({"op": "test"})).unwrap();
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
    fn a_clients_topic_never_tags_another_clients_section() {
        let (_env, _root) = crate::paths::testenv::folders("brain-one-tree");
        let tp = |layer: &str, label: &str| Topic { layer: layer.into(), label: label.into(), level: 1, ..Default::default() };
        let mut t = Taxonomy { topics: vec![tp(SHARED, "Commercials"), tp("globex", "Data")], ..Default::default() };
        for l in [SHARED, "globex", "acme"] {
            t.layers.insert(l.into(), Layer { name: l.to_uppercase(), ..Default::default() });
        }
        save(t, json!({"op": "test"})).unwrap();
        let t = current().unwrap();
        let c = store::open(&super::super::store_path()).unwrap();
        let n = |id: &str| -> i64 { c.query_row("SELECT count(*) FROM chunk_topics WHERE category_id = ?1", [id], |r| r.get(0)).unwrap() };
        for (i, client) in [(1, "globex"), (99, "acme")] {
            c.execute("INSERT INTO chunks(id, source, ord, status, title, text) VALUES (?1, 'call/x', ?1, 'ACTIVE', '', '')", [i]).unwrap();
            c.execute("INSERT INTO scopes(chunk_id, kind, id) VALUES (?1, 'client', ?2)", params![i, client]).unwrap();
            c.execute("INSERT INTO chunk_topics VALUES (?1, 'globex__data', 'Data', 'intent_l1')", [i]).unwrap();
        }
        build_graph(&c, &t, None).unwrap();
        assert_eq!(n("globex__data"), 1, "acme's section lost the tag");
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
            // A name only in the minutes' header line, which the full-text index leaves out.
            doc("call/6", "acme", "## G\n**Who:** [Kim Login](echo://person/kim@acme.example), Sam Rivera\nNothing else was said.\n"),
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
        let edited = full();
        assert!(edited.contains(&s("call/3", "term__ledger")) && edited.contains(&s("call/6", "term__ledger")), "{edited:?}");
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
    fn an_old_single_list_reads_as_the_shared_layer() {
        let t: Taxonomy = serde_json::from_value(json!({"version": 1, "provisional": true, "goal": "g", "topics": [{"label": "Budget", "level": 1}], "terms": []})).unwrap();
        assert_eq!(t.topics[0].layer, SHARED);
        assert!(t.provisional && t.layers.is_empty());
    }
}

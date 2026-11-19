//! The dictionary at scale, for the window (Settings → Vocabulary): by group and letter, a page at a time. The
//! taxonomy's view is kept until the store, the taxonomy or the threads change (`fingerprint`).

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use super::store;

/// What changes when the store, the taxonomy or the threads change: the key under which costly answers are kept.
pub fn fingerprint() -> String {
    let stamp = |p: &std::path::Path| std::fs::metadata(p).map(|m| format!("{}:{:?};", m.len(), m.modified().ok())).unwrap_or_default();
    let db = super::store_path();
    let mut out = store::changed_stamp(&db);
    // Intake on or off: the dictionary's counts come from the index only with it on (`taxonomy::view_in`).
    out += if super::enabled() { "on;" } else { "off;" };
    // The taxonomy's folder: a new version, a dismissed move or a snoozed rebuild is written by a rename into it.
    out += &stamp(&db.with_file_name("taxonomy"));
    // The threads (a stage closes one: archived) and the clients.
    let projects = crate::paths::home().join("projects");
    out += &stamp(&projects.join("clients.json"));
    if let Ok(rd) = std::fs::read_dir(&projects) {
        for e in rd.flatten() {
            out += &stamp(&e.path().join("profile.json"));
        }
    }
    out
}

static CACHE: Mutex<Option<HashMap<String, (String, Value)>>> = Mutex::new(None);

/// `make()` once per `key` until the store changes.
pub fn cached(key: &str, make: impl FnOnce() -> Result<Value>) -> Result<Value> {
    let fp = fingerprint();
    if let Some((f, v)) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).get(key) {
        if *f == fp {
            return Ok(v.clone());
        }
    }
    // Built from a stale count (`store::kept`): given, never kept — the next call builds it again from the fresh one.
    let (v, stale) = store::noting_stale(make);
    let v = v?;
    if stale {
        return Ok(v);
    }
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let m = g.get_or_insert_with(HashMap::new);
    if m.len() > 24 {
        m.clear();
    }
    m.insert(key.to_string(), (fp, v.clone()));
    Ok(v)
}

/// The taxonomy as a scope sees it (`taxonomy::view_in`), kept until the store changes: the dictionary's pages and
/// its panel read the same one.
pub fn view(scope: store::Scope) -> Result<Value> {
    cached(&format!("view|{scope:?}"), || super::taxonomy::view_in(scope))
}

/// The dictionary a piece at a time: group counts, the letters of the group asked for, and a page of its terms
/// (A–Z, from `letter` on; or every term matching `q` by any of its names).
pub fn terms(scope: store::Scope, group: Option<&str>, letter: Option<&str>, q: Option<&str>, offset: usize, limit: usize) -> Result<Value> {
    let all = view(scope)?["terms"].clone();
    let terms: Vec<&Value> = all.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let current: Vec<&&Value> = terms.iter().filter(|t| t["archived"].is_null()).collect();
    let mut groups: BTreeMap<String, i64> = BTreeMap::new();
    for t in &current {
        *groups.entry(t["group"].as_str().unwrap_or("other").to_string()).or_default() += 1;
    }
    let closed = terms.len() - current.len();
    let words: Vec<String> = q.unwrap_or("").to_lowercase().split_whitespace().map(str::to_owned).collect();
    let hit = |t: &Value| {
        let hay = [&t["name"], &t["description"], &t["kind"]].into_iter().chain(t["synonyms"].as_array().into_iter().flatten()).chain(t["variants"].as_array().into_iter().flatten())
            .filter_map(Value::as_str).collect::<Vec<_>>().join(" ").to_lowercase();
        words.iter().all(|w| hay.contains(w.as_str()))
    };
    let mut pool: Vec<&Value> = match group {
        _ if !words.is_empty() => terms.iter().copied().filter(|t| hit(t)).collect(),
        Some("closed") => terms.iter().copied().filter(|t| !t["archived"].is_null()).collect(),
        Some(g) => current.iter().map(|t| **t).filter(|t| t["group"].as_str().unwrap_or("other") == g).collect(),
        None => vec![],
    };
    pool.sort_by_key(|t| t["name"].as_str().unwrap_or("").to_lowercase());
    let first = |t: &Value| t["name"].as_str().and_then(|n| n.chars().next()).map(|ch| if ch.is_alphabetic() { ch.to_uppercase().to_string() } else { "#".into() }).unwrap_or_default();
    let mut letters: BTreeMap<String, i64> = BTreeMap::new();
    for t in &pool {
        *letters.entry(first(t)).or_default() += 1;
    }
    let start = letter.map(|l| pool.iter().position(|t| first(t).as_str() >= l).unwrap_or(pool.len())).unwrap_or(0) + offset;
    let limit = limit.clamp(1, 200);
    let items: Vec<Value> = pool.iter().skip(start).take(limit).map(|t| (*t).clone()).collect();
    Ok(json!({"total": terms.len(), "current": current.len(), "closed": closed, "groups": groups, "letters": letters, "matches": pool.len(),
              "offset": start, "items": items, "more": pool.len().saturating_sub(start + limit)}))
}

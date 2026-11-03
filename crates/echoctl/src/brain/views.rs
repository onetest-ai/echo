//! What the window shows of the Brain: its overview (what it holds, its scopes, the taxonomy), the
//! graph (documents clustered around their topics, related documents linked), and a call's "Also
//! discussed" (the nearest moments of other documents, by moment). No model: all from the store.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::store::{self, Scope};

/// A document's link: a call opens in Echo; other sources carry their own link on their sections.
pub(crate) fn doc_link(source: &str, refs: &Value) -> Option<String> {
    if source.starts_with("call/") {
        return refs["call"].as_str().map(|c| format!("echo://call/{c}"));
    }
    // A channel post or a mail: its own link, in Teams / Outlook.
    refs["url"].as_str().filter(|u| u.starts_with("https://")).map(str::to_owned)
}

/// What the Brain holds and its topics and terms — never its documents: the Sources tab pages them
/// (`browse::source_groups`, `browse::source_list`), tens of thousands of them.
pub fn overview(scope: Scope) -> Result<Value> {
    let path = super::store_path();
    let mut tx = super::browse::view(scope)?;
    // Terms come a page at a time (`browse::terms`); a thread's Profile gets its own 30 (pinned here, then mentioned).
    let terms = tx.get_mut("terms").map(Value::take).unwrap_or(json!([]));
    let all: Vec<Value> = terms.as_array().cloned().unwrap_or_default();
    tx["terms_n"] = json!(all.len());
    if let Some(("thread", id)) = scope {
        let pinned = |x: &Value| x["pinned"].as_array().into_iter().flatten().any(|t| t == id);
        let mut mine: Vec<Value> = all.into_iter().filter(|x| x["sections"].as_i64().unwrap_or(0) > 0 || pinned(x)).collect();
        mine.sort_by_key(|x| std::cmp::Reverse((pinned(x), x["layer"] != "shared", x["sections"].as_i64().unwrap_or(0))));
        let mut seen = BTreeSet::new();
        mine.retain(|x| seen.insert(x["name"].as_str().unwrap_or("").to_lowercase()));
        tx["terms_more"] = json!(mine.len().saturating_sub(30));
        mine.truncate(30);
        tx["terms"] = json!(mine);
    }
    Ok(json!({
        "status": super::status()?,
        "scopes": super::browse::cached("scopes", || store::scopes(&path))?,
        "taxonomy": tx,
        "scope": scope.map(|s| json!({"kind": s.0, "id": s.1})),
    }))
}

/// "Also discussed" for a call: for each of its sections, the nearest sections of other documents
/// (Brain's `related`), each shown once, under the moment it is nearest to. The client decides:
/// the same client's from 0.6, another client's only from 0.75 (generic sections — "On screen",
/// "Minutes" — look alike across clients).
pub fn also(call: &str) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let dir = crate::paths::calls_dir().join(call);
    let src = format!("call/{}", crate::calls::stamp_of(&dir));
    let anchors: Vec<(i64, String, Option<String>)> = c
        .prepare("SELECT id, title, link FROM chunks WHERE source = ?1 ORDER BY ord")?
        .query_map([&src], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let titles: BTreeMap<String, (Option<String>, Option<String>)> = c
        .prepare("SELECT source, title, event_date FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<Result<_, _>>()?;
    let clients_of = |src: &str| -> BTreeSet<String> {
        c.prepare("SELECT DISTINCT s.id FROM scopes s JOIN chunks ch ON ch.id = s.chunk_id WHERE ch.source = ?1 AND s.kind = 'client' AND s.id != '-'")
            .and_then(|mut st| st.query_map([src], |r| r.get::<_, String>(0)).and_then(|rows| rows.collect()))
            .unwrap_or_default()
    };
    let mine = clients_of(&src);
    let mut theirs: BTreeMap<String, bool> = BTreeMap::new();
    let mut best: BTreeMap<i64, (f64, usize)> = BTreeMap::new();
    let mut st = c.prepare("SELECT r.related_id, r.score, o.source FROM related r JOIN chunks o ON o.id = r.related_id WHERE r.chunk_id = ?1 AND o.source != ?2 AND r.score >= 0.6")?;
    for (i, (id, ..)) in anchors.iter().enumerate() {
        for row in st.query_map(params![id, src], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?, r.get::<_, String>(2)?)))? {
            let (o, s, osrc) = row?;
            let same = *theirs.entry(osrc.clone()).or_insert_with(|| !mine.is_disjoint(&clients_of(&osrc)));
            if !same && s < 0.75 {
                continue;
            }
            if best.get(&o).is_none_or(|b| s > b.0) {
                best.insert(o, (s, i));
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<Value>> = BTreeMap::new();
    for (o, (s, i)) in best {
        let v = c.query_row("SELECT source, title, text, link, speaker FROM chunks WHERE id = ?1", [o], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?))
        })?;
        let (dt, dd) = titles.get(&v.0).cloned().unwrap_or_default();
        groups.entry(i).or_default().push(json!({"chunk_id": o.to_string(), "source": v.0, "doc": dt, "date": dd, "section": v.1,
                                                 "text": v.2.chars().take(220).collect::<String>(), "link": v.3, "speaker": v.4, "score": (s * 100.0).round() / 100.0}));
    }
    let out: Vec<Value> = groups
        .into_iter()
        .map(|(i, mut rel)| {
            rel.sort_by(|a, b| b["score"].as_f64().partial_cmp(&a["score"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
            rel.truncate(3);
            json!({"anchor": anchors[i].1, "link": anchors[i].2, "related": rel})
        })
        .collect();
    let topics: Vec<String> = c
        .prepare("SELECT k.category_label, count(*) n FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE ch.source = ?1 AND k.kind IN ('intent_l1', 'intent_l2') GROUP BY k.category_id ORDER BY n DESC LIMIT 5")?
        .query_map([&src], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(json!({"source": src, "topics": topics, "groups": out}))
}

/// One topic or term (by node id): the topic or term itself as the taxonomy now has it (`item`; the panel reads it
/// from here — the dictionary comes a page at a time), and its latest sections (within `scope`), with their document
/// and link.
pub fn node(id: &str, scope: Scope) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let view = super::browse::view(scope)?;
    let item = ["topics", "terms"].iter().flat_map(|k| view[*k].as_array().cloned().unwrap_or_default()).find(|x| x["id"] == id).unwrap_or(Value::Null);
    let rows: Vec<Value> = c
        .prepare(
            "SELECT ch.id, ch.source, ch.title, ch.text, ch.link, ch.speaker, d.title, d.event_date FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id
             LEFT JOIN documents d ON d.source = ch.source WHERE k.category_id = ?1
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = ch.id AND s.kind = ?2 AND s.id = ?3))
             ORDER BY ch.event_date DESC, ch.ord LIMIT 12",
        )?
        .query_map(params![id, scope.map(|s| s.0), scope.map(|s| s.1)], |r| {
            Ok((
                (r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?),
                (r.get::<_, Option<String>>(6)?, r.get::<_, Option<String>>(7)?),
            ))
        })?
        .map(|r| {
            r.map(|((cid, src, title, text, link, speaker), (doc, date))| {
                json!({"chunk_id": cid.to_string(), "source": src, "doc": doc, "date": date, "section": title, "text": text.chars().take(240).collect::<String>(), "link": link, "speaker": speaker})
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(json!({"id": id, "item": item, "sections": rows}))
}

/// "Mentioned also in" for a work item: for each of its quotes (with the call it came from), the
/// section saying it and its nearest sections elsewhere in the thread; then a search of its title in
/// the thread. Sources the item already cites are left out.
pub fn mentions(thread: &str, title: &str, quotes: &[(String, String)]) -> Result<Value> {
    let path = super::store_path();
    let c = store::open(&path)?;
    let scope: Scope = Some(("thread", thread));
    let cited: BTreeSet<String> = quotes.iter().map(|(stamp, _)| format!("call/{stamp}")).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<Value> = vec![];
    let mut take = |v: &Value, why: &str, out: &mut Vec<Value>| {
        let src = v["source"].as_str().unwrap_or("").to_string();
        let id = v["chunk_id"].as_str().unwrap_or("").to_string();
        if cited.contains(&src) || !seen.insert(id) || out.len() >= 6 {
            return;
        }
        let mut x = v.clone();
        x["why"] = json!(why);
        x["text"] = json!(v["text"].as_str().unwrap_or("").chars().take(220).collect::<String>());
        out.push(x);
    };
    for (stamp, quote) in quotes {
        let q: String = quote.split_whitespace().take(8).collect::<Vec<_>>().join(" ");
        if q.is_empty() {
            continue;
        }
        let id: Option<i64> = c
            .query_row("SELECT id FROM chunks WHERE source = ?1 AND instr(lower(text), lower(?2)) > 0 LIMIT 1", params![format!("call/{stamp}"), q], |r| r.get(0))
            .ok();
        if let Some(id) = id {
            let rel = store::related_in(&path, id, 4, scope)?;
            for r in rel["related"].as_array().into_iter().flatten() {
                if r["score"].as_f64().unwrap_or(0.0) >= 0.6 {
                    take(r, "near what was said", &mut out);
                }
            }
        }
    }
    if out.len() < 3 && !title.trim().is_empty() {
        let emb = super::embedder();
        let hits = store::search_in(&path, title, 6, None, scope, emb.as_ref().map(|e| e as &dyn super::embed::Embedder))?;
        for h in hits["hits"].as_array().into_iter().flatten() {
            take(h, "about the same", &mut out);
        }
    }
    let titles: BTreeMap<String, (Option<String>, Option<String>)> = c
        .prepare("SELECT source, title, event_date FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<Result<_, _>>()?;
    for x in out.iter_mut() {
        let (t, d) = titles.get(x["source"].as_str().unwrap_or("")).cloned().unwrap_or_default();
        x["doc"] = json!(t);
        x["date"] = json!(d);
    }
    Ok(json!({"items": out}))
}

/// One source for the graph's details panel: what it is, where it belongs, its topics, its first
/// sections and the sources most related to it (in the scope).
pub fn doc(source: &str, scope: Scope) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let Some((title, kind, date, refs)) = c
        .query_row("SELECT title, kind, event_date, refs FROM documents WHERE source = ?1", [source], |r| {
            Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?))
        })
        .optional()?
    else {
        return Ok(json!({"source": source, "missing": true}));
    };
    let refs: Value = refs.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    let topics: Vec<Value> = c
        .prepare(
            "SELECT k.category_id, k.category_label, count(*) n, max(k.kind) FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id
             WHERE ch.source = ?1 AND k.category_id != '__no_topic__' GROUP BY k.category_id ORDER BY n DESC LIMIT 20",
        )?
        .query_map([source], |r| {
            let id: String = r.get(0)?;
            let term = r.get::<_, Option<String>>(3)?.as_deref() == Some("term");
            Ok(json!({"id": id, "label": r.get::<_, Option<String>>(1)?, "sections": r.get::<_, i64>(2)?, "term": term}))
        })?
        .collect::<Result<_, _>>()?;
    let total: i64 = c.query_row("SELECT count(*) FROM chunks WHERE source = ?1", [source], |r| r.get(0))?;
    let sections: Vec<Value> = c
        .prepare("SELECT id, title, text, link FROM chunks WHERE source = ?1 ORDER BY ord LIMIT 8")?
        .query_map([source], |r| {
            Ok(json!({"chunk_id": r.get::<_, i64>(0)?.to_string(), "section": r.get::<_, Option<String>>(1)?,
                      "text": r.get::<_, String>(2)?.chars().take(220).collect::<String>(), "link": r.get::<_, Option<String>>(3)?}))
        })?
        .collect::<Result<_, _>>()?;
    // Related sources: the related edges from its sections to other documents', summed by document.
    let related: Vec<Value> = c
        .prepare(
            "SELECT o.source, d.title, d.kind, d.event_date, count(*) n, max(r.score) s FROM related r
             JOIN chunks a ON a.id = r.chunk_id JOIN chunks o ON o.id = r.related_id JOIN documents d ON d.source = o.source
             WHERE a.source = ?1 AND o.source != ?1
               AND (?2 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = o.id AND s.kind = ?2 AND s.id = ?3))
             GROUP BY o.source ORDER BY n DESC, s DESC LIMIT 8",
        )?
        .query_map(params![source, scope.map(|s| s.0), scope.map(|s| s.1)], |r| {
            Ok(json!({"source": r.get::<_, String>(0)?, "title": r.get::<_, Option<String>>(1)?, "kind": r.get::<_, Option<String>>(2)?,
                      "date": r.get::<_, Option<String>>(3)?, "links": r.get::<_, i64>(4)?, "score": (r.get::<_, f64>(5)? * 100.0).round() / 100.0}))
        })?
        .collect::<Result<_, _>>()?;
    // Only in closed threads: the archive (#55), said with when each closed.
    let closed = crate::projects::closed_threads();
    let ids: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    let archived = (!ids.is_empty() && ids.iter().all(|t| closed.contains_key(*t))).then(|| ids.iter().filter_map(|t| closed.get(*t).cloned()).collect::<Vec<_>>());
    Ok(json!({"source": source, "title": title, "kind": kind, "date": date, "link": doc_link(source, &refs), "closed": archived,
              "threads": refs["threads"], "client": refs["client"], "rel": refs["rel"], "fidelity": refs["fidelity"],
              "sections_total": total, "topics": topics, "sections": sections, "related": related}))
}

//! What the window shows of the Brain: its overview (what it holds, its scopes, the taxonomy), the
//! graph (documents clustered around their topics, related documents linked), and a call's "Also
//! discussed" (the nearest moments of other documents, by moment). No model: all from the store.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::store::{self, Scope};
use super::taxonomy;

/// A document's link: a call opens in Echo; other sources carry their own link on their sections.
fn doc_link(source: &str, refs: &Value) -> Option<String> {
    if source.starts_with("call/") {
        return refs["call"].as_str().map(|c| format!("echo://call/{c}"));
    }
    // A channel post or a mail: its own link, in Teams / Outlook.
    refs["url"].as_str().filter(|u| u.starts_with("https://")).map(str::to_owned)
}

pub fn overview(scope: Scope) -> Result<Value> {
    let path = super::store_path();
    let c = store::open(&path)?;
    let docs: Vec<Value> = c
        .prepare(
            "SELECT d.source, d.title, d.kind, d.event_date, d.refs, (SELECT count(*) FROM chunks c WHERE c.source = d.source),
               (SELECT count(*) FROM chunks c WHERE c.source = d.source AND c.status = 'SUPERSEDED') FROM documents d
             WHERE ?1 IS NULL OR EXISTS (SELECT 1 FROM chunks c JOIN scopes s ON s.chunk_id = c.id WHERE c.source = d.source AND s.kind = ?1 AND s.id = ?2)
             ORDER BY d.event_date DESC",
        )?
        .query_map(params![scope.map(|s| s.0), scope.map(|s| s.1)], |r| {
            let refs: Value = r.get::<_, Option<String>>(4)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            let src: String = r.get(0)?;
            Ok(json!({"source": src, "title": r.get::<_, Option<String>>(1)?, "kind": r.get::<_, Option<String>>(2)?, "date": r.get::<_, Option<String>>(3)?,
                      "sections": r.get::<_, i64>(5)?, "threads": refs["threads"], "link": doc_link(&src, &refs),
                      "channel": refs["channel_name"], "folder": refs["root"], "history": r.get::<_, i64>(6)? > 0}))
        })?
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "status": super::status()?,
        "scopes": store::scopes(&path)?,
        "taxonomy": taxonomy::view_in(scope)?,
        "documents": docs,
        "scope": scope.map(|s| json!({"kind": s.0, "id": s.1})),
    }))
}

/// A document as the graph reads it: source, title, kind, date, refs, sections.
type DocRow = (String, Option<String>, Option<String>, Option<String>, Value, i64);

/// The graph: documents (with their client, threads, top topics) and level-1 topics as nodes; edges
/// document → topic (how many of its sections are tagged with it) and document ↔ document (how many
/// related section pairs join them, at least two).
pub fn graph(scope: Scope) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let allowed: Option<BTreeSet<String>> = match scope {
        Some((k, v)) => Some(
            c.prepare("SELECT DISTINCT ch.source FROM scopes s JOIN chunks ch ON ch.id = s.chunk_id WHERE s.kind = ?1 AND s.id = ?2")?
                .query_map(params![k, v], |r| r.get(0))?
                .collect::<Result<_, _>>()?,
        ),
        None => None,
    };
    let ok = |s: &str| allowed.as_ref().is_none_or(|a| a.contains(s));
    let mut nodes = vec![];
    let docs: Vec<DocRow> = c
        .prepare("SELECT d.source, d.title, d.kind, d.event_date, d.refs, (SELECT count(*) FROM chunks c WHERE c.source = d.source) FROM documents d")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<String>>(4)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null), r.get(5)?)))?
        .collect::<Result<_, _>>()?;
    for (src, title, kind, date, refs, n) in &docs {
        if !ok(src) {
            continue;
        }
        let client = refs["threads"].as_array().into_iter().flatten().find_map(|t| t["client"].as_str()).or(refs["client"].as_str());
        nodes.push(json!({"id": src, "type": "doc", "label": title, "kind": kind, "date": date, "sections": n, "client": client,
                          "threads": refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect::<Vec<_>>(), "link": doc_link(src, refs)}));
    }
    // Document → level-1 topic, by tagged sections.
    let mut dt: BTreeMap<(String, String), i64> = BTreeMap::new();
    let mut st = c.prepare("SELECT ch.source, k.category_id FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE k.kind = 'intent_l1'")?;
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (s, t) = row?;
        if ok(&s) {
            *dt.entry((s, t)).or_default() += 1;
        }
    }
    let labels: BTreeMap<String, String> = c.prepare("SELECT id, label FROM graph_nodes WHERE kind = 'intent_l1'")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let used: BTreeSet<&String> = dt.keys().map(|(_, t)| t).collect();
    // Each topic's layer (shared, or a client's), so the window groups and colours them by client.
    let layer_of: BTreeMap<String, String> = taxonomy::current()
        .map(|t| t.topics.iter().map(|x| (taxonomy::topic_id(&x.layer, &x.label), x.layer.clone())).collect())
        .unwrap_or_default();
    for (id, label) in &labels {
        if used.contains(id) {
            let n: i64 = dt.iter().filter(|((_, t), _)| t == id).map(|(_, n)| n).sum();
            nodes.push(json!({"id": format!("topic:{id}"), "type": "topic", "label": label, "sections": n, "layer": layer_of.get(id).cloned().unwrap_or_else(|| "shared".into())}));
        }
    }
    let mut edges: Vec<Value> = dt.iter().filter(|((_, t), _)| labels.contains_key(t)).map(|((s, t), n)| json!({"source": s, "target": format!("topic:{t}"), "kind": "about", "weight": n})).collect();
    // Document ↔ document, by related section pairs.
    let mut dd: BTreeMap<(String, String), i64> = BTreeMap::new();
    let mut st = c.prepare("SELECT a.source, b.source FROM related r JOIN chunks a ON a.id = r.chunk_id JOIN chunks b ON b.id = r.related_id WHERE a.source < b.source")?;
    for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
        let (a, b) = row?;
        if ok(&a) && ok(&b) {
            *dd.entry((a, b)).or_default() += 1;
        }
    }
    edges.extend(dd.into_iter().filter(|(_, n)| *n >= 2).map(|((a, b), n)| json!({"source": a, "target": b, "kind": "related", "weight": n})));
    Ok(json!({"nodes": nodes, "edges": edges}))
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

/// One topic or term (by node id): its latest sections (within `scope`), with their document and link.
pub fn node(id: &str, scope: Scope) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let titles: BTreeMap<String, (Option<String>, Option<String>)> = c
        .prepare("SELECT source, title, event_date FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?
        .collect::<Result<_, _>>()?;
    let rows: Vec<Value> = c
        .prepare(
            "SELECT ch.id, ch.source, ch.title, ch.text, ch.link, ch.speaker FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE k.category_id = ?1
             AND (?2 IS NULL OR EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = ch.id AND s.kind = ?2 AND s.id = ?3))
             ORDER BY ch.event_date DESC, ch.ord LIMIT 12",
        )?
        .query_map(params![id, scope.map(|s| s.0), scope.map(|s| s.1)], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?))
        })?
        .map(|r| {
            r.map(|(cid, src, title, text, link, speaker)| {
                let (doc, date) = titles.get(&src).cloned().unwrap_or_default();
                json!({"chunk_id": cid.to_string(), "source": src, "doc": doc, "date": date, "section": title, "text": text.chars().take(240).collect::<String>(), "link": link, "speaker": speaker})
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(json!({"id": id, "sections": rows}))
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

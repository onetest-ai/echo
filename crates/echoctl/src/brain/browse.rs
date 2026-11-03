//! The Brain at scale, for the window: what it holds a piece at a time — sources grouped (client → kind, channels,
//! folders) and listed a page at a time, a folder's files a page at a time, the dictionary by group and letter, and a
//! graph drawn as you go (topics first; a topic's sources, a client's sources on asking). Nothing here reads every
//! section to answer: lists come from `documents` (its list facts, `store::DOC_FACTS`), counts from SQL aggregates,
//! the costly ones kept until the store changes (`fingerprint`).

use anyhow::Result;
use rusqlite::{params, params_from_iter, Connection};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::Mutex;

use super::store;

/// What changes when the store, the taxonomy or the threads change: the key under which costly answers are kept.
pub fn fingerprint() -> String {
    let stamp = |p: &std::path::Path| std::fs::metadata(p).map(|m| format!("{}:{:?};", m.len(), m.modified().ok())).unwrap_or_default();
    let db = super::store_path();
    let mut out = store::changed_stamp(&db);
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

/// `make()` counted from the store and kept while it is unchanged (`store::kept`: stale while counted again).
pub fn kept(key: &str, make: impl FnOnce() -> Result<Value> + Send + 'static) -> Result<Value> {
    Ok(store::kept(format!("{}|{key}", super::store_path().display()), fingerprint(), make)?.as_ref().clone())
}

/// The taxonomy as a scope sees it (`taxonomy::view_in`), kept until the store changes: the overview and the
/// dictionary's pages read the same one.
pub fn view(scope: store::Scope) -> Result<Value> {
    cached(&format!("view|{scope:?}"), || super::taxonomy::view_in(scope))
}

/// The SQL that keeps a document within a scope (a client's, or one thread's — `-`: in none), and its parameters: as
/// the rest of the Brain scopes it, by its sections' scopes (a document's sections share them: its first one says).
fn doc_scope(scope: store::Scope) -> (String, Vec<String>) {
    match scope {
        Some((k, id)) => (
            "EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = (SELECT ch.id FROM chunks ch WHERE ch.source = documents.source LIMIT 1) AND s.kind = ? AND s.id = ?)".into(),
            vec![k.into(), id.into()],
        ),
        None => ("1".into(), vec![]),
    }
}

fn client_names() -> BTreeMap<String, String> {
    crate::projects::orgs().unwrap_or_default().into_iter().map(|c| (c.id, c.name)).collect()
}

/// Sources grouped: per client, how many of each kind (and their sections), its channels and folders; newest
/// client first. One aggregate over `documents` (indexed), never their sections.
pub fn source_groups(scope: store::Scope) -> Result<Value> {
    let owned: Option<(String, String)> = scope.map(|(k, v)| (k.to_string(), v.to_string()));
    kept(&format!("groups|{scope:?}"), move || {
        let scope = owned.as_ref().map(|(k, v)| (k.as_str(), v.as_str()));
        let c = store::open(&super::store_path())?;
        let (w, p) = doc_scope(scope);
        let mut clients: BTreeMap<String, (Value, String)> = BTreeMap::new();
        let names = client_names();
        let mut st = c.prepare(&format!("SELECT coalesce(client, '-'), coalesce(kind, 'other'), count(*), coalesce(sum(sections), 0), max(event_date) FROM documents WHERE {w} GROUP BY 1, 2"))?;
        for row in st.query_map(params_from_iter(p.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, Option<String>>(4)?)))? {
            let (cl, kind, n, secs, last) = row?;
            let e = clients.entry(cl.clone()).or_insert_with(|| {
                (json!({"client": cl, "name": if cl == "-" { "No client".to_string() } else { names.get(&cl).cloned().unwrap_or_else(|| cl.clone()) }, "kinds": [], "docs": 0, "sections": 0}), String::new())
            });
            if let Some(a) = e.0["kinds"].as_array_mut() {
                a.push(json!({"kind": kind, "docs": n, "sections": secs}));
            }
            e.0["docs"] = json!(e.0["docs"].as_i64().unwrap_or(0) + n);
            e.0["sections"] = json!(e.0["sections"].as_i64().unwrap_or(0) + secs);
            if let Some(l) = last.filter(|l| *l > e.1) {
                e.1 = l;
            }
        }
        // A channel and a folder are one row each, opened to their own pages.
        let mut st = c.prepare(&format!("SELECT coalesce(client, '-'), json_extract(refs, '$.channel_name'), count(*) FROM documents WHERE kind = 'channel' AND {w} GROUP BY 1, 2 ORDER BY 3 DESC"))?;
        for row in st.query_map(params_from_iter(p.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, i64>(2)?)))? {
            let (cl, ch, n) = row?;
            if let Some(Value::Array(a)) = clients.get_mut(&cl).and_then(|e| e.0.as_object_mut()).map(|o| o.entry("channels").or_insert(json!([]))) {
                a.push(json!({"name": ch.unwrap_or_else(|| "(channel)".into()), "posts": n}));
            }
        }
        let mut st = c.prepare(&format!("SELECT coalesce(client, '-'), root, count(*) FROM documents WHERE root IS NOT NULL AND {w} GROUP BY 1, 2 ORDER BY 3 DESC"))?;
        for row in st.query_map(params_from_iter(p.iter()), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)))? {
            let (cl, root, n) = row?;
            if let Some(Value::Array(a)) = clients.get_mut(&cl).and_then(|e| e.0.as_object_mut()).map(|o| o.entry("folders").or_insert(json!([]))) {
                a.push(json!({"root": root, "name": root.rsplit(['/', '\\']).next().unwrap_or(&root), "docs": n}));
            }
        }
        let mut list: Vec<(Value, String)> = clients.into_values().collect();
        list.sort_by(|a, b| (a.0["client"] == "-").cmp(&(b.0["client"] == "-")).then(b.0["docs"].as_i64().cmp(&a.0["docs"].as_i64())));
        let total: i64 = list.iter().map(|x| x.0["docs"].as_i64().unwrap_or(0)).sum();
        let by_kind: BTreeMap<String, i64> = list.iter().flat_map(|x| x.0["kinds"].as_array().cloned().unwrap_or_default()).fold(BTreeMap::new(), |mut m, k| {
            *m.entry(k["kind"].as_str().unwrap_or("other").to_string()).or_default() += k["docs"].as_i64().unwrap_or(0);
            m
        });
        Ok(json!({"total": total, "by_kind": by_kind, "clients": list.into_iter().map(|x| x.0).collect::<Vec<_>>()}))
    })
}

/// What a page of sources is filtered by.
#[derive(Default)]
pub struct ListQuery {
    pub client: Option<String>,
    pub kind: Option<String>,
    pub root: Option<String>,
    pub channel: Option<String>,
    pub q: Option<String>,
    /// The cursor: the last row's `date\tsource`.
    pub before: Option<String>,
    pub limit: usize,
}

/// A page of sources, newest first (by date, then source), with a cursor for the next.
pub fn source_list(scope: store::Scope, q: &ListQuery) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let (w, mut p) = doc_scope(scope);
    let mut wh = vec![w];
    if let Some(x) = &q.client {
        wh.push("coalesce(client, '-') = ?".into());
        p.push(x.clone());
    }
    if let Some(x) = &q.kind {
        wh.push("kind = ?".into());
        p.push(x.clone());
    }
    if let Some(x) = &q.root {
        wh.push("root = ?".into());
        p.push(x.clone());
    }
    if let Some(x) = &q.channel {
        wh.push("json_extract(refs, '$.channel_name') = ?".into());
        p.push(x.clone());
    }
    for word in q.q.as_deref().unwrap_or("").split_whitespace().take(8) {
        wh.push("lower(coalesce(title, '') || ' ' || source) LIKE ? ESCAPE '\\'".into());
        p.push(format!("%{}%", word.to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")));
    }
    let base = wh.join(" AND ");
    let total: i64 = c.query_row(&format!("SELECT count(*) FROM documents WHERE {base}"), params_from_iter(p.iter()), |r| r.get(0))?;
    let mut cond = base.clone();
    let mut pp = p.clone();
    if let Some((d, s)) = q.before.as_deref().and_then(|b| b.split_once('\t')) {
        cond.push_str(" AND (coalesce(event_date, '') < ? OR (coalesce(event_date, '') = ? AND source < ?))");
        pp.extend([d.to_string(), d.to_string(), s.to_string()]);
    }
    let limit = q.limit.clamp(1, 200);
    pp.push(limit.to_string());
    let rows: Vec<Value> = c
        .prepare(&format!(
            "SELECT source, title, kind, event_date, refs, coalesce(sections, 0), client, root,
               EXISTS (SELECT 1 FROM chunks c WHERE c.source = documents.source AND c.status = 'SUPERSEDED')
             FROM documents WHERE {cond} ORDER BY coalesce(event_date, '') DESC, source DESC LIMIT ?"
        ))?
        .query_map(params_from_iter(pp.iter()), |r| {
            let src: String = r.get(0)?;
            let refs: Value = r.get::<_, Option<String>>(4)?.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
            Ok(json!({"source": src, "title": r.get::<_, Option<String>>(1)?, "kind": r.get::<_, Option<String>>(2)?, "date": r.get::<_, Option<String>>(3)?,
                      "sections": r.get::<_, i64>(5)?, "client": r.get::<_, Option<String>>(6)?, "folder": r.get::<_, Option<String>>(7)?, "history": r.get::<_, bool>(8)?,
                      "threads": refs["threads"], "channel": refs["channel_name"], "rel": refs["rel"], "link": super::views::doc_link(&src, &refs)}))
        })?
        .collect::<Result<_, _>>()?;
    let next = (rows.len() == limit).then(|| rows.last().map(|x| format!("{}\t{}", x["date"].as_str().unwrap_or(""), x["source"].as_str().unwrap_or("")))).flatten();
    Ok(json!({"items": rows, "next": next, "total": total}))
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

/// The graph's first sight: level-1 topics (their sections in the scope, their layer) and how they are related —
/// related section pairs between them, the strongest few per topic.
pub fn graph_topics(scope: store::Scope) -> Result<Value> {
    let owned: Option<(String, String)> = scope.map(|(k, v)| (k.to_string(), v.to_string()));
    kept(&format!("graph_topics|{scope:?}"), move || {
        let scope = owned.as_ref().map(|(k, v)| (k.as_str(), v.as_str()));
        let c = store::open(&super::store_path())?;
        let (sc, sp): (String, Vec<String>) = match scope {
            Some((k, v)) => ("AND EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = k.chunk_id AND s.kind = ? AND s.id = ?)".into(), vec![k.into(), v.into()]),
            None => (String::new(), vec![]),
        };
        let labels: BTreeMap<String, String> = c.prepare("SELECT id, label FROM graph_nodes WHERE kind = 'intent_l1'")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let layer_of: BTreeMap<String, String> = super::taxonomy::current()
            .map(|t| t.topics.iter().map(|x| (super::taxonomy::topic_id(&x.layer, &x.label), x.layer.clone())).collect())
            .unwrap_or_default();
        let counts: Vec<(String, i64, i64)> = c
            .prepare(&format!("SELECT k.category_id, count(DISTINCT k.chunk_id), count(DISTINCT ch.source) FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE k.kind = 'intent_l1' {sc} GROUP BY k.category_id"))?
            .query_map(params_from_iter(sp.iter()), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        let nodes: Vec<Value> = counts.iter().filter(|(id, ..)| labels.contains_key(id)).map(|(id, n, docs)| {
            json!({"id": format!("topic:{id}"), "type": "topic", "label": labels[id], "sections": n, "sources": docs, "layer": layer_of.get(id).cloned().unwrap_or_else(|| "shared".into())})
        }).collect();
        // Topic ↔ topic: related section pairs across them; each topic keeps its 6 strongest.
        let mut pairs: Vec<(String, String, i64)> = c
            .prepare(&format!(
                "SELECT k.category_id, kb.category_id, count(*) FROM related r JOIN chunk_topics k ON k.chunk_id = r.chunk_id AND k.kind = 'intent_l1'
                 JOIN chunk_topics kb ON kb.chunk_id = r.related_id AND kb.kind = 'intent_l1' WHERE k.category_id < kb.category_id {sc} GROUP BY 1, 2"
            ))?
            .query_map(params_from_iter(sp.iter()), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        pairs.sort_by_key(|p| std::cmp::Reverse(p.2));
        let mut per: HashMap<String, usize> = HashMap::new();
        let mut edges = vec![];
        for (a, b, n) in pairs {
            if !labels.contains_key(&a) || !labels.contains_key(&b) {
                continue;
            }
            let (na, nb) = (*per.get(&a).unwrap_or(&0), *per.get(&b).unwrap_or(&0));
            if na >= 6 && nb >= 6 {
                continue;
            }
            *per.entry(a.clone()).or_default() += 1;
            *per.entry(b.clone()).or_default() += 1;
            edges.push(json!({"source": format!("topic:{a}"), "target": format!("topic:{b}"), "kind": "topics", "weight": n}));
        }
        let sources: i64 = c.query_row("SELECT count(*) FROM documents", [], |r| r.get(0))?;
        Ok(json!({"nodes": nodes, "edges": edges, "sources_total": sources}))
    })
}

fn doc_nodes(c: &Connection, sources: &[String]) -> Result<Vec<Value>> {
    let mut out = vec![];
    let mut st = c.prepare("SELECT source, title, kind, event_date, refs, coalesce(sections, 0), client FROM documents WHERE source = ?1")?;
    for s in sources {
        if let Some(v) = st
            .query_map([s], |r| {
                let src: String = r.get(0)?;
                let refs: Value = r.get::<_, Option<String>>(4)?.and_then(|x| serde_json::from_str(&x).ok()).unwrap_or(Value::Null);
                Ok(json!({"id": src, "type": "doc", "label": r.get::<_, Option<String>>(1)?, "kind": r.get::<_, Option<String>>(2)?, "date": r.get::<_, Option<String>>(3)?,
                          "sections": r.get::<_, i64>(5)?, "client": r.get::<_, Option<String>>(6)?.filter(|c| c != "-"),
                          "threads": refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect::<Vec<_>>(), "link": super::views::doc_link(&src, &refs)}))
            })?
            .next()
        {
            out.push(v?);
        }
    }
    Ok(out)
}

/// One topic opened in the graph: the sources most tagged with it (in the scope), each with how many sections.
pub fn graph_topic(id: &str, scope: store::Scope, limit: usize) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let id = id.strip_prefix("topic:").unwrap_or(id);
    let (sc, mut p): (String, Vec<String>) = match scope {
        Some((k, v)) => ("AND EXISTS (SELECT 1 FROM scopes s WHERE s.chunk_id = k.chunk_id AND s.kind = ? AND s.id = ?)".into(), vec![id.into(), k.into(), v.into()]),
        None => (String::new(), vec![id.into()]),
    };
    p.push(limit.clamp(1, 200).to_string());
    let top: Vec<(String, i64)> = c
        .prepare(&format!("SELECT ch.source, count(*) n FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE k.category_id = ? {sc} GROUP BY ch.source ORDER BY n DESC LIMIT ?"))?
        .query_map(params_from_iter(p.iter()), |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    p.pop();
    let total: i64 = c.query_row(&format!("SELECT count(DISTINCT ch.source) FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE k.category_id = ? {sc}"), params_from_iter(p.iter()), |r| r.get(0))?;
    let nodes = doc_nodes(&c, &top.iter().map(|t| t.0.clone()).collect::<Vec<_>>())?;
    let edges: Vec<Value> = top.iter().map(|(s, n)| json!({"source": s, "target": format!("topic:{id}"), "kind": "about", "weight": n})).collect();
    Ok(json!({"topic": format!("topic:{id}"), "nodes": nodes, "edges": edges, "shown": top.len(), "total": total}))
}

/// One client opened in the graph: its sources with the most sections (up to `limit`) and the main topics each is about.
pub fn graph_client(client: &str, limit: usize) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    let limit = limit.clamp(1, 400);
    let top: Vec<String> = c
        .prepare("SELECT source FROM documents WHERE client = ?1 ORDER BY sections DESC, event_date DESC LIMIT ?2")?
        .query_map(params![client, limit as i64], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let total: i64 = c.query_row("SELECT count(*) FROM documents WHERE client = ?1", [client], |r| r.get(0))?;
    let nodes = doc_nodes(&c, &top)?;
    let edges = main_topics(&c, &top)?;
    Ok(json!({"client": client, "nodes": nodes, "edges": edges, "shown": top.len(), "total": total}))
}

/// Each source joined to its 3 main topics (the ones most of its sections are tagged with): more is a hairball.
fn main_topics(c: &Connection, sources: &[String]) -> Result<Vec<Value>> {
    let mut edges = vec![];
    let mut st = c.prepare("SELECT k.category_id, count(*) n FROM chunk_topics k JOIN chunks ch ON ch.id = k.chunk_id WHERE ch.source = ?1 AND k.kind = 'intent_l1' GROUP BY 1 ORDER BY n DESC LIMIT 3")?;
    for s in sources {
        for row in st.query_map([s], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (t, n) = row?;
            edges.push(json!({"source": s, "target": format!("topic:{t}"), "kind": "about", "weight": n}));
        }
    }
    Ok(edges)
}

/// The graph's search (Enter): the Brain searched (words and meaning), its hits' sources as the graph draws
/// them — titled, with their main topics — and how many sections were found.
pub fn graph_search(q: &str, scope: store::Scope, emb: Option<&dyn super::embed::Embedder>) -> Result<Value> {
    let path = super::store_path();
    let r = store::search_in(&path, q, 40, None, scope, emb)?;
    let hits = r["hits"].as_array().cloned().unwrap_or_default();
    let mut sources: Vec<String> = vec![];
    for h in &hits {
        if let Some(s) = h["source"].as_str() {
            if !sources.iter().any(|x| x == s) {
                sources.push(s.to_string());
            }
        }
    }
    let c = store::open(&path)?;
    Ok(json!({"nodes": doc_nodes(&c, &sources)?, "edges": main_topics(&c, &sources)?, "sections": hits.len(), "total": sources.len()}))
}

/// A folder's files a page at a time (the registry, `files`), with what happened to each.
pub fn folder_files(root: &str, q: Option<&str>, offset: usize, limit: usize) -> Result<Value> {
    let c = store::open(&super::store_path())?;
    super::files::folder::ensure_schema(&c)?;
    let like = format!("%{}%", q.unwrap_or("").to_lowercase().replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
    let total: i64 = c.query_row("SELECT count(*) FROM files WHERE root = ?1 AND state = 'active' AND lower(rel) LIKE ?2 ESCAPE '\\'", params![root, like], |r| r.get(0))?;
    let rows: Vec<Value> = c
        .prepare("SELECT id, rel, status, reason, method, fidelity, event_date, pages, flagged, read, size FROM files WHERE root = ?1 AND state = 'active' AND lower(rel) LIKE ?2 ESCAPE '\\' ORDER BY rel LIMIT ?3 OFFSET ?4")?
        .query_map(params![root, like, limit.clamp(1, 200) as i64, offset as i64], |r| {
            Ok(json!({"id": r.get::<_, String>(0)?, "rel": r.get::<_, String>(1)?, "status": r.get::<_, Option<String>>(2)?, "reason": r.get::<_, Option<String>>(3)?,
                      "method": r.get::<_, Option<String>>(4)?, "fidelity": r.get::<_, Option<String>>(5)?, "date": r.get::<_, Option<String>>(6)?,
                      "pages": r.get::<_, Option<i64>>(7)?, "flagged": r.get::<_, Option<i64>>(8)?, "read": r.get::<_, Option<i64>>(9)?, "size": r.get::<_, Option<i64>>(10)?}))
        })?
        .collect::<Result<_, _>>()?;
    let by: BTreeMap<String, i64> = c
        .prepare("SELECT coalesce(status, '?'), count(*) FROM files WHERE root = ?1 AND state = 'active' GROUP BY 1")?
        .query_map([root], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(json!({"root": root, "items": rows, "total": total, "offset": offset, "by_status": by}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::store::{sync, Input};

    fn doc(source: &str, kind: &str, client: &str, date: &str, sections: usize, extra: Value) -> Input {
        let s = source.to_string();
        let md: String = (0..sections).map(|i| format!("## Part {i}\n{} about the pilot, part {i}, {}.\n", "Text ".repeat(60), source)).collect();
        let mut refs = json!({"threads": [{"id": format!("{client}-thread"), "client": client}]});
        if let (Some(r), Some(e)) = (refs.as_object_mut(), extra.as_object()) {
            r.extend(e.clone());
        }
        Input { source: s.clone(), kind: kind.into(), title: format!("Title of {source}"), event_date: Some(date.into()), md, refs, link: Box::new(move |_| s.clone()), speaker: Box::new(|_| None) }
    }

    #[test]
    fn sources_come_a_page_at_a_time_grouped_and_whole() {
        let (_env, _root) = crate::paths::testenv::folders("brain-browse");
        let path = crate::brain::store_path();
        // Acme: 130 mails over 20 days (ties on every date), a channel of 7 posts, 3 calls; Globex: 4 mails.
        let mut inputs = vec![];
        for i in 0..130 {
            inputs.push(doc(&format!("mail/a{i:03}"), "mail", "acme", &format!("2026-10-{:02}", 1 + i % 20), 1 + i % 3, json!({})));
        }
        for i in 0..7 {
            inputs.push(doc(&format!("channel/x/{i}"), "channel", "acme", "2026-10-05", 1, json!({"channel_name": "acme-pilot"})));
        }
        for i in 0..3 {
            inputs.push(doc(&format!("call/c{i}"), "call", "acme", "2026-10-07", 2, json!({})));
        }
        for i in 0..4 {
            inputs.push(doc(&format!("mail/g{i}"), "mail", "globex", "2026-10-09", 1, json!({})));
        }
        sync(&path, inputs, None).unwrap();
        // Groups: per client, its kinds with counts and sections, its channels; biggest client first.
        let g = source_groups(None).unwrap();
        assert_eq!(g["total"], 144);
        assert_eq!(g["by_kind"]["mail"], 134);
        let acme = &g["clients"][0];
        assert_eq!((acme["client"].as_str(), acme["docs"].as_i64()), (Some("acme"), Some(140)));
        let mails = acme["kinds"].as_array().unwrap().iter().find(|k| k["kind"] == "mail").unwrap();
        assert_eq!(mails["docs"], 130);
        assert_eq!(mails["sections"].as_i64().unwrap(), (0..130).map(|i| 1 + i % 3).sum::<i64>());
        assert_eq!(acme["channels"][0], json!({"name": "acme-pilot", "posts": 7}));
        // Acme's mail page by page: every one exactly once, newest first, though many share a date.
        let (mut seen, mut before, mut last): (Vec<String>, Option<String>, Option<(String, String)>) = (vec![], None, None);
        loop {
            let p = source_list(None, &ListQuery { client: Some("acme".into()), kind: Some("mail".into()), before: before.clone(), limit: 50, ..Default::default() }).unwrap();
            assert_eq!(p["total"], 130);
            for x in p["items"].as_array().unwrap() {
                let k = (x["date"].as_str().unwrap().to_string(), x["source"].as_str().unwrap().to_string());
                assert!(last.as_ref().is_none_or(|l| *l > k), "newest first: {last:?} then {k:?}");
                last = Some(k.clone());
                seen.push(k.1);
            }
            match p["next"].as_str() {
                Some(n) => before = Some(n.to_string()),
                None => break,
            }
        }
        let n = seen.len();
        seen.sort();
        seen.dedup();
        assert_eq!((n, seen.len()), (130, 130), "no gaps, no repeats");
        // A channel's posts; a search across everything; a scope keeps to its client.
        let ch = source_list(None, &ListQuery { client: Some("acme".into()), channel: Some("acme-pilot".into()), limit: 50, ..Default::default() }).unwrap();
        assert_eq!(ch["total"], 7);
        let q = source_list(None, &ListQuery { q: Some("title A12".into()), limit: 50, ..Default::default() }).unwrap();
        assert_eq!(q["total"], 10, "mail/a120…a129");
        let gx = source_list(Some(("client", "globex")), &ListQuery { limit: 50, ..Default::default() }).unwrap();
        assert_eq!(gx["total"], 4);
        let th = source_groups(Some(("thread", "globex-thread"))).unwrap();
        assert_eq!(th["total"], 4);
    }
}

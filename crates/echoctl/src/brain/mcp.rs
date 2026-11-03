//! `echoctl brain mcp`: the Brain as an MCP server over stdio (JSON-RPC, one message per line),
//! read-only, with Brain's own tool names and shapes (`mcp/brain/fastmcp_server.py`), so the `kb`
//! skills — which recognise a Brain by its tools — and any Claude Code session use it unchanged.
//! Lanes not built yet (numbers, taxonomy) answer `not_modeled`, never a guess.

use anyhow::Result;
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

use super::store;

const PROTOCOL: &str = "2025-06-18";

fn tools() -> Value {
    let s = |props: Value, req: &[&str]| json!({"type": "object", "properties": props, "required": req});
    json!([
        {"name": "health", "description": "[identity] Is the Brain usable: counts per lane, the embedding model, what it covers.", "inputSchema": s(json!({}), &[])},
        {"name": "list_scopes", "description": "[identity] The user's clients and threads (engagements) the Brain covers, with their ids, names and how much each holds. Scope every search to a client or a thread when the question is about one.", "inputSchema": s(json!({}), &[])},
        {"name": "search_knowledge", "description": "[identity] Search the user's calls (minutes, transcripts with speakers and times, chats), their threads' Teams channels, mail and documents: keywords in any language fused with meaning. Scope it with `client` or `thread` (ids from list_scopes) when the question is about one. Documents a thread marks as history are left out unless `latest_only` is false, or `as_of` (YYYY-MM-DD) asks what was current on a day. Each hit has its source, section, text, date, speaker, its client and threads, its status, and a link to the moment or page.",
         "inputSchema": s(json!({"query": {"type": "string"}, "limit": {"type": "integer", "default": 5}, "client": {"type": "string"}, "thread": {"type": "string"}, "source_contains": {"type": "string"}, "latest_only": {"type": "boolean"}, "as_of": {"type": "string"}, "tag": {"type": "string"}}), &["query"])},
        {"name": "list_sources", "description": "[identity] The documents in the Brain — calls, mail conversations, Teams channel posts, folder documents — newest first; `client` / `thread` narrow them.", "inputSchema": s(json!({"contains": {"type": "string"}, "client": {"type": "string"}, "thread": {"type": "string"}, "offset": {"type": "integer"}}), &[])},
        {"name": "read_document", "description": "[identity] One document's sections in order (a call: its minutes, transcript, chat; a mail conversation or channel post: its messages' facts; a document: its pages).",
         "inputSchema": s(json!({"source": {"type": "string"}, "from_ord": {"type": "integer"}, "to_ord": {"type": "integer"}, "titles_only": {"type": "boolean"}, "max_chars": {"type": "integer"}}), &["source"])},
        {"name": "find_related_content", "description": "[identity] Sections from other sources nearest to a section (by chunk_id) or to a query.",
         "inputSchema": s(json!({"chunk_id": {"type": "string"}, "query": {"type": "string"}, "client": {"type": "string"}, "thread": {"type": "string"}, "limit": {"type": "integer", "default": 6}}), &[])},
        {"name": "get_evidence", "description": "[identity] One section verbatim, with its source and link.", "inputSchema": s(json!({"chunk_id": {"type": "string"}, "include_page_text": {"type": "boolean"}}), &["chunk_id"])},
        {"name": "get_taxonomy", "description": "[identity] The topics and terms (names, systems, acronyms) of the user's work — calls, mail, channels, documents — each with its sections, clients and threads; `client` / `thread` show what comes up there; `label` gives one node and its latest sections.", "inputSchema": s(json!({"label": {"type": "string"}, "client": {"type": "string"}, "thread": {"type": "string"}, "relation": {"type": "string"}, "kind": {"type": "string"}, "limit": {"type": "integer"}}), &[])},
        {"name": "list_metrics", "description": "[identity] Governed metrics from the client's reporting workbooks (the numbers lane): name, unit, description, definition (reported or computed), and per grain the entities and months the facts cover. Use it before any numeric answer.", "inputSchema": s(json!({"client": {"type": "string"}}), &[])},
        {"name": "get_metric", "description": "[identity] A governed metric's values, computed from the real cells, each with its source file: filter by grain (branch, region, division, overall…), entity (exact) or entity_like, month (YYYY-MM) or months. Says when a figure was restated by a later report or reports conflict (other_reported_values), and what a merged row merged. No rows = not modeled for that filter: say so, never estimate.", "inputSchema": s(json!({"name": {"type": "string"}, "client": {"type": "string"}, "grain": {"type": "string"}, "entity": {"type": "string"}, "entity_like": {"type": "string"}, "month": {"type": "string"}, "months": {"type": "array", "items": {"type": "string"}}}), &["name"])},
        {"name": "find_people", "description": "[identity] Who is who: the people of the user's threads — name, organisation, title, the threads they are in and their role there, when last seen. `query` matches names, addresses, organisations, titles and roles; `client` / `thread` narrow it. Then get_person for one.", "inputSchema": s(json!({"query": {"type": "string"}, "client": {"type": "string"}, "thread": {"type": "string"}, "limit": {"type": "integer", "default": 20}}), &[])},
        {"name": "get_person", "description": "[identity] One person (by key from find_people, an address or a name): who they are, their threads with role and stance, reporting line, open tasks they owe, who they are in calls with, and the facts said about them — each with its words, date and a link to the moment, mail or chat.", "inputSchema": s(json!({"person": {"type": "string"}, "facts": {"type": "integer", "default": 40}}), &["person"])},
        {"name": "get_metric_history", "description": "[identity] Every report of a metric-month: each file that reported it, what it said and which value is current — to cite \"the July workbook reported July as X\".", "inputSchema": s(json!({"name": {"type": "string"}, "client": {"type": "string"}, "grain": {"type": "string"}, "entity": {"type": "string"}, "month": {"type": "string"}}), &["name"])},
    ])
}

fn not_modeled(what: &str) -> Value {
    json!({"status": "not_modeled", "reason": format!("{what} isn't built in Echo's Brain yet"), "guidance": "Answer from search_knowledge and say what isn't modeled."})
}

/// The scope a tool call asks for: a client or a thread (by id).
fn scope_of(a: &Value) -> store::Scope<'_> {
    a["client"].as_str().filter(|s| !s.is_empty()).map(|c| ("client", c)).or_else(|| a["thread"].as_str().filter(|s| !s.is_empty()).map(|t| ("thread", t)))
}

/// What this server may show: one AI profile's sources (strict routing), narrowed for a call's session
/// to its client or threads. Set once when it starts; without one, the default profile's.
static ACCESS: std::sync::OnceLock<super::access::Access> = std::sync::OnceLock::new();

fn access() -> &'static super::access::Access {
    ACCESS.get_or_init(super::access::Access::default_profile)
}

fn call(name: &str, a: &Value) -> Result<Value> {
    let path = super::store_path();
    let acc = access();
    // People come from the threads' files, not the store: only the threads this reader may see.
    if name == "find_people" || name == "get_person" {
        let mut threads = acc.people_threads();
        if let Some(t) = a["thread"].as_str().filter(|t| !t.is_empty()) {
            threads.retain(|x| x == t);
        }
        if let Some(c) = a["client"].as_str().filter(|c| !c.is_empty()) {
            let of: Vec<String> = crate::projects::list().into_iter().filter(|p| p.client.as_deref() == Some(c)).map(|p| p.id).collect();
            threads.retain(|x| of.contains(x));
        }
        return if name == "find_people" {
            crate::people::find(&threads, a["query"].as_str().unwrap_or(""), a["limit"].as_u64().unwrap_or(20).clamp(1, 100) as usize)
        } else {
            crate::people::card(&threads, a["person"].as_str().unwrap_or(""), a["facts"].as_u64().unwrap_or(40).clamp(1, 300) as usize)
        };
    }
    // A narrowed reader searches its own client or thread whatever it asks.
    let scope = acc.scope().or_else(|| scope_of(a));
    let (allowed_src, allowed_ids) = {
        let c = store::open(&path)?;
        (acc.sources(&c)?, acc.chunks(&c)?)
    };
    let src_ok = |s: &str| allowed_src.contains(s);
    let emb = super::embedder();
    let emb = emb.as_ref().map(|e| e as &dyn super::embed::Embedder);
    match name {
        "health" => {
            let st = store::status(&path)?;
            let chunks = st["chunks"].as_i64().unwrap_or(0);
            Ok(json!({"status": if chunks > 0 { "healthy" } else { "degraded" }, "database_check": "ok", "vector_extension": "echo (in-process)",
                      "counts": {"chunks": chunks, "chunks_fts": chunks, "chunks_vec": st["embedded"], "graph_nodes": st["graph_nodes"], "graph_edges": 0, "chunk_topics": st["chunk_topics"], "facts": 0},
                      "empty_lanes": if st["graph_nodes"].as_i64().unwrap_or(0) > 0 { json!(["facts"]) } else { json!(["facts", "taxonomy"]) }, "knowledge_version": st["built_at"],
                      "about": {"name": "Echo — the user's work", "goal": "What was said in the user's calls, mail, Teams channels and documents: decisions, commitments, positions, people, over time", "audience": "the user"}}))
        }
        "search_knowledge" => {
            let q = a["query"].as_str().unwrap_or("");
            let history = match (a["as_of"].as_str().filter(|d| d.len() == 10), a["latest_only"].as_bool()) {
                (Some(d), _) => store::History::AsOf(d),
                (None, Some(false)) => store::History::All,
                _ => store::History::Latest,
            };
            let mut v = store::search_at(&path, q, a["limit"].as_u64().unwrap_or(5).clamp(1, 30) as usize, a["source_contains"].as_str(), scope, emb, history, Some(&allowed_ids))?;
            v["guidance"] = json!("Cite each claim with its source's title, date and time (the hit's link opens the moment in Echo). Say 'not modeled' for what no hit supports.");
            Ok(v)
        }
        "list_scopes" => {
            // Only the clients and threads it may read.
            let mut v = store::scopes(&path)?;
            let threads_ok: Vec<String> = {
                let default = crate::ai::default_id();
                super::access::thread_profiles(&default).into_iter().filter(|(_, p)| *p == acc.profile).map(|(t, _)| t).collect()
            };
            if let Some(a) = v["threads"].as_array_mut() {
                a.retain(|t| {
                    let id = t["id"].as_str().unwrap_or("");
                    (id == "-" || threads_ok.iter().any(|x| x == id)) && (acc.threads.is_empty() || acc.threads.iter().any(|x| x == id)) && acc.client.as_deref().is_none_or(|c| t["client"].as_str() == Some(c))
                });
            }
            if let Some(a) = v["clients"].as_array_mut() {
                a.retain(|c| {
                    let id = c["id"].as_str().unwrap_or("");
                    (id == "-" || super::access::client_profile_ok(id, &acc.profile)) && acc.client.as_deref().is_none_or(|x| x == id)
                });
            }
            Ok(v)
        }
        "list_sources" => {
            let c = store::open(&path)?;
            let like = format!("%{}%", a["contains"].as_str().unwrap_or(""));
            let off = a["offset"].as_i64().unwrap_or(0);
            let rows: Vec<Value> = c
                .prepare("SELECT d.source, d.title, d.event_date, d.kind, (SELECT count(*) FROM chunks c WHERE c.source = d.source), (SELECT sum(length(text)) FROM chunks c WHERE c.source = d.source), d.refs
                          FROM documents d WHERE (d.source LIKE ?1 OR d.title LIKE ?1)
                          AND (?3 IS NULL OR EXISTS (SELECT 1 FROM chunks c JOIN scopes s ON s.chunk_id = c.id WHERE c.source = d.source AND s.kind = ?3 AND s.id = ?4))
                          ORDER BY d.event_date DESC LIMIT 50 OFFSET ?2")?
                .query_map(params![like, off, scope.map(|s| s.0), scope.map(|s| s.1)], |r| Ok(json!({"source": r.get::<_, String>(0)?, "title": r.get::<_, Option<String>>(1)?, "event_date": r.get::<_, Option<String>>(2)?,
                                                         "kind": r.get::<_, Option<String>>(3)?, "n_sections": r.get::<_, i64>(4)?, "chars": r.get::<_, Option<i64>>(5)?,
                                                         "refs": r.get::<_, Option<String>>(6)?.and_then(|s| serde_json::from_str::<Value>(&s).ok())})))?
                .collect::<Result<_, _>>()?;
            let next = (rows.len() == 50).then_some(off + 50);
            let rows: Vec<Value> = rows.into_iter().filter(|r| src_ok(r["source"].as_str().unwrap_or(""))).collect();
            Ok(json!({"sources": rows, "next_offset": next}))
        }
        "read_document" => {
            let c = store::open(&path)?;
            let want = a["source"].as_str().unwrap_or("");
            let found: Vec<String> = c
                .prepare("SELECT source FROM documents WHERE source = ?1 UNION SELECT source FROM documents WHERE source LIKE ?2 OR title LIKE ?2 LIMIT 5")?
                .query_map(params![want, format!("%{want}%")], |r| r.get(0))?
                .collect::<Result<Vec<String>, _>>()?
                .into_iter()
                .filter(|s| src_ok(s))
                .collect();
            let src = if found.iter().any(|s| s == want) { want.to_string() } else if found.len() == 1 { found[0].clone() } else if found.is_empty() {
                return Ok(json!({"status": "not_found", "source": want}));
            } else {
                return Ok(json!({"status": "ambiguous", "candidates": found}));
            };
            let (from, to) = (a["from_ord"].as_i64().unwrap_or(0), a["to_ord"].as_i64().unwrap_or(i64::MAX));
            let titles = a["titles_only"].as_bool().unwrap_or(false);
            let mut budget = a["max_chars"].as_i64().unwrap_or(24000).clamp(1000, 48000);
            let mut out = vec![];
            let mut next = None;
            let total: i64 = c.query_row("SELECT count(*) FROM chunks WHERE source = ?1", [&src], |r| r.get(0))?;
            let mut st = c.prepare("SELECT id, ord, title, breadcrumb_path, text, link FROM chunks WHERE source = ?1 AND ord BETWEEN ?2 AND ?3 ORDER BY ord")?;
            let rows = st.query_map(params![src, from, to], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, String>(4)?, r.get::<_, Option<String>>(5)?)))?;
            for row in rows {
                let (id, ord, title, crumb, text, link) = row?;
                if !titles && budget < text.len() as i64 && !out.is_empty() {
                    next = Some(ord);
                    break;
                }
                budget -= text.len() as i64;
                out.push(if titles { json!({"chunk_id": id.to_string(), "ord": ord, "section": title, "breadcrumb_path": crumb, "preview": text.chars().take(160).collect::<String>(), "link": link}) }
                         else { json!({"chunk_id": id.to_string(), "ord": ord, "section": title, "breadcrumb_path": crumb, "text": text, "link": link}) });
            }
            Ok(json!({"source": src, "total_sections": total, "sections": out, "next_from_ord": next}))
        }
        "find_related_content" => {
            let limit = a["limit"].as_u64().unwrap_or(6).clamp(1, 20) as usize;
            let filtered = |mut v: Value| -> Value {
                if let Some(r) = v["related"].as_array_mut() {
                    r.retain(|x| src_ok(x["source"].as_str().unwrap_or("")));
                }
                v
            };
            if let Some(id) = a["chunk_id"].as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| a["chunk_id"].as_i64()) {
                if !allowed_ids.contains(&id) {
                    return Ok(json!({"status": "not_found", "chunk_id": id.to_string()}));
                }
                return Ok(filtered(store::related_in(&path, id, limit * 3, scope)?));
            }
            let q = a["query"].as_str().unwrap_or("");
            let top = store::search_at(&path, q, 1, None, scope, emb, store::History::Latest, Some(&allowed_ids))?;
            match top["hits"][0]["chunk_id"].as_str().and_then(|s| s.parse::<i64>().ok()) {
                Some(id) => Ok(filtered(store::related_in(&path, id, limit * 3, scope)?)),
                None => Ok(json!({"status": "not_found", "query": q})),
            }
        }
        "get_evidence" => {
            let c = store::open(&path)?;
            let id = a["chunk_id"].as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| a["chunk_id"].as_i64()).unwrap_or(0);
            if !allowed_ids.contains(&id) {
                return Ok(json!({"status": "not_found", "chunk_id": id.to_string()}));
            }
            let v = c
                .query_row("SELECT source, ord, title, text, link, event_date, speaker FROM chunks WHERE id = ?1", [id], |r| {
                    Ok(json!({"chunk_id": id.to_string(), "source": r.get::<_, String>(0)?, "ord": r.get::<_, i64>(1)?, "section": r.get::<_, String>(2)?,
                              "text": r.get::<_, String>(3)?, "link": r.get::<_, Option<String>>(4)?, "event_date": r.get::<_, Option<String>>(5)?,
                              "speaker": r.get::<_, Option<String>>(6)?, "page_asset": null, "verbatim_page_text": null, "extracted_tables": null}))
                })
                .optional()?;
            let Some(mut v) = v else { return Ok(json!({"status": "not_found", "chunk_id": id.to_string()})) };
            // A section of a thread folder's page: the page's factual content beside the reading —
            // its text layer, its table cells (cite numbers from these), its image for a client that can look.
            let link = v["link"].as_str().unwrap_or("").to_string();
            if let Some(rest) = link.strip_prefix("echo://file/") {
                let (fid, q) = rest.split_once('?').unwrap_or((rest, ""));
                let page = q.strip_prefix("page=").and_then(|n| n.parse::<usize>().ok());
                if let Some(page) = page {
                    let e = super::files::folder::evidence(fid, page);
                    v["page_asset"] = e["image"].clone();
                    v["extracted_tables"] = e["tables"].clone();
                    if a["include_page_text"].as_bool().unwrap_or(true) {
                        v["verbatim_page_text"] = e["text_layer"].clone();
                    }
                    v["page"] = json!(page);
                }
                v["file"] = json!(super::files::folder::path_of(fid).map(|p| p.to_string_lossy().to_string()));
            }
            Ok(v)
        }
        "get_taxonomy" => {
            let v = super::taxonomy::view_in(scope)?;
            if v["exists"] != true {
                return Ok(not_modeled("The topics taxonomy"));
            }
            let limit = a["limit"].as_u64().unwrap_or(20).clamp(1, 100) as i64;
            if let Some(label) = a["label"].as_str() {
                let id = super::taxonomy::nid(label);
                let node = v["topics"].as_array().into_iter().chain(v["terms"].as_array()).flatten()
                    .find(|x| x["id"] == id.as_str() || x["id"] == format!("term__{id}").as_str() || x["label"] == label || x["name"] == label).cloned();
                let Some(node) = node else { return Ok(json!({"status": "not_found", "label": label})) };
                let nid = node["id"].as_str().unwrap_or("").to_string();
                let subs: Vec<Value> = v["topics"].as_array().into_iter().flatten().filter(|x| x["parent"].as_str().map(super::taxonomy::nid) == Some(nid.clone())).cloned().collect();
                let c = store::open(&path)?;
                let tagged: Vec<Value> = c
                    .prepare("SELECT c.id, c.source, c.title, c.link, c.event_date FROM chunk_topics k JOIN chunks c ON c.id = k.chunk_id WHERE k.category_id = ?1 ORDER BY c.event_date DESC LIMIT ?2")?
                    .query_map(params![nid, limit], |r| Ok(json!({"chunk_id": r.get::<_, i64>(0)?.to_string(), "source": r.get::<_, String>(1)?, "section": r.get::<_, String>(2)?, "link": r.get::<_, Option<String>>(3)?, "event_date": r.get::<_, Option<String>>(4)?})))?
                    .collect::<Result<Vec<Value>, _>>()?
                    .into_iter()
                    .filter(|x| src_ok(x["source"].as_str().unwrap_or("")))
                    .collect();
                return Ok(json!({"node": node, "subclasses": subs, "tagged_sections": tagged}));
            }
            Ok(json!({"provisional": v["provisional"], "topics": v["topics"], "terms": v["terms"], "coverage": v["coverage"]}))
        }
        // Numbers only of clients whose threads run on this profile (and for a call's session, its own).
        "list_metrics" | "get_metric" | "get_metric_history" => {
            let ok = acc.numbers_clients();
            let asked = a["client"].as_str().map(str::to_owned);
            let clients: Vec<String> = match &asked {
                Some(c) => ok.iter().filter(|x| *x == c).cloned().collect(),
                None => ok,
            };
            if clients.is_empty() {
                return Ok(not_modeled("Numbers for this client (or on this AI profile)"));
            }
            if name == "list_metrics" {
                let mut all = vec![];
                for c in &clients {
                    all.extend(super::numbers::list_metrics(Some(c))?["metrics"].as_array().cloned().unwrap_or_default());
                }
                return Ok(json!({"metrics": all, "count": all.len()}));
            }
            let mut last = not_modeled("That metric");
            for c in &clients {
                let mut args = a.clone();
                args["client"] = json!(c);
                let v = if name == "get_metric" { super::numbers::get_metric(&args)? } else { super::numbers::get_metric_history(&args)? };
                if v["status"] != "not_modeled" {
                    return Ok(v);
                }
                last = v;
            }
            Ok(last)
        }
        _ => anyhow::bail!("no tool {name:?}"),
    }
}

/// What this reader's Brain holds and when to go there (#34), generated when the server starts; the fixed
/// rules on scoping and citing either way.
fn instructions() -> String {
    let rules = "When a question is about a client or a thread, pass its id (list_scopes) to every search — a fact from another client is not about this one. Cite every claim with its source (title, date, time) — never print raw ids; say 'not modeled' when nothing supports it. Read-only.";
    match super::advert::block(access()) {
        Some(b) => format!("{b}\n\n{rules}"),
        None => format!("The user's Brain in Echo: their calls, mail, Teams channels and documents as cited facts, organised by client and by thread. {rules}"),
    }
}

/// Who runs this server (folder | call | routine), for the usage counts.
static WHO: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn answer(req: &Value) -> Option<Value> {
    let id = req.get("id").cloned()?; // a notification gets no answer
    let r = match req["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({"protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL), "capabilities": {"tools": {}},
                                  "serverInfo": {"name": "echo-brain", "version": env!("CARGO_PKG_VERSION")},
                                  "instructions": instructions()})),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => {
            let name = req["params"]["name"].as_str().unwrap_or("");
            super::advert::count(WHO.get().map(String::as_str).unwrap_or("other"), name);
            let args = req["params"]["arguments"].clone();
            Ok(match call(name, &args) {
                Ok(v) => json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": v, "isError": false}),
                Err(e) => json!({"content": [{"type": "text", "text": json!({"error": format!("{e:#}"), "how_to_fix": "Run `echoctl brain status`; set the embedding server in Settings → Brain; `echoctl brain sync`."}).to_string()}], "isError": true}),
            })
        }
        m => Err(json!({"code": -32601, "message": format!("no method {m}")})),
    };
    Some(match r {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    })
}

/// Serve until stdin closes.
pub fn serve(access: Option<super::access::Access>, who: Option<String>) -> Result<()> {
    if let Some(a) = access {
        let _ = ACCESS.set(a);
    }
    let _ = WHO.set(who.filter(|w| ["folder", "call", "routine"].contains(&w.as_str())).unwrap_or_else(|| "other".into()));
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) => answer(&req),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}})),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speaks_mcp_with_brains_tool_names() {
        // Its own empty data folder: no numbers built, whatever other tests or this computer have.
        let (_env, _root) = crate::paths::testenv::folders("mcp-names");
        let init = answer(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26"}})).unwrap();
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert!(answer(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let names: Vec<String> = answer(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap()["result"]["tools"]
            .as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
        // kb recognises a Brain by this set.
        for n in ["health", "search_knowledge", "get_metric", "get_taxonomy", "get_evidence", "find_related_content", "list_metrics"] {
            assert!(names.contains(&n.to_string()), "{n}");
        }
        let r = answer(&json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "get_metric", "arguments": {"name": "x"}}})).unwrap();
        assert_eq!(r["result"]["structuredContent"]["status"], "not_modeled");
        assert_eq!(answer(&json!({"jsonrpc": "2.0", "id": 4, "method": "nope"})).unwrap()["error"]["code"], -32601);
    }
}

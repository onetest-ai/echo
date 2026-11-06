//! What the Echo window asks of Knowledge (spec §9): JSON routes under `/api/kn`, the `kn_*` actions, the folder the
//! user picked in the system's dialog (the page never sends a path of its own), and the jobs the page waits on.
//! Kept apart from `window.rs` so it is tested without a window.

#![cfg_attr(not(feature = "tray"), allow(dead_code))]

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

/// A route's answer: JSON, or a file's bytes with their type.
pub enum Reply {
    Json(Value),
    Bytes(Vec<u8>, &'static str),
}

use std::path::PathBuf;
#[cfg(test)]
use std::path::Path;
use std::sync::Mutex;

use super::{checks, export, links, page, scan, starter, Vault};

fn query_param(q: &str, key: &str) -> Option<String> {
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| links::percent_decode(&v.replace('+', " ")))
    })
}

fn vault_of(id: &str) -> Result<Vault> {
    if id.is_empty() || !id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        bail!("no such vault");
    }
    super::get(id)
}

/// A vault as the switcher and Settings show it: its JSON, with its page count when its folder is there.
fn summary(v: &Vault) -> Value {
    let mut j = super::vault_value(v);
    let pages = if super::available(v) { scan::index(v).map(|ix| ix.pages.len()).ok() } else { None };
    j["pages"] = json!(pages);
    j
}

/// Knowledge's state: the vaults, whether Recall is on, the data folder (where new vaults go), the Obsidian plugin's
/// vaults not connected yet.
pub fn state() -> Value {
    let vaults: Vec<Value> = super::list().unwrap_or_default().iter().map(summary).collect();
    #[cfg(feature = "brain")]
    let recall = crate::brain::enabled();
    #[cfg(not(feature = "brain"))]
    let recall = false;
    json!({
        "vaults": vaults,
        "recall": recall,
        "home": crate::paths::home().join("knowledge"),
        "default_profile": crate::ai::default_id(),
        "plugin_vaults": super::plugin_vaults_waiting().len(),
        "git": super::git::program().is_some(),
    })
}

/// A page path in the page's answer: a vault-relative path the vault has.
fn rel_param(q: &str) -> Result<String> {
    query_param(q, "path").filter(|p| !p.is_empty()).ok_or_else(|| anyhow!("which page?"))
}

pub fn route(rest: &str, query: &str) -> Result<Reply> {
    if rest == "detect" {
        bail!("only a folder picked in Echo's dialog");
    }
    if rest == "pick" {
        return Ok(Reply::Json(picked()));
    }
    if let Some(since) = rest.strip_prefix("jobs") {
        let _ = since;
        return Ok(Reply::Json(jobs(query_param(query, "since").and_then(|s| s.parse().ok()).unwrap_or(0))));
    }
    let (id, what) = rest.split_once('/').ok_or_else(|| anyhow!("no such route"))?;
    let v = vault_of(id)?;
    if !super::available(&v) && what != "settings" {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    let to_vault = |p: &str| scan::vault_rel(&v, p);
    // An Ask conversation's events after `from` (spec K12-06): at once when there are some, else when some come or
    // after 25 s (a long poll, as `/api/pty`).
    #[cfg(feature = "tray")]
    if let Some(conversation) = what.strip_prefix("chat/") {
        let from = query_param(query, "from").and_then(|f| f.parse().ok()).unwrap_or(0);
        let c = super::chat::read(&v.id, conversation, from, std::time::Duration::from_secs(25)).ok_or_else(|| anyhow!("no such conversation"))?;
        return Ok(Reply::Json(json!({ "events": c.events, "next": c.next, "live": c.live, "busy": c.busy, "waiting": c.waiting })));
    }
    Ok(Reply::Json(match what {
        "tree" => {
            let ix = scan::index(&v)?;
            let listed = checks::index_links(&v, &ix);
            let scope = checks::in_scope(&v);
            let pages: Vec<Value> = ix.pages.values().map(|p| json!({
                "path": to_vault(&p.path), "title": p.title, "tags": p.tags, "mtime": p.mtime,
                "in_index": listed.as_ref().filter(|_| scope(&p.path)).map(|l| l.contains(&p.path)),
            })).collect();
            json!({ "rev": ix.rev, "root": v.path("pages"), "index": v.path("index"), "notes": v.path("notes"), "pages": pages })
        }
        "rev" => json!({ "rev": scan::rev(&v) }),
        // Ask's conversations in the vault.
        #[cfg(feature = "tray")]
        "chats" => super::chat::list_value(&v),
        "page" => page::read(&v, &rel_param(query)?)?,
        "find" => {
            let ix = scan::index(&v)?;
            let q = query_param(query, "q").unwrap_or_default();
            let hits: Vec<Value> = scan::search(&v, &ix, &q, 30).into_iter().map(|mut h| { h["path"] = json!(to_vault(h["path"].as_str().unwrap_or(""))); h }).collect();
            json!({ "q": q, "hits": hits })
        }
        "graph" => {
            let ix = scan::index(&v)?;
            let index_page = v.path("index").and_then(|i| scan::page_rel(&v, &i));
            let mut g = links::graph(&ix, index_page.as_deref(), &query_param(query, "mode").unwrap_or_else(|| "links".into()));
            let fix = |x: &Value| if x.as_str().is_some_and(|s| s.starts_with('#')) { x.clone() } else { json!(to_vault(x.as_str().unwrap_or(""))) };
            if let Some(nodes) = g["nodes"].as_array_mut() {
                for n in nodes.iter_mut() { let id = fix(&n["id"]); n["id"] = id; }
            }
            if let Some(edges) = g["edges"].as_array_mut() {
                for e in edges.iter_mut() { *e = json!([fix(&e[0]), fix(&e[1])]); }
            }
            if let Some(h) = g["hubs"].as_array_mut() {
                for x in h.iter_mut() { *x = fix(x); }
            }
            g
        }
        "checks" => {
            let ix = scan::index(&v)?;
            let c = checks::run(&v, &ix);
            json!({
                "not_in_index": c.not_in_index.iter().map(|p| to_vault(p)).collect::<Vec<_>>(),
                "broken": c.broken.iter().map(|(p, l)| json!([to_vault(p), l])).collect::<Vec<_>>(),
                "isolated": c.isolated.iter().map(|p| to_vault(p)).collect::<Vec<_>>(),
                "no_props": c.no_props.iter().map(|p| to_vault(p)).collect::<Vec<_>>(),
                "unknown_tags": c.unknown_tags.iter().map(|(p, t)| json!([to_vault(p), t])).collect::<Vec<_>>(),
                "meeting_tag_missing": c.meeting_tag_missing, "no_index": c.no_index,
            })
        }
        "rows" => {
            let paths: Vec<String> = query_param(query, "paths").and_then(|p| serde_json::from_str(&p).ok()).unwrap_or_default();
            json!({ "rows": page::index_rows(&v, &paths)? })
        }
        "history" => page::history(&v, &rel_param(query)?),
        "version" => page::version(&v, &rel_param(query)?, &query_param(query, "rev").unwrap_or_default())?,
        "asset" => {
            let (b, mime) = page::asset(&v, &rel_param(query)?)?;
            return Ok(Reply::Bytes(b, mime));
        }
        "sample" => export::sample(&v.id)?,
        // The file *Use /twin from any Claude Code session* would write, shown before it is (K2-10).
        "twin-text" => {
            let name = query_param(query, "name").filter(|n| !n.is_empty()).unwrap_or_else(|| starter::global_twin_name(&v));
            json!({ "name": name, "text": starter::twin_skill_text(&v, &name), "own": super::join(&v.root, ".claude/skills/twin/SKILL.md").is_file() })
        }
        "settings" => {
            let tag_file = checks::tag_taxonomy(&v);
            let tags = tag_file.as_ref().and_then(|t| std::fs::read_to_string(super::join(&v.root, t)).ok()).map(|t| checks::tags_in_order(&t).len());
            let terms = v.detected.taxonomies.iter().find(|t| t.to_ascii_uppercase().contains("TERM")).and_then(|t| std::fs::read_to_string(super::join(&v.root, t)).ok())
                .map(|t| t.lines().filter(|l| l.starts_with('|') && !l.contains("---") ).count().saturating_sub(1));
            let ix = if super::available(&v) { scan::index(&v).ok() } else { None };
            let index_bytes = v.path("index").and_then(|i| std::fs::metadata(super::join(&v.root, &i)).ok()).map(|m| m.len());
            let cache = super::cache_dir(&v.id).join("index.json");
            json!({
                "vault": summary(&v),
                "skills": v.detected.skills,
                "has_twin": v.detected.skills.iter().any(|s| s.name == "twin" && s.kind == "skill"),
                "global_twin": starter::global_twin_installed(&v),
                "global_twin_name": starter::global_twin_name(&v),
                "tags": tags, "terms": terms, "tag_file": tag_file,
                "meeting_tag": checks::meeting_tag(&v),
                "index_bytes": index_bytes,
                "pages": ix.as_ref().map(|i| i.pages.len()),
                "cache_bytes": std::fs::metadata(&cache).map(|m| m.len()).ok(),
                "past": export::past(&v.id).ok(),
                "templates": v.path("templates").map(|t| {
                    let mut f: Vec<String> = std::fs::read_dir(super::join(&v.root, &t)).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.to_ascii_lowercase().ends_with(".md")).map(|n| format!("{t}/{n}")).collect()).unwrap_or_default();
                    f.sort();
                    f
                }).unwrap_or_default(),
                "in_home": dunce::canonicalize(&v.root).ok().zip(dunce::canonicalize(crate::paths::home()).ok()).is_some_and(|(r, h)| r.starts_with(h)),
            })
        }
        "preview" => export::preview(&crate::calls::resolve_call(&query_param(query, "call").unwrap_or_default())?, &v.id, query_param(query, "stem").as_deref().filter(|s| !s.is_empty()))?,
        _ => bail!("no such route"),
    }))
}

// ── the folder picked in the system's dialog ──

struct Pick {
    seq: u64,
    path: Option<PathBuf>,
    busy: bool,
}

static PICK: Mutex<Pick> = Mutex::new(Pick { seq: 0, path: None, busy: false });

/// Open the system's folder dialog (on a thread of its own); the page polls `picked()`.
pub fn pick(prompt: &'static str) {
    {
        let mut p = PICK.lock().unwrap_or_else(|e| e.into_inner());
        if p.busy {
            return;
        }
        p.busy = true;
    }
    std::thread::spawn(move || {
        let dir = crate::app::choose_folder(prompt).map(|d| dunce::canonicalize(&d).unwrap_or(d));
        let mut p = PICK.lock().unwrap_or_else(|e| e.into_inner());
        p.busy = false;
        if dir.is_some() {
            p.seq += 1;
            p.path = dir;
        }
    });
}

#[cfg(test)]
pub fn set_picked_for_test(path: &Path) {
    let mut p = PICK.lock().unwrap_or_else(|e| e.into_inner());
    p.seq += 1;
    p.path = Some(dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
}

fn picked_path(seq: u64) -> Result<PathBuf> {
    let p = PICK.lock().unwrap_or_else(|e| e.into_inner());
    match &p.path {
        Some(path) if p.seq == seq => Ok(path.clone()),
        _ => bail!("choose the folder again"),
    }
}

/// The folder picked last: what connecting it would find, and what's in the way.
pub fn picked() -> Value {
    let (seq, path, busy) = {
        let p = PICK.lock().unwrap_or_else(|e| e.into_inner());
        (p.seq, p.path.clone(), p.busy)
    };
    let Some(path) = path else { return json!({ "seq": seq, "busy": busy, "path": null }) };
    let empty = std::fs::read_dir(&path).map(|rd| rd.flatten().all(|e| e.file_name() == ".DS_Store" || e.file_name() == "Thumbs.db")).unwrap_or(false);
    let detected = super::detect::detect(&path).ok();
    let git = super::git::info(&path);
    let index_bytes = detected.as_ref().and_then(|d| d.index.as_ref()).and_then(|i| std::fs::metadata(super::join(&path, i)).ok()).map(|m| m.len());
    let refused = super::refuse_add(&path).err().map(|e| format!("{e:#}"));
    json!({ "seq": seq, "busy": busy, "path": path, "name": path.file_name().map(|n| n.to_string_lossy().into_owned()), "empty": empty, "detected": detected, "git": git, "index_bytes": index_bytes, "refused": refused })
}

// ── jobs: an action the page waits on, run on a thread of its own ──

static JOBS: Mutex<(u64, Vec<Value>)> = Mutex::new((0, Vec::new()));

/// Run `f` on a thread; its result is kept for the page (`jobs`).
pub fn start_job(op: &str, f: impl FnOnce() -> Result<Value> + Send + 'static) -> u64 {
    let seq = {
        let mut j = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        j.0 += 1;
        let seq = j.0;
        j.1.push(json!({ "seq": seq, "op": op, "done": false }));
        // The last 50 only.
        let n = j.1.len();
        if n > 50 {
            j.1.drain(..n - 50);
        }
        seq
    };
    std::thread::spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| Err(anyhow!("it failed")));
        let mut j = JOBS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(x) = j.1.iter_mut().find(|x| x["seq"] == seq) {
            x["done"] = json!(true);
            match r {
                Ok(v) => x["value"] = v,
                Err(e) => x["error"] = json!(format!("{e:#}")),
            }
        }
    });
    seq
}

/// Jobs after `since`.
pub fn jobs(since: u64) -> Value {
    let j = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    json!({ "last": j.0, "jobs": j.1.iter().filter(|x| x["seq"].as_u64().is_some_and(|s| s > since)).cloned().collect::<Vec<_>>() })
}

// ── actions ──

fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

/// Run a `kn_*` action; its answer (or error) for the page.
pub fn act(cmd: &str, v: &Value) -> Result<Value> {
    super::refuse_headless()?;
    match cmd {
        "kn_add" => {
            let path = picked_path(v["pick"].as_u64().unwrap_or(0))?;
            let vault = super::add(&path, super::AddOpts {
                name: Some(s(v, "name").to_string()).filter(|n| !n.trim().is_empty()),
                profile: Some(s(v, "profile").to_string()).filter(|p| !p.is_empty()),
                minutes: Some(s(v, "minutes").to_string()).filter(|m| !m.is_empty()),
                commit: v["commit"].as_bool(),
                template: None,
            })?;
            for (k, val) in v["overrides"].as_object().into_iter().flatten() {
                super::set(&vault.id, k, val.as_str().unwrap_or(""))?;
            }
            Ok(json!({ "vault": summary(&super::get(&vault.id)?) }))
        }
        "kn_new" => {
            let folder = match v["pick"].as_u64() {
                Some(seq) => Some(picked_path(seq)?),
                None => None,
            };
            let c = starter::create(s(v, "name"), folder, v["git"].as_bool().unwrap_or(true), if s(v, "minutes").is_empty() { "auto" } else { s(v, "minutes") })?;
            Ok(starter::created_value(&c))
        }
        "kn_set" => Ok(json!({ "vault": summary(&super::set(s(v, "id"), s(v, "key"), &match &v["value"] { Value::String(x) => x.clone(), Value::Bool(b) => b.to_string(), Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(","), other => other.to_string() })?) })),
        "kn_remove" => {
            super::remove(s(v, "id"))?;
            scan::forget(s(v, "id"));
            Ok(json!({ "removed": s(v, "id") }))
        }
        "kn_reread" => {
            scan::forget(s(v, "id"));
            Ok(json!({ "vault": summary(&super::reread(s(v, "id"))?) }))
        }
        "kn_preview" => Ok(json!({ "path": s(v, "path"), "html": page::render(&vault_of(s(v, "id"))?, s(v, "path"), s(v, "text"))? })),
        "kn_save_page" => page::save(&vault_of(s(v, "id"))?, s(v, "path"), s(v, "text"), s(v, "base"), v["message"].as_str()),
        "kn_new_page" => {
            let vault = vault_of(s(v, "id"))?;
            let template = v["template"].as_str().filter(|t| !t.is_empty());
            Ok(json!({ "path": page::create(&vault, s(v, "folder"), s(v, "name"), template, v["row"].as_bool().unwrap_or(true))? }))
        }
        "kn_restore" => page::restore(&vault_of(s(v, "id"))?, s(v, "path"), s(v, "rev")),
        "kn_index_add" => {
            let paths: Vec<String> = v["paths"].as_array().into_iter().flatten().filter_map(|p| p.as_str().map(str::to_owned)).collect();
            page::index_add(&vault_of(s(v, "id"))?, &paths)
        }
        "kn_save_minutes" => {
            let call = crate::calls::resolve_call(s(v, "call"))?;
            if s(v, "vault").is_empty() {
                return Ok(json!({ "saved": export::save_by_rules(&call) }));
            }
            let how = export::How::Manual(export::Manual {
                new: v["new"].as_bool().unwrap_or(false),
                replace: v["replace"].as_bool().unwrap_or(false),
                anyway: v["anyway"].as_bool().unwrap_or(false),
                stem: v["stem"].as_str().map(str::to_owned).filter(|x| !x.trim().is_empty()),
                transcript: v["transcript"].as_bool(),
                chat: v["chat"].as_bool(),
            });
            match export::save(&call, s(v, "vault"), how) {
                Ok(saved) => Ok(serde_json::to_value(saved)?),
                Err(e) => match e.downcast_ref::<export::ProfileMismatch>() {
                    Some(m) => Ok(json!({ "mismatch": { "call": m.call, "vault": m.vault } })),
                    None => Err(e),
                },
            }
        }
        "kn_call_choice" => {
            let call = crate::calls::resolve_call(s(v, "call"))?;
            export::choose(&call, v["vault"].as_str().filter(|x| !x.is_empty()))?;
            Ok(json!({ "ok": true }))
        }
        "kn_twin" => {
            let vault = vault_of(s(v, "id"))?;
            match s(v, "op") {
                "add" => Ok(json!({ "added": starter::add_twin_skill(&vault)? })),
                "global" => Ok(json!({ "installed": starter::global_twin(&vault, v["name"].as_str().filter(|n| !n.is_empty()))? })),
                "global_remove" => Ok(json!({ "removed": starter::global_twin_remove(&vault)? })),
                _ => bail!("add, global or global_remove"),
            }
        }
        "kn_past" => Ok(if v["apply"].as_bool().unwrap_or(false) { export::save_past(s(v, "id"))? } else { export::past(s(v, "id"))? }),
        "kn_import_plugin" => Ok(json!({ "connected": super::import_plugin_vaults()? })),
        // Ask (spec K12-07, K12-09, K12-12): a message (a new conversation when none is named), Stop, and a conversation
        // taken to the terminal panel. Only pages are named in a message here (attached calls are K12-10's).
        #[cfg(feature = "tray")]
        "kn_chat_send" => {
            let context = v["context"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().map(|x| x["page"].as_str().map(str::to_owned).ok_or_else(|| anyhow!("only pages can be named in a message"))).collect::<Result<Vec<_>>>()?;
            let mode = super::chat::Mode::parse(s(v, "mode"))?;
            let id = super::chat::send(s(v, "id"), v["conversation"].as_str().filter(|c| !c.is_empty()), s(v, "text"), mode, &context)?;
            Ok(json!({ "conversation": id }))
        }
        #[cfg(feature = "tray")]
        "kn_chat_stop" => {
            super::chat::stop(s(v, "conversation"));
            Ok(json!({ "stopped": true }))
        }
        #[cfg(feature = "tray")]
        "kn_chat_terminal" => super::chat::to_terminal(s(v, "id"), s(v, "conversation")),
        other => bail!("no action {other}"),
    }
}

/// A file of a vault the window may reveal in Finder / Explorer or open in Obsidian (checked like any page path).
pub fn file_of(id: &str, rel: &str) -> Result<PathBuf> {
    let v = vault_of(id)?;
    if rel.is_empty() {
        return Ok(v.root);
    }
    page::inside(&v, rel)
}

pub fn obsidian_uri(id: &str, rel: &str) -> Result<String> {
    Ok(page::obsidian_uri(&file_of(id, rel)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{self, fixture, AddOpts};

    fn json(rest: &str, q: &str) -> Value {
        match route(rest, q) {
            Ok(Reply::Json(v)) => v,
            Ok(Reply::Bytes(..)) => panic!("bytes"),
            Err(e) => panic!("{rest}?{q}: {e:#}"),
        }
    }

    fn vault(root: &std::path::Path) -> knowledge::Vault {
        let t = fixture::twin(&root.join("twin"));
        knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
    }

    #[test]
    fn the_twin_sheet_shows_the_file_it_installs() {
        let (_env, root) = crate::paths::testenv::folders("kn-api-twin");
        std::env::set_var("ECHOCTL_CLAUDE_HOME", root.join("claude-home"));
        let c = starter::create("Team wiki", Some(root.join("wiki")), false, "auto").unwrap();
        let shown = json(&format!("{}/twin-text", c.vault.id), "name=team-notes");
        assert_eq!(shown["name"], "team-notes");
        let installed = starter::global_twin(&c.vault, Some("team-notes")).unwrap();
        assert_eq!(shown["text"].as_str().unwrap(), std::fs::read_to_string(&installed).unwrap(), "what is shown is what is written");
        assert_eq!(json(&format!("{}/twin-text", c.vault.id), "")["name"], "team-wiki-twin", "the default name");
        std::env::remove_var("ECHOCTL_CLAUDE_HOME");
    }

    #[test]
    fn kn_routes_answer_for_a_fixture_vault() {
        let (_env, root) = crate::paths::testenv::folders("kn-api-routes");
        let v = vault(&root);
        let s = state();
        assert_eq!(s["vaults"][0]["id"], v.id);
        assert_eq!(s["vaults"][0]["pages"], 7);
        assert!(s.get("recall").is_some() && s.get("home").is_some());
        let tree = json(&format!("{}/tree", v.id), "");
        let paths: Vec<&str> = tree["pages"].as_array().unwrap().iter().map(|p| p["path"].as_str().unwrap()).collect();
        assert!(paths.contains(&"knowledge/clients/acme.md") && paths.contains(&"knowledge/CATALOG.md"));
        assert_eq!(tree["index"], "knowledge/CATALOG.md");
        let in_index = |path: &str| tree["pages"].as_array().unwrap().iter().find(|p| p["path"] == path).unwrap()["in_index"].clone();
        assert_eq!(in_index("knowledge/notes/2026-01-03-c.md"), false);
        assert_eq!(in_index("knowledge/clients/acme.md"), true);
        for p in ["knowledge/CATALOG.md", "knowledge/TAG_TAXONOMY.md", "knowledge/notes/README.md"] {
            assert!(in_index(p).is_null(), "the index doesn't list {p}, as the checks say");
        }
        assert!(tree["rev"].as_str().is_some_and(|r| !r.is_empty()));
        assert_eq!(json(&format!("{}/rev", v.id), "")["rev"], tree["rev"]);
        let page = json(&format!("{}/page", v.id), "path=knowledge%2Fclients%2Facme.md");
        assert_eq!(page["title"], "Acme Corp");
        let found = json(&format!("{}/find", v.id), "q=acme");
        assert_eq!(found["hits"][0]["path"], "knowledge/clients/acme.md");
        let g = json(&format!("{}/graph", v.id), "mode=links");
        assert!(g["nodes"].as_array().unwrap().iter().any(|n| n["id"] == "knowledge/clients/acme.md"));
        assert_eq!(g["hubs"], serde_json::json!(["knowledge/CATALOG.md"]));
        let c = json(&format!("{}/checks", v.id), "");
        assert_eq!(c["not_in_index"], serde_json::json!(["knowledge/notes/2026-01-03-c.md"]));
        assert_eq!(json(&format!("{}/history", v.id), "path=knowledge%2Fclients%2Facme.md")["no_history"], true);
        let settings = json(&format!("{}/settings", v.id), "");
        assert_eq!(settings["skills"].as_array().unwrap().len(), 2);
        assert!(settings.get("global_twin").is_some());
    }

    #[test]
    fn kn_paths_from_the_page_are_checked() {
        let (_env, root) = crate::paths::testenv::folders("kn-api-paths");
        let v = vault(&root);
        for q in ["path=..%2F..%2Fetc%2Fpasswd", "path=%2Fetc%2Fpasswd", "path=.git%2Fconfig"] {
            assert!(route(&format!("{}/page", v.id), q).is_err(), "{q}");
            assert!(route(&format!("{}/asset", v.id), q).is_err(), "{q}");
        }
        assert!(route("nope/tree", "").is_err());
        assert!(route("../x/tree", "").is_err());
        // The page can't make Echo look at a folder of its choosing.
        assert!(route("detect", "path=%2FUsers").is_err());
    }

    #[test]
    fn actions_change_vaults_and_pages() {
        let (_env, root) = crate::paths::testenv::folders("kn-api-acts");
        let v = vault(&root);
        let r = act("kn_set", &json!({"id": v.id, "key": "minutes", "value": "ask"})).unwrap();
        assert_eq!(r["vault"]["minutes"]["mode"], "ask");
        let p = json(&format!("{}/page", v.id), "path=knowledge%2Fclients%2Facme.md");
        let saved = act("kn_save_page", &json!({"id": v.id, "path": "knowledge/clients/acme.md", "text": "# Acme\n", "base": p["base"]})).unwrap();
        assert_eq!(saved["path"], "knowledge/clients/acme.md");
        let again = act("kn_save_page", &json!({"id": v.id, "path": "knowledge/clients/acme.md", "text": "# Again\n", "base": p["base"]})).unwrap_err().to_string();
        assert!(again.contains("changed_on_disk"), "{again}");
        let pv = act("kn_preview", &json!({"id": v.id, "path": "knowledge/clients/acme.md", "text": "# T\n\nSee [[2026-01-01-a]] <b>x</b>"})).unwrap();
        assert!(pv["html"].as_str().unwrap().contains("#kn:knowledge/notes/2026-01-01-a.md") && pv["html"].as_str().unwrap().contains("&lt;b&gt;"));
        let made = act("kn_new_page", &json!({"id": v.id, "folder": "knowledge/clients", "name": "Beta Inc", "row": true})).unwrap();
        assert_eq!(made["path"], "knowledge/clients/beta-inc.md");
        assert!(act("kn_remove", &json!({"id": v.id})).is_ok());
        assert!(knowledge::list().unwrap().is_empty());
        assert!(act("kn_nope", &json!({})).is_err());
    }

    #[test]
    fn a_picked_folder_is_connected_without_its_path_from_the_page() {
        let (_env, root) = crate::paths::testenv::folders("kn-api-pick");
        let t = fixture::twin(&root.join("twin"));
        set_picked_for_test(&t);
        let p = picked();
        assert_eq!(p["detected"]["index"], "knowledge/CATALOG.md");
        assert_eq!(p["empty"], false);
        let seq = p["seq"].as_u64().unwrap();
        let r = act("kn_add", &json!({"pick": seq, "name": "Twin", "minutes": "auto", "commit": false})).unwrap();
        assert_eq!(r["vault"]["id"], "twin");
        assert!(act("kn_add", &json!({"pick": seq + 7, "name": "X"})).is_err(), "only the folder picked");
        // An empty folder: the starter, there.
        std::fs::create_dir_all(root.join("empty")).unwrap();
        set_picked_for_test(&root.join("empty"));
        let e = picked();
        assert_eq!(e["empty"], true);
        let n = act("kn_new", &json!({"name": "Fresh", "pick": e["seq"], "git": false, "minutes": "auto"})).unwrap();
        assert_eq!(n["vault"]["id"], "fresh");
        assert!(root.join("empty/knowledge/CATALOG.md").is_file());
    }

    #[cfg(all(unix, feature = "tray"))]
    #[test]
    fn chat_routes_and_actions() {
        let (_env, root) = crate::paths::testenv::folders("kn-api-chat");
        let _fake = crate::ai::fake::install(&root);
        knowledge::chat::reset();
        let p = crate::ai::fake::profile("w", None, &crate::ai::fake::signed_in("pat@example.com"));
        std::fs::write(crate::ai::fake::file(&p, "chat.jsonl"), knowledge::chat::turn("Noted.")).unwrap();
        let t = fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), profile: Some("w".into()), ..Default::default() }).unwrap();
        let sent = act("kn_chat_send", &json!({"id": v.id, "text": "What is new at Acme?", "mode": "ask", "context": [{"page": "knowledge/clients/acme.md"}]})).unwrap();
        let id = sent["conversation"].as_str().unwrap().to_string();
        knowledge::chat::turn_events(&v.id, &id);
        let c = json(&format!("{}/chat/{id}", v.id), "from=0");
        assert_eq!(c["events"][0]["type"], "user");
        assert_eq!(c["events"][0]["context"][0]["page"], "knowledge/clients/acme.md");
        assert!(c["next"].as_u64().unwrap() >= 3 && c["live"] == true && c["busy"] == false && c["waiting"] == false, "{c}");
        assert!(json(&format!("{}/chats", v.id), "").as_array().unwrap().iter().any(|x| x["id"] == id.as_str()));
        assert!(route(&format!("{}/chat/nope", v.id), "from=0").is_err());
        assert!(act("kn_chat_send", &json!({"id": v.id, "text": "Hi", "mode": "shell"})).is_err());
        assert!(act("kn_chat_stop", &json!({"conversation": id})).is_ok());
        let moved = act("kn_chat_terminal", &json!({"id": v.id, "conversation": id})).unwrap();
        assert_eq!(moved["conversation"], id.as_str());
        assert!(!knowledge::chat::live().contains(&id));
        knowledge::chat::reset();
    }

    #[test]
    fn jobs_report_once_done() {
        let seq = start_job("kn_test", || Ok(json!({"x": 1})));
        crate::paths::testenv::eventually("the job ends", || jobs(seq - 1)["jobs"].as_array().is_some_and(|a| a.iter().any(|j| j["seq"] == seq && j["done"] == true)));
        let j = jobs(seq - 1);
        let mine = j["jobs"].as_array().unwrap().iter().find(|j| j["seq"] == seq).unwrap();
        assert_eq!(mine["value"]["x"], 1);
        let failing = start_job("kn_test", || Err(anyhow!("it broke")));
        crate::paths::testenv::eventually("the job fails", || jobs(failing - 1)["jobs"].as_array().is_some_and(|a| a.iter().any(|j| j["seq"] == failing && j["done"] == true)));
        let f = jobs(failing - 1)["jobs"].as_array().unwrap().iter().find(|j| j["seq"] == failing).unwrap().clone();
        assert_eq!(f["error"], "it broke");
    }
}


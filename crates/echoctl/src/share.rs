//! What Echo shares with a plugin that asked for it at install (`uses.echo.read` / `write`, `ctx.echo`): its sources
//! — a call's minutes and transcript, a mail conversation's or channel post's facts from Recall —, the list of them,
//! people, the dictionary; and the dictionary's one write (names added, never removed). Plus the events that say a
//! source is new or changed (`minutes_written`, `sources_changed`). A source's id: `call:<folder>`, `mail:<Recall
//! source>` (`mail/…`, `channel/…`).

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::path::Path;

/// A call's source id and link.
fn call_id(dir: &Path) -> (String, String) {
    let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
    (format!("call:{name}"), format!("echo://call/{name}"))
}

/// A thread in a Recall document's refs: `{id, …}` (or a bare id, older).
#[cfg(feature = "brain")]
fn ref_thread(t: &Value) -> Option<&str> {
    t["id"].as_str().or(t.as_str())
}

fn threads_of_call(dir: &Path) -> Vec<Value> {
    crate::projects::of_call(dir).unwrap_or_default().into_iter().map(|(p, _)| json!({"id": p.id, "name": p.name, "client": p.client})).collect()
}

/// One source in the shape a plugin's ingest takes: `{id, kind, title, date, text, link, threads, profile,
/// transcript?}` — `profile` is the AI profile its text may go to (strict routing: a call's own, a mail's threads').
pub fn source(id: &str) -> Result<Value> {
    if let Some(name) = id.strip_prefix("call:") {
        anyhow::ensure!(crate::links::Link::parse(&format!("echo://call/{name}")).is_some(), "not a call: {id}");
        let dir = crate::calls::resolve_call(name).map_err(|_| anyhow!("no call {name:?}"))?;
        let meta = crate::calls::read_json(&dir.join("meta.json"));
        let text = std::fs::read_to_string(dir.join("notes.md")).map_err(|_| anyhow!("{name}: no minutes yet"))?;
        let (id, link) = call_id(&dir);
        return Ok(json!({
            "id": id, "kind": if meta["kind"] == "memo" { "memo" } else { "call" },
            "title": meta["title"], "date": meta["started_at"].as_str().and_then(|s| s.get(..10)),
            "text": text, "link": link, "threads": threads_of_call(&dir),
            "profile": crate::ai::call_profile_id(&dir).ok(),
            "transcript": std::fs::read_to_string(dir.join("transcript.md")).ok(),
        }));
    }
    if let Some(src) = id.strip_prefix("mail:") {
        return mail_source(src);
    }
    bail!("not a source id Echo knows: {id} (call:<folder> | mail:<source>)")
}

#[cfg(feature = "brain")]
fn mail_source(src: &str) -> Result<Value> {
    anyhow::ensure!(matches!(crate::links::Link::parse(&format!("echo://mail/{src}")), Some(crate::links::Link::Mail(_))), "not a mail or post: {src}");
    let c = crate::brain::store::open(&crate::brain::store_path())?;
    let (kind, title, date, refs): (String, Option<String>, Option<String>, Option<String>) = c
        .query_row("SELECT kind, title, event_date, refs FROM documents WHERE source = ?1", [src], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|_| anyhow!("no {src:?} in Recall"))?;
    let refs: Value = refs.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    let text: Vec<String> = c.prepare("SELECT text FROM chunks WHERE source = ?1 ORDER BY ord")?.query_map([src], |r| r.get(0))?.collect::<Result<_, _>>()?;
    let all = crate::projects::list();
    let threads: Vec<Value> = refs["threads"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|t| all.iter().find(|p| Some(p.id.as_str()) == ref_thread(t)))
        .map(|p| json!({"id": p.id, "name": p.name, "client": p.client}))
        .collect();
    // Its threads' profile when they share one (strict routing: never a guess between two).
    let default = crate::ai::default_id();
    let profiles: std::collections::BTreeSet<String> = threads
        .iter()
        .filter_map(|t| all.iter().find(|p| Some(p.id.as_str()) == t["id"].as_str()))
        .map(|p| p.ai_profile.clone().unwrap_or_else(|| default.clone()))
        .collect();
    Ok(json!({
        "id": format!("mail:{src}"), "kind": kind, "title": title, "date": date, "text": text.join("\n\n"),
        "link": format!("echo://mail/{src}"), "url": refs["url"], "threads": threads,
        "profile": if profiles.len() == 1 { profiles.into_iter().next() } else { None },
    }))
}

#[cfg(not(feature = "brain"))]
fn mail_source(src: &str) -> Result<Value> {
    bail!("{src}: this build has no Recall")
}

/// The sources, newest first: `{id, kind, title, date, threads}` — calls with minutes, and Recall's mail and channel
/// posts. `opts`: `kind` (call | mail | channel), `thread`, `since` (YYYY-MM-DD), `limit` (≤ 1000, default 200).
pub fn sources(opts: &Value) -> Result<Value> {
    let kind = opts["kind"].as_str().unwrap_or("");
    let thread = opts["thread"].as_str().filter(|t| !t.is_empty());
    let since = opts["since"].as_str().unwrap_or("");
    let limit = opts["limit"].as_u64().unwrap_or(200).clamp(1, 1000) as usize;
    let mut out: Vec<Value> = vec![];
    if kind.is_empty() || kind == "call" {
        for dir in crate::calls::all_calls() {
            if !dir.join("notes.md").exists() {
                continue;
            }
            let meta = crate::calls::read_json(&dir.join("meta.json"));
            let date = meta["started_at"].as_str().and_then(|s| s.get(..10)).unwrap_or("").to_string();
            if date.as_str() < since {
                continue;
            }
            let threads = threads_of_call(&dir);
            if thread.is_some_and(|t| !threads.iter().any(|x| x["id"] == t)) {
                continue;
            }
            out.push(json!({"id": call_id(&dir).0, "kind": "call", "title": meta["title"], "date": date, "threads": threads.iter().map(|t| t["id"].clone()).collect::<Vec<_>>()}));
        }
    }
    #[cfg(feature = "brain")]
    if kind.is_empty() || kind == "mail" || kind == "channel" {
        if let Ok(c) = crate::brain::store::open(&crate::brain::store_path()) {
            let mut st = c.prepare("SELECT source, kind, title, event_date, refs FROM documents WHERE kind IN ('mail', 'channel')")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?)))?;
            for (src, k, title, date, refs) in rows.flatten() {
                let date = date.unwrap_or_default();
                if (!kind.is_empty() && k != kind) || date.as_str() < since {
                    continue;
                }
                let refs: Value = refs.and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
                let threads: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(ref_thread).collect();
                if thread.is_some_and(|t| !threads.contains(&t)) {
                    continue;
                }
                out.push(json!({"id": format!("mail:{src}"), "kind": k, "title": title, "date": date, "threads": threads}));
            }
        }
    }
    out.sort_by(|a, b| b["date"].as_str().cmp(&a["date"].as_str()));
    out.truncate(limit);
    Ok(json!(out))
}

/// People across the threads: `{key, name, organisation, title, threads, link}` (People's own; roles stay there).
pub fn people() -> Result<Value> {
    let v = crate::people::everyone(chrono::Local::now().date_naive())?;
    Ok(json!(v["people"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["archived"] != true)
        .map(|x| {
            let key = x["key"].as_str().unwrap_or("");
            json!({"key": key, "name": x["name"], "organisation": x["org"]["name"], "title": x["title"], "threads": x["threads"],
                   "link": format!("echo://person/{}", key.replace('%', "%25").replace(' ', "%20"))})
        })
        .collect::<Vec<_>>()))
}

/// The dictionary: `{terms: [{name, synonyms, spellings, kind, group, client}]}` — `client` none = the practice's.
#[cfg(feature = "brain")]
pub fn dictionary() -> Result<Value> {
    let t = crate::brain::taxonomy::current().unwrap_or_default();
    Ok(json!({"terms": t.terms.iter().map(|x| json!({
        "name": x.name, "synonyms": x.synonyms, "spellings": x.variants, "kind": x.kind,
        "group": if x.group.is_empty() { crate::brain::taxonomy::group_of(&x.kind).to_string() } else { x.group.clone() },
        "client": (x.layer != "shared").then_some(&x.layer),
    })).collect::<Vec<_>>()}))
}

#[cfg(not(feature = "brain"))]
pub fn dictionary() -> Result<Value> {
    Ok(json!({"terms": []}))
}

/// Names a plugin adds to the dictionary: `{name, synonyms?, spellings?, kind?, client?}`. Merged into the term any
/// of its names already is (its other names added; nothing renamed or removed), else a new term in the client's
/// layer (or the practice's). Answers `{term, added: [names new to it], new}`.
#[cfg(feature = "brain")]
pub fn dictionary_add(plugin: &str, v: &Value) -> Result<Value> {
    let name = v["name"].as_str().map(str::trim).filter(|n| !n.is_empty() && n.chars().count() <= 80).ok_or_else(|| anyhow!("a term needs a name (≤ 80 characters)"))?;
    let words = |k: &str| -> Vec<String> {
        v[k].as_array().into_iter().flatten().filter_map(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty() && x.chars().count() <= 80).map(str::to_owned).take(20).collect()
    };
    let client = v["client"].as_str().filter(|c| crate::projects::clients().ok().is_some_and(|cs| cs.iter().any(|x| x.id == *c)));
    crate::brain::taxonomy::dictionary_add(name, &words("synonyms"), &words("spellings"), v["kind"].as_str(), client, &format!("plugin:{plugin}"))
}

#[cfg(not(feature = "brain"))]
pub fn dictionary_add(_plugin: &str, _v: &Value) -> Result<Value> {
    bail!("this build has no dictionary")
}

/// Tell the plugins a call's minutes are written (the app's run or a session's).
pub fn minutes_written(dir: &Path) {
    let (id, link) = call_id(dir);
    crate::jshost::event("minutes_written", &json!({"id": id, "link": link}));
}

/// Tell the plugins which of Recall's mail and channel posts are new, changed or gone after a sync.
pub fn sources_changed(touched: &[String], gone: &[String]) {
    let mine = |v: &[String]| v.iter().filter(|s| s.starts_with("mail/") || s.starts_with("channel/")).map(|s| format!("mail:{s}")).collect::<Vec<_>>();
    let (changed, removed) = (mine(touched), mine(gone));
    if !changed.is_empty() || !removed.is_empty() {
        crate::jshost::event("sources_changed", &json!({"changed": changed, "removed": removed}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_calls_source_is_its_minutes_with_its_transcript() {
        let (_env, _root) = crate::paths::testenv::folders("share-source");
        let d = crate::paths::calls_dir().join("2026-10-28_110000-aurora");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), r#"{"title": "Aurora weekly", "started_at": "2026-10-28T11:00:00+02:00"}"#).unwrap();
        assert!(source("call:2026-10-28_110000-aurora").is_err(), "no minutes yet");
        std::fs::write(d.join("notes.md"), "# Aurora weekly\n- SSO first [12:05]\n").unwrap();
        std::fs::write(d.join("transcript.md"), "**Lena Varga** [12:05]\nSSO has to land first.\n").unwrap();
        let s = source("call:2026-10-28_110000-aurora").unwrap();
        assert_eq!((s["kind"].as_str(), s["date"].as_str(), s["link"].as_str()), (Some("call"), Some("2026-10-28"), Some("echo://call/2026-10-28_110000-aurora")));
        assert!(s["text"].as_str().unwrap().contains("SSO first") && s["transcript"].as_str().unwrap().contains("Lena Varga"));
        let list = sources(&json!({})).unwrap();
        assert_eq!(list[0]["id"], "call:2026-10-28_110000-aurora");
        assert!(sources(&json!({"since": "2026-11-01"})).unwrap().as_array().unwrap().is_empty());
        for bad in ["call:../x", "mail:../x", "nope:x"] {
            assert!(source(bad).is_err(), "{bad}");
        }
    }

    #[cfg(feature = "brain")]
    #[test]
    fn plugins_add_names_never_take_them() {
        let (_env, _root) = crate::paths::testenv::folders("share-dict");
        let r = dictionary_add("obsidian", &json!({"name": "Signal Grid", "synonyms": ["SG"], "kind": "product"})).unwrap();
        assert_eq!(r["new"], true);
        // Another name of the same thing: merged, nothing renamed.
        let r = dictionary_add("obsidian", &json!({"name": "SG", "synonyms": ["SignalGrid"], "spellings": ["signal greed"]})).unwrap();
        assert_eq!((r["term"].as_str(), r["new"].as_bool()), (Some("Signal Grid"), Some(false)), "{r}");
        let d = dictionary().unwrap();
        let t = d["terms"].as_array().unwrap().iter().find(|t| t["name"] == "Signal Grid").unwrap().clone();
        assert_eq!(t["synonyms"], json!(["SG", "SignalGrid"]));
        assert_eq!(t["spellings"], json!(["signal greed"]));
        assert!(dictionary_add("obsidian", &json!({"name": ""})).is_err());
    }
}

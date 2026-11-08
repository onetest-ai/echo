//! The Brain: its pages and its background work (only with the `brain` feature).

use super::*;

/// The numbers lane's jobs running now (client → op), and the last error per client (`<client>:error`).
pub(super) static NUMBERS_BUSY: std::sync::LazyLock<std::sync::Mutex<std::collections::BTreeMap<String, Value>>> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::BTreeMap::new()));

/// Settings → Brain's Sync now: one at a time, in the background; its report for the page.
pub(super) static BRAIN_SYNC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static BRAIN_LAST: std::sync::Mutex<Value> = std::sync::Mutex::new(Value::Null);
/// Scopes whose insights are being refreshed from the window.
pub(super) static BRAIN_INSIGHTS: std::sync::Mutex<std::collections::BTreeSet<String>> = std::sync::Mutex::new(std::collections::BTreeSet::new());
/// Taxonomy edits running now, and the last one's error with its number (the page that made an edit shows an error
/// numbered after the one it saw when it asked).
pub(super) static BRAIN_EDITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(super) static BRAIN_EDIT_ERROR: std::sync::Mutex<(u64, String)> = std::sync::Mutex::new((0, String::new()));

/// A change to a project or a client from the page (`{cmd: "project", op, …}`).
/// Start a Brain sync in the background unless one runs (its outcome: `BRAIN_LAST`).
pub(super) fn brain_sync_now() {
    if !BRAIN_SYNC.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(|| {
            let r = match crate::brain::sync() {
                Ok(r) => {
                    // New sections get their topics, as after the scheduler's sync.
                    if crate::brain::EXTRAS && r.chunks > 0 && crate::brain::taxonomy::current().is_some() {
                        if let Err(e) = crate::brain::taxonomy::classify(None, false, &|_| {}) {
                            eprintln!("brain: tagging: {e:#}");
                        }
                    }
                    json!(r)
                }
                Err(e) => json!({ "error": format!("{e:#}") }),
            };
            if let Ok(mut l) = BRAIN_LAST.lock() {
                *l = r;
            }
            BRAIN_SYNC.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
}

/// A document in the app the system opens it with (a deck in PowerPoint or Keynote, a PDF in Preview).
pub(super) fn open_in_app(path: &Path) {
    if !path.is_file() {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").no_window().arg(path).spawn();
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        // Pickers for a thread's sources: the user's teams, a team's channels (Microsoft 365).
        // The numbers lane: per client its families (proposed / accepted), catalog, last build.
        "/api/brain/numbers" => json_reply({
            let mut v = crate::brain::numbers::overview().unwrap_or_else(|e| json!({ "error": format!("{e:#}") }));
            v["busy"] = NUMBERS_BUSY.lock().map(|b| json!(*b)).unwrap_or(Value::Null);
            v
        }),
        // The thread folders' files: what was read, how, and why the others weren't.
        "/api/brain/files" => json_reply(crate::brain::files::folder::status().unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))),
        // How each thread source was last read (`brain/sources.json`).
        "/api/brain/sources" => json_reply(json!({ "state": crate::brain::sources::state(), "senders": crate::brain::senders::view(), "signed_in": crate::graph::account().is_some(), "syncing": BRAIN_SYNC.load(std::sync::atomic::Ordering::SeqCst) })),
        // The Brain: its overview, graph and a call's "Also discussed" (?client= / ?thread= scope them).
        "/api/brain" => {
            let q = req.uri().query().unwrap_or("");
            let (client, thread) = (query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            let mut v = crate::brain::views::overview(scope).unwrap_or_else(|e| json!({ "error": format!("{e:#}") }));
            // Topic rebuilds and re-tagging running now (`brain_rebuild_topics`, `brain_retag_topics`), by layer.
            let mut jobs: Vec<String> = BRAIN_INSIGHTS.lock().map(|r| r.iter().filter(|k| k.starts_with("taxonomy:") || k.starts_with("retag:") || k.starts_with("split:")).cloned().collect()).unwrap_or_default();
            if BRAIN_EDITS.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                jobs.push("edit:taxonomy".into());
            }
            v["topic_jobs"] = json!(jobs);
            if let Ok(e) = BRAIN_EDIT_ERROR.lock() {
                v["edit_error"] = json!({"n": e.0, "text": e.1});
            }
            json_reply(v)
        }
        // The Brain a piece at a time (`brain::browse`): sources grouped and paged, a folder's files, the
        // dictionary by group and letter, the graph drawn as it is opened (topics; a topic's or a client's sources).
        p if p.starts_with("/api/brain/browse/") => {
            use crate::brain::browse;
            let q = req.uri().query().unwrap_or("");
            let (client, thread) = (query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            let num = |k: &str, d: usize| query_param(q, k).and_then(|v| v.parse().ok()).unwrap_or(d);
            let r = if !crate::brain::enabled() { Err(anyhow::anyhow!("the Brain is off")) } else { match &p["/api/brain/browse/".len()..] {
                "groups" => browse::source_groups(scope),
                "sources" => browse::source_list(scope, &browse::ListQuery {
                    client: query_param(q, "of"), kind: query_param(q, "kind"), root: query_param(q, "root"), channel: query_param(q, "channel"),
                    q: query_param(q, "q"), before: query_param(q, "before"), limit: num("limit", 50),
                }),
                "folder" => browse::folder_files(&query_param(q, "root").unwrap_or_default(), query_param(q, "q").as_deref(), num("offset", 0), num("limit", 50)),
                "terms" => browse::terms(scope, query_param(q, "group").as_deref(), query_param(q, "letter").as_deref(), query_param(q, "q").as_deref(), num("offset", 0), num("limit", 50)),
                "graph" => browse::graph_topics(scope),
                "graph-topic" => browse::graph_topic(&query_param(q, "id").unwrap_or_default(), scope, num("limit", 50)),
                "graph-client" => browse::graph_client(&query_param(q, "id").unwrap_or_default(), num("limit", 400)),
                "graph-search" => {
                    let emb = crate::brain::embedder();
                    browse::graph_search(query_param(q, "q").unwrap_or_default().trim(), scope, emb.as_ref().map(|e| e as &dyn crate::brain::embed::Embedder))
                }
                _ => Err(anyhow::anyhow!("not found")),
            } };
            json_reply(r.unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        // The Brain searched from its graphs (words and meaning, as Claude's search_knowledge): the hits'
        // sources light up. Off the main thread, as every echo:// request: the embedder may take a moment.
        "/api/brain/search" => {
            let q = req.uri().query().unwrap_or("");
            let (text, client, thread) = (query_param(q, "q").unwrap_or_default(), query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            let emb = crate::brain::embedder();
            json_reply(if !crate::brain::enabled() || text.trim().is_empty() {
                json!({ "hits": [] })
            } else {
                crate::brain::store::search_in(&crate::brain::store_path(), text.trim(), 40, None, scope, emb.as_ref().map(|e| e as &dyn crate::brain::embed::Embedder))
                    .unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))
            })
        }
        "/api/brain/insights" => {
            let q = req.uri().query().unwrap_or("");
            let (client, thread) = (query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            json_reply(match scope {
                // Insights are off (`brain::EXTRAS`): none to show.
                Some(_) if !crate::brain::EXTRAS => json!({ "off": true }),
                Some(s) => crate::brain::insights::view(s).map(|mut v| {
                    v["running"] = json!(BRAIN_INSIGHTS.lock().map(|r| r.contains(&format!("{}:{}", s.0, s.1))).unwrap_or(false));
                    v
                }).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })),
                None => json!({ "error": "a client or a thread" }),
            })
        }
        // A work item's mentions elsewhere in its thread: POST-less, the item rides in the query.
        "/api/brain/mentions" => {
            crate::brain::advert::count("window", "mentioned_also_in");
            let q = req.uri().query().unwrap_or("");
            let thread = query_param(q, "thread").unwrap_or_default();
            let title = query_param(q, "title").unwrap_or_default();
            let quotes: Vec<(String, String)> = query_param(q, "quotes")
                .and_then(|s| serde_json::from_str::<Vec<(String, String)>>(&s).ok())
                .unwrap_or_default();
            json_reply(crate::brain::views::mentions(&thread, &title, &quotes).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        "/api/brain/node" => {
            let q = req.uri().query().unwrap_or("");
            let (client, thread) = (query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            json_reply(crate::brain::views::node(&query_param(q, "id").unwrap_or_default(), scope).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        // One source for the graph's details panel.
        "/api/brain/doc" => {
            let q = req.uri().query().unwrap_or("");
            let (client, thread) = (query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            json_reply(crate::brain::views::doc(&query_param(q, "source").unwrap_or_default(), scope).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        p if p.starts_with("/api/brain/also/") => json_reply({
            crate::brain::advert::count("window", "also_discussed");
            crate::brain::views::also(&urldecode(&p["/api/brain/also/".len()..])).unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))
        }),
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // Settings: one plain value, as the page shows it now.
        // The taxonomy: the user's review (accept, rename, merge, move, remove, add, set).
        // The user's edit of topics or terms, off the window's thread: on a large Brain a new version re-reads the
        // mentions it changed (seconds). `topic_jobs` shows it running; its error comes back once (`edit_error`).
        "brain_taxonomy" => {
            let body = v["body"].clone();
            BRAIN_EDITS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::spawn(move || {
                let res = std::panic::catch_unwind(|| crate::brain::taxonomy::edit(&body)).unwrap_or_else(|_| Err(anyhow::anyhow!("the edit failed")));
                if let Err(e) = &res {
                    if let Ok(mut x) = BRAIN_EDIT_ERROR.lock() {
                        *x = (x.0 + 1, format!("{e:#}"));
                    }
                }
                BRAIN_EDITS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            });
            true.into()
        }
        // Refresh a scope's insights now (in the background; one run per scope at a time).
        "brain_insights" => {
            let (kind, id) = (v["kind"].as_str().unwrap_or("").to_string(), v["id"].as_str().unwrap_or("").to_string());
            let k = format!("{kind}:{id}");
            if (kind == "client" || kind == "thread") && !id.is_empty() && BRAIN_INSIGHTS.lock().map(|mut r| r.insert(k.clone())).unwrap_or(false) {
                std::thread::spawn(move || {
                    if let Err(e) = crate::brain::insights::run((&kind, &id), &|_| {}) {
                        eprintln!("brain: insights {k}: {e:#}");
                    }
                    if let Ok(mut r) = BRAIN_INSIGHTS.lock() {
                        r.remove(&k);
                    }
                });
            }
            true.into()
        }
        // A layer's topics rebuilt from its calls (Claude, in the background; provisional as any build).
        "brain_rebuild_topics" => {
            let layer = v["layer"].as_str().unwrap_or("").to_string();
            let k = format!("taxonomy:{layer}");
            if !layer.is_empty() && BRAIN_INSIGHTS.lock().map(|mut r| r.insert(k.clone())).unwrap_or(false) {
                std::thread::spawn(move || {
                    // However it ends (a panic too): the Inbox collects again, then it stops showing as running.
                    let _end = OnEnd(Some(move || {
                        crate::attention::stale();
                        if let Ok(mut r) = BRAIN_INSIGHTS.lock() {
                            r.remove(&k);
                        }
                    }));
                    match crate::brain::taxonomy::induce(&layer, &|_| {}) {
                        Ok(_) => crate::app::notify("Topics rebuilt", "Review them on the Brain's Topics tab, then accept"),
                        Err(e) => crate::app::notify("Topics not rebuilt", &format!("{e:#}")),
                    }
                });
            }
            true.into()
        }
        // A topic too broad, split into subtopics (Claude, in the background; #31).
        "brain_split_topic" => {
            let (layer, label) = (v["layer"].as_str().unwrap_or("").to_string(), v["label"].as_str().unwrap_or("").to_string());
            // Tracked under the layer whose drift asked for it (a client's catch-all is often a practice topic).
            let asker = v["for"].as_str().filter(|f| !f.is_empty()).unwrap_or(&layer).to_string();
            let k = format!("split:{asker}");
            if BRAIN_INSIGHTS.lock().map(|r| r.contains(&format!("taxonomy:{asker}")) || r.contains(&format!("taxonomy:{layer}"))).unwrap_or(false) {
                return Some(notice("Its topics are being rebuilt: split it when that is done"));
            }
            if !layer.is_empty() && !label.is_empty() && BRAIN_INSIGHTS.lock().map(|mut r| r.insert(k.clone())).unwrap_or(false) {
                std::thread::spawn(move || {
                    let _end = OnEnd(Some(move || {
                        crate::attention::stale();
                        if let Ok(mut r) = BRAIN_INSIGHTS.lock() {
                            r.remove(&k);
                        }
                    }));
                    match crate::brain::taxonomy::split(&layer, &label, &|_| {}) {
                        Ok(v) => crate::app::notify(&format!("“{label}” split"), &format!("Into {} — its sections are tagged again; review them on the Brain's Topics tab", v["into"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default())),
                        Err(e) => crate::app::notify(&format!("“{label}” not split"), &format!("{e:#}")),
                    }
                });
            }
            true.into()
        }
        // A layer's sections tagged again with its topics as they are now (Claude, in the background).
        "brain_retag_topics" => {
            let layer = v["layer"].as_str().unwrap_or("").to_string();
            let k = format!("retag:{layer}");
            if !layer.is_empty() && BRAIN_INSIGHTS.lock().map(|mut r| r.insert(k.clone())).unwrap_or(false) {
                std::thread::spawn(move || {
                    match crate::brain::taxonomy::classify(Some(&layer), true, &|_| {}) {
                        Ok(n) => crate::app::notify("Sections tagged again", &format!("{n} sections, with the topics as they are now")),
                        Err(e) => crate::app::notify("Sections not tagged", &format!("{e:#}")),
                    }
                    if let Ok(mut r) = BRAIN_INSIGHTS.lock() {
                        r.remove(&k);
                    }
                });
            }
            true.into()
        }
        // A suggested topic move (up to the practice, down to a client), done or turned down.
        "brain_topic_move" => match (v["key"].as_str(), v["do"].as_str()) {
            (Some(k), Some("apply")) => match crate::brain::taxonomy::apply_move(k) {
                Ok(said) => notice(&said),
                Err(e) => notice(&format!("{e:#}")),
            },
            (Some(k), Some("dismiss")) => crate::brain::taxonomy::dismiss_move(k).is_ok().into(),
            _ => false.into(),
        },
        "brain_topics_snooze" => v["layer"].as_str().is_some_and(|l| crate::brain::taxonomy::snooze(l).is_ok()).into(),
        "brain_sync" => {
            brain_sync_now();
            true.into()
        }
        // A sender of mail or posts trusted, ignored or forgotten for a client: read again at the next sync.
        "brain_sender" => {
            let (client, key, decision) = (v["client"].as_str().unwrap_or(""), v["key"].as_str().unwrap_or(""), v["decision"].as_str().unwrap_or(""));
            match crate::brain::senders::decide(client, key, decision) {
                Ok(()) => {
                    brain_sync_now();
                    true.into()
                }
                Err(e) => {
                    eprintln!("brain_sender: {e:#}");
                    false.into()
                }
            }
        }
        // The numbers lane: Claude proposes a client's families (minutes), the user accepts them, a build.
        "brain_numbers" => {
            let (op, client) = (v["op"].as_str().unwrap_or("").to_string(), v["client"].as_str().unwrap_or("").to_string());
            if client.is_empty() || NUMBERS_BUSY.lock().map(|b| b.contains_key(&client)).unwrap_or(true) {
                return Some(false.into());
            }
            if let Ok(mut b) = NUMBERS_BUSY.lock() {
                b.insert(client.clone(), json!({ "op": op, "since": chrono::Utc::now().to_rfc3339() }));
            }
            std::thread::spawn(move || {
                use crate::brain::numbers as n;
                let r: anyhow::Result<Value> = match op.as_str() {
                    "propose" => n::profile::propose(&client, None),
                    "accept" => n::load_config(&client).ok_or_else(|| anyhow::anyhow!("nothing proposed")).and_then(|mut c| {
                        c.check()?;
                        c._echo = Some(json!({ "status": "accepted", "at": chrono::Utc::now().to_rfc3339(), "was": c._echo }));
                        n::save_config(&client, &c)?;
                        n::build::run(&client, &c, None)
                    }),
                    "build" => n::load_config(&client).filter(|c| c.accepted()).ok_or_else(|| anyhow::anyhow!("nothing accepted")).and_then(|c| n::build::run(&client, &c, None)),
                    _ => Err(anyhow::anyhow!("no op {op}")),
                };
                if let Ok(mut b) = NUMBERS_BUSY.lock() {
                    b.remove(&client);
                    if let Err(e) = r {
                        b.insert(format!("{client}:error"), json!(format!("{e:#}")));
                    } else {
                        b.remove(&format!("{client}:error"));
                    }
                }
            });
            true.into()
        }
        _ => return None,
    })
}

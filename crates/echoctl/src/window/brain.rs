//! The Brain: its pages and its background work (only with the `brain` feature).

use super::*;

/// Settings → Brain's Sync now: one at a time, in the background; its report for the page.
pub(super) static BRAIN_SYNC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub(super) static BRAIN_LAST: std::sync::Mutex<Value> = std::sync::Mutex::new(Value::Null);
/// Taxonomy edits running now, and the last one's error with its number (the page that made an edit shows an error
/// numbered after the one it saw when it asked).
pub(super) static BRAIN_EDITS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
pub(super) static BRAIN_EDIT_ERROR: std::sync::Mutex<(u64, String)> = std::sync::Mutex::new((0, String::new()));

/// Start a Brain sync in the background unless one runs (its outcome: `BRAIN_LAST`).
pub(super) fn brain_sync_now() {
    if !BRAIN_SYNC.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(|| {
            let r = match crate::brain::sync() {
                Ok(r) => json!(r),
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
            // A taxonomy edit running now (`brain_taxonomy`), and the last one's error with its number.
            v["topic_jobs"] = json!(if BRAIN_EDITS.load(std::sync::atomic::Ordering::SeqCst) > 0 { vec!["edit:taxonomy"] } else { vec![] });
            if let Ok(e) = BRAIN_EDIT_ERROR.lock() {
                v["edit_error"] = json!({"n": e.0, "text": e.1});
            }
            json_reply(v)
        }
        // The Brain a piece at a time (`brain::browse`): sources grouped and paged, a folder's files, the
        // dictionary by group and letter.
        p if p.starts_with("/api/brain/browse/") => {
            use crate::brain::browse;
            let q = req.uri().query().unwrap_or("");
            let (client, thread) = (query_param(q, "client"), query_param(q, "thread"));
            let scope = client.as_deref().map(|c| ("client", c)).or(thread.as_deref().map(|t| ("thread", t)));
            let num = |k: &str, d: usize| query_param(q, k).and_then(|v| v.parse().ok()).unwrap_or(d);
            let r = if !crate::brain::enabled() { Err(anyhow::anyhow!("Recall is off")) } else { match &p["/api/brain/browse/".len()..] {
                "groups" => browse::source_groups(scope),
                "sources" => browse::source_list(scope, &browse::ListQuery {
                    client: query_param(q, "of"), kind: query_param(q, "kind"), root: query_param(q, "root"), channel: query_param(q, "channel"),
                    q: query_param(q, "q"), before: query_param(q, "before"), limit: num("limit", 50),
                }),
                "folder" => browse::folder_files(&query_param(q, "root").unwrap_or_default(), query_param(q, "q").as_deref(), num("offset", 0), num("limit", 50)),
                "terms" => browse::terms(scope, query_param(q, "group").as_deref(), query_param(q, "letter").as_deref(), query_param(q, "q").as_deref(), num("offset", 0), num("limit", 50)),
                _ => Err(anyhow::anyhow!("not found")),
            } };
            json_reply(r.unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
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
        _ => return None,
    })
}

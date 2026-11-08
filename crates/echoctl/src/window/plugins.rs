//! Echo plugins: their files, commands, marketplaces and deliveries.

use super::*;

/// Claude Code's Echo plugin installs being updated (`update_plugins`): one update at a time.
pub(super) static PLUGINS_UPDATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `/plugin/<id>/<path>`: a file of an installed, enabled plugin (its client half, styles, icons).
pub(super) fn plugin_file(rest: &str) -> Response<Cow<'static, [u8]>> {
    let Some((id, path)) = rest.split_once('/') else { return reply(404, "text/plain", b"no such file".to_vec()) };
    let Some(p) = crate::plugins::enabled().into_iter().find(|p| p.id == id) else { return reply(404, "text/plain", b"no such plugin".to_vec()) };
    match crate::plugins::inside_plugin(&crate::plugins::plugin_dir(&p), path).and_then(|f| Ok(std::fs::read(f)?)) {
        Ok(bytes) => {
            let kind = match path.rsplit('.').next().unwrap_or("") {
                "js" | "mjs" => "text/javascript; charset=utf-8",
                "css" => "text/css; charset=utf-8",
                "json" => "application/json",
                "svg" => "image/svg+xml",
                "png" => "image/png",
                "html" => "text/plain; charset=utf-8",
                _ => "application/octet-stream",
            };
            reply(200, kind, bytes)
        }
        Err(_) => reply(404, "text/plain", b"no such file".to_vec()),
    }
}

pub(super) fn deliver_op(v: &Value) -> Handled {
    match v["op"].as_str().unwrap_or("") {
        "test" => {
            let kind = v["kind"].as_str().unwrap_or("").to_string();
            std::thread::spawn(move || {
                let r = crate::deliver::ready(&kind).and_then(|_| crate::deliver::send(&kind, &crate::deliver::Msg { title: "Echo test", markdown: "This is how Echo's messages will reach you.", about: Some("A test from Settings"), ..Default::default() }));
                match r {
                    Ok(()) => crate::app::notify("Echo", &format!("Test sent to your {kind}")),
                    Err(e) => crate::app::notify("Not sent", &format!("{e:#}")),
                }
            });
            true.into()
        }
        o => notice(&format!("unknown delivery operation “{o}”")),
    }
}

/// For the Inbox: at most every 10 minutes, off the request, the stale marketplaces fetched again — quietly (Settings
/// shows no "Reading…" for it) — and the Inbox read again after, so a newly found update shows at once.
pub(super) fn refresh_markets_for_inbox() {
    static LAST: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(600)) {
            return;
        }
        *last = Some(std::time::Instant::now());
    }
    std::thread::spawn(|| {
        if crate::plugins::any_stale() {
            crate::plugins::refresh_stale(false);
            crate::attention::stale();
        }
    });
}

/// Marketplaces not fetched for a while, fetched again (one pass at a time, in the background).
pub(super) fn refresh_plugin_markets() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if !RUNNING.swap(true, Ordering::SeqCst) {
        // Said busy before the answer is made, so the page asks again until the read ends.
        if !crate::plugins::any_stale() {
            RUNNING.store(false, Ordering::SeqCst);
            return;
        }
        if let Ok(mut b) = crate::plugins::BUSY.lock() {
            b.insert("markets".into(), "Reading marketplaces…".into());
        }
        std::thread::spawn(|| {
            let _end = OnEnd(Some(|| RUNNING.store(false, Ordering::SeqCst)));
            crate::plugins::refresh_stale(false);
            if let Ok(mut b) = crate::plugins::BUSY.lock() {
                b.remove("markets");
            }
        });
    }
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        // Settings → Plugins; marketplaces not read for 30 min, or by another Echo version, are fetched again in the background.
        "/api/plugins" => {
            refresh_plugin_markets();
            json_reply(crate::plugins::snapshot())
        }
        // Where Echo can send to the user (the plugins that deliver).
        "/api/deliver" => json_reply(crate::deliver::status()),
        // A plugin's command (`ctx.command` in its host half), for its client half: JSON in, JSON out.
        #[cfg(feature = "js")]
        p if p.starts_with("/api/plugin/") => {
            let rest = &p["/api/plugin/".len()..];
            match rest.split_once("/call/") {
                Some((id, cmd)) if crate::plugins::valid_id(id) && !cmd.is_empty() && !cmd.contains('/') => {
                    let q = req.uri().query().unwrap_or("");
                    let args: Value = query_param(q, "args").and_then(|a| serde_json::from_str(&a).ok()).unwrap_or(Value::Null);
                    json_reply(match crate::jshost::call(id, cmd, &args) {
                        Ok(v) => json!({"value": v}),
                        Err(e) => json!({"error": format!("{e:#}")}),
                    })
                }
                _ => reply(404, "text/plain", b"no such call".to_vec()),
            }
        }
        // A plugin's own files (its client half and what it loads), from where it is installed.
        p if p.starts_with("/plugin/") => plugin_file(&p["/plugin/".len()..]),
        // The permission sheet for installing a plugin (from what its marketplace last gave).
        "/api/plugins/review" => {
            let q = req.uri().query().unwrap_or("");
            let (id, market) = (query_param(q, "id").unwrap_or_default(), query_param(q, "market"));
            json_reply(crate::plugins::review_value(&id, market.as_deref().filter(|m| !m.is_empty())))
        }
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        "deliver_op" => deliver_op(v),
        // A plugin's command from Echo's own places (an Inbox item's button): in the background.
        #[cfg(feature = "js")]
        "plugin_call" => {
            let (id, command, args) = (v["id"].as_str().unwrap_or("").to_string(), v["command"].as_str().unwrap_or("").to_string(), v["args"].clone());
            std::thread::spawn(move || {
                if let Err(e) = crate::jshost::call(&id, &command, &args) {
                    crate::app::notify(&id, &format!("{e:#}"));
                }
                crate::attention::stale();
            });
            true.into()
        }
        // Settings → Plugins: install / update in the background, the rest at once.
        "plugin_op" => match crate::plugins::page_op(v) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Bring Claude Code's Echo plugin installs up to this Echo (Needs you, Settings → Health).
        "update_plugins" => {
            use std::sync::atomic::Ordering;
            if !PLUGINS_UPDATING.swap(true, Ordering::SeqCst) {
                std::thread::spawn(|| {
                    let _end = OnEnd(Some(|| {
                        crate::attention::stale();
                        PLUGINS_UPDATING.store(false, Ordering::SeqCst);
                    }));
                    match crate::app::update_plugins() {
                        Ok(_) => crate::app::notify("Echo", "The Echo plugin in Claude Code is up to date — restart Claude Code to use it."),
                        Err(e) => crate::app::notify("Echo", &format!("Couldn't update the Echo plugin in Claude Code: {e:#}")),
                    }
                });
            }
            true.into()
        }
        _ => return None,
    })
}

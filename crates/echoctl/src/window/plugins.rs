//! Echo plugins: their files, commands, marketplaces and deliveries.

use super::*;

/// Claude Code's Echo plugin installs being updated (`update_plugins`): one update at a time.
pub(super) static PLUGINS_UPDATING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `/plugin/<id>/<path>`: a file of an installed, enabled plugin (its client half, styles, icons).
pub(super) fn plugin_file(rest: &str) -> Response<Cow<'static, [u8]>> {
    let Some((id, path)) = rest.split_once('/') else { return reply(404, "text/plain", b"no such file".to_vec()) };
    let Some(p) = crate::plugins::enabled().into_iter().find(|p| p.id == id) else { return reply(404, "text/plain", b"no such plugin".to_vec()) };
    if let Some(file) = path.strip_prefix("store/") {
        return store_file(id, file);
    }
    match crate::plugins::inside_plugin(&crate::plugins::plugin_dir(&p), path).and_then(|f| Ok(std::fs::read(f)?)) {
        Ok(bytes) => {
            let kind = match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
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

/// `/plugin/<id>/store/<file>`: audio or an image the plugin's host half saved in its own store (`ctx.http.fetch` with
/// `save`), for an `<audio>` or `<img>` of its window half. Only these kinds go out, so the store's settings and notes
/// stay inside; the path is kept inside the store once links are followed.
fn store_file(id: &str, rel: &str) -> Response<Cow<'static, [u8]>> {
    let kind = match rel.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return reply(404, "text/plain", b"no such file".to_vec()),
    };
    let read = || -> anyhow::Result<Vec<u8>> {
        let root = dunce::canonicalize(crate::plugins::data_dir(id))?;
        let file = crate::grants::inside(&root, rel)?;
        anyhow::ensure!(file.is_file(), "no such file");
        Ok(std::fs::read(file)?)
    };
    match read() {
        Ok(bytes) => reply(200, kind, bytes),
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
            #[cfg_attr(not(feature = "js"), allow(unused_mut))]
            let mut v = crate::plugins::snapshot();
            // Why a plugin's work rows were left out (its provider failed or was slow): a quiet line on its card.
            #[cfg(feature = "js")]
            for x in v["installed"].as_array_mut().into_iter().flatten() {
                if let Some((place, e)) = x["id"].as_str().and_then(crate::jshost::work::error) {
                    x["work_error"] = json!({"place": place, "error": e});
                }
            }
            json_reply(v)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_gets_media_from_a_plugins_store_and_nothing_else() {
        let (_env, _root) = crate::paths::testenv::folders("window-store");
        let store = crate::plugins::data_dir("hello");
        std::fs::create_dir_all(store.join("audio")).unwrap();
        std::fs::write(store.join("audio/a.mp3"), [0xff, 0xfb, 0x90]).unwrap();
        std::fs::write(store.join("pic.JPG"), [0xff, 0xd8]).unwrap();
        std::fs::write(store.join("settings.json"), "{}").unwrap();
        std::fs::write(store.join("notes.txt"), "x").unwrap();
        std::fs::write(store.join("page.svg"), "<svg/>").unwrap();
        let got = |rel: &str| {
            let r = store_file("hello", rel);
            (r.status().as_u16(), r.headers().get(CONTENT_TYPE).map(|v| v.to_str().unwrap().to_string()).unwrap_or_default(), r.body().to_vec())
        };
        assert_eq!(got("audio/a.mp3"), (200, "audio/mpeg".to_string(), vec![0xff, 0xfb, 0x90]));
        assert_eq!(got("pic.JPG"), (200, "image/jpeg".to_string(), vec![0xff, 0xd8]), "the kind is read from the name in any case");
        // Only audio and pictures: the store's settings, notes and a page that could carry script stay in.
        for kept in ["settings.json", "notes.txt", "page.svg"] {
            assert_eq!(got(kept).0, 404, "{kept}");
        }
        // Not there, outside the store, or a Windows device name.
        for no in ["audio/none.mp3", "audio", "../x.mp3", "audio/../../x.png", "/etc/x.mp3", "NUL.mp3", "audio/CON.mp3"] {
            assert_eq!(got(no).0, 404, "{no}");
        }
        // A link inside the store that leads out of it isn't followed.
        #[cfg(unix)]
        {
            let out = store.parent().unwrap().join("outside.mp3");
            std::fs::write(&out, "secret").unwrap();
            std::os::unix::fs::symlink(&out, store.join("audio/link.mp3")).unwrap();
            assert_eq!(got("audio/link.mp3").0, 404);
        }
    }
}

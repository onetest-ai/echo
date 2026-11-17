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
    if let Some(rest) = path.strip_prefix("folder/") {
        return folder_image(&p, rest);
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

/// The kind of an image a page of a plugin shows from a granted folder: never SVG, which can carry script.
fn image_kind(rel: &str) -> Option<&'static str> {
    Some(match rel.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    })
}

/// `/plugin/<id>/folder/<root>/<path>`: an image in one of the folders the user gave the plugin (`<root>` as
/// `ctx.fs.folders()` names it), for an `<img>` of its window half — the plugin asking for `uses.folders`, images only,
/// 10 MB at most, resolved by `grants::resolve` (inside the folder once links are followed).
fn folder_image(p: &crate::plugins::Installed, rest: &str) -> Response<Cow<'static, [u8]>> {
    let read = || -> anyhow::Result<(&'static str, Vec<u8>)> {
        let m = crate::plugins::read_manifest(&crate::plugins::plugin_dir(p))?;
        anyhow::ensure!(m.uses.get("folders").is_some(), "it didn't ask for folders");
        // The longest granted folder the request starts with, then the path inside it.
        let (root, rel) = crate::grants::folders(&p.id)
            .into_iter()
            .map(|f| f.display().to_string())
            .filter_map(|f| rest.strip_prefix(&f).and_then(|r| r.strip_prefix(['/', '\\'])).map(|r| (f.clone(), r.to_string())))
            .max_by_key(|(f, _)| f.len())
            .ok_or_else(|| anyhow::anyhow!("not one of its folders"))?;
        let kind = image_kind(&rel).ok_or_else(|| anyhow::anyhow!("not an image"))?;
        let file = crate::grants::resolve(&p.id, &root, &rel)?;
        // Never waiting on a FIFO or device put in the folder.
        let f = crate::grants::open_regular(&file)?;
        anyhow::ensure!(f.metadata()?.len() <= 10 << 20, "not an image of 10 MB at most");
        let mut bytes = vec![];
        use std::io::Read;
        f.take(10 << 20).read_to_end(&mut bytes)?;
        Ok((kind, bytes))
    };
    match read() {
        Ok((kind, bytes)) => reply(200, kind, bytes),
        Err(_) => reply(404, "text/plain", b"no such image".to_vec()),
    }
}

// ── files dropped on the window (`ctx.onFileDrop`) ──

/// The plugin whose view the page shows and that listens for dropped files (the page says so, `plugin_drops`): files
/// dropped on the window go to it; none, Echo's own drop.
#[cfg(feature = "js")]
static DROPS_FOR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
/// The handles made of the files last dropped, and their plugin, until the page takes them (`plugin_dropped`).
#[cfg(feature = "js")]
static DROPPED: std::sync::Mutex<Option<(String, Vec<Value>)>> = std::sync::Mutex::new(None);

/// Whether files dropped on the window now go to a plugin (its view shown, listening).
#[cfg(feature = "js")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn drops_wanted() -> bool {
    DROPS_FOR.lock().is_ok_and(|d| d.is_some())
}

/// Files dropped on the window: handles of the plugin they go to, kept for the page. Whether there were any.
#[cfg(feature = "js")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn dropped(paths: &[PathBuf]) -> bool {
    let Some(id) = DROPS_FOR.lock().ok().and_then(|d| d.clone()) else { return false };
    let handles: Vec<Value> = paths.iter().filter_map(|p| crate::jshost::files::make(&id, p).ok()).collect();
    if handles.is_empty() {
        return false;
    }
    if let Ok(mut d) = DROPPED.lock() {
        *d = Some((id, handles));
    }
    true
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
            // Echo's background jobs, read by a client half that asked for them (`ctx.echo.jobs.list`, echo-plugins R25);
            // a change goes only through the page's IPC (`plugin_jobs_set`), never a GET.
            if let Some(id) = rest.strip_suffix("/jobs").filter(|id| crate::plugins::valid_id(id)) {
                return Some(json_reply(match crate::jshost::may_tune_jobs(id) {
                    Ok(()) => json!({"value": crate::jobs::list()}),
                    Err(e) => json!({"error": format!("{e:#}")}),
                }));
            }
            // Echo's jobs as routines and their runs (`ctx.echo.routines`), read only.
            if let Some(id) = rest.strip_suffix("/routines").filter(|id| crate::plugins::valid_id(id)) {
                return Some(json_reply(match crate::jshost::may_tune_jobs(id) {
                    Ok(()) => json!({"value": crate::jobs::routines()}),
                    Err(e) => json!({"error": format!("{e:#}")}),
                }));
            }
            if let Some(id) = rest.strip_suffix("/routines/runs").filter(|id| crate::plugins::valid_id(id)) {
                let q = req.uri().query().unwrap_or("");
                let limit = query_param(q, "limit").and_then(|l| l.parse::<usize>().ok()).unwrap_or(20).min(500);
                return Some(json_reply(match crate::jshost::may_tune_jobs(id).and_then(|()| crate::jobs::routine_runs(&query_param(q, "id").unwrap_or_default(), limit)) {
                    Ok(v) => json!({"value": v}),
                    Err(e) => json!({"error": format!("{e:#}")}),
                }));
            }
            // A job's runs (`ctx.echo.jobs.runs`), read only.
            if let Some(id) = rest.strip_suffix("/jobs/runs").filter(|id| crate::plugins::valid_id(id)) {
                let q = req.uri().query().unwrap_or("");
                let limit = query_param(q, "limit").and_then(|l| l.parse::<usize>().ok()).unwrap_or(20).min(500);
                return Some(json_reply(match crate::jshost::may_tune_jobs(id).and_then(|()| crate::jobs::runs(&query_param(q, "job").unwrap_or_default(), limit)) {
                    Ok(v) => json!({"value": v}),
                    Err(e) => json!({"error": format!("{e:#}")}),
                }));
            }
            // A page of its Markdown as safe HTML, and its destinations to resolve first (`ctx.markdown`, read only).
            for (suffix, links) in [("/markdown", false), ("/markdown/links", true)] {
                if let Some(id) = rest.strip_suffix(suffix).filter(|id| crate::plugins::enabled().iter().any(|p| p.id == *id)) {
                    let q = req.uri().query().unwrap_or("");
                    let md = query_param(q, "md").unwrap_or_default();
                    let opts: Value = query_param(q, "opts").and_then(|o| serde_json::from_str(&o).ok()).unwrap_or(Value::Null);
                    let v = if links { crate::plugins::markdown::links(&md) } else { crate::plugins::markdown::render(id, &md, &opts).map(Value::from) };
                    return Some(json_reply(v.map_or_else(|e| json!({"error": format!("{e:#}")}), |v| json!({"value": v}))));
                }
            }
            match rest.split_once("/call/") {
                Some((id, cmd)) if crate::plugins::valid_id(id) && !cmd.is_empty() && !cmd.contains('/') => {
                    let q = req.uri().query().unwrap_or("");
                    let args: Value = query_param(q, "args").and_then(|a| serde_json::from_str(&a).ok()).unwrap_or(Value::Null);
                    // Handles of files the user handed this plugin (`ctx.call(…, {files})`), each checked as its own.
                    let files: Vec<Value> = query_param(q, "files").and_then(|f| serde_json::from_str(&f).ok()).unwrap_or_default();
                    json_reply(match crate::jshost::call_with(id, cmd, &args, &files, crate::jshost::CALL_TIMEOUT) {
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
        // A plugin's change to Echo's background jobs (`ctx.echo.jobs.set`, sent only on the user's click or key):
        // the plugin on and asking for `uses.echo.jobs`, checked as `echoctl jobs set`; the answer to its request.
        #[cfg(feature = "js")]
        "plugin_jobs_set" => {
            let id = v["id"].as_str().unwrap_or("");
            let settings: Vec<String> = v["settings"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_owned)).collect();
            let r = crate::jshost::may_tune_jobs(id).and_then(|()| crate::jobs::set(v["job"].as_str().unwrap_or(""), &settings));
            let answer = match r {
                Ok(list) => json!({"req": v["req"], "value": list}),
                Err(e) => json!({"req": v["req"], "error": format!("{e:#}")}),
            };
            Handled { refresh: false, reply: Some(format!("window.echo && window.echo.jobsSet({answer})")) }
        }
        // The plugin whose view is shown and listens for dropped files (`ctx.onFileDrop`), or none.
        #[cfg(feature = "js")]
        "plugin_drops" => {
            let id = v["id"].as_str().filter(|id| crate::plugins::enabled().iter().any(|p| p.id == *id)).map(str::to_owned);
            if let Ok(mut d) = DROPS_FOR.lock() {
                *d = id;
            }
            false.into()
        }
        // Files were dropped (the window's drop handler): their handles to the plugin's window half — names and handles,
        // never the paths.
        #[cfg(feature = "js")]
        "plugin_dropped" => match DROPPED.lock().ok().and_then(|mut d| d.take()) {
            Some((id, files)) => Handled { refresh: false, reply: Some(format!("window.echo && window.echo.pluginDrop({}, {})", json!(id), json!(files))) },
            None => false.into(),
        },
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
    fn a_plugins_page_gets_images_of_its_granted_folders_and_nothing_else() {
        let (_env, root) = crate::paths::testenv::folders("window-folder-images");
        let vault = root.join("vault");
        std::fs::create_dir_all(vault.join("assets")).unwrap();
        for (n, b) in [("assets/logo.png", &b"\x89PNG"[..]), ("assets/Photo.JPEG", b"\xff\xd8"), ("assets/a.webp", b"RIFF"), ("assets/x.svg", b"<svg/>"), ("Note.md", b"# Acme")] {
            std::fs::write(vault.join(n), b).unwrap();
        }
        std::fs::File::create(vault.join("assets/huge.png")).unwrap().set_len((10 << 20) + 1).unwrap();
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.png"), b"\x89PNG").unwrap();
        let mk = root.join("market");
        for (id, uses) in [("pages", json!({"folders": {"what": "vaults"}})), ("plain", json!({}))] {
            let d = mk.join("plugins").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "uses": uses}).to_string()).unwrap();
        }
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "pages", "source": "./plugins/pages"}, {"id": "plain", "source": "./plugins/plain"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        for id in ["pages", "plain"] {
            let agreed = crate::plugins::review(id, Some(&name)).unwrap().uses;
            crate::plugins::install(id, Some(&name), &agreed).unwrap();
            crate::grants::grant(id, &vault).unwrap();
        }
        let real = dunce::canonicalize(&vault).unwrap().display().to_string();
        let got = |id: &str, rest: &str| {
            let r = plugin_file(&format!("{id}/folder/{rest}"));
            (r.status().as_u16(), r.headers().get(CONTENT_TYPE).map(|v| v.to_str().unwrap().to_string()).unwrap_or_default())
        };
        assert_eq!(got("pages", &format!("{real}/assets/logo.png")), (200, "image/png".into()));
        assert_eq!(got("pages", &format!("{real}/assets/Photo.JPEG")), (200, "image/jpeg".into()), "the kind from the name, in any case");
        assert_eq!(got("pages", &format!("{real}/assets/a.webp")).0, 200);
        assert_eq!(plugin_file(&format!("pages/folder/{real}/assets/logo.png")).body().as_ref(), b"\x89PNG");
        // Not an image (SVG can carry script), over 10 MB, out of the folder, a folder it wasn't given, a plugin that
        // didn't ask for folders, one switched off.
        for no in ["assets/x.svg", "Note.md", "assets/huge.png", "../outside/secret.png", "assets/../../outside/secret.png", "assets/none.png"] {
            assert_eq!(got("pages", &format!("{real}/{no}")).0, 404, "{no}");
        }
        assert_eq!(got("pages", &format!("{}/secret.png", dunce::canonicalize(&outside).unwrap().display())).0, 404);
        assert_eq!(got("plain", &format!("{real}/assets/logo.png")).0, 404, "it didn't ask for folders");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.join("secret.png"), vault.join("assets/link.png")).unwrap();
            assert_eq!(got("pages", &format!("{real}/assets/link.png")).0, 404, "a link out of the folder isn't followed");
        }
        // A FIFO in the folder: never waited on (opened without blocking on macOS).
        #[cfg(target_os = "macos")]
        {
            assert!(std::process::Command::new("mkfifo").arg(vault.join("assets/pipe.png")).status().unwrap().success());
            assert_eq!(got("pages", &format!("{real}/assets/pipe.png")).0, 404, "a FIFO is never waited on");
        }
        crate::plugins::set_enabled("pages", false).unwrap();
        assert_eq!(got("pages", &format!("{real}/assets/logo.png")).0, 404, "off: nothing");
    }

    #[cfg(feature = "js")]
    #[test]
    fn a_window_half_gets_its_markdown_as_safe_html_with_only_its_own_routes() {
        let (_env, root) = crate::paths::testenv::folders("window-markdown");
        let mk = root.join("market");
        let d = mk.join("plugins/pages");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "pages", "name": "pages", "version": "0.1.0"}).to_string()).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "pages", "source": "./plugins/pages"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        let agreed = crate::plugins::review("pages", Some(&name)).unwrap().uses;
        crate::plugins::install("pages", Some(&name), &agreed).unwrap();
        let ask = |path: &str| {
            let req = Request::builder().uri(format!("echo://localhost{path}")).body(Vec::new()).unwrap();
            let r = route(&super::super::urldecode(req.uri().path()), &req).unwrap();
            serde_json::from_slice::<Value>(r.body()).unwrap_or(Value::Null)
        };
        let md = super::super::urlencode("[[Plan]] [[Other]] <b>x</b>");
        assert_eq!(ask(&format!("/api/plugin/pages/markdown/links?md={md}"))["value"], json!({"links": ["Plan", "Other"], "images": []}));
        let opts = super::super::urlencode(&json!({"links": {"Plan": "#pages:main/Plan", "Other": "#another:main/Other"}}).to_string());
        assert_eq!(ask(&format!("/api/plugin/pages/markdown?md={md}&opts={opts}"))["value"], "<p><a href=\"#pages:main/Plan\">Plan</a> Other &lt;b&gt;x&lt;/b&gt;</p>");
        // A plugin that isn't installed and on gets no answer of its own.
        crate::plugins::set_enabled("pages", false).unwrap();
        assert_eq!(ask(&format!("/api/plugin/pages/markdown?md={md}")), Value::Null);
    }

    #[cfg(feature = "js")]
    #[test]
    fn files_dropped_go_as_handles_to_the_plugin_whose_view_listens_and_else_stay_echos() {
        let (_env, root) = crate::paths::testenv::folders("window-drops");
        let mk = root.join("market");
        let d = mk.join("plugins/ingest");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "ingest", "name": "Ingest", "version": "0.1.0"}).to_string()).unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "ingest", "source": "./plugins/ingest"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        let agreed = crate::plugins::review("ingest", Some(&name)).unwrap().uses;
        crate::plugins::install("ingest", Some(&name), &agreed).unwrap();
        let f = root.join("Kickoff notes.md");
        std::fs::write(&f, "# Kickoff").unwrap();
        let page = |v: Value| handle(&v).unwrap().reply;
        // No plugin view listening (or one that isn't installed): Echo's own drop, nothing made.
        for id in [Value::Null, json!("nobody")] {
            page(json!({"cmd": "plugin_drops", "id": id}));
            assert!(!drops_wanted() && !dropped(std::slice::from_ref(&f)));
            assert_eq!(page(json!({"cmd": "plugin_dropped"})), None);
        }
        // Its view shown and listening: the files become its handles; the page gets names and handles, never paths.
        page(json!({"cmd": "plugin_drops", "id": "ingest"}));
        assert!(drops_wanted() && dropped(&[f.clone(), root.join("gone.md")]));
        let js = page(json!({"cmd": "plugin_dropped"})).unwrap();
        assert!(js.starts_with("window.echo && window.echo.pluginDrop(\"ingest\", [{") && js.contains("\"name\":\"Kickoff notes.md\""), "{js}");
        assert!(!js.contains(root.to_str().unwrap()) && !js.contains("gone.md"), "{js}");
        let file: Value = serde_json::from_str(js.split_once(", ").unwrap().1.trim_end_matches(')')).unwrap();
        assert!(crate::jshost::files::own_list("ingest", file.as_array().unwrap()).is_ok());
        assert!(crate::jshost::files::own_list("other", file.as_array().unwrap()).is_err(), "its own only");
        assert_eq!(page(json!({"cmd": "plugin_dropped"})), None, "taken once");
        page(json!({"cmd": "plugin_drops", "id": null}));
    }

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

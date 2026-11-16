//! Settings: the page's snapshot and its writes, skills, meeting types, templates, the window's layout.

use super::*;
#[cfg(feature = "brain")]
use super::brain::{BRAIN_LAST, BRAIN_SYNC};

/// What Settings shows: each setting's value and what is installed, signed in or granted. Flows
/// with a dialog run as the tray's own actions (`settings_action`); plain values save here
/// (`settings_set`).
pub(super) fn settings_snapshot() -> Value {
    let cfg = crate::config::Config::load();
    let c = cfg.as_ref().ok();
    let on = |v: Option<Option<bool>>| v.flatten().unwrap_or(true);
    let health: Vec<Value> = crate::permissions::report()
        .map(|r| r.checks.iter().map(|k| json!({ "id": k.id, "name": k.name, "status": format!("{:?}", k.status).to_lowercase(), "required": k.required, "why": k.why, "how": k.how, "url": k.settings_url })).collect())
        .unwrap_or_default();
    let mut v = json!({
        "config_error": cfg.as_ref().err().map(|e| format!("{e:#}")),
        "home": paths::home(),
        "home_source": paths::home_source(),
        "autostart": crate::app::autostart_enabled(),
        "cli": crate::app::cli_installed(),
        "version": env!("CARGO_PKG_VERSION"),
        "update": crate::update::AVAILABLE.lock().ok().and_then(|a| a.as_ref().map(|r| r.version.clone())),
        "scheduled": c.map(|c| c.scheduled.clone().unwrap_or_else(|| "ask".into())),
        "unscheduled": c.map(|c| c.unscheduled.clone().unwrap_or_else(|| "ask".into())),
        "languages": c.map(|c| c.languages.clone()).unwrap_or_default(),
        "keep_audio": c.is_some_and(|c| c.keep_audio),
        "minutes_page": c.is_some_and(|c| c.minutes_page),
        "auto_minutes": c.is_some_and(|c| c.auto_minutes),
        "project_sync": c.is_some_and(|c| c.project_sync),
        "folder_skip_permissions": c.is_some_and(|c| c.folder_skip_permissions),
        "hide_dock_icon": c.is_some_and(|c| c.hide_dock_icon),
        "own_title_bar": on(c.map(|c| c.own_title_bar)),
        // Where mail drafts go (#135): the choice as stored (null: the default, `drafts_now` says which).
        "drafts": c.and_then(|c| c.drafts.clone()),
        // The folder OneDrive keeps here (#469): as set (`none`: off), and the one in use (set or found).
        "onedrive": c.and_then(|c| c.onedrive.clone()),
        "onedrive_now": crate::onedrive::root().map(|p| p.display().to_string()),
        "drafts_now": match crate::drafts::chosen(&crate::config::Config { drafts: c.and_then(|c| c.drafts.clone()), ..Default::default() }, crate::graph::account().is_some()) {
            crate::drafts::Where::M365 => "m365",
            crate::drafts::Where::MailApp => "mail-app",
            crate::drafts::Where::App(_) => "app",
        },
        "capture_frames": c.is_some_and(|c| c.capture_frames),
        "follow_app_mute": on(c.map(|c| c.follow_app_mute)),
        "capture_ui": on(c.map(|c| c.capture_ui)),
        "capture_chat": on(c.map(|c| c.capture_chat)),
        "echo_cancel": on(c.map(|c| c.echo_cancel)),
        "notes_in": c.and_then(|c| c.notes_in.clone()),
        "vocabulary": c.map(|c| c.vocabulary.clone()).unwrap_or_default(),
        // Terms the minutes learned, with what they are used again for.
        "learned": c.map(|c| crate::vocab::learned_all(c).iter().map(|x| json!({ "term": x.term, "subjects": x.subjects, "people": x.people, "calls": x.calls.len() })).collect::<Vec<_>>()).unwrap_or_default(),
        "me": c.and_then(|c| c.me.clone()),
        // The profile the minutes read (me.md), as text and rendered.
        "me_profile": crate::calls::ensure_me_profile().ok().and_then(|p| fs::read_to_string(&p).ok()).map(|t| json!({ "text": t, "html": crate::minutes::markdown_html(&t) })),
        "calendar": crate::graph::account(),
        "graph_custom": c.is_some_and(|c| c.graph_client_id.is_some()),
        "health": health,
        "stale_plugins": crate::app::stale_plugins(),
        // Claude Code really set up: it runs, a terminal finds it, its plugin is in (checked in the background).
        "claude_code": crate::claude_setup::status(false),
        "models": crate::models::list_value().ok(),
        "downloading": crate::models::downloading().map(|(n, pct)| json!({ "name": n, "pct": pct })),
    });
    // Colleagues' photos: [the switch, the sign-in may read them].
    v["photos"] = json!([c.is_some_and(|c| c.people_photos == Some(true)), crate::graph::account().is_some() && crate::photos::allowed()]);
    #[cfg(target_os = "macos")]
    {
        v["frames_permitted"] = json!(crate::frames::permitted());
    }
    #[cfg(feature = "transcribe")]
    {
        let serve = crate::remote::serve_settings();
        v["engine"] = json!(crate::gpu::engine_status());
        v["server"] = json!({ "client": crate::remote::client_label(), "info": crate::remote::client_info(),
                              "connecting": SERVER_CONNECT.lock().map(|s| s.clone()).unwrap_or(Value::Null), "serving": serve.on,
                              "url": serve.on.then(|| crate::remote::serve_url(&serve)), "key": serve.on.then(|| serve.key.clone()) });
    }
    #[cfg(feature = "brain")]
    {
        let mut b = crate::brain::status().unwrap_or_else(|e| json!({ "error": format!("{e:#}") }));
        b["auto"] = json!(crate::brain::settings().auto());
        b["enabled"] = json!(crate::brain::enabled());
        b["syncing"] = json!(BRAIN_SYNC.load(std::sync::atomic::Ordering::SeqCst));
        // How often Claude asks it, and the window's views of it (#34): counts only.
        b["usage"] = crate::brain::advert::usage(7);
        b["last"] = BRAIN_LAST.lock().map(|l| l.clone()).unwrap_or(Value::Null);
        // Documents: what lays them out and reads their pages here.
        b["documents"] = json!({ "pdfium": crate::brain::files::pdf::library().is_some(), "libreoffice": crate::brain::files::render::soffice().is_some(),
                                 "pending": crate::brain::files::folder::pending() });
        v["brain"] = b;
    }
    home_as_tilde(&mut v, &paths::user_home());
    v
}

/// Claude Code's paths with the user's home folder as `~` (#104): Health says where it is without the account's home
/// path, which a screenshot of a test run (or of a bug report) would otherwise carry.
fn home_as_tilde(v: &mut Value, home: &Path) {
    // A home that is the root (or nothing) would turn every path into ~/…: leave them as they are.
    if home.parent().is_none() {
        return;
    }
    for at in ["/claude_code/path", "/claude_code/terminal/found", "/claude_code/terminal/dir"] {
        let Some(x) = v.pointer_mut(at) else { continue };
        if let Some(rest) = x.as_str().and_then(|s| Path::new(s).strip_prefix(home).ok()) {
            // `~/a/b` on every system (a join would write `~\a\b` on Windows); the home folder itself is `~`.
            let parts: Vec<String> = rest.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
            *x = json!(if parts.is_empty() { "~".to_string() } else { format!("~/{}", parts.join("/")) });
        }
    }
}

/// The last OpenAI-compatible transcription server connection: `{busy}`, `{ok, …}` or `{error}`.
#[cfg(feature = "transcribe")]
pub(super) static SERVER_CONNECT: std::sync::Mutex<Value> = std::sync::Mutex::new(Value::Null);

/// Save one plain setting with the value the page shows now (never a toggle of a stale value).
pub(super) fn settings_set(key: &str, value: &Value) -> anyhow::Result<()> {
    use crate::config::Config;
    match key {
        // AI settings live in ai.json, changed under its lock.
        "live_notes" => {
            let on = value.as_bool().unwrap_or(false);
            return crate::ai::Settings::update(|s| {
                s.live_notes = on;
                Ok(())
            });
        }
        "default_profile" => {
            let id = value.as_str().unwrap_or(crate::ai::SYSTEM).to_string();
            return crate::ai::Settings::update(|s| {
                s.default = (id != crate::ai::SYSTEM).then_some(id);
                Ok(())
            });
        }
        "autostart" => return crate::app::set_autostart(value.as_bool().unwrap_or(false)),
        // Settings → Brain: the embedding server, in <config dir>/brain.json.
        #[cfg(feature = "brain")]
        k if k.starts_with("brain_") => {
            let mut s = crate::brain::settings();
            let text = value.as_str().unwrap_or("").trim().to_string();
            match k {
                "brain_endpoint" => s.endpoint = text,
                "brain_api" => s.api = text,
                "brain_model" => s.model = text,
                "brain_key" => s.key = (!text.is_empty()).then_some(text),
                "brain_auto" => s.auto = Some(value.as_bool().unwrap_or(true)),
                "brain_enabled" => s.enabled = Some(value.as_bool().unwrap_or(true)),
                "brain_pages" => {
                    anyhow::ensure!(["claude", "endpoint", "off"].contains(&text.as_str()), "the page reader is claude, endpoint or off");
                    s.pages = Some(text);
                }
                "brain_vision_endpoint" => s.vision_endpoint = text,
                "brain_vision_api" => s.vision_api = text,
                "brain_vision_model" => s.vision_model = text,
                "brain_vision_key" => s.vision_key = (!text.is_empty()).then_some(text),
                _ => anyhow::bail!("no setting {k}"),
            }
            return crate::brain::save_settings(&s);
        }
        _ => {}
    }
    let mut cfg = Config::load()?;
    let b = || value.as_bool().ok_or_else(|| anyhow::anyhow!("{key} takes true or false"));
    match key {
        "scheduled" | "unscheduled" => {
            let v = value.as_str().filter(|v| matches!(*v, "ask" | "manual")).ok_or_else(|| anyhow::anyhow!("{key} is ask or manual"))?;
            if key == "scheduled" { cfg.scheduled = Some(v.into()) } else { cfg.unscheduled = Some(v.into()) }
        }
        "keep_audio" => cfg.keep_audio = b()?,
        "minutes_page" => cfg.minutes_page = b()?,
        "auto_minutes" => cfg.set_auto_minutes(b()?),
        "project_sync" => cfg.project_sync = b()?,
        "folder_skip_permissions" => cfg.folder_skip_permissions = b()?,
        "hide_dock_icon" => cfg.hide_dock_icon = b()?,
        "own_title_bar" => cfg.own_title_bar = Some(b()?),
        "drafts" => cfg.drafts = crate::drafts::setting(value.as_str().unwrap_or(""))?,
        "onedrive" => cfg.onedrive = crate::onedrive::setting(value.as_str().unwrap_or(""))?,
        "capture_frames" => {
            cfg.capture_frames = b()?;
            #[cfg(target_os = "macos")]
            if cfg.capture_frames && !crate::frames::permitted() {
                std::thread::spawn(|| {
                    if !crate::frames::request() {
                        crate::app::notify("Screen Recording needed", "Enable Echo in Privacy & Security → Screen & System Audio Recording, then restart Echo.");
                    }
                });
            }
        }
        "follow_app_mute" => cfg.follow_app_mute = Some(b()?),
        "capture_ui" => cfg.capture_ui = Some(b()?),
        "capture_chat" => cfg.capture_chat = Some(b()?),
        "people_photos" => cfg.people_photos = Some(b()?),
        "echo_cancel" => cfg.echo_cancel = Some(b()?),
        "languages" => {
            let langs: Vec<String> = value.as_array().ok_or_else(|| anyhow::anyhow!("languages is a list"))?.iter()
                .filter_map(|l| l.as_str()).map(|l| l.trim().to_lowercase()).filter(|l| l.len() == 2 && l.bytes().all(|b| b.is_ascii_lowercase())).collect();
            cfg.languages = langs;
        }
        "vocabulary" => {
            cfg.vocabulary = value.as_array().ok_or_else(|| anyhow::anyhow!("vocabulary is a list"))?.iter()
                .filter_map(|t| t.as_str()).map(|t| t.trim().chars().take(50).collect::<String>()).filter(|t| !t.is_empty()).take(200).collect();
        }
        // A learned term: forgotten, or used in every call from now on.
        "learned_remove" | "learned_always" => {
            let term = value.as_str().ok_or_else(|| anyhow::anyhow!("{key} takes a term"))?.trim().to_string();
            crate::vocab::forget_learned(&mut cfg, &term);
            if key == "learned_always" && !cfg.vocabulary.iter().any(|v| v.eq_ignore_ascii_case(&term)) {
                cfg.vocabulary.push(term);
            }
        }
        // The profile the minutes read (me.md), written as the user edited it in Settings.
        "me_profile" => {
            let text = value.as_str().ok_or_else(|| anyhow::anyhow!("me_profile takes text"))?;
            anyhow::ensure!(text.len() <= 64 * 1024, "the profile is too long (64 KB at most)");
            let p = crate::calls::ensure_me_profile()?;
            paths::write_atomic(&p, text.as_bytes())?;
            return Ok(());
        }
        "me" => {
            let name = value["name"].as_str().unwrap_or("").trim().to_string();
            let email = value["email"].as_str().unwrap_or("").trim().to_string();
            cfg.me = (!name.is_empty() || !email.is_empty()).then(|| crate::meeting::Person { name, email: (!email.is_empty()).then_some(email) });
        }
        other => anyhow::bail!("no setting {other:?}"),
    }
    cfg.save()
}

/// A flow the tray runs as it does from its menu (a dialog, a sign-in, a download): the page names
/// it, `tray.rs` maps the name to its own action.
pub fn settings_action(msg: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(msg).ok()?;
    (v["cmd"].as_str()? == "settings_action").then(|| (v["action"].as_str().unwrap_or("").to_string(), v["arg"].as_str().unwrap_or("").to_string()))
}

// ── the window's layout (`<config dir>/window-layout.json`, per computer) ────────────────────

pub(super) fn layout_file() -> PathBuf {
    paths::config_dir().join("window-layout.json")
}

/// The saved layout, or `{}` (the page's defaults) when there is none or it can't be read.
pub(super) fn load_layout() -> Value {
    fs::read(layout_file())
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// Keep the page's layout: a JSON object of a sensible size (the page checks its fields when it
/// reads it back).
pub(super) fn save_layout(layout: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(layout.is_object(), "not an object");
    let bytes = serde_json::to_vec_pretty(layout)?;
    anyhow::ensure!(bytes.len() <= 16 * 1024, "too large ({} bytes)", bytes.len());
    fs::create_dir_all(paths::config_dir())?;
    paths::write_atomic(&layout_file(), &bytes)?;
    Ok(())
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, _req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        "/api/layout" => json_reply(load_layout()),
        "/api/settings" => json_reply(settings_snapshot()),
        // Settings → Templates: the minutes templates, Echo's and the user's.
        "/api/templates" => json_reply(crate::calls::templates_view()),
        // What's new: every version's entry (⌘K and Settings → General open it again).
        "/api/whats-new" => json_reply(crate::whats_new::all()),
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // Settings → Transcription → OpenAI-compatible server: checked (its models, a test
        // transcription) before it is kept; the outcome in the settings snapshot.
        #[cfg(feature = "transcribe")]
        "server_openai" => {
            let (url, key, model) = (v["url"].as_str().unwrap_or("").to_string(), v["key"].as_str().unwrap_or("").to_string(), v["model"].as_str().unwrap_or("").to_string());
            if let Ok(mut s) = SERVER_CONNECT.lock() {
                *s = json!({ "busy": true });
            }
            std::thread::spawn(move || {
                let r = crate::remote::connect_openai(&url, &key, (!model.trim().is_empty()).then_some(model.as_str()), "type one in the model field");
                if let Ok(mut s) = SERVER_CONNECT.lock() {
                    *s = match r {
                        Ok(c) => json!({ "ok": true, "model": c.model, "words": c.words, "seconds": c.seconds }),
                        Err(e) => json!({ "error": format!("{e:#}") }),
                    };
                }
            });
            true.into()
        }
        // A release's one-time banner, answered with one of its buttons (`announce::answer`).
        "announcement" => match crate::announce::answer(v["id"].as_str().unwrap_or("")) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // What's new was shown: the running version counts as seen, never shown again.
        "whats_new_seen" => match crate::whats_new::mark_seen() {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Settings: one plain value, as the page shows it now.
        "settings_set" => match settings_set(v["key"].as_str().unwrap_or(""), &v["value"]) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Settings → Templates → Skills: Echo keeps the catalogue; a skill's content is written in an editor or by Claude.
        // A new or customized skill opens its SKILL.md in the default editor; "Open in VS Code" opens all the skills.
        "skill_new" | "skill_customize" => {
            let made = if v["cmd"] == "skill_new" { crate::skills::create(v["name"].as_str().unwrap_or("")) } else { crate::skills::customize(v["id"].as_str().unwrap_or("")) };
            match made {
                Ok(d) => {
                    crate::app::open_path(&d.join("SKILL.md"));
                    let id = d.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    Handled { refresh: false, reply: Some(format!("window.echo && window.echo.templateSaved({}, true, 'sk')", json!(id))) }
                }
                Err(e) => notice(&format!("{e:#}")),
            }
        }
        "skill_remove" => match crate::skills::remove(v["id"].as_str().unwrap_or("")) {
            Ok(()) => Handled { refresh: false, reply: Some("window.echo && window.echo.templateSaved(null, false, 'sk')".into()) },
            Err(e) => notice(&format!("{e:#}")),
        },
        "skill_on" => match crate::skills::set_on(v["id"].as_str().unwrap_or(""), v["on"] == true) {
            Ok(()) => Handled { refresh: false, reply: Some(format!("window.echo && window.echo.templateSaved({}, false, 'sk')", json!(v["id"]))) },
            Err(e) => notice(&format!("{e:#}")),
        },
        "skill_open" => {
            match crate::skills::folder(v["id"].as_str().unwrap_or("")) {
                Some(d) if v["how"] == "finder" => crate::app::reveal_file(&d.join("SKILL.md")),
                Some(d) => crate::app::open_in_code(d.parent().unwrap_or(&d)),
                None => return Some(notice("Echo's own skill: Customize it to have a folder of your own")),
            }
            false.into()
        }
        // Settings → Templates → Meeting types: save one (a built-in keeps its id), take the user's away.
        "meeting_type_save" => match crate::calls::meeting_type_save(v["id"].as_str(), v["name"].as_str().unwrap_or(""), v["about"].as_str().unwrap_or("")) {
            Ok(id) => Handled { refresh: false, reply: Some(format!("window.echo && window.echo.templateSaved({}, {}, 'mt')", json!(id), v["id"].as_str().is_none_or(str::is_empty))) },
            Err(e) => notice(&format!("{e:#}")),
        },
        "meeting_type_remove" => match crate::calls::meeting_type_remove(v["id"].as_str().unwrap_or("")) {
            Ok(()) => Handled { refresh: false, reply: Some("window.echo && window.echo.templateSaved(null, false, 'mt')".into()) },
            Err(e) => notice(&format!("{e:#}")),
        },
        // Settings → Templates: save one (a built-in keeps its id: edited), take the user's file away (Reset / Delete).
        "template_save" => {
            let words: Vec<String> = v["match"].as_array().into_iter().flatten().filter_map(|w| w.as_str().map(str::to_owned)).collect();
            let sections: Vec<Value> = v["sections"].as_array().cloned().unwrap_or_default();
            match crate::calls::template_save(v["id"].as_str(), v["name"].as_str().unwrap_or(""), &words, v["header"].as_str().unwrap_or(""), &sections) {
                // `true`: a new one (the window selects it).
                Ok(id) => Handled { refresh: false, reply: Some(format!("window.echo && window.echo.templateSaved({}, {})", json!(id), v["id"].as_str().is_none_or(str::is_empty))) },
                Err(e) => notice(&format!("{e:#}")),
            }
        }
        "template_remove" => match crate::calls::template_remove(v["id"].as_str().unwrap_or("")) {
            Ok(()) => Handled { refresh: false, reply: Some("window.echo && window.echo.templateSaved(null)".into()) },
            Err(e) => notice(&format!("{e:#}")),
        },
        "view_state" => {
            if let Err(e) = crate::view::save(&v["view"]) {
                eprintln!("window view not saved: {e:#}");
            }
            false.into()
        }
        // Where the window's panels and views go (`Layout` in app.js); nothing else reads it.
        "save_layout" => {
            if let Err(e) = save_layout(&v["layout"]) {
                eprintln!("window layout not saved: {e:#}");
            }
            false.into()
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_codes_paths_show_the_home_folder_as_a_tilde() {
        let home = Path::new("/Users/someone");
        let mut v = json!({ "claude_code": { "path": "/Users/someone/.local/bin/claude", "version": "2.1",
                            "terminal": { "ok": true, "found": "/Users/someone/.local/bin/claude", "dir": "/Users/someone/.local/bin" } } });
        home_as_tilde(&mut v, home);
        assert_eq!(v["claude_code"]["path"], "~/.local/bin/claude");
        assert_eq!(v["claude_code"]["terminal"]["found"], "~/.local/bin/claude");
        assert_eq!(v["claude_code"]["terminal"]["dir"], "~/.local/bin");
        assert_eq!(v["claude_code"]["version"], "2.1");
    }

    #[test]
    fn the_home_folder_itself_is_a_tilde() {
        let mut v = json!({ "claude_code": { "terminal": { "dir": "/Users/someone" } } });
        home_as_tilde(&mut v, Path::new("/Users/someone"));
        assert_eq!(v["claude_code"]["terminal"]["dir"], "~");
    }

    #[test]
    fn paths_outside_the_home_folder_and_missing_ones_stay() {
        let home = Path::new("/Users/someone");
        let mut v = json!({ "claude_code": { "path": "/opt/homebrew/bin/claude", "terminal": { "ok": null, "why": "no answer" } } });
        home_as_tilde(&mut v, home);
        assert_eq!(v["claude_code"]["path"], "/opt/homebrew/bin/claude");
        assert!(v["claude_code"]["terminal"].get("found").is_none());
        // A folder whose name only starts like the home folder's is not in it.
        let mut w = json!({ "claude_code": { "path": "/Users/someone-else/bin/claude" } });
        home_as_tilde(&mut w, home);
        assert_eq!(w["claude_code"]["path"], "/Users/someone-else/bin/claude");
        let mut c = json!({ "claude_code": { "checking": true } });
        home_as_tilde(&mut c, home);
        assert_eq!(c, json!({ "claude_code": { "checking": true } }));
        // A home that is the root would make every path ~/…: nothing changes.
        let mut r = json!({ "claude_code": { "path": "/opt/homebrew/bin/claude" } });
        home_as_tilde(&mut r, Path::new("/"));
        assert_eq!(r["claude_code"]["path"], "/opt/homebrew/bin/claude");
    }
}

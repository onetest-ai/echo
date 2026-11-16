//! AI profiles: `echoctl ai …` (split out of `ai.rs`, moved as it was).

use super::*;

// ── `echoctl ai …` ──────────────────────────────────────────────────────────────────────────

/// `echoctl ai default <id>`
pub fn set_default(id: &str) -> Result<()> {
    let name = Settings::update(|s| {
        let name = s.get(id)?.name;
        s.default = (id != SYSTEM).then(|| id.to_string());
        Ok(name)
    })?;
    println!("new calls use {name}");
    Ok(())
}

/// `echoctl ai live on|off`
pub fn set_live(on: bool) -> Result<()> {
    Settings::update(|s| {
        s.live_notes = on;
        Ok(())
    })?;
    println!("live notes during calls: {}", if on { "on" } else { "off" });
    Ok(())
}

pub(super) fn add(p: Profile) -> Result<()> {
    if !valid_id(&p.id) {
        bail!("a profile id is lower-case letters, digits and dashes (and not \"system\")");
    }
    let (id, name) = (p.id.clone(), p.name.clone());
    Settings::update(|s| {
        if s.profiles.iter().any(|x| x.id == p.id) {
            bail!("there is already a profile \"{}\" — `echoctl ai remove {}` first", p.id, p.id);
        }
        s.profiles.push(p);
        Ok(())
    })?;
    println!("added {name} ({id})");
    Ok(())
}

/// `echoctl ai add-local <id> --model … [--live-model …] [--url …] [--token …] [--name …]`
pub fn add_local(id: &str, name: Option<String>, model: &str, live_model: Option<String>, url: &str, token: Option<String>) -> Result<()> {
    let url = url.trim().trim_end_matches('/').to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("the server address starts with http:// or https:// (Ollama: http://localhost:11434)");
    }
    add(Profile {
        id: id.into(),
        name: name.unwrap_or_else(|| format!("{model} · this computer")),
        kind: Kind::Local,
        base_url: Some(url),
        token,
        live_model: live_model.or_else(|| Some(model.to_string())),
        notes_model: Some(model.to_string()),
        account: None,
        email: None,
        sso: false,
        wrapper: None,
    })
}

/// `echoctl ai add-claude <id> [--name …] [--live-model haiku] [--notes-model sonnet] [--email …] [--sso]`
pub fn add_claude(id: &str, name: Option<String>, live_model: Option<String>, notes_model: Option<String>, email: Option<String>, sso: bool) -> Result<()> {
    add(Profile {
        id: id.into(),
        name: name.unwrap_or_else(|| format!("Claude · {id}")),
        kind: Kind::Claude,
        base_url: None,
        token: None,
        live_model: Some(live_model.unwrap_or_else(|| crate::config::DEFAULT_LIVE_MODEL.into())),
        notes_model: Some(notes_model.unwrap_or_else(|| crate::config::DEFAULT_NOTES_MODEL.into())),
        account: None,
        email: email_arg(email)?,
        sso,
        wrapper: None,
    })?;
    println!("sign it in once: Sign in… in the Echo window, or echoctl ai login {id}");
    Ok(())
}

/// `echoctl ai add-wrapper <id> --command <program> [--name …]`, and the window's: a wrapper profile, only by the user
/// (never inside a run Echo started), only once its command resolves outside Echo's folders and answers as Claude Code
/// (claude-code-runs R13). `offered_by`: the plugin whose offer the user took.
pub fn add_wrapper(id: &str, command: &str, name: Option<String>, offered_by: Option<String>) -> Result<Profile> {
    if crate::notes::headless() {
        bail!("refused inside a run Echo started: which program Echo starts as Claude Code is the user's to choose");
    }
    let (path, version) = wrapper::admit(command)?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| id.to_string());
    let p = Profile {
        id: id.into(),
        name: name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or(stem),
        kind: Kind::Wrapper,
        base_url: None,
        token: None,
        live_model: None,
        notes_model: None,
        account: None,
        email: None,
        sso: false,
        wrapper: Some(wrapper::Wrapper { command: path.display().to_string(), offered_command: offered_by.is_some().then(|| command.trim().to_string()), offered_by }),
    };
    add(p.clone())?;
    println!("{} starts {} ({version}); its jobs, call transcripts included, go wherever that program sends them", p.name, path.display());
    Ok(p)
}

/// The wrapper profiles plugins offer (`uses.ai.wrapper`), for AI & minutes and `ai status`: each enabled plugin's
/// `{plugin, plugin_name, name, command}`, with the profile the user added from it (`profile`), and that profile's
/// program when the plugin now offers another (`update`: taken only on the user's yes, `update_wrapper`).
pub fn offered() -> Vec<Value> {
    let plugins: Vec<(String, String, Value)> = crate::plugins::enabled_manifests().into_iter().map(|(id, m)| (id, m.name.clone(), Value::Object(m.uses.clone()))).collect();
    offered_in(&plugins, &Settings::load().unwrap_or_default())
}

/// `wrapper_offers` from `(id, name, uses)` of the enabled plugins and the AI settings.
pub(super) fn offered_in(plugins: &[(String, String, Value)], s: &Settings) -> Vec<Value> {
    plugins
        .iter()
        .filter_map(|(id, plugin_name, uses)| {
            let w = &uses["ai"]["wrapper"];
            let command = w["command"].as_str().filter(|c| wrapper::program_name(c))?;
            let name = crate::plugins::wrapper_name(w)?;
            let taken = s.profiles.iter().find(|p| p.wrapper.as_ref().is_some_and(|w| w.offered_by.as_deref() == Some(id.as_str())));
            let update = taken.and_then(|p| p.wrapper.as_ref()).filter(|w| w.offered_command.as_deref() != Some(command)).map(|w| w.command.clone());
            Some(json!({"plugin": id, "plugin_name": plugin_name, "name": name, "command": command, "profile": taken.map(|p| &p.id), "update": update}))
        })
        .collect()
}

/// `echoctl ai add-wrapper <id> --plugin <plugin>`, the window's Add: the profile a plugin offers, added by the user —
/// the same checks as one typed in (`add_wrapper`). `id`: none, one made from the plugin's.
pub fn add_offered_wrapper(plugin: &str, id: Option<&str>) -> Result<Profile> {
    let offer = offered().into_iter().find(|o| o["plugin"] == plugin).ok_or_else(|| anyhow!("no enabled plugin “{plugin}” offers a wrapper"))?;
    if let Some(p) = offer["profile"].as_str() {
        bail!("its wrapper is the profile “{p}” already");
    }
    let id = match id {
        Some(i) => i.to_string(),
        None => {
            let s = Settings::load()?;
            let base: String = plugin.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').map(|c| c.to_ascii_lowercase()).take(32).collect();
            let base = if base.is_empty() || base == SYSTEM { "wrapper".to_string() } else { base };
            (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|c| !s.profiles.iter().any(|p| p.id == *c)).unwrap_or(base)
        }
    };
    add_wrapper(&id, offer["command"].as_str().unwrap_or(""), offer["name"].as_str().map(str::to_owned), Some(plugin.to_string()))
}

/// `echoctl ai update-wrapper <id>`, the window's Use it: the program its plugin now offers, taken on the user's yes —
/// found and checked as when it was added. Its full path.
pub fn update_wrapper(id: &str) -> Result<String> {
    if crate::notes::headless() {
        bail!("refused inside a run Echo started: which program Echo starts as Claude Code is the user's to choose");
    }
    let offer = offered().into_iter().find(|o| o["profile"] == id).ok_or_else(|| anyhow!("no enabled plugin offers “{id}” another program"))?;
    if offer["update"].is_null() {
        bail!("“{id}” starts what its plugin offers already");
    }
    let command = offer["command"].as_str().unwrap_or("").to_string();
    let (path, version) = wrapper::admit(&command)?;
    let shown = path.display().to_string();
    Settings::update(|s| {
        let p = s.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        let w = p.wrapper.as_mut().ok_or_else(|| anyhow!("{} isn't a wrapper profile", p.name))?;
        w.command = shown.clone();
        w.offered_command = Some(command.clone());
        Ok(())
    })?;
    println!("{id} now starts {shown} ({version})");
    Ok(shown)
}

pub(super) fn email_arg(email: Option<String>) -> Result<Option<String>> {
    let email = email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    if email.as_deref().is_some_and(|e| !e.contains('@') || e.chars().any(char::is_whitespace)) {
        bail!("that isn't an email address");
    }
    Ok(email)
}

/// `echoctl ai set <id> [--email … | --no-email] [--sso on|off]`: how a Claude profile signs in.
pub fn set_sign_in(id: &str, email: Option<String>, no_email: bool, sso: Option<bool>) -> Result<()> {
    let email = email_arg(email)?;
    let (name, email, sso) = Settings::update(|s| {
        let p = s.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        if !p.kind.signs_in() {
            bail!("{} doesn't sign in (only Claude account profiles do)", p.name);
        }
        if no_email {
            p.email = None;
        } else if email.is_some() {
            p.email = email.clone();
        }
        if let Some(sso) = sso {
            p.sso = sso;
        }
        Ok((p.name.clone(), p.email.clone(), p.sso))
    })?;
    println!("{name}: email {}, single sign-on {}", email.as_deref().unwrap_or("not set"), if sso { "on" } else { "off" });
    Ok(())
}

/// `echoctl ai set <id> --notes-model <m>`, the menu's "Minutes model for …": the model that writes
/// a profile's minutes. `system` → `config.notes_model` ("default" = Claude Code's own; empty or
/// sonnet = unset); `claude` / `local` → its `notes_model` (empty = back to sonnet / its live
/// model). Returns the profile's name and its minutes model now, in words.
pub fn set_notes_model(id: &str, model: &str) -> Result<(String, String)> {
    let m = model.trim().to_string();
    if id == SYSTEM {
        let mut cfg = Config::load()?;
        cfg.notes_model = Some(m).filter(|m| !m.is_empty() && m != crate::config::DEFAULT_NOTES_MODEL);
        cfg.save()?;
        let p = system_profile();
        return Ok((p.name.clone(), p.notes_model_label()));
    }
    Settings::update(|s| {
        let p = s.profiles.iter_mut().find(|p| p.id == id).ok_or_else(|| anyhow!("no profile \"{id}\""))?;
        p.notes_model = Some(m.clone()).filter(|m| !m.is_empty());
        Ok((p.name.clone(), p.notes_model_label()))
    })
}

/// `echoctl ai remove <id>` (its Claude Code folder, with the sign-in, is kept).
pub fn remove(id: &str) -> Result<()> {
    let was_default = Settings::update(|s| {
        let before = s.profiles.len();
        s.profiles.retain(|p| p.id != id);
        if s.profiles.len() == before {
            bail!("no profile \"{id}\"");
        }
        let was = s.default.as_deref() == Some(id);
        if was {
            s.default = None;
        }
        Ok(was)
    })?;
    if was_default {
        println!("new calls now use Claude Code (as configured)");
    }
    println!("removed {id}");
    Ok(())
}

/// `echoctl ai status [<id>]`: each profile, its account (Claude profiles sign-in checked now).
pub fn status(id: Option<&str>) -> Result<()> {
    let s = Settings::load()?;
    let mut out = vec![];
    for p in s.all().into_iter().filter(|p| id.is_none_or(|i| i == p.id)) {
        let (signed_in, account, problem) = match p.kind {
            Kind::Claude => {
                match observe(&p, &|| false) {
                    Ok(Some(a)) => (Some(true), Some(a.label()), settle(&p, Some(&a)).err().map(|e| format!("{e:#}"))),
                    Ok(None) => (Some(false), None, None),
                    Err(e) => (None, None, Some(format!("{e:#}"))),
                }
            }
            _ => (None, None, check(&p, None).err().map(|e| format!("{e:#}"))),
        };
        let conflict = p.kind.managed().then(|| policy_conflict(&p)).flatten();
        out.push(crate::ollama::with_context(&p, json!({
            "id": p.id, "name": p.name, "kind": p.kind, "default": s.default_id() == p.id,
            "live_model": p.live_model(), "notes_model": p.notes_model(), "base_url": p.base_url,
            "command": p.wrapper.as_ref().map(|w| &w.command), "offered_by": p.wrapper.as_ref().and_then(|w| w.offered_by.as_ref()),
            "signed_in": signed_in, "account": account, "expected_account": p.account,
            "problem": problem.or(conflict),
        })));
    }
    println!("{}", serde_json::to_string_pretty(&json!({ "live_notes": s.live_notes, "profiles": out, "wrapper_offers": offered() }))?);
    Ok(())
}

/// For the window.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn summary() -> Value {
    match Settings::load() {
        Ok(s) => json!({
            "default": s.default_id(),
            "live_notes": s.live_notes,
            "wrapper_offers": offered(),
            "profiles": s.all().iter().map(|p| json!({
                "id": p.id, "name": p.name, "kind": p.kind, "live_model": p.live_model(),
                "notes_model": p.notes_model(), "notes_model_label": p.notes_model_label(), "account": p.account, "email": p.email, "sso": p.sso,
                // `local`: where the requests go when it isn't this computer
                "host": (p.kind == Kind::Local && !p.on_this_computer()).then(|| p.base_url.clone()).flatten(),
                // `claude`: its sign-in now (`signin::state`; no process runs for it)
                "signin": crate::signin::state(p),
                // `wrapper`: the program Echo starts in place of `claude`, and the plugin that offered it
                "command": p.wrapper.as_ref().map(|w| &w.command), "offered_by": p.wrapper.as_ref().and_then(|w| w.offered_by.as_ref()),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e:#}") }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapper(id: &str, command: &str, by: Option<&str>, offered: Option<&str>) -> Profile {
        Profile { id: id.into(), name: id.into(), kind: Kind::Wrapper, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false,
            wrapper: Some(wrapper::Wrapper { command: command.into(), offered_by: by.map(str::to_owned), offered_command: offered.map(str::to_owned) }) }
    }

    #[test]
    fn offers_are_the_enabled_plugins_wrappers_with_the_profile_taken_and_its_update() {
        let uses = |command: &str| json!({"ai": {"wrapper": {"name": "Acme Claude", "command": command}}});
        let plugins = vec![
            ("acme-tools".to_string(), "Acme Tools".to_string(), uses("acme-claude2")),
            ("globex".to_string(), "Globex".to_string(), uses("globex-claude")),
            ("bad".to_string(), "Bad".to_string(), uses("/bin/sh")),
            ("none".to_string(), "None".to_string(), json!({"network": {}})),
        ];
        let s = Settings { default: None, live_notes: false, profiles: vec![wrapper("acme", "/opt/acme/acme-claude", Some("acme-tools"), Some("acme-claude")), wrapper("mine", "/opt/x/y", None, None)] };
        let o = offered_in(&plugins, &s);
        assert_eq!(o.len(), 2, "only a valid offer: {o:?}");
        assert_eq!((o[0]["plugin"].as_str(), o[0]["profile"].as_str(), o[0]["update"].as_str()), (Some("acme-tools"), Some("acme"), Some("/opt/acme/acme-claude")), "its update names another program");
        assert_eq!((o[1]["plugin"].as_str(), o[1]["profile"].is_null(), o[1]["update"].is_null()), (Some("globex"), true, true), "not added yet");
        let same = Settings { profiles: vec![wrapper("acme", "/opt/acme/acme-claude2", Some("acme-tools"), Some("acme-claude2"))], ..Settings::default() };
        assert!(offered_in(&plugins, &same)[0]["update"].is_null(), "the program it took: nothing to ask");
        // The plugin off or removed: its offer goes; the profile the user added stays theirs.
        assert!(offered_in(&plugins[1..], &s).iter().all(|x| x["plugin"] != "acme-tools"));
    }
}

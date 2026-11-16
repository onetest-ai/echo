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
    })?;
    println!("sign it in once: Sign in… in the Echo window, or echoctl ai login {id}");
    Ok(())
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
        let conflict = (p.kind != Kind::System).then(|| policy_conflict(&p)).flatten();
        out.push(crate::ollama::with_context(&p, json!({
            "id": p.id, "name": p.name, "kind": p.kind, "default": s.default_id() == p.id,
            "live_model": p.live_model(), "notes_model": p.notes_model(), "base_url": p.base_url,
            "signed_in": signed_in, "account": account, "expected_account": p.account,
            "problem": problem.or(conflict),
        })));
    }
    println!("{}", serde_json::to_string_pretty(&json!({ "live_notes": s.live_notes, "profiles": out }))?);
    Ok(())
}

/// For the window.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn summary() -> Value {
    match Settings::load() {
        Ok(s) => json!({
            "default": s.default_id(),
            "live_notes": s.live_notes,
            "profiles": s.all().iter().map(|p| json!({
                "id": p.id, "name": p.name, "kind": p.kind, "live_model": p.live_model(),
                "notes_model": p.notes_model(), "notes_model_label": p.notes_model_label(), "account": p.account, "email": p.email, "sso": p.sso,
                // `local`: where the requests go when it isn't this computer
                "host": (p.kind == Kind::Local && !p.on_this_computer()).then(|| p.base_url.clone()).flatten(),
                // `claude`: its sign-in now (`signin::state`; no process runs for it)
                "signin": crate::signin::state(p),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e:#}") }),
    }
}

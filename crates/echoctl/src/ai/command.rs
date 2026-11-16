//! AI profiles: `echoctl ai …` (split out of `ai.rs`, moved as it was).

use super::*;

// ── `echoctl ai …` ──────────────────────────────────────────────────────────────────────────

/// `echoctl ai default <id>`
pub fn set_default(id: &str) -> Result<()> {
    let name = Settings::update(|s| {
        wrapper_in_run(s, id, crate::notes::headless())?;
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

/// Which plugin's offer a wrapper profile was added from: its id, marketplace and name, and the program it offered.
pub struct Offer {
    pub plugin: String,
    pub market: String,
    pub plugin_name: String,
    pub command: String,
}

const IN_RUN: &str = "refused inside a run Echo started: which program Echo starts as Claude Code is the user's to choose";

/// `echoctl ai add-wrapper <id> --command <program> [--name …]`, and the window's: a wrapper profile, only by the user
/// (never inside a run Echo started), only once its command resolves outside Echo's folders and answers as Claude Code
/// (claude-code-runs R13). `checked`: the full path the user was shown (the window's Check) — added only if the
/// command still resolves to it. `offer`: the plugin's offer the user took.
pub fn add_wrapper(id: &str, command: &str, name: Option<String>, offer: Option<Offer>, checked: Option<&str>) -> Result<Profile> {
    add_wrapper_in(id, command, name, offer, checked, crate::notes::headless())
}

fn add_wrapper_in(id: &str, command: &str, name: Option<String>, offer: Option<Offer>, checked: Option<&str>, in_run: bool) -> Result<Profile> {
    if in_run {
        bail!("{IN_RUN}");
    }
    let (path, version) = wrapper::admit(command)?;
    if let Some(c) = checked.filter(|c| Path::new(c) != path) {
        bail!("{} now starts {}, not the {c} you checked — check it again", command.trim(), path.display());
    }
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
        wrapper: Some(match offer {
            Some(o) => wrapper::Wrapper { command: path.display().to_string(), offered_by: Some(o.plugin), offered_market: Some(o.market), offered_name: Some(o.plugin_name), offered_command: Some(o.command) },
            None => wrapper::Wrapper { command: path.display().to_string(), offered_by: None, offered_market: None, offered_name: None, offered_command: None },
        }),
    };
    add(p.clone())?;
    println!("{} starts {} ({version}); its jobs, call transcripts included, go wherever that program sends them", p.name, path.display());
    Ok(p)
}

/// A run Echo started never makes a wrapper profile the default, a call's or a thread's (claude-code-runs R13).
pub fn wrapper_in_run(s: &Settings, id: &str, in_run: bool) -> Result<()> {
    if in_run && s.profiles.iter().any(|p| p.id == id && p.kind == Kind::Wrapper) {
        bail!("refused inside a run Echo started: only the user chooses a wrapper profile for their calls and threads");
    }
    Ok(())
}

/// A profile a plugin's job names (`ctx.claude.run({profile})`): a wrapper profile only when it is the user's default or
/// `thread_pin` — the thread's own (claude-code-runs R13).
#[cfg_attr(not(feature = "js"), allow(dead_code))] // plugins' jobs (`js`)
pub fn plugin_may_name(s: &Settings, id: &str, thread_pin: Option<&str>) -> Result<()> {
    let wrapper = s.profiles.iter().any(|p| p.id == id && p.kind == Kind::Wrapper);
    if wrapper && s.default_id() != id && thread_pin != Some(id) {
        bail!("a plugin's run can't choose the wrapper profile “{id}”: it runs on one only when it is your default");
    }
    Ok(())
}

/// The wrapper profiles plugins offer (`uses.ai.wrapper`), for AI & minutes and `ai status`: each enabled plugin's
/// `{plugin, market, plugin_name, name, command}`, with the profile the user added from it (`profile`: from that plugin
/// of that marketplace), and that profile's program when the plugin now offers another (`update`: taken only on the
/// user's yes, `update_wrapper`). An offer whose program is a wrapper profile already isn't offered again.
pub fn offered() -> Vec<Value> {
    let plugins: Vec<(String, String, String, Value)> = crate::plugins::enabled()
        .into_iter()
        .filter_map(|p| crate::plugins::read_manifest(&crate::plugins::plugin_dir(&p)).ok().map(|m| (p.id.clone(), p.market.clone(), m.name.clone(), Value::Object(m.uses.clone()))))
        .collect();
    offered_in(&plugins, &Settings::load().unwrap_or_default())
}

/// `offered` from `(id, marketplace, name, uses)` of the enabled plugins and the AI settings.
pub(super) fn offered_in(plugins: &[(String, String, String, Value)], s: &Settings) -> Vec<Value> {
    plugins
        .iter()
        .filter_map(|(id, market, plugin_name, uses)| {
            let w = &uses["ai"]["wrapper"];
            let command = w["command"].as_str().filter(|c| wrapper::program_name(c))?;
            let name = crate::plugins::wrapper_name(w)?;
            let taken = s.profiles.iter().find(|p| p.wrapper.as_ref().is_some_and(|w| w.offered_by.as_deref() == Some(id.as_str()) && w.offered_market.as_deref().is_none_or(|m| m == market)));
            if taken.is_none() && s.profiles.iter().any(|p| p.wrapper.as_ref().is_some_and(|w| Path::new(&w.command).file_name().is_some_and(|f| f == command))) {
                return None;
            }
            let update = taken.and_then(|p| p.wrapper.as_ref()).filter(|w| w.offered_command.as_deref() != Some(command)).map(|w| w.command.clone());
            Some(json!({"plugin": id, "market": market, "plugin_name": plugin_name, "name": name, "command": command, "profile": taken.map(|p| &p.id), "update": update}))
        })
        .collect()
}

/// `echoctl ai add-wrapper [<id>] --plugin <plugin>`, the window's Add: the profile a plugin offers, added by the user —
/// the same checks as one typed in (`add_wrapper`). `id`: none, one made from the plugin's. `checked`: as `add_wrapper`'s,
/// with the program the user checked (`checked_command`), which must be the one offered now.
pub fn add_offered_wrapper(plugin: &str, id: Option<&str>, checked_command: Option<&str>, checked: Option<&str>) -> Result<Profile> {
    add_offered_in(&offered(), &Settings::load()?, plugin, id, checked_command, checked, crate::notes::headless())
}

#[allow(clippy::too_many_arguments)]
fn add_offered_in(offers: &[Value], s: &Settings, plugin: &str, id: Option<&str>, checked_command: Option<&str>, checked: Option<&str>, in_run: bool) -> Result<Profile> {
    if in_run {
        bail!("{IN_RUN}");
    }
    let offer = offers.iter().find(|o| o["plugin"] == plugin).ok_or_else(|| anyhow!("no enabled plugin “{plugin}” offers a wrapper"))?;
    if let Some(p) = offer["profile"].as_str() {
        bail!("its wrapper is the profile “{p}” already");
    }
    let command = offer["command"].as_str().unwrap_or("").to_string();
    if checked_command.is_some_and(|c| c != command) {
        bail!("the plugin now offers {command}, not the program you checked — check it again");
    }
    let id = match id {
        Some(i) => i.to_string(),
        None => {
            let base: String = plugin.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').map(|c| c.to_ascii_lowercase()).take(32).collect();
            let base = if base.is_empty() || base == SYSTEM { "wrapper".to_string() } else { base };
            (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|c| !s.profiles.iter().any(|p| p.id == *c)).unwrap_or(base)
        }
    };
    let o = Offer { plugin: plugin.into(), market: offer["market"].as_str().unwrap_or("").into(), plugin_name: offer["plugin_name"].as_str().unwrap_or(plugin).into(), command: command.clone() };
    add_wrapper_in(&id, &command, offer["name"].as_str().map(str::to_owned), Some(o), checked, false)
}

/// `echoctl ai update-wrapper <id>`, the window's Use it: the program its plugin now offers, taken on the user's yes —
/// found and checked as when it was added. Its full path.
pub fn update_wrapper(id: &str) -> Result<String> {
    let (command, path, version) = update_checked(&offered(), id, crate::notes::headless())?;
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

/// What `update_wrapper` would take: the program offered, found and checked.
fn update_checked(offers: &[Value], id: &str, in_run: bool) -> Result<(String, PathBuf, String)> {
    if in_run {
        bail!("{IN_RUN}");
    }
    let offer = offers.iter().find(|o| o["profile"] == id).ok_or_else(|| anyhow!("no enabled plugin offers “{id}” another program"))?;
    if offer["update"].is_null() {
        bail!("“{id}” starts what its plugin offers already");
    }
    let command = offer["command"].as_str().unwrap_or("").to_string();
    let (path, version) = wrapper::admit(&command)?;
    Ok((command, path, version))
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
            "command": p.wrapper.as_ref().map(|w| &w.command), "offered_by": p.wrapper.as_ref().and_then(|w| w.offered_by.as_ref()), "offered_name": p.wrapper.as_ref().and_then(|w| w.offered_name.as_ref().or(w.offered_by.as_ref())),
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
                "command": p.wrapper.as_ref().map(|w| &w.command), "offered_by": p.wrapper.as_ref().and_then(|w| w.offered_by.as_ref()), "offered_name": p.wrapper.as_ref().and_then(|w| w.offered_name.as_ref().or(w.offered_by.as_ref())),
            })).collect::<Vec<_>>(),
        }),
        Err(e) => json!({ "error": format!("{e:#}") }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrapper(id: &str, command: &str, by: Option<(&str, &str)>, offered: Option<&str>) -> Profile {
        Profile { id: id.into(), name: id.into(), kind: Kind::Wrapper, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false,
            wrapper: Some(wrapper::Wrapper { command: command.into(), offered_by: by.map(|b| b.0.to_owned()), offered_market: by.map(|b| b.1.to_owned()), offered_name: by.map(|b| b.0.to_uppercase()), offered_command: offered.map(str::to_owned) }) }
    }

    fn plugin(id: &str, market: &str, command: &str) -> (String, String, String, Value) {
        (id.into(), market.into(), format!("{id} name"), json!({"ai": {"wrapper": {"name": "Acme Claude", "command": command}}}))
    }

    fn settings(profiles: Vec<Profile>, default: Option<&str>) -> Settings {
        Settings { default: default.map(str::to_owned), live_notes: false, profiles }
    }

    #[test]
    fn offers_are_the_enabled_plugins_wrappers_with_the_profile_taken_and_its_update() {
        let plugins = vec![
            plugin("acme-tools", "acme-market", "acme-claude2"),
            plugin("globex", "globex-market", "globex-claude"),
            plugin("bad", "m", "/bin/sh"),
            ("none".to_string(), "m".to_string(), "None".to_string(), json!({"network": {}})),
        ];
        let s = settings(vec![wrapper("acme", "/opt/acme/acme-claude", Some(("acme-tools", "acme-market")), Some("acme-claude")), wrapper("mine", "/opt/x/y", None, None)], None);
        let o = offered_in(&plugins, &s);
        assert_eq!(o.len(), 2, "only a valid offer: {o:?}");
        assert_eq!((o[0]["plugin"].as_str(), o[0]["profile"].as_str(), o[0]["update"].as_str()), (Some("acme-tools"), Some("acme"), Some("/opt/acme/acme-claude")), "its update names another program");
        assert_eq!((o[1]["plugin"].as_str(), o[1]["profile"].is_null(), o[1]["update"].is_null()), (Some("globex"), true, true), "not added yet");
        let same = settings(vec![wrapper("acme", "/opt/acme/acme-claude2", Some(("acme-tools", "acme-market")), Some("acme-claude2"))], None);
        assert!(offered_in(&plugins, &same)[0]["update"].is_null(), "the program it took: nothing to ask");
        // The plugin off or removed: its offer goes; the profile the user added stays theirs.
        assert!(offered_in(&plugins[1..], &s).iter().all(|x| x["plugin"] != "acme-tools"));
    }

    #[test]
    fn a_same_named_plugin_from_another_marketplace_offers_nothing_to_the_profile() {
        let s = settings(vec![wrapper("acme", "/opt/acme/acme-claude", Some(("acme-tools", "acme-market")), Some("acme-claude"))], None);
        let o = offered_in(&[plugin("acme-tools", "elsewhere", "evil-claude")], &s);
        assert_eq!((o[0]["profile"].is_null(), o[0]["update"].is_null()), (true, true), "not its update: {o:?}");
    }

    #[test]
    fn an_offer_whose_program_is_a_profile_already_isnt_offered_again() {
        let s = settings(vec![wrapper("mine", "/opt/acme/bin/acme-claude", None, None)], None);
        assert!(offered_in(&[plugin("acme-tools", "m", "acme-claude")], &s).is_empty());
        assert_eq!(offered_in(&[plugin("acme-tools", "m", "other-claude")], &s).len(), 1);
    }

    #[test]
    fn a_plugins_job_runs_on_a_wrapper_only_when_it_is_the_default_or_the_threads() {
        let s = settings(vec![wrapper("acme", "/opt/acme/acme-claude", None, None)], Some("work"));
        assert!(plugin_may_name(&s, "acme", None).unwrap_err().to_string().contains("can't choose the wrapper profile"));
        assert!(plugin_may_name(&s, "acme", Some("acme")).is_ok(), "the thread's pin");
        assert!(plugin_may_name(&s, "work", None).is_ok(), "not a wrapper");
        let def = settings(vec![wrapper("acme", "/opt/acme/acme-claude", None, None)], Some("acme"));
        assert!(plugin_may_name(&def, "acme", None).is_ok(), "the user's default");
    }

    #[test]
    fn a_run_never_makes_a_wrapper_the_default_a_calls_or_a_threads() {
        let s = settings(vec![wrapper("acme", "/opt/acme/acme-claude", None, None)], None);
        assert!(wrapper_in_run(&s, "acme", true).unwrap_err().to_string().contains("refused inside a run"));
        assert!(wrapper_in_run(&s, "acme", false).is_ok());
        assert!(wrapper_in_run(&s, "work", true).is_ok(), "another kind is the run's own business");
    }

    #[test]
    fn an_offer_is_added_and_updated_only_by_the_user_once_and_only_as_checked() {
        let offers = vec![
            json!({"plugin": "acme-tools", "market": "m", "plugin_name": "Acme Tools", "name": "Acme Claude", "command": "acme-claude", "profile": null, "update": null}),
            json!({"plugin": "globex", "market": "m", "plugin_name": "Globex", "name": "Globex", "command": "globex-claude", "profile": "globex", "update": null}),
            json!({"plugin": "initech", "market": "m", "plugin_name": "Initech", "name": "Initech", "command": "initech-claude2", "profile": "initech", "update": "/opt/initech-claude"}),
        ];
        let s = Settings::default();
        assert!(add_offered_in(&offers, &s, "acme-tools", None, None, None, true).unwrap_err().to_string().contains("refused inside a run"));
        assert!(add_offered_in(&offers, &s, "globex", None, None, None, false).unwrap_err().to_string().contains("already"));
        assert!(add_offered_in(&offers, &s, "acme-tools", None, Some("other-claude"), None, false).unwrap_err().to_string().contains("check it again"));
        assert!(add_offered_in(&offers, &s, "nobody", None, None, None, false).is_err());
        assert!(update_checked(&offers, "initech", true).unwrap_err().to_string().contains("refused inside a run"));
        assert!(update_checked(&offers, "globex", false).unwrap_err().to_string().contains("already"), "nothing to update");
        assert!(update_checked(&offers, "acme", false).unwrap_err().to_string().contains("no enabled plugin"));
    }

    #[cfg(unix)]
    #[test]
    fn added_only_if_it_still_starts_what_check_showed() {
        let (_env, root) = crate::paths::testenv::folders("ai-wrapper-checked");
        let _fake = super::super::fake::install(&root);
        let w = root.join("acme-claude");
        fs::write(&w, "#!/bin/sh\n[ \"$ECHOCTL_HEADLESS\" = 1 ] && echo '2.1.400 (Claude Code)'\n").unwrap();
        fs::set_permissions(&w, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let real = dunce::canonicalize(&w).unwrap();
        let e = add_wrapper_in("acme", w.to_str().unwrap(), None, None, Some("/opt/elsewhere/acme-claude"), false).unwrap_err().to_string();
        assert!(e.contains("not the /opt/elsewhere/acme-claude you checked"), "{e}");
        assert!(Settings::load().unwrap().get("acme").is_err(), "nothing added");
        let p = add_wrapper_in("acme", w.to_str().unwrap(), None, None, Some(real.to_str().unwrap()), false).unwrap();
        assert_eq!(p.wrapper.unwrap().command, real.display().to_string());
    }
}

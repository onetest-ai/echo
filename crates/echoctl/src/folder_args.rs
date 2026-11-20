//! What every Claude Code session Echo starts in its data folder gets — the folder session (`terminal::start_folder`),
//! a plugin's run (a routine, `ctx.claude.run`) and a Yolo task (`claudejob::run`) alike (#590): the same knowledge
//! (the core's SessionStart hook, what plugins lay out in `.plugins/`, Echo's own plugin, the user's skills, the
//! window's view hook) and the same permissions (the plugins' commands and scripts allowed, the folders plugins were
//! given, the OneDrive and `.plugins` write checks, and how the user chose each to handle permission prompts).

// Without the window (`tray`) or plugin code (`js`) nobody starts one.
#![cfg_attr(not(any(feature = "tray", feature = "js")), allow(dead_code))]

use std::path::Path;

use anyhow::Result;

use crate::config::Permissions;
use crate::paths;

/// Echo commands any of its sessions runs without a prompt (`Bash(<cli> <command>:*)`, each a fixed command and its own
/// arguments): what reads Echo's data, and edits of Echo's own data — a thread's work (`project work <thread> [<edit>]`,
/// one command for the tree and an edit, so no pattern reaches another ledger command), a mail left as a draft. Never a
/// send, an install or a setting.
pub const FREE_COMMANDS: [&str; 16] = [
    "context", "attention", "prep", "previous", "status", "calendar events", "read", "project list", "project show", "project of",
    "project tasks", "project people", "client list", "org list",
    // Echo's own data.
    "project work", "compose",
];

/// Who a session is: the folder session (the user there), a plugin's session the user opened in a folder (the user
/// there, with these permissions: `terminal::start_plugin`), or a routine's or Yolo's run, acting on its own with these
/// permissions (`claudejob`: the routine's own choice, else `run_permissions`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Who {
    Folder,
    Session(Permissions),
    Run(Permissions),
}

impl Who {
    /// How it handles permission prompts.
    fn permissions(self) -> Permissions {
        match self {
            Who::Folder => folder_permissions(),
            Who::Session(p) | Who::Run(p) => p,
        }
    }
}

/// Where a session works: Echo's data folder, or a plugin's folder (`claudejob::in_folder`: a folder the user gave it,
/// or one of its store) where it may change files (`Edit`), or only read (`Read`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    Data,
    Edit,
    Read,
}

/// What a run that changes nothing may run of Echo's CLI (`Place::Read`): `FREE_COMMANDS`' reading ones, but `prep`
/// (`--write` writes one) and `project people` (`--refresh` asks Microsoft 365 and keeps what it says).
const READ_COMMANDS: [&str; 12] = ["context", "attention", "previous", "status", "calendar events", "read", "project list", "project show", "project of", "project tasks", "client list", "org list"];

/// The arguments, its settings file first (`--settings <settings>`, written here): `managed` — a profile of its own,
/// given Echo's plugin (a profile on the user's own Claude Code has it installed already); `who` — a routine's or
/// Yolo's run gets the window's view hook only at its start (who the user is), not what the window shows; `place` — in a
/// plugin's folder no other folder plugins offer sessions (that folder's own, its source behind a copy: never added),
/// and where it only reads only Echo's reading commands, none of the plugins' commands or scripts.
pub fn build(managed: bool, exe: &Path, settings: &Path, who: Who, place: Place) -> Result<Vec<String>> {
    let unattended = matches!(who, Who::Run(_));
    let mut args: Vec<String> = vec![];
    // What plugins add (never the user's own Claude Code): their part for Claude, laid out in the data folder
    // (`.plugins/<id>/`: loaded when Claude first reads a file there, as the core's SessionStart hook tells it), and the
    // folders the user gave them to read (an Obsidian vault…).
    crate::plugins::agent::sync_all();
    // A plugin whose part isn't laid out (its runtime skills, no prefix yet): its session copy, so its skills still
    // reach Echo's sessions — once each, never beside a laid-out folder.
    let unlaid: Vec<std::path::PathBuf> = crate::grants::session_plugins()
        .into_iter()
        .filter(|d| d.file_name().is_none_or(|id| !crate::plugins::agent::folder(&id.to_string_lossy()).is_dir()))
        .collect();
    for d in &unlaid {
        args.extend(["--plugin-dir".into(), d.display().to_string()]);
    }
    // The commands plugins let Echo's sessions run (`uses.sessions.commands`): no prompt for them.
    let mut clis = crate::plugins::agent::clis();
    clis.extend(crate::grants::session_clis(&unlaid));
    if let Some(d) = crate::notes::plugin_dir() {
        clis.extend(crate::notes::run_clis(&d));
    }
    let commands = if place == Place::Read { vec![] } else { crate::plugins::session_commands() };
    for cli in &clis {
        for (id, c) in &commands {
            args.extend(["--allowedTools".into(), format!("Bash({cli} plugin call {id} {c}:*)")]);
        }
    }
    // Echo's reads and its own data's edits, by Echo's own CLI (its plugin's launcher, `echoctl` on PATH).
    for cli in crate::notes::plugin_dir().map(|d| crate::notes::run_clis(&d)).unwrap_or_else(|| vec!["echoctl".into()]) {
        for c in if place == Place::Read { &READ_COMMANDS[..] } else { &FREE_COMMANDS[..] } {
            args.extend(["--allowedTools".into(), format!("Bash({cli} {c}:*)")]);
        }
    }
    // Their declared scripts, by their launchers' full paths (`uses.scripts`): no prompt for them either.
    for s in crate::plugins::agent::scripts().into_iter().filter(|_| place != Place::Read) {
        args.extend(["--allowedTools".into(), format!("Bash({s}:*)")]);
    }
    // The user's skills (Settings → Templates → Skills), as a plugin of their own: only their descriptions in its
    // context until one is used; Send to Claude on a task names one.
    match crate::skills::plugin() {
        Ok(d) => args.extend(["--plugin-dir".into(), d.display().to_string()]),
        Err(e) => eprintln!("skills: {e:#}"),
    }
    // Echo's own plugin (status line, record / minutes / doctor skills): installed in the folder's project scope it
    // reaches Claude Code as the user set it up; a profile of its own reads only its own settings, so it gets the
    // plugin shipped with the app.
    if managed {
        if let Some(d) = crate::notes::plugin_dir() {
            args.extend(["--plugin-dir".into(), d.display().to_string()]);
        }
    }
    for d in crate::grants::session_read().into_iter().filter(|_| place == Place::Data) {
        args.extend(["--add-dir".into(), d.display().to_string()]);
    }
    // Its own settings file (the user's own settings apply as ever): what the window shows before each prompt and who
    // the user is, the OneDrive and `.plugins` checks before a write, and the core.
    let mut hooks = crate::view::hook_settings(exe);
    if unattended {
        if let Some(h) = hooks.as_object_mut() {
            h.remove("UserPromptSubmit");
        }
    }
    hooks["PreToolUse"] = crate::view::onedrive_hook(exe);
    if let Some(a) = hooks["PreToolUse"].as_array_mut() {
        a.push(crate::plugins::agent::write_hook_entry(exe));
    }
    if !hooks["SessionStart"].is_array() {
        hooks["SessionStart"] = serde_json::json!([]);
    }
    if let Some(a) = hooks["SessionStart"].as_array_mut() {
        a.push(crate::plugins::agent::hook_entry(exe));
    }
    if let Some(f) = settings.parent() {
        std::fs::create_dir_all(f)?;
    }
    std::fs::write(settings, serde_json::to_vec_pretty(&serde_json::json!({"hooks": hooks}))?)?;
    args.splice(0..0, ["--settings".to_string(), settings.display().to_string()]);
    Ok(args)
}

/// What a session in Echo's data folder runs with, after its own conversation's arguments: `model`, `build` (its
/// settings file `settings`) and the Brain (`who`: whose usage it counts). The folder session's
/// (`terminal::start_folder`), and — but for `-p`, its prompt and its model — a routine's and a Yolo task's
/// (`claudejob::run`, `who`).
pub fn session(p: &crate::ai::Profile, exe: &Path, settings: &Path, usage: &str, model: Option<&str>, who: Who) -> Result<Vec<String>> {
    session_as(p, exe, settings, usage, model, who, Place::Data)
}

/// `session` in a plugin's folder (`place`, `claudejob::in_folder`); one that only reads (`Place::Read`) with
/// `read_only_args` whatever the setting.
pub fn session_as(p: &crate::ai::Profile, exe: &Path, settings: &Path, usage: &str, model: Option<&str>, who: Who, place: Place) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![];
    if let Some(m) = model {
        args.extend(["--model".into(), m.to_string()]);
    }
    args.extend(build(p.kind.managed(), exe, settings, who, place)?);
    // The user's choice (Settings → AI & minutes) as this profile and model can run it.
    args.extend(if place == Place::Read { read_only_args().0 } else { permission_args(who.permissions(), p, model, true).0 });
    // The Brain beside the user's own MCP servers (read-only; its tools allowed without asking).
    #[cfg(feature = "tray")]
    if let Some(f) = crate::terminal::brain_mcp_config(exe, &["--profile".into(), p.id.clone(), "--who".into(), usage.into()]) {
        args.extend(["--mcp-config".to_string(), f, "--allowedTools".into(), "mcp__echo".into()]);
    }
    #[cfg(not(feature = "tray"))]
    let _ = usage;
    // What plugins offer (`ctx.expose`), when one does: its tools allowed without asking; a reading run calls only
    // commands marked reads.
    #[cfg(feature = "js")]
    if let Some(f) = crate::jshost::mcp::config(exe, place == Place::Read) {
        args.extend(["--mcp-config".to_string(), f, "--allowedTools".into(), format!("mcp__{}", crate::jshost::mcp::SERVER)]);
    }
    Ok(args)
}

/// A run that changes nothing in its folder, whatever the user's setting: Claude Code asks nothing and runs only what
/// is allowed — reading inside its folders, Echo's reading commands (`build`, `Place::Read`) and the web; any other
/// command or an edit is refused. With its words for the run's record.
pub fn read_only_args() -> (Vec<String>, &'static str) {
    (
        ["--permission-mode", "dontAsk", "--allowedTools", "WebFetch", "--allowedTools", "WebSearch"].map(String::from).to_vec(),
        "nothing asked: only reading, Echo's reading commands and the web run",
    )
}

/// How the folder session handles permission prompts (Settings → AI & minutes; unreadable settings: ask).
pub fn folder_permissions() -> Permissions {
    crate::config::Config::load().map_or(Permissions::Ask, |c| c.folder_permissions())
}

/// What Claude Code starts with for permission setting `p` on `profile` running `model` (None: Claude Code's own
/// default), and its words for a run's record and log: the one mapping the folder session, routines, Yolo and plugins'
/// folder runs that change files (theirs, else the routines' setting) all use. Auto is Claude Code's auto mode only where it works
/// (`auto_mode_works`); elsewhere Claude Code would start in its manual mode, where a headless run does nothing, so
/// it is `acceptEdits` for a run that may edit (`may_edit`) and `dontAsk` for a read-only one.
pub fn permission_args(p: Permissions, profile: &crate::ai::Profile, model: Option<&str>, may_edit: bool) -> (Vec<String>, &'static str) {
    let mode = |m: &str| vec!["--permission-mode".to_string(), m.to_string()];
    match p {
        Permissions::Ask => (vec![], "asks each time"),
        Permissions::Skip => (vec!["--dangerously-skip-permissions".into()], "every permission check skipped"),
        Permissions::Auto if auto_mode_works(profile, model) => (mode("auto"), "auto mode: Claude Code decides each action"),
        Permissions::Auto if may_edit => (mode("acceptEdits"), "edits accepted: this profile's model has no auto mode"),
        Permissions::Auto => (mode("dontAsk"), "read-only, nothing asked: this profile's model has no auto mode"),
    }
}

/// Claude Code's auto mode works on `profile` running `model`: a Claude account (`claude`) or the user's own Claude
/// Code (`system`) — never a model server (`local`) or a wrapper — and no model pinned below what auto mode needs:
/// Claude's names (`sonnet`, `opus`, `haiku`, `fable`, `opusplan`, `sonnet[1m]`, `opus[1m]`) and Claude Code's default
/// are recent; a full id needs Opus or Sonnet 4.6, Haiku 5.5 or any Fable — on Bedrock or Vertex (`anthropic.` / `@` in
/// the id) Sonnet 5 or Opus 4.7. An id it can't read: no.
pub fn auto_mode_works(profile: &crate::ai::Profile, model: Option<&str>) -> bool {
    if !matches!(profile.kind, crate::ai::Kind::Claude | crate::ai::Kind::System) {
        return false;
    }
    let Some(m) = model.map(|m| m.trim().to_ascii_lowercase()).filter(|m| !m.is_empty() && m != "default") else { return true };
    if ["sonnet", "opus", "haiku", "fable", "opusplan", "sonnet[1m]", "opus[1m]"].contains(&m.as_str()) || m.contains("fable") {
        return true;
    }
    let cloud = m.contains("anthropic.") || m.contains('@');
    let at = |family: &str| m.find(&format!("{family}-")).map(|i| &m[i + family.len() + 1..]);
    // `claude-sonnet-4-6…`, `claude-opus-4-7@…`: the version's first two numbers.
    let version = |rest: &str| -> Option<(u32, u32)> {
        let mut n = rest.split(['-', '@', '.', ':', '[']).map_while(|x| x.parse::<u32>().ok().filter(|v| *v < 100));
        Some((n.next()?, n.next().unwrap_or(0)))
    };
    let need = |family: &str| match (family, cloud) {
        ("opus", false) | ("sonnet", false) => (4, 6),
        ("opus", true) => (4, 7),
        ("sonnet", true) => (5, 0),
        _ => (5, 5),
    };
    ["opus", "sonnet", "haiku"].iter().any(|f| at(f).and_then(version).is_some_and(|v| v >= need(f)))
}

/// What Auto gets on each of the user's AI profiles, by its name, on the model its runs use (Settings and `echoctl
/// config` show it).
pub fn auto_on_profiles() -> Vec<(String, &'static str)> {
    let all = crate::ai::Settings::load().map(|s| s.all()).unwrap_or_default();
    all.iter().map(|p| (p.name.clone(), permission_args(Permissions::Auto, p, p.notes_model().map(|m| p.model(&m)).as_deref(), true).1)).collect()
}

/// The user's setting for a routine's (`yolo` false: also a plugin's folder run that names none, Echo's own jobs as routines) or
/// Yolo's run that doesn't say itself: auto or skip (unreadable settings: auto).
pub fn run_permissions(yolo: bool) -> Permissions {
    crate::config::Config::load().map_or(Permissions::Auto, |c| if yolo { c.yolo_permissions() } else { c.routine_permissions() })
}

/// The environment its hooks read: it is one of Echo's sessions, on AI profile `profile`.
pub fn env(profile: &str) -> Vec<(&'static str, String)> {
    vec![(crate::view::SESSION_VAR, "1".into()), (crate::view::PROFILE_VAR, profile.to_string())]
}

/// Where a run's own settings file goes (`build`): one per run, removed when it ends.
pub fn run_settings() -> std::path::PathBuf {
    paths::config_dir().join("minutes-run").join(format!("job-{}-{}.json", std::process::id(), chrono::Local::now().timestamp_nanos_opt().unwrap_or(0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_plugin_not_laid_out_reaches_the_sessions_once_by_its_session_copy_and_a_laid_out_one_not_at_all() {
        let (_env, root) = paths::testenv::folders("folder-args-unlaid");
        let mk = root.join("market");
        for (id, agent) in [("vaults", json!(null)), ("kit", json!({"prefix": "kit"}))] {
            let d = mk.join("plugins").join(id);
            std::fs::create_dir_all(&d).unwrap();
            let mut m = json!({"id": id, "name": id, "version": "0.1.0", "uses": {"sessions": {"skills": true}}});
            if !agent.is_null() {
                m["agent"] = agent;
            }
            std::fs::write(d.join("echo-plugin.json"), m.to_string()).unwrap();
        }
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "vaults", "source": "./plugins/vaults"}, {"id": "kit", "source": "./plugins/kit"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        for id in ["vaults", "kit"] {
            let sheet = crate::plugins::review(id, Some(&name)).unwrap().uses;
            crate::plugins::install(id, Some(&name), &sheet).unwrap();
            crate::grants::set_session_skills(id, &json!([{"name": "notes", "description": "Notes.", "body": "Read them."}])).unwrap();
        }
        let args = build(false, Path::new("/Apps/Echo.app/echoctl"), &root.join("s.json"), Who::Run(Permissions::Auto), Place::Data).unwrap();
        let dirs: Vec<&String> = args.iter().zip(args.iter().skip(1)).filter(|(a, _)| *a == "--plugin-dir").map(|(_, d)| d).collect();
        let copy = |id: &str| crate::grants::session_dir(id).display().to_string();
        assert_eq!(dirs.iter().filter(|d| ***d == copy("vaults")).count(), 1, "no prefix: its session copy, once: {dirs:?}");
        assert!(!dirs.iter().any(|d| **d == copy("kit")), "laid out in `.plugins`: not again: {dirs:?}");
        assert!(crate::plugins::agent::folder("kit").join(".claude/skills/kit-notes").is_dir());
    }

    #[test]
    fn every_session_gets_the_plugins_scripts_and_echos_commands_by_their_own_shape() {
        let (_env, root) = paths::testenv::folders("folder-args-scripts");
        let d = root.join("market/plugins/kit");
        std::fs::create_dir_all(d.join("skills/staffing/scripts")).unwrap();
        std::fs::write(d.join("skills/staffing/SKILL.md"), "---\nname: staffing\ndescription: Staffing.\n---\n\nRun its scripts.\n").unwrap();
        for f in ["read.sh", "write.sh"] {
            std::fs::write(d.join("skills/staffing/scripts").join(f), "#!/bin/sh\necho x\n").unwrap();
        }
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "kit", "name": "Kit", "version": "0.1.0", "skills": ["skills/staffing"], "agent": {"prefix": "kit"},
            "uses": {"folders": {"what": "notes"}, "scripts": [{"skill": "staffing", "script": "scripts/read.sh", "readonly": true}, {"skill": "staffing", "script": "scripts/write.sh"}]}}).to_string()).unwrap();
        std::fs::write(root.join("market/echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "kit", "source": "./plugins/kit"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(root.join("market").to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        let sheet = crate::plugins::review("kit", Some(&name)).unwrap().uses;
        crate::plugins::install("kit", Some(&name), &sheet).unwrap();
        let exe = Path::new("/Apps/Echo.app/echoctl");
        let has = |who: Who, script: &str| build(false, exe, &root.join("s.json"), who, Place::Data).unwrap().iter().any(|a| a.starts_with("Bash(") && a.contains(&format!("/scripts/{script}:*)")));
        // A script once marked `readonly` is a script like any other.
        for who in [Who::Folder, Who::Run(Permissions::Auto), Who::Run(Permissions::Skip)] {
            assert!(has(who, "read.sh") && has(who, "write.sh"), "{who:?}");
        }
        // Echo's own commands, each by its prefix: no wildcard reaches another ledger command.
        let args = build(false, exe, &root.join("s.json"), Who::Folder, Place::Data).unwrap();
        assert!(args.iter().any(|a| a.ends_with(" project work:*)")) && !args.iter().any(|a| a.starts_with("Bash(") && a.contains("ledger")), "{args:?}");
        // A folder the plugin offers sessions (an Obsidian vault): added to a session in the data folder, never to one
        // in a plugin's folder (that folder itself, or the source behind a copy).
        let vault = root.join("acme-vault");
        std::fs::create_dir_all(&vault).unwrap();
        crate::grants::grant("kit", &vault).unwrap();
        let v = dunce::canonicalize(&vault).unwrap().display().to_string();
        crate::grants::set_session_read("kit", std::slice::from_ref(&v)).unwrap();
        let added = |place| build(false, exe, &root.join("s.json"), Who::Run(Permissions::Auto), place).unwrap().iter().any(|a| *a == v);
        assert!(added(Place::Data) && !added(Place::Edit) && !added(Place::Read));
        // In a plugin's folder that only reads: Echo's reading commands only — no edit of Echo's data, no draft, no
        // plugin's script; one that edits keeps them all.
        let read = build(false, exe, &root.join("s.json"), Who::Run(Permissions::Auto), Place::Read).unwrap();
        assert!(read.iter().any(|a| a.ends_with(" project show:*)")) && read.iter().any(|a| a.ends_with(" context:*)")), "{read:?}");
        assert!(READ_COMMANDS.iter().all(|c| FREE_COMMANDS.contains(c)));
        assert!(!read.iter().any(|a| a.ends_with(" project work:*)") || a.ends_with(" compose:*)") || a.ends_with(" prep:*)") || a.ends_with(" project people:*)") || a.contains("/scripts/read.sh") || a.contains("/scripts/write.sh")), "{read:?}");
        assert!(has(Who::Run(Permissions::Auto), "write.sh"));
        let edit = build(false, exe, &root.join("s.json"), Who::Run(Permissions::Auto), Place::Edit).unwrap();
        assert!(edit.iter().any(|a| a.ends_with(" compose:*)")) && edit.iter().any(|a| a.contains("/scripts/write.sh:*)")), "{edit:?}");
    }

    #[test]
    fn each_session_starts_with_its_own_permission_setting_auto_by_default() {
        let (_env, root) = paths::testenv::folders("folder-args-permissions");
        let exe = Path::new("/Apps/Echo.app/echoctl");
        let claude = profile("claude");
        let mode = |who: Who| {
            let a = session(&claude, exe, &root.join("s.json"), "test", None, who).unwrap();
            let skip = a.iter().any(|x| x == "--dangerously-skip-permissions");
            let m = a.windows(2).find(|w| w[0] == "--permission-mode").map(|w| w[1].clone());
            (skip, m)
        };
        let set = |f: &dyn Fn(&mut crate::config::Config)| {
            let mut c = crate::config::Config::load().unwrap();
            f(&mut c);
            c.save().unwrap();
        };
        let auto = (false, Some("auto".to_string()));
        let skip = (true, None);
        // Nothing set: the folder session in auto mode; routines and Yolo act on their own, in auto mode too.
        assert_eq!(mode(Who::Folder), auto);
        assert_eq!((run_permissions(false), run_permissions(true)), (Permissions::Auto, Permissions::Auto));
        assert_eq!(mode(Who::Run(run_permissions(false))), auto);
        // Each its own: the routines' skip changes neither Yolo nor the folder session.
        set(&|c| c.routine_permissions = Some(Permissions::Skip));
        assert_eq!((run_permissions(false), run_permissions(true)), (Permissions::Skip, Permissions::Auto));
        assert_eq!((mode(Who::Run(run_permissions(false))), mode(Who::Run(run_permissions(true))), mode(Who::Folder)), (skip.clone(), auto.clone(), auto.clone()));
        set(&|c| {
            c.routine_permissions = None;
            c.yolo_permissions = Some(Permissions::Skip);
        });
        assert_eq!((mode(Who::Run(run_permissions(false))), mode(Who::Run(run_permissions(true)))), (auto.clone(), skip.clone()));
        set(&|c| c.folder_permissions = Some(Permissions::Skip));
        assert_eq!(mode(Who::Folder), skip, "the folder session's own choice");
        // Ask each time: only the folder session gets Claude Code's prompts; routines and Yolo keep their own.
        set(&|c| c.folder_permissions = Some(Permissions::Ask));
        assert_eq!(mode(Who::Folder), (false, None));
        assert_eq!((run_permissions(false), run_permissions(true)), (Permissions::Auto, Permissions::Skip));
    }

    fn profile(kind: &str) -> crate::ai::Profile {
        serde_json::from_value(json!({"id": kind, "name": kind, "kind": kind})).unwrap()
    }

    #[test]
    fn auto_is_claude_codes_auto_mode_only_where_the_profiles_model_has_it_else_edits_accepted_or_read_only() {
        let flags = |kind: &str, model: Option<&str>, may_edit: bool| permission_args(Permissions::Auto, &profile(kind), model, may_edit).0.join(" ");
        // A Claude account or the user's own Claude Code, on Claude's names or Claude Code's default: auto mode.
        for kind in ["claude", "system"] {
            for model in [None, Some("sonnet"), Some("opus"), Some("haiku"), Some("opus[1m]"), Some("sonnet[1m]"), Some("opusplan"), Some("default"), Some("claude-sonnet-4-6"), Some("claude-opus-4-7[1m]"), Some("claude-haiku-5-5"), Some("claude-fable-1")] {
                assert_eq!(flags(kind, model, true), "--permission-mode auto", "{kind} {model:?}");
            }
            // A model pinned below what auto mode needs, or one it can't read.
            for model in ["claude-sonnet-4-5", "claude-opus-4-1", "claude-haiku-4-5", "claude-3-5-sonnet-20241022", "acme-gw/large", "us.anthropic.claude-sonnet-4-6-v1:0", "claude-opus-4-6@20260101"] {
                assert_eq!(flags(kind, Some(model), true), "--permission-mode acceptEdits", "{kind} {model}");
            }
            assert_eq!(flags(kind, Some("us.anthropic.claude-sonnet-5-0-v1:0"), true), "--permission-mode auto", "Bedrock: Sonnet 5");
            assert_eq!(flags(kind, Some("claude-opus-4-7@20260101"), true), "--permission-mode auto", "Vertex: Opus 4.7");
        }
        // A model server or a wrapper: never auto mode — edits accepted, or nothing asked for a read-only run.
        for kind in ["local", "wrapper"] {
            assert_eq!(flags(kind, Some("sonnet"), true), "--permission-mode acceptEdits", "{kind}");
            assert_eq!(flags(kind, None, false), "--permission-mode dontAsk", "{kind}");
        }
        // Skip and Ask are the same on every profile; the words name what it got.
        for kind in ["claude", "system", "local", "wrapper"] {
            assert_eq!(permission_args(Permissions::Skip, &profile(kind), None, false), (vec!["--dangerously-skip-permissions".to_string()], "every permission check skipped"));
            assert_eq!(permission_args(Permissions::Ask, &profile(kind), None, true), (vec![], "asks each time"));
        }
        assert_eq!(permission_args(Permissions::Auto, &profile("local"), None, true).1, "edits accepted: this profile's model has no auto mode");
    }
}

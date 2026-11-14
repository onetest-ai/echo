//! What every Claude Code session Echo starts in its data folder gets — the folder session (`terminal::start_folder`),
//! a plugin's run (a routine, `ctx.claude.run`) and a Yolo task (`claudejob::run`) alike (#590): the same knowledge
//! (the core's SessionStart hook, what plugins lay out in `.plugins/`, Echo's own plugin, the user's skills, the
//! window's view hook) and the same permissions (the plugins' commands and scripts allowed, the folders plugins were
//! given, the OneDrive and `.plugins` write checks, and the user's choice to skip permission prompts).

// Without the window (`tray`) or plugin code (`js`) nobody starts one.
#![cfg_attr(not(any(feature = "tray", feature = "js")), allow(dead_code))]

use std::path::Path;

use anyhow::Result;

use crate::paths;

/// Echo commands any of its sessions runs without a prompt (`Bash(<cli> <command> …)`): what reads Echo's data, and
/// edits of Echo's own data — a thread's work, a mail left as a draft. Never a send, an install or a setting.
pub const FREE_COMMANDS: [&str; 18] = [
    "context", "attention", "prep", "previous", "status", "calendar events", "read", "project list", "project show", "project of",
    "project tasks", "project people", "project ledger * tree", "project ledger * status", "client list", "org list",
    // Echo's own data.
    "project ledger * edit *", "compose",
];

/// The arguments, its settings file first (`--settings <settings>`, written here): `managed` — a profile of its own,
/// given Echo's plugin (a profile on the user's own Claude Code has it installed already); `unattended` — a routine's
/// or Yolo's run, which gets the window's view hook only at its start (who the user is), not what the window shows.
pub fn build(managed: bool, exe: &Path, settings: &Path, unattended: bool) -> Result<Vec<String>> {
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
    let commands = crate::plugins::session_commands();
    for cli in &clis {
        for (id, c) in &commands {
            args.extend(["--allowedTools".into(), format!("Bash({cli} plugin call {id} {c}:*)")]);
        }
    }
    // Echo's reads and its own data's edits, by Echo's own CLI (its plugin's launcher, `echoctl` on PATH).
    for cli in crate::notes::plugin_dir().map(|d| crate::notes::run_clis(&d)).unwrap_or_else(|| vec!["echoctl".into()]) {
        for c in FREE_COMMANDS {
            // A pattern with `*` as it is (Claude Code's wildcards); a plain command and whatever follows it.
            args.extend(["--allowedTools".into(), if c.contains('*') { format!("Bash({cli} {c})") } else { format!("Bash({cli} {c}:*)") }]);
        }
    }
    // Their declared scripts, by their launchers' full paths (`uses.scripts`): no prompt for them either.
    for s in crate::plugins::agent::scripts() {
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
    for d in crate::grants::session_read() {
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
    // The user's choice (Settings → AI & minutes): no permission prompts. Without it a headless run simply can't do
    // what would ask.
    if skip_permissions() {
        args.push("--dangerously-skip-permissions".into());
    }
    Ok(args)
}

/// What a session in Echo's data folder runs with, after its own conversation's arguments: `model`, `build` (its
/// settings file `settings`) and the Brain (`who`: whose usage it counts). The folder session's
/// (`terminal::start_folder`), and — but for `-p`, its prompt and its model — a routine's and a Yolo task's
/// (`claudejob::run`, `unattended`).
pub fn session(p: &crate::ai::Profile, exe: &Path, settings: &Path, who: &str, model: Option<&str>, unattended: bool) -> Result<Vec<String>> {
    let mut args: Vec<String> = vec![];
    if let Some(m) = model {
        args.extend(["--model".into(), m.to_string()]);
    }
    args.extend(build(p.kind.managed(), exe, settings, unattended)?);
    // The Brain beside the user's own MCP servers (read-only; its tools allowed without asking).
    #[cfg(feature = "tray")]
    if let Some(f) = crate::terminal::brain_mcp_config(exe, &["--profile".into(), p.id.clone(), "--who".into(), who.into()]) {
        args.extend(["--mcp-config".to_string(), f, "--allowedTools".into(), "mcp__echo".into()]);
    }
    #[cfg(not(feature = "tray"))]
    let _ = who;
    Ok(args)
}

/// The user's choice to skip permission prompts in Echo's sessions (Settings → AI & minutes).
pub fn skip_permissions() -> bool {
    crate::config::Config::load().is_ok_and(|c| c.folder_skip_permissions)
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
        let args = build(false, Path::new("/Apps/Echo.app/echoctl"), &root.join("s.json"), true).unwrap();
        let dirs: Vec<&String> = args.iter().zip(args.iter().skip(1)).filter(|(a, _)| *a == "--plugin-dir").map(|(_, d)| d).collect();
        let copy = |id: &str| crate::grants::session_dir(id).display().to_string();
        assert_eq!(dirs.iter().filter(|d| ***d == copy("vaults")).count(), 1, "no prefix: its session copy, once: {dirs:?}");
        assert!(!dirs.iter().any(|d| **d == copy("kit")), "laid out in `.plugins`: not again: {dirs:?}");
        assert!(crate::plugins::agent::folder("kit").join(".claude/skills/kit-notes").is_dir());
    }
}

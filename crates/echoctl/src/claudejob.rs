//! A Claude Code run a plugin asks for (`ctx.claude.run`, `docs/plugins-design.md` v4), or Yolo's: headless, on one of
//! the user's AI profiles (strict routing, the profile's lease), with the Brain narrowed to that profile.
//!
//! A routine's or Yolo's run (`Spec::session`) runs as the folder session does (#590): in the data folder, with its
//! arguments, settings and hooks (`folder_args::build`) and the user's choice to skip permission prompts; the `tools`
//! it asked for is kept in its record (`permissions.json`), not applied. Otherwise (Won't do's no-tools run) the
//! permissions are the run's own:
//! - `read` — the guard (`guard.rs`, `RUN_MODE=routine`): read the data folder (and the run's own saved
//!   tool output), run read-only `echoctl` commands (`guard::ROUTINE_COMMANDS`), ask the Brain; no edits;
//! - `folder` — the user's own Claude Code settings, `dontAsk` (a headless run can't ask);
//! - `skip` — `--dangerously-skip-permissions`.
//!
//! The plugin writes the prompt; Echo adds what only it knows — the time, and how to reach its data in the
//! chosen mode. Each run leaves `prompt.md`, `log.txt` and `answer.md` in a folder of the plugin's store.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use serde_json::Value;

use crate::{guard, notes, paths};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tools {
    /// No tools at all: the prompt holds everything (a plugin's analysis of what it read).
    None,
    Read,
    Folder,
    Skip,
}

impl Tools {
    pub fn parse(s: &str) -> Result<Tools> {
        match s.trim() {
            "" | "read" => Ok(Tools::Read),
            "folder" => Ok(Tools::Folder),
            "skip" => Ok(Tools::Skip),
            "none" => Ok(Tools::None),
            o => bail!("tools “{o}”: none, read, folder or skip"),
        }
    }

    /// As a plugin names it (`tools`).
    pub fn name(self) -> &'static str {
        match self {
            Tools::None => "none",
            Tools::Read => "read",
            Tools::Folder => "folder",
            Tools::Skip => "skip",
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Tools::None => "no tools",
            Tools::Read => "read",
            Tools::Folder => "folder",
            Tools::Skip => "skip",
        }
    }
}

/// What to run.
#[derive(Debug, Clone)]
pub struct Spec {
    /// What the run is called in its log ("Monday asks").
    pub title: String,
    pub prompt: String,
    /// An AI profile id ("" = the default).
    pub profile: String,
    /// "" = the profile's minutes model.
    pub model: String,
    pub tools: Tools,
    /// Where the run's files go.
    pub run_dir: PathBuf,
    pub timeout: Duration,
    /// A routine's or Yolo's run: as the folder session (`folder_args`), whatever `tools` asked.
    pub session: bool,
    /// Yolo's task (`<project>/<node>`): the run may edit that one task (`guard::RUN_TASK`; the CLI's other headless
    /// limits stay), and a profile of its own gets Echo's plugin (manage, Microsoft 365), as the panel does.
    pub task: Option<String>,
    /// Yolo's thread (a task's, or an item of Needs you now's): the run reads its data — brief, people, work, calls
    /// pinned to the run's profile (`guard::RUN_THREAD`).
    pub thread: Option<String>,
    /// Echo's plugin (manage, Microsoft 365) for a profile of its own, as the panel has it: Yolo's runs, a task's or an
    /// item of Needs you now's (with or without a thread).
    pub echo_plugin: bool,
}

impl Spec {
    /// From a plugin's `{prompt, title, profile, model, tools, dir, timeout}`; `dir` is relative to its store.
    pub fn from_value(v: &Value, store: &Path) -> Result<Spec> {
        let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
        let prompt = v["prompt"].as_str().unwrap_or("").to_string();
        if prompt.trim().is_empty() {
            bail!("a Claude run needs a prompt");
        }
        let dir = match s("dir") {
            d if d.is_empty() => format!("runs/{}", Local::now().format("%Y%m%d-%H%M%S")),
            d => d,
        };
        let rel = Path::new(&dir);
        if rel.is_absolute() || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            bail!("“{dir}” isn't a folder inside the plugin's store");
        }
        let minutes = v["timeout"].as_u64().unwrap_or(20).clamp(1, 60);
        Ok(Spec {
            title: Some(s("title")).filter(|t| !t.is_empty()).unwrap_or_else(|| "a plugin's run".into()),
            prompt,
            profile: s("profile"),
            model: s("model"),
            tools: Tools::parse(&s("tools"))?,
            run_dir: store.join(rel),
            timeout: Duration::from_secs(minutes * 60),
            session: true,
            task: None,
            thread: None,
            echo_plugin: false,
        })
    }
}

fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// What Echo tells the run around the plugin's prompt.
fn preamble(tools: Tools, cli: &str) -> String {
    let reach = match tools {
        Tools::Read => format!(
            "You may read Echo's data folder (its calls' transcripts, minutes and action items, threads, people), search what was said \
             and who people are (the `echo` MCP tools), and run these read-only Echo commands as {cli}, one per Bash call, plain words \
             only: {}. You change nothing.",
            guard::ROUTINE_COMMANDS.iter().map(|c| format!("`echoctl {}`", c.replace('*', "<id>"))).collect::<Vec<_>>().join(", ")
        ),
        Tools::None => "You have no tools: everything you need is below.".to_string(),
        _ => format!("Echo's CLI is {cli}; what was said and who people are: the `echo` MCP tools."),
    };
    format!(
        "Started by Echo; nobody is watching: don't ask anything, do the work and answer. Now: {}. The working folder is Echo's data \
         folder. {reach}",
        Local::now().format("%A %Y-%m-%d %H:%M (UTC%:z, the user's local time)")
    )
}

/// What a run cost: tokens (input, cache read / write, output), dollars as Claude Code counts them, time.
pub fn usage(result: &Value) -> Value {
    let u = &result["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    serde_json::json!({
        "input_tokens": n("input_tokens"), "cache_read_tokens": n("cache_read_input_tokens"),
        "cache_write_tokens": n("cache_creation_input_tokens"), "output_tokens": n("output_tokens"),
        "cost_usd": result["total_cost_usd"].as_f64().unwrap_or(0.0),
        "duration_ms": result["duration_ms"].as_u64().unwrap_or(0), "turns": result["num_turns"].as_u64().unwrap_or(0),
    })
}

/// Run it: the answer, the model that wrote it (as named to Claude Code; "" = its own default), and
/// what it cost (`usage`).
pub fn run(spec: &Spec) -> Result<(String, String, Value)> {
    fs::create_dir_all(&spec.run_dir)?;
    let mut log = fs::File::create(spec.run_dir.join("log.txt"))?;
    let profile_id = if spec.profile.is_empty() { crate::ai::default_id() } else { spec.profile.clone() };
    let ready = crate::ai::ready(&profile_id)?;
    let profile = ready.profile();
    let model = Some(spec.model.clone()).filter(|m| !m.is_empty()).or_else(|| profile.notes_model());
    let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    let home = dunce::canonicalize(paths::home())?;
    // A session-like run starts in the data folder, as the folder session does: `.plugins/<id>/` loads from there.
    let cwd = if spec.session || profile.kind != crate::ai::Kind::System { home.clone() } else { crate::ai::system_folder()? };
    let clis = notes::run_clis(&plugin);
    // The launcher by its full path, told which Echo to run (`ECHOCTL_BIN`, below): this one — `echoctl` on
    // PATH may be another version.
    let launcher = clis.iter().find(|c| c.contains('/') && c.contains("scripts")).cloned().unwrap_or_else(|| "echoctl".into());
    // What the Brain holds and when to go there (#34): only when it is on and the run has tools to ask it.
    #[cfg(feature = "brain")]
    // Only where the run gets the echo server (the window's builds; any tool mode but none).
    let brain_text = (cfg!(feature = "tray") && (spec.session || spec.tools != Tools::None)).then(|| crate::brain::advert::block(&crate::brain::access::Access { profile: profile.id.clone(), ..Default::default() })).flatten();
    #[cfg(not(feature = "brain"))]
    let brain_text: Option<String> = None;
    let prompt = format!(
        "{}{}\n\n{}",
        preamble(if spec.session { Tools::Folder } else { spec.tools }, &format!("`{launcher}` (its full path, exactly so)")),
        brain_text.map(|b| format!("\n\n<echo_recall>\n{b}\n</echo_recall>")).unwrap_or_default(),
        spec.prompt
    );
    let _ = fs::write(spec.run_dir.join("prompt.md"), &prompt);
    // What it asked for, and what it runs with (a routine or Yolo: the folder session's permissions).
    let used = if !spec.session {
        spec.tools.word().to_string()
    } else if crate::folder_args::skip_permissions() {
        "the folder session's: every permission prompt skipped".to_string()
    } else {
        "the folder session's: no prompts, so what would ask is refused".to_string()
    };
    let _ = fs::write(spec.run_dir.join("permissions.json"), serde_json::json!({"asked": spec.tools.name(), "used": used}).to_string());
    let _ = writeln!(log, "{}  Echo runs “{}” on {}{} (asked: {} permissions; runs with {used})", Local::now().format("%H:%M:%S"), spec.title, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default(), spec.tools.word());
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    let mut settings_file = None;
    // A routine or Yolo: the folder session's arguments, but for `-p` and its prompt.
    #[cfg(feature = "tray")]
    if spec.session {
        let f = crate::folder_args::run_settings();
        let a = session_run_args(&prompt, profile, &exe, &f);
        return run_child(spec, profile, &cwd, &exe, a, Some(f), crate::folder_args::env(&profile.id), log, model);
    }
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    if let Some(m) = &model {
        a.extend(["--model".into(), m.clone()]);
    }
    // The Brain, narrowed to this profile's sources (strict routing).
    #[cfg(all(feature = "tray", feature = "brain"))]
    let brain = crate::terminal::brain_mcp_config(&exe, &["--profile".into(), profile.id.clone(), "--who".into(), "routine".into()]);
    #[cfg(not(all(feature = "tray", feature = "brain")))]
    let brain: Option<String> = None;
    let env = legacy_args(spec, profile, &exe, &home, &cwd, brain.as_deref(), &clis, &mut a, &mut settings_file)?;
    run_child(spec, profile, &cwd, &exe, Ok(a), settings_file, env, log, model)
}

/// A routine's or Yolo's arguments: `-p` and its prompt, then the folder session's (`terminal::folder_session_args`),
/// its settings file `settings`.
#[cfg(feature = "tray")]
fn session_run_args(prompt: &str, profile: &crate::ai::Profile, exe: &Path, settings: &Path) -> Result<Vec<String>> {
    let mut a: Vec<String> = vec!["-p".into(), prompt.to_string(), "--output-format".into(), "stream-json".into(), "--verbose".into()];
    a.extend(crate::terminal::folder_session_args(profile, exe, settings, "routine")?);
    Ok(a)
}

/// Start the run with arguments `a` and environment `env`, wait for it, keep its answer.
#[allow(clippy::too_many_arguments)]
fn run_child(spec: &Spec, profile: &crate::ai::Profile, cwd: &Path, exe: &Path, a: Result<Vec<String>>, settings_file: Option<PathBuf>, env: Vec<(&'static str, String)>, mut log: fs::File, model: Option<String>) -> Result<(String, String, Value)> {
    let a = match a {
        Ok(a) => a,
        Err(e) => {
            if let Some(f) = &settings_file {
                let _ = fs::remove_file(f);
            }
            return Err(e);
        }
    };
    let child = crate::ai::guard_ready(profile, cwd, "run a plugin's job").and_then(|()| notes::admit_child(|| {
        let mut c = crate::ai::prepare_claude(profile, &a, cwd, false)?;
        c.env("ECHOCTL_BIN", exe);
        // A mail the run writes (`echoctl compose`) is noted here, for its outcome (`drafts::of_run`).
        c.env(crate::drafts::RUN_DRAFTS, spec.run_dir.join("drafts.jsonl"));
        if let Some(t) = &spec.task {
            c.env(guard::RUN_TASK, t).env(guard::RUN_PROFILE, &profile.id);
        }
        if let Some(t) = &spec.thread {
            c.env(guard::RUN_THREAD, t).env(guard::RUN_PROFILE, &profile.id);
        }
        for (k, v) in &env {
            c.env(k, v);
        }
        Ok(c.spawn()?)
    }));
    let out = child.and_then(|c| crate::projectsync::finish_run_result(c, &mut log, spec.timeout, "run a plugin's job", "Echo quit during the run"));
    if let Some(f) = settings_file {
        let _ = fs::remove_file(f);
    }
    let (answer, result) = out?;
    let _ = fs::write(spec.run_dir.join("answer.md"), &answer);
    Ok((answer, model.unwrap_or_default(), usage(&result)))
}

/// A run with its own permissions (`tools`), not the folder session's: its arguments into `a`, its settings file, and
/// the environment its guard reads. Kept for runs that aren't a routine's or Yolo's (Won't do: no tools at all).
#[allow(clippy::too_many_arguments)]
fn legacy_args(spec: &Spec, profile: &crate::ai::Profile, exe: &Path, home: &Path, cwd: &Path, brain: Option<&str>, clis: &[String], a: &mut Vec<String>, settings_file: &mut Option<PathBuf>) -> Result<Vec<(&'static str, String)>> {
    let mut env: Vec<(&'static str, String)> = vec![];
    let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    // What plugins add to Echo's sessions: their skills, and folders to read (also for a read-only run).
    let (session_plugins, session_read) = (crate::grants::session_plugins(), crate::grants::session_read());
    // The commands plugins let Echo's sessions run, by the CLI their skills name too.
    let session_commands = crate::plugins::session_commands();
    // The plugin scripts marked readonly, by their launchers' paths: what a read-only routine may run of them.
    let session_scripts = if spec.tools == Tools::Read { crate::grants::session_scripts(true) } else { vec![] };
    let mut clis = clis.to_vec();
    clis.extend(crate::grants::session_clis(&session_plugins));
    if spec.tools != Tools::None {
        for d in &session_plugins {
            a.extend(["--plugin-dir".into(), d.display().to_string()]);
        }
        if spec.echo_plugin && profile.kind.managed() {
            a.extend(["--plugin-dir".into(), plugin.display().to_string()]);
        }
    }
    match spec.tools {
        Tools::None => {
            a.extend(["--tools".into(), String::new(), "--strict-mcp-config".into(), "--no-session-persistence".into()]);
        }
        Tools::Read => {
            crate::ai::hooks_conflict(profile, cwd).map_err(|w| anyhow!(w))?;
            let folder = paths::config_dir().join("minutes-run");
            fs::create_dir_all(&folder)?;
            let f = folder.join(format!("job-{}-{}.json", std::process::id(), Local::now().timestamp_millis()));
            fs::write(&f, serde_json::to_vec_pretty(&guard::settings(exe, &fwd(&paths::calls_dir())))?)?;
            let tools = if session_plugins.is_empty() { "Bash,Read,Glob,Grep" } else { "Bash,Read,Glob,Grep,Skill" };
            a.extend(["--settings".into(), f.display().to_string(), "--tools".into(), tools.into(), "--strict-mcp-config".into()]);
            if let Some(b) = brain {
                a.extend(["--mcp-config".into(), b.to_string()]);
            }
            a.extend(["--permission-mode".into(), "dontAsk".into(), "--allowedTools".into()]);
            let abs = format!("/{}", notes::rule_path(home));
            a.extend(["Read", "Glob", "Grep"].map(|t| format!("{t}({abs}/**)")));
            for d in &session_read {
                let abs = format!("/{}", notes::rule_path(d));
                a.extend(["Read", "Glob", "Grep"].map(|t| format!("{t}({abs}/**)")));
                a.extend(["--add-dir".into(), d.display().to_string()]);
            }
            if !session_plugins.is_empty() {
                a.push("Skill".into());
            }
            a.extend(clis.iter().flat_map(|cli| guard::ROUTINE_COMMANDS.iter().map(move |c| format!("Bash({cli} {}:*)", c.split(" *").next().unwrap_or(c)))));
            a.extend(clis.iter().flat_map(|cli| session_commands.iter().map(move |(id, c)| format!("Bash({cli} plugin call {id} {c}:*)"))));
            a.extend(session_scripts.iter().map(|s| format!("Bash({s}:*)")));
            a.push("mcp__echo".into());
            *settings_file = Some(f);
        }
        Tools::Folder | Tools::Skip => {
            // Unguarded, yet never a write where Echo lays out what plugins tell Claude (`.plugins/`): a hook runs even
            // with every permission check skipped.
            let folder = paths::config_dir().join("minutes-run");
            fs::create_dir_all(&folder)?;
            let f = folder.join(format!("job-{}-{}.json", std::process::id(), Local::now().timestamp_millis()));
            fs::write(&f, serde_json::to_vec_pretty(&unguarded_settings(exe))?)?;
            a.extend(["--settings".into(), f.display().to_string()]);
            *settings_file = Some(f);
            if let Some(b) = brain {
                a.extend(["--mcp-config".into(), b.to_string(), "--allowedTools".into(), "mcp__echo".into()]);
            }
            if spec.tools == Tools::Skip {
                a.push("--dangerously-skip-permissions".into());
            } else {
                a.extend(["--permission-mode".into(), "dontAsk".into()]);
            }
        }
    }
    if spec.tools == Tools::Read {
        env.extend([
            (guard::RUN_MODE, "routine".to_string()),
            (guard::RUN_HOME, home.display().to_string()),
            (guard::RUN_CLIS, clis.join("\n")),
            (guard::RUN_READ, session_read.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join("\n")),
            // The skills it may load: those of the plugins it was given, and those laid out in `.plugins/`.
            (guard::RUN_SKILLS, crate::grants::skill_names(&session_plugins).into_iter().chain(crate::plugins::agent::skill_names()).collect::<Vec<_>>().join("\n")),
            (guard::RUN_PLUGIN_COMMANDS, session_commands.iter().map(|(id, c)| format!("{id} {c}")).collect::<Vec<_>>().join("\n")),
            (guard::RUN_PLUGIN_SCRIPTS, session_scripts.join("\n")),
        ]);
    }
    Ok(env)
}

/// The settings of a run without Echo's guard (`folder`, `skip`): only the hook that keeps writes out of `.plugins/`.
fn unguarded_settings(exe: &Path) -> serde_json::Value {
    serde_json::json!({"hooks": {"PreToolUse": [crate::plugins::agent::write_hook_entry(exe)]}})
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--mcp-config <file>`'s file named by who asked (its usage counts): the same server either way.
    fn same_brain(args: &[String]) -> Vec<String> {
        let mut out = args.to_vec();
        for i in 1..out.len() {
            if out[i - 1] == "--mcp-config" {
                out[i] = "<the Brain>".into();
            }
        }
        out
    }

    #[test]
    #[cfg(all(unix, feature = "tray"))]
    fn a_routine_and_a_yolo_task_run_as_the_folder_session_does() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-session");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        let home = dunce::canonicalize(paths::home()).unwrap();
        let file = |n: &str| fs::read_to_string(crate::ai::fake::file(&p, n)).unwrap_or_default();
        // A routine asking for read-only permissions: the folder session's, in the data folder, its ask kept.
        let spec = Spec::from_value(&serde_json::json!({"prompt": "List what changed", "tools": "read", "profile": "work", "dir": "runs/r1"}), &root.join("store")).unwrap();
        assert!(spec.session);
        let _ = run(&spec);
        assert_eq!(file("p.cwd").trim(), home.display().to_string());
        let args: Vec<String> = file("p.args").lines().map(str::to_owned).collect();
        assert!(args.contains(&"--settings".to_string()) && !args.contains(&"--tools".to_string()) && !args.contains(&"--dangerously-skip-permissions".to_string()), "{args:?}");
        let env = file("p.env");
        assert!(env.contains("ECHOCTL_TERMINAL=1") && env.contains("ECHOCTL_SESSION_PROFILE=work") && !env.contains("ECHOCTL_RUN_MODE="), "no read-only guard");
        let settings: Value = serde_json::from_str(&file("p.settings")).unwrap();
        let commands = |k: &str| settings["hooks"][k].as_array().unwrap().iter().flat_map(|h| h["hooks"].as_array().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert!(commands("SessionStart").iter().any(|c| c.ends_with(" core --hook")) && commands("SessionStart").iter().any(|c| c.ends_with(" view-hook")));
        assert!(commands("PreToolUse").iter().any(|c| c.ends_with(" onedrive-hook")) && commands("PreToolUse").iter().any(|c| c.ends_with(" plugins-hook")));
        assert!(commands("UserPromptSubmit").iter().any(|c| c.ends_with(" view-hook")));
        let record: Value = serde_json::from_str(&fs::read_to_string(root.join("store/runs/r1/permissions.json")).unwrap()).unwrap();
        assert_eq!(record["asked"], "read");
        assert!(record["used"].as_str().unwrap().starts_with("the folder session's"));
        // Its arguments are the folder session's, but for `-p` and its prompt.
        let exe = std::env::current_exe().unwrap();
        let s = root.join("settings.json");
        let routine = session_run_args("List what changed", &p, &exe, &s).unwrap();
        let folder = crate::terminal::folder_session_args(&p, &exe, &s, "folder").unwrap();
        assert_eq!(routine[..5], ["-p", "List what changed", "--output-format", "stream-json", "--verbose"]);
        assert_eq!(same_brain(&routine[5..]), same_brain(&folder));
        // With the user's choice to skip permission prompts on, a Yolo task skips them; off, it doesn't, whatever it asked.
        let mut c = crate::config::Config::load().unwrap_or_default();
        c.folder_skip_permissions = true;
        c.save().unwrap();
        let yolo = Spec { title: "Yolo: x".into(), prompt: "Do it".into(), profile: "work".into(), model: String::new(), tools: Tools::Skip, run_dir: root.join("yolo/1"), timeout: Duration::from_secs(60), session: true, task: None, thread: None, echo_plugin: true };
        let _ = run(&yolo);
        assert!(file("p.args").lines().any(|l| l == "--dangerously-skip-permissions"));
        c.folder_skip_permissions = false;
        c.save().unwrap();
        let _ = run(&yolo);
        assert!(!file("p.args").lines().any(|l| l == "--dangerously-skip-permissions"));
        assert_eq!(file("p.cwd").trim(), home.display().to_string());
    }

    #[test]
    #[cfg(unix)]
    fn a_run_without_the_guard_still_writes_nothing_where_plugins_tell_claude() {
        let v = unguarded_settings(Path::new("/Apps/Echo.app/echoctl"));
        let h = &v["hooks"]["PreToolUse"][0];
        assert_eq!(h["matcher"], "Write|Edit|MultiEdit|NotebookEdit|Bash");
        assert_eq!(h["hooks"][0]["command"], "'/Apps/Echo.app/echoctl' plugins-hook");
    }

    #[test]
    fn a_spec_stays_in_the_plugins_store() {
        let store = Path::new("/tmp/store");
        let v = serde_json::json!({"prompt": "hi", "tools": "folder", "dir": "runs/x", "timeout": 500});
        let s = Spec::from_value(&v, store).unwrap();
        assert_eq!(s.tools, Tools::Folder);
        assert_eq!(s.run_dir, store.join("runs/x"));
        assert_eq!(s.timeout, Duration::from_secs(3600));
        assert!(Spec::from_value(&serde_json::json!({"prompt": "hi", "dir": "../x"}), store).is_err());
        assert!(Spec::from_value(&serde_json::json!({"prompt": " "}), store).is_err());
        assert!(Spec::from_value(&serde_json::json!({"prompt": "x", "tools": "root"}), store).is_err());
    }
}

//! A Claude Code run a plugin asks for (`ctx.claude.run`, `docs/plugins-design.md` v4), or Yolo's: headless, on one of
//! the user's AI profiles (strict routing, the profile's lease), with the Brain narrowed to that profile.
//!
//! A routine's or Yolo's run (`Spec::session`) runs as the folder session does (#590): in the data folder, with its
//! arguments, settings and hooks (`folder_args::session`). It acts on its own: in Claude Code's auto mode (where the
//! profile's model has it) or with every permission check skipped — as the routine itself says (`tools`: `auto` or
//! `skip`), else the user's routines' or Yolo's setting (Settings → AI & minutes, `folder_args::run_permissions`) —
//! with the CLI's limits for runs lifted as in the folder session (`notes::RUN_SESSION=acts`; Yolo keeps its own task
//! and thread), every change and send noted.
//!
//! What it asked for is kept in its record (`permissions.json`), with a start in another permission mode than asked
//! (`finish`); `none` (a plugin's analysis of what it read, Won't
//! do) keeps no tools at all.
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

/// The permissions a run asks for (`tools`): a routine's own choice, or the user's setting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tools {
    /// No tools at all: the prompt holds everything (a plugin's analysis of what it read).
    None,
    /// The user's setting for routines (or Yolo's, for Yolo).
    Setting,
    Auto,
    Skip,
}

impl Tools {
    /// `auto` or `skip` (a routine's choice in the Scheduler), `none`, or nothing (the user's setting). The values of
    /// before, `read` and `folder`, are taken as `auto`.
    pub fn parse(s: &str) -> Result<Tools> {
        match s.trim() {
            "" => Ok(Tools::Setting),
            "auto" | "read" | "folder" => Ok(Tools::Auto),
            "skip" => Ok(Tools::Skip),
            "none" => Ok(Tools::None),
            o => bail!("tools “{o}”: auto, skip or none"),
        }
    }

    /// As its record names it.
    pub fn name(self) -> &'static str {
        match self {
            Tools::None => "none",
            Tools::Setting => "setting",
            Tools::Auto => "auto",
            Tools::Skip => "skip",
        }
    }

    /// The permissions it asked for itself (None: no tools, or the user's setting).
    fn permissions(self) -> Option<crate::config::Permissions> {
        match self {
            Tools::Auto => Some(crate::config::Permissions::Auto),
            Tools::Skip => Some(crate::config::Permissions::Skip),
            Tools::None | Tools::Setting => None,
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
    /// Yolo's run (else a routine's): acting on its own, it takes Yolo's permission setting.
    pub yolo: bool,
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
        let tools = Tools::parse(&s("tools"))?;
        if !s("profile").is_empty() {
            crate::ai::plugin_may_name(&crate::ai::Settings::load()?, &s("profile"), None)?;
        }
        Ok(Spec {
            title: Some(s("title")).filter(|t| !t.is_empty()).unwrap_or_else(|| "a plugin's run".into()),
            prompt,
            profile: s("profile"),
            model: s("model"),
            tools,
            run_dir: store.join(rel),
            timeout: Duration::from_secs(minutes * 60),
            // `tools: "none"` (a plugin's analysis of what it read) keeps no tools; any other runs as the folder session.
            session: tools != Tools::None,
            task: None,
            thread: None,
            yolo: false,
        })
    }
}

/// What Echo tells the run around the plugin's prompt: `tools` — a routine's or Yolo's run as the folder session,
/// acting on its own; else no tools at all.
fn preamble(tools: bool, cli: &str) -> String {
    let reach = if tools {
        format!(
            "Echo's CLI is {cli}; what was said and who people are: the `echo` MCP tools. The user lets you act on your own: \
             you may change Echo's data and send through Echo's CLI, as in their own session. Every change and send you make with Echo's CLI is listed for them."
        )
    } else {
        "You have no tools: everything you need is below.".to_string()
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
/// what it cost (`usage`). The changes and sends it made with Echo's CLI are in `<run dir>/actions.jsonl`
/// (`notes::run_actions`).
pub fn run(spec: &Spec) -> Result<(String, String, Value)> {
    fs::create_dir_all(&spec.run_dir)?;
    let mut log = fs::File::create(spec.run_dir.join("log.txt"))?;
    let profile_id = if spec.profile.is_empty() { crate::ai::default_id() } else { spec.profile.clone() };
    let ready = crate::ai::ready(&profile_id)?;
    let profile = ready.profile();
    let model = Some(spec.model.clone()).filter(|m| !m.is_empty()).or_else(|| profile.notes_model()).map(|m| profile.model(&m));
    let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    let home = dunce::canonicalize(paths::home())?;
    // A session-like run starts in the data folder, as the folder session does: `.plugins/<id>/` loads from there.
    let cwd = if spec.session || profile.kind.managed() { home.clone() } else { crate::ai::system_folder()? };
    let clis = notes::run_clis(&plugin);
    // The launcher by its full path, told which Echo to run (`ECHOCTL_BIN`, below): this one — `echoctl` on
    // PATH may be another version.
    let launcher = clis.iter().find(|c| c.contains('/') && c.contains("scripts")).cloned().unwrap_or_else(|| "echoctl".into());
    // A routine or Yolo acts on its own: as the routine asked, else as the user set routines or Yolo.
    let acts = spec.session.then(|| spec.tools.permissions().unwrap_or_else(|| crate::folder_args::run_permissions(spec.yolo)));
    // What the Brain holds and when to go there (#34): only when it is on and the run has tools to ask it.
    #[cfg(feature = "brain")]
    // Only where the run gets the echo server (the window's builds; a run with tools).
    let brain_text = (cfg!(feature = "tray") && spec.session).then(|| crate::brain::advert::block(&crate::brain::access::Access { profile: profile.id.clone(), ..Default::default() })).flatten();
    #[cfg(not(feature = "brain"))]
    let brain_text: Option<String> = None;
    let prompt = format!(
        "{}{}\n\n{}",
        preamble(acts.is_some(), &format!("`{launcher}` (its full path, exactly so)")),
        brain_text.map(|b| format!("\n\n<echo_recall>\n{b}\n</echo_recall>")).unwrap_or_default(),
        spec.prompt
    );
    let _ = fs::write(spec.run_dir.join("prompt.md"), &prompt);
    // What it asked for, and what it runs with (a routine or Yolo: the folder session's arguments, its own permissions).
    let used = match acts {
        None => "no tools".to_string(),
        Some(m) => format!("the folder session's: acts on its own ({})", crate::folder_args::permission_args(m, profile, model.as_deref(), true).1),
    };
    let actions = spec.run_dir.join("actions.jsonl");
    let _ = fs::remove_file(&actions);
    let _ = fs::write(spec.run_dir.join("permissions.json"), serde_json::json!({"asked": spec.tools.name(), "used": used}).to_string());
    let _ = writeln!(log, "{}  Echo runs “{}” on {}{} (asked: {}; runs with {used})", Local::now().format("%H:%M:%S"), spec.title, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default(), spec.tools.name());
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    let (settings_file, env) = match acts {
        // A routine or Yolo: the folder session's arguments, but for `-p` and its prompt.
        Some(acts) => {
            let f = crate::folder_args::run_settings();
            a.extend(crate::folder_args::session(profile, &exe, &f, "routine", model.as_deref(), crate::folder_args::Who::Run(acts))?);
            let mut env = crate::folder_args::env(&profile.id);
            env.push((notes::RUN_SESSION, "acts".into()));
            env.push((notes::RUN_ACTIONS, actions.display().to_string()));
            (Some(f), env)
        }
        // No tools at all: what it reads is in its prompt.
        None => {
            if let Some(m) = &model {
                a.extend(["--model".into(), m.clone()]);
            }
            a.extend(["--tools".into(), String::new(), "--strict-mcp-config".into(), "--no-session-persistence".into()]);
            (None, vec![])
        }
    };
    // The permission mode it asks Claude Code for: checked against the one it starts in.
    let asked = acts.map(|_| asked_mode(&a));
    let child = crate::ai::guard_ready(profile, &cwd, "run a plugin's job").and_then(|()| notes::admit_child(|| {
        let mut c = crate::ai::prepare_claude(profile, &a, &cwd, false)?;
        c.env("ECHOCTL_BIN", &exe);
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
    let out = child.and_then(|c| finish(c, &mut log, &spec.run_dir, spec.timeout, asked));
    if let Some(f) = settings_file {
        let _ = fs::remove_file(f);
    }
    // What it changed and sent with Echo's CLI, in its log too.
    for x in notes::run_actions(&actions) {
        let line: String = format!("{}: echoctl {}{}", x["kind"].as_str().unwrap_or(""), x["command"].as_str().unwrap_or(""), x["outcome"].as_str().map(|o| format!(" → {o}")).unwrap_or_default());
        // One line each: its words as they are, line breaks and control characters shown, not obeyed.
        let line: String = line.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        let _ = writeln!(log, "{}  {line}", x["at"].as_str().unwrap_or(""));
    }
    let (answer, result) = out?;
    let _ = fs::write(spec.run_dir.join("answer.md"), &answer);
    Ok((answer, model.unwrap_or_default(), usage(&result)))
}

/// The permission mode Claude Code is asked to start in by `args`: its `--permission-mode`, `bypassPermissions` when
/// every check is skipped, else its manual mode (`default`).
fn asked_mode(args: &[String]) -> &'static str {
    match args.windows(2).find(|w| w[0] == "--permission-mode").map(|w| w[1].as_str()) {
        Some("auto") => "auto",
        Some("acceptEdits") => "acceptEdits",
        Some("dontAsk") => "dontAsk",
        _ if args.iter().any(|a| a == "--dangerously-skip-permissions") => "bypassPermissions",
        _ => "default",
    }
}

/// Why a run's start event (`system/init`) isn't in the permission mode Echo asked for (`asked`), or None. Claude Code
/// can start in its manual mode without a word — auto mode not on for this model, provider or account, or turned off
/// by an admin — where a run on its own has every edit refused.
fn mode_differs(asked: &str, init: &Value) -> Option<String> {
    let ran = init["permissionMode"].as_str().filter(|m| !m.is_empty() && *m != asked)?;
    Some(match (asked, ran) {
        ("auto", "default") => "Claude Code ran in manual mode, not auto: its edits were refused — choose Skip all permission checks or a model with auto mode".into(),
        _ => format!("Claude Code ran in permission mode {ran}, not {asked} as Echo asked"),
    })
}

/// The run to its end (`projectsync::finish_run_watched`), its start checked against the permission mode Echo asked
/// for (`asked`; None: a run with no tools): when it started in another, its log and its record (`permissions.json`,
/// `mode`) say so.
fn finish(child: std::process::Child, log: &mut fs::File, run_dir: &Path, timeout: Duration, asked: Option<&str>) -> Result<(String, Value)> {
    let mut on_init = |init: &Value, log: &mut fs::File| {
        let Some(why) = asked.and_then(|a| mode_differs(a, init)) else { return };
        let _ = writeln!(log, "{}  {why}", Local::now().format("%H:%M:%S"));
        let record = run_dir.join("permissions.json");
        let mut r: Value = fs::read_to_string(&record).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| serde_json::json!({}));
        r["mode"] = Value::String(why);
        let _ = fs::write(&record, r.to_string());
    };
    crate::projectsync::finish_run_watched(child, log, timeout, "run a plugin's job", "Echo quit during the run", &mut on_init)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Permissions;

    /// The arguments of a session in Echo's data folder, by what doesn't differ from run to run: its settings file and
    /// the Brain's config (each a file of its own) named by their flag, a run's own conversation and model left out.
    fn shared(args: &[String]) -> Vec<String> {
        let mut out: Vec<String> = vec![];
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-p" | "--session-id" | "--resume" | "--model" | "--output-format" => i += 2,
                "--verbose" => i += 1,
                "--settings" | "--mcp-config" => {
                    out.extend([args[i].clone(), "<file>".into()]);
                    i += 2;
                }
                a => {
                    out.push(a.to_string());
                    i += 1;
                }
            }
        }
        out
    }

    /// The user's three permission settings: the folder session's, the routines' and Yolo's.
    fn set_permissions(folder: Permissions, routine: Option<Permissions>, yolo: Option<Permissions>) {
        let mut c = crate::config::Config::load().unwrap_or_default();
        c.folder_permissions = Some(folder);
        c.routine_permissions = routine;
        c.yolo_permissions = yolo;
        c.save().unwrap();
    }


    #[test]
    #[cfg(all(unix, feature = "tray"))]
    fn a_routine_and_a_yolo_task_run_as_the_folder_session_does() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-session");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        let home = dunce::canonicalize(paths::home()).unwrap();
        let file = |n: &str| fs::read_to_string(crate::ai::fake::file(&p, n)).unwrap_or_default();
        let lines = |n: &str| file(n).split('\0').filter(|a| !a.is_empty()).map(str::to_owned).collect::<Vec<String>>();
        // A routine that asked for the read-only permissions of before: the folder session's, in the data folder, acting
        // on its own in auto mode, its model the one it asked for, its ask kept. The folder session in auto mode too.
        set_permissions(Permissions::Auto, None, None);
        let spec = Spec::from_value(&serde_json::json!({"prompt": "List what changed", "tools": "read", "profile": "work", "model": "claude-opus-4-7", "dir": "runs/r1"}), &root.join("store")).unwrap();
        assert!(spec.session);
        let (_, model, _) = run(&spec).unwrap_or_default();
        let routine = lines("p.args");
        assert_eq!(file("p.cwd").trim(), home.display().to_string());
        assert!(routine.windows(2).any(|w| w[0] == "--model" && w[1] == "claude-opus-4-7"), "{routine:?}");
        assert!(fs::read_to_string(root.join("store/runs/r1/log.txt")).unwrap().contains(", model claude-opus-4-7 (asked: auto; runs with the folder session's: acts on its own (auto mode"));
        // The profile's own model for a Claude name: what the run asks for, and what its log says.
        crate::ai::set_model_names("work", &["opus=acme-gw/opus-large".into()]).unwrap();
        let spec = Spec::from_value(&serde_json::json!({"prompt": "List what changed", "tools": "read", "profile": "work", "model": "opus", "dir": "runs/r1b"}), &root.join("store")).unwrap();
        let _ = run(&spec);
        assert!(lines("p.args").windows(2).any(|w| w[0] == "--model" && w[1] == "acme-gw/opus-large"), "{:?}", lines("p.args"));
        assert!(fs::read_to_string(root.join("store/runs/r1b/log.txt")).unwrap().contains(", model acme-gw/opus-large"));
        crate::ai::set_model_names("work", &["opus=".into()]).unwrap();
        let _ = model;
        assert!(!routine.contains(&"--tools".to_string()) && !routine.contains(&"--dangerously-skip-permissions".to_string()), "{routine:?}");
        assert!(routine.windows(2).any(|w| w[0] == "--permission-mode" && w[1] == "auto"), "{routine:?}");
        // Echo's reads and its own data's edits allowed by name; a send is left to Claude Code's auto mode.
        let cli = crate::notes::plugin_dir().map(|d| crate::notes::run_clis(&d)[0].clone()).unwrap();
        for rule in [format!("Bash({cli} status:*)"), format!("Bash({cli} project show:*)"), format!("Bash({cli} project work:*)"), format!("Bash({cli} compose:*)")] {
            assert!(routine.contains(&rule), "{rule}: {routine:?}");
        }
        assert!(!routine.iter().any(|a| a.contains(" m365")), "no send allowed by name");
        let env = file("p.env");
        assert!(env.contains("ECHOCTL_TERMINAL=1") && env.contains("ECHOCTL_SESSION_PROFILE=work") && env.contains("ECHOCTL_RUN_SESSION=acts") && !env.contains("ECHOCTL_RUN_MODE="), "{env}");
        let settings: Value = serde_json::from_str(&file("p.settings")).unwrap();
        let commands = |s: &Value, k: &str| s["hooks"][k].as_array().into_iter().flatten().flat_map(|h| h["hooks"].as_array().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert!(commands(&settings, "SessionStart").iter().any(|c| c.ends_with(" core --hook")) && commands(&settings, "SessionStart").iter().any(|c| c.ends_with(" view-hook")));
        assert!(commands(&settings, "PreToolUse").iter().any(|c| c.ends_with(" plugins-hook")));
        assert!(commands(&settings, "UserPromptSubmit").is_empty(), "what the window shows isn't an unattended run's");
        let record: Value = serde_json::from_str(&fs::read_to_string(root.join("store/runs/r1/permissions.json")).unwrap()).unwrap();
        assert_eq!(record["asked"], "auto", "`read` of before is auto");
        assert!(record["used"].as_str().unwrap().contains("acts on its own (auto mode"));
        // The real folder session's arguments: the same, but for its own conversation, `-p` and the model.
        let id = crate::terminal::start_folder("claude:folder-test", Some("work"), None, 80, 20).unwrap();
        crate::paths::testenv::eventually("the folder session started", || crate::ai::fake::file(&p, "session.args").is_file());
        let folder = lines("session.args");
        crate::terminal::close(id);
        assert_eq!(shared(&routine), shared(&folder));
        assert_eq!(file("session.cwd").trim(), file("p.cwd").trim());
        let fs_settings: Value = serde_json::from_str(&file("session.settings")).unwrap();
        assert!(!commands(&fs_settings, "UserPromptSubmit").is_empty(), "the folder session gets what the window shows");
        // `tools: "none"` keeps no tools, whatever the settings.
        set_permissions(Permissions::Skip, Some(Permissions::Skip), Some(Permissions::Skip));
        let none = Spec::from_value(&serde_json::json!({"prompt": "Describe this", "tools": "none", "profile": "work", "dir": "runs/r2"}), &root.join("store")).unwrap();
        assert!(!none.session);
        let _ = run(&none);
        let args: Vec<String> = file("p.args").split('\0').map(str::to_owned).collect();
        assert!(args.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()) && !args.contains(&"--dangerously-skip-permissions".to_string()) && !args.contains(&"--settings".to_string()), "{args:?}");
    }

    #[test]
    #[cfg(unix)]
    fn wont_do_reads_others_words_with_no_tools() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-wont");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        set_permissions(Permissions::Skip, Some(Permissions::Skip), Some(Permissions::Skip));
        let wont = Spec { title: "Won't do: x".into(), prompt: "Who is it for?".into(), profile: "work".into(), model: String::new(), tools: Tools::None, run_dir: root.join("wont/1"), timeout: Duration::from_secs(60), session: false, task: None, thread: None, yolo: false };
        let _ = run(&wont);
        let args: Vec<String> = fs::read_to_string(crate::ai::fake::file(&p, "p.args")).unwrap().split('\0').map(str::to_owned).collect();
        assert!(args.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()), "{args:?}");
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions" || a == "--settings" || a == "--plugin-dir" || a == "--permission-mode"), "{args:?}");
    }

    #[test]
    #[cfg(all(unix, feature = "tray"))]
    fn a_routine_and_yolo_acting_on_their_own_each_run_in_their_own_permission_mode_auto_by_default() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-permissions");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        let file = |n: &str| fs::read_to_string(crate::ai::fake::file(&p, n)).unwrap_or_default();
        let args = || file("p.args").split('\0').filter(|a| !a.is_empty()).map(str::to_owned).collect::<Vec<String>>();
        // What Claude Code started with: (skip every check, its `--permission-mode`), the run's session marker, its record.
        let started = |dir: &Path| {
            let a = args();
            let mode = a.windows(2).find(|w| w[0] == "--permission-mode").map(|w| w[1].clone());
            let session = file("p.env").lines().find_map(|l| l.strip_prefix("ECHOCTL_RUN_SESSION=").map(str::to_owned));
            let used = serde_json::from_str::<Value>(&fs::read_to_string(dir.join("permissions.json")).unwrap()).unwrap()["used"].as_str().unwrap().to_string();
            let log = fs::read_to_string(dir.join("log.txt")).unwrap();
            assert!(log.lines().next().unwrap().ends_with(&format!("runs with {used})")), "the log names its mode: {log}");
            (a.contains(&"--dangerously-skip-permissions".to_string()), mode, session.unwrap_or_default(), used)
        };
        let n = std::cell::Cell::new(0);
        let routine_asking = |tools: &str| {
            n.set(n.get() + 1);
            let s = Spec::from_value(&serde_json::json!({"prompt": "Sum it up", "tools": tools, "profile": "work", "dir": format!("runs/p{}", n.get())}), &root.join("store")).unwrap();
            let _ = run(&s);
            started(&s.run_dir)
        };
        let routine = || routine_asking("");
        let yolo = || {
            n.set(n.get() + 1);
            let s = Spec { title: "Yolo: x".into(), prompt: "Do it".into(), profile: "work".into(), model: String::new(), tools: Tools::Setting, run_dir: root.join(format!("yolo/{}", n.get())), timeout: Duration::from_secs(60), session: true, task: Some("acme/T001".into()), thread: Some("acme".into()), yolo: true };
            let _ = run(&s);
            started(&s.run_dir)
        };
        let auto = (false, Some("auto".to_string()), "acts".to_string(), "the folder session's: acts on its own (auto mode: Claude Code decides each action)".to_string());
        let skip = (true, None, "acts".to_string(), "the folder session's: acts on its own (every permission check skipped)".to_string());
        // Nothing set (a new folder, or the old switch off): both act on their own in auto mode.
        assert!(crate::config::Config::load().unwrap().folder_permissions.is_none());
        assert_eq!((routine(), yolo()), (auto.clone(), auto.clone()));
        // Each its own: skipping every check for routines leaves Yolo in auto mode, and the other way round.
        set_permissions(Permissions::Auto, Some(Permissions::Skip), None);
        assert_eq!((routine(), yolo()), (skip.clone(), auto.clone()));
        set_permissions(Permissions::Skip, None, Some(Permissions::Skip));
        assert_eq!((routine(), yolo()), (auto.clone(), skip.clone()), "the folder session's skip isn't the routines'");
        // The folder session asking each time: routines and Yolo still act on their own, each with its own setting.
        set_permissions(Permissions::Ask, Some(Permissions::Skip), None);
        assert_eq!((routine(), yolo()), (skip.clone(), auto.clone()));
        // A routine's own choice wins over the setting: `auto` (and `read` / `folder` of before), or `skip`.
        for tools in ["auto", "read", "folder"] {
            assert_eq!(routine_asking(tools), auto, "{tools}");
        }
        set_permissions(Permissions::Auto, None, None);
        assert_eq!(routine_asking("skip"), skip);
        assert!(Spec::from_value(&serde_json::json!({"prompt": "x", "tools": "ask"}), &root.join("store")).unwrap_err().to_string().contains("auto, skip or none"));
    }

    /// Claude Code may start in its manual mode though Echo asked for auto mode: the run's log and record say so.
    #[test]
    #[cfg(unix)]
    fn a_run_that_claude_code_started_in_another_permission_mode_than_asked_says_so_in_its_log_and_record() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-mode");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        let n = std::cell::Cell::new(0);
        // A run asking for `asked`, Claude Code starting in `ran`: its answer, log and record.
        let run = |asked: Option<&str>, ran: &str| {
            n.set(n.get() + 1);
            let dir = root.join(format!("runs/m{}", n.get()));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("permissions.json"), r#"{"asked":"setting","used":"x"}"#).unwrap();
            let init = serde_json::json!({"type": "system", "subtype": "init", "claude_code_version": "9.0.0", "permissionMode": ran});
            let result = serde_json::json!({"type": "result", "result": "nothing done", "is_error": false});
            fs::write(crate::ai::fake::file(&p, "p.reply"), format!("{init}\n{result}\n")).unwrap();
            let child = crate::ai::prepare_claude(&p, &["-p".into(), "x".into()], &root, false).unwrap().spawn().unwrap();
            let mut log = fs::File::create(dir.join("log.txt")).unwrap();
            let (answer, _) = finish(child, &mut log, &dir, Duration::from_secs(30), asked).unwrap();
            let record: Value = serde_json::from_str(&fs::read_to_string(dir.join("permissions.json")).unwrap()).unwrap();
            (answer, fs::read_to_string(dir.join("log.txt")).unwrap(), record)
        };
        let manual = "Claude Code ran in manual mode, not auto: its edits were refused — choose Skip all permission checks or a model with auto mode";
        // Auto asked, manual started: said in the log (after the start line) and the record; the run still finishes.
        let (answer, log, record) = run(Some("auto"), "default");
        assert_eq!(answer, "nothing done");
        let lines: Vec<&str> = log.lines().collect();
        assert!(lines[0].contains("Claude Code started") && lines[1].ends_with(manual), "{log}");
        assert_eq!((record["mode"].as_str(), record["used"].as_str()), (Some(manual), Some("x")));
        // Started as asked: nothing said.
        let (_, log, record) = run(Some("auto"), "auto");
        assert!(!log.contains("ran in") && record.get("mode").is_none(), "{log}");
        // Another mode than asked: named.
        let (_, log, _) = run(Some("bypassPermissions"), "default");
        assert!(log.contains("Claude Code ran in permission mode default, not bypassPermissions as Echo asked"), "{log}");
        // A run with no tools: its mode isn't checked.
        let (_, log, record) = run(None, "default");
        assert!(!log.contains("ran in") && record.get("mode").is_none(), "{log}");
        // What each setting asks for.
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(asked_mode(&args(&["-p", "x", "--permission-mode", "auto"])), "auto");
        assert_eq!(asked_mode(&args(&["--permission-mode", "acceptEdits"])), "acceptEdits");
        assert_eq!(asked_mode(&args(&["--dangerously-skip-permissions"])), "bypassPermissions");
        assert_eq!(asked_mode(&args(&["-p", "x"])), "default");
    }

    #[test]
    fn a_plugins_run_cant_choose_a_wrapper_profile_that_isnt_the_users_default() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-wrapper");
        std::fs::create_dir_all(crate::paths::config_dir()).unwrap();
        std::fs::write(crate::paths::config_dir().join("ai.json"), r#"{"profiles":[{"id":"acme","name":"Acme","kind":"wrapper","wrapper":{"command":"/opt/acme/acme-claude"}}]}"#).unwrap();
        let e = Spec::from_value(&serde_json::json!({"prompt": "x", "profile": "acme"}), &root.join("store")).unwrap_err();
        assert!(e.to_string().contains("can't choose the wrapper profile"), "{e}");
        std::fs::write(crate::paths::config_dir().join("ai.json"), r#"{"default":"acme","profiles":[{"id":"acme","name":"Acme","kind":"wrapper","wrapper":{"command":"/opt/acme/acme-claude"}}]}"#).unwrap();
        assert!(Spec::from_value(&serde_json::json!({"prompt": "x", "profile": "acme"}), &root.join("store")).is_ok(), "the user's default");
    }
}

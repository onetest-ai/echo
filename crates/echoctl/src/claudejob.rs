//! A Claude Code run a plugin asks for (`ctx.claude.run`, `docs/plugins-design.md` v4), or Yolo's: headless, on one of
//! the user's AI profiles (strict routing, the profile's lease), with the Brain narrowed to that profile.
//!
//! A routine's or Yolo's run (`Spec::session`) runs as the folder session does (#590): in the data folder, with its
//! arguments, settings and hooks (`folder_args::session`), and the user's one choice (Settings → AI & minutes):
//! - **acts on its own** — every permission prompt skipped, and the CLI's limits for runs lifted as in the folder
//!   session (`notes::RUN_SESSION=acts`; Yolo keeps its own task and thread), every change and send noted;
//! - **prepares and asks** — Echo's reads and its own data's edits run without a prompt (`folder_args::FREE_COMMANDS`),
//!   a mail is left as a draft (`compose`), anything else that would ask is refused.
//!
//! The `tools` it asked for is kept in its record (`permissions.json`); only `none` (a plugin's analysis of what it
//! read, Won't do) is honoured: no tools at all.
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
        })
    }
}

/// What Echo tells the run around the plugin's prompt: `acts` — a routine's or Yolo's run as the folder session (some:
/// the user lets it act on its own or not); none — no tools at all.
fn preamble(acts: Option<bool>, cli: &str) -> String {
    let reach = match acts {
        None => "You have no tools: everything you need is below.".to_string(),
        Some(true) => format!(
            "Echo's CLI is {cli}; what was said and who people are: the `echo` MCP tools. The user lets you act on your own: \
             you may change Echo's data and send through Echo's CLI, as in their own session. Every change and send you make with Echo's CLI is listed for them."
        ),
        Some(false) => format!(
            "Echo's CLI is {cli}; what was said and who people are: the `echo` MCP tools. You may read Echo's data and edit its own \
             data (a thread's work: `project work <thread> '<edit JSON>'`, its tree: `project work <thread>`) without asking; a \
             plugin's commands only read, and only its scripts marked readonly run. Nothing is sent: leave a mail as a draft with \
             `compose --to … --subject … --body '…'`. Anything else that would need the user's yes can't be done here: say what you didn't do and why."
        ),
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
    let model = Some(spec.model.clone()).filter(|m| !m.is_empty()).or_else(|| profile.notes_model());
    let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    let home = dunce::canonicalize(paths::home())?;
    // A session-like run starts in the data folder, as the folder session does: `.plugins/<id>/` loads from there.
    let cwd = if spec.session || profile.kind != crate::ai::Kind::System { home.clone() } else { crate::ai::system_folder()? };
    let clis = notes::run_clis(&plugin);
    // The launcher by its full path, told which Echo to run (`ECHOCTL_BIN`, below): this one — `echoctl` on
    // PATH may be another version.
    let launcher = clis.iter().find(|c| c.contains('/') && c.contains("scripts")).cloned().unwrap_or_else(|| "echoctl".into());
    let acts = spec.session.then(crate::folder_args::skip_permissions);
    // What the Brain holds and when to go there (#34): only when it is on and the run has tools to ask it.
    #[cfg(feature = "brain")]
    // Only where the run gets the echo server (the window's builds; a run with tools).
    let brain_text = (cfg!(feature = "tray") && spec.session).then(|| crate::brain::advert::block(&crate::brain::access::Access { profile: profile.id.clone(), ..Default::default() })).flatten();
    #[cfg(not(feature = "brain"))]
    let brain_text: Option<String> = None;
    let prompt = format!(
        "{}{}\n\n{}",
        preamble(acts, &format!("`{launcher}` (its full path, exactly so)")),
        brain_text.map(|b| format!("\n\n<echo_recall>\n{b}\n</echo_recall>")).unwrap_or_default(),
        spec.prompt
    );
    let _ = fs::write(spec.run_dir.join("prompt.md"), &prompt);
    // What it asked for, and what it runs with (a routine or Yolo: the folder session's permissions).
    let used = match acts {
        None => "no tools".to_string(),
        Some(true) => "the folder session's: acts on its own (every permission prompt skipped)".to_string(),
        Some(false) => "the folder session's: prepares and asks (reads and Echo's own edits; nothing sent; what would ask is refused)".to_string(),
    };
    let actions = spec.run_dir.join("actions.jsonl");
    let _ = fs::remove_file(&actions);
    let _ = fs::write(spec.run_dir.join("permissions.json"), serde_json::json!({"asked": spec.tools.name(), "used": used}).to_string());
    let _ = writeln!(log, "{}  Echo runs “{}” on {}{} (asked: {} permissions; runs with {used})", Local::now().format("%H:%M:%S"), spec.title, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default(), spec.tools.word());
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    let (settings_file, env) = match acts {
        // A routine or Yolo: the folder session's arguments, but for `-p` and its prompt.
        Some(acts) => {
            let f = crate::folder_args::run_settings();
            a.extend(crate::folder_args::session(profile, &exe, &f, "routine", model.as_deref(), crate::folder_args::Who::Run { acts })?);
            let mut env = crate::folder_args::env(&profile.id);
            env.push((notes::RUN_SESSION, if acts { "acts" } else { "asks" }.into()));
            env.push((notes::RUN_ACTIONS, actions.display().to_string()));
            // Preparing and asking: a plugin's commands it runs read and change nothing, and only its readonly scripts
            // run (`jshost::read_only_here`, `echoctl plugin script`).
            if !acts {
                env.push((guard::RUN_MODE, "routine".into()));
            }
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
    let out = child.and_then(|c| crate::projectsync::finish_run_result(c, &mut log, spec.timeout, "run a plugin's job", "Echo quit during the run"));
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

#[cfg(test)]
mod tests {
    use super::*;

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

    fn set_acts(on: bool) {
        let mut c = crate::config::Config::load().unwrap_or_default();
        c.folder_skip_permissions = on;
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
        // A routine asking for read-only permissions, the user's choice "prepares and asks": the folder session's, in
        // the data folder, its model the one it asked for, its ask kept.
        set_acts(false);
        let spec = Spec::from_value(&serde_json::json!({"prompt": "List what changed", "tools": "read", "profile": "work", "model": "opus-test", "dir": "runs/r1"}), &root.join("store")).unwrap();
        assert!(spec.session);
        let (_, model, _) = run(&spec).unwrap_or_default();
        let routine = lines("p.args");
        assert_eq!(file("p.cwd").trim(), home.display().to_string());
        assert!(routine.windows(2).any(|w| w[0] == "--model" && w[1] == "opus-test"), "{routine:?}");
        assert!(fs::read_to_string(root.join("store/runs/r1/log.txt")).unwrap().contains(", model opus-test (asked: read permissions"));
        let _ = model;
        assert!(!routine.contains(&"--tools".to_string()) && !routine.contains(&"--dangerously-skip-permissions".to_string()), "{routine:?}");
        // Echo's reads and its own data's edits without a prompt; never a send.
        let cli = crate::notes::plugin_dir().map(|d| crate::notes::run_clis(&d)[0].clone()).unwrap();
        for rule in [format!("Bash({cli} status:*)"), format!("Bash({cli} project show:*)"), format!("Bash({cli} project work:*)"), format!("Bash({cli} compose:*)")] {
            assert!(routine.contains(&rule), "{rule}: {routine:?}");
        }
        assert!(!routine.iter().any(|a| a.contains(" m365")), "no send without a prompt");
        let env = file("p.env");
        assert!(env.contains("ECHOCTL_TERMINAL=1") && env.contains("ECHOCTL_SESSION_PROFILE=work") && env.contains("ECHOCTL_RUN_SESSION=asks") && env.contains("ECHOCTL_RUN_MODE=routine"), "preparing and asking: a plugin's commands only read: {env}");
        let settings: Value = serde_json::from_str(&file("p.settings")).unwrap();
        let commands = |s: &Value, k: &str| s["hooks"][k].as_array().into_iter().flatten().flat_map(|h| h["hooks"].as_array().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert!(commands(&settings, "SessionStart").iter().any(|c| c.ends_with(" core --hook")) && commands(&settings, "SessionStart").iter().any(|c| c.ends_with(" view-hook")));
        assert!(commands(&settings, "PreToolUse").iter().any(|c| c.ends_with(" plugins-hook")));
        assert!(commands(&settings, "UserPromptSubmit").is_empty(), "what the window shows isn't an unattended run's");
        let record: Value = serde_json::from_str(&fs::read_to_string(root.join("store/runs/r1/permissions.json")).unwrap()).unwrap();
        assert_eq!(record["asked"], "read");
        assert!(record["used"].as_str().unwrap().contains("prepares and asks"));
        // The real folder session's arguments: the same, but for its own conversation, `-p` and the model.
        let id = crate::terminal::start_folder("claude:folder-test", Some("work"), None, 80, 20).unwrap();
        crate::paths::testenv::eventually("the folder session started", || crate::ai::fake::file(&p, "session.args").is_file());
        let folder = lines("session.args");
        crate::terminal::close(id);
        assert_eq!(shared(&routine), shared(&folder));
        assert_eq!(file("session.cwd").trim(), file("p.cwd").trim());
        let fs_settings: Value = serde_json::from_str(&file("session.settings")).unwrap();
        assert!(!commands(&fs_settings, "UserPromptSubmit").is_empty(), "the folder session gets what the window shows");
        // With the user's choice "acts on its own", a Yolo task skips prompts; off, it doesn't, whatever it asked.
        set_acts(true);
        let yolo = Spec { title: "Yolo: x".into(), prompt: "Do it".into(), profile: "work".into(), model: String::new(), tools: Tools::Skip, run_dir: root.join("yolo/1"), timeout: Duration::from_secs(60), session: true, task: None, thread: None };
        let _ = run(&yolo);
        assert!(lines("p.args").contains(&"--dangerously-skip-permissions".to_string()) && file("p.env").contains("ECHOCTL_RUN_SESSION=acts") && !file("p.env").contains("ECHOCTL_RUN_MODE="));
        set_acts(false);
        let _ = run(&yolo);
        assert!(!lines("p.args").contains(&"--dangerously-skip-permissions".to_string()));
        // `tools: "none"` keeps no tools in either mode.
        set_acts(true);
        let none = Spec::from_value(&serde_json::json!({"prompt": "Describe this", "tools": "none", "profile": "work", "dir": "runs/r2"}), &root.join("store")).unwrap();
        assert!(!none.session);
        let _ = run(&none);
        let args: Vec<String> = file("p.args").split('\0').map(str::to_owned).collect();
        assert!(args.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()) && !args.contains(&"--dangerously-skip-permissions".to_string()) && !args.contains(&"--settings".to_string()), "{args:?}");
        set_acts(false);
    }

    #[test]
    #[cfg(unix)]
    fn wont_do_reads_others_words_with_no_tools() {
        let (_env, root) = crate::paths::testenv::folders("claudejob-wont");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        set_acts(true);
        let wont = Spec { title: "Won't do: x".into(), prompt: "Who is it for?".into(), profile: "work".into(), model: String::new(), tools: Tools::None, run_dir: root.join("wont/1"), timeout: Duration::from_secs(60), session: false, task: None, thread: None };
        let _ = run(&wont);
        let args: Vec<String> = fs::read_to_string(crate::ai::fake::file(&p, "p.args")).unwrap().split('\0').map(str::to_owned).collect();
        assert!(args.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()), "{args:?}");
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions" || a == "--settings" || a == "--plugin-dir"), "{args:?}");
        set_acts(false);
    }
}

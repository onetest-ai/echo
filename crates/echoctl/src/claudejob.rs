//! A Claude Code run a plugin asks for (`ctx.claude.run`, `docs/plugins-design.md` v4): headless, on one of
//! the user's AI profiles (strict routing, the profile's lease), with the Brain narrowed to that profile,
//! and with the permissions the plugin passes on from the user:
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
    /// More plugins for the run (`--plugin-dir`): Yolo's task gets the user's skills.
    pub plugins: Vec<PathBuf>,
    /// The user's own run (Yolo), not a plugin's: Echo's CLI answers it as it answers the user (no headless limits),
    /// and a profile of its own gets Echo's plugin too (its skills: manage, Microsoft 365), as the panel does.
    pub as_user: bool,
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
            plugins: vec![],
            as_user: false,
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
    let cwd = if profile.kind == crate::ai::Kind::System { crate::ai::system_folder()? } else { home.clone() };
    let clis = notes::run_clis(&plugin);
    // The launcher by its full path, told which Echo to run (`ECHOCTL_BIN`, below): this one — `echoctl` on
    // PATH may be another version.
    let launcher = clis.iter().find(|c| c.contains('/') && c.contains("scripts")).cloned().unwrap_or_else(|| "echoctl".into());
    // What the Brain holds and when to go there (#34): only when it is on and the run has tools to ask it.
    #[cfg(feature = "brain")]
    // Only where the run gets the echo server (the window's builds; any tool mode but none).
    let brain_text = (cfg!(feature = "tray") && spec.tools != Tools::None).then(|| crate::brain::advert::block(&crate::brain::access::Access { profile: profile.id.clone(), ..Default::default() })).flatten();
    #[cfg(not(feature = "brain"))]
    let brain_text: Option<String> = None;
    let prompt = format!(
        "{}{}\n\n{}",
        preamble(spec.tools, &format!("`{launcher}` (its full path, exactly so)")),
        brain_text.map(|b| format!("\n\n<echo_recall>\n{b}\n</echo_recall>")).unwrap_or_default(),
        spec.prompt
    );
    let _ = fs::write(spec.run_dir.join("prompt.md"), &prompt);
    let _ = writeln!(log, "{}  Echo runs “{}” on {}{} ({} permissions)", Local::now().format("%H:%M:%S"), spec.title, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default(), spec.tools.word());
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    if let Some(m) = &model {
        a.extend(["--model".into(), m.clone()]);
    }
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    // The Brain, narrowed to this profile's sources (strict routing).
    #[cfg(all(feature = "tray", feature = "brain"))]
    let brain = crate::terminal::brain_mcp_config(&exe, &["--profile".into(), profile.id.clone(), "--who".into(), "routine".into()]);
    #[cfg(not(all(feature = "tray", feature = "brain")))]
    let brain: Option<String> = None;
    let mut settings_file = None;
    // What plugins add to Echo's sessions: their skills, and folders to read (also for a read-only run).
    let (session_plugins, session_read) = (crate::grants::session_plugins(), crate::grants::session_read());
    if spec.tools != Tools::None {
        for d in session_plugins.iter().chain(&spec.plugins) {
            a.extend(["--plugin-dir".into(), d.display().to_string()]);
        }
        if spec.as_user && profile.kind.managed() {
            a.extend(["--plugin-dir".into(), plugin.display().to_string()]);
        }
    }
    match spec.tools {
        Tools::None => {
            a.extend(["--tools".into(), String::new(), "--strict-mcp-config".into(), "--no-session-persistence".into()]);
        }
        Tools::Read => {
            crate::ai::hooks_conflict(profile, &cwd).map_err(|w| anyhow!(w))?;
            let folder = paths::config_dir().join("minutes-run");
            fs::create_dir_all(&folder)?;
            let f = folder.join(format!("job-{}-{}.json", std::process::id(), Local::now().timestamp_millis()));
            fs::write(&f, serde_json::to_vec_pretty(&guard::settings(&exe, &fwd(&paths::calls_dir())))?)?;
            let tools = if session_plugins.is_empty() { "Bash,Read,Glob,Grep" } else { "Bash,Read,Glob,Grep,Skill" };
            a.extend(["--settings".into(), f.display().to_string(), "--tools".into(), tools.into(), "--strict-mcp-config".into()]);
            if let Some(b) = &brain {
                a.extend(["--mcp-config".into(), b.clone()]);
            }
            a.extend(["--permission-mode".into(), "dontAsk".into(), "--allowedTools".into()]);
            let abs = format!("/{}", notes::rule_path(&home));
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
            a.push("mcp__echo".into());
            settings_file = Some(f);
        }
        Tools::Folder | Tools::Skip => {
            if let Some(b) = &brain {
                a.extend(["--mcp-config".into(), b.clone(), "--allowedTools".into(), "mcp__echo".into()]);
            }
            if spec.tools == Tools::Skip {
                a.push("--dangerously-skip-permissions".into());
            } else {
                a.extend(["--permission-mode".into(), "dontAsk".into()]);
            }
        }
    }
    let child = notes::admit_child(|| {
        let mut c = crate::ai::prepare_claude(profile, &a, &cwd, false)?;
        c.env("ECHOCTL_BIN", &exe);
        if spec.as_user {
            c.env_remove(notes::HEADLESS);
        }
        if spec.tools == Tools::Read {
            c.env(guard::RUN_MODE, "routine").env(guard::RUN_HOME, &home).env(guard::RUN_CLIS, clis.join("\n"));
            c.env(guard::RUN_READ, session_read.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join("\n"));
            c.env(guard::RUN_SKILLS, crate::grants::skill_names(&session_plugins).join("\n"));
        }
        Ok(c.spawn()?)
    });
    let out = child.and_then(|c| crate::projectsync::finish_run_result(c, &mut log, spec.timeout, "run a plugin's job", "Echo quit during the run"));
    if let Some(f) = settings_file {
        let _ = fs::remove_file(f);
    }
    let (answer, result) = out?;
    let _ = fs::write(spec.run_dir.join("answer.md"), &answer);
    Ok((answer, model.unwrap_or_default(), usage(&result)))
}

#[cfg(test)]
mod tests {
    use super::*;

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

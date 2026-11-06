//! A project's sync (`docs/projects-design.md`, taska's `/taska:run`): its calls become evidence
//! (`ledger::sync`), then Claude Code reads the new items into the work tree in two runs of the
//! `project-sync` skill — **extract** (with the dependency pass), then **verify** in a fresh process
//! that never saw the extraction (independence is the point: only its `supports` changes the tree).
//!
//! Each run is guarded like a minutes run (`guard.rs`, `RUN_MODE=project`): it reads only the
//! project's ledger, writes only `.json` files in its own run folder, and runs only the ledger's
//! agent steps for this project. It uses the project's AI profile and model (Profile → For Claude;
//! default: the default profile, its minutes model). The first sync of a project reads its last 20
//! calls; later ones what came since, at most `MAX_ITEMS` a run, oldest first.
//!
//! State: `<project>/ledger/sync.json` (`{state, at, message, run, processed, verdicts}`); each
//! run's files and log in `<project>/ledger/runs/<run>/`. The app's scheduler syncs projects with new
//! calls ended ≥ 15 min ago (`config.project_sync`, default on); `echoctl project ledger <id> run`
//! or the window's Sync now does it at once.

use std::fs;
use std::io::{BufRead, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use serde_json::{json, Value};

use crate::{guard, ledger, notes, paths, projects};

/// The calls the first sync of a project reads (the backfill).
pub const BACKFILL: usize = 20;
/// Evidence items one run reads at most (the next run takes the rest).
const MAX_ITEMS: usize = 10;
/// A run that takes longer is stopped.
const TIMEOUT: Duration = Duration::from_secs(40 * 60);
/// A call is read once it ended this long ago (its minutes and the user's notes come first).
pub(crate) const SETTLE: chrono::Duration = chrono::Duration::minutes(15);
const TOOLS: &str = "Bash,Read,Glob,Grep,Write,Edit,Skill";

fn ledger_dir(id: &str) -> PathBuf {
    projects::dir().join(id).join("ledger")
}

fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// The project's sync state for the window.
pub fn state(id: &str) -> Value {
    fs::read(ledger_dir(id).join("sync.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

fn save_state(id: &str, v: &Value) {
    let _ = fs::create_dir_all(ledger_dir(id));
    let _ = paths::write_atomic(&ledger_dir(id).join("sync.json"), &serde_json::to_vec_pretty(v).unwrap_or_default());
}

/// Sync one project now: evidence, extract, verify. One at a time per project.
pub fn run(id: &str) -> Result<Value> {
    let p = projects::load(id)?;
    fs::create_dir_all(ledger_dir(id))?;
    let _one = crate::app::FileLock::try_take(&ledger_dir(id).join(".sync.lock"))?.ok_or_else(|| anyhow!("{} is being synced already", p.name))?;
    let started = Local::now();
    let first = ledger::Ledger::of(id)?.events()?.iter().all(|e| e["type"] != "processed");
    let mut written = ledger::sync(id, first.then_some(BACKFILL))?;
    // Its meeting chats between calls (a chat that can't be read keeps its error; the calls go on).
    match ledger::chat_sync(id) {
        Ok(w) => written.extend(w),
        Err(e) => eprintln!("project sync {id}: the meeting chat: {e:#}"),
    }
    // Its ✉ Mail sources, trusted senders only (a source that can't be read keeps its error).
    match ledger::mail_sync(id) {
        Ok(w) => written.extend(w),
        Err(e) => eprintln!("project sync {id}: mail: {e:#}"),
    }
    let backlog = ledger::pending(id)?;
    let before = backlog.len();
    let pending: Vec<String> = backlog.into_iter().filter_map(|x| x["id"].as_str().map(str::to_owned)).take(MAX_ITEMS).collect();
    let claims = ledger::verify_prep(id, false)?.len();
    // The people pass reads calls the work sync already read (threads synced before it existed too).
    let people_read = read_people(id);
    let for_people: Vec<String> = pending
        .iter()
        .cloned()
        .chain(ledger::processed_ids(id)?.into_iter().rev())
        .filter(|e| !people_read.contains(e))
        .fold(vec![], |mut v, e| {
            if !v.contains(&e) {
                v.push(e);
            }
            v
        })
        .into_iter()
        .take(MAX_ITEMS)
        .collect();
    if pending.is_empty() && claims == 0 && for_people.is_empty() {
        let v = json!({"state": "done", "at": started.to_rfc3339(), "message": "nothing new", "written": written.len(), "processed": 0});
        save_state(id, &v);
        // Announced as a sync that read the calls is (new mail or chat may have been written).
        #[cfg(feature = "js")]
        crate::jshost::event("thread_synced", &json!({"thread": id, "processed": 0, "left": 0}));
        return Ok(v);
    }
    save_state(id, &json!({"state": "running", "at": started.to_rfc3339(), "message": format!("reading {} call{}", pending.len(), if pending.len() == 1 { "" } else { "s" })}));
    let outcome = (|| -> Result<Value> {
        let profile_id = p.ai_profile.clone().unwrap_or_else(crate::ai::default_id);
        let ready = crate::ai::ready(&profile_id)?;
        let profile = ready.profile();
        let model = p.model.clone().or_else(|| profile.notes_model());
        let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
        let cwd = if profile.kind == crate::ai::Kind::System { crate::ai::system_folder()? } else { paths::home() };
        crate::ai::hooks_conflict(profile, &cwd).map_err(|w| anyhow!(w))?;
        let run_id = started.format("%Y%m%d-%H%M%S").to_string();
        let run_dir = ledger_dir(id).join("runs").join(&run_id);
        fs::create_dir_all(&run_dir)?;
        let run_dir = dunce::canonicalize(&run_dir)?;
        let settings = settings_file(&run_id)?;
        let mut log = fs::File::create(run_dir.join("log.txt"))?;
        let _ = writeln!(log, "{}  Echo syncs {} ({}) on {}{}", Local::now().format("%H:%M:%S"), p.name, id, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default());
        let mut extract_report = String::new();
        if !pending.is_empty() {
            let prompt = prompt(&p, "extract", &pending, &run_dir);
            extract_report = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log)?;
        }
        // Fresh eyes: the verifier is another process, started after the extraction ended.
        let mut verify_report = String::new();
        if !ledger::verify_prep(id, false)?.is_empty() {
            let prompt = prompt(&p, "verify", &[], &run_dir);
            verify_report = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log)?;
        }
        // Who's who: proposals about people and organisations from the same calls, with their words;
        // they wait for the user (`people::decide`).
        let mut people_report = String::new();
        if !for_people.is_empty() {
            let prompt = prompt(&p, "people", &for_people, &run_dir);
            people_report = match claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log) {
                Ok(r) => {
                    mark_people_read(id, &for_people);
                    r
                }
                Err(e) => format!("people pass: {e:#}"),
            };
        }
        let _ = fs::remove_file(&settings);
        let left = ledger::pending(id)?.len();
        Ok(json!({"state": "done", "at": started.to_rfc3339(), "finished_at": Local::now().to_rfc3339(), "run": run_id,
                  "processed": before.saturating_sub(left), "left": left, "message": format!("{extract_report}\n{verify_report}\n{people_report}").trim().to_string()}))
    })();
    let v = match outcome {
        Ok(v) => v,
        Err(e) => {
            let signin = e.downcast_ref::<crate::ai::SignInNeeded>().map(|s| s.profile.clone());
            json!({"state": "failed", "at": started.to_rfc3339(), "message": format!("{e:#}"), "needs_sign_in": signin})
        }
    };
    save_state(id, &v);
    // A thread whose work was read again: the plugins that asked hear it (its id, how many were read, how many wait).
    #[cfg(feature = "js")]
    if v["state"] == "done" {
        crate::jshost::event("thread_synced", &json!({"thread": id, "processed": v["processed"], "left": v["left"]}));
    }
    if v["state"] == "failed" {
        bail!("{}", v["message"].as_str().unwrap_or("the sync failed"));
    }
    Ok(v)
}

/// Evidence the people pass has read (`ledger/people-read-2.json`). Version 2 = since it keeps facts
/// and who someone is: everything is read once more for them (what was proposed before isn't again).
const PEOPLE_READ: &str = "people-read-2.json";

fn read_people(id: &str) -> Vec<String> {
    fs::read(ledger_dir(id).join(PEOPLE_READ)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn mark_people_read(id: &str, ids: &[String]) {
    let mut all = read_people(id);
    all.extend(ids.iter().filter(|i| !all.contains(i)).cloned().collect::<Vec<_>>());
    let _ = paths::write_atomic(&ledger_dir(id).join(PEOPLE_READ), &serde_json::to_vec(&all).unwrap_or_default());
}

fn settings_file(run_id: &str) -> Result<PathBuf> {
    let folder = paths::config_dir().join("minutes-run");
    fs::create_dir_all(&folder)?;
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    let f = folder.join(format!("sync-{run_id}.json"));
    fs::write(&f, serde_json::to_vec_pretty(&guard::settings(&exe, &fwd(&projects::dir())))?)?;
    Ok(f)
}

fn prompt(p: &projects::Project, mode: &str, ids: &[String], run_dir: &Path) -> String {
    let cli = if cfg!(windows) { "the plugin's launcher by its full path (`<plugin>/scripts/echoctl`)" } else { "plain `echoctl` (it is on PATH)" };
    let what = if mode == "extract" {
        format!("Mode: extract. Evidence ids, strictly in this order: {}.", ids.join(", "))
    } else if mode == "people" {
        format!("Mode: people. Evidence ids: {}.", ids.join(", "))
    } else {
        "Mode: verify. Judge every claim `verify-prep` gives you.".to_string()
    };
    format!(
        "/echo:project-sync\n\nProject: `{}` ({}). {what}\n\nThis run was started from the Echo app and nobody is watching it: do \
         every step without asking anything. Run the Echo CLI as {cli}: `echoctl project ledger {} <step> …`, one command per Bash \
         call, with no variables, `cd`, pipes, redirects or `&&`; quote arguments with single quotes, never use `$` or backticks. \
         This run's folder — write your JSON files there with the Write tool, nowhere else: `{}`. Read only the ledger's files. \
         Finish with a report of two to five lines.",
        p.id, p.name, p.id, fwd(run_dir)
    )
}

#[allow(clippy::too_many_arguments)]
fn claude(profile: &crate::ai::Profile, model: Option<&str>, plugin: &Path, cwd: &Path, settings: &Path, prompt: &str, id: &str, run_dir: &Path, log: &mut fs::File) -> Result<String> {
    let ledger = dunce::canonicalize(ledger_dir(id))?;
    let abs = |p: &Path| format!("/{}", notes::rule_path(p));
    let clis = notes::run_clis(plugin);
    let mut a: Vec<String> = vec!["-p".into(), prompt.into(), "--output-format".into(), "stream-json".into(), "--verbose".into()];
    if let Some(m) = model {
        a.extend(["--model".into(), m.into()]);
    }
    a.extend(["--plugin-dir".into(), plugin.display().to_string(), "--settings".into(), settings.display().to_string()]);
    a.extend(["--tools".into(), TOOLS.into(), "--strict-mcp-config".into(), "--permission-mode".into(), "dontAsk".into(), "--allowedTools".into()]);
    a.extend(["Read", "Glob", "Grep"].map(|t| format!("{t}({}/**)", abs(&ledger))));
    a.extend(["Write", "Edit"].map(|t| format!("{t}({}/**)", abs(run_dir))));
    a.push("Skill(echo:project-sync)".into());
    a.push(format!("Read({}/**)", abs(&dunce::canonicalize(plugin.join("skills").join("project-sync")).unwrap_or_else(|_| plugin.join("skills").join("project-sync")))));
    a.extend(clis.iter().flat_map(|cli| guard::PROJECT_STEPS.iter().map(move |s| format!("Bash({cli} project ledger {id} {s}:*)"))));
    let child = notes::admit_child(|| {
        let mut c = crate::ai::prepare_claude(profile, &a, cwd, false)?;
        c.env(guard::RUN_MODE, "project").env(guard::RUN_LEDGER, &ledger).env(guard::RUN_DIR, run_dir).env(guard::RUN_PROJECT, id).env(guard::RUN_CLIS, clis.join("\n"));
        c.env(guard::RUN_SKILL, plugin.join("skills").join("project-sync"));
        Ok(c.spawn()?)
    })?;
    finish_run(child, log, TIMEOUT, "sync a thread", "Echo quit during the sync: it goes on next time")
}

/// Follow a headless Claude Code run started with `--output-format stream-json`: its steps into `log`,
/// stopped after `timeout` or when it is too old for the guard (`what` it was for), and its final
/// answer — or why there is none. Shared by the project sync and routines.
pub fn finish_run(child: std::process::Child, log: &mut fs::File, timeout: Duration, what: &str, quit: &str) -> Result<String> {
    finish_run_result(child, log, timeout, what, quit).map(|(text, _)| text)
}

/// The same, with Claude Code's result event (its usage, cost, duration, turns).
pub fn finish_run_result(mut child: std::process::Child, log: &mut fs::File, timeout: Duration, what: &str, quit: &str) -> Result<(String, Value)> {
    let pid = child.id();
    let err = child.stderr.take().map(|e| std::thread::spawn(move || BufReader::new(e).lines().map_while(Result::ok).collect::<Vec<_>>().join("\n")));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let done = done.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            while t.elapsed() < timeout {
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            crate::ai::stop_group(pid);
        });
    }
    let (mut result, mut stopped) = (None::<Value>, None::<String>);
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let Ok(ev) = serde_json::from_str::<Value>(&line) else {
                if !line.trim().is_empty() {
                    let _ = writeln!(log, "{}  {line}", Local::now().format("%H:%M:%S"));
                }
                continue;
            };
            if ev["type"] == "system" && ev["subtype"] == "init" && !guard::new_enough(ev["claude_code_version"].as_str().unwrap_or("")) {
                let [x, y, z] = guard::MIN_CLAUDE;
                stopped = Some(format!("Claude Code is too old to {what} — update it to {x}.{y}.{z} or later"));
                crate::ai::stop_group(pid);
            }
            for l in notes::describe(&ev) {
                let _ = writeln!(log, "{}  {l}", Local::now().format("%H:%M:%S"));
            }
            if ev["type"] == "result" {
                result = Some(ev);
            }
        }
    }
    let _ = child.wait();
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    notes::forget_child(pid);
    let err = err.and_then(|h| h.join().ok()).unwrap_or_default();
    for l in err.lines().filter(|l| !l.trim().is_empty()) {
        let _ = writeln!(log, "{}  stderr: {l}", Local::now().format("%H:%M:%S"));
    }
    if let Some(why) = stopped {
        bail!(why);
    }
    if notes::quitting() {
        bail!("{quit}");
    }
    let r = result.ok_or_else(|| anyhow!("Claude Code ended without a result{}", err.lines().find(|l| !l.trim().is_empty()).map(|l| format!(": {l}")).unwrap_or_default()))?;
    let text = r["result"].as_str().unwrap_or("").trim().to_string();
    if r["is_error"] == true {
        bail!("Claude Code failed: {}", text.lines().next().unwrap_or("no reason given"));
    }
    Ok((text, r))
}

/// Projects whose new calls are due a sync: not closed; a call of theirs ended ≥ 15 min ago and
/// isn't read yet (or nothing was ever read: the backfill).
fn due() -> Vec<String> {
    let now = Local::now();
    projects::list()
        .into_iter()
        .filter(|p| !p.stage.closed())
        .filter(|p| {
            let Ok(l) = ledger::Ledger::of(&p.id) else { return false };
            let known = l.evidence();
            let processed: std::collections::BTreeSet<String> = l.events().unwrap_or_default().iter().filter(|e| e["type"] == "processed").filter_map(|e| e["evidence"].as_str().map(str::to_owned)).collect();
            let failed_recently = state(&p.id)["state"] == "failed" && state(&p.id)["at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| now.signed_duration_since(t) < chrono::Duration::hours(1));
            if failed_recently {
                return false;
            }
            // A meeting chat or a mail source due a read, or an item of theirs not read yet.
            if ledger::chat_due(p) || ledger::mail_due(p) || known.keys().any(|k| (k.starts_with("chat-") || k.starts_with("mail-")) && !processed.contains(k)) {
                return true;
            }
            projects::calls_of(p).unwrap_or_default().iter().rev().take(BACKFILL).any(|(d, _)| {
                let meta = crate::calls::read_json(&d.join("meta.json"));
                let ended = meta["ended_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
                let settled = ended.is_some_and(|t| now.signed_duration_since(t) >= SETTLE) && d.join("segments.json").is_file();
                let stamp = crate::calls::stamp_of(d);
                settled && (!known.contains_key(&stamp) || !processed.contains(&stamp))
            })
        })
        .map(|p| p.id)
        .collect()
}

/// The app's scheduler: every 5 minutes, sync the projects that are due, one at a time.
pub fn start_scheduler() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(300));
        if notes::quitting() || !crate::config::Config::load().is_ok_and(|c| c.project_sync) {
            continue;
        }
        for id in due() {
            if notes::quitting() {
                break;
            }
            if let Err(e) = run(&id) {
                eprintln!("project sync {id}: {e:#}");
            }
        }
    });
}

/// Sync a project in the background (the window's Sync now); false when one is running already.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn start(id: &str) -> bool {
    if state(id)["state"] == "running" && crate::app::FileLock::try_take(&ledger_dir(id).join(".sync.lock")).ok().flatten().is_none() {
        return false;
    }
    let id = id.to_string();
    std::thread::spawn(move || {
        if let Err(e) = run(&id) {
            eprintln!("project sync {id}: {e:#}");
        }
    });
    true
}

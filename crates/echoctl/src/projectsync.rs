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

use std::collections::BTreeMap;
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
    let claims = ledger::verify_count(id)?;
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
    // What each pass used (Claude Code's result event), kept in the state — a failed sync's too — and
    // `ledger/sync-usage.jsonl`.
    let mut used = serde_json::Map::new();
    let outcome = (|| -> Result<Value> {
        let profile_id = p.ai_profile.clone().unwrap_or_else(crate::ai::default_id);
        let ready = crate::ai::ready(&profile_id)?;
        let profile = ready.profile();
        let model = p.model.clone().or_else(|| profile.notes_model());
        let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
        let cwd = if !profile.kind.managed() { crate::ai::system_folder()? } else { paths::home() };
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
            let r = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log);
            extract_report = pass(id, &run_id, "extract", r, &mut log, &mut used)?;
        }
        // Fresh eyes: the verifier is another process, started after the extraction ended.
        let mut verify_report = String::new();
        if !ledger::verify_prep(id, false)?.is_empty() {
            let prompt = prompt(&p, "verify", &[], &run_dir);
            let r = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log);
            verify_report = pass(id, &run_id, "verify", r, &mut log, &mut used)?;
        }
        // Who's who: proposals about people and organisations from the same calls, with their words;
        // they wait for the user (`people::decide`).
        let mut people_report = String::new();
        if !for_people.is_empty() {
            let prompt = prompt(&p, "people", &for_people, &run_dir);
            let r = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log);
            people_report = match pass(id, &run_id, "people", r, &mut log, &mut used) {
                Ok(r) => {
                    mark_people_read(id, &for_people);
                    r
                }
                Err(e) => format!("people pass: {e:#}"),
            };
        }
        let _ = fs::remove_file(&settings);
        let left = ledger::pending_count(id)?;
        Ok(json!({"state": "done", "at": started.to_rfc3339(), "finished_at": Local::now().to_rfc3339(), "run": run_id,
                  "processed": before.saturating_sub(left), "left": left, "usage": used, "message": format!("{extract_report}\n{verify_report}\n{people_report}").trim().to_string()}))
    })();
    let v = match outcome {
        Ok(v) => v,
        Err(e) => {
            let signin = e.downcast_ref::<crate::ai::SignInNeeded>().map(|s| s.profile.clone());
            json!({"state": "failed", "at": started.to_rfc3339(), "message": format!("{e:#}"), "needs_sign_in": signin, "usage": used})
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
fn claude(profile: &crate::ai::Profile, model: Option<&str>, plugin: &Path, cwd: &Path, settings: &Path, prompt: &str, id: &str, run_dir: &Path, log: &mut fs::File) -> Result<(String, Value)> {
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
    crate::ai::guard_ready(profile, cwd, "sync a thread")?;
    let child = notes::admit_child(|| {
        let mut c = crate::ai::prepare_claude(profile, &a, cwd, false)?;
        c.env(guard::RUN_MODE, "project").env(guard::RUN_LEDGER, &ledger).env(guard::RUN_DIR, run_dir).env(guard::RUN_PROJECT, id).env(guard::RUN_CLIS, clis.join("\n"));
        c.env(guard::RUN_SKILL, plugin.join("skills").join("project-sync"));
        Ok(c.spawn()?)
    })?;
    let (text, result) = finish_run_result(child, log, TIMEOUT, "sync a thread", "Echo quit during the sync: it goes on next time")?;
    Ok((text, usage_of(&result)))
}

/// Each pass's usage, one line a pass, in the thread's ledger folder (`sync_stats` sums them).
const USAGE: &str = "sync-usage.jsonl";

/// What a run used, from Claude Code's result event: its tokens (cache reads and writes apart), turns, cost.
pub fn usage_of(r: &Value) -> Value {
    let u = &r["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    json!({"input": n("input_tokens"), "output": n("output_tokens"), "cache_read": n("cache_read_input_tokens"),
           "cache_write": n("cache_creation_input_tokens"), "turns": r["num_turns"].as_u64().unwrap_or(0), "cost_usd": r["total_cost_usd"].as_f64()})
}

/// All the tokens of a usage: input, output and the cache's reads and writes.
fn tokens(u: &Value) -> u64 {
    ["input", "output", "cache_read", "cache_write"].iter().map(|k| u[k].as_u64().unwrap_or(0)).sum()
}

/// A pass's outcome into its usage records: `Ok` as it is; a failure with what it used when Claude Code's result event
/// came with it (`RunFailed`), else with its usage unknown (null) — then the failure goes on.
fn pass(id: &str, run: &str, name: &str, r: Result<(String, Value)>, log: &mut fs::File, used: &mut serde_json::Map<String, Value>) -> Result<String> {
    match r {
        Ok((text, u)) => {
            record_usage(id, run, name, &u, None, log, used);
            Ok(text)
        }
        Err(e) => {
            let u = e.downcast_ref::<RunFailed>().map(|f| usage_of(&f.result)).unwrap_or(Value::Null);
            record_usage(id, run, name, &u, Some(&format!("{e:#}")), log, used);
            Err(e)
        }
    }
}

fn record_usage(id: &str, run: &str, pass: &str, u: &Value, error: Option<&str>, log: &mut fs::File, used: &mut serde_json::Map<String, Value>) {
    let now = Local::now();
    let what = if u.is_null() {
        "usage unknown".to_string()
    } else {
        format!("{} tokens (input {}, output {}, cache read {}, cache write {}), {} turns", tokens(u), u["input"], u["output"], u["cache_read"], u["cache_write"], u["turns"])
    };
    let _ = writeln!(log, "{}  {pass}: {what}{}", now.format("%H:%M:%S"), error.map(|e| format!(" — failed: {e}")).unwrap_or_default());
    let mut kept = if u.is_null() { json!({}) } else { u.clone() };
    kept["ok"] = json!(error.is_none());
    if let Some(e) = error {
        kept["error"] = json!(e);
    }
    used.insert(pass.into(), kept);
    let mut line = json!({"at": now.to_rfc3339(), "run": run, "pass": pass, "usage": u, "ok": error.is_none()});
    if let Some(e) = error {
        line["error"] = json!(e);
    }
    // One write of the whole line, as the ledger appends its events.
    let appended = fs::OpenOptions::new().create(true).append(true).open(ledger_dir(id).join(USAGE)).and_then(|mut f| f.write_all(format!("{line}\n").as_bytes()));
    if let Err(e) = appended {
        let _ = writeln!(log, "{}  the usage couldn't be kept: {e}", now.format("%H:%M:%S"));
    }
}

/// Every thread's sync usage over the last `days` days (today included), summed per day and pass, newest day first:
/// `{days: [{day, runs, tokens, passes: {<pass>: {runs, failed, tokens, input, output, cache_read, cache_write, turns,
/// cost_usd, cost_runs}}}], total}`. A run whose usage is unknown counts as a run, with no tokens; `cost_usd` sums the
/// `cost_runs` that gave one. Only the threads there are now: a deleted thread's usage leaves with it.
pub fn sync_stats(days: u32) -> Value {
    let today = Local::now().date_naive();
    let from = today.checked_sub_days(chrono::Days::new(days.saturating_sub(1).into())).unwrap_or(chrono::NaiveDate::MIN);
    let mut by_day: BTreeMap<chrono::NaiveDate, BTreeMap<String, Value>> = Default::default();
    let add = |sum: &mut Value, v: &Value| {
        let u = &v["usage"];
        sum["runs"] = json!(sum["runs"].as_u64().unwrap_or(0) + 1);
        sum["failed"] = json!(sum["failed"].as_u64().unwrap_or(0) + u64::from(v["ok"] == false));
        sum["tokens"] = json!(sum["tokens"].as_u64().unwrap_or(0) + tokens(u));
        for k in ["input", "output", "cache_read", "cache_write", "turns"] {
            sum[k] = json!(sum[k].as_u64().unwrap_or(0) + u[k].as_u64().unwrap_or(0));
        }
        if let Some(c) = u["cost_usd"].as_f64() {
            sum["cost_usd"] = json!(sum["cost_usd"].as_f64().unwrap_or(0.0) + c);
            sum["cost_runs"] = json!(sum["cost_runs"].as_u64().unwrap_or(0) + 1);
        }
    };
    let mut total = json!({});
    for p in projects::list() {
        let Ok(text) = fs::read_to_string(ledger_dir(&p.id).join(USAGE)) else { continue };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let (Some(at), Some(pass)) = (v["at"].as_str(), v["pass"].as_str()) else { continue };
            let Ok(at) = chrono::DateTime::parse_from_rfc3339(at) else { continue };
            let day = at.with_timezone(&Local).date_naive();
            if day < from || day > today {
                continue;
            }
            add(by_day.entry(day).or_default().entry(pass.to_string()).or_insert_with(|| json!({})), &v);
            add(&mut total, &v);
        }
    }
    let days: Vec<Value> = by_day
        .into_iter()
        .rev()
        .map(|(day, passes)| {
            let (runs, tokens): (u64, u64) = passes.values().fold((0, 0), |(r, t), v| (r + v["runs"].as_u64().unwrap_or(0), t + v["tokens"].as_u64().unwrap_or(0)));
            json!({"day": day.format("%Y-%m-%d").to_string(), "runs": runs, "tokens": tokens, "passes": passes})
        })
        .collect();
    json!({"days": days, "total": total})
}

/// Claude Code ended with an error result (`is_error`, its turns used up among them): the result event, with what the
/// run used, goes with the failure.
#[derive(Debug)]
pub struct RunFailed {
    pub message: String,
    pub result: Value,
}

impl std::fmt::Display for RunFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RunFailed {}

/// Follow a headless Claude Code run started with `--output-format stream-json`: its steps into `log`,
/// stopped after `timeout` or when it is too old for the guard (`what` it was for), and its final
/// answer with Claude Code's result event (its usage, cost, turns) — or why there is none. Shared by the project sync
/// and routines.
pub fn finish_run_result(mut child: std::process::Child, log: &mut fs::File, timeout: Duration, what: &str, quit: &str) -> Result<(String, Value)> {
    let pid = child.id();
    let err = child.stderr.take().map(|e| std::thread::spawn(move || BufReader::new(e).lines().map_while(Result::ok).collect::<Vec<_>>().join("\n")));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // Set when the time limit stopped it: then that is why it ended, not what Claude Code printed.
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let (done, timed_out) = (done.clone(), timed_out.clone());
        std::thread::spawn(move || {
            let t = Instant::now();
            while t.elapsed() < timeout {
                if done.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            timed_out.store(true, std::sync::atomic::Ordering::SeqCst);
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
            for l in crate::run_log::describe(&ev) {
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
    if result.is_none() && timed_out.load(std::sync::atomic::Ordering::SeqCst) {
        let s = timeout.as_secs();
        let limit = if s >= 60 && s.is_multiple_of(60) { format!("{} min", s / 60) } else { format!("{s} s") };
        bail!("stopped after {limit} without finishing (its time limit); its steps are in the run's log");
    }
    let r = result.ok_or_else(|| anyhow!("Claude Code ended without a result{}", err.lines().find(|l| !l.trim().is_empty()).map(|l| format!(": {l}")).unwrap_or_default()))?;
    let text = r["result"].as_str().unwrap_or("").trim().to_string();
    if r["is_error"] == true {
        return Err(RunFailed { message: format!("Claude Code failed: {}", text.lines().next().unwrap_or("no reason given")), result: r }.into());
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A usage line at `at`, as `record_usage` writes one.
    fn usage_line(id: &str, at: chrono::DateTime<Local>, pass: &str, input: u64, read: u64, turns: u64, ok: bool) {
        let u = json!({"input": input, "output": 100, "cache_read": read, "cache_write": 1000, "turns": turns, "cost_usd": if ok { json!(0.5) } else { Value::Null }});
        let line = json!({"at": at.to_rfc3339(), "run": "r", "pass": pass, "usage": u, "ok": ok});
        fs::create_dir_all(ledger_dir(id)).unwrap();
        fs::OpenOptions::new().create(true).append(true).open(ledger_dir(id).join(USAGE)).and_then(|mut f| writeln!(f, "{line}")).unwrap();
    }

    #[test]
    fn sync_stats_sums_every_threads_usage_per_day_and_pass() {
        let (_env, _root) = crate::paths::testenv::folders("sync-stats");
        let a = projects::create("Acme pilot", None, None).unwrap().id;
        let b = projects::create("Globex rollout", None, None).unwrap().id;
        // Fixed times: today's noon and three days before it, whenever the test runs.
        let noon = |days_back: u64| {
            let d = Local::now().date_naive().checked_sub_days(chrono::Days::new(days_back)).unwrap();
            d.and_hms_opt(12, 0, 0).unwrap().and_local_timezone(Local).earliest().unwrap()
        };
        usage_line(&a, noon(0), "extract", 10, 5000, 7, true);
        usage_line(&a, noon(0), "verify", 20, 3000, 5, true);
        usage_line(&b, noon(0), "extract", 30, 4000, 6, false);
        usage_line(&b, noon(3), "people", 1, 1, 1, true);
        fs::OpenOptions::new().append(true).open(ledger_dir(&b).join(USAGE)).and_then(|mut f| writeln!(f, "not json")).unwrap();
        let today = sync_stats(1);
        let days = today["days"].as_array().unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0]["runs"], 3);
        assert_eq!(days[0]["tokens"], 6110 + 4120 + 5130);
        let extract = &days[0]["passes"]["extract"];
        assert_eq!((extract["runs"].as_u64(), extract["failed"].as_u64(), extract["cache_read"].as_u64(), extract["turns"].as_u64()), (Some(2), Some(1), Some(9000), Some(13)));
        assert_eq!((extract["cost_usd"].as_f64(), extract["cost_runs"].as_u64()), (Some(0.5), Some(1)), "the cost of the runs that gave one");
        assert_eq!(days[0]["passes"]["verify"]["input"], 20);
        let week = sync_stats(7);
        assert_eq!(week["days"].as_array().unwrap().len(), 2, "the older day too, newest first");
        assert_eq!(week["days"][1]["passes"]["people"]["runs"], 1);
        assert_eq!(sync_stats(u32::MAX)["days"].as_array().unwrap().len(), 2, "any number of days");
    }

    #[test]
    fn a_pass_is_logged_kept_in_the_state_and_its_usage_line_written_whole() {
        let (_env, root) = crate::paths::testenv::folders("sync-usage");
        let a = projects::create("Acme pilot", None, None).unwrap().id;
        fs::create_dir_all(ledger_dir(&a)).unwrap();
        let mut log = fs::File::create(root.join("log.txt")).unwrap();
        let mut used = serde_json::Map::new();
        let r = json!({"num_turns": 7, "usage": {"input_tokens": 10, "output_tokens": 100, "cache_read_input_tokens": 5000, "cache_creation_input_tokens": 1000}});
        assert_eq!(pass(&a, "run-1", "extract", Ok(("done".into(), usage_of(&r))), &mut log, &mut used).unwrap(), "done");
        assert!(fs::read_to_string(root.join("log.txt")).unwrap().contains("extract: 6110 tokens (input 10, output 100, cache read 5000, cache write 1000), 7 turns"));
        assert_eq!((used["extract"]["cache_read"].as_u64(), used["extract"]["ok"].as_bool()), (Some(5000), Some(true)));
        let line: Value = serde_json::from_str(fs::read_to_string(ledger_dir(&a).join(USAGE)).unwrap().lines().next().unwrap()).unwrap();
        assert_eq!((line["run"].as_str(), line["pass"].as_str(), line["ok"].as_bool(), line["usage"]["turns"].as_u64()), (Some("run-1"), Some("extract"), Some(true), Some(7)));
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_pass_keeps_what_it_used_or_says_its_usage_is_unknown() {
        let (_env, root) = crate::paths::testenv::folders("sync-usage-failed");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("u", None, &crate::ai::fake::signed_in("pat@example.com"));
        let id = projects::create("Acme pilot", None, None).unwrap().id;
        fs::create_dir_all(ledger_dir(&id)).unwrap();
        // Out of turns: an error result, with all it used.
        let init = json!({"type": "system", "subtype": "init", "claude_code_version": "9.0.0"});
        let result = json!({"type": "result", "subtype": "error_max_turns", "result": "turns used up", "is_error": true, "num_turns": 40, "total_cost_usd": 2.5,
                            "usage": {"input_tokens": 900, "output_tokens": 80, "cache_read_input_tokens": 300000, "cache_creation_input_tokens": 20000}});
        fs::write(crate::ai::fake::file(&p, "p.reply"), format!("{init}\n{result}\n")).unwrap();
        let child = crate::ai::prepare_claude(&p, &["-p".into(), "x".into()], &root, false).unwrap().spawn().unwrap();
        let mut log = fs::File::create(root.join("log.txt")).unwrap();
        let r = finish_run_result(child, &mut log, Duration::from_secs(30), "sync a thread", "quit").map(|(t, r)| (t, usage_of(&r)));
        let mut used = serde_json::Map::new();
        let e = pass(&id, "run-1", "extract", r, &mut log, &mut used).unwrap_err().to_string();
        assert_eq!(e, "Claude Code failed: turns used up", "the failure goes on as before");
        assert_eq!((used["extract"]["cache_read"].as_u64(), used["extract"]["turns"].as_u64(), used["extract"]["ok"].as_bool()), (Some(300000), Some(40), Some(false)));
        // Stopped with no result event: a failed run whose usage is unknown.
        let e = pass(&id, "run-1", "verify", Err(anyhow!("stopped after 40 min without finishing")), &mut log, &mut used).unwrap_err();
        assert!(e.to_string().starts_with("stopped after"));
        assert_eq!(used["verify"]["ok"], false);
        assert!(used["verify"].get("input").is_none());
        let text = fs::read_to_string(root.join("log.txt")).unwrap();
        assert!(text.contains("extract: 320980 tokens (input 900, output 80, cache read 300000, cache write 20000), 40 turns — failed: Claude Code failed: turns used up"), "{text}");
        assert!(text.contains("verify: usage unknown — failed: stopped after 40 min"), "{text}");
        let stats = sync_stats(1);
        let passes = &stats["days"][0]["passes"];
        assert_eq!((passes["extract"]["failed"].as_u64(), passes["extract"]["tokens"].as_u64(), passes["extract"]["cost_usd"].as_f64()), (Some(1), Some(320980), Some(2.5)));
        assert_eq!((passes["verify"]["runs"].as_u64(), passes["verify"]["failed"].as_u64(), passes["verify"]["tokens"].as_u64()), (Some(1), Some(1), Some(0)));
    }

    #[cfg(unix)]
    #[test]
    fn a_runs_usage_comes_from_claude_codes_result_event() {
        let (_env, root) = crate::paths::testenv::folders("run-usage");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("u", None, &crate::ai::fake::signed_in("pat@example.com"));
        let init = json!({"type": "system", "subtype": "init", "claude_code_version": "9.0.0"});
        let result = json!({"type": "result", "result": "done", "is_error": false, "num_turns": 3,
                            "usage": {"input_tokens": 12, "output_tokens": 34, "cache_read_input_tokens": 5600, "cache_creation_input_tokens": 780}});
        fs::write(crate::ai::fake::file(&p, "p.reply"), format!("{init}\n{result}\n")).unwrap();
        let child = crate::ai::prepare_claude(&p, &["-p".into(), "x".into()], &root, false).unwrap().spawn().unwrap();
        let mut log = fs::File::create(root.join("log.txt")).unwrap();
        let (_, r) = finish_run_result(child, &mut log, Duration::from_secs(30), "sync a thread", "quit").unwrap();
        let u = usage_of(&r);
        assert_eq!((u["input"].as_u64(), u["output"].as_u64(), u["cache_read"].as_u64(), u["cache_write"].as_u64(), u["turns"].as_u64()), (Some(12), Some(34), Some(5600), Some(780), Some(3)));
        assert_eq!(tokens(&u), 12 + 34 + 5600 + 780);
        assert!(u["cost_usd"].is_null(), "no cost given: none made up");
    }

    #[cfg(unix)]
    #[test]
    fn a_run_past_its_time_limit_says_it_ran_out_of_time_not_its_first_stderr_line() {
        let (_env, root) = crate::paths::testenv::folders("run-time-limit");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("slow", None, &crate::ai::fake::signed_in("pat@example.com"));
        fs::write(crate::ai::fake::file(&p, "p.hang"), "[claude-code:unrecognized_model] {\"model\":\"qwen3.8:27b\"}\n").unwrap();
        let child = crate::ai::prepare_claude(&p, &["-p".into(), "x".into()], &root, false).unwrap().spawn().unwrap();
        let mut log = fs::File::create(root.join("log.txt")).unwrap();
        let e = finish_run_result(child, &mut log, Duration::from_secs(1), "sync a thread", "quit").unwrap_err().to_string();
        assert!(e.contains("stopped after 1 s without finishing"), "{e}");
        assert!(!e.contains("unrecognized_model"), "the warning stays in the log: {e}");
        assert!(fs::read_to_string(root.join("log.txt")).unwrap().contains("unrecognized_model"));
    }

    /// #215: routines' and plugins' runs (`claudejob::run`) finish here, so a Claude Code older than
    /// `guard::MIN_CLAUDE` is stopped for them too — even when it goes on to answer.
    #[cfg(unix)]
    #[test]
    fn a_run_finished_here_on_a_claude_code_too_old_for_the_guard_is_stopped() {
        let (_env, root) = crate::paths::testenv::folders("run-too-old");
        let _fake = crate::ai::fake::install(&root);
        let p = crate::ai::fake::profile("old", None, &crate::ai::fake::signed_in("pat@example.com"));
        let run = |version: &str| {
            let init = serde_json::json!({"type": "system", "subtype": "init", "claude_code_version": version});
            let result = serde_json::json!({"type": "result", "result": "the answer", "is_error": false});
            fs::write(crate::ai::fake::file(&p, "p.reply"), format!("{init}\n{result}\n")).unwrap();
            let child = crate::ai::prepare_claude(&p, &["-p".into(), "x".into()], &root, false).unwrap().spawn().unwrap();
            let mut log = fs::File::create(root.join("log.txt")).unwrap();
            finish_run_result(child, &mut log, Duration::from_secs(30), "run a plugin's job", "quit").map(|(text, _)| text)
        };
        let e = run("2.1.200").unwrap_err().to_string();
        assert!(e.contains("too old to run a plugin's job"), "{e}");
        let [x, y, z] = guard::MIN_CLAUDE;
        assert_eq!(run(&format!("{x}.{y}.{z}")).unwrap(), "the answer");
        assert!(run("").unwrap_err().to_string().contains("too old"), "an unknown version is stopped too");
    }
}

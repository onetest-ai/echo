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
/// Sync now (the window, the CLI): everything waiting is read, mail and chat too.
pub fn run(id: &str) -> Result<Value> {
    run_with(id, false)
}

/// As the app's scheduler syncs (`echoctl project ledger <id> run --scheduled`): mail and chat wait for their batch.
pub fn run_scheduled(id: &str) -> Result<Value> {
    run_with(id, true)
}

/// Mail and chat wait for one another: a thread's sync reads them once the first has waited `BATCH_WAIT`, or
/// `BATCH_ITEMS` wait, or with a call; calls keep their own settle time (`SETTLE`).
const BATCH_WAIT: chrono::Duration = chrono::Duration::hours(2);
const BATCH_ITEMS: usize = 5;

/// When each mail or chat item still waiting was first seen waiting (`ledger/waiting.json`): a chat read again
/// once it changed waits from then, not from its first message.
const WAITING: &str = "waiting.json";

fn mail_or_chat(id: &str) -> bool {
    id.starts_with("mail-") || id.starts_with("chat-")
}

/// The thread's mail and chat items waiting to be read, each with when it was first seen waiting (kept up to date
/// in `ledger/waiting.json`), oldest first.
fn waiting(id: &str, now: chrono::DateTime<Local>) -> Vec<(String, chrono::DateTime<Local>)> {
    let Ok(pending) = ledger::pending(id) else { return vec![] };
    let file = ledger_dir(id).join(WAITING);
    let known: BTreeMap<String, String> = fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let mut now_waiting: BTreeMap<String, String> = BTreeMap::new();
    for i in pending.iter().filter_map(|x| x["id"].as_str()).filter(|i| mail_or_chat(i)) {
        now_waiting.insert(i.to_string(), known.get(i).cloned().unwrap_or_else(|| now.to_rfc3339()));
    }
    if now_waiting != known {
        let _ = paths::write_atomic(&file, &serde_json::to_vec(&now_waiting).unwrap_or_default());
    }
    let mut v: Vec<(String, chrono::DateTime<Local>)> =
        now_waiting.into_iter().map(|(i, t)| (i, chrono::DateTime::parse_from_rfc3339(&t).map(|t| t.with_timezone(&Local)).unwrap_or(now))).collect();
    v.sort_by_key(|(_, t)| *t);
    v
}

/// Mail and chat are due a read: the first has waited `BATCH_WAIT`, or `BATCH_ITEMS` wait.
fn batch_ready(w: &[(String, chrono::DateTime<Local>)], now: chrono::DateTime<Local>) -> bool {
    w.len() >= BATCH_ITEMS || w.first().is_some_and(|(_, t)| now.signed_duration_since(*t) >= BATCH_WAIT)
}

/// A sync; `batch`: mail and chat wait for their batch unless a call is read anyway.
fn run_with(id: &str, batch: bool) -> Result<Value> {
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
    let mut pending: Vec<String> = backlog.into_iter().filter_map(|x| x["id"].as_str().map(str::to_owned)).collect();
    // Mail and chat held for their batch, unless a call is read now anyway (they go with it).
    let mut held = None;
    if batch && pending.iter().all(|i| mail_or_chat(i)) {
        let w = waiting(id, started);
        if !batch_ready(&w, started) {
            held = w.first().map(|(_, t)| (w.len(), *t + BATCH_WAIT));
            pending.clear();
        }
    }
    pending.truncate(MAX_ITEMS);
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
        let message = match held {
            Some((n, at)) => format!("{n} mail or chat item{} wait{}: read at {} or once {BATCH_ITEMS} wait", if n == 1 { "" } else { "s" }, if n == 1 { "s" } else { "" }, at.format("%H:%M")),
            None => "nothing new".to_string(),
        };
        let v = json!({"state": "done", "at": started.to_rfc3339(), "message": message, "written": written.len(), "processed": 0});
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
        let cwd = if !profile.kind.managed() { crate::ai::system_folder()? } else { paths::home() };
        crate::ai::hooks_conflict(profile, &cwd).map_err(|w| anyhow!(w))?;
        let run_id = started.format("%Y%m%d-%H%M%S").to_string();
        let run_dir = ledger_dir(id).join("runs").join(&run_id);
        fs::create_dir_all(&run_dir)?;
        let run_dir = dunce::canonicalize(&run_dir)?;
        let settings = settings_file(&run_id)?;
        let mut log = fs::File::create(run_dir.join("log.txt"))?;
        let _ = writeln!(log, "{}  Echo syncs {} ({}) on {}{}", Local::now().format("%H:%M:%S"), p.name, id, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default());
        // What each pass used (Claude Code's result event), kept in the state and `ledger/sync-usage.jsonl`.
        let mut used = serde_json::Map::new();
        let mut extract_report = String::new();
        if !pending.is_empty() {
            let prompt = prompt(&p, "extract", &pending, &run_dir);
            let (r, u) = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log)?;
            extract_report = r;
            record_usage(id, &run_id, "extract", &u, &mut log, &mut used);
        }
        // Fresh eyes: the verifier is another process, started after the extraction ended.
        let mut verify_report = String::new();
        if !ledger::verify_prep(id, false)?.is_empty() {
            let prompt = prompt(&p, "verify", &[], &run_dir);
            let (r, u) = claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log)?;
            verify_report = r;
            record_usage(id, &run_id, "verify", &u, &mut log, &mut used);
        }
        // Who's who: proposals about people and organisations from the same calls, with their words;
        // they wait for the user (`people::decide`).
        let mut people_report = String::new();
        if !for_people.is_empty() {
            let prompt = prompt(&p, "people", &for_people, &run_dir);
            people_report = match claude(profile, model.as_deref(), &plugin, &cwd, &settings, &prompt, id, &run_dir, &mut log) {
                Ok((r, u)) => {
                    mark_people_read(id, &for_people);
                    record_usage(id, &run_id, "people", &u, &mut log, &mut used);
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

fn record_usage(id: &str, run: &str, pass: &str, u: &Value, log: &mut fs::File, used: &mut serde_json::Map<String, Value>) {
    let _ = writeln!(
        log,
        "{}  {pass}: {} tokens (input {}, output {}, cache read {}, cache write {}), {} turns",
        Local::now().format("%H:%M:%S"),
        tokens(u),
        u["input"],
        u["output"],
        u["cache_read"],
        u["cache_write"],
        u["turns"]
    );
    used.insert(pass.into(), u.clone());
    let line = json!({"at": Local::now().to_rfc3339(), "run": run, "pass": pass, "usage": u});
    let appended = fs::OpenOptions::new().create(true).append(true).open(ledger_dir(id).join(USAGE)).and_then(|mut f| writeln!(f, "{line}"));
    if let Err(e) = appended {
        let _ = writeln!(log, "{}  the usage couldn't be kept: {e}", Local::now().format("%H:%M:%S"));
    }
}

/// Every thread's sync usage over the last `days` days (today included), summed per day and pass, newest day first:
/// `{days: [{day, passes: {extract: {runs, tokens, input, output, cache_read, cache_write, turns, cost_usd}, …}, runs, tokens}], total}`.
pub fn sync_stats(days: u32) -> Value {
    let from = (Local::now() - chrono::Duration::days(days.saturating_sub(1).into())).format("%Y-%m-%d").to_string();
    let mut by_day: std::collections::BTreeMap<String, std::collections::BTreeMap<String, Value>> = Default::default();
    let add = |sum: &mut Value, u: &Value| {
        sum["runs"] = json!(sum["runs"].as_u64().unwrap_or(0) + 1);
        sum["tokens"] = json!(sum["tokens"].as_u64().unwrap_or(0) + tokens(u));
        for k in ["input", "output", "cache_read", "cache_write", "turns"] {
            sum[k] = json!(sum[k].as_u64().unwrap_or(0) + u[k].as_u64().unwrap_or(0));
        }
        if let Some(c) = u["cost_usd"].as_f64() {
            sum["cost_usd"] = json!(sum["cost_usd"].as_f64().unwrap_or(0.0) + c);
        }
    };
    let mut total = json!({});
    for p in projects::list() {
        let Ok(text) = fs::read_to_string(ledger_dir(&p.id).join(USAGE)) else { continue };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            let (Some(at), Some(pass)) = (v["at"].as_str(), v["pass"].as_str()) else { continue };
            let Ok(at) = chrono::DateTime::parse_from_rfc3339(at) else { continue };
            let day = at.with_timezone(&Local).format("%Y-%m-%d").to_string();
            if day < from {
                continue;
            }
            add(by_day.entry(day).or_default().entry(pass.to_string()).or_insert_with(|| json!({})), &v["usage"]);
            add(&mut total, &v["usage"]);
        }
    }
    let days: Vec<Value> = by_day
        .into_iter()
        .rev()
        .map(|(day, passes)| {
            let (runs, tokens): (u64, u64) = passes.values().fold((0, 0), |(r, t), v| (r + v["runs"].as_u64().unwrap_or(0), t + v["tokens"].as_u64().unwrap_or(0)));
            json!({"day": day, "runs": runs, "tokens": tokens, "passes": passes})
        })
        .collect();
    json!({"days": days, "total": total})
}

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
            // A meeting chat or a mail source due a fetch (no model run unless its batch is ready), or mail and chat
            // whose batch is ready.
            if ledger::chat_due(p) || ledger::mail_due(p) || (known.keys().any(|k| mail_or_chat(k) && !processed.contains(k)) && batch_ready(&waiting(&p.id, now), now)) {
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
            if let Err(e) = run_scheduled(&id) {
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

    #[test]
    fn each_passs_usage_is_kept_and_summed_per_day_and_pass_over_every_thread() {
        let (_env, root) = crate::paths::testenv::folders("sync-usage");
        let a = projects::create("Acme pilot", None, None).unwrap().id;
        let b = projects::create("Globex rollout", None, None).unwrap().id;
        let result = |input: u64, read: u64, turns: u64| {
            json!({"type": "result", "num_turns": turns, "total_cost_usd": 0.5,
                   "usage": {"input_tokens": input, "output_tokens": 100, "cache_read_input_tokens": read, "cache_creation_input_tokens": 1000}})
        };
        let mut log = fs::File::create(root.join("log.txt")).unwrap();
        let mut used = serde_json::Map::new();
        for (id, pass, r) in [(&a, "extract", result(10, 5000, 7)), (&a, "verify", result(20, 3000, 5)), (&b, "extract", result(30, 4000, 6))] {
            fs::create_dir_all(ledger_dir(id)).unwrap();
            record_usage(id, "20261116-090000", pass, &usage_of(&r), &mut log, &mut used);
        }
        // An older day, outside a one-day report.
        let old = json!({"at": (Local::now() - chrono::Duration::days(3)).to_rfc3339(), "run": "x", "pass": "people", "usage": usage_of(&result(1, 1, 1))});
        fs::OpenOptions::new().append(true).open(ledger_dir(&b).join(USAGE)).and_then(|mut f| writeln!(f, "{old}\nnot json")).unwrap();

        assert_eq!(used["extract"]["cache_read"], 4000, "the state keeps each pass's last usage");
        assert!(fs::read_to_string(root.join("log.txt")).unwrap().contains("extract: 6110 tokens (input 10, output 100, cache read 5000, cache write 1000), 7 turns"));
        let today = sync_stats(1);
        let days = today["days"].as_array().unwrap();
        assert_eq!(days.len(), 1);
        assert_eq!(days[0]["runs"], 3);
        assert_eq!(days[0]["tokens"], 6110 + 4120 + 5130);
        let extract = &days[0]["passes"]["extract"];
        assert_eq!((extract["runs"].as_u64(), extract["cache_read"].as_u64(), extract["turns"].as_u64()), (Some(2), Some(9000), Some(13)));
        assert_eq!(days[0]["passes"]["verify"]["input"], 20);
        assert_eq!(today["total"]["cost_usd"], 1.5);
        let week = sync_stats(7);
        assert_eq!(week["days"].as_array().unwrap().len(), 2, "the older day too, newest first");
        assert_eq!(week["days"][1]["passes"]["people"]["runs"], 1);
    }

    /// A mail or chat item of `project`, as `mail_sync` / `chat_sync` write one.
    fn mail_item(project: &str, id: &str) {
        let dir = ledger_dir(project).join("evidence");
        fs::create_dir_all(&dir).unwrap();
        let at = Local::now() - chrono::Duration::days(1);
        fs::write(dir.join(format!("{id}.json")), json!({"id": id, "at": at.to_rfc3339(), "date": at.date_naive().to_string(), "title": "Acme rollout status"}).to_string()).unwrap();
        fs::write(dir.join(format!("{id}.md")), "Test mail, nothing real.").unwrap();
    }

    #[test]
    fn mail_and_chat_are_due_once_the_first_has_waited_two_hours_or_five_wait() {
        let now = Local::now();
        let w = |n: usize, mins: i64| (0..n).map(|i| (format!("mail-{i}"), now - chrono::Duration::minutes(mins))).collect::<Vec<_>>();
        assert!(!batch_ready(&[], now));
        assert!(!batch_ready(&w(4, 119), now), "four items, under two hours");
        assert!(batch_ready(&w(5, 0), now), "five wait");
        assert!(batch_ready(&w(1, 120), now), "the first has waited two hours");
    }

    #[test]
    fn an_item_waits_from_when_it_was_first_seen_waiting_and_leaves_once_read() {
        let (_env, _root) = crate::paths::testenv::folders("sync-waiting");
        let id = projects::create("Acme rollout", None, None).unwrap().id;
        mail_item(&id, "mail-a");
        let t0 = Local::now() - chrono::Duration::hours(3);
        let first = waiting(&id, t0);
        assert_eq!(first.len(), 1);
        // Seen again later: still waiting since then (its own sent time, a day ago, doesn't count).
        mail_item(&id, "mail-b");
        let later = waiting(&id, Local::now());
        assert_eq!(later.iter().map(|(i, _)| i.as_str()).collect::<Vec<_>>(), ["mail-a", "mail-b"]);
        assert_eq!(later[0].1.timestamp(), t0.timestamp());
        assert!(batch_ready(&later, Local::now()), "mail-a has waited three hours");
        ledger::mark_processed(&id, "mail-a", "read").unwrap();
        let after = waiting(&id, Local::now());
        assert_eq!(after.iter().map(|(i, _)| i.as_str()).collect::<Vec<_>>(), ["mail-b"], "read: no longer waiting");
        assert!(!batch_ready(&after, Local::now()));
    }

    #[test]
    fn the_scheduler_holds_a_lone_new_mail_and_sync_now_reads_it() {
        let (_env, _root) = crate::paths::testenv::folders("sync-batch");
        let id = projects::create("Acme rollout", None, None).unwrap().id;
        mail_item(&id, "mail-a");
        assert!(!due().contains(&id), "one mail, just seen: not due");
        let v = run_with(&id, true).unwrap();
        assert_eq!(v["state"], "done");
        assert_eq!(v["processed"], 0);
        assert!(v["message"].as_str().unwrap().starts_with("1 mail or chat item waits: read at "), "{v}");
        for i in ["mail-b", "mail-c", "mail-d", "mail-e"] {
            mail_item(&id, i);
        }
        assert!(due().contains(&id), "five wait");
        // Sync now doesn't wait: it goes to the model (none in a test: it fails trying).
        let one = projects::create("Globex pilot", None, None).unwrap().id;
        mail_item(&one, "mail-z");
        let e = run(&one).unwrap_err().to_string();
        assert!(!e.contains("wait"), "{e}");
        assert_eq!(state(&one)["state"], "failed");
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

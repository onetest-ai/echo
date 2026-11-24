//! The minutes run's guard (`echoctl run-guard`, hidden): a `PreToolUse` hook Claude Code runs
//! before every tool call of a minutes run Echo started (`notes.rs` passes it in the run's own
//! settings file). A run follows other people's words unattended, so whatever its model is talked
//! into, it may only:
//! - run the Echo commands the minutes need (`notes::RUN_COMMANDS`), as plain commands — no
//!   variables, redirects, pipes or command lists;
//! - read files in its own call's folder, and the previous minutes of the same meeting;
//! - write and edit only what the minutes are: `notes.md` and its new action list
//!   (`actions.new*.json`) in that folder — not Echo's own files of the call (`meta.json` would pin
//!   it to another profile for later runs), the user's notes, or an instruction file Claude Code
//!   would load by itself (`CLAUDE.md`, `AGENTS.md`, `.claude/`: its `@imports` would load other
//!   files with no tool call), nor anything through a link.
//!
//! Anything else is refused (exit 2, the reason on stderr), and so is anything it can't judge.
//! Claude Code runs the hook before every tool call, also one no permission is needed for, and a
//! refusal wins over any other hook or rule (code.claude.com/docs/en/hooks); `"onFailure":
//! "block"` refuses a call whose guard didn't run (Claude Code ≥ 2.1.295, `MIN_CLAUDE`). The
//! run's call, its previous minutes and the CLI's names come only from the environment Echo set
//! for the run, never from the tool call (a settings `env` could set them too: only the user's or
//! an admin's, which the run can't write). Measured with Claude Code 2.1.310: a `cat` of another call that an
//! allow rule permitted, and a Read inside the working folder, were both refused.
use serde_json::{json, Value};
use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

/// The run's call folder (absolute), set by the app for the run's Claude Code.
pub const RUN_CALL: &str = "ECHOCTL_RUN_CALL";
/// The previous minutes of the same meeting (`notes.md` of another call), if any.
pub const RUN_PREVIOUS: &str = "ECHOCTL_RUN_PREVIOUS";
/// The names the run may use for the CLI, one per line.
pub const RUN_CLIS: &str = "ECHOCTL_RUN_CLIS";
/// `ask`: the Echo window's Claude session on a call — it reads, it changes nothing (no Edit,
/// Write or skill; only `ASK_COMMANDS`). Unset: a minutes run.
pub const RUN_MODE: &str = "ECHOCTL_RUN_MODE";
/// The Echo commands a call's Claude session may run (read-only).
pub const ASK_COMMANDS: [&str; 1] = ["context"];
/// `RUN_MODE=project`: a project's sync run (`projectsync.rs`) — its ledger folder (reads), the run's
/// own folder in it (its proposal and verdict files), the project's id (its ledger commands).
pub const RUN_LEDGER: &str = "ECHOCTL_RUN_LEDGER";
pub const RUN_DIR: &str = "ECHOCTL_RUN_DIR";
pub const RUN_PROJECT: &str = "ECHOCTL_RUN_PROJECT";
/// The evidence items a sync run may mark done (`done`, `apply --done`), one id a line: those Echo gave it for the work.
pub const RUN_ITEMS: &str = "ECHOCTL_RUN_ITEMS";
/// Yolo's run (`claudejob`): the one task it may edit, `<project>/<node>` (`echoctl project ledger <project> edit`).
pub const RUN_TASK: &str = "ECHOCTL_RUN_TASK";
/// Yolo's run: the AI profile it was started on (resolved at launch), so `echoctl read` sends it only that profile's
/// calls even if the thread is re-pinned while it runs.
pub const RUN_PROFILE: &str = "ECHOCTL_RUN_PROFILE";
/// Yolo's run: the thread it works in (a task's, or an item of Needs you now's), whose data it may read — its brief,
/// people, work and calls (`run_thread`); nothing of it changes but its own task's state.
pub const RUN_THREAD: &str = "ECHOCTL_RUN_THREAD";

/// The thread a run Echo started may read (Yolo's: `RUN_THREAD`, else its task's); none for any other run.
pub fn run_thread() -> Option<String> {
    let t = std::env::var(RUN_THREAD).ok().filter(|t| !t.is_empty());
    t.or_else(|| std::env::var(RUN_TASK).ok().and_then(|t| t.split_once('/').map(|(p, _)| p.to_string())).filter(|t| !t.is_empty()))
}
/// The Brain's MCP tools, as Claude Code names them (`mcp__<server>__<tool>`; all read-only).
pub const BRAIN_TOOLS: &str = "mcp__echo__";

/// The ledger steps a sync run may take (`echoctl project ledger <id> <step>`): its proposals and its items marked done.
/// What it reads Echo gives it in its prompt (`projectsync`); the verifier runs with no tools.
pub const PROJECT_STEPS: [&str; 3] = ["apply", "done", "people-apply"];
/// The oldest Claude Code whose hooks block when they fail (`onFailure`).
pub const MIN_CLAUDE: [u32; 3] = [2, 1, 295];

/// What a run may touch.
#[derive(Debug)]
pub struct Scope {
    call: PathBuf,
    previous: Option<PathBuf>,
    clis: Vec<String>,
    /// The window's session on the call (`RUN_MODE=ask`): read-only.
    ask: bool,
    /// A project's sync run (`RUN_MODE=project`): `call` is then its ledger folder.
    project: Option<ProjectScope>,
}

#[derive(Debug)]
struct ProjectScope {
    id: String,
    /// The run's own folder: the only place it writes (its JSON files).
    run_dir: PathBuf,
    /// The items it may mark done (`RUN_ITEMS`).
    items: Vec<String>,
}

/// `echoctl run-guard`: judge the tool call on stdin; the exit code is Claude Code's answer.
pub fn command() -> i32 {
    let verdict = std::panic::catch_unwind(|| {
        let mut raw = String::new();
        std::io::stdin()
            .read_to_string(&mut raw)
            .map_err(|e| format!("the tool call can't be read: {e}"))
            .and_then(|_| serde_json::from_str::<Value>(&raw).map_err(|e| format!("the tool call isn't JSON: {e}")))
            .and_then(|call| Scope::from_env()?.judge(&call))
    })
    .unwrap_or_else(|_| Err("the guard failed".into()));
    match verdict {
        Ok(()) => 0,
        Err(why) => {
            let who = match std::env::var(RUN_MODE).as_deref() {
                Ok("ask") => "Echo (this call's Claude session reads only)",
                Ok("project") => "Echo's project sync",
                _ => "Echo's minutes run",
            };
            let _ = writeln!(std::io::stderr(), "{who}: {why}");
            2
        }
    }
}

/// A minutes run works only on its own call: `dir` must be the call `run_call` names (the
/// environment of the run, `RUN_CALL`). Checked by the CLI's call lookup inside a run, which then
/// works on the path returned — resolved, never the spelling asked for (`other/../own` would put
/// what is derived from it, such as the frames' trash, inside another call's folder).
pub fn own_call(dir: &Path, run_call: Option<&std::ffi::OsStr>) -> anyhow::Result<PathBuf> {
    let run = run_call.filter(|c| !c.is_empty()).ok_or_else(|| anyhow::anyhow!("a minutes run works only on its own call, and this one names none"))?;
    match (dunce::canonicalize(dir), dunce::canonicalize(run)) {
        (Ok(a), Ok(b)) if a == b => Ok(a),
        _ => anyhow::bail!("a minutes run works only on its own call, not {}", dir.display()),
    }
}

/// The run's own settings for Claude Code (`--settings`): the guard on every tool call, hooks on
/// whatever the user's settings say, and no instruction file from the call folders (`calls`: the
/// calls folder's absolute path with forward slashes, as `claudeMdExcludes` matches files).
pub fn settings(exe: &Path, calls: &str) -> Value {
    json!({
        "disableAllHooks": false,
        "claudeMdExcludes": [format!("{calls}/**")],
        "hooks": {"PreToolUse": [{"matcher": "*", "hooks": [{
            "type": "command",
            "command": hook_command(exe),
            "timeout": 10,
            "onFailure": "block",
        }]}]},
    })
}

/// The guard as a hook command line: this very binary. Single quotes for a POSIX shell; on
/// Windows double quotes and forward slashes, which Git Bash and cmd both take.
fn hook_command(exe: &Path) -> String {
    let p = exe.display().to_string();
    if cfg!(windows) {
        format!("\"{}\" run-guard", p.replace('\\', "/"))
    } else {
        format!("'{}' run-guard", p.replace('\'', r"'\''"))
    }
}

/// The gate before each tool call of a run through a wrapper (`echoctl proof gate`, claude-code-runs R14): the run
/// marked checked by Echo within `hold` (then the guard judges the call), or why the call is refused — always, for a
/// run Echo gave no tools (`deny_all`).
pub fn checked(value: &str, deny_all: bool, hold: std::time::Duration) -> Result<(), String> {
    if deny_all {
        return Err("Echo: this run has no tools".into());
    }
    let t = std::time::Instant::now();
    loop {
        if crate::ai::wrapper::marked(value) {
            return Ok(());
        }
        if t.elapsed() >= hold {
            return Err("Echo hasn't checked this run's start yet: no tool runs before it has".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// Is `version` (Claude Code's `claude_code_version`, "2.1.310") at least `MIN_CLAUDE`?
pub fn new_enough(version: &str) -> bool {
    let parts: Vec<u32> = version.split(['.', '-', ' ']).take(3).map_while(|p| p.parse().ok()).collect();
    parts.len() == 3 && parts.as_slice() >= MIN_CLAUDE.as_slice()
}

/// The session's own saved tool output (`<transcript without .jsonl>/tool-results`), named by Claude Code's hook
/// input: a long command answer Claude Code put aside for the model to Read. Never another session's.
fn own_results(call: &Value) -> Option<PathBuf> {
    let t = call["transcript_path"].as_str().map(native).filter(|t| t.is_absolute() && t.extension().is_some_and(|e| e == "jsonl"))?;
    let session = call["session_id"].as_str().unwrap_or("");
    let r = (!session.is_empty() && t.file_stem().is_some_and(|s| s == session)).then(|| t.with_extension("").join("tool-results"))?;
    Some(dunce::canonicalize(&r).unwrap_or(r))
}

impl Scope {
    /// From the run's environment; none outside a run Echo started (refused).
    pub fn from_env() -> Result<Scope, String> {
        Scope::from_vars(|k| std::env::var_os(k))
    }

    fn from_vars(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Result<Scope, String> {
        let clis_of = |v: Option<std::ffi::OsString>| -> Vec<String> { v.and_then(|c| c.into_string().ok()).unwrap_or_default().lines().filter(|l| !l.is_empty()).map(str::to_owned).collect() };
        if var(RUN_MODE).is_some_and(|m| m == "project") {
            let ledger = var(RUN_LEDGER).filter(|c| !c.is_empty()).ok_or("this isn't a sync run Echo started")?;
            let ledger = dunce::canonicalize(&ledger).map_err(|e| format!("the run's ledger can't be found: {e}"))?;
            if !ledger.join("evidence").is_dir() {
                return Err(format!("{} isn't a project's ledger", ledger.display()));
            }
            let run_dir = var(RUN_DIR).and_then(|d| dunce::canonicalize(d).ok()).filter(|d| d.parent() == Some(ledger.join("runs").as_path())).ok_or("the run's own folder isn't in its ledger")?;
            let id = var(RUN_PROJECT).and_then(|i| i.into_string().ok()).filter(|i| !i.is_empty() && i.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).ok_or("the run names no project")?;
            let clis = clis_of(var(RUN_CLIS));
            if clis.is_empty() {
                return Err("the run names no Echo CLI".into());
            }
            let items = clis_of(var(RUN_ITEMS));
            return Ok(Scope { call: ledger, previous: None, clis, ask: false, project: Some(ProjectScope { id, run_dir, items }) });
        }
        let call = var(RUN_CALL).filter(|c| !c.is_empty()).ok_or("this isn't a minutes run Echo started")?;
        let call = dunce::canonicalize(&call).map_err(|e| format!("the run's call folder can't be found: {e}"))?;
        if !call.join("meta.json").is_file() {
            return Err(format!("{} isn't a call folder", call.display()));
        }
        let previous = var(RUN_PREVIOUS).filter(|p| !p.is_empty()).and_then(|p| dunce::canonicalize(p).ok());
        let clis = clis_of(var(RUN_CLIS));
        if clis.is_empty() {
            return Err("the run names no Echo CLI".into());
        }
        let ask = var(RUN_MODE).is_some_and(|m| m == "ask");
        Ok(Scope { call, previous, clis, ask, project: None })
    }

    /// Allowed, or why not.
    pub fn judge(&self, call: &Value) -> Result<(), String> {
        let tool = call["tool_name"].as_str().unwrap_or("");
        let input = &call["tool_input"];
        // What plugins tell Claude (`.plugins/` in the data folder) is Echo's to write: a session there loads it.
        if matches!(tool, "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
            let raw = input["file_path"].as_str().or_else(|| input["notebook_path"].as_str());
            if let Ok(p) = resolve(raw) {
                if crate::plugins::agent::inside_root(&p) {
                    return Err(format!("{} is what a plugin tells Claude: only Echo writes there", p.display()));
                }
            }
        }
        if let Some(ps) = &self.project {
            return self.judge_project(ps, tool, input);
        }
        // A command's answer too long to show goes to the session's own tool results, for the model to Read there.
        let theirs = |p: &Path| own_results(call).is_some_and(|r| p.starts_with(&r));
        match tool {
            "Bash" => self.bash(input["command"].as_str().unwrap_or("")),
            "Read" => {
                let p = resolve(input["file_path"].as_str())?;
                if self.inside(&p) || self.previous.as_ref() == Some(&p) || theirs(&p) {
                    Ok(())
                } else {
                    Err(format!("only {} can be read, not {}", self.place(), p.display()))
                }
            }
            "Edit" | "Write" if self.ask => Err("this session reads the call; it changes no file".into()),
            "Edit" | "Write" => {
                let raw = input["file_path"].as_str();
                let p = resolve(raw)?;
                if p.parent() != Some(self.call.as_path()) || !minutes_file(&p) {
                    return Err(format!("a minutes run writes only notes.md and its new action list in this call's folder, not {}", p.display()));
                }
                // A link there would be written through (one that points nowhere doesn't resolve).
                if raw.map(native).is_some_and(|f| std::fs::symlink_metadata(f).is_ok_and(|m| m.file_type().is_symlink())) {
                    return Err(format!("{} is a link: not written", p.display()));
                }
                Ok(())
            }
            "Glob" | "Grep" => {
                let p = resolve(input["path"].as_str()).map_err(|e| format!("{tool} needs a path in this call's folder ({e})"))?;
                if !self.inside(&p) && !theirs(&p) {
                    return Err(format!("{tool} works only in {}, not {}", self.place(), p.display()));
                }
                let pattern = if tool == "Glob" { input["pattern"].as_str() } else { input["glob"].as_str() };
                match pattern {
                    Some(g) if escapes(g) => Err(format!("the pattern {g} reaches outside this call's folder")),
                    _ => Ok(()),
                }
            }
            "Skill" if self.ask => Err("no skill runs in this session".into()),
            // The Brain's read-only tools (`echoctl intake mcp`): the window's session may search the
            // user's other calls; a minutes run may not (it writes from its own call only).
            t if self.ask && t.starts_with(BRAIN_TOOLS) => Ok(()),
            "Skill" => {
                let name = input["skill"].as_str().or_else(|| input["command"].as_str()).unwrap_or("");
                if matches!(name.trim_start_matches('/'), "echo:process-call" | "process-call") {
                    Ok(())
                } else {
                    Err(format!("only the process-call skill runs here, not {name:?}"))
                }
            }
            "" => Err("no tool named".into()),
            other => Err(format!("{other} isn't available in a minutes run")),
        }
    }

    /// A project's sync run: read its ledger, write its own JSON files, run its ledger steps.
    fn judge_project(&self, ps: &ProjectScope, tool: &str, input: &Value) -> Result<(), String> {
        match tool {
            "Bash" => {
                let words = shell_words(input["command"].as_str().unwrap_or(""))?;
                let (cli, rest) = words.split_first().ok_or("an empty command")?;
                if !self.clis.iter().any(|c| c == cli) {
                    return Err(format!("only the Echo CLI runs here, not {cli}"));
                }
                let ok = rest.len() >= 4 && rest[0] == "project" && rest[1] == "ledger" && rest[2] == ps.id && PROJECT_STEPS.contains(&rest[3].as_str());
                if !ok {
                    return Err(format!("only `echoctl project ledger {} <{}>` runs here", ps.id, PROJECT_STEPS.join("|")));
                }
                // Done: only an item Echo gave this run for the work.
                let marks = rest[3] == "done" || rest.iter().any(|w| w == "--done" || w.starts_with("--done="));
                let evidence = rest.iter().position(|w| w == "--evidence").and_then(|k| rest.get(k + 1)).map(String::as_str).or_else(|| rest.iter().find_map(|w| w.strip_prefix("--evidence=")));
                if marks && !evidence.is_some_and(|e| ps.items.iter().any(|i| i == e)) {
                    return Err(format!("a sync run marks done only its own items ({})", ps.items.join(", ")));
                }
                Ok(())
            }
            // What it reads Echo gives it in its prompt.
            "Read" | "Glob" | "Grep" => Err("a sync run reads nothing: Echo gives it its text".into()),
            "Edit" => Err("a sync run writes its JSON files whole, with Write".into()),
            "Write" => {
                let raw = input["file_path"].as_str();
                let p = resolve(raw)?;
                let json = p.extension().is_some_and(|e| e == "json");
                if p.parent() != Some(ps.run_dir.as_path()) || !json {
                    return Err(format!("a sync run writes only its own JSON files in {}, not {}", ps.run_dir.display(), p.display()));
                }
                if raw.map(native).is_some_and(|f| std::fs::symlink_metadata(f).is_ok_and(|m| m.file_type().is_symlink())) {
                    return Err(format!("{} is a link: not written", p.display()));
                }
                Ok(())
            }
            "" => Err("no tool named".into()),
            other => Err(format!("{other} isn't available in a project sync")),
        }
    }

    fn place(&self) -> &'static str {
        "this call's folder"
    }

    fn inside(&self, p: &Path) -> bool {
        p.starts_with(&self.call)
    }

    /// One allowed Echo command, as plain words.
    fn bash(&self, line: &str) -> Result<(), String> {
        let words = shell_words(line)?;
        let (cli, rest) = words.split_first().ok_or("an empty command")?;
        if !self.clis.iter().any(|c| c == cli) {
            return Err(format!("only the Echo CLI runs here, not {cli}"));
        }
        let commands: &[&str] = if self.ask { &ASK_COMMANDS } else { &crate::notes::RUN_COMMANDS };
        let allowed = commands.iter().any(|c| {
            let c: Vec<&str> = c.split(' ').collect();
            rest.len() >= c.len() && rest.iter().zip(&c).all(|(w, c)| *c == "*" || w.as_str() == *c)
        });
        if allowed {
            Ok(())
        } else {
            let who = if self.ask { "this session" } else { "a minutes run" };
            Err(format!("echoctl {} isn't one of the commands {who} uses", rest.first().map(String::as_str).unwrap_or("")))
        }
    }
}

/// A command line as the shell would split it, for plain commands only: no variables or command
/// substitution anywhere (`$`, backtick), no line breaks, and outside quotes none of the characters
/// that make a command more than one program with its words (lists, pipes, redirects, subshells,
/// globs, braces, `~`, comments).
fn shell_words(line: &str) -> Result<Vec<String>, String> {
    if let Some(c) = line.chars().find(|c| matches!(c, '$' | '`' | '\n' | '\r')) {
        return Err(format!("{c:?} isn't allowed in the commands of a run Echo started"));
    }
    let mut words = vec![];
    let mut w = String::new();
    let mut started = false; // a word is open (also an empty quoted one)
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' => {
                if started {
                    words.push(std::mem::take(&mut w));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => w.push(c),
                        None => return Err("a quote isn't closed".into()),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(c @ ('"' | '\\')) => w.push(c),
                            Some(c) => {
                                w.push('\\');
                                w.push(c);
                            }
                            None => return Err("a quote isn't closed".into()),
                        },
                        Some(c) => w.push(c),
                        None => return Err("a quote isn't closed".into()),
                    }
                }
            }
            '\\' => {
                started = true;
                w.push(chars.next().ok_or("a line ending in a backslash")?);
            }
            ';' | '&' | '|' | '<' | '>' | '(' | ')' | '{' | '}' | '*' | '?' | '[' | ']' | '~' | '#' | '!' => {
                return Err(format!("{c:?} outside quotes isn't allowed in the commands of a run Echo started"));
            }
            c => {
                started = true;
                w.push(c);
            }
        }
    }
    if started {
        words.push(w);
    }
    Ok(words)
}

/// An absolute path as it is on disk: symlinks and `..` resolved; for a file not there yet, its
/// nearest existing folder resolved and the rest kept (no `..` in it).
fn resolve(p: Option<&str>) -> Result<PathBuf, String> {
    let p = p.filter(|p| !p.is_empty()).ok_or("no path given")?;
    let p = native(p);
    if !p.is_absolute() {
        return Err(format!("{} isn't an absolute path", p.display()));
    }
    let mut base = p.as_path();
    let mut rest = vec![];
    loop {
        if let Ok(real) = dunce::canonicalize(base) {
            if rest.iter().any(|c: &Component| matches!(c, Component::ParentDir)) {
                return Err(format!("{} goes up with ..", p.display()));
            }
            return Ok(rest.iter().rev().fold(real, |acc, c| acc.join(c)));
        }
        let (Some(parent), Some(last)) = (base.parent(), base.components().next_back()) else {
            return Err(format!("{} can't be resolved", p.display()));
        };
        rest.push(last);
        base = parent;
    }
}

/// Windows: Git Bash's `/c/Users/…` is `C:\Users\…`. Elsewhere the path as it is.
fn native(p: &str) -> PathBuf {
    if cfg!(windows) {
        let b = p.as_bytes();
        if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b'/' {
            return PathBuf::from(format!("{}:\\{}", (b[1] as char).to_ascii_uppercase(), p[3..].replace('/', "\\")));
        }
    }
    PathBuf::from(p)
}

/// The minutes a run writes: `notes.md`, and the new action list it applies (`actions.new*.json`,
/// the app's prompt names the run's own: `calls::actions_new_of`).
fn minutes_file(p: &Path) -> bool {
    // A memo's split by thread too (`memo threads <call> --file memo-threads.new.json`).
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == "notes.md" || n == "memo-threads.new.json" || (n.starts_with("actions.new") && n.ends_with(".json")))
}

/// A glob that could reach outside the folder it is given: absolute, from home, going up, or with
/// braces (`{..,x}` expands to a way up).
fn escapes(g: &str) -> bool {
    let g = g.replace('\\', "/");
    g.starts_with('/') || g.starts_with('~') || g.as_bytes().get(1) == Some(&b':') || g.split('/').any(|s| s == "..") || g.contains(['{', '}'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scope(root: &Path) -> Scope {
        Scope {
            call: dunce::canonicalize(root.join("calls/2026-10-25_100000-call")).unwrap(),
            previous: dunce::canonicalize(root.join("calls/2026-10-18_100000-call/notes.md")).ok(),
            clis: vec!["echoctl".into(), "/Apps/Echo.app/plugin/scripts/echoctl".into()],
            ask: false,
            project: None,
        }
    }

    #[test]
    fn a_project_sync_writes_its_run_and_runs_only_its_apply_people_apply_and_done_steps() {
        let (_env, _) = crate::paths::testenv::folders("guard-project-sync");
        let root = std::env::temp_dir().join(format!("echo-guard-project-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let ledger = root.join("projects/aurora/ledger");
        fs::create_dir_all(ledger.join("evidence")).unwrap();
        fs::create_dir_all(ledger.join("runs/r1")).unwrap();
        fs::create_dir_all(root.join("projects/helix/ledger/evidence")).unwrap();
        fs::write(ledger.join("evidence/2026-10-28_110000.md"), "x").unwrap();
        let vars = |k: &str| -> Option<std::ffi::OsString> {
            match k {
                RUN_MODE => Some("project".into()),
                RUN_LEDGER => Some(ledger.clone().into()),
                RUN_DIR => Some(ledger.join("runs/r1").into()),
                RUN_PROJECT => Some("aurora".into()),
                RUN_CLIS => Some("echoctl".into()),
                RUN_ITEMS => Some("2026-10-28_110000".into()),
                _ => None,
            }
        };
        let s = Scope::from_vars(vars).unwrap();
        let ok = |name: &str, input: Value| s.judge(&tool(name, input));
        let l = dunce::canonicalize(&ledger).unwrap();
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora apply --evidence 2026-10-28_110000 --proposals /x/p.json --done 'read'"})).is_ok());
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora people-apply --evidence 2026-10-28_110000 --file /x/p.json"})).is_ok());
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora done --evidence 2026-10-28_110000 --note 'nothing here'"})).is_ok());
        // Done only for its own items: another item of the thread is refused, as are apply's --done and --evidence=.
        let mine = "a sync run marks done only its own items (2026-10-28_110000)";
        assert_eq!(ok("Bash", json!({"command": "echoctl project ledger aurora done --evidence 2026-10-01_090000 --note 'x'"})).unwrap_err(), mine);
        assert_eq!(ok("Bash", json!({"command": "echoctl project ledger aurora apply --evidence 2026-10-01_090000 --proposals /x/p.json --done 'x'"})).unwrap_err(), mine);
        assert_eq!(ok("Bash", json!({"command": "echoctl project ledger aurora done --evidence=2026-10-01_090000"})).unwrap_err(), mine);
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora done --evidence=2026-10-28_110000"})).is_ok());
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora apply --evidence 2026-10-01_090000 --proposals /x/p.json"})).is_ok(), "a fix, not done");
        assert_eq!(ok("Bash", json!({"command": "echoctl project ledger aurora done"})).unwrap_err(), mine, "no item named");
        // What it reads is in its prompt: the reading steps, and the verifier's, are refused.
        for step in ["pending", "context", "people-context --items '2026-10-28_110000'", "verify-prep", "verify-apply --file /x/v.json"] {
            let e = ok("Bash", json!({"command": format!("echoctl project ledger aurora {step}")})).unwrap_err();
            assert_eq!(e, "only `echoctl project ledger aurora <apply|done|people-apply>` runs here", "{step}");
        }
        assert!(ok("Bash", json!({"command": "echoctl project people aurora"})).is_err(), "not the people tab's own command");
        assert!(ok("Bash", json!({"command": "echoctl project ledger helix context"})).is_err(), "another project");
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora edit '{}'"})).is_err(), "the user's edits");
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora set T001 --state done"})).is_err(), "the user's edits, by flags too");
        assert!(ok("Bash", json!({"command": "echoctl project ledger aurora done --evidence x; rm -rf /"})).is_err());
        // It reads nothing (Echo gives it its text), not even its own ledger or its session's saved output.
        let claude = std::env::temp_dir().join(format!("echo-guard-sync-claude-{}", std::process::id()));
        let results = claude.join("s9/tool-results");
        let read = json!({"tool_name": "Read", "tool_input": {"file_path": results.join("ctx.txt").display().to_string()},
                          "session_id": "s9", "transcript_path": claude.join("s9.jsonl").display().to_string()});
        assert_eq!(s.judge(&read).unwrap_err(), "a sync run reads nothing: Echo gives it its text");
        assert!(ok("Bash", json!({"command": "echoctl notes latest"})).is_err());
        assert!(ok("Read", json!({"file_path": l.join("evidence/2026-10-28_110000.md")})).is_err());
        assert!(ok("Grep", json!({"path": l, "pattern": "x"})).is_err());
        assert!(ok("Write", json!({"file_path": l.join("runs/r1/2026-10-28_110000.json")})).is_ok());
        assert!(ok("Edit", json!({"file_path": l.join("runs/r1/2026-10-28_110000.json")})).is_err(), "Write only");
        assert!(ok("Write", json!({"file_path": l.join("runs/r1/people-applied.txt")})).is_err(), "never Echo's note of people-apply");
        assert!(ok("Write", json!({"file_path": l.join("runs/r1/notes.md")})).is_err(), "JSON only");
        assert!(ok("Write", json!({"file_path": l.join("events.jsonl")})).is_err(), "never the log itself");
        assert!(ok("Edit", json!({"file_path": l.join("evidence/2026-10-28_110000.md")})).is_err());
        assert!(ok("Glob", json!({"path": l, "pattern": "../../**"})).is_err());
        assert!(ok("Skill", json!({"skill": "echo:project-sync"})).is_err(), "its instructions are in its prompt: no skill");
        assert!(ok("Read", json!({"file_path": root.join("plugin/skills/project-sync/EXTRACT.md")})).is_err());
        assert!(ok("Skill", json!({"skill": "echo:process-call"})).is_err());
        assert!(ok("WebFetch", json!({"url": "https://example.com"})).is_err());
        // A run folder outside the ledger's runs is no run.
        let bad = |k: &str| if k == RUN_DIR { Some(root.join("projects").into()) } else { vars(k) };
        assert!(Scope::from_vars(bad).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    fn tool(name: &str, input: Value) -> Value {
        json!({"hook_event_name": "PreToolUse", "tool_name": name, "tool_input": input})
    }

    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("echo-guard-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for d in ["calls/2026-10-25_100000-call/frames", "calls/2026-10-18_100000-call", "calls/2026-10-24_100000-other", ".echo"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        for f in ["calls/2026-10-25_100000-call/meta.json", "calls/2026-10-25_100000-call/transcript.md", "calls/2026-10-18_100000-call/notes.md", "calls/2026-10-18_100000-call/transcript.md", "calls/2026-10-24_100000-other/transcript.md", ".echo/config.json"] {
            fs::write(root.join(f), "x").unwrap();
        }
        root
    }

    #[test]
    fn only_this_calls_files_and_the_previous_minutes() {
        let (_env, _) = crate::paths::testenv::folders("guard-files");
        let root = fixture("files");
        let s = scope(&root);
        let call = root.join("calls/2026-10-25_100000-call");
        let ok = |t: &str, input: Value| s.judge(&tool(t, input)).map_err(|e| format!("{t}: {e}"));
        let path = |p: &Path| p.display().to_string();
        // Its own call: read anything there; write and edit the minutes and its new action list only.
        ok("Read", json!({"file_path": path(&call.join("transcript.md"))})).unwrap();
        ok("Read", json!({"file_path": path(&call.join("frames/0001.jpg"))})).unwrap();
        ok("Write", json!({"file_path": path(&call.join("notes.md")), "content": "x"})).unwrap();
        ok("Edit", json!({"file_path": path(&call.join("notes.md"))})).unwrap();
        ok("Write", json!({"file_path": path(&call.join("actions.new-r1.json")), "content": "[]"})).unwrap();
        ok("Write", json!({"file_path": path(&call.join("actions.new.json")), "content": "[]"})).unwrap();
        ok("Glob", json!({"pattern": "frames/*.jpg", "path": path(&call)})).unwrap();
        ok("Grep", json!({"pattern": "TBD", "path": path(&call), "glob": "*.md"})).unwrap();
        // The previous minutes: read only that file, nothing else of that call.
        ok("Read", json!({"file_path": path(&root.join("calls/2026-10-18_100000-call/notes.md"))})).unwrap();
        for (t, input) in [
            // Echo's own files of the call, the user's notes, the transcript, anything deeper.
            ("Write", json!({"file_path": path(&call.join("meta.json")), "content": "{\"ai_profile\": \"personal\"}"})),
            ("Edit", json!({"file_path": path(&call.join("transcript.md"))})),
            ("Write", json!({"file_path": path(&call.join("my-notes.md"))})),
            ("Write", json!({"file_path": path(&call.join("actions.json"))})),
            ("Write", json!({"file_path": path(&call.join("notes-run.json"))})),
            ("Write", json!({"file_path": path(&call.join("new/notes.md"))})),
            ("Write", json!({"file_path": path(&call.join("frames/notes.md"))})),
            ("Write", json!({"file_path": path(&call.join("notes.md."))})),
            ("Write", json!({"file_path": path(&call.join("notes.md::$DATA"))})),
            ("Read", json!({"file_path": path(&root.join("calls/2026-10-18_100000-call/transcript.md"))})),
            ("Write", json!({"file_path": path(&root.join("calls/2026-10-18_100000-call/notes.md"))})),
            ("Read", json!({"file_path": path(&root.join("calls/2026-10-24_100000-other/transcript.md"))})),
            ("Read", json!({"file_path": path(&root.join(".echo/config.json"))})),
            ("Read", json!({"file_path": format!("{}/../2026-10-24_100000-other/transcript.md", path(&call))})),
            ("Write", json!({"file_path": format!("{}/new/../../2026-10-24_100000-other/x.md", path(&call))})),
            ("Read", json!({"file_path": "transcript.md"})),
            ("Read", json!({})),
            ("Glob", json!({"pattern": "*.md"})),
            ("Glob", json!({"pattern": "../*/transcript.md", "path": path(&call)})),
            ("Glob", json!({"pattern": "/etc/*", "path": path(&call)})),
            ("Glob", json!({"pattern": "{..,x}/*/transcript.md", "path": path(&call)})),
            ("Grep", json!({"pattern": "x", "path": path(&root.join("calls"))})),
            ("Grep", json!({"pattern": "x", "path": path(&call), "glob": "~/x"})),
        ] {
            assert!(s.judge(&tool(t, input.clone())).is_err(), "{t} {input}");
        }
        // A long call's context saved by Claude Code (#98): this session's own copy may be read, nobody else's.
        let claude = std::env::temp_dir().join(format!("echo-guard-minutes-claude-{}", std::process::id()));
        let results = claude.join("m1/tool-results");
        fs::create_dir_all(&results).unwrap();
        fs::write(results.join("ctx.txt"), "x").unwrap();
        let read = |session: &str| json!({"tool_name": "Read", "tool_input": {"file_path": path(&results.join("ctx.txt"))},
                                          "session_id": session, "transcript_path": path(&claude.join("m1.jsonl"))});
        assert!(s.judge(&read("m1")).is_ok(), "its own saved output");
        assert!(s.judge(&read("other")).is_err(), "not another session's");
        let in_results = |input: Value| json!({"tool_name": "Write", "tool_input": input, "session_id": "m1", "transcript_path": path(&claude.join("m1.jsonl"))});
        assert!(s.judge(&in_results(json!({"file_path": path(&results.join("notes.md")), "content": "x"}))).is_err(), "read, never written");
        let climb = json!({"tool_name": "Glob", "tool_input": {"path": path(&results), "pattern": "../../*.jsonl"}, "session_id": "m1", "transcript_path": path(&claude.join("m1.jsonl"))});
        assert!(s.judge(&climb).is_err(), "a pattern climbing out of it");
        let _ = fs::remove_dir_all(&claude);
    }

    #[test]
    fn the_windows_session_on_a_call_reads_and_changes_nothing() {
        let (_env, _) = crate::paths::testenv::folders("guard-windows-session");
        let root = fixture("ask");
        let s = Scope { ask: true, ..scope(&root) };
        assert!(s.judge(&json!({"tool_name": "mcp__echo__search_sources", "tool_input": {"query": "budget"}})).is_ok(), "the Brain reads");
        assert!(s.judge(&json!({"tool_name": "mcp__other__send_mail", "tool_input": {}})).is_err(), "nothing else");
        assert!(Scope { ask: false, ..scope(&root) }.judge(&json!({"tool_name": "mcp__echo__search_sources", "tool_input": {}})).is_err(), "not in a minutes run");
        let call = root.join("calls/2026-10-25_100000-call");
        let path = |p: &Path| p.display().to_string();
        let ok = |t: &str, input: Value| s.judge(&tool(t, input)).map_err(|e| format!("{t}: {e}"));
        // Reading stays as in a run: the call, the previous minutes, Echo's context.
        ok("Read", json!({"file_path": path(&call.join("transcript.md"))})).unwrap();
        ok("Read", json!({"file_path": path(&root.join("calls/2026-10-18_100000-call/notes.md"))})).unwrap();
        ok("Grep", json!({"pattern": "SSO", "path": path(&call)})).unwrap();
        ok("Bash", json!({"command": format!("echoctl context {}", path(&call))})).unwrap();
        // Nothing is written, no skill runs, and no other command — not even the minutes' own.
        for (t, input) in [
            ("Write", json!({"file_path": path(&call.join("notes.md")), "content": "x"})),
            ("Edit", json!({"file_path": path(&call.join("notes.md"))})),
            ("Skill", json!({"skill": "echo:process-call"})),
            ("Bash", json!({"command": format!("echoctl title {} 'x'", path(&call))})),
            ("Bash", json!({"command": format!("echoctl actions {} --apply x.json", path(&call))})),
            ("Bash", json!({"command": "ls ~"})),
            ("Read", json!({"file_path": path(&root.join("calls/2026-10-24_100000-other/transcript.md"))})),
            ("WebFetch", json!({"url": "https://example.com"})),
        ] {
            assert!(s.judge(&tool(t, input.clone())).is_err(), "{t} {input}");
        }
        // The environment says which: `ECHOCTL_RUN_MODE=ask`.
        let vars = |k: &str| match k {
            RUN_CALL => Some(call.clone().into_os_string()),
            RUN_CLIS => Some("echoctl".into()),
            RUN_MODE => Some("ask".into()),
            _ => None,
        };
        assert!(Scope::from_vars(vars).unwrap().ask);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_neighbour_named_like_the_call_is_another_call() {
        let (_env, _) = crate::paths::testenv::folders("guard-neighbour");
        let root = fixture("neighbour");
        // A second call in the same second gets `-2`; an untitled call is its bare stamp: both
        // start like a neighbour's name.
        for (own, twin) in [("2026-10-25_100000-call", "2026-10-25_100000-call-2"), ("2026-10-25_110000", "2026-10-25_110000-x")] {
            for d in [own, twin] {
                fs::create_dir_all(root.join("calls").join(d)).unwrap();
                fs::write(root.join("calls").join(d).join("transcript.md"), "x").unwrap();
            }
            let s = Scope { call: dunce::canonicalize(root.join("calls").join(own)).unwrap(), previous: None, clis: vec!["echoctl".into()], ask: false, project: None };
            let p = |f: &str| root.join("calls").join(twin).join(f).display().to_string();
            for (t, input) in [
                ("Read", json!({"file_path": p("transcript.md")})),
                ("Write", json!({"file_path": p("notes.md")})),
                ("Glob", json!({"pattern": "*", "path": p("")})),
                ("Grep", json!({"pattern": "x", "path": p("")})),
            ] {
                assert!(s.judge(&tool(t, input.clone())).is_err(), "{own}: {t} {input}");
            }
        }
    }

    #[test]
    fn without_its_environment_the_guard_allows_nothing() {
        let root = fixture("no-env");
        let call = root.join("calls/2026-10-25_100000-call").into_os_string();
        let vars = |call: Option<std::ffi::OsString>, clis: Option<&'static str>| {
            move |k: &str| match k {
                RUN_CALL => call.clone(),
                RUN_CLIS => clis.map(Into::into),
                _ => None,
            }
        };
        assert!(Scope::from_vars(vars(None, Some("echoctl"))).is_err());
        assert!(Scope::from_vars(vars(Some("".into()), Some("echoctl"))).is_err());
        assert!(Scope::from_vars(vars(Some(root.join("calls/gone").into_os_string()), Some("echoctl"))).is_err());
        assert!(Scope::from_vars(vars(Some(root.join("calls").into_os_string()), Some("echoctl"))).is_err(), "not a call folder");
        assert!(Scope::from_vars(vars(Some(call.clone()), None)).is_err(), "no CLI named");
        let s = Scope::from_vars(vars(Some(call), Some("echoctl\n/x/echoctl"))).unwrap();
        assert_eq!(s.clis, ["echoctl", "/x/echoctl"]);
        assert!(s.previous.is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_call_is_followed_and_refused() {
        let (_env, _) = crate::paths::testenv::folders("guard-link-out");
        let root = fixture("links");
        let s = scope(&root);
        let call = root.join("calls/2026-10-25_100000-call");
        std::os::unix::fs::symlink(root.join("calls/2026-10-24_100000-other"), call.join("other")).unwrap();
        std::os::unix::fs::symlink(root.join("calls/2026-10-24_100000-other/transcript.md"), call.join("t.md")).unwrap();
        for f in ["other/transcript.md", "t.md"] {
            let input = json!({"file_path": call.join(f).display().to_string()});
            assert!(s.judge(&tool("Read", input.clone())).is_err() && s.judge(&tool("Write", input)).is_err(), "{f}");
        }
        // A link named like the minutes, also one that points nowhere yet: never written through.
        std::os::unix::fs::symlink(root.join("calls/2026-10-24_100000-other/notes.md"), call.join("notes.md")).unwrap();
        std::os::unix::fs::symlink(root.join("calls/2026-10-24_100000-other/transcript.md"), call.join("actions.new.json")).unwrap();
        for f in ["notes.md", "actions.new.json"] {
            assert!(s.judge(&tool("Write", json!({"file_path": call.join(f).display().to_string()}))).is_err(), "{f}");
        }
    }

    #[test]
    fn no_instruction_files_are_written() {
        let (_env, _) = crate::paths::testenv::folders("guard-instructions");
        let root = fixture("instructions");
        let s = scope(&root);
        let call = root.join("calls/2026-10-25_100000-call");
        for f in ["CLAUDE.md", "claude.md", "CLAUDE.local.md", "AGENTS.md", "frames/CLAUDE.md", ".claude/rules/x.md", ".claude/settings.json", ".CLAUDE/x"] {
            let input = json!({"file_path": call.join(f).display().to_string(), "content": "@../2026-10-24_100000-other/transcript.md"});
            assert!(s.judge(&tool("Write", input.clone())).is_err() && s.judge(&tool("Edit", input)).is_err(), "{f}");
        }
        // Reading one is no harm (it was there before the run): the CLAUDE.md files under calls/
        // aren't loaded (`claudeMdExcludes`).
        s.judge(&tool("Read", json!({"file_path": call.join("CLAUDE.md").display().to_string()}))).unwrap();
    }

    #[test]
    fn what_plugins_tell_claude_is_never_written_by_a_run() {
        let (_env, root) = crate::paths::testenv::folders("guard-plugins");
        let kit = dunce::canonicalize(root.join("home")).unwrap().join(".plugins/kit");
        fs::create_dir_all(kit.join(".claude/skills/x")).unwrap();
        fs::write(kit.join("CLAUDE.md"), "# Kit").unwrap();
        // Even a run whose own folder it were: a write there is Echo's alone.
        let s = Scope { call: kit.clone(), previous: None, clis: vec!["echoctl".into()], ask: false, project: None };
        for (t, f) in [("Write", "notes.md"), ("Edit", "CLAUDE.md"), ("Write", ".claude/skills/x/SKILL.md"), ("MultiEdit", "notes.md")] {
            let e = s.judge(&tool(t, json!({"file_path": kit.join(f).display().to_string(), "content": "x"}))).unwrap_err();
            assert!(e.contains("only Echo writes there"), "{t} {f}: {e}");
        }
        let e = s.judge(&tool("NotebookEdit", json!({"notebook_path": kit.join("n.ipynb").display().to_string()}))).unwrap_err();
        assert!(e.contains("only Echo writes there"), "{e}");
        // Reading it is what a session does.
        s.judge(&tool("Read", json!({"file_path": kit.join("CLAUDE.md").display().to_string()}))).unwrap();
    }

    #[test]
    fn only_the_minutes_commands_as_plain_words() {
        let root = fixture("bash");
        let s = scope(&root);
        let ok = |c: &str| s.judge(&tool("Bash", json!({"command": c})));
        for c in [
            "echoctl context /data/calls/2026-10-25_100000-call",
            "echoctl actions '/data/My Calls/x' --apply '/data/My Calls/x/actions.new-r1.json'",
            r#"echoctl speakers /x 2="Jane Doe" 3='Ann O'\''Neil'"#,
            r#"echoctl title /x "Q&A (Atlas) — planning; next steps""#,
            "echoctl vocab add --call /x Kubernetes 'Atlas DB'",
            "echoctl frames /x --keep 0012.jpg 0040.jpg",
            "echoctl transcribe /x",
            "/Apps/Echo.app/plugin/scripts/echoctl context /x",
            r#""/Apps/Echo.app/plugin/scripts/echoctl" context /x"#,
        ] {
            ok(c).map_err(|e| format!("{c}: {e}")).unwrap();
        }
        for c in [
            "echoctl context /x > /data/.echo/stop.request",
            "echoctl context /x >> f",
            "echoctl context /x; cat /etc/passwd",
            "echoctl context /x && rm -rf /",
            "echoctl context /x | tee f",
            "echoctl context $(cat f)",
            "echoctl context `cat f`",
            "echoctl context \"$HOME\"",
            "echoctl context /x\ncat f",
            "echoctl context /x &",
            "echoctl context ~/x",
            "echoctl context /data/calls/*",
            "echoctl config --notes-model x",
            "echoctl vocab remove x",
            "echoctl app quit",
            "echoctl",
            "cat /data/calls/x/transcript.md",
            "ECHOCTL_RUN_CALL=/y echoctl context /y",
            "/tmp/echoctl context /x",
            "echoctl 'context /x",
            "",
        ] {
            assert!(ok(c).is_err(), "{c:?}");
        }
    }

    #[test]
    fn other_tools_and_unknown_input_are_refused() {
        let root = fixture("tools");
        let s = scope(&root);
        s.judge(&tool("Skill", json!({"skill": "echo:process-call"}))).unwrap();
        for (t, input) in [
            ("Skill", json!({"skill": "pdf"})),
            ("WebFetch", json!({"url": "https://example.com"})),
            ("Task", json!({"prompt": "x"})),
            ("mcp__x__y", json!({})),
            ("NotebookEdit", json!({})),
            ("", json!({})),
        ] {
            assert!(s.judge(&tool(t, input)).is_err(), "{t}");
        }
        assert!(s.judge(&json!("not a tool call")).is_err());
    }

    #[test]
    fn the_cli_works_only_on_the_runs_call() {
        let root = fixture("own-call");
        let call = root.join("calls/2026-10-25_100000-call");
        let real = dunce::canonicalize(&call).unwrap();
        assert_eq!(own_call(&call, Some(call.as_os_str())).unwrap(), real);
        // However it is spelled — and what the CLI then works on is the resolved path, not a
        // spelling through another call (`other/../own` would put the frames' trash beside it).
        assert_eq!(own_call(&root.join("calls/./2026-10-25_100000-call"), Some(call.as_os_str())).unwrap(), real);
        assert_eq!(own_call(&root.join("calls/2026-10-24_100000-other/../2026-10-25_100000-call"), Some(call.as_os_str())).unwrap(), real);
        assert!(own_call(&root.join("calls/2026-10-24_100000-other"), Some(call.as_os_str())).is_err());
        assert!(own_call(&call, None).is_err() && own_call(&call, Some(std::ffi::OsStr::new(""))).is_err());
    }

    #[test]
    fn versions_and_the_hook_command() {
        assert!(new_enough("2.1.295") && new_enough("2.1.310") && new_enough("2.2.0") && new_enough("3.0.0 (Claude Code)"));
        assert!(!new_enough("2.1.294") && !new_enough("1.9.999") && !new_enough("") && !new_enough("2.1"));
        let s = settings(Path::new("/Apps/Echo's.app/echoctl"), "/data/calls");
        assert_eq!(s["hooks"]["PreToolUse"][0]["matcher"], "*", "every tool");
        let hook = &s["hooks"]["PreToolUse"][0]["hooks"][0];
        assert_eq!(hook["onFailure"], "block");
        assert_eq!(s["disableAllHooks"], false);
        assert_eq!(s["claudeMdExcludes"][0], "/data/calls/**");
        if cfg!(windows) {
            assert_eq!(hook_command(Path::new(r"C:\Program Files\echo\echoctl.exe")), r#""C:/Program Files/echo/echoctl.exe" run-guard"#);
        } else {
            assert_eq!(hook["command"], r"'/Apps/Echo'\''s.app/echoctl' run-guard");
        }
    }
}

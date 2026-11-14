//! A plugin's scripts: programs its package skills ship, which Echo runs only as `uses.scripts` declares them —
//! `[{skill, script, readonly?, timeout?}]`. They run on this computer with the user's rights, outside every limit
//! of the host half, so the sheet names each one. Every run goes through `run`: the host half's
//! `ctx.scripts.run`, and Echo's sessions through the wrapper Echo puts at the script's path in its session copy
//! (`wrapper`), which hands it to `echoctl plugin script` — never Claude Code's own environment.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::app::NoWindow;

/// The time a script may take unless its manifest says otherwise, and the most it may say (seconds).
pub const DEFAULT_TIMEOUT: u64 = 300;
pub const MAX_TIMEOUT: u64 = 3600;
/// What Echo keeps of a script's stdout and of its stderr, each.
const OUTPUT_CAP: usize = 1 << 20;
const MAX_SCRIPTS: usize = 20;
const MAX_ARGS: usize = 100;

/// A script a plugin declared in `uses.scripts`.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    /// One of its package skills (the folder's name).
    pub skill: String,
    /// Its path inside that skill's folder (`scripts/read.sh`).
    pub script: String,
    /// Changes nothing (the plugin says so; Echo can't check it): a read-only routine may run it.
    pub readonly: bool,
    /// Seconds it may take.
    pub timeout: u64,
}

impl Script {
    pub fn name(&self) -> String {
        format!("{}/{}", self.skill, self.script)
    }
}

/// A script's path inside its skill folder: plain names (letters, digits, `.`, `_`, `-`) joined by `/`.
fn plain_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 200
        && Path::new(p).components().all(|c| matches!(c, Component::Normal(n) if !n.to_string_lossy().starts_with('.')))
        && p.split('/').all(|w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-')))
}

/// The scripts `uses` declares, their shape checked (the files are checked by `check`).
pub fn declared(uses: &serde_json::Map<String, Value>) -> Result<Vec<Script>> {
    let Some(v) = uses.get("scripts") else { return Ok(vec![]) };
    let list = v.as_array().ok_or_else(|| anyhow!("“scripts” is a list of {{skill, script, readonly?, timeout?}}"))?;
    if list.is_empty() || list.len() > MAX_SCRIPTS {
        bail!("“scripts” names 1–{MAX_SCRIPTS} scripts");
    }
    let mut out: Vec<Script> = vec![];
    for s in list {
        let o = s.as_object().ok_or_else(|| anyhow!("a script is {{skill, script, readonly?, timeout?}}"))?;
        if let Some(k) = o.keys().find(|k| !["skill", "script", "readonly", "timeout"].contains(&k.as_str())) {
            bail!("a script has no “{k}” (skill, script, readonly, timeout)");
        }
        let skill = o.get("skill").and_then(Value::as_str).unwrap_or("");
        let script = o.get("script").and_then(Value::as_str).unwrap_or("").trim_start_matches("./");
        if !super::valid_skill_name(skill) || !plain_path(script) {
            bail!("a script names one of its skills and a plain path inside its folder (“{skill}” “{script}”)");
        }
        let readonly = match o.get("readonly") {
            None => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => bail!("script {skill}/{script}: readonly is true or false"),
        };
        let timeout = match o.get("timeout") {
            None => DEFAULT_TIMEOUT,
            Some(t) => t.as_u64().filter(|t| (1..=MAX_TIMEOUT).contains(t)).ok_or_else(|| anyhow!("script {skill}/{script}: timeout is 1–{MAX_TIMEOUT} seconds"))?,
        };
        let s = Script { skill: skill.into(), script: script.into(), readonly, timeout };
        if out.iter().any(|o| o.name() == s.name()) {
            bail!("script {} is named twice", s.name());
        }
        out.push(s);
    }
    Ok(out)
}

/// The sheet's line for `uses.scripts`, in Echo's words.
pub(super) fn describe_scripts(v: &Value) -> Result<String> {
    let mut uses = serde_json::Map::new();
    uses.insert("scripts".into(), v.clone());
    let items: Vec<String> = declared(&uses)?
        .iter()
        .map(|s| {
            let mut notes = vec![];
            if s.readonly {
                notes.push("read-only routines may run it; Echo can't check it only reads".to_string());
            }
            if s.timeout > DEFAULT_TIMEOUT {
                notes.push(format!("up to {}", minutes(s.timeout)));
            }
            if notes.is_empty() { format!("“{}”", s.name()) } else { format!("“{}” ({})", s.name(), notes.join("; ")) }
        })
        .collect();
    Ok(format!("Runs programs from its package on this computer with your rights — they can reach the network and anything you can: {}", items.join(", ")))
}

fn minutes(secs: u64) -> String {
    match secs {
        s if s % 3600 == 0 => format!("{} h", s / 3600),
        s if s % 60 == 0 => format!("{} min", s / 60),
        s => format!("{s} s"),
    }
}

/// Its declared scripts checked against its package (`dir`): each in one of its package skills, a file there. Gives
/// what the sheet adds so that an update changing any file of those skill folders asks again: their files' hash.
pub(super) fn check(dir: &Path, m: &super::Manifest) -> Result<Option<String>> {
    let scripts = declared(&m.uses)?;
    if scripts.is_empty() {
        return Ok(None);
    }
    let skills = super::package_skills(dir, m)?;
    let mut folders: Vec<&PathBuf> = vec![];
    for s in &scripts {
        let (_, folder) = skills.iter().find(|(n, _)| *n == s.skill).ok_or_else(|| anyhow!("script {}: “{}” isn't one of its skills", s.name(), s.skill))?;
        let f = folder.join(&s.script);
        if !std::fs::symlink_metadata(&f).is_ok_and(|m| m.is_file()) {
            bail!("script {} isn't a file in its skill", s.name());
        }
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    let mut all = String::new();
    for f in folders {
        all.push_str(&super::sources::tree_hash(f)?);
    }
    use sha2::Digest;
    Ok(Some(format!("{:x}", sha2::Sha256::digest(all.as_bytes()))[..8].to_string()))
}

/// What Echo's sessions find at a declared script's path in the session copy: a launcher handing it to
/// `echoctl plugin script` (`launcher`: the session plugin's `scripts/echoctl`), so it runs as every script does.
pub fn wrapper(id: &str, s: &Script, launcher: &Path) -> String {
    let q = |t: &str| format!("'{}'", t.replace('\'', "'\\''"));
    format!(
        "#!/bin/sh\n# Echo runs this plugin's script itself (its limits, its environment): {}\nexec {} plugin script {} {} {} -- \"$@\"\n",
        s.name(),
        q(&launcher.display().to_string().replace('\\', "/")),
        q(id),
        q(&s.skill),
        q(&s.script)
    )
}

/// Make the declared scripts in an installed package runnable (a package from git or a tarball may not keep the mode).
pub fn mark_runnable(dir: &Path, m: &super::Manifest) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let skills = super::package_skills(dir, m).unwrap_or_default();
        for s in declared(&m.uses).unwrap_or_default() {
            if let Some((_, folder)) = skills.iter().find(|(n, _)| *n == s.skill) {
                let _ = std::fs::set_permissions(folder.join(&s.script), std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = (dir, m);
}

// ── running ──

/// Where a script's lock and its holder's process group are: one run of a script at a time across processes (the app's
/// host half, a session's `echoctl plugin script`), and any process can stop it. The system drops a lock with its process.
fn locks() -> PathBuf {
    crate::paths::config_dir().join("plugins").join(".locks").join("scripts")
}

fn lock_file(id: &str, s: &Script) -> PathBuf {
    // A script's path holds only letters, digits, `.`, `_`, `-` and `/`: `~` for `/` keeps it one name.
    locks().join(format!("{id}.{}.{}.lock", s.skill, s.script.replace('/', "~")))
}

fn group_file(lock: &Path) -> PathBuf {
    lock.with_extension("pid")
}

/// Stop plugin `id`'s running scripts, each with everything it started, whichever process runs it (unloaded, updated,
/// removed).
pub fn stop_plugin(id: &str) {
    stop_where(|name| name.starts_with(&format!("{id}.")));
}

/// Stop every running script (Echo quits).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn stop_all() {
    stop_where(|_| true);
}

fn stop_where(which: impl Fn(&str) -> bool) {
    for e in std::fs::read_dir(locks()).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.ends_with(".lock") || !which(&name) {
            continue;
        }
        // Held: a run is on; its group is in the file beside the lock. Not held: nothing to stop.
        if matches!(crate::app::FileLock::try_take(&e.path()), Ok(None)) {
            if let Some(pid) = std::fs::read_to_string(group_file(&e.path())).ok().and_then(|t| t.trim().parse::<u32>().ok()) {
                crate::ai::stop_group(pid);
            }
        }
    }
}

/// Run plugin `id`'s declared script `skill`/`script` with `args` (plain words, no shell), within `timeout` seconds (at
/// most what it declared, which is the default): `{status, stdout, stderr, truncated?, timed_out?}` — `status` null
/// when it was stopped. `read_only`: a read-only routine asks, which runs only a script marked `readonly`.
pub fn run(id: &str, skill: &str, script: &str, args: &[String], timeout: Option<u64>, read_only: bool) -> Result<Value> {
    let p = super::enabled().into_iter().find(|p| p.id == id).ok_or_else(|| anyhow!("{id} isn't installed and on"))?;
    let dir = super::plugin_dir(&p);
    let m = super::read_manifest(&dir)?;
    let script = script.trim_start_matches("./");
    let s = declared(&m.uses)?.into_iter().find(|s| s.skill == skill && s.script == script).ok_or_else(|| anyhow!("{id} didn't declare the script “{skill}/{script}” when it was installed"))?;
    if read_only && !s.readonly {
        bail!("{} isn't marked readonly: a read-only routine doesn't run it", s.name());
    }
    let limit = match timeout {
        None => s.timeout,
        Some(t) if (1..=s.timeout).contains(&t) => t,
        Some(t) => bail!("{} may take {} s at most, not {t}", s.name(), s.timeout),
    };
    if args.len() > MAX_ARGS || args.iter().any(|a| a.contains('\0') || a.len() > 32 * 1024) {
        bail!("a script takes up to {MAX_ARGS} arguments, each plain text of 32 KB at most");
    }
    // The original, from the package Echo installed; it runs in its skill's session copy.
    let (_, folder) = super::package_skills(&dir, &m)?.into_iter().find(|(n, _)| *n == s.skill).ok_or_else(|| anyhow!("{} isn't one of its skills", s.skill))?;
    let file = crate::grants::inside(&dunce::canonicalize(&folder)?, &s.script)?;
    let cwd = crate::grants::session_skill_dir(id, &s.skill)?;
    let cache = super::data_dir(id).join("cache");
    std::fs::create_dir_all(&cache)?;
    std::fs::create_dir_all(locks())?;
    let lock_path = lock_file(id, &s);
    let Some(_lock) = crate::app::FileLock::try_take(&lock_path)? else { bail!("{} is already running", s.name()) };
    let group = group_file(&lock_path);
    let out = spawn_and_wait(&file, args, &cwd, &cache, Duration::from_secs(limit), &group);
    let _ = std::fs::remove_file(&group);
    out
}

/// The command for a script file: itself (Unix: its `#!` line), or on Windows PowerShell for a `.ps1` and the
/// interpreter its `#!` line names for another.
fn command_for(file: &Path) -> Result<Command> {
    #[cfg(unix)]
    {
        let _ = interpreter(file);
        Ok(Command::new(file))
    }
    #[cfg(windows)]
    {
        if file.extension().is_some_and(|e| e.eq_ignore_ascii_case("ps1")) {
            let mut c = Command::new("powershell.exe");
            c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(file);
            return Ok(c);
        }
        let (prog, extra) = interpreter(file).ok_or_else(|| anyhow!("{} starts with no #! line naming what runs it", file.display()))?;
        let mut c = Command::new(crate::app::find_program(&prog));
        c.args(extra).arg(file);
        Ok(c)
    }
}

/// What a `#!` line names: `#!/usr/bin/env node` → `node`, `#!/bin/sh -e` → `sh` and `-e`.
fn interpreter(file: &Path) -> Option<(String, Vec<String>)> {
    let mut head = [0u8; 256];
    let n = std::fs::File::open(file).and_then(|mut f| f.read(&mut head)).ok()?;
    let line = String::from_utf8_lossy(&head[..n]).lines().next()?.strip_prefix("#!")?.trim().to_string();
    let mut words = line.split_whitespace().map(str::to_owned);
    let first = words.next()?;
    let mut rest: Vec<String> = words.collect();
    let name = first.rsplit(['/', '\\']).next()?.to_string();
    if name == "env" {
        rest.retain(|w| w != "-S");
        if rest.is_empty() {
            return None;
        }
        let prog = rest.remove(0);
        return Some((prog, rest));
    }
    Some((name, rest))
}

/// The PATH a script gets: the app's, then where programs usually are (the app started at login has a short one).
fn script_path() -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = vec![];
    for c in crate::app::program_candidates("x") {
        if let Some(d) = c.parent() {
            if !dirs.iter().any(|x| x == d) {
                dirs.push(d.to_path_buf());
            }
        }
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// What a script doesn't get of Echo's environment: Echo's and Claude Code's own (keys, profiles' values, run scopes) —
/// but test data's guard it does (`ECHOCTL_TEST_OPEN_LOG_ONLY`, `ECHOCTL_TEST_OPEN_LOG`), so a program it starts that
/// opens a page (a sign-in's browser) only logs it there, as Echo does. Echo's data and config folders and its binary
/// are set anew, and inside a routine or Yolo the run's markers (`spawn_and_wait`, `run_markers`).
fn withheld(key: &str) -> bool {
    let up = key.to_ascii_uppercase();
    if up == "ECHOCTL_TEST_OPEN_LOG_ONLY" || up == "ECHOCTL_TEST_OPEN_LOG" {
        return false;
    }
    up.starts_with("CLAUDE") || up.starts_with("ANTHROPIC") || up.starts_with("ECHOCTL")
}

/// Inside a routine's or Yolo's run, the run's markers, for an `echoctl` the script starts to be limited and recorded
/// as the run is (with the headless marker those limits read); none outside one.
fn run_markers() -> Vec<(&'static str, std::ffi::OsString)> {
    use crate::{guard, notes};
    let mut m: Vec<_> = [notes::RUN_SESSION, guard::RUN_MODE, notes::RUN_ACTIONS, guard::RUN_TASK, guard::RUN_THREAD]
        .into_iter()
        .filter_map(|k| std::env::var_os(k).map(|v| (k, v)))
        .collect();
    if !m.is_empty() {
        if let Some(v) = std::env::var_os(notes::HEADLESS) {
            m.push((notes::HEADLESS, v));
        }
    }
    m
}

fn spawn_and_wait(file: &Path, args: &[String], cwd: &Path, cache: &Path, limit: Duration, group: &Path) -> Result<Value> {
    let mut c = command_for(file)?.no_window();
    c.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // The user's environment, without what is Echo's or Claude Code's (keys, profiles' values, run scopes).
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy().to_string();
        if withheld(&k) {
            c.env_remove(&k);
        }
    }
    c.env("PATH", script_path()).env("ECHO_PLUGIN_SCRIPT", "1").env("ECHO_PLUGIN_CACHE", cache);
    // An `echoctl` it starts (the plugin's `scripts/echoctl`) is this Echo, on its data and config folders (#569); the
    // data folder only once one is chosen, so before setup it doesn't look done.
    if crate::paths::home_source() != "default" {
        c.env("ECHOCTL_HOME", crate::paths::home());
    }
    c.env("ECHOCTL_CONFIG_DIR", crate::paths::config_dir());
    if let Ok(exe) = std::env::current_exe() {
        c.env("ECHOCTL_BIN", exe);
    }
    for (k, v) in run_markers() {
        c.env(k, v);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        c.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    let mut child = c.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => match interpreter(file) {
            Some((prog, _)) => anyhow!("{} needs {prog}, which isn't on this computer", file.file_name().unwrap_or_default().to_string_lossy()),
            None => anyhow!("{} can't be started: {e}", file.display()),
        },
        std::io::ErrorKind::PermissionDenied => anyhow!("{} isn't a program Echo can start (is its #! line there?)", file.display()),
        _ => anyhow!("{} can't be started: {e}", file.display()),
    })?;
    // Its group (its own, led by it), for whoever stops it.
    let _ = std::fs::write(group, child.id().to_string());
    let read = |mut s: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let (mut kept, mut cut, mut buf) = (Vec::new(), false, [0u8; 8192]);
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let room = OUTPUT_CAP.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..n.min(room)]);
                cut |= n > room;
            }
            (kept, cut)
        })
    };
    let out = read(Box::new(child.stdout.take().expect("piped")));
    let err = read(Box::new(child.stderr.take().expect("piped")));
    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(st) = child.try_wait()? {
            break st;
        }
        if started.elapsed() >= limit {
            timed_out = true;
            crate::ai::stop_group(child.id());
            let _ = child.kill();
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    // What the script started may still hold its output open: then (the group still has a process, so its id is still
    // its) stop the group before reading to the end. A group stopped on its time is gone already.
    let ends = Instant::now();
    while !(out.is_finished() && err.is_finished()) && ends.elapsed() < Duration::from_secs(1) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let read_all = out.is_finished() && err.is_finished();
    if !timed_out && !read_all {
        crate::ai::stop_group(child.id());
    }
    let ((stdout, cut_out), (stderr, cut_err)) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    let mut v = json!({
        "status": if timed_out { Value::Null } else { status.code().map(Value::from).unwrap_or(Value::Null) },
        "stdout": String::from_utf8_lossy(&stdout),
        "stderr": String::from_utf8_lossy(&stderr),
    });
    if cut_out || cut_err {
        v["truncated"] = json!(true);
    }
    if timed_out {
        v["timed_out"] = json!(true);
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_is_declared_by_its_skill_and_a_plain_path_with_its_time() {
        let uses = |v: Value| json!({"scripts": v}).as_object().cloned().unwrap();
        let got = declared(&uses(json!([{"skill": "staffing", "script": "scripts/read.sh", "readonly": true}, {"skill": "staffing", "script": "./scripts/sync.mjs", "timeout": 1800}]))).unwrap();
        assert_eq!(got[0], Script { skill: "staffing".into(), script: "scripts/read.sh".into(), readonly: true, timeout: 300 });
        assert_eq!((got[1].script.as_str(), got[1].readonly, got[1].timeout), ("scripts/sync.mjs", false, 1800));
        assert_eq!(
            describe_scripts(&json!([{"skill": "staffing", "script": "scripts/read.sh", "readonly": true}, {"skill": "staffing", "script": "scripts/sync.mjs", "timeout": 1800}])).unwrap(),
            "Runs programs from its package on this computer with your rights — they can reach the network and anything you can: “staffing/scripts/read.sh” (read-only routines may run it; Echo can't check it only reads), “staffing/scripts/sync.mjs” (up to 30 min)"
        );
        for bad in [
            json!([]),
            json!({"skill": "s"}),
            json!([{"skill": "s", "script": "../x.sh"}]),
            json!([{"skill": "s", "script": "/bin/sh"}]),
            json!([{"skill": "s", "script": "a b.sh"}]),
            json!([{"skill": "s", "script": "scripts/.hidden"}]),
            json!([{"skill": "S", "script": "x.sh"}]),
            json!([{"skill": "s", "script": "x.sh", "timeout": 3601}]),
            json!([{"skill": "s", "script": "x.sh", "timeout": 0}]),
            json!([{"skill": "s", "script": "x.sh", "readonly": "yes"}]),
            json!([{"skill": "s", "script": "x.sh", "shell": true}]),
            json!([{"skill": "s", "script": "x.sh"}, {"skill": "s", "script": "./x.sh"}]),
        ] {
            assert!(declared(&uses(bad.clone())).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_script_gets_test_datas_open_guard_but_nothing_else_of_echos_or_claudes() {
        for k in ["ECHOCTL_TEST_OPEN_LOG_ONLY", "ECHOCTL_TEST_OPEN_LOG"] {
            assert!(!withheld(k), "{k}");
        }
        for k in ["ECHOCTL_HOME", "ECHOCTL_CONFIG", "ECHOCTL_TEST_OPEN_LOG_ONLYX", "echoctl_secret", "CLAUDE_CONFIG_DIR", "ANTHROPIC_API_KEY", "ClaudeCode"] {
            assert!(withheld(k), "{k}");
        }
        assert!(!withheld("HOME") && !withheld("PATH"));
    }

    #[cfg(unix)]
    #[test]
    fn an_echoctl_a_script_starts_is_this_echo_on_its_data_and_config_folders_with_its_secrets_still_withheld() {
        let (_env, root) = crate::paths::testenv::folders("scripts-echo-folders");
        let f = root.join("env.sh");
        std::fs::write(&f, "#!/bin/sh\nprintf '%s|%s|%s|%s' \"$ECHOCTL_HOME\" \"$ECHOCTL_CONFIG_DIR\" \"$ECHOCTL_BIN\" \"${ECHOCTL_P_TEST_KEY:-none}\"\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var("ECHOCTL_P_TEST_KEY", "secret");
        let out = spawn_and_wait(&f, &[], &root, &root, Duration::from_secs(10), &root.join("group"));
        std::env::remove_var("ECHOCTL_P_TEST_KEY");
        let out = out.unwrap();
        let exe = std::env::current_exe().unwrap();
        let want = format!("{}|{}|{}|none", crate::paths::home().display(), crate::paths::config_dir().display(), exe.display());
        assert_eq!(out["stdout"], want, "{out}");
        // Before setup (no data folder chosen) it gets no data folder: its echoctl sees setup isn't done.
        std::env::remove_var("ECHOCTL_HOME");
        assert_eq!(crate::paths::home_source(), "default");
        let out = spawn_and_wait(&f, &[], &root, &root, Duration::from_secs(10), &root.join("group")).unwrap();
        assert_eq!(out["stdout"], format!("|{}|{}|none", crate::paths::config_dir().display(), exe.display()), "{out}");
    }

    #[cfg(unix)]
    #[test]
    fn a_script_in_a_routine_or_yolo_gets_the_runs_markers_and_outside_one_none() {
        use crate::{guard, notes};
        let (_env, root) = crate::paths::testenv::folders("scripts-run-markers");
        let f = root.join("env.sh");
        std::fs::write(&f, "#!/bin/sh\nenv | grep -E '^ECHOCTL_(RUN_|HEADLESS)' | sort\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
        let keys = [notes::HEADLESS, notes::RUN_SESSION, guard::RUN_MODE, notes::RUN_ACTIONS, guard::RUN_TASK, guard::RUN_THREAD];
        let run = || spawn_and_wait(&f, &[], &root, &root, Duration::from_secs(10), &root.join("group")).unwrap()["stdout"].as_str().unwrap().to_string();
        // Outside a routine or Yolo: none (a minutes run's headless marker alone isn't passed either).
        std::env::set_var(notes::HEADLESS, "1");
        let outside = run();
        let vals = ["1", "asks", "routine", "/tmp/run/actions.jsonl", "pilot/T005", "pilot"];
        for (k, v) in keys.iter().zip(vals) {
            std::env::set_var(k, v);
        }
        let inside = run();
        for k in keys {
            std::env::remove_var(k);
        }
        assert_eq!(outside, "", "{outside}");
        let want: Vec<String> = {
            let mut w: Vec<String> = keys.iter().zip(vals).map(|(k, v)| format!("{k}={v}")).collect();
            w.sort();
            w
        };
        assert_eq!(inside.lines().collect::<Vec<_>>(), want, "{inside}");
    }

    #[test]
    fn a_shebang_names_the_interpreter() {
        let (_env, root) = crate::paths::testenv::folders("scripts-shebang");
        let f = root.join("s");
        for (line, want) in [
            ("#!/usr/bin/env node\n", Some(("node", vec![]))),
            ("#!/usr/bin/env -S python3 -u\n", Some(("python3", vec!["-u"]))),
            ("#!/bin/sh -e\n", Some(("sh", vec!["-e"]))),
            ("echo hi\n", None),
        ] {
            std::fs::write(&f, line).unwrap();
            let got = interpreter(&f);
            assert_eq!(got.as_ref().map(|(p, a)| (p.as_str(), a.iter().map(String::as_str).collect::<Vec<_>>())), want, "{line}");
        }
    }
}

//! Is Claude Code really set up for Echo? Found is not enough (2026-11-04: an install whose `claude` didn't run, or
//! that a terminal couldn't find, and a Connect that never checked the plugin went in). Three facts, checked off the
//! UI's thread and kept a minute: Claude Code runs (`claude --version`), a terminal finds it (the login shell's
//! `command -v claude`; Windows: the PATH), and Claude Code lists Echo's plugin where Echo's sessions see it.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::app::NoWindow;

/// The last check and when it was made; one check runs at a time.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
static LAST: Mutex<Option<(Instant, Value)>> = Mutex::new(None);
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const FRESH: Duration = Duration::from_secs(60);

/// The last check (`{checking: true}` before the first one), starting another in the background when it is older
/// than a minute or `again`.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn status(again: bool) -> Value {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let stale = again || last.as_ref().is_none_or(|(at, _)| at.elapsed() > FRESH);
    if stale && !RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(|| {
            let v = check();
            *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), v));
            RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
    match last {
        Some((_, mut v)) => {
            v["checking"] = json!(stale);
            v
        }
        None => json!({"checking": true}),
    }
}

/// The check itself (seconds: it runs Claude Code and the login shell).
pub fn check() -> Value {
    let status = crate::app::claude_status();
    let path = crate::app::claude_bin();
    let (version, error) = match &path {
        Some(p) => match version_of(p) {
            Ok(v) => (Some(v), None),
            Err(e) => (None, Some(format!("{e:#}"))),
        },
        None => (None, None),
    };
    let terminal = path.as_ref().map(|p| in_terminal(p));
    json!({
        "path": path,
        "version": version,
        // Found but `claude --version` failed: it can't write minutes either.
        "error": error,
        // {ok, found: what a terminal runs, why}: None when Claude Code isn't installed at all.
        "terminal": terminal,
        "plugin": status["plugin_installed"],
        "installs": status["installs"],
        "stale": crate::app::stale_plugins(),
    })
}

/// `claude --version`, at most 15 s. Its own folder goes first on the PATH: an npm install is a node script, and
/// node sits beside it.
pub fn version_of(claude: &Path) -> Result<String> {
    let mut c = Command::new(claude).no_window();
    c.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = claude.parent() {
        let rest = std::env::var_os("PATH").unwrap_or_default();
        let joined = std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(std::env::split_paths(&rest))).unwrap_or(rest);
        c.env("PATH", joined);
    }
    let out = run_bounded(c, Duration::from_secs(15))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() || text.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("`{} --version` failed{}", claude.display(), if err.is_empty() { String::new() } else { format!(": {}", err.lines().last().unwrap_or("")) });
    }
    Ok(text.lines().next().unwrap_or("").to_string())
}

/// Does a terminal find this `claude`? macOS / Linux: the login shell's `command -v claude` (between markers: its
/// startup files may print); Windows: the PATH Echo was started with (a new terminal's).
fn in_terminal(claude: &Path) -> Value {
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
        let mut c = Command::new(&shell);
        c.args(["-l", "-i", "-c", "printf '<<echo:claude>>%s<</echo:claude>>' \"$(command -v claude)\""]).current_dir(crate::paths::user_home());
        crate::ai::detach(&mut c);
        let found = match run_bounded(c, Duration::from_secs(15)) {
            Ok(o) => {
                let t = String::from_utf8_lossy(&o.stdout).to_string();
                match t.rsplit_once("<<echo:claude>>").and_then(|(_, r)| r.split_once("<</echo:claude>>")) {
                    Some((f, _)) => f.trim().to_string(),
                    None => return json!({"ok": null, "why": "your login shell gave no answer"}),
                }
            }
            Err(e) => return json!({"ok": null, "why": format!("{e:#}")}),
        };
        let ok = !found.is_empty();
        json!({"ok": ok, "found": found, "dir": claude.parent(), "shell": shell, "rc": rc_file(&shell)})
    }
    #[cfg(windows)]
    {
        let dir = claude.parent().map(Path::to_path_buf);
        let path = std::env::var_os("PATH").unwrap_or_default();
        let ok = dir.as_ref().is_some_and(|d| std::env::split_paths(&path).any(|p| p.eq(d)));
        json!({"ok": ok, "found": if ok { claude.display().to_string() } else { String::new() }, "dir": dir})
    }
}

/// Run `c` with its output piped, stopped after `limit`.
fn run_bounded(mut c: Command, limit: Duration) -> Result<std::process::Output> {
    c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let child = c.spawn()?;
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(limit) {
        Ok(r) => Ok(r?),
        Err(_) => {
            crate::ai::stop_group(pid);
            bail!("no answer within {} s", limit.as_secs())
        }
    }
}

/// The startup file a login shell reads that Echo may add a PATH line to: zsh's `.zshrc`, bash's `.bash_profile`.
/// None for another shell (the user adds it themselves).
#[cfg_attr(windows, allow(dead_code))]
fn rc_file(shell: &str) -> Option<PathBuf> {
    let home = crate::paths::user_home();
    match Path::new(shell).file_name()?.to_str()? {
        "zsh" => Some(home.join(".zshrc")),
        "bash" => Some(home.join(".bash_profile")),
        _ => None,
    }
}

/// Settings → Health's "Add to PATH": Claude Code's folder for new terminals. macOS / Linux: one marked line at the
/// end of the login shell's startup file (once); Windows: the user PATH. What was done, for the notification.
pub fn add_to_path() -> Result<String> {
    let Some(claude) = crate::app::claude_bin() else { bail!("Claude Code is not installed") };
    let Some(dir) = claude.parent().map(Path::to_path_buf) else { bail!("no folder for {}", claude.display()) };
    #[cfg(unix)]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let Some(rc) = rc_file(&shell) else {
            bail!("add {} to your PATH in your shell's startup file ({} isn't zsh or bash)", dir.display(), shell)
        };
        append_path_line(&rc, &dir)?;
        Ok(format!("{} is on the PATH of new terminals ({}).", dir.display(), rc.display()))
    }
    #[cfg(windows)]
    {
        let d = dir.display().to_string();
        let script = format!(
            "$p = [Environment]::GetEnvironmentVariable('Path','User'); \
             if (($p -split ';') -notcontains '{d}') {{ [Environment]::SetEnvironmentVariable('Path', ($p.TrimEnd(';') + ';{d}'), 'User') }}",
            d = d.replace('\'', "''")
        );
        let ok = Command::new("powershell").no_window().args(["-NoProfile", "-Command", &script]).status()?.success();
        if !ok {
            bail!("could not update the user PATH");
        }
        Ok(format!("{d} is on the PATH of new terminals."))
    }
}

/// `export PATH="<dir>:$PATH"`, marked as Echo's, at the end of `rc` — once.
#[cfg_attr(windows, allow(dead_code))]
fn append_path_line(rc: &Path, dir: &Path) -> Result<()> {
    let d = dir.display().to_string();
    let have = std::fs::read_to_string(rc).unwrap_or_default();
    if have.lines().any(|l| l.contains("# added by Echo") && l.contains(&d)) {
        return Ok(());
    }
    let line = format!("{}export PATH=\"{}:$PATH\"  # added by Echo: Claude Code\n", if have.is_empty() || have.ends_with('\n') { "" } else { "\n" }, d.replace('"', "\\\""));
    use std::io::Write;
    std::fs::OpenOptions::new().create(true).append(true).open(rc)?.write_all(line.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_line_is_added_once_after_what_is_there() {
        let dir = std::env::temp_dir().join(format!("echo-test-rc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let rc = dir.join(".zshrc");
        std::fs::write(&rc, "alias ll='ls -l'").unwrap();
        append_path_line(&rc, Path::new("/x/bin")).unwrap();
        append_path_line(&rc, Path::new("/x/bin")).unwrap();
        let t = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(t, "alias ll='ls -l'\nexport PATH=\"/x/bin:$PATH\"  # added by Echo: Claude Code\n");
        // No file yet: made.
        let fresh = dir.join(".bash_profile");
        append_path_line(&fresh, Path::new("/y")).unwrap();
        assert!(std::fs::read_to_string(&fresh).unwrap().starts_with("export PATH=\"/y:$PATH\""));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! AI profiles: Claude Code's managed policy and hooks that would override a profile (split out of `ai.rs`, moved as it was).

use super::*;

/// Admin-managed Claude Code policy always applies, and its `env` beats the environment Echo
/// sets. If a policy source could route requests (a routing variable, an API key or policy
/// helper), or exists but can't be read, a Claude Code run for a `local` / `claude` profile is
/// refused rather than sent somewhere other than the profile shown (`local` live jobs don't start
/// Claude Code). Sources: the managed settings files, macOS managed preferences, Windows policy
/// keys (not parsed: any key refuses), and for a `claude` profile the server-managed settings
/// Claude Code cached in the profile's own folder.
pub fn policy_conflict(p: &Profile) -> Option<String> {
    const WHY: &str = "your organisation's Claude Code policy decides where requests go — use the \"Claude Code (as configured)\" profile";
    let mut sources: Vec<(String, Result<Value, String>)> = vec![];
    // A source counts unless it surely isn't there: "can't tell" refuses.
    let present = |f: &Path| !matches!(f.try_exists(), Ok(false));
    let files = match managed_files() {
        Ok(f) => f,
        Err(e) => return Some(format!("{WHY} ({e})")),
    };
    for f in files {
        if present(&f) {
            sources.push((f.display().to_string(), read_policy_json(&f)));
        }
    }
    #[cfg(target_os = "macos")]
    for f in managed_plists() {
        if present(&f) {
            let parsed = Command::new("plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&f)
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).map_err(|e| e.to_string()));
            sources.push((f.display().to_string(), parsed));
        }
    }
    #[cfg(windows)]
    for key in [r"HKLM\SOFTWARE\Policies\ClaudeCode", r"HKCU\SOFTWARE\Policies\ClaudeCode"] {
        use crate::app::NoWindow;
        match Command::new("reg").no_window().args(["query", key]).output() {
            Ok(o) if !o.status.success() => {}
            _ => return Some(format!("{WHY} ({key})")),
        }
    }
    if p.kind.signs_in() {
        let f = p.config_folder().join("remote-settings.json");
        if present(&f) {
            sources.push((f.display().to_string(), read_policy_json(&f)));
        }
    }
    for (name, v) in sources {
        match v {
            Err(_) => {
                if let Some(why) = unread(WHY, &name) {
                    return Some(why);
                }
            }
            Ok(v) if routes(&v) => return Some(format!("{WHY} ({name})")),
            Ok(_) => {}
        }
    }
    None
}

/// The minutes run's guard is a settings-file hook (`guard.rs`): admin policy that switches those
/// off (`disableAllHooks`, `allowManagedHooksOnly`), or that can't be read, refuses the run —
/// whatever the profile. Sources: the managed settings files, macOS managed preferences, Windows
/// policy keys (their `Settings` JSON; one that can't be read refuses), and the server-managed
/// settings Claude Code cached
/// in the profile's folder (`system`: every folder its Claude Code may use, `system_config_dirs`).
/// Claude Code fetches those again when it starts: an organisation turning such a policy on is
/// seen from the run after that (known residual).
/// Ok: the Claude Code settings folders checked (for the run's log).
pub fn hooks_conflict(p: &Profile, cwd: &Path) -> Result<Vec<PathBuf>, String> {
    const WHY: &str = "the minutes run needs Echo's guard (a Claude Code hook), and a Claude Code policy switches hooks off or can't be read — ask for hooks to be allowed, or run /echo:process-call in a Claude Code session";
    let present = |f: &Path| !matches!(f.try_exists(), Ok(false));
    let mut files = match managed_files() {
        Ok(f) => f,
        Err(e) => return Err(format!("{WHY} ({e})")),
    };
    let folders = if p.kind == Kind::System {
        match system_config_dirs(cwd) {
            Ok(f) => f,
            Err(e) => return Err(format!("Echo can't tell which Claude Code settings the minutes run would use ({e}), so it can't check they allow its guard")),
        }
    } else {
        vec![p.config_folder()]
    };
    files.extend(folders.iter().map(|f| f.join("remote-settings.json")));
    #[cfg_attr(not(any(target_os = "macos", windows)), allow(unused_mut))] // macOS and Windows add their own
    let mut sources: Vec<(String, Result<Value, String>)> = files.iter().filter(|f| present(f)).map(|f| (f.display().to_string(), read_policy_json(f))).collect();
    #[cfg(target_os = "macos")]
    for f in managed_plists() {
        if present(&f) {
            let parsed = Command::new("plutil")
                .args(["-convert", "json", "-o", "-"])
                .arg(&f)
                .output()
                .map_err(|e| e.to_string())
                .and_then(|o| serde_json::from_slice::<Value>(&o.stdout).map_err(|e| e.to_string()));
            sources.push((f.display().to_string(), parsed));
        }
    }
    #[cfg(windows)]
    for (hklm, key) in [(true, r"HKLM\SOFTWARE\Policies\ClaudeCode"), (false, r"HKCU\SOFTWARE\Policies\ClaudeCode")] {
        match windows_policy(hklm) {
            Ok(None) => {}
            Ok(Some(json)) => sources.push((key.to_string(), serde_json::from_str::<Value>(&json).map_err(|e| e.to_string()))),
            Err(e) => return Err(format!("{WHY} ({key} can't be read: {e})")),
        }
    }
    for (name, v) in sources {
        match v {
            Err(_) => {
                if let Some(why) = unread(WHY, &name) {
                    return Err(why);
                }
            }
            Ok(v) if restricts_hooks(&v) => return Err(format!("{WHY} ({name})")),
            Ok(_) => {}
        }
    }
    Ok(folders)
}

/// The folders the built-in profile's Claude Code may keep its settings in: what its login shell
/// sets `CLAUDE_CONFIG_DIR` to (the run starts through that shell), Echo's own value, and
/// `~/.claude` — all are checked, since Echo can't tell which one the run will use. The shell is
/// asked in the run's own folder (`cwd`), as the run starts there. Err when it can't say (it fails,
/// hangs or prints no bounded answer): the check can't be made.
pub(super) fn system_config_dirs(cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let mut v = vec![];
    if use_shell() && fake_claude().is_none() {
        let shell = std::env::var("SHELL").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "/bin/zsh".into());
        let mut c = Command::new(shell);
        // Its folder after its startup files (they may `cd`; Claude Code starts there) and its value.
        c.args(["-l", "-i", "-c", "printf '<<echo:config-dir>>%s\\n%s<</echo:config-dir>>' \"$PWD\" \"$CLAUDE_CONFIG_DIR\""])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        detach(&mut c);
        let child = c.spawn().map_err(|e| format!("your login shell can't be started: {e}"))?;
        let pid = child.id();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });
        let out = match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(format!("your login shell failed: {e}")),
            Err(_) => {
                stop_group(pid);
                return Err("your login shell didn't answer within 15 s".into());
            }
        };
        let text = String::from_utf8_lossy(&out.stdout);
        let (pwd, d) = config_dir_answer(&text).ok_or("your login shell didn't say which settings folder it uses")?;
        // A relative one as Claude Code sees it: from the folder the shell ends up in.
        v.extend(Some(d).filter(|d| !d.is_empty()).map(|d| cwd.join(pwd).join(d)));
    }
    v.extend(std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()).map(|d| cwd.join(d)));
    v.push(paths::user_home().join(".claude"));
    v.dedup();
    Ok(v)
}

/// The shell's folder and `CLAUDE_CONFIG_DIR`, between the probe's two markers (what the shell
/// prints before or after — its rc files, a logout script — stays out); None without both.
pub(super) fn config_dir_answer(out: &str) -> Option<(&str, &str)> {
    let (_, rest) = out.rsplit_once("<<echo:config-dir>>")?;
    rest.split_once("<</echo:config-dir>>")?.0.split_once('\n')
}

/// Claude Code's Windows policy: the JSON in `<hive>\SOFTWARE\Policies\ClaudeCode`, value
/// `Settings`. None when there is none; Err when it is there but can't be read (access denied, …).
#[cfg(windows)]
pub(super) fn windows_policy(hklm: bool) -> Result<Option<String>, String> {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_NOEXPAND, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ};
    let hive = if hklm { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER };
    let (key, value) = (w!(r"SOFTWARE\Policies\ClaudeCode"), w!("Settings"));
    let mut len = 0u32;
    // SAFETY: plain registry reads into a buffer of the size the first call reports.
    let r = unsafe { RegGetValueW(hive, key, value, RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND, None, None, Some(&mut len)) };
    if r == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if r.is_err() {
        return Err(format!("Windows error {}", r.0));
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2)];
    let r = unsafe { RegGetValueW(hive, key, value, RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | RRF_NOEXPAND, None, Some(buf.as_mut_ptr() as *mut _), Some(&mut len)) };
    if r.is_err() {
        return Err(format!("Windows error {}", r.0));
    }
    Ok(Some(String::from_utf16_lossy(&buf).trim_end_matches('\0').to_string()))
}

/// A policy document that switches settings-file hooks off, or a helper that may. Server-managed
/// settings may wrap the settings in a `settings` object.
pub(super) fn restricts_hooks(v: &Value) -> bool {
    // A policy helper decides the settings when Claude Code starts: Echo can't tell what it allows.
    let doc = |v: &Value| v["disableAllHooks"] == true || v["allowManagedHooksOnly"] == true || v.get("policyHelper").is_some() || v.get("policyHelpers").is_some();
    doc(v) || doc(&v["settings"])
}

thread_local! {
    /// The policy files the user let a run go without, for the runs this thread starts (`without_policy`).
    static WITHOUT: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `f` with these unreadable policy files let go (the user's yes for the call, #404): a run doesn't wait on a file
/// Claude Code can't read either.
pub fn without_policy<R>(files: &[String], f: impl FnOnce() -> R) -> R {
    WITHOUT.with(|w| *w.borrow_mut() = files.to_vec());
    let r = f();
    WITHOUT.with(|w| w.borrow_mut().clear());
    r
}

fn let_go(name: &str) -> bool {
    WITHOUT.with(|w| w.borrow().iter().any(|x| x == name))
}

fn denied(name: &str) -> bool {
    fs::read(name).is_err_and(|e| e.kind() == std::io::ErrorKind::PermissionDenied)
}

fn machine_wide(name: &str) -> bool {
    managed_files().is_ok_and(|f| f.iter().any(|x| x.display().to_string() == name))
}

/// A policy file that can't be read (#404), per Claude Code's own rule (code.claude.com/docs/en/managed-settings): a
/// managed file the system won't let it read is left out — so a run goes on when the user said yes for it; one it
/// reads but can't take (not a JSON object, an I/O error) stops Claude Code from starting at all — a run can't help
/// there. Any other (a profile's cached server settings) refuses until the user chooses (`unreadable`). None: let go.
fn unread(why: &str, name: &str) -> Option<String> {
    let machine = machine_wide(name);
    // Only a file the system won't let this user read: Claude Code, run as the same user, can't read it either.
    if let_go(name) && denied(name) {
        return None;
    }
    if machine && !denied(name) {
        return Some(format!("Claude Code won't start while the policy file {name} is as it is (it isn't a JSON object it can read) — ask your admin to fix it"));
    }
    Some(format!("{why} ({name} can't be read)"))
}

/// What the user may choose when a run was refused for a policy file it couldn't read (#404): `"anyway"` — a file the
/// system won't let this user read (Claude Code, as the same user, starts without it); `"system"` — a profile's own
/// cached server settings that can't be taken: write on Claude Code (as configured) instead. None: nothing to offer
/// (a cache of the built-in profile's own that isn't JSON: Claude Code fetches those settings again itself).
pub fn unreadable(p: &Profile, cwd: &Path) -> Option<(String, &'static str)> {
    let bad = |f: &Path| f.try_exists().unwrap_or(true) && read_policy_json(f).is_err() && !let_go(&f.display().to_string());
    let shut = |f: &Path| bad(f) && denied(&f.display().to_string());
    if let Some(f) = managed_files().ok()?.into_iter().find(|f| shut(f)) {
        return Some((f.display().to_string(), "anyway"));
    }
    if p.kind == Kind::System {
        let dirs = system_config_dirs(cwd).ok()?;
        return dirs.iter().map(|d| d.join("remote-settings.json")).find(|f| shut(f)).map(|f| (f.display().to_string(), "anyway"));
    }
    let f = p.config_folder().join("remote-settings.json");
    bad(&f).then(|| (f.display().to_string(), "system"))
}

pub(super) fn read_policy_json(f: &Path) -> Result<Value, String> {
    fs::read(f).map_err(|e| e.to_string()).and_then(|b| serde_json::from_slice(&b).map_err(|e| e.to_string()))
}

/// A policy document that can decide the route: a routing variable in its `env`, or a helper.
/// Server-managed settings may wrap the settings in a `settings` object.
pub(super) fn routes(v: &Value) -> bool {
    let doc = |v: &Value| {
        v["env"].as_object().is_some_and(|env| env.keys().any(|k| routing_var(k)))
            || v.get("apiKeyHelper").is_some()
            || v.get("policyHelper").is_some()
            || v.get("policyHelpers").is_some()
    };
    doc(v) || doc(&v["settings"])
}

/// The managed settings files; an error when the drop-in folder exists but can't be listed.
pub(super) fn managed_files() -> Result<Vec<PathBuf>, String> {
    let base = PathBuf::from(if cfg!(target_os = "macos") {
        "/Library/Application Support/ClaudeCode"
    } else if cfg!(windows) {
        r"C:\Program Files\ClaudeCode"
    } else {
        "/etc/claude-code"
    });
    let mut files = vec![base.join("managed-settings.json")];
    let dropins = base.join("managed-settings.d");
    match fs::read_dir(&dropins) {
        Ok(rd) => {
            let mut more: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "json")).collect();
            more.sort();
            files.extend(more);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{} can't be read: {e}", dropins.display())),
    }
    Ok(files)
}

#[cfg(target_os = "macos")]
pub(super) fn managed_plists() -> Vec<PathBuf> {
    let root = Path::new("/Library/Managed Preferences");
    let user = std::env::var("USER").ok().filter(|u| !u.is_empty()).or_else(|| {
        std::env::var_os("HOME").map(PathBuf::from).and_then(|h| h.file_name().map(|n| n.to_string_lossy().into_owned()))
    });
    let mut v = vec![root.join("com.anthropic.claudecode.plist")];
    if let Some(u) = user {
        v.push(root.join(u).join("com.anthropic.claudecode.plist"));
    }
    v
}

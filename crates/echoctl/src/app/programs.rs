//! The app: programs, Claude Code and its plugin, the command-line tool (split out of `app.rs`, moved as it was).

use super::*;

// ── programs, Claude Code, command-line tool ───────────────────────────────────────────────

pub(super) fn user_home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

/// A program by name: PATH first, then the usual install locations — the app started at login
/// gets a minimal PATH without Homebrew's, nvm's or Claude Code's directories.
pub fn find_program(name: &str) -> String {
    let home = user_home();
    let names: Vec<String> = if cfg!(windows) { vec![format!("{name}.exe"), format!("{name}.cmd")] } else { vec![name.to_string()] };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend([home.join(".local/bin"), home.join(".claude/local"), home.join(".npm-global/bin")]);
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    if let Some(d) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(d).join("npm"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        dirs.push(PathBuf::from(pf).join("nodejs"));
    }
    // nvm: newest installed version first
    let mut versions: Vec<PathBuf> = fs::read_dir(home.join(".nvm/versions/node"))
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path().join("bin"))).collect())
        .unwrap_or_default();
    versions.sort();
    dirs.extend(versions.into_iter().rev());
    dirs.iter()
        .flat_map(|d| names.iter().map(move |n| d.join(n)))
        .find(|p| p.is_file())
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| names[0].clone())
}

/// Where the plugin's marketplace comes from (`ECHOCTL_MARKETPLACE` overrides, e.g. a local
/// checkout for development).
pub(super) fn marketplace_source() -> String {
    std::env::var("ECHOCTL_MARKETPLACE").unwrap_or_else(|_| "onetest-ai/echo".into())
}
pub(super) const MARKETPLACE: &str = "onetest";

pub fn claude_bin() -> Option<PathBuf> {
    let p = PathBuf::from(find_program("claude"));
    p.is_file().then_some(p)
}

/// Claude Code's record of its plugins: in `CLAUDE_CONFIG_DIR` when set (a terminal that exports it installs
/// there), else `~/.claude`.
pub(super) fn installed_plugins_file() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()).map(PathBuf::from).unwrap_or_else(|| user_home().join(".claude")).join("plugins/installed_plugins.json")
}

/// The Echo plugin's installs, from Claude Code's own records: [(scope, project)].
pub(super) fn plugin_installs() -> Vec<(String, Option<String>)> {
    let path = installed_plugins_file();
    let v: Value = fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let plugins = v.get("plugins").unwrap_or(&v);
    plugins
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k.starts_with("echo@"))
        .flat_map(|(_, list)| list.as_array().cloned().unwrap_or_default())
        .map(|i| (i["scope"].as_str().unwrap_or("?").to_string(), i["projectPath"].as_str().map(str::to_owned)))
        .collect()
}

/// Echo plugin installs older than this Echo: [{scope, project, version}]. An old plugin's terminal
/// pane did things the app does now (it claimed every transcribed call for its own minutes).
pub fn stale_plugins() -> Vec<Value> {
    let path = installed_plugins_file();
    let v: Value = fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let plugins = v.get("plugins").unwrap_or(&v);
    plugins
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k.starts_with("echo@"))
        .flat_map(|(k, list)| list.as_array().cloned().unwrap_or_default().into_iter().map(move |i| (k.clone(), i)))
        .filter(|(_, i)| i["version"].as_str().is_some_and(|ver| crate::update::newer(env!("CARGO_PKG_VERSION"), ver)))
        .map(|(k, i)| serde_json::json!({"plugin": k, "scope": i["scope"], "project": i["projectPath"], "version": i["version"]}))
        .collect()
}

/// Bring every Echo plugin install up to date: the marketplace first, then each install in its scope
/// (a project's from its folder). What each step said, for the log; an error when one failed.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn update_plugins() -> Result<String> {
    let claude = claude_bin().ok_or_else(|| anyhow::anyhow!("Claude Code is not installed"))?;
    let stale = stale_plugins();
    let mut said = String::new();
    let run = |args: &[&str], cwd: Option<&str>, said: &mut String| -> Result<()> {
        let mut c = std::process::Command::new(&claude);
        c.args(args).stdin(std::process::Stdio::null());
        if let Some(d) = cwd.filter(|d| Path::new(d).is_dir()) {
            c.current_dir(d);
        }
        let o = c.output()?;
        said.push_str(&String::from_utf8_lossy(&o.stdout));
        said.push_str(&String::from_utf8_lossy(&o.stderr));
        anyhow::ensure!(o.status.success(), "claude {} failed", args.join(" "));
        Ok(())
    };
    let markets: std::collections::BTreeSet<String> = stale.iter().filter_map(|s| s["plugin"].as_str()?.split_once('@').map(|(_, m)| m.to_string())).collect();
    for m in &markets {
        run(&["plugin", "marketplace", "update", m], None, &mut said)?;
    }
    for s in &stale {
        let (Some(p), Some(scope)) = (s["plugin"].as_str(), s["scope"].as_str()) else { continue };
        run(&["plugin", "update", p, "--scope", scope], s["project"].as_str(), &mut said)?;
    }
    Ok(said)
}

/// Where Claude Code installed the Echo plugin (the newest user-scope install, else any).
pub fn plugin_root() -> Option<PathBuf> {
    let path = installed_plugins_file();
    let v: Value = fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let plugins = v.get("plugins").unwrap_or(&v);
    let mut installs: Vec<Value> = plugins
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k.starts_with("echo@"))
        .flat_map(|(_, list)| list.as_array().cloned().unwrap_or_default())
        .collect();
    installs.sort_by_key(|i| i["scope"].as_str() != Some("user"));
    installs.iter().filter_map(|i| i["installPath"].as_str()).map(PathBuf::from).find(|p| p.is_dir())
}

/// The same folder, after links (`/private/var` vs `/var` on macOS).
pub(super) fn same_folder(a: &Path, b: &Path) -> bool {
    dunce::canonicalize(a).ok().zip(dunce::canonicalize(b).ok()).is_some_and(|(a, b)| a == b) || a == b
}

/// `echoctl claude`: is Claude Code installed, and is the plugin?
pub fn claude_status() -> Value {
    let installs = plugin_installs();
    json!({
        "claude": claude_bin(),
        // Where Echo's sessions see it: every session (user scope), or the data folder (project / local scope there).
        "plugin_installed": installs.iter().any(|(s, p)| s == "user" || p.as_deref().is_some_and(|p| same_folder(Path::new(p), &paths::home()))),
        "installs": installs.iter().map(|(s, p)| json!({ "scope": s, "project": p })).collect::<Vec<_>>(),
        "marketplace": marketplace_source(),
    })
}

/// `echoctl claude connect`: add the marketplace and install the plugin at `scope` — by default the project scope
/// of Echo's data folder, where Echo's own Claude sessions run: Claude Code elsewhere doesn't need Echo.
pub fn connect_claude(scope: &str, project: Option<&Path>) -> Result<()> {
    let Some(claude) = claude_bin() else {
        bail!("Claude Code is not installed (no `claude` command found) — see https://claude.com/claude-code");
    };
    // Found isn't working: a `claude` that doesn't start can't install anything either.
    crate::claude_setup::version_of(&claude).map_err(|e| anyhow::anyhow!("Claude Code at {} doesn't run: {e:#}", claude.display()))?;
    let cwd = match project {
        Some(p) => p.to_path_buf(),
        None if scope == "user" => std::env::current_dir().unwrap_or_else(|_| user_home()),
        None => {
            let home = paths::home();
            std::fs::create_dir_all(&home)?;
            home
        }
    };
    let run = |args: &[&str]| -> Result<String> {
        let out = Command::new(&claude).no_window().args(args).current_dir(&cwd).output()?;
        let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        if !out.status.success() {
            bail!("`claude {}` failed: {}", args.join(" "), text.trim());
        }
        Ok(text)
    };
    let known = run(&["plugin", "marketplace", "list"]).unwrap_or_default();
    if !known.lines().any(|l| l.trim().trim_start_matches('❯').trim() == MARKETPLACE) {
        run(&["plugin", "marketplace", "add", &marketplace_source(), "--scope", scope])?;
    }
    run(&["plugin", "install", &format!("echo@{MARKETPLACE}"), "--scope", scope])?;
    // Claude Code said yes: is it in its own list, where Echo's sessions see it, with its files there?
    let here = |s: &str, p: &Option<String>| s == scope && (scope == "user" || p.as_deref().is_some_and(|p| same_folder(Path::new(p), &cwd)));
    if !plugin_installs().iter().any(|(s, p)| here(s, p)) {
        bail!("Claude Code finished, but the Echo plugin isn't in its list of plugins ({scope} scope{}) — try `claude plugin install echo@{MARKETPLACE} --scope {scope}` in a terminal there",
            if scope == "user" { String::new() } else { format!(", {}", cwd.display()) });
    }
    if plugin_root().is_none() {
        bail!("Claude Code lists the Echo plugin, but its files aren't there — install it again");
    }
    println!("Echo plugin installed ({scope} scope{}) — open a new Claude Code session or run /reload-plugins",
        if scope == "user" { String::new() } else { format!(", {}", cwd.display()) });
    Ok(())
}

/// The binary the command-line tool should point at: the installed app's, else this one.
pub(super) fn app_binary() -> Result<PathBuf> {
    refuse_translocated()?;
    #[cfg(target_os = "macos")]
    if let Some(b) = installed_app() {
        return Ok(b.join("Contents/MacOS/echoctl"));
    }
    Ok(std::env::current_exe()?)
}

#[cfg(target_os = "macos")]
pub(super) const CLI_LINK: &str = "/usr/local/bin/echoctl";

/// Is `echoctl` available in a terminal?
pub fn cli_installed() -> bool {
    #[cfg(target_os = "macos")]
    return fs::symlink_metadata(CLI_LINK).is_ok();
    #[allow(unreachable_code)]
    Path::new(&find_program("echoctl")).is_file()
}

/// `echoctl app install-cli`: put `echoctl` on the PATH. macOS: a /usr/local/bin symlink to
/// the app's binary (macOS asks for an administrator password when that folder is root-owned);
/// Windows: the app's folder is added to the user's PATH.
pub fn install_cli() -> Result<()> {
    let target = app_binary()?;
    #[cfg(target_os = "macos")]
    {
        let direct = fs::create_dir_all("/usr/local/bin").is_ok() && {
            let _ = fs::remove_file(CLI_LINK);
            std::os::unix::fs::symlink(&target, CLI_LINK).is_ok()
        };
        if !direct {
            let q = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
            let shell = format!("mkdir -p /usr/local/bin && ln -sf {} {}", q(&target.display().to_string()), q(CLI_LINK));
            let script = format!("do shell script {} with prompt \"Echo wants to add its command-line tool to /usr/local/bin.\" with administrator privileges", applescript_str(&shell));
            let ok = Command::new("osascript").args(["-e", &script]).status()?.success();
            if !ok {
                bail!("not installed (cancelled)");
            }
        }
        println!("{CLI_LINK} → {}", target.display());
        Ok(())
    }
    #[cfg(windows)]
    {
        let dir = target.parent().unwrap();
        add_to_user_path(dir)?;
        println!("added {} to your PATH (open a new terminal)", dir.display());
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = target;
        bail!("not supported on this platform")
    }
}

/// Windows: `dir` at the end of the user PATH (once). Its value is read and written unexpanded, as the
/// `REG_EXPAND_SZ` it is (`%USERPROFILE%\…` stays so), an absent one is made, and Windows is told the
/// environment changed, so terminals opened from now on have it.
#[cfg(windows)]
pub fn add_to_user_path(dir: &Path) -> Result<()> {
    let d = dir.display().to_string().replace('\'', "''");
    let script = format!(
        "$k = Get-Item 'HKCU:\\Environment'; $p = $k.GetValue('Path', '', 'DoNotExpandEnvironmentNames'); \
         $parts = @($p -split ';' | Where-Object {{ $_ }}); \
         if ($parts -notcontains '{d}') {{ \
           New-ItemProperty -Path 'HKCU:\\Environment' -Name Path -PropertyType ExpandString -Value (($parts + '{d}') -join ';') -Force | Out-Null; \
           [Environment]::SetEnvironmentVariable('ECHO_PATH_CHANGED', '1', 'User'); [Environment]::SetEnvironmentVariable('ECHO_PATH_CHANGED', $null, 'User') }}"
    );
    let ok = Command::new("powershell").no_window().args(["-NoProfile", "-Command", &script]).status()?.success();
    if !ok {
        bail!("could not update the user PATH");
    }
    Ok(())
}

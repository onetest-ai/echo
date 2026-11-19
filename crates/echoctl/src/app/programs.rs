//! The app: programs, Claude Code and its plugin, the command-line tool (split out of `app.rs`, moved as it was).

use super::*;

// ── programs, Claude Code, command-line tool ───────────────────────────────────────────────

pub(super) fn user_home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_default()
}

/// A program by name: PATH first, then the usual install locations — the app started at login
/// gets a minimal PATH without Homebrew's, nvm's or Claude Code's directories.
pub fn find_program(name: &str) -> String {
    let names: Vec<String> = program_names(name);
    program_candidates(name).into_iter().find(|p| p.is_file()).map(|p| p.display().to_string()).unwrap_or_else(|| names[0].clone())
}

/// The file names a program goes by here: `gh`, or `gh.exe` / `gh.cmd` on Windows.
fn program_names(name: &str) -> Vec<String> {
    if cfg!(windows) { vec![format!("{name}.exe"), format!("{name}.cmd")] } else { vec![name.to_string()] }
}

/// Every place [`find_program`] looks for `name`, in its order (existing or not).
pub fn program_candidates(name: &str) -> Vec<PathBuf> {
    let home = user_home();
    let names = program_names(name);
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
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).collect()
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

/// Every Claude config Echo's runs use: the default one (`installed_plugins_file`'s) and each AI profile's own
/// folder (`Profile::config_folder`, for the profiles Echo manages). [(profile name, config folder)].
fn claude_configs() -> Vec<(Option<String>, PathBuf)> {
    let mut out = vec![(None, installed_plugins_file().parent().and_then(Path::parent).map(Path::to_path_buf).unwrap_or_default())];
    for p in crate::ai::Settings::load().map(|s| s.all()).unwrap_or_default() {
        if p.kind.managed() && !out.iter().any(|(_, d)| *d == p.config_folder()) {
            out.push((Some(p.name.clone()), p.config_folder()));
        }
    }
    out
}

/// Echo plugin installs older than this Echo, in every Claude config: [{scope, project, version, plugin, profile,
/// config}]. An old plugin's terminal pane did things the app does now (it claimed every transcribed call for its
/// own minutes). A project install whose folder is gone isn't listed: Claude Code can't load it.
pub fn stale_plugins() -> Vec<Value> {
    let mut out = Vec::new();
    for (profile, dir) in claude_configs() {
        let v: Value = fs::read(dir.join("plugins/installed_plugins.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let plugins = v.get("plugins").unwrap_or(&v);
        for (k, list) in plugins.as_object().into_iter().flatten().filter(|(k, _)| k.starts_with("echo@")) {
            for i in list.as_array().into_iter().flatten() {
                let old = i["version"].as_str().is_some_and(|ver| crate::update::newer(env!("CARGO_PKG_VERSION"), ver));
                let gone = i["projectPath"].as_str().is_some_and(|p| !Path::new(p).is_dir());
                if old && !gone {
                    out.push(json!({"plugin": k, "scope": i["scope"], "project": i["projectPath"], "version": i["version"], "profile": profile, "config": dir}));
                }
            }
        }
    }
    out
}

/// Bring every Echo plugin install up to date: per Claude config the marketplace first (with that config's
/// `CLAUDE_CONFIG_DIR`), then each install in its scope (a project's from its folder). One failing step doesn't
/// stop the rest. What each step said, for the log; an error naming every failure (and what did update) when any failed.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn update_plugins() -> Result<String> {
    update_plugins_within(Duration::from_secs(120))
}

/// `update_plugins`, each `claude` call stopped (and counted as failed) after `limit`: a git fetch can hang.
fn update_plugins_within(limit: Duration) -> Result<String> {
    let claude = claude_bin().ok_or_else(|| anyhow::anyhow!("Claude Code is not installed"))?;
    let stale = stale_plugins();
    let (mut said, mut done, mut failed) = (String::new(), Vec::<String>::new(), Vec::<String>::new());
    // `config`: a profile's own folder for `CLAUDE_CONFIG_DIR`; the default config inherits the app's environment.
    let mut run = |args: &[&str], config: Option<&str>, cwd: Option<&str>, what: String| {
        let mut c = std::process::Command::new(&claude);
        c.args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        if let Some(d) = config {
            c.env("CLAUDE_CONFIG_DIR", d);
        }
        if let Some(d) = cwd.filter(|d| Path::new(d).is_dir()) {
            c.current_dir(d);
        }
        let mut child = match c.spawn() {
            Ok(ch) => ch,
            Err(e) => return failed.push(format!("{what} ({e})")),
        };
        let readers: Vec<_> = [child.stdout.take().map(|r| Box::new(r) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|r| Box::new(r) as Box<dyn std::io::Read + Send>)]
            .into_iter()
            .flatten()
            .map(|mut r| {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let mut b = Vec::new();
                    let _ = r.read_to_end(&mut b);
                    let _ = tx.send(b);
                });
                rx
            })
            .collect();
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(20)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        // A killed call's descendants may still hold the pipes: don't wait for them.
        for rx in readers {
            if let Ok(b) = rx.recv_timeout(if status.is_some() { Duration::from_secs(5) } else { Duration::from_millis(200) }) {
                said.push_str(&String::from_utf8_lossy(&b));
            }
        }
        match status {
            Some(st) if st.success() => done.push(what),
            Some(_) => failed.push(what),
            None => failed.push(format!("{what} (no answer in {} s)", limit.as_secs().max(1))),
        }
    };
    let markets: std::collections::BTreeSet<(String, String, String)> = stale
        .iter()
        .filter_map(|s| Some((s["plugin"].as_str()?.split_once('@')?.1.to_string(), s["profile"].as_str().unwrap_or_default().to_string(), s["config"].as_str()?.to_string())))
        .collect();
    for (m, profile, config) in &markets {
        run(&["plugin", "marketplace", "update", m], (!profile.is_empty()).then_some(config.as_str()), None, format!("marketplace {m}{}", in_profile(profile)));
    }
    for s in &stale {
        let (Some(p), Some(scope)) = (s["plugin"].as_str(), s["scope"].as_str()) else { continue };
        let what = format!("{p} {} ({scope}{})", s["version"].as_str().unwrap_or("?"), in_profile(s["profile"].as_str().unwrap_or_default()));
        run(&["plugin", "update", p, "--scope", scope], s["profile"].as_str().and_then(|_| s["config"].as_str()), s["project"].as_str(), what);
    }
    anyhow::ensure!(failed.is_empty(), "couldn't update {}{}", failed.join(", "), if done.is_empty() { String::new() } else { format!("; updated {}", done.join(", ")) });
    Ok(said)
}

fn in_profile(profile: &str) -> String {
    if profile.is_empty() { String::new() } else { format!(", profile {profile}") }
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
    crate::paths::refuse_real_folders();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn installs(dir: &Path, version: &str, scope: &str, project: Option<&Path>) {
        fs::create_dir_all(dir.join("plugins")).unwrap();
        let mut i = json!({"scope": scope, "version": version, "installPath": dir.join("cache")});
        if let Some(p) = project {
            i["projectPath"] = json!(p);
        }
        fs::write(dir.join("plugins/installed_plugins.json"), json!({"version": 2, "plugins": {"echo@onetest": [i]}}).to_string()).unwrap();
    }

    fn work_profile() -> crate::ai::Profile {
        crate::ai::Settings::update(|s| {
            s.profiles.push(crate::ai::Profile { id: "work".into(), name: "work".into(), kind: crate::ai::Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false, models: Default::default(), wrapper: None });
            Ok(())
        })
        .unwrap();
        crate::ai::Settings::load().unwrap().get("work").unwrap()
    }

    #[test]
    fn a_project_install_whose_folder_is_gone_isnt_stale_and_a_profiles_old_install_names_the_profile() {
        let (_env, root) = crate::paths::testenv::folders("stale-plugins");
        let default = installed_plugins_file().parent().unwrap().parent().unwrap().to_path_buf();
        let gone = root.join("deleted-copy");
        installs(&default, "0.0.1", "project", Some(&gone));
        assert!(stale_plugins().is_empty(), "an orphaned project install can't load");
        fs::create_dir_all(&gone).unwrap();
        assert_eq!(stale_plugins().len(), 1, "the folder is there: stale");
        installs(&default, env!("CARGO_PKG_VERSION"), "user", None);
        assert!(stale_plugins().is_empty());
        let p = work_profile();
        installs(&p.config_folder(), "0.0.1", "user", None);
        let s = stale_plugins();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0]["profile"], "work");
        assert_eq!(s[0]["config"], json!(p.config_folder()));
    }

    #[cfg(unix)]
    #[test]
    fn update_plugins_runs_each_install_in_its_own_config_and_reports_each_failure() {
        use std::os::unix::fs::PermissionsExt;
        let (_env, root) = crate::paths::testenv::folders("update-plugins");
        let default = installed_plugins_file().parent().unwrap().parent().unwrap().to_path_buf();
        installs(&default, "0.0.1", "user", None);
        let p = work_profile();
        installs(&p.config_folder(), "0.0.1", "user", None);
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let log = root.join("calls.log");
        // The fake records its config folder and arguments; the plugin update in the profile's config fails.
        fs::write(bin.join("claude"), format!("#!/bin/sh\necho \"$CLAUDE_CONFIG_DIR|$*\" >> '{}'\ncase \"$CLAUDE_CONFIG_DIR@$1$2\" in */claude/work@pluginupdate) exit 1;; esac\nexit 0\n", log.display())).unwrap();
        fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH");
        std::env::set_var("PATH", &bin);
        // The default config is the app's own environment: with the variable unset (~/.claude) its calls get none.
        std::env::remove_var("CLAUDE_CONFIG_DIR");
        let r = update_plugins();
        match path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
        let err = format!("{:#}", r.unwrap_err());
        assert!(err.contains("couldn't update echo@onetest 0.0.1 (user, profile work)") && err.contains("updated marketplace onetest") && err.contains("echo@onetest 0.0.1 (user)"), "{err}");
        let calls: Vec<String> = fs::read_to_string(&log).unwrap().lines().map(str::to_owned).collect();
        let w = p.config_folder().display().to_string();
        for c in ["|plugin marketplace update onetest".to_string(), "|plugin update echo@onetest --scope user".to_string(), format!("{w}|plugin marketplace update onetest"), format!("{w}|plugin update echo@onetest --scope user")] {
            assert!(calls.contains(&c), "{c} in {calls:?}");
        }
        assert_eq!(calls.len(), 4);
    }

    #[cfg(unix)]
    #[test]
    fn a_claude_call_that_doesnt_answer_is_stopped_and_the_others_go_on() {
        use std::os::unix::fs::PermissionsExt;
        let (_env, root) = crate::paths::testenv::folders("update-plugins-timeout");
        let default = installed_plugins_file().parent().unwrap().parent().unwrap().to_path_buf();
        installs(&default, "0.0.1", "user", None);
        let p = work_profile();
        installs(&p.config_folder(), "0.0.1", "user", None);
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let log = root.join("calls.log");
        // Every call in the profile's config hangs; the default config's answer.
        fs::write(bin.join("claude"), format!("#!/bin/sh\necho \"$CLAUDE_CONFIG_DIR|$*\" >> '{}'\ncase \"$CLAUDE_CONFIG_DIR\" in */claude/work) exec /bin/sleep 30;; esac\nexit 0\n", log.display())).unwrap();
        fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::var_os("PATH");
        std::env::set_var("PATH", &bin);
        std::env::remove_var("CLAUDE_CONFIG_DIR");
        let t = Instant::now();
        let r = update_plugins_within(Duration::from_millis(300));
        match path {
            Some(p) => std::env::set_var("PATH", p),
            None => std::env::remove_var("PATH"),
        }
        assert!(t.elapsed() < Duration::from_secs(10));
        let err = format!("{:#}", r.unwrap_err());
        assert!(err.contains("marketplace onetest, profile work (no answer") && err.contains("echo@onetest 0.0.1 (user, profile work) (no answer") && err.contains("; updated marketplace onetest, echo@onetest 0.0.1 (user)"), "{err}");
        assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 4);
    }
}

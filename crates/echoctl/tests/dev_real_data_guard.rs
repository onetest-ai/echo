//! A development `echoctl` started by Claude Code (`CLAUDECODE`) without the run's folders (`ECHOCTL_CONFIG_DIR`,
//! `ECHOCTL_HOME`) refuses before it touches the user's real config or data; with them it works as always. Runs the
//! real binary with a made-up home (HOME, USERPROFILE, APPDATA), so the real folders stay out of reach either way.
#![cfg(debug_assertions)]

use std::path::Path;
use std::process::{Command, Output};

const WRITES: [&[&str]; 3] = [&["today", "reset"], &["plugin", "install", "acme-notes", "--yes"], &["config", "--me-name", "Sam Rivera"]];

fn echoctl(user: &Path, run_env: &[(&str, &Path)], args: &[&str]) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_echoctl"));
    c.args(args).env("CLAUDECODE", "1").env("HOME", user).env("USERPROFILE", user).env("APPDATA", user).env("CLAUDE_CONFIG_DIR", user.join(".claude"));
    c.env_remove("ECHOCTL_CONFIG_DIR").env_remove("ECHOCTL_HOME").env_remove("ZDOTDIR");
    for (k, v) in run_env {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn files(dir: &Path) -> Vec<String> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        out.push(e.path().display().to_string());
        if e.path().is_dir() {
            out.extend(files(&e.path()));
        }
    }
    out
}

fn fresh(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("echo-dev-guard-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("user")).unwrap();
    root
}

#[test]
fn without_the_runs_env_a_write_command_refuses_and_writes_nothing() {
    let root = fresh("refuse");
    let user = root.join("user");
    let (config, home) = (root.join("config"), root.join("home"));
    for (run_env, missing) in [
        (vec![], "ECHOCTL_CONFIG_DIR"),
        (vec![("ECHOCTL_HOME", home.as_path())], "ECHOCTL_CONFIG_DIR"),
        (vec![("ECHOCTL_CONFIG_DIR", config.as_path())], "ECHOCTL_HOME"),
    ] {
        for args in WRITES {
            let out = echoctl(&user, &run_env, args);
            let err = String::from_utf8_lossy(&out.stderr);
            assert_eq!(out.status.code(), Some(2), "{args:?} with {run_env:?}: {err}");
            assert!(err.contains(missing) && err.contains(". <dir>/env"), "{args:?} names {missing} and the env: {err}");
            assert_eq!(files(&user), Vec::<String>::new(), "{args:?} with {run_env:?} wrote under the home");
            assert!(!config.exists() && !home.exists(), "{args:?} with {run_env:?} wrote the run's folders");
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn without_the_runs_env_autostart_off_leaves_the_start_at_login_as_it_is() {
    let root = fresh("autostart");
    let user = root.join("user");
    let agents = user.join("Library/LaunchAgents");
    std::fs::create_dir_all(&agents).unwrap();
    let plist = agents.join("ai.onetest.echo.plist");
    std::fs::write(&plist, "the installed Echo's").unwrap();
    for run_env in [vec![], vec![("ECHOCTL_CONFIG_DIR", root.join("config"))]] {
        let run_env: Vec<(&str, &Path)> = run_env.iter().map(|(k, v)| (*k, v.as_path())).collect();
        let out = echoctl(&user, &run_env, &["app", "autostart", "off"]);
        assert_eq!(out.status.code(), Some(2), "{run_env:?}: {}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(std::fs::read_to_string(&plist).unwrap(), "the installed Echo's", "{run_env:?}: start at login untouched");
        assert_eq!(files(&agents), vec![plist.display().to_string()], "{run_env:?}: nothing else in LaunchAgents");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn with_the_runs_env_it_works_under_claude_code() {
    let root = fresh("allow");
    let user = root.join("user");
    let (config, home) = (root.join("config"), root.join("home"));
    let run_env = [("ECHOCTL_CONFIG_DIR", config.as_path()), ("ECHOCTL_HOME", home.as_path())];
    for args in [&["today", "reset"][..], &["config", "--me-name", "Sam Rivera"]] {
        let out = echoctl(&user, &run_env, args);
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(config.join("today-layout.json").is_file(), "today reset wrote the run's config");
    assert!(std::fs::read_to_string(home.join(".echo/config.json")).unwrap().contains("Sam Rivera"), "config wrote the run's data");
    assert_eq!(files(&user), Vec::<String>::new(), "nothing under the home");
    let _ = std::fs::remove_dir_all(&root);
}

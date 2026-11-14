//! A CLI command that loads a plugin only to answer (`echoctl widget show`, `echoctl work show`) loads it as a terminal
//! run: `ctx.terminal` is true, so the plugin skips its start-up work. Runs the real binary on a made-up data folder.

use std::path::Path;
use std::process::Command;

fn echoctl(root: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_echoctl"))
        .args(args)
        .env("ECHOCTL_HOME", root.join("data"))
        .env("ECHOCTL_CONFIG_DIR", root.join("config"))
        .env("HOME", root.join("home"))
        .env("CLAUDE_CONFIG_DIR", root.join("home/.claude"))
        .env_remove("ZDOTDIR")
        .output()
        .unwrap();
    assert!(out.status.success(), "echoctl {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn widget_show_and_work_show_load_a_plugin_as_a_terminal_run() {
    let root = std::env::temp_dir().join(format!("echo-cli-terminal-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let plugin = root.join("market/plugins/acme-startup");
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::write(
        plugin.join("echo-plugin.json"),
        r#"{"id":"acme-startup","name":"Acme startup","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","widgets":[{"id":"w","label":"Acme","kind":"stat"}],"uses":{"work":{}}}"#,
    )
    .unwrap();
    // Start-up work (`started.txt`) is for the app only; `terminal.txt` says what `ctx.terminal` was.
    std::fs::write(
        plugin.join("main.js"),
        "export function apply(ctx) {\n  ctx.store.write('terminal.txt', String(ctx.terminal))\n  if (!ctx.terminal) ctx.store.write('started.txt', 'ran')\n  ctx.widgets.provide('w', () => ({ value: 1 }))\n  ctx.work.provide(() => [])\n}\n",
    )
    .unwrap();
    std::fs::write(root.join("market/echo-marketplace.json"), r#"{"name":"acme-local","plugins":[{"id":"acme-startup","source":"./plugins/acme-startup"}]}"#).unwrap();
    echoctl(&root, &["plugin", "marketplace", "add", root.join("market").to_str().unwrap(), "--name", "acme-local"]);
    echoctl(&root, &["plugin", "install", "acme-startup", "--yes"]);
    let store = root.join("data/.echo/plugins/acme-startup");
    let _ = std::fs::remove_file(store.join("started.txt"));

    for args in [["widget", "show", "plugin:acme-startup:w"], ["work", "show", ""]] {
        let _ = std::fs::remove_file(store.join("terminal.txt"));
        echoctl(&root, &args[..if args[2].is_empty() { 2 } else { 3 }]);
        assert_eq!(std::fs::read_to_string(store.join("terminal.txt")).unwrap_or_default(), "true", "{args:?}: loaded as a terminal run");
        assert!(!store.join("started.txt").exists(), "{args:?}: the plugin's start-up work didn't run");
    }
    let _ = std::fs::remove_dir_all(&root);
}

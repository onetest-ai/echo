//! `echo_plugins`, the MCP server Echo's sessions get when a plugin offers itself (`ctx.expose`): four tools that
//! show what plugins offer, a level at a time, and call an offered command (`offers.rs`), the plugin's host half
//! loaded in this process as `echoctl plugin call` loads it.

use std::io::{BufRead, Write};
use std::path::Path;

use anyhow::Result;
use serde_json::{json, Value};

use super::offers;
use crate::paths;

const PROTOCOL: &str = "2025-03-26";
pub const SERVER: &str = "echo_plugins";

fn tools() -> Value {
    let obj = |props: Value, req: &[&str]| json!({"type": "object", "properties": props, "required": req});
    let s = |d: &str| json!({"type": "string", "description": d});
    json!([
        {"name": "plugins", "description": "Every Echo plugin that gives you something: id, name, what it is for, how many commands it offers, and its guide to Read first (`guide`, with `entries` of its own) when it has one. Then plugin(id) for its commands.", "inputSchema": obj(json!({}), &[])},
        {"name": "plugin", "description": "One plugin: its commands, each with when to call it, and its guide. Then command(id, name) for a command's arguments.", "inputSchema": obj(json!({"id": s("the plugin's id")}), &["id"])},
        {"name": "command", "description": "One command's arguments: names, which are optional, defaults (args null: any JSON object).", "inputSchema": obj(json!({"id": s("the plugin's id"), "name": s("the command")}), &["id", "name"])},
        {"name": "call", "description": "Call a plugin's command with its arguments; its answer as JSON.", "inputSchema": obj(json!({"id": s("the plugin's id"), "name": s("the command"), "args": {"type": "object", "description": "its arguments"}}), &["id", "name"])},
    ])
}

/// The plugins with a guide laid out for Claude (`plugins::agent::rows`): id → (what it gives, its CLAUDE.md, its
/// entries' [{name, summary, guide}]).
fn guides() -> std::collections::BTreeMap<String, (String, String, Vec<Value>)> {
    let mut out: std::collections::BTreeMap<String, (String, String, Vec<Value>)> = Default::default();
    for r in crate::plugins::agent::rows() {
        let parts: Vec<&str> = r.read.split('/').collect();
        match parts.as_slice() {
            [".plugins", id, "CLAUDE.md"] => {
                out.insert(id.to_string(), (r.what.clone(), r.read.clone(), vec![]));
            }
            [".plugins", id, _, "CLAUDE.md"] => {
                if let Some(g) = out.get_mut(*id) {
                    let name = r.plugin.rsplit(" · ").next().unwrap_or("").to_string();
                    g.2.push(json!({"name": name, "summary": r.what, "guide": r.read}));
                }
            }
            _ => {}
        }
    }
    out
}

/// Every plugin that offers commands or gives a guide: [{id, name, description, commands, guide, entries}].
fn everything() -> Value {
    let offered: std::collections::BTreeMap<String, offers::Offer> = offers::all().into_iter().collect();
    let guides = guides();
    let names: std::collections::BTreeMap<String, String> = crate::plugins::enabled().into_iter().map(|p| (p.id, p.name)).collect();
    let ids: std::collections::BTreeSet<&String> = offered.keys().chain(guides.keys()).collect();
    json!(ids.into_iter().map(|id| {
        let (o, g) = (offered.get(id), guides.get(id));
        json!({"id": id, "name": names.get(id).cloned().unwrap_or_else(|| id.clone()),
               "description": o.map(|o| o.description.clone()).or_else(|| g.map(|g| g.0.clone())).unwrap_or_default(),
               "commands": o.map_or(0, |o| o.commands.len()),
               "guide": g.map(|g| g.1.clone()), "entries": g.map(|g| g.2.clone()).unwrap_or_default()})
    }).collect::<Vec<_>>())
}

/// One plugin: what `offers::detail` gives (commands) and its guide; a plugin with only a guide has no commands.
fn one(id: &str) -> Result<Value> {
    let g = guides().remove(id);
    let mut v = match (offers::detail(id), &g) {
        (Ok(v), _) => v,
        (Err(_), Some(g)) => json!({"id": id, "name": crate::plugins::enabled().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string()), "description": g.0, "commands": []}),
        (Err(e), None) => anyhow::bail!("no plugin “{id}” offers commands or a guide ({e:#})"),
    };
    v["guide"] = json!(g.as_ref().map(|g| g.1.clone()));
    v["entries"] = json!(g.map(|g| g.2).unwrap_or_default());
    Ok(v)
}

fn run(name: &str, a: &Value, reading: bool) -> Result<Value> {
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    match name {
        "plugins" => Ok(everything()),
        "plugin" => one(&s("id")),
        "command" => offers::command_detail(&s("id"), &s("name")),
        "call" => offers::call(&s("id"), &s("name"), &a["args"], "claude", reading),
        t => anyhow::bail!("no tool {t}"),
    }
}

fn answer(req: &Value, reading: bool) -> Option<Value> {
    let id = req.get("id").cloned()?;
    let r = match req["method"].as_str().unwrap_or("") {
        "initialize" => Ok(json!({"protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL), "capabilities": {"tools": {}},
                                  "serverInfo": {"name": SERVER, "version": env!("CARGO_PKG_VERSION")},
                                  "instructions": if reading { "Commands the user's Echo plugins offer: plugins → plugin(id) → command(id, name) → call. This run only reads: only commands marked reads run." } else { "Commands the user's Echo plugins offer: plugins → plugin(id) → command(id, name) → call." }})),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => Ok(match run(req["params"]["name"].as_str().unwrap_or(""), &req["params"]["arguments"], reading) {
            Ok(v) => json!({"content": [{"type": "text", "text": v.to_string()}], "isError": false}),
            Err(e) => json!({"content": [{"type": "text", "text": json!({"error": format!("{e:#}")}).to_string()}], "isError": true}),
        }),
        m => Err(json!({"code": -32601, "message": format!("no method {m}")})),
    };
    Some(match r {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    })
}

/// Serve until stdin closes; `reading`: a run that only reads (`folder_args::Place::Read`).
pub fn serve(reading: bool) -> Result<()> {
    // A session's host runs commands, never the plugins' timers: those are the app's.
    super::no_timers();
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(req) => answer(&req, reading),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}})),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// The `--mcp-config` file for a session's `echo_plugins` server; None when no plugin offers commands or gives a guide
/// (nothing added).
pub fn config(exe: &Path, reading: bool) -> Option<String> {
    if offers::all().is_empty() && crate::plugins::agent::rows().is_empty() {
        return None;
    }
    let f = paths::config_dir().join("minutes-run").join(format!("echo-plugins-mcp{}.json", if reading { "-read" } else { "" }));
    std::fs::create_dir_all(f.parent()?).ok()?;
    let mut args = vec!["plugins-mcp".to_string()];
    if reading {
        args.push("--read".into());
    }
    let v = json!({"mcpServers": {SERVER: {"command": exe.display().to_string(), "args": args}}});
    paths::write_atomic(&f, &serde_json::to_vec_pretty(&v).ok()?).ok()?;
    Some(f.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_shows_four_tools_and_a_reading_run_calls_only_reading_commands() {
        let (_env, root) = paths::testenv::folders("plugins-mcp");
        assert_eq!(config(Path::new("/x/echoctl"), false), None, "no plugin offers itself: no server");
        super::super::offers::tests::install(&root, &[("vaults", r#"
          export function apply(ctx) {
            const api = ctx.expose('Search across the vaults')
            api.command('search', ({query}) => ['found', query], {description: 'Find notes', reads: true})
            api.command('note', ({path, text}) => 'written', {description: 'Write a note'})
          }"#)]);
        let f = config(Path::new("/x/echoctl"), true).unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(f).unwrap()).unwrap();
        assert_eq!(v["mcpServers"][SERVER]["args"], json!(["plugins-mcp", "--read"]));
        let ask = |name: &str, args: Value, reading: bool| answer(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}}), reading).unwrap()["result"].clone();
        let list = answer(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}), false).unwrap();
        assert_eq!(list["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect::<Vec<_>>(), ["plugins", "plugin", "command", "call"]);
        assert!(ask("plugins", json!({}), false)["content"][0]["text"].as_str().unwrap().contains("Search across the vaults"));
        assert!(ask("command", json!({"id": "vaults", "name": "note"}), false)["content"][0]["text"].as_str().unwrap().contains("\"text\""));
        let call = |name: &str, reading: bool| ask("call", json!({"id": "vaults", "name": name, "args": {"query": "acme", "path": "a.md", "text": "x"}}), reading);
        let r = call("search", true);
        assert_eq!((r["isError"].clone(), r["content"][0]["text"].clone()), (json!(false), json!("[\"found\",\"acme\"]")));
        // Inside a run that keeps its changes: a called command that may change things is noted, a reading one and a
        // refused one aren't.
        let actions = root.join("actions.jsonl");
        std::env::set_var(crate::notes::HEADLESS, "1");
        std::env::set_var(crate::notes::RUN_ACTIONS, &actions);
        let r = call("note", true);
        assert!(r["isError"] == json!(true) && r["content"][0]["text"].as_str().unwrap().contains("this run only reads"), "{r}");
        assert_eq!(call("note", false)["content"][0]["text"], json!("\"written\""));
        call("search", false);
        std::env::remove_var(crate::notes::RUN_ACTIONS);
        std::env::remove_var(crate::notes::HEADLESS);
        let noted = crate::notes::run_actions(&actions);
        assert_eq!(noted.iter().map(|a| (a["kind"].as_str().unwrap(), a["command"].as_str().unwrap())).collect::<Vec<_>>(), [("change", "plugin call vaults note")]);
        // Echo's sessions get it: a reading run with --read, its tools allowed without asking.
        let claude = crate::ai::Profile { id: "work".into(), name: "Work".into(), kind: crate::ai::Kind::Claude, base_url: None, token: None, live_model: None, notes_model: None, account: None, email: None, sso: false, models: Default::default(), wrapper: None };
        for (place, args) in [(crate::folder_args::Place::Read, json!(["plugins-mcp", "--read"])), (crate::folder_args::Place::Data, json!(["plugins-mcp"]))] {
            let a = crate::folder_args::session_as(&claude, Path::new("/x/echoctl"), &root.join("s.json"), "routine", None, crate::folder_args::Who::Run(crate::config::Permissions::Auto), place).unwrap();
            let f = a.windows(2).filter(|w| w[0] == "--mcp-config").map(|w| w[1].clone()).find(|f| f.contains("echo-plugins-mcp")).unwrap();
            let v: Value = serde_json::from_slice(&std::fs::read(f).unwrap()).unwrap();
            assert_eq!(v["mcpServers"][SERVER]["args"], args);
            assert!(a.windows(2).any(|w| w[0] == "--allowedTools" && w[1] == "mcp__echo_plugins"), "{a:?}");
        }
        // Showing what plugins offer is no change of a run's; calling a command that may change things is noted.
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!((crate::notes::action_kind(&argv(&["plugins-mcp", "--read"])), crate::notes::action_kind(&argv(&["plugin", "commands", "vaults"]))), (None, None));
        crate::jshost::unload_all();
    }

    #[test]
    fn a_plugin_with_only_a_guide_is_listed_by_the_plugins_tool_and_brings_the_server() {
        let (_env, root) = paths::testenv::folders("plugins-mcp-guides");
        let mk = root.join("market");
        let d = mk.join("plugins/staff");
        std::fs::create_dir_all(d.join("agent")).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": "staff", "name": "Staff", "version": "0.1.0", "summary": "A made-up plugin", "agent": {"summary": "Staffing of Acme projects"}}).to_string()).unwrap();
        std::fs::write(d.join("agent/CLAUDE.md"), "# Staff\n").unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": [{"id": "staff", "source": "./plugins/staff"}]}).to_string()).unwrap();
        let name = crate::plugins::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        crate::plugins::refresh(&name).unwrap();
        let agreed = crate::plugins::review("staff", Some(&name)).unwrap().uses;
        crate::plugins::install("staff", Some(&name), &agreed).unwrap();
        crate::plugins::agent::lay_out("staff").unwrap();
        // No plugin offers commands, one gives a guide: the server is there, its plugins tool lists it with its guide.
        assert!(offers::all().is_empty());
        assert!(config(Path::new("/x/echoctl"), false).is_some());
        assert_eq!(run("plugins", &json!({}), false).unwrap(), json!([{"id": "staff", "name": "Staff", "description": "Staffing of Acme projects", "commands": 0, "guide": ".plugins/staff/CLAUDE.md", "entries": []}]));
        assert_eq!(run("plugin", &json!({"id": "staff"}), false).unwrap()["guide"], ".plugins/staff/CLAUDE.md");
        assert!(run("plugin", &json!({"id": "nobody"}), false).unwrap_err().to_string().contains("offers commands or a guide"));
        // Switched off: nothing gives Claude anything, no server.
        crate::plugins::set_enabled("staff", false).unwrap();
        crate::plugins::agent::lay_out("staff").unwrap();
        assert_eq!(config(Path::new("/x/echoctl"), false), None);
    }
}

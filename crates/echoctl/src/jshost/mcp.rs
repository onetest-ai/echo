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
        {"name": "plugins", "description": "The user's Echo plugins that offer commands: id, name, what each is for. Then plugin(id) for its commands.", "inputSchema": obj(json!({}), &[])},
        {"name": "plugin", "description": "One plugin's commands, each with when to call it. Then command(id, name) for its arguments.", "inputSchema": obj(json!({"id": s("the plugin's id")}), &["id"])},
        {"name": "command", "description": "One command's arguments: names, which are optional, defaults (args null: any JSON object).", "inputSchema": obj(json!({"id": s("the plugin's id"), "name": s("the command")}), &["id", "name"])},
        {"name": "call", "description": "Call a plugin's command with its arguments; its answer as JSON.", "inputSchema": obj(json!({"id": s("the plugin's id"), "name": s("the command"), "args": {"type": "object", "description": "its arguments"}}), &["id", "name"])},
    ])
}

fn run(name: &str, a: &Value, reading: bool) -> Result<Value> {
    let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
    match name {
        "plugins" => Ok(offers::listing()),
        "plugin" => offers::detail(&s("id")),
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

/// The `--mcp-config` file for a session's `echo_plugins` server; None when no plugin offers itself (nothing added).
pub fn config(exe: &Path, reading: bool) -> Option<String> {
    if offers::all().is_empty() {
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
        let r = call("note", true);
        assert!(r["isError"] == json!(true) && r["content"][0]["text"].as_str().unwrap().contains("this run only reads"), "{r}");
        assert_eq!(call("note", false)["content"][0]["text"], json!("\"written\""));
        crate::jshost::unload_all();
    }
}

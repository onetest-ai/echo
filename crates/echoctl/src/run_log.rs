//! A Claude Code run's stream-json events as the lines of its log (a minutes run's `<call>/claude.log`, a thread sync's).

use serde_json::Value;

/// One stream-json event as log lines a person can follow.
pub(crate) fn describe(ev: &Value) -> Vec<String> {
    let clip = |s: &str, n: usize| -> String {
        let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
        if one.chars().count() > n { format!("{}…", one.chars().take(n).collect::<String>()) } else { one }
    };
    match ev["type"].as_str() {
        Some("system") if ev["subtype"] == "init" => vec![format!(
            "Claude Code started · model {} · session {}",
            ev["model"].as_str().unwrap_or("?"),
            ev["session_id"].as_str().unwrap_or("?")
        )],
        Some("assistant") => ev["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| match c["type"].as_str() {
                Some("text") => c["text"].as_str().filter(|t| !t.trim().is_empty()).map(|t| format!("Claude: {}", clip(t, 400))),
                Some("tool_use") => {
                    let i = &c["input"];
                    let what = i["command"].as_str().or(i["file_path"].as_str()).or(i["skill"].as_str()).or(i["pattern"].as_str()).unwrap_or("");
                    Some(format!("→ {} {}", c["name"].as_str().unwrap_or("tool"), clip(what, 200)))
                }
                _ => None,
            })
            .collect(),
        Some("user") => ev["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|c| c["type"] == "tool_result" && c["is_error"].as_bool() == Some(true))
            .map(|c| {
                let text = c["content"].as_str().map(str::to_owned).unwrap_or_else(|| {
                    c["content"].as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ")
                });
                format!("  ✗ {}", clip(&text, 300))
            })
            .collect(),
        Some("result") => {
            let mut out = vec![format!(
                "Finished: {} · {:.0}s · ${:.2}",
                if ev["is_error"].as_bool() == Some(true) { "error" } else { "ok" },
                ev["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0,
                ev["total_cost_usd"].as_f64().unwrap_or(0.0)
            )];
            if let Some(t) = ev["result"].as_str().filter(|t| !t.trim().is_empty()) {
                out.push(format!("Result: {}", clip(t, 600)));
            }
            for d in ev["permission_denials"].as_array().into_iter().flatten() {
                out.push(format!("Not allowed: {} {}", d["tool_name"].as_str().unwrap_or("?"), clip(&d["tool_input"].to_string(), 200)));
            }
            out
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_lines_from_stream_events() {
        let tool = serde_json::json!({ "type": "assistant", "message": { "content": [
            { "type": "text", "text": "Reading the context." },
            { "type": "tool_use", "name": "Bash", "input": { "command": "echoctl context /x" } } ] } });
        assert_eq!(describe(&tool), vec!["Claude: Reading the context.", "→ Bash echoctl context /x"]);
        let res = serde_json::json!({ "type": "result", "is_error": false, "duration_ms": 61000.0, "total_cost_usd": 0.4, "result": "Done.",
            "permission_denials": [{ "tool_name": "WebFetch", "tool_input": { "url": "https://x" } }] });
        let lines = describe(&res);
        assert_eq!(lines[0], "Finished: ok · 61s · $0.40");
        assert!(lines[2].starts_with("Not allowed: WebFetch"));
    }
}

//! A past conversation from Claude Code's own transcript (spec K12-11): `<profile's config>/projects/*/<id>.jsonl` read
//! into the events the live stream gives ([`stream::Parser`], so a tool row and its summary look the same), for a
//! conversation Echo holds no process or events of (Echo restarted, or it began elsewhere). The transcript holds the
//! stream's `user` and `assistant` lines among bookkeeping lines of other types, which are skipped. Permissions aren't in
//! it, and print mode has no todos. Everything in it is others' words, handled as the stream's are (untrusted-text R3).

use serde_json::{json, Value};
use std::io::{BufRead as _, BufReader};
use std::path::Path;

use super::{stream, Vault};
use crate::terminal;

/// A line longer than this is not read (a transcript line holds one message).
const LINE_MAX: usize = 32 << 20;
/// Echo's framing after the user's own words (`chat::message`): the pages it was about, then attached minutes.
const PAGES_MARK: &str = "\n\n(The pages this is about: ";
const MINUTES_MARK: &str = "\n\nMinutes of ";

/// Conversation `id`'s events (without `n`), oldest first. `None`: the transcript can't be read here — it isn't in the
/// Claude Code folder `cfg`, can't be opened, or is no transcript (no line of it is a JSON object with a `type`).
pub fn events(cfg: &Path, id: &str, vault: &Vault) -> Option<Vec<Value>> {
    let file = terminal::transcript(cfg, id)?;
    let reader = BufReader::new(std::fs::File::open(file).ok()?);
    let mut parser = stream::Parser::new(vault);
    let (mut out, mut known) = (vec![], false);
    for line in reader.split(b'\n').map_while(Result::ok) {
        // A line that isn't UTF-8, is too long or isn't JSON is skipped.
        let Ok(line) = String::from_utf8(line) else { continue };
        if line.len() > LINE_MAX {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        if v["type"].as_str().is_none() {
            continue;
        }
        known = true;
        if v["isSidechain"] == true {
            continue;
        }
        match v["type"].as_str() {
            Some("user") => match user_text(&v) {
                Some(Some(e)) => out.push(e),
                Some(None) => {}
                None => out.extend(parser.feed(&line)),
            },
            Some("assistant") => out.extend(parser.feed(&line)),
            _ => {}
        }
    }
    out.extend(parser.finish());
    // The live process numbers its blocks from b1 as well: a history block keeps its own names.
    for e in out.iter_mut().filter(|e| e["block"].is_string()) {
        e["block"] = json!(format!("h{}", e["block"].as_str().unwrap_or("").trim_start_matches('b')));
    }
    known.then_some(out)
}

/// A `user` line that holds the person's own words: `Some(Some(event))`; one that holds harness text or nothing:
/// `Some(None)`; a tool result: `None` (the parser's).
fn user_text(v: &Value) -> Option<Option<Value>> {
    let content = &v["message"]["content"];
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(a) if a.iter().any(|b| b["type"] == "tool_result") => return None,
        Value::Array(a) => a.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => return Some(None),
    };
    let t = text.trim();
    if v["isMeta"] == true || t.is_empty() || t.starts_with('<') || t.starts_with("[Request interrupted") {
        return Some(None);
    }
    let (words, pages) = match t.split_once(PAGES_MARK) {
        Some((w, rest)) => (w, rest.split_once(")\n\n").map_or(rest, |(p, _)| p).trim_end_matches(')')),
        None => (t, ""),
    };
    let words = words.split(MINUTES_MARK).next().unwrap_or(words).trim();
    let context: Vec<Value> = pages.split(", ").map(|p| p.trim().trim_matches('`')).filter(|p| !p.is_empty()).map(|p| json!({"page": p})).collect();
    Some(Some(json!({"type": "user", "text": words, "context": context})))
}

#[cfg(test)]
mod tests;

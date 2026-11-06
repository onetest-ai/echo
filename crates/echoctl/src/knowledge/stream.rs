//! Claude Code's `stream-json` output as Echo's own events (spec §5.3, K12-04, K12-05, K13-03). Ask's chat view, ingest
//! runs and AI checks all read Claude Code through [`Parser`]: one stdout line in, zero or more events out (without `n`,
//! which the chat ring adds). The line shapes are recorded in `testdata/stream/` and written up in the design note's
//! *Chat protocol*.
//!
//! Everything in a stream is others' words: text and tool results are carried as data and a text block's HTML comes
//! only from the vault-page writer ([`page::render_fragment`], untrusted-text R3). The parser never panics on a line:
//! one that isn't JSON is counted and skipped, one it doesn't know is ignored, and every size it keeps is bounded.

// Nothing calls the parser until the chat process, ingest runs and AI checks (plan 2, Tasks 5 onward) do.
#![allow(dead_code)]

use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use super::{diff, page, Vault};

/// A streaming text block is rendered again at most this often (K12-05: 10 renders per second).
const RENDER_EVERY: Duration = Duration::from_millis(100);
/// A tool result's preview.
const PREVIEW_MAX: usize = 4096;
/// A text or thinking block holds at most this much (the rest of its deltas is dropped).
const BLOCK_MAX: usize = 2 << 20;
/// A tool's streamed input holds at most this much; a bigger one arrives as `{}`.
const INPUT_MAX: usize = 4 << 20;
/// A one-line summary.
const SUMMARY_MAX: usize = 160;
/// Strings in the input echoed in an event.
const INPUT_STRING_MAX: usize = 4096;
/// The steps file an ingest run keeps (K12-04).
const STEPS_FILE: &str = ".echo-ingest/steps.json";

enum Kind {
    Thinking,
    Text,
    Tool { id: String, name: String },
}

/// A content block being streamed.
struct Block {
    kind: Kind,
    id: String,
    /// A sub-agent's tool_use id (`parent_tool_use_id`).
    parent: Option<String>,
    text: String,
    /// A tool's input, as the fragments joined so far.
    json: String,
    started: Instant,
    last_render: Option<Instant>,
}

/// The state of one Claude Code process's stdout.
pub struct Parser {
    vault: Vault,
    bad: usize,
    session: Option<String>,
    blocks: HashMap<(Option<String>, u64), Block>,
    next_block: u32,
    /// Messages whose blocks came as stream events: their `assistant` lines repeat them.
    streamed: HashSet<String>,
    /// Tool calls announced: id → name, so a result and a repeated `assistant` line find them.
    tools: HashMap<String, String>,
    /// The last known content of the steps file.
    steps: Option<String>,
}

impl Parser {
    pub fn new(vault: &Vault) -> Parser {
        Parser { vault: vault.clone(), bad: 0, session: None, blocks: HashMap::new(), next_block: 0, streamed: HashSet::new(), tools: HashMap::new(), steps: None }
    }

    /// One stdout line → its events.
    pub fn feed(&mut self, line: &str) -> Vec<Value> {
        self.feed_at(line, Instant::now())
    }

    /// [`feed`](Self::feed) with the clock given (the throttle and a thinking block's seconds read it).
    pub fn feed_at(&mut self, line: &str, now: Instant) -> Vec<Value> {
        let line = line.trim();
        if line.is_empty() {
            return vec![];
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            self.bad += 1;
            return vec![];
        };
        match v["type"].as_str() {
            Some("stream_event") => self.stream_event(&v, now),
            Some("assistant") => self.assistant(&v, now),
            Some("user") => self.user(&v),
            Some("result") => self.result(&v),
            Some("prompt_suggestion") => match v["suggestion"].as_str().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => vec![json!({"type": "suggestion", "text": s})],
                None => vec![],
            },
            Some("control_request") => self.control_request(&v),
            Some("system") => self.system(&v),
            _ => vec![],
        }
    }

    /// The stream ended (or the process did): blocks still open are closed with what they hold. A tool whose input
    /// never completed gives no event.
    pub fn finish(&mut self) -> Vec<Value> {
        let mut keys: Vec<_> = self.blocks.keys().cloned().collect();
        keys.sort_by_key(|k| (self.blocks[k].started, k.1));
        let now = Instant::now();
        let mut out = vec![];
        for k in keys {
            if let Some(b) = self.blocks.remove(&k) {
                out.extend(self.close(b, now));
            }
        }
        out
    }

    /// Lines that were not JSON.
    pub fn bad_lines(&self) -> usize {
        self.bad
    }

    /// The session id of the latest `init` line (Echo reads it from there, not from what it passed).
    pub fn session_id(&self) -> Option<&str> {
        self.session.as_deref()
    }

    fn block_id(&mut self) -> String {
        self.next_block += 1;
        format!("b{}", self.next_block)
    }

    fn system(&mut self, v: &Value) -> Vec<Value> {
        match v["subtype"].as_str() {
            Some("init") => {
                self.session = v["session_id"].as_str().map(str::to_owned);
                vec![]
            }
            Some("permission_denied") => vec![json!({"type": "denied", "tool": v["tool_name"].as_str().unwrap_or(""), "reason": one_line(v["message"].as_str().unwrap_or(""), 300)})],
            _ => vec![],
        }
    }

    fn control_request(&mut self, v: &Value) -> Vec<Value> {
        let r = &v["request"];
        if r["subtype"] != "can_use_tool" {
            return vec![];
        }
        let name = r["tool_name"].as_str().unwrap_or("");
        let input = &r["input"];
        let mut e = Map::new();
        e.insert("type".into(), json!("permission"));
        e.insert("request".into(), v["request_id"].clone());
        e.insert("tool".into(), json!(name));
        e.insert("input".into(), cap_strings(input));
        if let Some(p) = tool_path(name, input, &self.vault) {
            e.insert("path".into(), json!(p));
        }
        if let Some(url) = input["url"].as_str() {
            e.insert("url".into(), json!(url));
            if let Some(d) = domain_of(url) {
                e.insert("domain".into(), json!(d));
            }
        }
        vec![Value::Object(e)]
    }

    fn result(&mut self, v: &Value) -> Vec<Value> {
        let u = &v["usage"];
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let tokens_in = n("input_tokens") + n("cache_read_input_tokens") + n("cache_creation_input_tokens");
        let aborted = v["terminal_reason"] == "aborted_streaming";
        let mut out = vec![];
        if v["is_error"] == true && !aborted {
            let text = v["result"].as_str().filter(|s| !s.trim().is_empty()).or_else(|| v["errors"][0].as_str()).unwrap_or("Claude Code stopped with an error");
            out.push(json!({"type": "error", "text": one_line(text, 300)}));
        }
        let mut e = Map::new();
        e.insert("type".into(), json!("result"));
        e.insert("tokens_in".into(), json!(tokens_in));
        e.insert("tokens_out".into(), json!(n("output_tokens")));
        e.insert("cache_read".into(), json!(if tokens_in == 0 { 0.0 } else { (n("cache_read_input_tokens") as f64 / tokens_in as f64 * 100.0).round() / 100.0 }));
        if let Some(c) = v["total_cost_usd"].as_f64().filter(|c| *c > 0.0) {
            e.insert("cost".into(), json!(c));
        }
        e.insert("seconds".into(), json!((v["duration_ms"].as_f64().unwrap_or(0.0) / 1000.0).round() as u64));
        let stop = v["stop_reason"].as_str().or_else(|| v["terminal_reason"].as_str()).or_else(|| v["subtype"].as_str());
        e.insert("stop".into(), json!(stop));
        out.push(Value::Object(e));
        out
    }

    fn stream_event(&mut self, v: &Value, now: Instant) -> Vec<Value> {
        let parent = v["parent_tool_use_id"].as_str().map(str::to_owned);
        let ev = &v["event"];
        let Some(index) = ev["index"].as_u64() else {
            if ev["type"] == "message_start" {
                if let Some(id) = ev["message"]["id"].as_str() {
                    self.streamed.insert(id.to_owned());
                }
            }
            return vec![];
        };
        let key = (parent.clone(), index);
        match ev["type"].as_str() {
            Some("content_block_start") => {
                let cb = &ev["content_block"];
                let kind = match cb["type"].as_str() {
                    Some("thinking") => Kind::Thinking,
                    Some("text") => Kind::Text,
                    Some("tool_use") => Kind::Tool { id: cb["id"].as_str().unwrap_or("").to_owned(), name: cb["name"].as_str().unwrap_or("").to_owned() },
                    _ => return vec![],
                };
                let thinking = matches!(kind, Kind::Thinking);
                let id = self.block_id();
                let text = cb["text"].as_str().or_else(|| cb["thinking"].as_str()).unwrap_or("");
                let b = Block { kind, id, parent, text: String::new(), json: String::new(), started: now, last_render: None };
                let mut b = b;
                push_capped(&mut b.text, text, BLOCK_MAX);
                let out = if thinking { vec![self.thinking_event(&b, false, now)] } else { vec![] };
                if thinking {
                    b.last_render = Some(now);
                }
                self.blocks.insert(key, b);
                out
            }
            Some("content_block_delta") => {
                let Some(mut b) = self.blocks.remove(&key) else { return vec![] };
                let d = &ev["delta"];
                let mut out = vec![];
                match (&b.kind, d["type"].as_str()) {
                    (Kind::Thinking, Some("thinking_delta")) => push_capped(&mut b.text, d["thinking"].as_str().unwrap_or(""), BLOCK_MAX),
                    (Kind::Text, Some("text_delta")) => push_capped(&mut b.text, d["text"].as_str().unwrap_or(""), BLOCK_MAX),
                    (Kind::Tool { .. }, Some("input_json_delta")) => push_capped(&mut b.json, d["partial_json"].as_str().unwrap_or(""), INPUT_MAX),
                    _ => {}
                }
                let due = b.last_render.is_none_or(|t| now.saturating_duration_since(t) >= RENDER_EVERY);
                if due && !b.text.is_empty() && !matches!(b.kind, Kind::Tool { .. }) {
                    b.last_render = Some(now);
                    out.push(match b.kind {
                        Kind::Thinking => self.thinking_event(&b, false, now),
                        _ => self.text_event(&b, false),
                    });
                }
                self.blocks.insert(key, b);
                out
            }
            Some("content_block_stop") => match self.blocks.remove(&key) {
                Some(b) => self.close(b, now),
                None => vec![],
            },
            _ => vec![],
        }
    }

    /// A block ends: its last event.
    fn close(&mut self, b: Block, now: Instant) -> Vec<Value> {
        match &b.kind {
            Kind::Thinking => vec![self.thinking_event(&b, true, now)],
            Kind::Text if b.text.is_empty() => vec![],
            Kind::Text => vec![self.text_event(&b, true)],
            Kind::Tool { id, name } => {
                let input = serde_json::from_str::<Value>(&b.json).ok().filter(Value::is_object).unwrap_or_else(|| json!({}));
                self.tool_events(id, name, &input, b.parent.as_deref())
            }
        }
    }

    fn thinking_event(&self, b: &Block, done: bool, now: Instant) -> Value {
        let mut e = json!({"type": "thinking", "block": b.id, "text": b.text, "done": done});
        if done {
            e["seconds"] = json!(now.saturating_duration_since(b.started).as_secs_f64().round() as u64);
        }
        with_parent(e, b.parent.as_deref())
    }

    fn text_event(&self, b: &Block, done: bool) -> Value {
        with_parent(json!({"type": "text", "block": b.id, "html": page::render_fragment(&self.vault, &b.text), "done": done}), b.parent.as_deref())
    }

    /// A complete tool call: its `tool` event (once per id) and, for the steps file, a `todo` event.
    fn tool_events(&mut self, id: &str, name: &str, input: &Value, parent: Option<&str>) -> Vec<Value> {
        if self.tools.contains_key(id) {
            return vec![];
        }
        if self.tools.len() > 4000 {
            self.tools.clear();
        }
        self.tools.insert(id.to_owned(), name.to_owned());
        let (verb, target, detail, readonly) = tool_line(name, input, &self.vault);
        let mut e = Map::new();
        e.insert("type".into(), json!("tool"));
        e.insert("id".into(), json!(id));
        e.insert("name".into(), json!(name));
        e.insert("verb".into(), json!(verb));
        e.insert("target".into(), json!(target));
        e.insert("detail".into(), json!(detail));
        if let Some(p) = tool_path(name, input, &self.vault) {
            e.insert("path".into(), json!(p));
        }
        e.insert("readonly".into(), json!(readonly));
        e.insert("input".into(), cap_strings(input));
        let mut out = vec![with_parent(Value::Object(e), parent)];
        out.extend(self.steps_event(name, input));
        out
    }

    /// A `Write` or `Edit` of the steps file → the run's steps (print mode has no `TodoWrite`, ruling R6).
    fn steps_event(&mut self, name: &str, input: &Value) -> Option<Value> {
        let path = input["file_path"].as_str()?.replace('\\', "/");
        if path != STEPS_FILE && !path.ends_with(&format!("/{STEPS_FILE}")) {
            return None;
        }
        let text = match name {
            "Write" => input["content"].as_str()?.to_owned(),
            "Edit" => {
                let (old, new) = (input["old_string"].as_str()?, input["new_string"].as_str()?);
                let cur = self.steps.as_deref()?;
                if old.is_empty() || !cur.contains(old) {
                    return None;
                }
                if input["replace_all"] == true { cur.replace(old, new) } else { cur.replacen(old, new, 1) }
            }
            _ => return None,
        };
        let items = parse_steps(&text);
        self.steps = Some(text);
        items.map(|items| json!({"type": "todo", "items": items}))
    }

    /// The finished message, one content block per line. With partial messages its text and thinking were already
    /// streamed; a tool call is announced once by its id either way.
    fn assistant(&mut self, v: &Value, now: Instant) -> Vec<Value> {
        let parent = v["parent_tool_use_id"].as_str().map(str::to_owned);
        let msg = &v["message"];
        let streamed = msg["id"].as_str().is_some_and(|m| self.streamed.contains(m));
        let mut out = vec![];
        for b in msg["content"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            match b["type"].as_str() {
                Some("tool_use") => out.extend(self.tool_events(b["id"].as_str().unwrap_or(""), b["name"].as_str().unwrap_or(""), &b["input"], parent.as_deref())),
                Some("text") if !streamed => {
                    let text = b["text"].as_str().unwrap_or("");
                    if !text.is_empty() {
                        let blk = Block { kind: Kind::Text, id: self.block_id(), parent: parent.clone(), text: cap(text, BLOCK_MAX), json: String::new(), started: now, last_render: None };
                        out.push(self.text_event(&blk, true));
                    }
                }
                Some("thinking") if !streamed => {
                    let blk = Block { kind: Kind::Thinking, id: self.block_id(), parent: parent.clone(), text: cap(b["thinking"].as_str().unwrap_or(""), BLOCK_MAX), json: String::new(), started: now, last_render: None };
                    out.push(self.thinking_event(&blk, true, now));
                }
                _ => {}
            }
        }
        out
    }

    /// A tool's result.
    fn user(&mut self, v: &Value) -> Vec<Value> {
        let mut out = vec![];
        for b in v["message"]["content"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            if b["type"] != "tool_result" {
                continue;
            }
            let id = b["tool_use_id"].as_str().unwrap_or("");
            let name = self.tools.get(id).map(String::as_str).unwrap_or("");
            let content = match &b["content"] {
                Value::String(s) => s.clone(),
                Value::Array(a) => a.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
                _ => String::new(),
            };
            let ok = b["is_error"] != true;
            let (summary, preview, lines) = tool_result_summary(name, &content, !ok);
            let mut e = json!({"type": "tool_result", "id": id, "ok": ok, "summary": summary, "preview": preview});
            if let Some((a, z)) = lines {
                e["lines"] = json!([a, z]);
            }
            out.push(with_parent(e, v["parent_tool_use_id"].as_str()));
        }
        out
    }
}

fn with_parent(mut e: Value, parent: Option<&str>) -> Value {
    if let Some(p) = parent {
        e["parent"] = json!(p);
    }
    e
}

/// `s` appended to `buf` while `buf` stays within `max` bytes (cut on a character boundary).
fn push_capped(buf: &mut String, s: &str, max: usize) {
    let room = max.saturating_sub(buf.len());
    buf.push_str(&cap(s, room));
}

/// At most `max` bytes of `s`, cut on a character boundary.
fn cap(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_owned();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

/// The first non-empty line of `s`, whitespace collapsed, at most `max` characters.
fn one_line(s: &str, max: usize) -> String {
    let first = s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let flat = first.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= max {
        return flat;
    }
    let cut: String = flat.chars().take(max.saturating_sub(1)).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// The input with every string cut to a size the window can carry.
fn cap_strings(v: &Value) -> Value {
    match v {
        Value::String(s) if s.len() > INPUT_STRING_MAX => json!(format!("{}\u{2026}", cap(s, INPUT_STRING_MAX))),
        Value::Array(a) => Value::Array(a.iter().map(cap_strings).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), cap_strings(v))).collect()),
        other => other.clone(),
    }
}

/// The steps file's text as `[{"text", "state"}]` (an unknown state is `todo`; entries without text are dropped).
fn parse_steps(text: &str) -> Option<Vec<Value>> {
    let items: Vec<Value> = serde_json::from_str::<Value>(text).ok()?.as_array()?.iter().filter_map(|i| {
        let t = i["text"].as_str().map(str::trim).filter(|t| !t.is_empty())?;
        let state = i["state"].as_str().filter(|s| matches!(*s, "active" | "done")).unwrap_or("todo");
        Some(json!({"text": cap(t, 300), "state": state}))
    }).collect();
    Some(items)
}

/// `path` (as Claude Code names it) relative to the vault, with `/`, when it is inside; a relative path is taken as
/// vault-relative. `None` for a path outside or with `..`.
fn vault_rel(v: &Vault, path: &str) -> Option<String> {
    let p = path.replace('\\', "/");
    let rel = if Path::new(&p).is_absolute() || p.starts_with('/') {
        let root = v.root.to_string_lossy().replace('\\', "/");
        let canon = v.root.canonicalize().ok().map(|c| c.to_string_lossy().replace('\\', "/"));
        [Some(root), canon].into_iter().flatten().find_map(|r| p.strip_prefix(r.trim_end_matches('/')).filter(|rest| rest.is_empty() || rest.starts_with('/')).map(|s| s.trim_start_matches('/').to_owned()))?
    } else {
        p
    };
    let rel = rel.trim_end_matches('/').to_owned();
    (!rel.split('/').any(|c| c == "..")).then_some(rel)
}

/// The vault-relative path a tool call names (`file_path`, `notebook_path`, or `path` of a search), when inside the
/// vault.
pub fn tool_path(_name: &str, input: &Value, v: &Vault) -> Option<String> {
    let p = ["file_path", "notebook_path", "path"].iter().find_map(|k| input[k].as_str())?;
    vault_rel(v, p).filter(|r| !r.is_empty())
}

/// `example.com` from a URL (no userinfo, port or case).
fn domain_of(url: &str) -> Option<String> {
    let (_, rest) = url.split_once("://")?;
    let auth = rest.split(['/', '?', '#']).next()?;
    let host = auth.rsplit('@').next()?;
    let host = if host.starts_with('[') { host.split_once(']').map_or(host, |(h, _)| h.trim_start_matches('[')) } else { host.split(':').next()? };
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// `example.com/pricing` from a URL, cut at 60 characters.
fn short_url(url: &str) -> String {
    let Some(host) = domain_of(url) else { return one_line(url, 60) };
    let rest = url.split_once("://").map_or("", |(_, r)| r);
    let path = rest.find(['/', '?', '#']).map_or("", |i| &rest[i..]);
    let path = path.split(['?', '#']).next().unwrap_or("");
    let s = format!("{host}{}", if path == "/" { "" } else { path });
    if s.chars().count() <= 60 {
        return s;
    }
    format!("{}\u{2026}", s.chars().take(60).collect::<String>())
}

/// A tool call as one row's words (K13-03): `(verb, target, detail, readonly)`.
pub fn tool_line(name: &str, input: &Value, v: &Vault) -> (String, String, String, bool) {
    let s = |k: &str| input[k].as_str().unwrap_or("");
    let shown = |p: &str| vault_rel(v, p).filter(|r| !r.is_empty()).unwrap_or_else(|| p.to_owned());
    let plural = |n: usize, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
    let (verb, target, detail, readonly) = match name {
        "Read" => {
            let (off, lim) = (input["offset"].as_u64(), input["limit"].as_u64());
            let detail = match (off, lim) {
                (Some(o), Some(l)) if l > 0 => format!("lines {o}\u{2013}{}", o + l - 1),
                (None, Some(l)) if l > 0 => format!("lines 1\u{2013}{l}"),
                (Some(o), _) if o > 1 => format!("from line {o}"),
                _ => String::new(),
            };
            ("Read".to_string(), shown(s("file_path")), detail, true)
        }
        "Grep" | "Glob" => {
            let dir = vault_rel(v, s("path")).filter(|r| !r.is_empty() && !s("path").is_empty());
            let detail = dir.map_or(String::new(), |d| format!("in {d}{}", if super::join(&v.root, &d).is_dir() { "/" } else { "" }));
            let target = if name == "Grep" { format!("\"{}\"", s("pattern")) } else { s("pattern").to_owned() };
            ("Searched".to_string(), target, detail, true)
        }
        "Skill" => ("Used skill".to_string(), s("skill").to_owned(), String::new(), true),
        "Edit" => {
            let (plus, minus) = diff::counts(&diff::lines(s("old_string"), s("new_string")));
            ("Edited".to_string(), shown(s("file_path")), format!("+{plus} \u{2212}{minus}"), false)
        }
        "Write" => {
            let exists = vault_rel(v, s("file_path")).is_some_and(|r| super::join(&v.root, &r).exists());
            let n = s("content").lines().count();
            (if exists { "Wrote" } else { "Created" }.to_string(), shown(s("file_path")), plural(n, "line"), false)
        }
        "WebSearch" => ("Searched the web".to_string(), format!("\"{}\"", s("query")), String::new(), true),
        "WebFetch" => ("Opened".to_string(), short_url(s("url")), String::new(), true),
        "Agent" | "Task" => ("Delegated".to_string(), one_line(s("description"), 80), String::new(), false),
        other => (other.to_owned(), String::new(), String::new(), false),
    };
    (verb, one_line(&target, 200), detail, readonly)
}

/// A tool's result: a one-line summary, a preview of at most 4 KB and, for a read, the line range.
pub fn tool_result_summary(name: &str, content: &str, is_error: bool) -> (String, String, Option<(u64, u64)>) {
    let mut preview = cap(content, PREVIEW_MAX - 3);
    if preview.len() < content.len() {
        preview.push('\u{2026}');
    }
    let first = || one_line(content, SUMMARY_MAX);
    let n = content.lines().filter(|l| !l.trim().is_empty()).count();
    let mut lines = None;
    let summary = if is_error {
        first()
    } else {
        match name {
            "Read" => {
                let num = |l: &str| l.split_once('\t').and_then(|(a, _)| a.trim().parse::<u64>().ok());
                lines = content.lines().find_map(num).zip(content.lines().rev().find_map(num));
                match lines {
                    Some((a, z)) => format!("{} lines", z - a.min(z) + 1),
                    None => first(),
                }
            }
            "Grep" if content.trim().is_empty() || content.starts_with("No ") => "no matches".to_owned(),
            "Grep" => {
                let hits: Vec<&str> = content.lines().filter(|l| l.split(':').nth(1).is_some_and(|m| !m.is_empty() && m.bytes().all(|b| b.is_ascii_digit()))).collect();
                if hits.is_empty() {
                    format!("{n} file{}", if n == 1 { "" } else { "s" })
                } else {
                    let files: HashSet<&str> = hits.iter().filter_map(|l| l.split(':').next()).collect();
                    format!("{} match{} in {} file{}", hits.len(), if hits.len() == 1 { "" } else { "es" }, files.len(), if files.len() == 1 { "" } else { "s" })
                }
            }
            "Glob" if content.trim().is_empty() || content.starts_with("No ") => "no files".to_owned(),
            "Glob" => format!("{n} file{}", if n == 1 { "" } else { "s" }),
            _ => match first() {
                f if f.is_empty() => "done".to_owned(),
                f => f,
            },
        }
    };
    (summary, preview, lines)
}

#[cfg(test)]
mod tests;

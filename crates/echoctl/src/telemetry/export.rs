//! Sending traces elsewhere (#859, phase 4): to the user's Langfuse or any OTLP collector they name, off until they set
//! it up (Settings → Activity, `set`). The app's exporter (`start`) sends each trace whose run ended at least a minute
//! ago (Claude Code's own spans are in by then), as OTLP/JSON over HTTP, through `curl` (its URL, headers and body from
//! files only the user can read, never on a command line). A failure is kept (`status`) and tried again later; it never
//! touches a job. What leaves: spans and their events, never the account's identity (email, ids, organisation) nor a
//! path on this computer; what went in and came out (prompts, replies, tool input and output, instructions) only when
//! the user said so (`text`). Spans carry Langfuse's names too, so model calls show as generations.

use super::{lines, valid_id, DIR, INDEX};
use crate::paths;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, Utc};
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

const FILE: &str = "telemetry-export.json";
const STATE: &str = "export-state.json";
/// Traces sent a pass at most.
const PER_PASS: usize = 50;
/// A trace is sent once its run ended this long ago (Claude Code's spans come a few seconds after it).
const SETTLE_SECS: i64 = 60;

/// Where traces go, as the user set it.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Settings {
    #[serde(default)]
    pub on: bool,
    /// `langfuse` or `otlp`.
    #[serde(default)]
    pub kind: String,
    /// Langfuse: its host (`https://cloud.langfuse.com`); OTLP: the collector's traces URL or its base.
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub public_key: String,
    #[serde(default)]
    pub secret_key: String,
    /// OTLP: extra headers, `Name: value` each.
    #[serde(default)]
    pub headers: Vec<String>,
    /// Send what went in and came out too.
    #[serde(default)]
    pub text: bool,
}

fn file() -> PathBuf {
    paths::config_dir().join(FILE)
}

pub fn load() -> Settings {
    std::fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// An address traces may go to: `https://`, or `http://` on this computer.
fn address_ok(url: &str) -> bool {
    let u = url.trim();
    let local = ["http://127.0.0.1", "http://localhost", "http://[::1]"].iter().any(|p| u.starts_with(p) && u[p.len()..].chars().next().is_none_or(|c| c == ':' || c == '/'));
    (u.starts_with("https://") || local) && !u.chars().any(|c| c.is_whitespace() || c == '"' || c.is_control())
}

/// Check what the page or the CLI sends, then keep it (readable only by the user: it holds a key). A key left empty
/// keeps the one kept (the page never sees it).
pub fn set(v: &Value) -> Result<Settings> {
    let old = load();
    let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
    let mut new = Settings {
        on: v["on"].as_bool().unwrap_or(false),
        kind: s("kind"),
        endpoint: s("endpoint").trim_end_matches('/').to_string(),
        public_key: s("public_key"),
        secret_key: s("secret_key"),
        headers: v["headers"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect(),
        text: v["text"].as_bool().unwrap_or(false),
    };
    if new.secret_key.is_empty() && new.kind == old.kind {
        new.secret_key = old.secret_key.clone();
    }
    if new.headers.is_empty() && v["keep_headers"] == true && new.kind == old.kind {
        new.headers = old.headers.clone();
    }
    if new.on {
        match new.kind.as_str() {
            "langfuse" | "otlp" => {}
            _ => bail!("send traces to Langfuse or an OTLP collector"),
        }
        if !address_ok(&new.endpoint) {
            bail!("“{}” isn't an address traces may go to: https://…, or http:// on this computer", new.endpoint);
        }
        if new.kind == "langfuse" && (new.public_key.is_empty() || new.secret_key.is_empty()) {
            bail!("Langfuse needs its public key (pk-lf-…) and secret key (sk-lf-…)");
        }
        for h in &new.headers {
            let ok = h.split_once(':').is_some_and(|(n, _)| !n.trim().is_empty() && n.trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '-')) && !h.chars().any(char::is_control);
            if !ok {
                bail!("“{}” isn't a header (Name: value)", h.split(':').next().unwrap_or(""));
            }
        }
    }
    write_private(&file(), &serde_json::to_vec_pretty(&new)?)?;
    Ok(new)
}

/// What the page and `echoctl traces export` show: the settings with the secret only as "set", and how sending goes.
pub fn status() -> Value {
    let s = load();
    let st = state();
    json!({"on": s.on, "kind": s.kind, "endpoint": s.endpoint, "public_key": s.public_key, "secret_key_set": !s.secret_key.is_empty(),
           "headers": s.headers.iter().map(|h| h.split(':').next().unwrap_or("").trim().to_string()).collect::<Vec<_>>(), "text": s.text,
           "sent_until": st["sent_until"], "sent": st["sent"], "last_sent": st["last_sent"], "error": st["error"], "error_at": st["error_at"]})
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("a folder")?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{}.{}.tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("x"), std::process::id()));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ── what a trace becomes ──────────────────────────────────────────────────────────────────────

/// Attributes that name the person or this computer: never sent.
fn identity(k: &str) -> bool {
    k.starts_with("user.") || k.starts_with("organization.") || k == "cwd" || k == "terminal.type" || k == "host.name" || k.starts_with("anthropic.")
}

/// Attributes that are what went in or came out: sent only with `text`.
fn text_attr(k: &str) -> bool {
    matches!(k, "args" | "files" | "system" | "input" | "output" | "query" | "prompt" | "response" | "user_prompt" | "used" | "title")
}

/// A JSON value as an OTLP attribute value (objects and arrays as JSON text).
fn value(v: &Value) -> Value {
    match v {
        Value::Bool(b) => json!({"boolValue": b}),
        Value::Number(n) if n.is_i64() || n.is_u64() => json!({"intValue": n.to_string()}),
        Value::Number(n) => json!({"doubleValue": n.as_f64().unwrap_or(0.0)}),
        Value::String(s) => json!({"stringValue": s}),
        other => json!({"stringValue": other.to_string()}),
    }
}

fn attributes(m: &Map<String, Value>) -> Vec<Value> {
    m.iter().filter(|(_, v)| !v.is_null()).map(|(k, v)| json!({"key": k, "value": value(v)})).collect()
}

fn nanos(t: &Value) -> String {
    t.as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).and_then(|d| d.with_timezone(&Utc).timestamp_nanos_opt()).unwrap_or(0).to_string()
}

/// The text of a stream-json message (Echo's `claude.input`), else the text as it is.
fn message_text(t: &str) -> String {
    serde_json::from_str::<Value>(t)
        .ok()
        .and_then(|m| match &m["message"]["content"] {
            Value::String(s) => Some(s.clone()),
            Value::Array(b) => Some(b.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join("\n")),
            _ => None,
        })
        .unwrap_or_else(|| t.to_string())
}

/// One trace's records (as `show` reads them, flat) as an OTLP/JSON request: Echo's spans under Echo, Claude Code's
/// under its own name; identity never, text only with `text`; Langfuse's names on the root and the model calls.
pub fn to_otlp(records: &[Value], text: bool) -> Value {
    let spans: Vec<&Value> = records.iter().filter(|r| r["t"] == "span").collect();
    let ids: std::collections::HashSet<&str> = spans.iter().filter_map(|s| s["span"].as_str()).collect();
    let mut by_source: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    for s in &spans {
        let id = s["span"].as_str().unwrap_or("");
        let mut a: Map<String, Value> = s["attrs"].as_object().cloned().unwrap_or_default();
        let events: Vec<&Value> = records.iter().filter(|r| r["t"] == "event" && r["span"].as_str() == Some(id)).collect();
        // Langfuse's names: the trace's, and a model call's (type, model, usage, cost, input and output).
        if s["parent"].is_null() || !s["parent"].as_str().is_some_and(|p| ids.contains(p)) {
            a.insert("langfuse.trace.name".into(), s["name"].clone());
        }
        let n = |a: &Map<String, Value>, k: &str| a.get(k).and_then(Value::as_u64).unwrap_or(0);
        match s["name"].as_str() {
            Some("claude.run") => {
                let (i, o) = (n(&a, "input") + n(&a, "cache_read") + n(&a, "cache_write"), n(&a, "output"));
                a.insert("langfuse.observation.type".into(), json!("generation"));
                if let Some(m) = a.get("model").cloned() {
                    a.insert("gen_ai.request.model".into(), m);
                }
                a.insert("gen_ai.usage.input_tokens".into(), json!(i));
                a.insert("gen_ai.usage.output_tokens".into(), json!(o));
                if let Some(c) = a.get("cost_usd").cloned() {
                    a.insert("gen_ai.usage.cost".into(), c);
                }
                if text {
                    let input = events.iter().find(|e| e["name"] == "claude.input").and_then(|e| e["attrs"]["text"].as_str()).map(message_text);
                    let output = events.iter().find(|e| e["name"].as_str().is_some_and(|n| n.starts_with("claude.result"))).and_then(|e| e["attrs"]["result"].as_str().map(str::to_owned));
                    if let Some(i) = input {
                        a.insert("langfuse.observation.input".into(), json!(Value::String(i).to_string()));
                    }
                    if let Some(o) = output {
                        a.insert("langfuse.observation.output".into(), json!(Value::String(o).to_string()));
                    }
                }
            }
            Some("model.request") => {
                a.insert("langfuse.observation.type".into(), json!("generation"));
                if let Some(m) = a.get("model").cloned() {
                    a.insert("gen_ai.request.model".into(), m);
                }
                let u = a.get("usage").cloned().unwrap_or(Value::Null);
                a.insert("gen_ai.usage.input_tokens".into(), u["input_tokens"].clone());
                a.insert("gen_ai.usage.output_tokens".into(), u["output_tokens"].clone());
                if text {
                    a.insert("langfuse.observation.input".into(), json!(a.get("input").cloned().unwrap_or(Value::Null).to_string()));
                    a.insert("langfuse.observation.output".into(), json!(a.get("output").cloned().unwrap_or(Value::Null).to_string()));
                }
            }
            _ => {}
        }
        a.retain(|k, _| !identity(k) && (text || !text_attr(k)));
        let evs: Vec<Value> = events
            .iter()
            .filter(|e| text || e["source"] == "claude-code")
            .map(|e| {
                let mut ea: Map<String, Value> = match &e["attrs"] {
                    Value::Object(m) => m.clone(),
                    other => Map::from_iter([("value".to_string(), other.clone())]),
                };
                ea.retain(|k, _| !identity(k) && (text || !text_attr(k)));
                json!({"name": e["name"], "timeUnixNano": nanos(&e["at"]), "attributes": attributes(&ea)})
            })
            .collect();
        let failed = a.get("ok") == Some(&json!(false)) || a.get("success") == Some(&json!("false"));
        let mut span = json!({"traceId": s["trace"], "spanId": s["span"], "name": s["name"], "kind": 1,
            "startTimeUnixNano": nanos(&s["start"]), "endTimeUnixNano": nanos(&s["end"]), "attributes": attributes(&a), "events": evs,
            "status": if failed { json!({"code": 2, "message": a.get("error").and_then(Value::as_str).filter(|_| text).unwrap_or("failed")}) } else { json!({"code": 1}) }});
        if let Some(p) = s["parent"].as_str() {
            span["parentSpanId"] = json!(p);
        }
        let source = if s["source"] == "claude-code" { "claude-code" } else { "echo" };
        by_source.entry(source.to_string()).or_default().push(span);
    }
    let resource_spans: Vec<Value> = by_source
        .into_iter()
        .map(|(source, spans)| {
            let version = if source == "echo" { env!("CARGO_PKG_VERSION").to_string() } else { String::new() };
            let mut r = Map::new();
            r.insert("service.name".into(), json!(source));
            if !version.is_empty() {
                r.insert("service.version".into(), json!(version));
            }
            json!({"resource": {"attributes": attributes(&r)}, "scopeSpans": [{"scope": {"name": "echo"}, "spans": spans}]})
        })
        .collect();
    json!({"resourceSpans": resource_spans})
}

// ── sending ──────────────────────────────────────────────────────────────────────────────────

fn state_file() -> PathBuf {
    paths::config_dir().join(DIR).join(STATE)
}

fn state() -> Value {
    std::fs::read(state_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

fn save_state(v: &Value) {
    let _ = std::fs::create_dir_all(state_file().parent().expect("a folder"));
    let _ = paths::write_atomic(&state_file(), &serde_json::to_vec_pretty(v).unwrap_or_default());
}

/// Where a request goes, and its headers.
fn target(s: &Settings) -> (String, Vec<String>) {
    use base64::Engine;
    match s.kind.as_str() {
        "langfuse" => {
            let auth = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", s.public_key, s.secret_key));
            (format!("{}/api/public/otel/v1/traces", s.endpoint), vec![format!("Authorization: Basic {auth}"), "x-langfuse-ingestion-version: 4".into()])
        }
        _ => {
            let url = if s.endpoint.ends_with("/v1/traces") { s.endpoint.clone() } else { format!("{}/v1/traces", s.endpoint) };
            (url, s.headers.clone())
        }
    }
}

/// curl's config: a value quoted as its config file reads it.
fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
}

/// Post `body` where the settings say: Ok, or why not (its status and the answer's start).
fn post(s: &Settings, body: &Value) -> Result<()> {
    let (url, headers) = target(s);
    let dir = paths::config_dir().join(DIR);
    let n = format!("{}-{}", std::process::id(), Utc::now().timestamp_nanos_opt().unwrap_or(0));
    let (cfg_path, body_path) = (dir.join(format!(".export-{n}.cfg")), dir.join(format!(".export-{n}.json")));
    let mut cfg = format!("url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\nmax-time = 30\nsilent\nshow-error\nwrite-out = \"\\n%{{http_code}}\"\n", quote(&url));
    for h in &headers {
        cfg.push_str(&format!("header = {}\n", quote(h)));
    }
    cfg.push_str(&format!("data-binary = {}\n", quote(&format!("@{}", body_path.display()))));
    write_private(&body_path, &serde_json::to_vec(body)?)?;
    write_private(&cfg_path, cfg.as_bytes())?;
    let mut c = std::process::Command::new("curl");
    #[cfg(windows)]
    {
        use crate::app::NoWindow;
        c = c.no_window();
    }
    let out = c.args(["-K"]).arg(&cfg_path).output();
    let _ = std::fs::remove_file(&cfg_path);
    let _ = std::fs::remove_file(&body_path);
    let out = out.context("running curl")?;
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (answer, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
    let code: u16 = code.trim().parse().unwrap_or(0);
    if !out.status.success() || !(200..300).contains(&code) {
        let why = if code == 0 { String::from_utf8_lossy(&out.stderr).trim().to_string() } else { format!("HTTP {code}: {}", answer.chars().take(200).collect::<String>()) };
        bail!("{why}");
    }
    Ok(())
}

/// The traces ended after `after` (an index line's `end`) and before `before`, oldest first, their day folder each.
fn ended(after: &str, before: DateTime<Utc>) -> Vec<(PathBuf, Value)> {
    let base = paths::config_dir().join(DIR);
    let mut out: Vec<(PathBuf, Value)> = Vec::new();
    for back in (0..2).rev() {
        let day = (Local::now().date_naive() - chrono::Duration::days(back)).format("%Y-%m-%d").to_string();
        for v in lines(&base.join(&day).join(INDEX)) {
            let end = v["end"].as_str().unwrap_or("");
            let settled = DateTime::parse_from_rfc3339(end).is_ok_and(|e| e.with_timezone(&Utc) <= before);
            if end > after && settled && v["id"].as_str().is_some_and(|i| valid_id(i, 32)) {
                out.push((base.join(&day), v));
            }
        }
    }
    out.sort_by(|a, b| a.1["end"].as_str().cmp(&b.1["end"].as_str()));
    out
}

/// One pass: the traces ended since the last sent, at most `PER_PASS`, sent in order; stops at the first that fails
/// (kept as the error, tried again next pass). Returns how many went.
pub fn pass(now: DateTime<Utc>) -> Result<usize> {
    let s = load();
    if !s.on {
        return Ok(0);
    }
    let mut st = state();
    // Never the past before it was turned on: from now on.
    let after = st["sent_until"].as_str().map(str::to_owned).unwrap_or_else(|| now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    if st["sent_until"].is_null() {
        st["sent_until"] = json!(after);
        save_state(&st);
    }
    let mut sent = 0;
    for (day, line) in ended(&after, now - chrono::Duration::seconds(SETTLE_SECS)).into_iter().take(PER_PASS) {
        let id = line["id"].as_str().unwrap_or("");
        let records = lines(&day.join(format!("{id}.jsonl")));
        if !records.is_empty() {
            if let Err(e) = post(&s, &to_otlp(&records, s.text)) {
                st["error"] = json!(format!("{e:#}"));
                st["error_at"] = json!(now.to_rfc3339());
                save_state(&st);
                return Err(e);
            }
            sent += 1;
            st["last_sent"] = json!(now.to_rfc3339());
            st["sent"] = json!(st["sent"].as_u64().unwrap_or(0) + 1);
        }
        st["sent_until"] = line["end"].clone();
        st["error"] = Value::Null;
        save_state(&st);
    }
    Ok(sent)
}

/// Send one trace now (the page's "Send a test": the newest that ended), whatever was sent before. Its id, or why not.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn send_newest() -> Result<String> {
    let s = load();
    if !s.on {
        bail!("sending traces is off");
    }
    let (day, line) = ended("", Utc::now()).into_iter().last().context("no trace to send yet: Echo hasn't run anything today")?;
    let id = line["id"].as_str().unwrap_or("").to_string();
    post(&s, &to_otlp(&lines(&day.join(format!("{id}.jsonl"))), s.text))?;
    Ok(id)
}

/// The app's exporter: a pass every 30 s while sending is on; after a failure, the next try in 5 min.
pub fn start() {
    let _ = std::thread::Builder::new().name("telemetry-export".into()).spawn(|| {
        let mut wait = Duration::from_secs(30);
        loop {
            std::thread::sleep(wait);
            wait = match pass(Utc::now()) {
                Ok(_) => Duration::from_secs(30),
                Err(_) => Duration::from_secs(300),
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::testenv;
    use std::io::{BufRead, BufReader, Read};
    use std::sync::{Arc, Mutex};

    /// A collector on this computer answering `codes` in turn: each request's first line, headers and body.
    fn collector(codes: Vec<u16>) -> (String, Arc<Mutex<Vec<(String, Vec<String>, String)>>>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let got = Arc::new(Mutex::new(Vec::new()));
        let g = got.clone();
        std::thread::spawn(move || {
            for code in codes {
                let Ok((mut s, _)) = l.accept() else { return };
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut first = String::new();
                r.read_line(&mut first).unwrap();
                let (mut headers, mut len) = (Vec::new(), 0);
                loop {
                    let mut h = String::new();
                    r.read_line(&mut h).unwrap();
                    if h.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                    headers.push(h.trim().to_string());
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                g.lock().unwrap().push((first.trim().to_string(), headers, String::from_utf8(body).unwrap()));
                let _ = s.write_all(format!("HTTP/1.1 {code} X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").as_bytes());
            }
        });
        (url, got)
    }

    /// A finished job's trace with a Claude Code run in it (what went in, its result, Claude Code's own request).
    fn a_trace() -> String {
        let run = crate::jobs::begin("needs");
        let p = crate::ai::fake::profile("work", None, crate::ai::fake::SIGNED_OUT);
        super::super::claude_run(&p, &paths::config_dir(), &["-p".into(), "the prompt".into()]);
        super::super::claude_input("what went in");
        super::super::claude_event(&json!({"type": "result", "is_error": false, "result": "what came out", "usage": {"input_tokens": 10, "output_tokens": 5}, "total_cost_usd": 0.01}));
        run.done(Some("checked".into()));
        super::super::flush(Duration::from_secs(5));
        super::super::list(1, None, None, 1)["traces"][0]["id"].as_str().unwrap().to_string()
    }

    fn records(id: &str) -> Vec<Value> {
        let day = Local::now().format("%Y-%m-%d").to_string();
        lines(&paths::config_dir().join(DIR).join(day).join(format!("{id}.jsonl")))
    }

    #[test]
    fn where_traces_go_is_checked_kept_privately_and_shown_without_its_secret() {
        let (_env, _root) = testenv::folders("export-settings");
        let refused = |v: Value| set(&v).unwrap_err().to_string();
        assert!(refused(json!({"on": true, "kind": "otlp", "endpoint": "http://collector.acme.example:4318"})).contains("isn't an address"), "plain http elsewhere");
        assert!(refused(json!({"on": true, "kind": "langfuse", "endpoint": "https://cloud.langfuse.com", "public_key": "pk-lf-1"})).contains("secret key"));
        assert!(refused(json!({"on": true, "kind": "otlp", "endpoint": "https://otel.acme.example", "headers": ["X Bad: 1"]})).contains("isn't a header"));
        assert!(refused(json!({"on": true, "kind": "zipkin", "endpoint": "https://z.acme.example"})).contains("Langfuse or an OTLP"));
        set(&json!({"on": true, "kind": "otlp", "endpoint": "http://localhost:4318/"})).unwrap();
        set(&json!({"on": true, "kind": "langfuse", "endpoint": "https://cloud.langfuse.com", "public_key": "pk-lf-1", "secret_key": "sk-lf-secret"})).unwrap();
        // The page sends the secret empty (it never has it): the kept one stays.
        let kept = set(&json!({"on": true, "kind": "langfuse", "endpoint": "https://cloud.langfuse.com", "public_key": "pk-lf-1", "secret_key": "", "text": true})).unwrap();
        assert_eq!((kept.secret_key.as_str(), kept.text), ("sk-lf-secret", true));
        let st = status();
        assert_eq!((st["secret_key_set"].clone(), st["public_key"].clone()), (json!(true), json!("pk-lf-1")));
        assert!(!st.to_string().contains("sk-lf-secret"), "never the secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(file()).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn a_trace_leaves_without_identity_and_with_its_text_only_when_allowed() {
        let (_env, _root) = testenv::folders("export-otlp");
        let id = a_trace();
        let mut recs = records(&id);
        // Claude Code's own event, with the account's identity in it (as it sends them).
        let run = recs.iter().find(|r| r["name"] == "claude.run").unwrap()["span"].as_str().unwrap().to_string();
        recs.push(json!({"t": "event", "source": "claude-code", "trace": id, "span": run, "name": "api_request", "at": "2026-11-20T12:00:00.000Z",
                         "attrs": {"user.email": "sam@initech.example", "organization.id": "org-1", "input_tokens": 10, "cost_usd": 0.01}}));
        let plain = to_otlp(&recs, false).to_string();
        for gone in ["sam@initech.example", "org-1", "what went in", "what came out", "the prompt", "\"cwd\""] {
            assert!(!plain.contains(gone), "{gone} isn't sent");
        }
        assert!(plain.contains("\"api_request\"") && plain.contains("langfuse.trace.name") && plain.contains("gen_ai.usage.input_tokens"));
        let with = to_otlp(&recs, true);
        let s = with.to_string();
        assert!(s.contains("what went in") && s.contains("what came out") && !s.contains("sam@initech.example"), "text when allowed, identity never");
        let spans: Vec<&Value> = with["resourceSpans"].as_array().unwrap().iter().flat_map(|r| r["scopeSpans"][0]["spans"].as_array().unwrap().iter()).collect();
        let claude = spans.iter().find(|x| x["name"] == "claude.run").unwrap();
        let attr = |k: &str| claude["attributes"].as_array().unwrap().iter().find(|a| a["key"] == k).map(|a| a["value"].clone());
        assert_eq!(attr("langfuse.observation.type"), Some(json!({"stringValue": "generation"})));
        assert_eq!(attr("gen_ai.usage.output_tokens"), Some(json!({"intValue": "5"})));
        assert_eq!(claude["traceId"], json!(id));
        assert_ne!(claude["startTimeUnixNano"], "0");
    }

    #[test]
    fn the_exporter_sends_what_ended_since_it_was_turned_on_and_keeps_a_failure_to_try_again() {
        let (_env, _root) = testenv::folders("export-pass");
        let (url, got) = collector(vec![500, 200]);
        set(&json!({"on": true, "kind": "otlp", "endpoint": url, "headers": ["X-Team: acme"]})).unwrap();
        // Turned on: from now on, never the past.
        let before = a_trace();
        assert_eq!(pass(Utc::now()).unwrap(), 0);
        std::thread::sleep(Duration::from_millis(20));
        let id = a_trace();
        // Not yet settled (Claude Code's spans may still come): not sent.
        assert_eq!(pass(Utc::now()).unwrap(), 0);
        let later = Utc::now() + chrono::Duration::seconds(SETTLE_SECS + 5);
        assert!(pass(later).unwrap_err().to_string().contains("HTTP 500"), "the collector refused it");
        assert!(status()["error"].as_str().is_some_and(|e| e.contains("500")));
        assert_eq!(pass(later).unwrap(), 1, "tried again: sent");
        let st = status();
        assert_eq!((st["sent"].clone(), st["error"].clone()), (json!(1), Value::Null));
        let got = got.lock().unwrap();
        let (first, headers, body) = &got[1];
        assert_eq!(first, "POST /v1/traces HTTP/1.1");
        assert!(headers.iter().any(|h| h == "X-Team: acme") && headers.iter().any(|h| h.eq_ignore_ascii_case("content-type: application/json")));
        assert!(body.contains(&id) && !body.contains(&before), "only what ended since it was turned on");
        assert_eq!(pass(later).unwrap(), 0, "sent once");
    }

    #[test]
    fn langfuse_gets_its_path_and_keys() {
        let (_env, _root) = testenv::folders("export-langfuse");
        let (url, got) = collector(vec![200]);
        set(&json!({"on": true, "kind": "langfuse", "endpoint": url, "public_key": "pk-lf-1", "secret_key": "sk-lf-2"})).unwrap();
        a_trace();
        send_newest().unwrap();
        let got = got.lock().unwrap();
        use base64::Engine;
        assert_eq!(got[0].0, "POST /api/public/otel/v1/traces HTTP/1.1");
        assert!(got[0].1.iter().any(|h| *h == format!("Authorization: Basic {}", base64::engine::general_purpose::STANDARD.encode("pk-lf-1:sk-lf-2"))));
        assert!(got[0].1.iter().any(|h| h == "x-langfuse-ingestion-version: 4"));
    }
}

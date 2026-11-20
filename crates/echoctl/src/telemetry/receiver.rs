//! Claude Code's OpenTelemetry for the runs this process starts (OTLP over HTTP, JSON): on 127.0.0.1, a port the system
//! picks, for the process's life. Its spans and log records are filed by their trace (`super::send`); metrics are
//! taken and not kept (Echo asks for none).

use super::{known, send, valid_id};
use chrono::{TimeZone, Utc};
use serde_json::{json, Map, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::OnceLock;
use std::time::Duration;

/// A request's body at most (Claude Code sends batches of a few hundred KB).
const MAX_BODY: usize = 16 << 20;
/// A request or header line at most.
const MAX_LINE: u64 = 16 << 10;
/// Connections served at once; more wait in the system's queue.
const MAX_OPEN: usize = 8;

static OPEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

static PORT: OnceLock<Option<u16>> = OnceLock::new();

/// Where Claude Code sends to (`http://127.0.0.1:<port>`), started on first use; none when it can't listen.
pub fn endpoint() -> Option<String> {
    PORT.get_or_init(listen).map(|p| format!("http://127.0.0.1:{p}"))
}

fn listen() -> Option<u16> {
    let l = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = l.local_addr().ok()?.port();
    std::thread::Builder::new()
        .name("telemetry-receiver".into())
        .spawn(move || {
            for s in l.incoming().flatten() {
                while OPEN.load(std::sync::atomic::Ordering::SeqCst) >= MAX_OPEN {
                    std::thread::sleep(Duration::from_millis(20));
                }
                OPEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let spawned = std::thread::Builder::new().name("telemetry-request".into()).spawn(move || {
                    let _ = serve(s, port);
                    OPEN.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                });
                if spawned.is_err() {
                    OPEN.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                }
            }
        })
        .ok()?;
    Some(port)
}

/// A connection: its requests, while it sends them in time (a minute in all, ten seconds between reads).
fn serve(mut s: TcpStream, port: u16) -> std::io::Result<()> {
    s.set_read_timeout(Some(Duration::from_secs(10)))?;
    let until = std::time::Instant::now() + Duration::from_secs(60);
    let mut r = BufReader::new(s.try_clone()?);
    while std::time::Instant::now() < until {
        let Some(req) = read_request(&mut r)? else { return Ok(()) };
        // Only Claude Code's JSON to this very address: a web page can't send it without a preflight nobody answers.
        let ours = req.json && (req.host == format!("127.0.0.1:{port}") || req.host == format!("localhost:{port}"));
        let status = match serde_json::from_slice::<Value>(&req.body).ok().filter(|_| ours) {
            Some(v) => match req.path.as_str() {
                "/v1/traces" => {
                    take_traces(&v);
                    "200 OK"
                }
                "/v1/logs" => {
                    take_logs(&v);
                    "200 OK"
                }
                "/v1/metrics" => "200 OK",
                _ => "404 Not Found",
            },
            None => "400 Bad Request",
        };
        s.write_all(format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}").as_bytes())?;
    }
    Ok(())
}

struct Request {
    path: String,
    host: String,
    json: bool,
    body: Vec<u8>,
}

/// A line of at most `MAX_LINE` bytes.
fn line(r: &mut impl BufRead) -> std::io::Result<String> {
    let mut l = String::new();
    r.take(MAX_LINE).read_line(&mut l)?;
    if l.len() as u64 >= MAX_LINE && !l.ends_with('\n') {
        return Err(std::io::Error::other("line too long"));
    }
    Ok(l)
}

/// One request (keep-alive: the next on the same connection); none when the client is done.
fn read_request(r: &mut impl BufRead) -> std::io::Result<Option<Request>> {
    let first = line(r)?;
    if first.is_empty() {
        return Ok(None);
    }
    let path = first.split_whitespace().nth(1).unwrap_or("").split('?').next().unwrap_or("").to_string();
    let (mut len, mut chunked, mut host, mut json) = (0usize, false, String::new(), false);
    loop {
        let h = line(r)?;
        if h.trim().is_empty() {
            break;
        }
        let (k, v) = h.split_once(':').unwrap_or((&h, ""));
        match k.trim().to_ascii_lowercase().as_str() {
            "content-length" => len = v.trim().parse().unwrap_or(0),
            "transfer-encoding" => chunked = v.to_ascii_lowercase().contains("chunked"),
            "host" => host = v.trim().to_string(),
            "content-type" => json = v.trim().to_ascii_lowercase().starts_with("application/json"),
            _ => {}
        }
    }
    let too_big = || std::io::Error::other("body too big");
    let mut body = Vec::new();
    if chunked {
        loop {
            let n = usize::from_str_radix(line(r)?.trim().split(';').next().unwrap_or(""), 16).unwrap_or(0);
            if n == 0 {
                line(r)?;
                break;
            }
            if body.len() + n > MAX_BODY {
                return Err(too_big());
            }
            r.take(n as u64).read_to_end(&mut body)?;
            line(r)?;
        }
    } else {
        if len > MAX_BODY {
            return Err(too_big());
        }
        // Read as it comes, never allocated ahead of what arrived.
        r.take(len as u64).read_to_end(&mut body)?;
        if body.len() < len {
            return Err(std::io::Error::other("body cut short"));
        }
    }
    Ok(Some(Request { path, host, json, body }))
}

/// An OTLP/JSON attribute value as plain JSON.
fn value(v: &Value) -> Value {
    if let Some(s) = v.get("stringValue") {
        return s.clone();
    }
    if let Some(i) = v.get("intValue") {
        return i.as_str().and_then(|s| s.parse::<i64>().ok()).map(Value::from).unwrap_or_else(|| i.clone());
    }
    if let Some(x) = v.get("doubleValue").or(v.get("boolValue")) {
        return x.clone();
    }
    if let Some(a) = v.get("arrayValue") {
        return json!(a["values"].as_array().into_iter().flatten().map(value).collect::<Vec<_>>());
    }
    if let Some(kv) = v.get("kvlistValue") {
        return Value::Object(attrs(&kv["values"]));
    }
    Value::Null
}

fn attrs(list: &Value) -> Map<String, Value> {
    list.as_array().into_iter().flatten().filter_map(|a| Some((a["key"].as_str()?.to_string(), value(&a["value"])))).collect()
}

/// Nanoseconds since the epoch (a string or a number) as a time.
fn time(v: &Value) -> Option<String> {
    let n = v.as_str().and_then(|s| s.parse::<i64>().ok()).or(v.as_i64())?;
    Utc.timestamp_nanos(n).to_rfc3339_opts(chrono::SecondsFormat::Millis, true).into()
}

/// A record's trace: its own id, else the one Echo gave the run (`echo.trace` of its resource).
/// Only a trace of a run this process started (`known`).
fn trace_of(own: &Value, resource: &Map<String, Value>) -> Option<String> {
    let own = own.as_str().map(str::to_ascii_lowercase).filter(|t| valid_id(t, 32));
    own.or_else(|| resource.get("echo.trace").and_then(Value::as_str).map(str::to_owned)).filter(|t| known(t))
}

/// What a span keeps of its resource: Claude Code's name and version (its account's identity is in the run's events).
fn kept(resource: &Map<String, Value>) -> Map<String, Value> {
    resource.iter().filter(|(k, _)| k.starts_with("service.") || k.starts_with("echo.")).map(|(k, v)| (k.clone(), v.clone())).collect()
}

pub(super) fn take_traces(v: &Value) {
    for rs in v["resourceSpans"].as_array().into_iter().flatten() {
        let resource = attrs(&rs["resource"]["attributes"]);
        for sp in rs["scopeSpans"].as_array().into_iter().flatten().flat_map(|ss| ss["spans"].as_array().into_iter().flatten()) {
            let Some(trace) = trace_of(&sp["traceId"], &resource) else { continue };
            let parent = sp["parentSpanId"].as_str().filter(|p| !p.is_empty()).map(str::to_ascii_lowercase);
            send(&trace, json!({"t": "span", "source": "claude-code", "trace": trace, "span": sp["spanId"].as_str().map(str::to_ascii_lowercase),
                "parent": parent, "name": sp["name"], "start": time(&sp["startTimeUnixNano"]), "end": time(&sp["endTimeUnixNano"]),
                "attrs": attrs(&sp["attributes"]), "status": sp["status"], "resource": kept(&resource)}));
        }
    }
}

pub(super) fn take_logs(v: &Value) {
    for rl in v["resourceLogs"].as_array().into_iter().flatten() {
        let resource = attrs(&rl["resource"]["attributes"]);
        for lr in rl["scopeLogs"].as_array().into_iter().flatten().flat_map(|sl| sl["logRecords"].as_array().into_iter().flatten()) {
            let Some(trace) = trace_of(&lr["traceId"], &resource) else { continue };
            let a = attrs(&lr["attributes"]);
            let span = lr["spanId"].as_str().filter(|s| !s.is_empty()).map(str::to_ascii_lowercase).or_else(|| resource.get("echo.span").and_then(Value::as_str).map(str::to_owned));
            let name = a.get("event.name").cloned().unwrap_or_else(|| value(&lr["body"]));
            send(&trace, json!({"t": "event", "source": "claude-code", "trace": trace, "span": span, "name": name,
                "at": time(&lr["timeUnixNano"]), "attrs": a}));
        }
    }
}

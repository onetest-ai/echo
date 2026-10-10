//! The Echo window: calls (notes, transcript, chat, frames, actions), today's meetings and
//! settings, in the system webview (WKWebView on macOS, WebView2 on Windows) on the tray's own
//! tao event loop.
//!
//! Everything the page shows comes through the `echo://` custom protocol — no web server, no open
//! port: `/` and `/assets/*` are compiled in, `/api/*` is JSON built from the call files, and
//! `/file/<call>/frames/<image>` serves frame images after a path check (a call folder under the
//! data folder, `frames/` inside it, no `..`). Actions (mark done, delete, record, …) come back as
//! IPC messages and run through the same functions the CLI and the tray use.

use std::borrow::Cow;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};
use tao::dpi::LogicalSize;
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder};
use wry::http::{header::CONTENT_TYPE, Request, Response};
use wry::{WebView, WebViewBuilder};

use crate::{calls, paths, watch};

const INDEX: &str = include_str!("../ui/index.html");
const APP_JS: &str = include_str!("../ui/app.js");
const APP_CSS: &str = include_str!("../ui/app.css");
const OCTOPUS_SVG: &str = include_str!("../assets/octopus.svg");
/// The onetest logo (violet disc, the full octopus character), 192 px, for the window.
const LOGO_PNG: &[u8] = include_bytes!("../assets/onetest-logo.png");

/// The page with the octopus mark defined once as an SVG symbol (`<use href="#octopus">`).
fn index_html() -> String {
    let inner = OCTOPUS_SVG
        .split_once('>')
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.rsplit_once("</svg>").map(|(body, _)| body))
        .unwrap_or("");
    let symbol = format!(
        "<svg width=\"0\" height=\"0\" style=\"position:absolute\" aria-hidden=\"true\"><symbol id=\"octopus\" viewBox=\"0 0 100 100\" fill=\"currentColor\">{inner}</symbol></svg>"
    );
    INDEX.replace("<!--OCTOPUS-->", &symbol)
}

/// Where the page lives: WebView2 maps custom schemes to `http://<scheme>.localhost`.
pub fn base_url() -> &'static str {
    if cfg!(windows) {
        "http://echo.localhost/"
    } else {
        "echo://localhost/"
    }
}

/// An open Echo window. Dropping it closes the window.
pub struct EchoWindow {
    pub window: Window,
    pub webview: WebView,
}

impl EchoWindow {
    /// Ask the page to reload its data (after an action, or when the files changed).
    pub fn refresh(&self) {
        let _ = self.webview.evaluate_script("window.echo && window.echo.refresh()");
    }

    /// Show a call (`name` = its folder name).
    pub fn show_call(&self, name: &str) {
        let _ = self.webview.evaluate_script(&format!("window.echo && window.echo.openCall({})", json!(name)));
    }
}

/// Open the window. `ipc` receives each message the page posts (JSON text).
pub fn open<T: 'static>(
    target: &EventLoopWindowTarget<T>,
    start_call: Option<&str>,
    ipc: impl Fn(String) + 'static,
) -> wry::Result<EchoWindow> {
    let window = WindowBuilder::new()
        .with_title("Echo")
        .with_inner_size(LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(LogicalSize::new(760.0, 480.0))
        .build(target)
        .map_err(|e| wry::Error::Io(std::io::Error::other(e.to_string())))?;
    let url = match start_call {
        Some(c) => format!("{}#call/{}", base_url(), urlencode(c)),
        None => base_url().to_string(),
    };
    let webview = WebViewBuilder::new()
        // Answered off the main thread: a calendar request may wait on Microsoft 365.
        .with_asynchronous_custom_protocol("echo".into(), |_id, req, responder| {
            std::thread::spawn(move || responder.respond(respond(&req)));
        })
        .with_ipc_handler(move |req: Request<String>| ipc(req.into_body()))
        // The page never navigates away from echo:// (links to the web open in the browser).
        .with_navigation_handler(|url| {
            let ours = url.starts_with(base_url()) || url.starts_with("about:");
            if !ours && (url.starts_with("https://") || url.starts_with("http://")) {
                open_external(&url);
            }
            ours
        })
        .with_url(url)
        .with_devtools(cfg!(debug_assertions))
        .build(&window)?;
    Ok(EchoWindow { window, webview })
}

fn open_external(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).status();
    #[cfg(windows)]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).status();
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn urldecode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
            let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
            out.push(hex(b[i + 1]) << 4 | hex(b[i + 2]));
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── echo:// ─────────────────────────────────────────────────────────────────────────────────

fn reply(status: u16, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, mime)
        // The page only ever talks to echo:// itself.
        .header("Content-Security-Policy", "default-src 'self' echo: http://echo.localhost; img-src 'self' echo: http://echo.localhost data:; style-src 'self' 'unsafe-inline' echo: http://echo.localhost; script-src 'self' echo: http://echo.localhost")
        .header("Cache-Control", "no-store")
        .body(Cow::Owned(body))
        .expect("valid response")
}

fn json_reply(v: Value) -> Response<Cow<'static, [u8]>> {
    reply(200, "application/json", serde_json::to_vec(&v).unwrap_or_default())
}

fn respond(req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = urldecode(req.uri().path());
    match path.as_str() {
        "/" | "/index.html" => reply(200, "text/html; charset=utf-8", index_html().into_bytes()),
        "/assets/app.js" => reply(200, "text/javascript; charset=utf-8", APP_JS.as_bytes().to_vec()),
        "/assets/app.css" => reply(200, "text/css; charset=utf-8", APP_CSS.as_bytes().to_vec()),
        "/assets/logo.png" => reply(200, "image/png", LOGO_PNG.to_vec()),
        "/api/state" => json_reply(state()),
        "/api/calls" => json_reply(Value::Array(calls::all_calls().iter().rev().map(|d| call_summary(d)).collect())),
        "/api/agenda" => json_reply(agenda()),
        "/api/calendar" => json_reply(calendar(req.uri().query().unwrap_or(""))),
        p if p.starts_with("/api/call/") => match call_dir(&p["/api/call/".len()..]) {
            Some(dir) => json_reply(call_detail(&dir)),
            None => reply(404, "text/plain", b"no such call".to_vec()),
        },
        p if p.starts_with("/file/") => match frame_file(&p["/file/".len()..]) {
            Some(file) => match fs::read(&file) {
                Ok(bytes) => reply(200, mime_of(&file), bytes),
                Err(_) => reply(404, "text/plain", b"not found".to_vec()),
            },
            None => reply(403, "text/plain", b"forbidden".to_vec()),
        },
        _ => reply(404, "text/plain", b"not found".to_vec()),
    }
}

fn mime_of(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        _ => "application/octet-stream",
    }
}

/// A call folder by its name — only a direct child of the calls folder.
fn call_dir(name: &str) -> Option<PathBuf> {
    let name = name.trim_end_matches('/');
    let plain = !name.is_empty() && Path::new(name).components().count() == 1 && matches!(Path::new(name).components().next(), Some(Component::Normal(_)));
    if !plain {
        return None;
    }
    let dir = paths::calls_dir().join(name);
    dir.join("meta.json").exists().then_some(dir)
}

/// `/file/<call>/frames/<image>` → the file, if it really is a frame inside that call.
fn frame_file(rest: &str) -> Option<PathBuf> {
    let (call, rel) = rest.split_once('/')?;
    let dir = call_dir(call)?;
    let rel = Path::new(rel);
    let mut parts = rel.components();
    let ok = matches!(parts.next(), Some(Component::Normal(f)) if f == "frames")
        && matches!(parts.next(), Some(Component::Normal(_)))
        && parts.next().is_none();
    if !ok {
        return None;
    }
    let file = dir.join(rel).canonicalize().ok()?;
    file.starts_with(dir.canonicalize().ok()?).then_some(file)
}

fn read_json(p: &Path) -> Value {
    fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

fn minutes(meta: &Value) -> Option<i64> {
    let t = |k: &str| meta[k].as_str().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
    Some(((t("ended_at")? - t("started_at")?).num_seconds() + 30).max(60) / 60)
}

fn status(dir: &Path, recording: Option<&str>) -> &'static str {
    if recording.is_some_and(|r| Path::new(r).ends_with(dir.file_name().unwrap_or_default())) {
        "recording"
    } else if dir.join(crate::transcribe_marker()).exists() {
        "transcribing"
    } else if dir.join("notes.md").exists() {
        "notes"
    } else if dir.join("transcript.md").exists() {
        "transcript"
    } else if dir.join("transcribe.error").exists() {
        "failed"
    } else {
        "audio"
    }
}

fn people(dir: &Path) -> Vec<String> {
    let m = read_json(&dir.join("meeting.json"));
    let mut out: Vec<String> = std::iter::once(&m["organizer"])
        .chain(m["attendees"].as_array().into_iter().flatten())
        .filter_map(|p| p["name"].as_str().filter(|n| !n.is_empty()).map(str::to_owned))
        .collect();
    out.dedup();
    out
}

fn call_summary(dir: &Path) -> Value {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let meta = read_json(&dir.join("meta.json"));
    let recording = watch::running_watcher().and_then(|w| w.recording);
    let title = meta["title"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| {
        name.get(18..).filter(|t| !t.is_empty()).map(|t| t.replace('-', " ")).unwrap_or_else(|| "Call".into())
    });
    json!({
        "name": name,
        "title": title,
        "started_at": meta["started_at"],
        "minutes": minutes(&meta),
        "status": status(dir, recording.as_deref()),
        "people": people(dir),
        "frames": dir.join("frames.jsonl").exists(),
        "chat": dir.join("chat.json").exists(),
    })
}

fn call_detail(dir: &Path) -> Value {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut v = call_summary(dir);
    let segments = match read_json(&dir.join("segments.json")) {
        Value::Array(a) => Value::Array(a),
        _ => Value::Array(
            fs::read_to_string(dir.join("live.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect(),
        ),
    };
    let frames: Vec<Value> = fs::read_to_string(dir.join("frames.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|f| json!({ "t": f["t"], "url": format!("{}file/{}/{}", base_url(), urlencode(&name), f["file"].as_str().unwrap_or("")) }))
        .collect();
    v["notes"] = json!(fs::read_to_string(dir.join("notes.md")).ok());
    v["my_notes"] = json!(fs::read_to_string(dir.join(calls::USER_NOTES)).ok());
    v["segments"] = segments;
    v["chat"] = read_json(&dir.join("chat.json"));
    v["frames"] = json!(frames);
    v["actions"] = read_json(&dir.join("actions.json"));
    v["meeting"] = read_json(&dir.join("meeting.json"));
    v["path"] = json!(dir.display().to_string());
    v
}

fn agenda() -> Value {
    let now = chrono::Utc::now();
    let mut meetings: Vec<Value> = watch::agenda()
        .into_iter()
        .filter(|m| m.end.as_deref().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).is_none_or(|e| e.with_timezone(&chrono::Utc) > now))
        .map(|m| {
            let recap = m.subject.as_deref().map(calls::recap).unwrap_or_default();
            json!({ "id": m.id, "subject": m.subject, "start": m.start, "end": m.end, "join_url": m.join_url,
                    "attendees": m.attendees.len(), "recap": recap })
        })
        .collect();
    meetings.sort_by(|a, b| a["start"].as_str().cmp(&b["start"].as_str()));
    Value::Array(meetings)
}

// ── calendar: Microsoft 365 events with what Echo knows about each ─────────────────────────

/// Raw Graph events by id from the last calendar fetch (to schedule one by id), and a short
/// cache so the page's refreshes don't hit Microsoft 365 each time.
type CalendarCache = (String, std::time::Instant, Vec<Value>);
static CALENDAR_CACHE: std::sync::Mutex<Option<CalendarCache>> = std::sync::Mutex::new(None);
static RAW_EVENTS: std::sync::Mutex<Vec<Value>> = std::sync::Mutex::new(Vec::new());
const CALENDAR_TTL: std::time::Duration = std::time::Duration::from_secs(120);

fn query_param(q: &str, key: &str) -> Option<String> {
    q.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| urldecode(v)))
}

fn rfc(v: &Value) -> Option<chrono::DateTime<chrono::Utc>> {
    v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&chrono::Utc))
}

/// `/api/calendar?start=YYYY-MM-DD&days=N[&fresh=1]`: events in [start, start + days) local
/// time, each with `recorded` (the call made of it) and `scheduled`, plus recordings that match
/// no event. Without a Microsoft 365 sign-in: today's agenda only (`source: "agenda"`).
fn calendar(q: &str) -> Value {
    use chrono::{Local, NaiveDate, TimeZone};
    let start = query_param(q, "start").and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()).unwrap_or_else(|| Local::now().date_naive());
    let days = query_param(q, "days").and_then(|d| d.parse::<i64>().ok()).unwrap_or(1).clamp(1, 14);
    let from = Local.from_local_datetime(&start.and_hms_opt(0, 0, 0).unwrap()).earliest().map(|d| d.with_timezone(&chrono::Utc));
    let Some(from) = from else { return json!({ "error": "bad date" }) };
    let to = from + chrono::Duration::days(days);
    let signed_in = crate::graph::account().is_some();

    let (events, source, error): (Vec<Value>, &str, Option<String>) = if signed_in {
        let key = format!("{start}/{days}");
        let cached = CALENDAR_CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
            .filter(|(k, at, _)| *k == key && at.elapsed() < CALENDAR_TTL && query_param(q, "fresh").is_none())
            .map(|(_, _, ev)| ev.clone());
        match cached.map(Ok).unwrap_or_else(|| crate::graph::calendar_view(from, to)) {
            Ok(raw) => {
                *CALENDAR_CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, std::time::Instant::now(), raw.clone()));
                *RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()) = raw.clone();
                (raw.iter().filter_map(crate::calendar::view_event).collect(), "microsoft365", None)
            }
            Err(e) => (vec![], "microsoft365", Some(format!("{e:#}"))),
        }
    } else {
        let ev: Vec<Value> = watch::agenda().into_iter().map(|m| json!({
            "id": m.id, "subject": m.subject, "start": m.start, "end": m.end, "all_day": false, "online": true,
            "join_url": m.join_url, "declined": false, "cancelled": false, "no_notetakers": false,
            "organizer": m.organizer.map(|o| o.name), "people": m.attendees.len(),
        })).collect();
        (ev, "agenda", None)
    };

    // Recorded calls in the range.
    let calls: Vec<(String, Value, Value)> = calls::all_calls()
        .into_iter()
        .filter_map(|d| {
            let meta = read_json(&d.join("meta.json"));
            let s = rfc(&meta["started_at"])?;
            let name = d.file_name()?.to_string_lossy().into_owned();
            (s >= from - chrono::Duration::hours(1) && s < to).then(|| (name, meta, read_json(&d.join("meeting.json"))))
        })
        .collect();
    let recording = watch::running_watcher().and_then(|w| w.recording);
    let mut used = vec![false; calls.len()];
    let scheduled: Vec<String> = crate::schedule::load().into_iter().filter_map(|m| m.id).collect();
    let mut out = vec![];
    for mut e in events {
        let (es, ee) = (rfc(&e["start"]), rfc(&e["end"]));
        let subject = e["subject"].as_str().unwrap_or("").to_lowercase();
        let hit = calls.iter().enumerate().find(|(i, (_, meta, meeting))| {
            if used[*i] {
                return false;
            }
            if meeting["id"].is_string() && meeting["id"] == e["id"] {
                return true;
            }
            let Some(cs) = rfc(&meta["started_at"]) else { return false };
            let near = es.zip(ee).is_some_and(|(s, en)| cs >= s - chrono::Duration::minutes(15) && cs <= en);
            let same_title = meeting["subject"].as_str().or(meta["title"].as_str()).is_some_and(|t| t.to_lowercase() == subject);
            near && same_title
        });
        if let Some((i, (name, meta, _))) = hit {
            used[i] = true;
            let dir = paths::calls_dir().join(name);
            e["recorded"] = json!({ "name": name, "status": status(&dir, recording.as_deref()), "minutes": minutes(meta) });
        }
        e["scheduled"] = json!(e["id"].as_str().is_some_and(|id| scheduled.iter().any(|s| s == id)));
        if e["recorded"].is_null() {
            let recap = e["subject"].as_str().map(calls::recap).unwrap_or_default();
            if !recap.is_empty() {
                e["recap"] = json!(recap);
            }
        }
        out.push(e);
    }
    let extra: Vec<Value> = calls
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, (name, meta, _))| {
            let dir = paths::calls_dir().join(name);
            json!({ "name": name, "title": meta["title"].as_str().filter(|t| !t.is_empty()).unwrap_or("Call"),
                    "start": meta["started_at"], "end": meta["ended_at"], "status": status(&dir, recording.as_deref()) })
        })
        .collect();
    json!({ "source": source, "signed_in": signed_in, "error": error, "start": start.to_string(), "days": days,
            "events": out, "calls": extra })
}

fn state() -> Value {
    let w = watch::running_watcher();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "recording": w.as_ref().and_then(|w| w.recording.clone()),
        "armed": w.as_ref().is_some_and(|w| w.armed),
        "in_call": w.as_ref().and_then(|w| w.in_call),
        "home": paths::home(),
        "calendar": crate::graph::account(),
        "platform": std::env::consts::OS,
    })
}

// ── actions from the page ───────────────────────────────────────────────────────────────────

/// Run an action the page posted; true if the page should refresh.
pub fn handle(msg: &str) -> bool {
    let v: Value = serde_json::from_str(msg).unwrap_or_default();
    let call = v["call"].as_str().and_then(call_dir);
    match v["cmd"].as_str().unwrap_or("") {
        "toggle_action" => {
            let (Some(dir), Some(i)) = (call, v["index"].as_u64()) else { return false };
            let path = dir.join("actions.json");
            let Value::Array(mut items) = read_json(&path) else { return false };
            if let Some(it) = items.get_mut(i as usize) {
                it["status"] = json!(if it["status"] == "done" { "todo" } else { "done" });
                let tmp = path.with_extension("json.tmp");
                if fs::write(&tmp, serde_json::to_vec_pretty(&items).unwrap_or_default()).is_ok() {
                    let _ = fs::rename(tmp, path);
                }
            }
            true
        }
        "delete" => call.is_some_and(|d| calls::delete(&d.display().to_string()).is_ok()),
        "reveal" => {
            if let Some(d) = call {
                crate::app::reveal(&d);
            }
            false
        }
        "record" => watch::arm(None, None, v["languages"].as_str().map(|l| vec![l.to_string()])).is_ok(),
        "stop" => watch::stop().is_ok(),
        "schedule" | "unschedule" => {
            let Some(id) = v["id"].as_str() else { return false };
            if v["cmd"] == "unschedule" {
                return crate::schedule::remove_id(id).is_ok();
            }
            let raw = RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e["id"].as_str() == Some(id)).cloned();
            let meeting = raw.as_ref().and_then(crate::calendar::to_meeting)
                .or_else(|| watch::agenda().into_iter().find(|m| m.id.as_deref() == Some(id)));
            meeting.is_some_and(|m| crate::schedule::add(m).is_ok())
        }
        "copy" => {
            if let Some(text) = v["text"].as_str() {
                copy_to_clipboard(text);
            }
            false
        }
        "arm_meeting" | "join_meeting" => {
            let id = v["id"].as_str().unwrap_or("");
            let Some(m) = watch::agenda().into_iter().find(|m| m.id.as_deref() == Some(id)) else { return false };
            if v["cmd"] == "join_meeting" { watch::join(m).is_ok() } else { watch::arm(m.subject.clone(), Some(m), None).is_ok() }
        }
        "open_url" => {
            if let Some(u) = v["url"].as_str().filter(|u| u.starts_with("https://")) {
                open_external(u);
            }
            false
        }
        _ => false,
    }
}

fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn();
    #[cfg(windows)]
    let cmd = std::process::Command::new("clip").stdin(std::process::Stdio::piped()).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let cmd: std::io::Result<std::process::Child> = Err(std::io::Error::other("no clipboard"));
    if let Ok(mut child) = cmd {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// (dev) Write what echo:// serves — the page, assets and every API response — into `dir`, so
/// the UI can be reviewed in an ordinary browser (`python3 -m http.server`).
pub fn export(dir: &Path) -> anyhow::Result<()> {
    let get = |path: &str| -> Vec<u8> {
        let req = Request::builder().uri(format!("echo://localhost{path}")).body(Vec::new()).expect("request");
        respond(&req).into_body().into_owned()
    };
    let write = |rel: &str, body: Vec<u8>| -> anyhow::Result<()> {
        let p = dir.join(rel.trim_start_matches('/'));
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(p, body)?;
        Ok(())
    };
    write("index.html", get("/"))?;
    for p in ["/assets/app.js", "/assets/app.css", "/assets/logo.png", "/api/state", "/api/calls", "/api/agenda"] {
        write(p, get(p))?;
    }
    let week_start = (chrono::Local::now() - chrono::Duration::days(1)).format("%Y-%m-%d");
    write("/api/calendar", get(&format!("/api/calendar?start={week_start}&days=7")))?;
    for d in calls::all_calls() {
        let name = d.file_name().unwrap().to_string_lossy().into_owned();
        write(&format!("/api/call/{name}"), get(&format!("/api/call/{}", urlencode(&name))))?;
    }
    println!("{}", dir.display());
    Ok(())
}

// ── self-test (`echoctl window --selftest`) ─────────────────────────────────────────────────

/// Open the window on its own event loop, let the page fetch through echo:// and report back
/// over IPC, print the report, exit. Proves the protocol, the API and IPC on this machine.
pub fn selftest() -> anyhow::Result<()> {
    use tao::event::{Event, StartCause};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let mut _win: Option<EchoWindow> = None; // kept alive for the loop's lifetime
    let t0 = std::time::Instant::now();
    event_loop.run(move |event, target, flow| {
        *flow = ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(200));
        match event {
            Event::NewEvents(StartCause::Init) => {
                let p = proxy.clone();
                match open(target, None, move |msg| drop(p.send_event(msg))) {
                    Ok(w) => {
                        let _ = w.webview.evaluate_script("setTimeout(() => window.echo && window.echo.selftest(), 300)");
                        _win = Some(w);
                    }
                    Err(e) => {
                        println!("{{\"ok\":false,\"error\":{}}}", json!(e.to_string()));
                        *flow = ControlFlow::Exit;
                    }
                }
            }
            Event::UserEvent(msg) if msg.contains("\"selftest\"") => {
                println!("{msg}");
                *flow = ControlFlow::Exit;
            }
            _ if t0.elapsed() > std::time::Duration::from_secs(15) => {
                println!("{{\"ok\":false,\"error\":\"the page did not report within 15 s\"}}");
                *flow = ControlFlow::Exit;
            }
            _ => {}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_frames_inside_a_call_are_served() {
        // Rejected before touching the disk: traversal, absolute paths, non-frame files.
        assert!(frame_file("../etc/passwd").is_none());
        assert!(frame_file("x/../../etc/passwd").is_none());
        assert!(frame_file("x/frames/../../meta.json").is_none());
        assert!(frame_file("x/notes.md").is_none());
        assert!(call_dir("../x").is_none() && call_dir("/abs").is_none() && call_dir("a/b").is_none() && call_dir("").is_none());
    }

    #[test]
    fn urls_round_trip() {
        assert_eq!(urldecode(&urlencode("2026-10-09_183453-lumen kickoff/é")), "2026-10-09_183453-lumen kickoff/é");
    }
}

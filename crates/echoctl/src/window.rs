//! The Echo window: calls (notes, transcript, chat, frames, actions), today's meetings and
//! settings, in the system webview (WKWebView on macOS, WebView2 on Windows) on the tray's own
//! tao event loop.
//!
//! Everything the page shows comes through the `echo://` custom protocol — no web server, no open
//! port: `/` and `/assets/*` are compiled in, `/api/*` is JSON built from the call files,
//! `/file/<call>/frames/<image>` serves frame images after a path check (a call folder under the
//! data folder, `frames/` inside it, no `..`), and `/minutes/<call>` the call's minutes page,
//! rendered fresh (the Minutes tab shows it in a sandboxed frame, under the page's own CSP).
//! Actions (mark done, delete, record, …) come back as IPC messages — only from the window's own
//! page, never from a frame inside it — and run through the same functions the CLI and the tray use.

#[cfg(windows)]
use crate::app::NoWindow;
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
/// xterm.js (MIT, `ui/vendor/README.md`): the terminals in the window.
const XTERM_JS: &str = include_str!("../ui/vendor/xterm.js");
const XTERM_FIT_JS: &str = include_str!("../ui/vendor/xterm-addon-fit.js");
const XTERM_CSS: &str = include_str!("../ui/vendor/xterm.css");

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

/// One size of `packaging/windows/echo.ico` (its entries are PNGs), as a window icon.
#[cfg(windows)]
fn exe_icon(size: u32) -> Option<tao::window::Icon> {
    static ICO: &[u8] = include_bytes!("../../../packaging/windows/echo.ico");
    let count = u16::from_le_bytes([ICO[4], ICO[5]]) as usize;
    let entry = (0..count).map(|i| &ICO[6 + 16 * i..22 + 16 * i]).find(|e| e[0] as u32 == size % 256)?;
    let len = u32::from_le_bytes(entry[8..12].try_into().ok()?) as usize;
    let at = u32::from_le_bytes(entry[12..16].try_into().ok()?) as usize;
    let img = image::load_from_memory(ICO.get(at..at + len)?).ok()?.to_rgba8();
    tao::window::Icon::from_rgba(img.into_raw(), size, size).ok()
}

/// Open the window. `ipc` receives each message the page posts (JSON text).
pub fn open<T: 'static>(
    target: &EventLoopWindowTarget<T>,
    start_call: Option<&str>,
    ipc: impl Fn(String) + 'static,
) -> wry::Result<EchoWindow> {
    crate::signin::refresh_all(); // who each Claude profile is signed in to, now (in the background)
    let builder = WindowBuilder::new()
        .with_title("Echo")
        .with_inner_size(LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(LogicalSize::new(760.0, 480.0));
    // macOS: the page draws the title bar (the Record pill sits in it, `app.js` → `window_drag`).
    #[cfg(target_os = "macos")]
    let builder = {
        use tao::platform::macos::WindowBuilderExtMacOS;
        builder
            .with_titlebar_transparent(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
            .with_traffic_light_inset(tao::dpi::LogicalPosition::new(16.0, 20.0))
    };
    // Windows: Echo's icon (the one in echoctl.exe) on the title bar and the taskbar button
    // (macOS takes the bundle's).
    #[cfg(windows)]
    let builder = {
        use tao::platform::windows::WindowBuilderExtWindows;
        builder.with_window_icon(exe_icon(32)).with_taskbar_icon(exe_icon(256))
    };
    let window = builder
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
        // Only the window's own page: WebKit lets frames post to the handler too (the minutes page).
        .with_ipc_handler(move |req: Request<String>| {
            if from_page(&req.uri().to_string(), base_url()) {
                ipc(req.into_body())
            }
        })
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

/// Is a message from the window's own page — `base` with the path `/` or `/index.html` — rather
/// than from a frame inside it? wry gives the sending frame's URL on macOS
/// (`WKScriptMessage.frameInfo`) and the sending document's on Windows (`WebMessageReceived`).
fn from_page(uri: &str, base: &str) -> bool {
    let Some(rest) = uri.strip_prefix(base.trim_end_matches('/')) else { return false };
    let path = rest.split(['?', '#']).next().unwrap_or("");
    path == "/" || path == "/index.html"
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

/// A page shown in a frame, under its own CSP (the window's would block its hashed inline scripts).
fn page_reply(html: String, csp: &str) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(200)
        .header(CONTENT_TYPE, "text/html; charset=utf-8")
        .header("Content-Security-Policy", csp)
        .header("Cache-Control", "no-store")
        .body(Cow::Owned(html.into_bytes()))
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
        "/assets/vendor/xterm.js" => reply(200, "text/javascript; charset=utf-8", XTERM_JS.as_bytes().to_vec()),
        "/assets/vendor/xterm-addon-fit.js" => reply(200, "text/javascript; charset=utf-8", XTERM_FIT_JS.as_bytes().to_vec()),
        "/assets/vendor/xterm.css" => reply(200, "text/css; charset=utf-8", XTERM_CSS.as_bytes().to_vec()),
        "/api/state" => json_reply(state()),
        "/api/calls" => json_reply(Value::Array(calls::all_calls().iter().rev().map(|d| call_summary(d)).collect())),
        "/api/agenda" => json_reply(agenda()),
        "/api/layout" => json_reply(load_layout()),
        "/api/settings" => json_reply(settings_snapshot()),
        "/api/sessions" => json_reply(json!(crate::terminal::saved_sessions())),
        "/api/ptys" => json_reply(Value::Array(crate::terminal::list().into_iter().map(|(id, key, exited)| json!({ "id": id, "key": key, "exited": exited })).collect())),
        // A terminal's output after byte `from`: answered at once when there is some, else when
        // some comes (or after 25 s) — the page asks again right away (a long poll).
        p if p.starts_with("/api/pty/") => {
            let id = p["/api/pty/".len()..].parse::<u64>().ok();
            let from = query_param(req.uri().query().unwrap_or(""), "from").and_then(|f| f.parse::<u64>().ok()).unwrap_or(0);
            match id.and_then(|id| crate::terminal::read(id, from, std::time::Duration::from_secs(25))) {
                Some(c) => {
                    use base64::Engine as _;
                    json_reply(json!({ "from": c.from, "next": c.next, "reset": c.reset, "exited": c.exited,
                                       "data": base64::engine::general_purpose::STANDARD.encode(&c.data) }))
                }
                None => reply(404, "text/plain", b"no such terminal".to_vec()),
            }
        }
        "/api/calendar" => json_reply(calendar(req.uri().query().unwrap_or(""))),
        p if p.starts_with("/api/call/") => {
            let rest = &p["/api/call/".len()..];
            let (name, rev) = rest.strip_suffix("/rev").map_or((rest, false), |n| (n, true));
            match call_dir(name) {
                Some(dir) if rev => json_reply(json!({ "rev": call_rev(&dir) })),
                Some(dir) => json_reply(call_detail(&dir)),
                None => reply(404, "text/plain", b"no such call".to_vec()),
            }
        }
        p if p.starts_with("/minutes/") => match call_dir(&p["/minutes/".len()..]).map(|d| crate::minutes::render_for_window(&d)) {
            Some(Ok((html, csp))) => page_reply(html, &csp),
            Some(Err(e)) => reply(500, "text/plain; charset=utf-8", format!("The minutes can't be shown: {e:#}").into_bytes()),
            None => reply(404, "text/plain", b"no such call".to_vec()),
        },
        "/selftest/frame" if SELFTEST.load(std::sync::atomic::Ordering::SeqCst) => {
            let (html, csp) = selftest_frame();
            page_reply(html, &csp)
        }
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
    // The revision first: anything that changes while the rest is read gives a newer one at the
    // next poll, so the page fetches again (taken last, it could stand for files read before).
    let rev = call_rev(dir);
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
    // The user's notes; one that can't be read is said, never shown as empty (the editor would start
    // from nothing).
    match fs::read_to_string(dir.join(calls::USER_NOTES)) {
        Ok(t) => v["my_notes"] = json!(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => v["my_notes"] = Value::Null,
        Err(e) => v["my_notes_error"] = json!(e.to_string()),
    }
    v["segments"] = segments;
    v["chat"] = read_json(&dir.join("chat.json"));
    v["frames"] = json!(frames);
    // With ids (a list from before ids gets them now, once: `calls::actions`).
    match calls::actions(dir) {
        Ok(a) => v["actions"] = json!(a),
        Err(e) => v["actions_error"] = json!(format!("{e:#}")),
    }
    v["meeting"] = read_json(&dir.join("meeting.json"));
    v["path"] = json!(dir.display().to_string());
    let run = crate::notes::status(dir);
    // The user's notes changed after the minutes were written — since the run that wrote them
    // started, or, for minutes without one (a session's, an older run's), since `notes.md` was
    // written: the Minutes tab offers an update.
    let mtime = |f: &str| fs::metadata(dir.join(f)).and_then(|m| m.modified()).ok().map(chrono::DateTime::<chrono::Local>::from);
    let written = match &run {
        Some(r) if r.state == "done" => r.started_at,
        _ => mtime("notes.md"),
    };
    v["notes_changed"] = json!(dir.join("notes.md").exists() && mtime(calls::USER_NOTES).zip(written).is_some_and(|(n, w)| n > w));
    v["notes_run"] = json!(run);
    v["auto"] = crate::notes::auto_state(dir);
    v["minutes_rev"] = json!(crate::minutes::revision(dir));
    v["rev"] = json!(rev);
    v["has_log"] = json!(dir.join(crate::notes::LOG).exists());
    let profile = crate::ai::call_profile_id(dir);
    v["live"] = json!({
        "profile_error": profile.as_ref().err().map(|e| format!("{e:#}")),
        "profile": profile.ok(),
        "notes": crate::livenotes::read_notes(dir),
        "answers": crate::livenotes::read_answers(dir),
        "status": crate::livenotes::status(dir),
        "prep": calls::prep(dir),
    });
    v
}

/// What everything `call_detail` returns depends on, as one short hash: every file of the call's
/// folder (name, size, time) and the parts kept in memory or computed — the live notes' state, the
/// minutes run's (a dead owner changes it without a file changing), the automatic minutes'. The
/// page fetches the call again only when it changes.
fn call_rev(dir: &Path) -> String {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    let mut files: Vec<(String, u64, u128)> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
                    Some((e.file_name().to_string_lossy().into_owned(), m.len(), t))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for (name, len, t) in files {
        h.update(format!("{name}\0{len}\0{t}\n"));
    }
    h.update(crate::livenotes::status(dir).to_string());
    h.update(json!(crate::notes::status(dir)).to_string());
    h.update(crate::notes::auto_state(dir).to_string());
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
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
    use chrono::{Local, NaiveDate};
    let start = query_param(q, "start").and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()).unwrap_or_else(|| Local::now().date_naive());
    let days = query_param(q, "days").and_then(|d| d.parse::<i64>().ok()).unwrap_or(1).clamp(1, 14);
    let to = start.checked_add_days(chrono::Days::new(days as u64)).and_then(crate::calendar::local_midnight);
    let (Some(from), Some(to)) = (crate::calendar::local_midnight(start), to) else { return json!({ "error": "bad date" }) };
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
    let scheduled: Vec<(String, Vec<String>)> = crate::schedule::load().into_iter().filter_map(|m| Some((m.id?, m.languages))).collect();
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
        let sched = e["id"].as_str().and_then(|id| scheduled.iter().find(|s| s.0 == id));
        e["scheduled"] = json!(sched.is_some());
        if let Some((_, l)) = sched.filter(|s| !s.1.is_empty()) {
            e["scheduled_languages"] = json!(l);
        }
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
        "notes": crate::notes::readiness(),
        "languages": crate::config::Config::load().map(|c| c.languages).unwrap_or_default(),
        "ai": crate::ai::summary(),
    })
}

// ── actions from the page ───────────────────────────────────────────────────────────────────

/// The language the page picked for a recording (`languages: "en"`); none = the configured ones.
fn languages(v: &Value) -> Option<Vec<String>> {
    v["languages"].as_str().filter(|l| !l.is_empty()).map(|l| vec![l.to_string()])
}

/// The AI profile the page picked for a recording or a run; none = the default / the call's own.
fn profile(v: &Value) -> Option<String> {
    v["profile"].as_str().filter(|p| !p.is_empty()).map(str::to_owned)
}

/// What running a page's action did: refresh the page (and the menu), and a script to answer
/// the page with.
#[derive(Default)]
pub struct Handled {
    pub refresh: bool,
    pub reply: Option<String>,
}

impl From<bool> for Handled {
    fn from(refresh: bool) -> Self {
        Handled { refresh, reply: None }
    }
}

/// The close handshake (`tray.rs`): the page saved what it holds (`close_ready`), the user chose
/// to close without saving (`close_now`), or to try closing again (`close_request`).
pub enum Closing {
    /// The page saved its notes (`ok`) for handshake `n`, or couldn't.
    Ready { ok: bool, n: u64 },
    Now,
    Request,
}

pub fn closing(msg: &str) -> Option<Closing> {
    let v: Value = serde_json::from_str(msg).ok()?;
    match v["cmd"].as_str()? {
        "close_ready" => Some(Closing::Ready { ok: v["ok"] == true, n: v["n"].as_u64().unwrap_or(0) }),
        "close_now" => Some(Closing::Now),
        "close_request" => Some(Closing::Request),
        _ => None,
    }
}

/// What the page asks of the window itself (macOS: it draws the title bar).
pub enum WindowCmd {
    Drag,
    Zoom,
}

pub fn window_cmd(msg: &str) -> Option<WindowCmd> {
    let v: Value = serde_json::from_str(msg).ok()?;
    match v["cmd"].as_str()? {
        "window_drag" => Some(WindowCmd::Drag),
        "window_zoom" => Some(WindowCmd::Zoom),
        _ => None,
    }
}

/// The user's own shell, in the data folder (a login shell: their PATH and setup).
fn shell_command() -> portable_pty::CommandBuilder {
    #[cfg(windows)]
    let mut c = portable_pty::CommandBuilder::new(std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into()));
    #[cfg(not(windows))]
    let mut c = {
        let mut c = portable_pty::CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into()));
        c.arg("-l");
        c
    };
    c.cwd(paths::home());
    c
}

// ── Settings (the window's Settings activity) ───────────────────────────────────────────────

/// What Settings shows: each setting's value and what is installed, signed in or granted. Flows
/// with a dialog run as the tray's own actions (`settings_action`); plain values save here
/// (`settings_set`).
fn settings_snapshot() -> Value {
    let cfg = crate::config::Config::load();
    let c = cfg.as_ref().ok();
    let on = |v: Option<Option<bool>>| v.flatten().unwrap_or(true);
    let health: Vec<Value> = crate::permissions::report()
        .map(|r| r.checks.iter().map(|k| json!({ "id": k.id, "name": k.name, "status": format!("{:?}", k.status).to_lowercase(), "required": k.required, "why": k.why, "how": k.how, "url": k.settings_url })).collect())
        .unwrap_or_default();
    #[allow(unused_mut)]
    let mut v = json!({
        "config_error": cfg.as_ref().err().map(|e| format!("{e:#}")),
        "home": paths::home(),
        "home_source": paths::home_source(),
        "autostart": crate::app::autostart_enabled(),
        "cli": crate::app::cli_installed(),
        "version": env!("CARGO_PKG_VERSION"),
        "update": crate::update::AVAILABLE.lock().ok().and_then(|a| a.as_ref().map(|r| r.version.clone())),
        "scheduled": c.map(|c| c.scheduled.clone().unwrap_or_else(|| "ask".into())),
        "unscheduled": c.map(|c| c.unscheduled.clone().unwrap_or_else(|| "ask".into())),
        "languages": c.map(|c| c.languages.clone()).unwrap_or_default(),
        "keep_audio": c.is_some_and(|c| c.keep_audio),
        "minutes_page": c.is_some_and(|c| c.minutes_page),
        "auto_minutes": c.is_some_and(|c| c.auto_minutes),
        "capture_frames": c.is_some_and(|c| c.capture_frames),
        "follow_app_mute": on(c.map(|c| c.follow_app_mute)),
        "capture_ui": on(c.map(|c| c.capture_ui)),
        "capture_chat": on(c.map(|c| c.capture_chat)),
        "echo_cancel": on(c.map(|c| c.echo_cancel)),
        "notes_in": c.and_then(|c| c.notes_in.clone()),
        "vocabulary": c.map(|c| c.vocabulary.clone()).unwrap_or_default(),
        // Terms the minutes learned, with what they are used again for.
        "learned": c.and_then(|c| c.learned.as_ref()).map(|l| l.iter().map(|x| json!({ "term": x.term, "subjects": x.subjects, "people": x.people, "calls": x.calls.len() })).collect::<Vec<_>>()).unwrap_or_default(),
        "me": c.and_then(|c| c.me.clone()),
        // The profile the minutes read (me.md), as text and rendered.
        "me_profile": crate::calls::ensure_me_profile().ok().and_then(|p| fs::read_to_string(&p).ok()).map(|t| json!({ "text": t, "html": crate::minutes::markdown_html(&t) })),
        "calendar": crate::graph::account(),
        "graph_custom": c.is_some_and(|c| c.graph_client_id.is_some()),
        "health": health,
        "models": crate::models::list_value().ok(),
        "downloading": crate::app::downloading().map(|(n, pct)| json!({ "name": n, "pct": pct })),
    });
    #[cfg(target_os = "macos")]
    {
        v["frames_permitted"] = json!(crate::frames::permitted());
    }
    #[cfg(feature = "transcribe")]
    {
        let serve = crate::remote::serve_settings();
        v["engine"] = json!(crate::gpu::engine_status());
        v["server"] = json!({ "client": crate::remote::client_label(), "serving": serve.on,
                              "url": serve.on.then(|| crate::remote::serve_url(&serve)), "key": serve.on.then(|| serve.key.clone()) });
    }
    v
}

/// Save one plain setting with the value the page shows now (never a toggle of a stale value).
fn settings_set(key: &str, value: &Value) -> anyhow::Result<()> {
    use crate::config::Config;
    match key {
        // AI settings live in ai.json, changed under its lock.
        "live_notes" => {
            let on = value.as_bool().unwrap_or(false);
            return crate::ai::Settings::update(|s| {
                s.live_notes = on;
                Ok(())
            });
        }
        "default_profile" => {
            let id = value.as_str().unwrap_or(crate::ai::SYSTEM).to_string();
            return crate::ai::Settings::update(|s| {
                s.default = (id != crate::ai::SYSTEM).then_some(id);
                Ok(())
            });
        }
        "autostart" => return crate::app::set_autostart(value.as_bool().unwrap_or(false)),
        _ => {}
    }
    let mut cfg = Config::load()?;
    let b = || value.as_bool().ok_or_else(|| anyhow::anyhow!("{key} takes true or false"));
    match key {
        "scheduled" | "unscheduled" => {
            let v = value.as_str().filter(|v| matches!(*v, "ask" | "manual")).ok_or_else(|| anyhow::anyhow!("{key} is ask or manual"))?;
            if key == "scheduled" { cfg.scheduled = Some(v.into()) } else { cfg.unscheduled = Some(v.into()) }
        }
        "keep_audio" => cfg.keep_audio = b()?,
        "minutes_page" => cfg.minutes_page = b()?,
        "auto_minutes" => cfg.set_auto_minutes(b()?),
        "capture_frames" => {
            cfg.capture_frames = b()?;
            #[cfg(target_os = "macos")]
            if cfg.capture_frames && !crate::frames::permitted() {
                std::thread::spawn(|| {
                    if !crate::frames::request() {
                        crate::app::notify("Screen Recording needed", "Enable Echo in Privacy & Security → Screen & System Audio Recording, then restart Echo.");
                    }
                });
            }
        }
        "follow_app_mute" => cfg.follow_app_mute = Some(b()?),
        "capture_ui" => cfg.capture_ui = Some(b()?),
        "capture_chat" => cfg.capture_chat = Some(b()?),
        "echo_cancel" => cfg.echo_cancel = Some(b()?),
        "languages" => {
            let langs: Vec<String> = value.as_array().ok_or_else(|| anyhow::anyhow!("languages is a list"))?.iter()
                .filter_map(|l| l.as_str()).map(|l| l.trim().to_lowercase()).filter(|l| l.len() == 2 && l.bytes().all(|b| b.is_ascii_lowercase())).collect();
            cfg.languages = langs;
        }
        "vocabulary" => {
            cfg.vocabulary = value.as_array().ok_or_else(|| anyhow::anyhow!("vocabulary is a list"))?.iter()
                .filter_map(|t| t.as_str()).map(|t| t.trim().chars().take(50).collect::<String>()).filter(|t| !t.is_empty()).take(200).collect();
        }
        // A learned term: forgotten, or used in every call from now on.
        "learned_remove" | "learned_always" => {
            let term = value.as_str().ok_or_else(|| anyhow::anyhow!("{key} takes a term"))?.trim().to_string();
            if let Some(l) = cfg.learned.as_mut() {
                l.retain(|x| !x.term.eq_ignore_ascii_case(&term));
            }
            if key == "learned_always" && !cfg.vocabulary.iter().any(|v| v.eq_ignore_ascii_case(&term)) {
                cfg.vocabulary.push(term);
            }
        }
        // The profile the minutes read (me.md), written as the user edited it in Settings.
        "me_profile" => {
            let text = value.as_str().ok_or_else(|| anyhow::anyhow!("me_profile takes text"))?;
            anyhow::ensure!(text.len() <= 64 * 1024, "the profile is too long (64 KB at most)");
            let p = crate::calls::ensure_me_profile()?;
            paths::write_atomic(&p, text.as_bytes())?;
            return Ok(());
        }
        "me" => {
            let name = value["name"].as_str().unwrap_or("").trim().to_string();
            let email = value["email"].as_str().unwrap_or("").trim().to_string();
            cfg.me = (!name.is_empty() || !email.is_empty()).then(|| crate::meeting::Person { name, email: (!email.is_empty()).then_some(email) });
        }
        other => anyhow::bail!("no setting {other:?}"),
    }
    cfg.save()
}

/// A flow the tray runs as it does from its menu (a dialog, a sign-in, a download): the page names
/// it, `tray.rs` maps the name to its own action.
pub fn settings_action(msg: &str) -> Option<(String, String)> {
    let v: Value = serde_json::from_str(msg).ok()?;
    (v["cmd"].as_str()? == "settings_action").then(|| (v["action"].as_str().unwrap_or("").to_string(), v["arg"].as_str().unwrap_or("").to_string()))
}

// ── the window's layout (`<config dir>/window-layout.json`, per computer) ────────────────────

fn layout_file() -> PathBuf {
    paths::config_dir().join("window-layout.json")
}

/// The saved layout, or `{}` (the page's defaults) when there is none or it can't be read.
fn load_layout() -> Value {
    fs::read(layout_file())
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// Keep the page's layout: a JSON object of a sensible size (the page checks its fields when it
/// reads it back).
fn save_layout(layout: &Value) -> anyhow::Result<()> {
    anyhow::ensure!(layout.is_object(), "not an object");
    let bytes = serde_json::to_vec_pretty(layout)?;
    anyhow::ensure!(bytes.len() <= 16 * 1024, "too large ({} bytes)", bytes.len());
    fs::create_dir_all(paths::config_dir())?;
    paths::write_atomic(&layout_file(), &bytes)?;
    Ok(())
}

/// A call named by the page: by its folder name, else by its stamp (retitled since).
fn call_named(v: &Value) -> Option<PathBuf> {
    let name = v["call"].as_str()?;
    call_dir(name).or_else(|| calls::by_stamp(&calls::stamp_of(Path::new(name))))
}

/// A notice for the page (an action that couldn't be done).
fn notice(text: &str) -> Handled {
    Handled { refresh: true, reply: Some(format!("window.echo && window.echo.notice({})", json!(text))) }
}

/// Run an action the page posted.
pub fn handle(msg: &str) -> Handled {
    let v: Value = serde_json::from_str(msg).unwrap_or_default();
    match v["cmd"].as_str().unwrap_or("") {
        // Settings: one plain value, as the page shows it now.
        "settings_set" => match settings_set(v["key"].as_str().unwrap_or(""), &v["value"]) {
            Ok(()) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        // Terminals in the window (`terminal.rs`): open one, type into it, size it, end it.
        "pty_open" => {
            let (cols, rows) = (v["cols"].as_u64().unwrap_or(100) as u16, v["rows"].as_u64().unwrap_or(30) as u16);
            let kind = v["kind"].as_str().unwrap_or("");
            let key = v["key"].as_str().unwrap_or(kind).to_string();
            let opened = match kind {
                "shell" => crate::terminal::open(&key, shell_command(), cols, rows).map(|id| (id, key.clone())),
                // Claude in the data folder, on the profile picked (the default when none).
                "claude-folder" => crate::terminal::start_folder(&key, v["profile"].as_str().filter(|p| !p.is_empty()), cols, rows).map(|id| (id, key.clone())),
                // Claude on one call, guarded, on the call's own profile.
                "claude-call" => match call_named(&v) {
                    Some(dir) => crate::terminal::start_call(&key, &dir, cols, rows).map(|id| (id, key.clone())),
                    None => Err(anyhow::anyhow!("that call can't be found")),
                },
                other => Err(anyhow::anyhow!("no terminal of kind {other:?}")),
            };
            let answer = match opened {
                Ok((id, key)) => json!({ "ok": true, "id": id, "key": key }),
                Err(e) => {
                    // A profile that needs a sign-in (or is being signed in): the page offers Sign in… for it.
                    let signin = e.downcast_ref::<crate::ai::SignInNeeded>().map(|s| s.profile.clone())
                        .or_else(|| e.downcast_ref::<crate::ai::SigningIn>().map(|s| s.profile.clone()));
                    json!({ "ok": false, "kind": kind, "key": key, "error": format!("{e:#}"), "signin": signin })
                }
            };
            Handled { refresh: false, reply: Some(format!("window.echo && window.echo.ptyOpened({answer})")) }
        }
        "pty_input" => {
            if let (Some(id), Some(data)) = (v["id"].as_u64(), v["data"].as_str()) {
                let _ = crate::terminal::write(id, data.as_bytes());
            }
            false.into()
        }
        "pty_resize" => {
            if let (Some(id), Some(c), Some(r)) = (v["id"].as_u64(), v["cols"].as_u64(), v["rows"].as_u64()) {
                let _ = crate::terminal::resize(id, c as u16, r as u16);
            }
            false.into()
        }
        "pty_close" => {
            if let Some(id) = v["id"].as_u64() {
                crate::terminal::close(id);
            }
            false.into()
        }
        // Where the window's panels and views go (`Layout` in app.js); nothing else reads it.
        "save_layout" => {
            if let Err(e) = save_layout(&v["layout"]) {
                eprintln!("window layout not saved: {e:#}");
            }
            false.into()
        }
        "toggle_action" => {
            let (Some(dir), Some(id), Some(status)) = (call_named(&v), v["id"].as_str(), v["status"].as_str()) else { return false.into() };
            match calls::set_action_status(&dir, id, status) {
                Ok(()) => true.into(),
                Err(e) => notice(&format!("{e:#}")),
            }
        }
        // The user's notes from the Notes tab (`calls::save_my_notes`); the page learns what the
        // file holds now.
        "save_my_notes" => {
            let id = v["id"].clone();
            let name = v["call"].clone();
            let answer = |r: Value| Handled { refresh: true, reply: Some(format!("window.echo && window.echo.saved({r})")) };
            let Some(dir) = call_named(&v) else {
                return answer(json!({ "call": name, "id": id, "error": "this call can't be found any more" }));
            };
            match calls::save_my_notes(&dir, v["text"].as_str().unwrap_or(""), v["base"].as_str().unwrap_or("")) {
                Ok(s) => answer(json!({ "call": name, "id": id, "stored": s.stored, "kept": s.kept })),
                Err(e) => answer(json!({ "call": name, "id": id, "error": format!("{e:#}") })),
            }
        }
        "open_minutes" | "reveal_minutes" => {
            let Some(dir) = call_named(&v) else { return false.into() };
            match crate::minutes::write(&dir) {
                Ok(page) if v["cmd"] == "open_minutes" => crate::app::open_path(&page),
                Ok(page) => crate::app::reveal_file(&page),
                Err(e) => return notice(&format!("{e:#}")),
            }
            false.into()
        }
        _ => act(&v).into(),
    }
}

/// The page's other actions; true if the page should refresh.
fn act(v: &Value) -> bool {
    let call = v["call"].as_str().and_then(call_dir);
    match v["cmd"].as_str().unwrap_or("") {
        "delete" => call.is_some_and(|d| calls::delete(&d.display().to_string()).is_ok()),
        "reveal" => {
            if let Some(d) = call {
                crate::app::reveal(&d);
            }
            false
        }
        "write_notes" => {
            let Some(d) = call else { return false };
            // Another profile for this run: it becomes the call's (`notes-run.json` records which
            // profile and model wrote the minutes). Not while a run of this call is on.
            if let Some(id) = profile(v) {
                if crate::notes::status(&d).is_some_and(|r| crate::notes::pending(&r)) {
                    return true; // the page shows that run
                }
                if crate::ai::call_profile_id(&d).ok().as_deref() != Some(id.as_str()) {
                    if let Err(e) = crate::ai::set_call_profile(&d, &id) {
                        crate::notes::refuse(&d, &format!("{e:#}"));
                        return true;
                    }
                }
            }
            crate::notes::start_in_background(d);
            true
        }
        "set_profile" => {
            let (Some(d), Some(id)) = (call, profile(v)) else { return false };
            // From now on an older choice's sign-in check opens nothing.
            let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            crate::signin::choose(&format!("call:{name}"), &id);
            match crate::ai::set_call_profile(&d, &id) {
                // While it records, the watcher applies it and tells the live notes itself (so
                // no job starts on the old profile in between) — and signs it in if needed.
                Ok(crate::ai::ProfileChange::Applied) => {
                    crate::livenotes::profile_changed(&d);
                    std::thread::spawn(move || crate::signin::applied(&name, &id));
                }
                Ok(crate::ai::ProfileChange::Requested) => {}
                Err(e) => crate::livenotes::report(&d, &format!("couldn't switch to {id}: {e:#}")),
            }
            true
        }
        // Signing a Claude profile in (`signin.rs`): the flows run on threads of their own.
        "sign_in" => {
            if let Some(id) = profile(v) {
                crate::signin::start(&id);
            }
            true
        }
        "sign_in_if_needed" => {
            let (Some(id), Some(purpose)) = (profile(v), v["purpose"].as_str().filter(|p| !p.is_empty()).map(str::to_owned)) else { return false };
            // Registered here, in the order the choices came; checked on a thread of its own.
            let request = crate::signin::choose(&purpose, &id);
            std::thread::spawn(move || crate::signin::ensure_request(&id, &purpose, request));
            true
        }
        "sign_in_cancel" => {
            if let Some(id) = profile(v) {
                crate::signin::cancel(&id);
            }
            true
        }
        "check_sign_in" => {
            if let Some(p) = profile(v).and_then(|id| crate::ai::Settings::load().ok()?.get(&id).ok()) {
                crate::signin::refresh(&p, true);
            }
            true
        }
        "accept_account" => {
            let (Some(id), Some(shown)) = (profile(v), v["account"].as_str().filter(|a| !a.is_empty())) else { return false };
            crate::signin::accept(&id, shown);
            true
        }
        "live_ask" => {
            let Some(d) = call else { return false };
            let text = v["text"].as_str().unwrap_or("");
            match v["kind"].as_str().and_then(crate::livenotes::AskKind::parse) {
                Some(kind) => {
                    if let Err(e) = crate::livenotes::ask(&d, kind, text) {
                        crate::livenotes::refused(&d, kind, text, &format!("{e:#}"));
                    }
                }
                None => crate::livenotes::refused(&d, crate::livenotes::AskKind::Ask, text, "unknown kind of question"),
            }
            true
        }
        "live_pause" | "live_resume" => {
            let Some(d) = call else { return false };
            crate::livenotes::pause(&d, v["cmd"] == "live_pause");
            true
        }
        "show_log" => {
            if let Some(d) = call {
                open_file(&d.join(crate::notes::LOG));
            }
            false
        }
        "record" => watch::arm(None, None, languages(v), profile(v)).is_ok(),
        "stop" => watch::stop().is_ok(),
        "schedule" | "unschedule" => {
            let Some(id) = v["id"].as_str() else { return false };
            if v["cmd"] == "unschedule" {
                return crate::schedule::remove_id(id).is_ok();
            }
            let raw = RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|e| e["id"].as_str() == Some(id)).cloned();
            let meeting = raw.as_ref().and_then(crate::calendar::to_meeting)
                .or_else(|| watch::agenda().into_iter().find(|m| m.id.as_deref() == Some(id)));
            meeting.is_some_and(|mut m| {
                m.languages = languages(v).unwrap_or_default();
                m.ai_profile = profile(v);
                crate::schedule::add(m).is_ok()
            })
        }
        "copy" => {
            if let Some(text) = v["text"].as_str() {
                copy_to_clipboard(text);
            }
            false
        }
        "arm_meeting" | "join_meeting" => {
            let id = v["id"].as_str().unwrap_or("");
            let Some(mut m) = watch::agenda().into_iter().find(|m| m.id.as_deref() == Some(id)) else { return false };
            m.languages = languages(v).unwrap_or_default();
            m.ai_profile = profile(v);
            if v["cmd"] == "join_meeting" { watch::join(m).is_ok() } else { watch::arm(m.subject.clone(), Some(m), None, None).is_ok() }
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

/// A text file in the user's default editor.
fn open_file(path: &Path) {
    if !path.is_file() {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-t").arg(path).spawn();
    #[cfg(windows)]
    let _ = std::process::Command::new("notepad").arg(path).spawn();
}

pub(crate) fn copy_to_clipboard(text: &str) {
    use std::io::Write;
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("pbcopy").stdin(std::process::Stdio::piped()).spawn();
    #[cfg(windows)]
    let cmd = std::process::Command::new("clip").no_window().stdin(std::process::Stdio::piped()).spawn();
    #[cfg(not(any(target_os = "macos", windows)))]
    let cmd: std::io::Result<std::process::Child> = Err(std::io::Error::other("no clipboard"));
    if let Ok(mut child) = cmd {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// (dev) Write what echo:// serves — the page, its assets, the API responses it loads first and
/// each call's minutes page — into `dir`, so the UI can be reviewed in an ordinary browser
/// (`python3 -m http.server`).
pub fn export(dir: &Path) -> anyhow::Result<()> {
    let fetch = |path: &str| respond(&Request::builder().uri(format!("echo://localhost{path}")).body(Vec::new()).expect("request"));
    let get = |path: &str| -> Vec<u8> { fetch(path).into_body().into_owned() };
    let write = |rel: &str, body: Vec<u8>| -> anyhow::Result<()> {
        let p = dir.join(rel.trim_start_matches('/'));
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(p, body)?;
        Ok(())
    };
    write("index.html", get("/"))?;
    for p in ["/assets/app.js", "/assets/app.css", "/assets/logo.png", "/api/state", "/api/calls", "/api/agenda", "/api/layout", "/api/settings", "/api/sessions", "/api/ptys"] {
        write(p, get(p))?;
    }
    let week_start = chrono::Local::now().date_naive().pred_opt().unwrap_or_default().format("%Y-%m-%d");
    write("/api/calendar", get(&format!("/api/calendar?start={week_start}&days=7")))?;
    for d in calls::all_calls() {
        let name = d.file_name().unwrap().to_string_lossy().into_owned();
        write(&format!("/api/call/{name}"), get(&format!("/api/call/{}", urlencode(&name))))?;
        // The Minutes tab's frame (`minutes/<call>?v=…`): a folder's index.html, which a static
        // server answers as a page (it redirects to the folder, query kept).
        // One that can't be rendered is written as what the frame would show.
        let page = fetch(&format!("/minutes/{}", urlencode(&name)));
        if page.status() == 200 || d.join("notes.md").exists() {
            write(&format!("/minutes/{name}/index.html"), page.into_body().into_owned())?;
        }
    }
    println!("{}", dir.display());
    Ok(())
}

// ── self-test (`echoctl window --selftest`) ─────────────────────────────────────────────────

/// Set while `echoctl window --selftest` runs: `/selftest/frame` is served only then.
static SELFTEST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The selftest's frame, sandboxed like the minutes page: it tells the page it ran, then posts to
/// the window's IPC directly — which `from_page` must drop.
const SELFTEST_FRAME_JS: &str = "window.parent.postMessage({ echo: 'ready', ipc: !!((window.webkit && window.webkit.messageHandlers && window.webkit.messageHandlers.ipc) || (window.chrome && window.chrome.webview)) }, '*');
try { window.webkit.messageHandlers.ipc.postMessage('{\"probe\":\"frame\"}'); } catch (e) {}
try { window.chrome.webview.postMessage('{\"probe\":\"frame\"}'); } catch (e) {}";

fn selftest_frame() -> (String, String) {
    use base64::Engine as _;
    use sha2::Digest as _;
    let hash = base64::engine::general_purpose::STANDARD.encode(sha2::Sha256::digest(SELFTEST_FRAME_JS.as_bytes()));
    let csp = format!("default-src 'none'; script-src 'sha256-{hash}'");
    (format!("<!doctype html><html><head><meta charset=\"utf-8\"></head><body><script>{SELFTEST_FRAME_JS}</script></body></html>"), csp)
}

/// Open the window on its own event loop, let the page fetch through echo:// and report back
/// over IPC, print the report, exit. Proves the protocol, the API and IPC on this machine.
pub fn selftest() -> anyhow::Result<()> {
    use tao::event::{Event, StartCause};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    SELFTEST.store(true, std::sync::atomic::Ordering::SeqCst);
    let event_loop = EventLoopBuilder::<String>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let mut win: Option<EchoWindow> = None; // kept alive for the loop's lifetime
    let mut probed = false; // a frame's message got through to IPC
    let mut report: Option<Value> = None; // the page's, waiting for the close handshake
    let t0 = std::time::Instant::now();
    event_loop.run(move |event, target, flow| {
        *flow = ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(200));
        match event {
            Event::NewEvents(StartCause::Init) => {
                let p = proxy.clone();
                match open(target, None, move |msg| drop(p.send_event(msg))) {
                    Ok(w) => {
                        let _ = w.webview.evaluate_script("setTimeout(() => window.echo && window.echo.selftest(), 300)");
                        win = Some(w);
                    }
                    Err(e) => {
                        println!("{{\"ok\":false,\"error\":{}}}", json!(e.to_string()));
                        *flow = ControlFlow::Exit;
                    }
                }
            }
            Event::UserEvent(msg) if msg.contains("\"probe\"") => probed = true,
            // The page's terminal check: open a shell, type into it (as the tray runs these).
            Event::UserEvent(msg) if msg.contains("\"cmd\":\"pty_") => {
                if let (Some(js), Some(w)) = (handle(&msg).reply, &win) {
                    let _ = w.webview.evaluate_script(&js);
                }
            }
            Event::UserEvent(msg) if msg.contains("\"selftest\"") => {
                let mut out: Value = serde_json::from_str(&msg).unwrap_or_default();
                out["frame_blocked"] = json!(!probed);
                report = Some(out);
                // Then the close handshake, as the tray runs it: the page saves and answers with its number.
                if let Some(w) = &win {
                    let _ = w.webview.evaluate_script("window.echo.prepareClose(7, false)");
                }
            }
            Event::UserEvent(msg) if report.is_some() && closing(&msg).is_some() => {
                let mut out = report.take().unwrap_or_default();
                out["close_handshake"] = json!(matches!(closing(&msg), Some(Closing::Ready { ok: true, n: 7 })));
                let ok = out["ok"] == true && out["frame_ready"] == true && !probed && out["close_handshake"] == true;
                out["ok"] = json!(ok);
                println!("{out}");
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
    fn only_the_windows_own_page_reaches_its_actions() {
        for base in ["echo://localhost/", "http://echo.localhost/"] {
            let b = base.trim_end_matches('/');
            for ok in ["/", "/index.html", "/#call/2026-10-23_090000-x/minutes", "/?x=1"] {
                assert!(from_page(&format!("{b}{ok}"), base), "{b}{ok}");
            }
            for no in ["/minutes/2026-10-23_090000-x?v=1", "/selftest/frame", "/assets/app.js", "/index.html/x", ""] {
                assert!(!from_page(&format!("{b}{no}"), base), "{b}{no}");
            }
        }
        assert!(!from_page("https://localhost/", "echo://localhost/"));
        assert!(!from_page("echo://localhostx/", "echo://localhost/"));
        assert!(!from_page("about:blank", "echo://localhost/"));
    }

    #[test]
    fn a_calls_revision_changes_with_what_it_shows() {
        let (_env, root) = crate::paths::testenv::folders("window-rev");
        let d = root.join("home/calls/2026-10-23_090000-x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("meta.json"), "{}").unwrap();
        let r0 = call_rev(&d);
        assert_eq!(call_rev(&d), r0);
        // An answer appended (no other file changes), a run's owner gone: both show.
        fs::write(d.join("live-asks.jsonl"), "{}\n").unwrap();
        let r1 = call_rev(&d);
        assert_ne!(r1, r0);
        let gone = std::process::Command::new("true").spawn().unwrap();
        let pid = gone.id();
        drop(gone.wait_with_output());
        fs::write(d.join(crate::notes::RUN), json!({ "state": "running", "pid": pid }).to_string()).unwrap();
        let r2 = call_rev(&d);
        assert_ne!(r2, r1);
    }

    #[test]
    fn an_update_is_offered_when_the_users_notes_changed_after_the_minutes() {
        let (_env, root) = crate::paths::testenv::folders("window-notes-changed");
        let d = root.join("home/calls/2026-10-23_090000-x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("meta.json"), "{}").unwrap();
        let at = |f: &str, ago: u64| fs::File::options().write(true).open(d.join(f)).unwrap().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(ago)).unwrap();
        // Minutes a session wrote (no run): compared with when they were written.
        fs::write(d.join("notes.md"), "# m").unwrap();
        fs::write(d.join(calls::USER_NOTES), "mine").unwrap();
        at("notes.md", 600);
        at(calls::USER_NOTES, 60);
        assert_eq!(call_detail(&d)["notes_changed"], true);
        at(calls::USER_NOTES, 900);
        assert_eq!(call_detail(&d)["notes_changed"], false);
        // Minutes a run wrote: compared with when that run started (it read the notes then).
        let started = chrono::Local::now() - chrono::Duration::seconds(300);
        fs::write(d.join(crate::notes::RUN), json!({ "state": "done", "started_at": started.to_rfc3339() }).to_string()).unwrap();
        at(calls::USER_NOTES, 400);
        assert_eq!(call_detail(&d)["notes_changed"], false);
        at(calls::USER_NOTES, 100);
        assert_eq!(call_detail(&d)["notes_changed"], true);
    }

    #[test]
    fn the_export_has_each_calls_minutes_page_where_its_frame_looks() {
        let (_env, root) = crate::paths::testenv::folders("window-export");
        let with = root.join("home/calls/2026-10-23_090000-with minutes");
        let without = root.join("home/calls/2026-10-23_100000-without");
        for d in [&with, &without] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join("meta.json"), "{}").unwrap();
        }
        fs::write(with.join("notes.md"), "# Kickoff\n\n- Budget agreed [01:02]\n").unwrap();
        let out = root.join("export");
        export(&out).unwrap();
        let page = fs::read_to_string(out.join("minutes/2026-10-23_090000-with minutes/index.html")).unwrap();
        assert!(page.contains("Budget agreed"), "the page itself");
        assert!(out.join("api/call/2026-10-23_100000-without").is_file() && !out.join("minutes/2026-10-23_100000-without").exists());
    }

    #[test]
    fn urls_round_trip() {
        assert_eq!(urldecode(&urlencode("2026-10-09_183453-lumen kickoff/é")), "2026-10-09_183453-lumen kickoff/é");
    }
}

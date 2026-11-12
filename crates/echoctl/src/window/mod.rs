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

mod assets;
mod call;
mod calendar;
mod settings;
mod threads;
mod today;
mod plugins;
mod terminals;
#[cfg(feature = "brain")]
mod brain;

use self::call::act;
pub use self::settings::settings_action;
pub(crate) use self::terminals::copy_to_clipboard;

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
    /// Windows: Echo's own title bar is on, and the maximised state the page was last told (`report_maximized`).
    #[cfg(windows)]
    own: std::cell::Cell<bool>,
    #[cfg(windows)]
    told_maximized: std::cell::Cell<Option<bool>>,
}

impl EchoWindow {
    /// Ask the page to reload its data (after an action, or when the files changed).
    pub fn refresh(&self) {
        let _ = self.webview.evaluate_script("window.echo && window.echo.refresh()");
    }

    /// Go where an echo:// link points (the page asks `/api/link` and routes).
    pub fn open_link(&self, link: &str) {
        let _ = self.webview.evaluate_script(&format!("window.echo && window.echo.openLink({})", json!(link)));
    }

    /// Show a call (`name` = its folder name).
    pub fn show_call(&self, name: &str) {
        let _ = self.webview.evaluate_script(&format!("window.echo && window.echo.openCall({})", json!(name)));
    }

    /// What the page asks of the window itself (`window_cmd`).
    pub fn page_cmd(&self, msg: &str) {
        match window_cmd(msg) {
            Some(WindowCmd::Drag) => drop(self.window.drag_window()),
            Some(WindowCmd::Zoom) => self.window.set_maximized(!self.window.is_maximized()),
            Some(WindowCmd::Minimize) => self.window.set_minimized(true),
            Some(WindowCmd::ResizeTop(d)) => drop(self.window.drag_resize_window(d)),
            // The page (re)loaded: it learns the title bar and the maximised state afresh.
            #[cfg(windows)]
            Some(WindowCmd::Sync) => self.tell_page(),
            #[cfg(not(windows))]
            Some(WindowCmd::Sync) => {}
            None => {}
        }
    }

    /// Windows: after a page command, drop or bring back the system title bar if it set "Echo's own title bar" (the
    /// strip then holds minimise, maximise and close).
    #[cfg(windows)]
    pub fn follow_settings(&self, msg: &str) {
        if let Some(on) = own_title_bar_set(msg) {
            self.own.set(on);
            self.window.set_decorations(!on);
            self.tell_page();
        }
    }

    /// Windows: the page's view of the title bar, as saved (a save that failed puts its switch back), and the
    /// maximised state afresh.
    #[cfg(windows)]
    fn tell_page(&self) {
        let js = format!("window.echo && window.echo.ownTitleBar && window.echo.ownTitleBar({})", self.own.get());
        let _ = self.webview.evaluate_script(&js);
        self.told_maximized.set(None);
        self.report_maximized();
    }

    /// Windows, Echo's own title bar: tell the page when the window was maximised or restored (its maximise /
    /// restore button) — on a change only, as resizes come many a second while dragging an edge.
    #[cfg(windows)]
    pub fn report_maximized(&self) {
        let max = self.window.is_maximized();
        if !self.own.get() || self.told_maximized.get() == Some(max) {
            return;
        }
        self.told_maximized.set(Some(max));
        let _ = self.webview.evaluate_script(&format!("window.echo && window.echo.maximized && window.echo.maximized({max})"));
    }
}

/// Settings → Layout → "Echo's own title bar" (Windows; macOS always draws its own under the traffic lights): on
/// unless the user turned it off — a config without it, or none to read, counts as on.
fn own_title_bar() -> bool {
    cfg!(windows) && crate::config::Config::load().map_or(true, |c| c.own_title_bar.unwrap_or(true))
}

/// One size of `packaging/windows/echo.ico` (its entries are PNGs), as a window icon.
#[cfg(windows)]
fn exe_icon(size: u32) -> Option<tao::window::Icon> {
    static ICO: &[u8] = include_bytes!("../../../../packaging/windows/echo.ico");
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
    let own = own_title_bar();
    #[cfg(windows)]
    let builder = {
        use tao::platform::windows::WindowBuilderExtWindows;
        // Settings → Layout → "Echo's own title bar": no system frame, its shadow kept (`follow_settings` switches it).
        builder
            .with_window_icon(exe_icon(32))
            .with_taskbar_icon(exe_icon(256))
            .with_decorations(!own)
            .with_undecorated_shadow(true)
    };
    let window = builder
        .build(target)
        .map_err(|e| wry::Error::Io(std::io::Error::other(e.to_string())))?;
    let url = match start_call {
        // An echo:// link: the page routes it as it loads (`#link=…`).
        Some(c) if c.starts_with("echo://") => format!("{}#link={}", base_url(), urlencode(c)),
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
    Ok(EchoWindow {
        window,
        webview,
        #[cfg(windows)]
        own: std::cell::Cell::new(own),
        #[cfg(windows)]
        told_maximized: std::cell::Cell::new(None),
    })
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
    open_external_with(url, |url| {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).status();
        #[cfg(windows)]
        let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).status();
        #[cfg(not(any(target_os = "macos", windows)))]
        let _ = url;
    });
}

/// `open_external` with what opens it given: test data (`ECHOCTL_TEST_OPEN_LOG_ONLY`) logs the link and never gets
/// there — any OS, the served window too.
fn open_external_with(url: &str, open: impl FnOnce(&str)) {
    if crate::app::open_log_only(url) {
        return;
    }
    open(url);
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

// Shared by the areas: a query's parameter, a job's end marker.
fn query_param(q: &str, key: &str) -> Option<String> {
    q.split('&').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| urldecode(v)))
}

/// Runs its closure when dropped: a background job's end, a panic too (no marker left behind).
struct OnEnd<F: FnOnce()>(pub(super) Option<F>);
impl<F: FnOnce()> Drop for OnEnd<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f()
        }
    }
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
    // The areas in turn, the first answer wins. Some match a whole prefix (threads `/api/client/` and
    // `/api/project/`, call `/api/call/`, `/minutes/` and `/file/`, plugins `/plugin/`): a new path under one goes in
    // that area, or a later area never sees it.
    if let Some(r) = assets::route(&path, req) {
        return r;
    }
    if let Some(r) = call::route(&path, req) {
        return r;
    }
    if let Some(r) = calendar::route(&path, req) {
        return r;
    }
    if let Some(r) = settings::route(&path, req) {
        return r;
    }
    if let Some(r) = threads::route(&path, req) {
        return r;
    }
    if let Some(r) = today::route(&path, req) {
        return r;
    }
    if let Some(r) = plugins::route(&path, req) {
        return r;
    }
    if let Some(r) = terminals::route(&path, req) {
        return r;
    }
    #[cfg(feature = "brain")]
    if let Some(r) = brain::route(&path, req) {
        return r;
    }
    match path.as_str() {
        "/api/state" => json_reply(state()),
        // Where an echo:// link goes (the page routes it: `window.echo.openLink`).
        "/api/link" => {
            let q = req.uri().query().unwrap_or("");
            json_reply(crate::links::target(&query_param(q, "u").unwrap_or_default()).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        "/selftest/frame" if SELFTEST.load(std::sync::atomic::Ordering::SeqCst) => {
            let (html, csp) = selftest_frame();
            page_reply(html, &csp)
        }
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

fn state() -> Value {
    let w = watch::running_watcher();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "recording": w.as_ref().and_then(|w| w.recording.clone()),
        "recording_memo": w.as_ref().is_some_and(|w| w.memo),
        // The recording call's language, which the pill changes (`call_language::change`).
        "recording_language": w.as_ref().and_then(|w| w.recording.as_ref()).and_then(|d| std::fs::read(std::path::Path::new(d).join("meta.json")).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|m| m["language"].as_str().map(str::to_owned)),
        "armed": w.as_ref().is_some_and(|w| w.armed),
        "in_call": w.as_ref().and_then(|w| w.in_call),
        "home": paths::home(),
        "calendar": crate::graph::account(),
        "platform": std::env::consts::OS,
        "own_title_bar": own_title_bar(),
        "notes": crate::notes::readiness(),
        "languages": crate::config::Config::load().map(|c| c.languages).unwrap_or_default(),
        // A release's one-time banner, until answered (`announce.rs`).
        "announcement": crate::announce::current(),
        "ai": crate::ai::summary(),
        // The Brain: built in and switched on (off hides it everywhere in the window).
        "brain": cfg!(feature = "brain") && { #[cfg(feature = "brain")] { crate::brain::enabled() } #[cfg(not(feature = "brain"))] { false } },
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

/// What the page asks of the window itself (it draws the title bar: macOS, and Windows with Echo's own title bar;
/// its close button sends `close_request`).
pub enum WindowCmd {
    Drag,
    Zoom,
    Minimize,
    /// Windows, Echo's own title bar: resize from the top edge (the page covers the window's own border there).
    ResizeTop(tao::window::ResizeDirection),
    /// The page loaded: tell it the title bar and maximised state.
    Sync,
}

pub fn window_cmd(msg: &str) -> Option<WindowCmd> {
    let v: Value = serde_json::from_str(msg).ok()?;
    match v["cmd"].as_str()? {
        "window_drag" => Some(WindowCmd::Drag),
        "window_zoom" => Some(WindowCmd::Zoom),
        "window_min" => Some(WindowCmd::Minimize),
        "window_sync" => Some(WindowCmd::Sync),
        "window_resize_top" => Some(WindowCmd::ResizeTop(match v["edge"].as_str()? {
            "n" => tao::window::ResizeDirection::North,
            "nw" => tao::window::ResizeDirection::NorthWest,
            "ne" => tao::window::ResizeDirection::NorthEast,
            _ => return None,
        })),
        _ => None,
    }
}

/// The page set Settings → Layout → "Echo's own title bar": the window follows at once, as saved.
#[cfg(windows)]
fn own_title_bar_set(msg: &str) -> Option<bool> {
    let v: Value = serde_json::from_str(msg).ok()?;
    (v["cmd"] == "settings_set" && v["key"] == "own_title_bar").then(own_title_bar)
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
    if let Some(r) = call::handle(&v) {
        return r;
    }
    if let Some(r) = settings::handle(&v) {
        return r;
    }
    if let Some(r) = threads::handle(&v) {
        return r;
    }
    if let Some(r) = today::handle(&v) {
        return r;
    }
    if let Some(r) = plugins::handle(&v) {
        return r;
    }
    if let Some(r) = terminals::handle(&v) {
        return r;
    }
    #[cfg(feature = "brain")]
    if let Some(r) = brain::handle(&v) {
        return r;
    }
    // Anything else is an action on a call.
    act(&v).into()
}

/// (dev) Write what echo:// serves — the page, its assets, the API responses it loads first and
/// each call's minutes page — into `dir`, so the UI can be reviewed in an ordinary browser
/// (`python3 -m http.server`).
/// `echoctl window --serve <port>` (dev): the page and its API answered live on 127.0.0.1, by the same `respond` as
/// the window, so a browser shows real data with paging and search working. With `ECHOCTL_SERVE_IPC=1` the page's
/// messages are taken too (`POST /ipc` → `handle`, as the window's), from that page only: a per-run token in a
/// SameSite=Strict cookie, and the request's Origin must be this server — a dev data folder's actions, never the user's.
pub fn serve(port: u16) -> anyhow::Result<()> {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
    let origin = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let ipc = std::env::var("ECHOCTL_SERVE_IPC").is_ok_and(|v| v == "1");
    let token: String = if ipc { (0..32).map(|_| format!("{:x}", rand_nibble())).collect() } else { String::new() };
    // Cookies belong to the host, not the port: each server its own name, so two served windows don't log each other out.
    let name = format!("echo_serve_{}", listener.local_addr()?.port());
    // With IPC on and no app running on this data folder, the served window stands in for the app: its plugins load
    // as the app's do and hear what other processes queued for them (`echoctl project delete` → `threads_changed`).
    // While the app runs, the app keeps them.
    #[cfg(feature = "js")]
    let app = ipc && crate::watch::running_watcher().is_none();
    #[cfg(not(feature = "js"))]
    let app = false;
    #[cfg(feature = "js")]
    if app {
        crate::jshost::as_app();
        crate::jshost::sync();
    }
    println!("{origin}/{}{}", if ipc { "  (IPC on)" } else { "" }, if app { "  (as the app: plugins' events here)" } else { "" });
    for conn in listener.incoming().flatten() {
        let (origin, token, name) = (origin.clone(), token.clone(), name.clone());
        std::thread::spawn(move || {
            let mut reader = BufReader::new(&conn);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                return;
            }
            let (mut length, mut cookie_ok, mut origin_ok) = (0usize, false, false);
            let mut h = String::new();
            while reader.read_line(&mut h).is_ok_and(|n| n > 2) {
                let (k, v) = h.split_once(':').map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).unwrap_or_default();
                match k.as_str() {
                    "content-length" => length = v.parse().unwrap_or(0).min(1 << 20),
                    "cookie" => cookie_ok = !token.is_empty() && v.split(';').any(|c| c.trim() == format!("{name}={token}")),
                    "origin" => origin_ok = v == origin,
                    _ => {}
                }
                h.clear();
            }
            let mut parts = line.split_whitespace();
            let (method, path) = (parts.next().unwrap_or("GET").to_string(), parts.next().unwrap_or("/").to_string());
            let mut out = &conn;
            if method == "POST" && path == "/ipc" {
                let mut body = vec![0u8; length];
                let ok = reader.read_exact(&mut body).is_ok() && cookie_ok && origin_ok;
                let answer = if ok {
                    let h = handle(&String::from_utf8_lossy(&body));
                    json!({"reply": h.reply, "refresh": h.refresh}).to_string()
                } else {
                    "{\"error\":\"refused\"}".into()
                };
                let _ = write!(out, "HTTP/1.1 {} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", if ok { 200 } else { 403 }, answer.len());
                return;
            }
            let res = respond(&Request::builder().uri(format!("echo://localhost{path}")).body(Vec::new()).expect("request"));
            let ctype = res.headers().get("Content-Type").and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream").to_string();
            let status = res.status().as_u16();
            let body = res.into_body();
            let cookie = if !token.is_empty() && ctype.starts_with("text/html") { format!("Set-Cookie: {name}={token}; SameSite=Strict; HttpOnly; Path=/\r\n") } else { String::new() };
            let _ = write!(out, "HTTP/1.1 {status} OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{cookie}Connection: close\r\n\r\n", body.len());
            let _ = out.write_all(&body);
        });
    }
    Ok(())
}

/// A random hex digit (the dev server's token), from the OS's randomness.
fn rand_nibble() -> u8 {
    let mut b = [0u8; 1];
    getrandom::fill(&mut b).expect("the OS's randomness");
    b[0] & 15
}

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
    for p in ["/assets/app.js", "/assets/app.css", "/assets/logo.png", "/assets/vendor/sigma.min.js", "/assets/vendor/graphology.umd.min.js", "/assets/vendor/graphology-library.min.js", "/api/state", "/api/calls", "/api/agenda", "/api/layout", "/api/settings", "/api/plugins", "/api/deliver", "/api/sessions", "/api/ptys"] {
        write(p, get(p))?;
    }
    let week_start = chrono::Local::now().date_naive().pred_opt().unwrap_or_default().format("%Y-%m-%d");
    write("/api/calendar", get(&format!("/api/calendar?start={week_start}&days=7")))?;
    write("/api/projects", get("/api/projects"))?;
    write("/api/found", get("/api/found"))?;
    write("/api/attention", get("/api/attention"))?;
    write("/api/tasks", get("/api/tasks"))?;
    #[cfg(feature = "brain")]
    {
        write("/api/brain/index.html", get("/api/brain"))?;
        for x in ["groups", "terms", "graph"] {
            write(&format!("/api/brain/browse/{x}"), get(&format!("/api/brain/browse/{x}")))?;
        }
        write("/api/brain/people", get("/api/brain/people"))?;
        for c in crate::projects::clients().unwrap_or_default() {
            write(&format!("/api/brain/insights-client-{}", c.id), get(&format!("/api/brain/insights?client={}", c.id)))?;
        }
        for p in crate::projects::list() {
            write(&format!("/api/brain/thread-{}", p.id), get(&format!("/api/brain?thread={}", p.id)))?;
        }
        write("/api/brain/sources", get("/api/brain/sources"))?;
        write("/api/brain/files", get("/api/brain/files"))?;
        write("/api/brain/numbers", get("/api/brain/numbers"))?;
    }
    for p in crate::projects::list() {
        // A folder (its work and logo live under it): the project itself as its index.
        write(&format!("/api/project/{}/index.html", p.id), get(&format!("/api/project/{}", p.id)))?;
        write(&format!("/api/project/{}/work", p.id), get(&format!("/api/project/{}/work", p.id)))?;
        write(&format!("/api/project/{}/people", p.id), get(&format!("/api/project/{}/people", p.id)))?;
        if crate::projects::logo(&p.id).is_some() {
            write(&format!("/api/project/{}/logo", p.id), get(&format!("/api/project/{}/logo", p.id)))?;
        }
    }
    for c in crate::projects::clients().unwrap_or_default() {
        write(&format!("/api/client/{}/index.html", c.id), get(&format!("/api/client/{}", c.id)))?;
        write(&format!("/api/client/{}/people", c.id), get(&format!("/api/client/{}/people", c.id)))?;
        if crate::projects::client_logo(&c.id).is_some() {
            write(&format!("/api/client/{}/logo", c.id), get(&format!("/api/client/{}/logo", c.id)))?;
        }
    }
    for d in calls::all_calls() {
        let name = d.file_name().unwrap().to_string_lossy().into_owned();
        write(&format!("/api/call/{name}"), get(&format!("/api/call/{}", urlencode(&name))))?;
        #[cfg(feature = "brain")]
        write(&format!("/api/brain/also/{name}"), get(&format!("/api/brain/also/{}", urlencode(&name))))?;
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
    use super::call::{call_detail, call_rev, calls_page};
    use super::terminals::{paste_bytes, shell_allowed};

    #[test]
    fn test_data_logs_an_outside_link_and_never_opens_it() {
        let (_env, root) = crate::paths::testenv::folders("window-open-log-only");
        let log = root.join("open.log");
        // Only ever set here, never unset: a test process opens nothing either way.
        std::env::set_var("ECHOCTL_TEST_OPEN_LOG_ONLY", "1");
        std::env::set_var("ECHOCTL_TEST_OPEN_LOG", &log);
        let mut opened = vec![];
        open_external_with("https://git.acme.example/pr/7", |u| opened.push(u.to_string()));
        open_external_with("obsidian://open?vault=v&file=f", |u| opened.push(u.to_string()));
        assert!(opened.is_empty(), "nothing opened: {opened:?}");
        assert_eq!(fs::read_to_string(&log).unwrap(), "https://git.acme.example/pr/7\nobsidian://open?vault=v&file=f\n", "each logged as the stand-in logs it");
        // A copy (the window's, a plugin's `ctx.ui.copy`): logged on one line, the computer's clipboard untouched.
        let mut copied = vec![];
        super::terminals::copy_to_clipboard_with("WDJB-MJHT", |t| copied.push(t.to_string()));
        super::terminals::copy_to_clipboard_with("two\nlines", |t| copied.push(t.to_string()));
        assert!(copied.is_empty(), "nothing copied: {copied:?}");
        assert!(fs::read_to_string(&log).unwrap().ends_with("copied: WDJB-MJHT\ncopied: two lines\n"));
    }

    #[test]
    fn paging_reaches_every_call_once_with_undated_folders_last() {
        let (_env, _root) = crate::paths::testenv::folders("window-calls-page-undated");
        for name in ["2026-10-20_100000-a", "2026-10-21_100000-b", "2026-10-22_100000-c", "old-dictation", "imported-standup"] {
            let d = crate::paths::calls_dir().join(name);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("meta.json"), json!({"title": name}).to_string()).unwrap();
        }
        // Page by two, following `next` as the page does: every call once, newest dated first, undated ones last.
        let (mut seen, mut before) = (Vec::new(), String::new());
        for _ in 0..5 {
            let page = calls_page(&format!("limit=2{}", if before.is_empty() { String::new() } else { format!("&before={before}") }));
            seen.extend(page["calls"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()));
            match page["next"].as_str() { Some(n) => before = n.to_string(), None => break }
        }
        assert_eq!(seen, ["2026-10-22_100000-c", "2026-10-21_100000-b", "2026-10-20_100000-a", "old-dictation", "imported-standup"]);
    }

    #[test]
    fn the_calls_list_comes_a_page_at_a_time_and_searches_in_the_app() {
        let (_env, _root) = crate::paths::testenv::folders("window-calls-page");
        for (stamp, title, who) in [("2026-10-20_100000-a", "Harbor weekly", "Maya Lindqvist"), ("2026-10-21_100000-b", "Verdant discovery", "Hannah Brooks"), ("2026-10-22_100000-c", "Harbor depot data", "Tom Okafor")] {
            let d = crate::paths::calls_dir().join(stamp);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("meta.json"), json!({"title": title, "started_at": "2026-10-20T10:00:00+02:00"}).to_string()).unwrap();
            fs::write(d.join("meeting.json"), json!({"attendees": [{"name": who}]}).to_string()).unwrap();
        }
        let names = |v: &Value| v["calls"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        let first = calls_page("limit=2");
        assert_eq!(names(&first), ["2026-10-22_100000-c", "2026-10-21_100000-b"], "newest first");
        assert_eq!((first["next"].as_str(), first["total"].as_u64()), (Some("2026-10-21_100000-b"), Some(3)));
        let rest = calls_page("limit=2&before=2026-10-21_100000-b");
        assert_eq!(names(&rest), ["2026-10-20_100000-a"]);
        assert!(rest["next"].is_null(), "the end");
        let found = calls_page("q=harbor");
        assert_eq!((names(&found).len(), found["total"].as_u64()), (2, Some(2)));
        assert_eq!(names(&calls_page("q=hannah")), ["2026-10-21_100000-b"], "by the people in it");
        assert_eq!(names(&calls_page("q=harbor%20tom")), ["2026-10-22_100000-c"], "every word");
    }

    #[test]
    fn a_client_route_without_an_id_is_a_404_not_a_panic() {
        let (_env, _root) = crate::paths::testenv::folders("window-client-no-id");
        let get = |path: &str| respond(&Request::builder().uri(format!("echo://localhost{path}")).body(Vec::new()).expect("request")).status().as_u16();
        // #146: these cut `[12..11]` out of the path and panicked the request's thread.
        for path in ["/api/client/people", "/api/client/logo", "/api/client//people", "/api/client//logo"] {
            assert_eq!(get(path), 404, "{path}");
        }
        assert_eq!(get("/api/client/acme/logo"), 404, "an id with no logo: still 404");
        assert_eq!(get("/api/client/acme/people"), 200, "an id: the people view");
    }

    #[test]
    fn a_paste_types_as_xterm_would() {
        assert_eq!(paste_bytes("a\r\nb\nc".into(), false), b"a\rb\rc");
        assert_eq!(paste_bytes("a\nb".into(), true), b"\x1b[200~a\rb\x1b[201~");
        // A forged end mark can't let the rest run as keys.
        assert_eq!(paste_bytes("x\x1b[201~rm -rf\n".into(), true), b"\x1b[200~xrm -rf\r\x1b[201~");
        assert_eq!(paste_bytes(String::new(), true), [0x16]);
    }

    #[test]
    fn a_normal_window_opens_no_shell() {
        assert!(!shell_allowed(false) && shell_allowed(true));
        // The page's own request outside the self-test: refused before anything starts.
        assert!(!SELFTEST.load(std::sync::atomic::Ordering::SeqCst));
        let reply = handle(r#"{"cmd":"pty_open","kind":"shell","key":"t","cols":80,"rows":24}"#).reply.unwrap();
        assert!(reply.contains(r#""ok":false"#) && reply.contains("no terminal of kind"), "{reply}");
    }

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
    fn a_failed_transcription_gives_its_first_line_as_the_reason() {
        let (_env, root) = crate::paths::testenv::folders("window-transcribe-error");
        let d = root.join("home/calls/2026-10-23_090000-x");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("meta.json"), "{}").unwrap();
        assert_eq!(call_detail(&d)["transcribe_error"], Value::Null);
        fs::write(d.join("transcribe.error"), format!("\n  the model could not be loaded: {}  \nstack line 1\n", "x".repeat(400))).unwrap();
        let e = call_detail(&d)["transcribe_error"].as_str().unwrap().to_owned();
        assert!(e.starts_with("the model could not be loaded: x"));
        assert_eq!(e.chars().count(), 300);
        assert_eq!(call_detail(&d)["status"], "failed");
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

    #[test]
    fn the_title_strip_minimises_and_resizes_from_its_top_edge() {
        use tao::window::ResizeDirection as D;
        assert!(matches!(window_cmd(r#"{"cmd":"window_min"}"#), Some(WindowCmd::Minimize)));
        assert!(matches!(window_cmd(r#"{"cmd":"window_resize_top","edge":"n"}"#), Some(WindowCmd::ResizeTop(D::North))));
        assert!(matches!(window_cmd(r#"{"cmd":"window_resize_top","edge":"nw"}"#), Some(WindowCmd::ResizeTop(D::NorthWest))));
        assert!(matches!(window_cmd(r#"{"cmd":"window_resize_top","edge":"ne"}"#), Some(WindowCmd::ResizeTop(D::NorthEast))));
        // No edge, or one the page never sends: not a resize.
        assert!(window_cmd(r#"{"cmd":"window_resize_top"}"#).is_none());
        assert!(window_cmd(r#"{"cmd":"window_resize_top","edge":"sw"}"#).is_none());
        // Closing isn't a window command: it goes the save-first way (`closing` → `close_request`).
        assert!(window_cmd(r#"{"cmd":"close_request"}"#).is_none());
        assert!(matches!(closing(r#"{"cmd":"close_request"}"#), Some(Closing::Request)));
    }
}

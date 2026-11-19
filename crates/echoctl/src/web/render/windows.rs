//! Windows: the page in a hidden WebView2 (no window shown, no taskbar button).
//!
//! - **Profile:** a new folder for each render (`echo-render-<pid>` in the temp folder), InPrivate, removed when the
//!   render ends.
//! - **Proxy:** the gate is the browser's only proxy, loopback included, and the render's token answers its 407
//!   (`BasicAuthenticationRequested`, for the gate's challenge only). WebRTC sends no UDP that isn't proxied; QUIC is off.
//! - **Request filter:** every request of every frame and worker is seen and counted. Only https to a name, or to a
//!   public address that isn't this computer's, goes on; no media.
//! - **Denied:** downloads, popups, permissions, script dialogs, web messages, devtools, the context menu.
//! - **Job:** the render and every process WebView2 starts are in a job of their own. When the render ends, the rest
//!   of the job is ended before its folder is removed; if the render itself is ended, closing the job ends the rest.

use std::cell::Cell;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use serde_json::json;
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::windows::WindowBuilderExtWindows;
use tao::window::{Window, WindowBuilder};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use webview2_com::{BasicAuthenticationRequestedEventHandler, DownloadStartingEventHandler, NewWindowRequestedEventHandler, WebResourceRequestedEventHandler};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcess, TerminateProcess, PROCESS_TERMINATE};
use windows_core::{Interface, HSTRING, PWSTR};
use wry::{NewWindowResponse, PageLoadEvent, PermissionResponse, WebContext, WebView, WebViewBuilder, WebViewBuilderExtWindows, WebViewExtWindows};

use super::{answer, fail, Render, INIT_JS, MAX_MOVES, QUIET, SETTLE_MOST, SNAPSHOT_JS};
use crate::web::gate::{self, Gate};
use crate::web::public;

enum Msg {
    Started,
    Loaded,
    Dom(String),
}

/// Requests the filter saw, and those it refused (the first few distinct ones kept for the stats).
#[derive(Default)]
struct Seen {
    requests: AtomicUsize,
    refused: AtomicUsize,
    examples: Mutex<Vec<String>>,
}

const EXAMPLES_KEPT: usize = 32;

/// The render's own job: this process and everything it starts (WebView2's browser, renderers, GPU, utility).
struct Job(HANDLE);

// SAFETY: a job handle may be used from any thread.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    /// A new job this process joins: whatever it starts from now on is in it, and closing the job's last handle (this
    /// process ending, however) ends them all.
    fn join() -> Result<Job> {
        // SAFETY: a job of our own, its limits set from a struct that lives through the call.
        unsafe {
            let job = Job(CreateJobObjectW(None, windows_core::PCWSTR::null())?);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(job.0, JobObjectExtendedLimitInformation, (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), std::mem::size_of_val(&limits) as u32)?;
            AssignProcessToJobObject(job.0, GetCurrentProcess())?;
            Ok(job)
        }
    }

    /// The job's processes other than this one.
    fn others(&self) -> Vec<u32> {
        // JOBOBJECT_BASIC_PROCESS_ID_LIST: two u32 counts (one usize here), then the ids.
        let mut list = vec![0usize; 1 + 512];
        let size = (list.len() * std::mem::size_of::<usize>()) as u32;
        // SAFETY: a buffer of `size` bytes, aligned for the list.
        if unsafe { QueryInformationJobObject(Some(self.0), JobObjectBasicProcessIdList, list.as_mut_ptr().cast(), size, None) }.is_err() {
            return vec![];
        }
        let n = (list[0] >> 32) as usize;
        let me = std::process::id() as usize;
        list[1..=n.min(512)].iter().filter(|&&id| id != me).map(|&id| id as u32).collect()
    }

    /// Every other process of the job ended, waiting up to 5 s for them to be gone.
    fn end_the_rest(&self) {
        let t = Instant::now();
        loop {
            let others = self.others();
            if others.is_empty() || t.elapsed() > Duration::from_secs(5) {
                return;
            }
            for id in others {
                // SAFETY: a process of our own job, its handle closed after use.
                unsafe {
                    if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, id) {
                        let _ = TerminateProcess(h, 1);
                        let _ = CloseHandle(h);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

/// The render's profile folder: `echo-render-<pid>` in the temp folder (`jshost::render` removes it too, should this
/// process be ended).
pub(crate) fn profile_dir(pid: u32) -> PathBuf {
    std::env::temp_dir().join(format!("{PROFILE_DIR}{pid}"))
}

const PROFILE_DIR: &str = "echo-render-";

/// Profile folders in `tmp` left by renders that were ended with their host (no process has their id now): removed.
fn sweep(tmp: &Path) {
    use windows::Win32::Foundation::STILL_ACTIVE;
    use windows::Win32::System::Threading::{GetExitCodeProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    for e in std::fs::read_dir(tmp).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.strip_prefix(PROFILE_DIR).and_then(|p| p.parse::<u32>().ok()) else { continue };
        // SAFETY: a handle opened only to read its exit code, closed after.
        let running = unsafe {
            OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).is_ok_and(|h| {
                let mut code = 0u32;
                let live = GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE.0 as u32;
                let _ = CloseHandle(h);
                live
            })
        };
        if !running {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// The render's end: the rest of its job ended, its profile folder removed, the process gone.
fn stop(job: &Job, dir: &Path, code: i32) -> ! {
    job.end_the_rest();
    for _ in 0..40 {
        if std::fs::remove_dir_all(dir).is_ok() || !dir.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    std::process::exit(code)
}

pub(super) fn run(r: Render) -> Result<()> {
    let job = Arc::new(Job::join()?);
    sweep(&std::env::temp_dir());
    let dir = profile_dir(std::process::id());
    let own = crate::netif::addresses()?;
    // The time cap, whatever the window's thread is doing.
    {
        let (job, dir, until) = (job.clone(), dir.clone(), r.until);
        std::thread::spawn(move || {
            std::thread::sleep(until.saturating_duration_since(Instant::now()) + Duration::from_millis(500));
            stop(&job, &dir, fail(&format!("the page didn't render within {} s", super::MOST_SECS)))
        });
    }
    let seen = Arc::new(Seen::default());
    let event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let mut view: Option<(WebView, Window, WebContext)> = None;
    let (mut moves, mut committed, mut loaded_at, mut asked) = (0usize, false, None::<Instant>, false);
    let read_by = r.until - Duration::from_secs(2);
    event_loop.run(move |event, target, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
        match event {
            Event::NewEvents(StartCause::Init) => {
                let made = WindowBuilder::new().with_visible(false).with_skip_taskbar(true).build(target).map_err(anyhow::Error::from).and_then(|w| {
                    let mut context = WebContext::new(Some(dir.clone()));
                    let v = build(&r, &mut context, &w, &own, seen.clone(), proxy.clone())?;
                    Ok((v, w, context))
                });
                match made {
                    Ok(v) => view = Some(v),
                    Err(e) => stop(&job, &dir, fail(&format!("the page's view couldn't be made: {e:#}"))),
                }
            }
            Event::UserEvent(Msg::Started) => {
                committed = true;
                moves += 1;
                if moves > MAX_MOVES + 1 {
                    stop(&job, &dir, fail(&format!("the page moved more than {MAX_MOVES} times")));
                }
            }
            Event::UserEvent(Msg::Loaded) => {
                loaded_at.get_or_insert_with(Instant::now);
            }
            Event::UserEvent(Msg::Dom(dom)) => {
                let at = view.as_ref().and_then(|(v, ..)| v.url().ok()).unwrap_or_default();
                let at = if at.starts_with("https://") || at.starts_with("http://") { at } else { r.url.clone() };
                match answer(&dom, &at, &r.gate) {
                    Ok(mut v) => {
                        v["stats"]["filter"] = json!({
                            "requests": seen.requests.load(Ordering::SeqCst),
                            "refused": seen.refused.load(Ordering::SeqCst),
                            "examples": *seen.examples.lock().unwrap_or_else(|e| e.into_inner()),
                        });
                        println!("{v}");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        stop(&job, &dir, 0)
                    }
                    Err(e) => stop(&job, &dir, fail(&format!("{e:#}"))),
                }
            }
            _ => {}
        }
        let now = Instant::now();
        let settled = loaded_at.is_some_and(|t| (now - t >= QUIET && r.gate.last_connection().elapsed() >= QUIET) || now - t >= SETTLE_MOST);
        if !asked && (settled || (committed && now >= read_by)) {
            if let Some((v, ..)) = &view {
                asked = true;
                let p = proxy.clone();
                if v.evaluate_script_with_callback(SNAPSHOT_JS, move |dom| drop(p.send_event(Msg::Dom(dom)))).is_err() {
                    stop(&job, &dir, fail("the page's DOM couldn't be read back"));
                }
            }
        }
        if now >= r.until {
            stop(&job, &dir, fail(&format!("the page didn't render within {} s", super::MOST_SECS)));
        }
    })
}

/// The browser's switches: the gate as its only proxy (loopback too), no name looked up by the browser itself (the
/// gate looks names up; even behind a proxy Chromium asked the system for the page's host), no UDP WebRTC doesn't
/// proxy, no QUIC, no background traffic of its own.
fn browser_args(gate: &Gate) -> String {
    format!(
        "--proxy-server=http://127.0.0.1:{} --proxy-bypass-list=<-loopback> --host-resolver-rules=\"MAP * ~NOTFOUND , EXCLUDE 127.0.0.1\" --force-webrtc-ip-handling-policy=disable_non_proxied_udp --disable-quic \
         --disable-background-networking --disable-component-update --no-pings --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection",
        gate.port
    )
}

/// The view, its handlers set before it loads the page.
fn build(r: &Render, context: &mut WebContext, window: &Window, own: &[IpAddr], seen: Arc<Seen>, proxy: tao::event_loop::EventLoopProxy<Msg>) -> Result<WebView> {
    let p = proxy.clone();
    let mine = own.to_vec();
    let view = WebViewBuilder::new_with_web_context(context)
        .with_additional_browser_args(browser_args(&r.gate))
        .with_incognito(true)
        .with_initialization_script_for_main_only(INIT_JS, false)
        // The main frame goes only where a request may (or to about:): a refused page would show the browser's error page.
        .with_navigation_handler(move |to| to.get(..6).is_some_and(|s| s.eq_ignore_ascii_case("about:")) || refused(&to, &mine).is_none())
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_on_page_load_handler(move |e, _| {
            let _ = p.send_event(match e {
                PageLoadEvent::Started => Msg::Started,
                PageLoadEvent::Finished => Msg::Loaded,
            });
        })
        .with_devtools(false)
        .with_default_context_menus(false)
        .with_browser_accelerator_keys(false)
        .build(window)?;
    let err = |e: windows_core::Error| anyhow!("{e}");
    let core = unsafe { view.controller().CoreWebView2() }.map_err(err)?;
    unsafe { lock_down(&core, &view.environment(), &r.gate, own, seen) }.map_err(err)?;
    view.load_url(&r.url)?;
    Ok(view)
}

/// What the page may not do, and every request it makes checked.
unsafe fn lock_down(core: &ICoreWebView2, env: &ICoreWebView2Environment, gate: &Gate, own: &[IpAddr], seen: Arc<Seen>) -> windows_core::Result<()> {
    let mut token = Default::default();
    let settings = core.Settings()?;
    settings.SetAreDefaultScriptDialogsEnabled(false)?;
    settings.SetIsWebMessageEnabled(false)?;
    settings.SetAreDevToolsEnabled(false)?;
    settings.SetAreDefaultContextMenusEnabled(false)?;
    if let Ok(s) = settings.cast::<ICoreWebView2Settings4>() {
        s.SetIsPasswordAutosaveEnabled(false)?;
        s.SetIsGeneralAutofillEnabled(false)?;
    }

    // The gate's 407, and only it: its challenge names the render's own realm (a site's own 401 gets nothing).
    let (user, pass, expected) = (HSTRING::from(gate::USER), HSTRING::from(gate.token.as_str()), gate.challenge.clone());
    core.cast::<ICoreWebView2_10>()?.add_BasicAuthenticationRequested(
        &BasicAuthenticationRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            let mut challenge = PWSTR::null();
            args.Challenge(&mut challenge)?;
            if take(challenge) == expected {
                let resp = args.Response()?;
                resp.SetUserName(&user)?;
                resp.SetPassword(&pass)?;
            } else {
                args.SetCancel(true)?;
            }
            Ok(())
        })),
        &mut token,
    )?;

    // Every request of every frame and worker.
    let core22 = core.cast::<ICoreWebView2_22>()?;
    core22.AddWebResourceRequestedFilterWithRequestSourceKinds(&HSTRING::from("*"), COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL)?;
    let (env, own) = (env.clone(), own.to_vec());
    core.add_WebResourceRequested(
        &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else { return Ok(()) };
            seen.requests.fetch_add(1, Ordering::SeqCst);
            let mut uri = PWSTR::null();
            args.Request()?.Uri(&mut uri)?;
            let uri = take(uri);
            let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
            args.ResourceContext(&mut context)?;
            let why = if context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA { Some("media") } else { refused(&uri, &own) };
            if let Some(why) = why {
                seen.refused.fetch_add(1, Ordering::SeqCst);
                let mut ex = seen.examples.lock().unwrap_or_else(|e| e.into_inner());
                let e = format!("{}: {why}", uri.chars().take(200).collect::<String>());
                if ex.len() < EXAMPLES_KEPT && !ex.contains(&e) {
                    ex.push(e);
                }
                args.SetResponse(&env.CreateWebResourceResponse(None, 403, &HSTRING::from("Forbidden"), &HSTRING::new())?)?;
            }
            Ok(())
        })),
        &mut token,
    )?;

    // No download, no popup.
    core.cast::<ICoreWebView2_4>()?.add_DownloadStarting(
        &DownloadStartingEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
                args.SetCancel(true)?;
                args.SetHandled(true)?;
            }
            Ok(())
        })),
        &mut token,
    )?;
    core.add_NewWindowRequested(
        &NewWindowRequestedEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
                args.SetHandled(true)?;
            }
            Ok(())
        })),
        &mut token,
    )?;

    #[cfg(debug_assertions)]
    if std::env::var_os("ECHOCTL_TEST_HOSTS").is_some() {
        accept_test_certificates(core)?;
    }
    Ok(())
}

fn take(s: PWSTR) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: a string WebView2 allocated for us, freed once read.
    unsafe {
        let out = s.to_string().unwrap_or_default();
        windows::Win32::System::Com::CoTaskMemFree(Some(s.0 as *const _));
        out
    }
}

/// Why a request may not go on, if it may not: anything but https, an address written as one that isn't public or is
/// this computer's (IPv6 ones all), `localhost` and its subdomains, `.local` and single-label names.
fn refused(uri: &str, own: &[IpAddr]) -> Option<&'static str> {
    let Some(rest) = uri.get(..8).filter(|s| s.eq_ignore_ascii_case("https://")).map(|_| &uri[8..]) else {
        return Some("not https");
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    if host.starts_with('[') {
        return Some("an IPv6 address");
    }
    let host = host.split(':').next().unwrap_or("").trim_end_matches('.').to_ascii_lowercase();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return (!public::public(ip) || public::this_computers(ip, own)).then_some("this computer or a private address");
    }
    if host.is_empty() || host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") || !host.contains('.') {
        return Some("a local name");
    }
    None
}

/// A development build's test: the fake sites' certificate (`test_tls`) is taken.
#[cfg(debug_assertions)]
unsafe fn accept_test_certificates(core: &ICoreWebView2) -> windows_core::Result<()> {
    let mut token = Default::default();
    core.cast::<ICoreWebView2_14>()?.add_ServerCertificateErrorDetected(
        &webview2_com::ServerCertificateErrorDetectedEventHandler::create(Box::new(|_, args| {
            if let Some(args) = args {
                args.SetAction(COREWEBVIEW2_SERVER_CERTIFICATE_ERROR_ACTION_ALWAYS_ALLOW)?;
            }
            Ok(())
        })),
        &mut token,
    )
}

/// A development build's test (`ECHOCTL_TEST_HOSTS`): the fakes speak plain http, and WebView2 sends only CONNECT to
/// the gate for https. So a connection to a fake's port 443 is put through TLS here (a made-up certificate, taken in
/// test runs only), and the fake gets plain http.
#[cfg(debug_assertions)]
pub(super) fn test_tls(net: gate::Net) -> Result<gate::Net> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let cert = CertificateDer::from_pem_slice(TEST_CERT.as_bytes()).map_err(|e| anyhow!("{e}"))?;
    let key = PrivateKeyDer::from_pem_slice(TEST_KEY.as_bytes()).map_err(|e| anyhow!("{e}"))?;
    let config = Arc::new(rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider())).with_safe_default_protocol_versions()?.with_no_client_auth().with_single_cert(vec![cert], key)?);
    let inner = net.connect.clone();
    Ok(gate::Net {
        connect: Arc::new(move |at, wait| {
            let up = inner(at, wait)?;
            if at.port() != 443 {
                return Ok(up);
            }
            let l = std::net::TcpListener::bind("127.0.0.1:0")?;
            let near = std::net::TcpStream::connect(l.local_addr()?)?;
            let (far, _) = l.accept()?;
            let config = config.clone();
            std::thread::spawn(move || tls_pump(far, up, config));
            Ok(near)
        }),
        ..net
    })
}

/// TLS from the gate's side (`down`) to plain bytes on the fake's (`up`), until either side closes.
#[cfg(debug_assertions)]
fn tls_pump(mut down: std::net::TcpStream, mut up: std::net::TcpStream, config: Arc<rustls::ServerConfig>) {
    use std::io::{ErrorKind, Read, Write};
    let Ok(mut tls) = rustls::ServerConnection::new(config) else { return };
    if down.set_nonblocking(true).is_err() || up.set_nonblocking(true).is_err() {
        return;
    }
    let write_all = |s: &mut std::net::TcpStream, mut b: &[u8]| -> bool {
        while !b.is_empty() {
            match s.write(b) {
                Ok(0) => return false,
                Ok(n) => b = &b[n..],
                Err(e) if e.kind() == ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(1)),
                Err(_) => return false,
            }
        }
        true
    };
    let mut buf = vec![0u8; 16 << 10];
    let idle = Cell::new(true);
    loop {
        idle.set(true);
        match tls.read_tls(&mut down) {
            Ok(0) => return,
            Ok(_) => {
                idle.set(false);
                if tls.process_new_packets().is_err() {
                    let _ = tls.write_tls(&mut down);
                    return;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => return,
        }
        loop {
            match tls.reader().read(&mut buf) {
                Ok(0) => return,
                Ok(n) => {
                    idle.set(false);
                    if !write_all(&mut up, &buf[..n]) {
                        return;
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => return,
            }
        }
        match up.read(&mut buf) {
            Ok(0) => {
                tls.send_close_notify();
                while tls.wants_write() && tls.write_tls(&mut down).is_ok() {}
                return;
            }
            Ok(n) => {
                idle.set(false);
                if tls.writer().write_all(&buf[..n]).is_err() {
                    return;
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(_) => return,
        }
        while tls.wants_write() {
            match tls.write_tls(&mut down) {
                Ok(_) => idle.set(false),
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(_) => return,
            }
        }
        if idle.get() {
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

/// A made-up certificate and key for the fake sites of a development build's tests (never trusted anywhere: a test
/// run takes any certificate, `accept_test_certificates`).
#[cfg(debug_assertions)]
const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----
MIIBqzCCAVGgAwIBAgIUeDVPktSMbnm1D9/8dog8Sy3b6vIwCgYIKoZIzj0EAwIw
GzEZMBcGA1UEAwwQZWNoby1yZW5kZXItdGVzdDAgFw0yNjExMTkxMTM5MDhaGA8y
MTI2MTAyNjExMzkwOFowGzEZMBcGA1UEAwwQZWNoby1yZW5kZXItdGVzdDBZMBMG
ByqGSM49AgEGCCqGSM49AwEHA0IABGc3kLttY58TZj8hkZ0EZo17p46ND9e9b06A
lPTxluqrSERuYcIYuRyGbCrkzH308Jw7kzcKTA16qbd6y4ygjbajcTBvMB0GA1Ud
DgQWBBRsY5s7Imh5ZWJ8hnHhaW/VN7lFzTAfBgNVHSMEGDAWgBRsY5s7Imh5ZWJ8
hnHhaW/VN7lFzTAPBgNVHRMBAf8EBTADAQH/MBwGA1UdEQQVMBOCEW5ld3MuYWNt
ZS5leGFtcGxlMAoGCCqGSM49BAMCA0gAMEUCIA1BZUdbaVltp5CakxpzINGIaF4U
6hcKoKgi6VwTd1V/AiEAtlDEKWhrpk7AhQqxNS3kfBsNZb4iHhf/swXr/20AdI8=
-----END CERTIFICATE-----";
#[cfg(debug_assertions)]
const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgYWeUMIYmch/qxj4P
WnkUTzQ5MGpNldDX7v8VK6VT84yhRANCAARnN5C7bWOfE2Y/IZGdBGaNe6eOjQ/X
vW9OgJT08Zbqq0hEbmHCGLkchmwq5Mx99PCcO5M3CkwNeqm3esuMoI22
-----END PRIVATE KEY-----";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_to_a_public_place_goes_on() {
        let own: Vec<IpAddr> = vec!["203.0.113.9".parse().unwrap(), "127.0.0.1".parse().unwrap()];
        for ok in ["https://news.acme.example/a", "https://NEWS.acme.example:443/a?b#c", "https://93.184.215.1/x", "https://u:p@news.acme.example/"] {
            assert_eq!(refused(ok, &own), None, "{ok}");
        }
        for bad in [
            "http://news.acme.example/", "ws://news.acme.example/", "file:///C:/Windows/win.ini", "data:text/html,x", "blob:https://a.example/1",
            "https://127.0.0.1/", "https://10.0.0.1/", "https://192.168.1.1:8443/", "https://169.254.169.254/", "https://203.0.113.9/", "https://[::1]/",
            "https://localhost/", "https://foo.localhost/", "https://printer.local/", "https://canary/", "https://u:p@127.0.0.1/", "https://news.acme.example.@10.0.0.1/",
        ] {
            assert!(refused(bad, &own).is_some(), "{bad}");
        }
    }

    #[test]
    fn a_killed_renders_profile_is_swept_next_time() {
        let tmp = std::env::temp_dir().join(format!("echo-render-sweep-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // A process id no process has (Windows' ids are multiples of 4), and this one's.
        let (dead, live) = (tmp.join(format!("{PROFILE_DIR}99999999")), tmp.join(format!("{PROFILE_DIR}{}", std::process::id())));
        for d in [&dead, &live] {
            std::fs::create_dir_all(d.join("EBWebView")).unwrap();
        }
        sweep(&tmp);
        assert!(!dead.exists() && live.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

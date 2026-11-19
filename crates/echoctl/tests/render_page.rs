//! `echoctl render-page` (#835) on this computer's own webview (WKWebView on a Mac, WebView2 on Windows). It needs a GUI
//! session, so the tests are ignored by default: `cargo test --test render_page -- --ignored --test-threads=1`.
//!
//! - **The site:** every page comes from a made-up site on 127.0.0.1, reached through the gate as
//!   `news.acme.example` (`ECHOCTL_TEST_HOSTS`, development builds only). Nothing reaches the real internet. On a Mac
//!   the page is asked for over http; on Windows over https, the child putting the site behind TLS itself.
//! - **The canary:** TCP on every interface, UDP on loopback. It records every connection a page manages to make to
//!   this computer or the user's network; there must be none.
#![cfg(all(any(target_os = "macos", windows), debug_assertions))]

use std::io::{Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Connections and packets that reached this computer outside the gate.
struct Canary {
    tcp: u16,
    udp: u16,
    hits: Arc<Mutex<Vec<String>>>,
}

fn canary() -> Canary {
    let tcp = TcpListener::bind("0.0.0.0:0").unwrap();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    let (tp, up) = (tcp.local_addr().unwrap().port(), udp.local_addr().unwrap().port());
    let hits = Arc::new(Mutex::new(vec![]));
    let h = hits.clone();
    std::thread::spawn(move || {
        for mut s in tcp.incoming().flatten() {
            let h = h.clone();
            std::thread::spawn(move || {
                let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
                let mut b = [0u8; 512];
                let n = s.read(&mut b).unwrap_or(0);
                h.lock().unwrap().push(format!("tcp from {:?}: {:?}", s.peer_addr(), String::from_utf8_lossy(&b[..n])));
            });
        }
    });
    let h = hits.clone();
    std::thread::spawn(move || {
        let mut b = [0u8; 2048];
        while let Ok((n, from)) = udp.recv_from(&mut b) {
            h.lock().unwrap().push(format!("udp {n} bytes from {from}"));
        }
    });
    Canary { tcp: tp, udp: up, hits }
}

/// This computer's address on its network (no packet sent), if it has one.
fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string()).filter(|ip| ip != "0.0.0.0")
}

/// The made-up site: `pages` by path (`{TCP}`, `{UDP}`, `{LAN}` filled in), each request's path recorded.
fn site(pages: Vec<(&'static str, String)>) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(vec![]));
    let s2 = seen.clone();
    let pages = Arc::new(pages);
    std::thread::spawn(move || {
        for mut s in l.incoming().flatten() {
            let (seen, pages) = (s2.clone(), pages.clone());
            std::thread::spawn(move || {
                let mut req = vec![];
                let mut b = [0u8; 4096];
                while !req.windows(4).any(|w| w == b"\r\n\r\n") {
                    match s.read(&mut b) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => req.extend_from_slice(&b[..n]),
                    }
                }
                let path = String::from_utf8_lossy(&req).split_whitespace().nth(1).unwrap_or("/").to_string();
                seen.lock().unwrap().push(path.clone());
                let body = pages.iter().find(|(p, _)| path.split('?').next() == Some(p)).map(|(_, b)| b.clone());
                let (status, body) = body.map_or(("404 Not Found", String::new()), |b| ("200 OK", b));
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    (port, seen)
}

/// The site's address as a render asks for it.
const SITE: &str = if cfg!(windows) { "https://news.acme.example" } else { "http://news.acme.example" };

/// `echoctl render-page` on `path` of the site, within `secs`: its output and how long it took. Its environment is
/// only what a render gets.
fn render(site: u16, path: &str, secs: u64) -> (Output, Duration) {
    let (out, took, _) = render_pid(site, path, secs);
    (out, took)
}

/// `render`, and the render's process id.
fn render_pid(site: u16, path: &str, secs: u64) -> (Output, Duration, u32) {
    let at = Instant::now();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_echoctl"));
    cmd.args(["render-page", "--url", &format!("{SITE}{path}"), "--secs", &secs.to_string()]).env_clear();
    let keep: &[&str] = if cfg!(windows) { &["SystemRoot", "SystemDrive", "windir", "TEMP", "TMP", "USERPROFILE", "LOCALAPPDATA", "APPDATA", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432", "ProgramData"] } else { &["HOME"] };
    for k in keep {
        if let Some(v) = std::env::var_os(k) {
            cmd.env(k, v);
        }
    }
    let child = cmd
        .env("TMPDIR", std::env::temp_dir())
        .env("ECHOCTL_TEST_HOSTS", format!("news.acme.example=http://127.0.0.1:{site}"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id();
    let out = child.wait_with_output().unwrap();
    (out, at.elapsed(), pid)
}

fn json(out: &Output) -> serde_json::Value {
    assert!(out.status.success(), "render failed: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A page that writes its text with JavaScript and, from every frame it can make, tries every way to this computer and
/// the user's network.
const HOSTILE: &str = r#"<!doctype html><html><head><title>Pilot notes</title><link rel="dns-prefetch" href="//{DNS}.example.org"><link rel="preconnect" href="https://{DNS}-p.example.org"></head><body><main><p id="out">loading</p></main><script>
const C = ['http://127.0.0.1:{TCP}/c', 'http://localhost:{TCP}/c', 'http://[::1]:{TCP}/c', 'http://{LAN}:{TCP}/c', 'http://10.0.0.1:{TCP}/c',
  'http://169.254.169.254/latest/meta-data/', 'http://2130706433:{TCP}/c', 'http://0.0.0.0:{TCP}/c', 'http://u:p@127.0.0.1:{TCP}/c', 'https://127.0.0.1:{TCP}/c',
  'http://news.acme.example:{TCP}/c', 'http://canary:{TCP}/c', 'http://canary.local:{TCP}/c', 'http://foo.localhost:{TCP}/c'];
const ICE = {iceServers: [{urls: ['stun:127.0.0.1:{UDP}', 'stun:{LAN}:{UDP}']}, {urls: ['turn:127.0.0.1:{TCP}?transport=tcp'], username: 'u', credential: 'c'}]};
function rtc(w) { try { const P = w.RTCPeerConnection || w.webkitRTCPeerConnection; if (P) { const pc = new P(ICE); pc.createDataChannel('x'); pc.createOffer().then(o => pc.setLocalDescription(o)); w.__pc = pc; } } catch (e) {} }
function every(w, d) {
  for (const u of C) {
    try { w.fetch(u, {mode: 'no-cors', method: 'POST', body: 'x'}).catch(() => {}); } catch (e) {}
    try { const x = new w.XMLHttpRequest(); x.open('GET', u); x.send(); } catch (e) {}
    try { const i = d.createElement('img'); i.src = u; d.body.appendChild(i); } catch (e) {}
    try { const f = d.createElement('iframe'); f.src = u; d.body.appendChild(f); } catch (e) {}
    try { new w.WebSocket(u.replace(/^http/, 'ws')); } catch (e) {}
    try { new w.EventSource(u); } catch (e) {}
    try { w.navigator.sendBeacon(u, 'x'); } catch (e) {}
    try { const l = d.createElement('link'); l.rel = 'preconnect'; l.href = u; d.head.appendChild(l); } catch (e) {}
    try { const s = d.createElement('script'); s.src = u; d.head.appendChild(s); } catch (e) {}
  }
  try { const f = d.createElement('form'); f.method = 'POST'; f.action = C[0]; f.target = 'sink'; const k = d.createElement('iframe'); k.name = 'sink'; d.body.appendChild(k); d.body.appendChild(f); f.submit(); } catch (e) {}
  rtc(w);
}
every(window, document);
// Frames made at once: about:blank (reached before any of its own code), nested, written, srcdoc, data:, blob:.
const blank = document.createElement('iframe'); document.body.appendChild(blank); rtc(blank.contentWindow); every(blank.contentWindow, blank.contentDocument);
const nested = blank.contentDocument.createElement('iframe'); blank.contentDocument.body.appendChild(nested); rtc(nested.contentWindow);
const written = document.createElement('iframe'); document.body.appendChild(written); written.contentDocument.write('<p>x</p>'); written.contentDocument.close(); rtc(written.contentWindow);
const own = s => '<script>(' + every.toString() + ')(window, document); (' + rtc.toString() + ')(window);<\/script>';
const C_ = 'const C = ' + JSON.stringify(C) + '; const ICE = ' + JSON.stringify(ICE) + ';';
const srcdoc = document.createElement('iframe'); srcdoc.srcdoc = '<script>' + C_ + '<\/script>' + own(); document.body.appendChild(srcdoc);
const sandboxed = document.createElement('iframe'); sandboxed.setAttribute('sandbox', 'allow-scripts'); sandboxed.srcdoc = '<script>' + C_ + '<\/script>' + own(); document.body.appendChild(sandboxed);
const data = document.createElement('iframe'); data.src = 'data:text/html,' + encodeURIComponent('<script>' + C_ + '<\/script>' + own()); document.body.appendChild(data);
const blob = document.createElement('iframe'); blob.src = URL.createObjectURL(new Blob(['<script>' + C_ + '<\/script>' + own()], {type: 'text/html'})); document.body.appendChild(blob);
const file = document.createElement('iframe'); file.src = 'file:///etc/hosts'; document.body.appendChild(file);
// A worker.
try { const w = new Worker(URL.createObjectURL(new Blob([C_ + 'for (const u of C) { fetch(u, {mode: "no-cors"}).catch(() => {}); try { new WebSocket(u.replace(/^http/, "ws")); } catch (e) {} }'], {type: 'text/javascript'}))); } catch (e) {}
// A popup, a download, dialogs and permissions.
try { window.open(C[0]); } catch (e) {}
try { const a = document.createElement('a'); a.href = C[0]; a.download = 'x.bin'; document.body.appendChild(a); a.click(); } catch (e) {}
try { const a = document.createElement('a'); a.href = C[0]; a.download = ''; a.click(); a.dispatchEvent(new MouseEvent('click', {bubbles: false})); } catch (e) {}
try { const m = document.createElement('map'); m.name = 'm'; const ar = document.createElement('area'); ar.href = C[1]; ar.download = 'y'; m.appendChild(ar); document.body.appendChild(m); ar.click(); } catch (e) {}
try { const a = blank.contentDocument.createElement('a'); a.href = C[0]; a.download = 'z'; blank.contentDocument.body.appendChild(a); a.click(); } catch (e) {}
// Links followed by a click, held every way (WebKit connects to a link's host on its activation): in the page, in no
// document, a span inside one, in a fragment, in a closed shadow root.
try { const a = document.createElement('a'); a.href = C[0]; document.body.appendChild(a); a.click(); } catch (e) {}
try { const a = document.createElement('a'); a.href = C[1]; a.click(); } catch (e) {}
try { const a = document.createElement('a'); a.href = C[0]; const s = document.createElement('span'); a.appendChild(s); s.click(); s.dispatchEvent(new MouseEvent('click', {bubbles: true})); } catch (e) {}
try { const f = document.createDocumentFragment(); const a = document.createElement('a'); a.href = C[0]; f.appendChild(a); a.click(); } catch (e) {}
try { const h = document.createElement('div'); document.body.appendChild(h); const r = h.attachShadow({mode: 'closed'}); const a = document.createElement('a'); a.href = C[0]; r.appendChild(a); a.click(); } catch (e) {}
try { Function.prototype.call = function () { try { const a = document.createElement('a'); a.href = C[0]; return this.apply(a, []); } catch (e) {} }; const a = document.createElement('a'); a.href = C[0]; a.click(); } catch (e) {}
try { alert('hi'); confirm('ok?'); } catch (e) {}
try { Notification.requestPermission(); } catch (e) {}
try { navigator.mediaDevices.getUserMedia({audio: true, video: true}).catch(() => {}); } catch (e) {}
try { navigator.geolocation.getCurrentPosition(() => {}, () => {}); } catch (e) {}
document.getElementById('out').textContent = 'Rendered by script: the pilot moves to November, three stores first.';
// At last the page itself tries to go to this computer.
setTimeout(() => { location.href = C[0]; }, 100);
</script></body></html>"#;

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_rendered_page_reaches_nothing_on_this_computer_or_the_users_network() {
    let c = canary();
    let lan = lan_ip().unwrap_or_else(|| "192.168.1.1".into());
    // A name of its own for each run, prefetched and preconnected: only this page could have it looked up.
    let dns = format!("echo-canary-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    let page = HOSTILE.replace("{TCP}", &c.tcp.to_string()).replace("{UDP}", &c.udp.to_string()).replace("{LAN}", &lan).replace("{DNS}", &dns);
    let (port, seen) = site(vec![("/hostile", page)]);
    let (out, _) = render(port, "/hostile", 20);
    let v = json(&out);
    assert!(v["html"].as_str().unwrap().contains("Rendered by script: the pilot moves to November"), "{}", v["html"]);
    assert_eq!(v["url"], format!("{SITE}/hostile"), "the page stayed where it was");
    // Give anything late its chance to arrive.
    std::thread::sleep(Duration::from_millis(500));
    assert!(c.hits.lock().unwrap().is_empty(), "the canary was reached: {:?}", c.hits.lock().unwrap());
    // The page's own host on another port: refused by the gate. On Windows a request may be refused first by the request
    // filter or the gate (WebSockets reach only the gate), and their lists are capped: the site, which would get any
    // port of its host in a test, got nothing but the page.
    if cfg!(windows) {
        assert!(seen.lock().unwrap().iter().all(|p| p == "/hostile" || p == "/favicon.ico"), "the site was reached: {:?}", seen.lock().unwrap());
    } else {
        let refused = v["stats"]["refused"].to_string();
        assert!(refused.contains(&format!("news.acme.example:{}", c.tcp)), "the gate refused the page's own host on another port: {refused}");
    }
}

const STORE: &str = r#"<!doctype html><body><p id="out"></p><script>
let idb = 'none';
const seen = 'cookie=[' + document.cookie + '] stored=[' + (localStorage.getItem('seen') || '') + '] session=[' + (sessionStorage.getItem('seen') || '') + ']';
document.cookie = 'seen=1; max-age=86400'; localStorage.setItem('seen', '1'); sessionStorage.setItem('seen', '1');
document.getElementById('out').textContent = seen;
</script></body>"#;

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn nothing_a_page_stores_outlives_its_render() {
    let (port, _) = site(vec![("/store", STORE.to_string())]);
    for n in 1..=2 {
        let (out, _, pid) = render_pid(port, "/store", 15);
        let v = json(&out);
        assert!(v["html"].as_str().unwrap().contains("cookie=[] stored=[] session=[]"), "render {n}: {}", v["html"]);
        // Windows: the render's profile folder is gone with it.
        if cfg!(windows) {
            let dir = std::env::temp_dir().join(format!("echo-render-{pid}"));
            assert!(!dir.exists(), "{} is left", dir.display());
        }
    }
}

/// WebView2's processes of the render `pid` (their command lines name its profile folder).
#[cfg(windows)]
fn webview_processes(pid: u32) -> usize {
    let q = format!("@(Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | Where-Object {{ $_.CommandLine -like '*echo-render-{pid}*' }}).Count");
    let out = Command::new("powershell").args(["-NoProfile", "-Command", &q]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[cfg(windows)]
#[test]
#[ignore = "needs a GUI session (WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn the_time_cap_ends_every_process_of_the_render_and_its_profile() {
    let (port, _) = site(vec![("/spin", "<!doctype html><body><p>spinning</p><script>for (;;) {}</script></body>".to_string())]);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_echoctl"));
    cmd.args(["render-page", "--url", &format!("{SITE}/spin"), "--secs", "6"]).env("ECHOCTL_TEST_HOSTS", format!("news.acme.example=http://127.0.0.1:{port}"));
    let mut child = cmd.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    let pid = child.id();
    let dir = std::env::temp_dir().join(format!("echo-render-{pid}"));
    std::thread::sleep(Duration::from_secs(3));
    let during = webview_processes(pid);
    assert!(during > 0 && dir.exists(), "WebView2 runs in the render's own profile: {during} processes, {} exists: {}", dir.display(), dir.exists());
    let started = Instant::now();
    let status = child.wait().unwrap();
    assert!(!status.success());
    assert!(started.elapsed() < Duration::from_secs(8), "{:?}", started.elapsed());
    assert_eq!(webview_processes(pid), 0, "every WebView2 process of the render ended with it");
    assert!(!dir.exists(), "{} is left", dir.display());
}

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_that_never_stops_is_ended_at_its_time() {
    let (port, _) = site(vec![("/spin", "<!doctype html><body><p>spinning</p><script>for (;;) {}</script></body>".to_string())]);
    let (out, took, _pid) = render_pid(port, "/spin", 4);
    assert!(!out.status.success());
    assert!(took < Duration::from_secs(10), "{took:?}");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("didn't render"), "{said}");
    // Windows: every WebView2 process of the render, its renderer in the page's loop too, ended with it.
    #[cfg(windows)]
    assert_eq!(webview_processes(_pid), 0, "WebView2 processes of the render left");
    // Its WebContent process, spinning in the page's loop, is stopped with it.
    #[cfg(not(windows))]
    {
        let pid = said.lines().find_map(|l| l.strip_prefix("web process ")?.split(' ').next()?.parse::<u32>().ok()).expect("the web process named");
        let alive = || Command::new("kill").args(["-0", &pid.to_string()]).output().unwrap().status.success();
        let at = Instant::now();
        while alive() && at.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(!alive(), "web process {pid} still running");
    }
}

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_of_too_many_elements_is_refused() {
    let big = "<!doctype html><body><script>const f = document.createDocumentFragment(); for (let i = 0; i < 300000; i++) f.appendChild(document.createElement('i')); document.body.appendChild(f);</script></body>";
    let (port, _) = site(vec![("/huge", big.to_string())]);
    let (out, _) = render(port, "/huge", 20);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("elements"), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_reaches_only_hosts_the_gate_lets_through() {
    // The page's own host, mapped; any other name (not mapped: refused, as a private one would be) gets nothing.
    let page = "<!doctype html><body><p id=out>x</p><script>fetch('/ok').then(r => r.text()).then(t => { document.getElementById('out').textContent = 'own host: ' + t; }); fetch('https://elsewhere.acme.example/x').catch(() => {});</script></body>";
    let (port, seen) = site(vec![("/page", page.to_string()), ("/ok", "fine".to_string())]);
    let v = json(&render(port, "/page", 15).0);
    assert!(v["html"].as_str().unwrap().contains("own host: fine"), "{}", v["html"]);
    assert!(seen.lock().unwrap().iter().all(|p| p == "/page" || p == "/ok" || p == "/favicon.ico"), "{:?}", seen.lock().unwrap());
    assert!(v["stats"]["refused"].to_string().contains("elsewhere.acme.example"), "{}", v["stats"]);
}

/// A page that first redefines every DOM property and method the link guard could read, then clicks links every way.
const TAMPER: &str = r#"<!doctype html><body><p id="out">x</p><script>
const U = 'http://127.0.0.1:{TCP}/c';
try { Object.defineProperty(Element.prototype, 'localName', {get() { return 'x'; }, configurable: true}); } catch (e) {}
try { Object.defineProperty(Node.prototype, 'parentNode', {get() { return null; }, configurable: true}); } catch (e) {}
try { Object.defineProperty(Node.prototype, 'nodeType', {get() { return 3; }, configurable: true}); } catch (e) {}
try { Event.prototype.preventDefault = function () {}; } catch (e) {}
try { Event.prototype.composedPath = function () { return []; }; } catch (e) {}
try { Object.defineProperty(Event.prototype, 'target', {get() { return null; }, configurable: true}); } catch (e) {}
try { Object.defineProperty(Event.prototype, 'type', {get() { return 'x'; }, configurable: true}); } catch (e) {}
try { Function.prototype.call = function () {}; Function.prototype.apply = function () {}; } catch (e) {}
try { Reflect.apply = function () {}; } catch (e) {}
try { const a = document.createElement('a'); a.href = U; document.body.appendChild(a); a.click(); } catch (e) {}
try { const a = document.createElement('a'); a.href = U; a.click(); } catch (e) {}
try { const a = document.createElement('a'); a.href = U; a.download = 'x'; document.body.appendChild(a); a.dispatchEvent(new MouseEvent('click', {bubbles: true})); } catch (e) {}
try { const a = document.createElement('a'); a.href = U; const s = document.createElement('span'); a.appendChild(s); s.click(); } catch (e) {}
try { const h = document.createElement('div'); document.body.appendChild(h); const r = h.attachShadow({mode: 'closed'}); const a = document.createElement('a'); a.href = U; r.appendChild(a); a.click(); } catch (e) {}
let hooks = [];
try { Object.defineProperty(HTMLElement.prototype, 'click', {value() {}}); hooks.push('click redefined'); } catch (e) { hooks.push('click kept'); }
try { delete EventTarget.prototype.dispatchEvent; hooks.push(typeof EventTarget.prototype.dispatchEvent); } catch (e) { hooks.push('dispatch kept'); }
document.getElementById('out').textContent = 'hooks: ' + hooks.join(', ');
</script></body>"#;

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_that_redefines_the_dom_still_follows_no_link() {
    let c = canary();
    let (port, _) = site(vec![("/tamper", TAMPER.replace("{TCP}", &c.tcp.to_string()))]);
    let v = json(&render(port, "/tamper", 15).0);
    std::thread::sleep(Duration::from_millis(500));
    assert!(c.hits.lock().unwrap().is_empty(), "the canary was reached: {:?}", c.hits.lock().unwrap());
    assert!(v["html"].as_str().unwrap().contains("hooks: click kept, function"), "the guard's own hooks can't be changed or removed: {}", v["html"]);
}

/// A page that plays media from this computer and from its own host, and looks for a worker every way.
const CLOSED: &str = r#"<!doctype html><body><video src="http://news.acme.example/x.m3u8" autoplay muted></video><audio src="http://127.0.0.1:{TCP}/a.mp3" autoplay></audio>
<video autoplay muted><source src="http://127.0.0.1:{TCP}/v.mp4" type="video/mp4"><source src="http://news.acme.example/v.mp4" type="video/mp4"></video>
<p id="out">x</p><script>
const out = [];
const v = document.createElement('video'); v.muted = true; v.src = 'http://news.acme.example/y.mp4'; document.body.appendChild(v); try { v.load(); v.play().catch(() => {}); } catch (e) {}
const a = new Audio('http://127.0.0.1:{TCP}/b.mp3'); try { a.play().catch(() => {}); } catch (e) {}
function kinds(w) { return [typeof w.Worker, typeof w.SharedWorker, typeof (w.navigator && w.navigator.serviceWorker), typeof w.WebTransport, typeof w.RTCDataChannel].join('/'); }
out.push('main ' + kinds(window));
try { new Worker(URL.createObjectURL(new Blob(['fetch("http://127.0.0.1:{TCP}/w")']))); out.push('worker STARTED'); } catch (e) {}
try { window.Worker = function () {}; out.push('put back ' + typeof window.Worker); } catch (e) { out.push('put back refused'); }
const f = document.createElement('iframe'); document.body.appendChild(f); out.push('fresh ' + kinds(f.contentWindow));
try { new f.contentWindow.Worker(URL.createObjectURL(new Blob(['1']))); out.push('frame worker STARTED'); } catch (e) {}
for (const u of ['https://127.0.0.1:{UDP}/', 'https://news.acme.example:{UDP}/']) { try { new WebTransport(u); out.push('webtransport STARTED'); } catch (e) {} }
fetch('https://127.0.0.1:{UDP}/h3', {mode: 'no-cors'}).catch(() => {});
const b = document.createElement('iframe'); b.src = URL.createObjectURL(new Blob(['<p>b</p>'], {type: 'text/html'}));
b.onload = () => { out.push('blob ' + kinds(b.contentWindow)); document.getElementById('out').textContent = out.join('; '); };
document.body.appendChild(b);
document.getElementById('out').textContent = out.join('; ');
</script></body>"#;

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_rendered_page_plays_no_media_and_starts_no_worker() {
    let c = canary();
    let page = CLOSED.replace("{TCP}", &c.tcp.to_string()).replace("{UDP}", &c.udp.to_string());
    let (port, seen) = site(vec![("/closed", page)]);
    let v = json(&render(port, "/closed", 15).0);
    // What the page reported (its script's own text is in the DOM too).
    let all = v["html"].as_str().unwrap();
    let html = all.split(r#"<p id="out">"#).nth(1).and_then(|r| r.split("</p>").next()).unwrap_or_default();
    let none = "undefined/undefined/undefined/undefined/undefined";
    for want in [format!("main {none}"), format!("fresh {none}"), "put back undefined".to_string()] {
        assert!(html.contains(&want), "{want}: {html}");
    }
    // A blob: frame isn't loaded at all (the main frame's navigations are http(s) and about: only); were it, it has none.
    assert!(!html.contains("blob ") || html.contains(&format!("blob {none}")), "{html}");
    assert!(!html.contains("STARTED"), "{html}");
    std::thread::sleep(Duration::from_millis(500));
    assert!(c.hits.lock().unwrap().is_empty(), "the canary (TCP and UDP) was reached: {:?}", c.hits.lock().unwrap());
    let asked = seen.lock().unwrap().clone();
    assert!(!asked.iter().any(|p| p.contains(".m3u8") || p.contains(".mp4") || p.contains(".mp3")), "media was fetched: {asked:?}");
}

/// Links held where the window's listener can't see them (in no document, a fragment, a closed shadow root), and one
/// in the page and an SVG one, each sent every event that could activate a link but a click.
const EVENTS: &str = r#"<!doctype html><body><p id="out">x</p><script>
const U = 'http://127.0.0.1:{TCP}/c';
function links() {
  const out = [];
  const a = document.createElement('a'); a.href = U; out.push(a);
  const f = document.createDocumentFragment(); const b = document.createElement('a'); b.href = U; f.appendChild(b); out.push(b);
  const h = document.createElement('div'); document.body.appendChild(h); const r = h.attachShadow({mode: 'closed'}); const c = document.createElement('a'); c.href = U; r.appendChild(c); out.push(c);
  const d = document.createElement('a'); d.href = U; document.body.appendChild(d); out.push(d);
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg'); const e = document.createElementNS('http://www.w3.org/2000/svg', 'a'); e.setAttribute('href', U); svg.appendChild(e); out.push(e);
  const s = document.createElement('span'); const g = document.createElement('a'); g.href = U; g.appendChild(s); out.push(s);
  return out;
}
const events = [
  () => new KeyboardEvent('keydown', {key: 'Enter', code: 'Enter', keyCode: 13, which: 13, bubbles: true, cancelable: true}),
  () => new KeyboardEvent('keypress', {key: 'Enter', code: 'Enter', keyCode: 13, charCode: 13, which: 13, bubbles: true, cancelable: true}),
  () => new KeyboardEvent('keyup', {key: ' ', code: 'Space', keyCode: 32, which: 32, bubbles: true, cancelable: true}),
  () => new MouseEvent('mousedown', {bubbles: true, cancelable: true}),
  () => new MouseEvent('mouseup', {bubbles: true, cancelable: true}),
  () => new UIEvent('DOMActivate', {bubbles: true, cancelable: true}),
  () => new PointerEvent('pointerup', {bubbles: true, cancelable: true}),
  () => new Event('auxclick', {bubbles: true}),
];
let sent = 0;
for (const make of events) { for (const l of links()) { try { l.focus && l.focus(); l.dispatchEvent(make()); sent++; } catch (e) {} } }
document.getElementById('out').textContent = 'sent ' + sent;
</script></body>"#;

#[test]
#[ignore = "needs a GUI session (WKWebView or WebView2): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_link_is_followed_by_no_event_at_all() {
    let c = canary();
    let (port, _) = site(vec![("/events", EVENTS.replace("{TCP}", &c.tcp.to_string()))]);
    let v = json(&render(port, "/events", 15).0);
    std::thread::sleep(Duration::from_millis(500));
    assert!(c.hits.lock().unwrap().is_empty(), "the canary was reached: {:?}", c.hits.lock().unwrap());
    assert!(v["html"].as_str().unwrap().contains("<p id=\"out\">sent "), "{}", v["html"]);
}

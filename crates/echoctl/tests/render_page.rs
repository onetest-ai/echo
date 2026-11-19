//! `echoctl render-page` (#835) on this computer's own webview. It needs a GUI session, so the tests are ignored by
//! default and run on a Mac: `cargo test --test render_page -- --ignored --test-threads=1`.
//!
//! - **The site:** every page comes from a made-up site on 127.0.0.1, reached through the gate as
//!   `news.acme.example` (`ECHOCTL_TEST_HOSTS`, development builds only). Nothing reaches the real internet.
//! - **The canary:** TCP on every interface, UDP on loopback. It records every connection a page manages to make to
//!   this computer or the user's network; there must be none.
#![cfg(all(target_os = "macos", debug_assertions))]

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

/// `echoctl render-page` on `path` of the site, within `secs`: its output and how long it took. Its environment is
/// only what a render gets.
fn render(site: u16, path: &str, secs: u64) -> (Output, Duration) {
    let at = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_echoctl"))
        .args(["render-page", "--url", &format!("http://news.acme.example{path}"), "--secs", &secs.to_string()])
        .env_clear()
        .env("HOME", std::env::var_os("HOME").unwrap_or_default())
        .env("TMPDIR", std::env::temp_dir())
        .env("ECHOCTL_TEST_HOSTS", format!("news.acme.example=http://127.0.0.1:{site}"))
        .output()
        .unwrap();
    (out, at.elapsed())
}

fn json(out: &Output) -> serde_json::Value {
    assert!(out.status.success(), "render failed: {}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap()
}

/// A page that writes its text with JavaScript and, from every frame it can make, tries every way to this computer and
/// the user's network.
const HOSTILE: &str = r#"<!doctype html><html><head><title>Pilot notes</title></head><body><main><p id="out">loading</p></main><script>
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
#[ignore = "needs a GUI session (WKWebView): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_rendered_page_reaches_nothing_on_this_computer_or_the_users_network() {
    let c = canary();
    let lan = lan_ip().unwrap_or_else(|| "192.168.1.1".into());
    let page = HOSTILE.replace("{TCP}", &c.tcp.to_string()).replace("{UDP}", &c.udp.to_string()).replace("{LAN}", &lan);
    let (port, _) = site(vec![("/hostile", page)]);
    let (out, _) = render(port, "/hostile", 20);
    let v = json(&out);
    assert!(v["html"].as_str().unwrap().contains("Rendered by script: the pilot moves to November"), "{}", v["html"]);
    assert_eq!(v["url"], "http://news.acme.example/hostile", "the page stayed where it was");
    // Give anything late its chance to arrive.
    std::thread::sleep(Duration::from_millis(500));
    assert!(c.hits.lock().unwrap().is_empty(), "the canary was reached: {:?}", c.hits.lock().unwrap());
    let refused = v["stats"]["refused"].to_string();
    assert!(refused.contains(&format!("news.acme.example:{}", c.tcp)), "the gate refused the page's own host on another port: {refused}");
}

const STORE: &str = r#"<!doctype html><body><p id="out"></p><script>
let idb = 'none';
const seen = 'cookie=[' + document.cookie + '] stored=[' + (localStorage.getItem('seen') || '') + '] session=[' + (sessionStorage.getItem('seen') || '') + ']';
document.cookie = 'seen=1; max-age=86400'; localStorage.setItem('seen', '1'); sessionStorage.setItem('seen', '1');
document.getElementById('out').textContent = seen;
</script></body>"#;

#[test]
#[ignore = "needs a GUI session (WKWebView): cargo test --test render_page -- --ignored --test-threads=1"]
fn nothing_a_page_stores_outlives_its_render() {
    let (port, _) = site(vec![("/store", STORE.to_string())]);
    for n in 1..=2 {
        let v = json(&render(port, "/store", 15).0);
        assert!(v["html"].as_str().unwrap().contains("cookie=[] stored=[] session=[]"), "render {n}: {}", v["html"]);
    }
}

#[test]
#[ignore = "needs a GUI session (WKWebView): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_that_never_stops_is_ended_at_its_time() {
    let (port, _) = site(vec![("/spin", "<!doctype html><body><p>spinning</p><script>for (;;) {}</script></body>".to_string())]);
    let (out, took) = render(port, "/spin", 4);
    assert!(!out.status.success());
    assert!(took < Duration::from_secs(10), "{took:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("didn't render"), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "needs a GUI session (WKWebView): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_of_too_many_elements_is_refused() {
    let big = "<!doctype html><body><script>const f = document.createDocumentFragment(); for (let i = 0; i < 300000; i++) f.appendChild(document.createElement('i')); document.body.appendChild(f);</script></body>";
    let (port, _) = site(vec![("/huge", big.to_string())]);
    let (out, _) = render(port, "/huge", 20);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("elements"), "{}", String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "needs a GUI session (WKWebView): cargo test --test render_page -- --ignored --test-threads=1"]
fn a_page_reaches_only_hosts_the_gate_lets_through() {
    // The page's own host, mapped; any other name (not mapped: refused, as a private one would be) gets nothing.
    let page = "<!doctype html><body><p id=out>x</p><script>fetch('/ok').then(r => r.text()).then(t => { document.getElementById('out').textContent = 'own host: ' + t; }); fetch('https://elsewhere.acme.example/x').catch(() => {});</script></body>";
    let (port, seen) = site(vec![("/page", page.to_string()), ("/ok", "fine".to_string())]);
    let v = json(&render(port, "/page", 15).0);
    assert!(v["html"].as_str().unwrap().contains("own host: fine"), "{}", v["html"]);
    assert!(seen.lock().unwrap().iter().all(|p| p == "/page" || p == "/ok" || p == "/favicon.ico"), "{:?}", seen.lock().unwrap());
    assert!(v["stats"]["refused"].to_string().contains("elsewhere.acme.example"), "{}", v["stats"]);
}

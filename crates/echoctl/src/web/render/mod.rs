//! `echoctl render-page --url <u> --secs <n>`: a web page rendered for a plugin (`ctx.web.read({render: true})`, #835)
//! in this short-lived child process, apart from Echo's window, its IPC and the plugin's `ctx`.
//!
//! The page loads in a hidden system webview whose every connection goes through the gate (`web::gate`): only public
//! addresses, checked and pinned. Its store is new and kept in memory, so nothing it stores outlives the render. It may
//! open no download, popup, permission or file. Once it has settled, its DOM is printed as one JSON line on stdout,
//! `{html, url, truncated, elements, stats}`; or why not on stderr, with exit 1. The child stops itself at `secs`, and
//! the host kills its process group soon after (`jshost::render`).
//!
//! Per OS: macOS (WKWebView) in `macos.rs`; Windows (WebView2) not yet.

#[cfg(target_os = "macos")]
mod macos;

use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::gate::{self, Gate, Net};

/// The longest render, and how long after the page's `load` it may keep loading (with no new connection for `QUIET`,
/// it has settled).
pub const MOST_SECS: u64 = 30;
const SETTLE_MOST: Duration = Duration::from_secs(15);
const QUIET: Duration = Duration::from_secs(1);
/// The DOM read back: elements at most (counted before it is serialized), and HTML in bytes.
const MAX_ELEMENTS: u64 = 250_000;
const MAX_HTML: usize = 5 << 20;
/// Main-frame navigations after the first (a page that moves itself on).
const MAX_MOVES: usize = 5;

/// The page's elements counted, then its DOM read back (a page that replaced its own `JSON` only fails its render).
const SNAPSHOT_JS: &str = "(function(){var n=document.getElementsByTagName('*').length;if(n>250000)return JSON.stringify({elements:n});var d=document.documentElement;return JSON.stringify({elements:n,html:d?d.outerHTML:''});})()";

/// Run at document start in every frame (about:blank and srcdoc ones too), before any of the page's code:
/// - no WebRTC, WebTransport, notifications, workers (dedicated, shared, service): each deleted, then set to nothing,
///   unwritable and unconfigurable, so a page can't put one back or find one in another frame (each frame runs this);
///   on macOS the webview's own setting switches WebRTC off as well (`macos::configuration`);
/// - a link (`a`, `area`) is never followed by a click: on a link's activation WebKit connects to its host before the
///   navigation is decided, outside the rule list (a loopback canary saw it, for download links and for links in no
///   document, a fragment or a closed shadow root; and for an `auxclick` event sent to one). Nobody clicks or types
///   in a hidden render, so a link gets no event from the page at all: `dispatchEvent` of any event to an element in
///   a link (`a`, `area`, SVG `a`) however it is held is refused, as is `click()`; and on the window, capturing, first
///   of all listeners, every `click`, `auxclick` and `DOMActivate` on a link is cancelled.
///
/// Every getter and method the guard uses is taken here, before the page runs, and called through `Reflect.apply`
/// taken here too: a page that redefines `localName`, `parentNode`, `preventDefault`, `composedPath`, `target` or
/// `Function.prototype.call` changes nothing it reads. Its `click` and `dispatchEvent` are unwritable and
/// unconfigurable.
const INIT_JS: &str = concat!(
    "(function(){var W=window,O=Object,R=Reflect.apply,G=O.getOwnPropertyDescriptor,P=O.defineProperty;",
    "function off(o,k){try{delete o[k]}catch(e){}try{P(o,k,{value:undefined,writable:false,configurable:false})}catch(e){}}",
    "var gone=['RTCPeerConnection','webkitRTCPeerConnection','RTCDataChannel','RTCIceCandidate','RTCSessionDescription','WebTransport','Notification','Worker','SharedWorker','ServiceWorker','ServiceWorkerContainer','ServiceWorkerRegistration'];",
    "for(var i=0;i<gone.length;i++)off(W,gone[i]);try{off(Navigator.prototype,'serviceWorker')}catch(e){}",
    "var EP=Element.prototype,NP=Node.prototype,VP=Event.prototype,TP=EventTarget.prototype,HP=HTMLElement.prototype;",
    "var gName=G(EP,'localName').get,gKind=G(NP,'nodeType').get,gUp=G(NP,'parentNode').get,gHost=G(ShadowRoot.prototype,'host').get,",
    "mPath=G(VP,'composedPath').value,mStop=G(VP,'preventDefault').value,gTarget=G(VP,'target').get,",
    "mListen=G(TP,'addEventListener').value,mClick=G(HP,'click').value,mDispatch=G(TP,'dispatchEvent').value;",
    "function nm(n){try{return R(gKind,n,[])===1?R(gName,n,[]):''}catch(e){return ''}}",
    "function up(n){var p=null;try{p=R(gUp,n,[])}catch(e){}if(p)return p;try{return R(gHost,n,[])}catch(e){return null}}",
    "function link(n){for(var k=0;n&&k<4096;k++){var g=nm(n);if(g==='a'||g==='area')return true;n=up(n)}return false}",
    "function guard(e){var p=[];try{p=R(mPath,e,[])}catch(x){}for(var k=0;k<p.length;k++){if(link(p[k])){R(mStop,e,[]);return}}",
    "var t=null;try{t=R(gTarget,e,[])}catch(x){}if(link(t))R(mStop,e,[])}",
    "var on=['click','auxclick','DOMActivate'];for(var k=0;k<on.length;k++)R(mListen,W,[on[k],guard,true]);",
    "P(HP,'click',{value:function(){if(!link(this))R(mClick,this,[])},writable:false,configurable:false});",
    "P(TP,'dispatchEvent',{value:function(e){if(link(this))return false;return R(mDispatch,this,[e])},writable:false,configurable:false});",
    "})();"
);

/// What the render needs on the way: its gate, its time.
pub(crate) struct Render {
    pub url: String,
    pub until: Instant,
    pub gate: Gate,
}

/// `echoctl render-page`: the exit code.
pub fn child_main(url: &str, secs: u64) -> i32 {
    match start(url, secs).and_then(run) {
        Ok(()) => 0,
        Err(e) => fail(&format!("{e:#}")),
    }
}

/// Why the render failed, on stderr (its first line is what the host says): exit 1.
pub(crate) fn fail(why: &str) -> i32 {
    eprintln!("{why}");
    1
}

fn start(url: &str, secs: u64) -> Result<Render> {
    let lower = url.get(..8).unwrap_or("").to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) || url.len() > 2048 {
        bail!("a page is rendered from an http(s) address of 2 KB at most");
    }
    let until = Instant::now() + Duration::from_secs(secs.clamp(1, MOST_SECS));
    let gate = Gate::start(net()?, gate::CAPS, until)?;
    Ok(Render { url: url.to_string(), until, gate })
}

#[cfg(target_os = "macos")]
fn run(r: Render) -> Result<()> {
    macos::run(r)
}

#[cfg(not(target_os = "macos"))]
fn run(_r: Render) -> Result<()> {
    // Windows (WebView2) is the next step of #835: its browser flags, a temporary InPrivate profile, the request filter
    // and a Job Object (design § Request interception per OS).
    bail!("rendering a page isn't on Windows yet")
}

/// The line printed when the page settled: its HTML (cut to `MAX_HTML`), the address it ended at, and the gate's stats.
pub(crate) fn answer(snapshot: &str, final_url: &str, gate: &Gate) -> Result<Value> {
    let inner: String = serde_json::from_str(snapshot).map_err(|_| anyhow::anyhow!("the page's DOM couldn't be read back"))?;
    let v: Value = serde_json::from_str(&inner).map_err(|_| anyhow::anyhow!("the page's DOM couldn't be read back"))?;
    let elements = v["elements"].as_u64().unwrap_or(0);
    if elements > MAX_ELEMENTS {
        bail!("the page has {elements} elements: Echo renders pages of {MAX_ELEMENTS} at most");
    }
    let mut html = v["html"].as_str().unwrap_or("").to_string();
    let truncated = html.len() > MAX_HTML;
    if truncated {
        let mut cut = MAX_HTML;
        while !html.is_char_boundary(cut) {
            cut -= 1;
        }
        html.truncate(cut);
    }
    Ok(json!({"html": html, "url": final_url, "truncated": truncated, "elements": elements, "stats": gate.stats()}))
}

/// The gate's way out: the system's, but in a development build's tests (`ECHOCTL_TEST_HOSTS`) a made-up internet
/// where each listed name is at a made-up public address that leads to its local fake service, and nothing else is
/// reached at all.
fn net() -> Result<Net> {
    #[cfg(debug_assertions)]
    if let Some(map) = std::env::var_os("ECHOCTL_TEST_HOSTS") {
        return test_net(map.to_str().ok_or_else(|| anyhow::anyhow!("ECHOCTL_TEST_HOSTS isn't text"))?);
    }
    Ok(Net::system())
}

/// `<host>=http://127.0.0.1:<port>,…`: each host at 93.184.215.<n> (its n-th), which is its port on this computer, on
/// port 80 too (the fakes speak plain http).
#[cfg(debug_assertions)]
fn test_net(map: &str) -> Result<Net> {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
    use std::sync::Arc;
    let mut hosts: Vec<(String, u16)> = vec![];
    for entry in map.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (host, base) = entry.split_once('=').ok_or_else(|| anyhow::anyhow!("ECHOCTL_TEST_HOSTS: “{entry}” isn't <host>=http://127.0.0.1:<port>"))?;
        let port = base.trim().strip_prefix("http://127.0.0.1:").and_then(|p| p.trim_end_matches('/').parse().ok()).ok_or_else(|| anyhow::anyhow!("ECHOCTL_TEST_HOSTS: {base} isn't http://127.0.0.1:<port>"))?;
        hosts.push((host.trim().to_ascii_lowercase(), port));
    }
    let fake = |n: usize| IpAddr::V4(Ipv4Addr::new(93, 184, 215, n as u8 + 1));
    let names = hosts.clone();
    Ok(Net {
        own: Arc::new(|| Ok(Vec::new())),
        // The fakes speak plain http: the page is asked for that way (`jshost::web::rendered`).
        ports: &[80, 443],
        lookup: Arc::new(move |host, _| names.iter().position(|(h, _)| h == host).map(|n| vec![fake(n)]).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "not mapped in ECHOCTL_TEST_HOSTS"))),
        connect: Arc::new(move |at: SocketAddr, wait| match (0..hosts.len()).find(|n| fake(*n) == at.ip()) {
            Some(n) => TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], hosts[n].1)), wait),
            None => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "a test never reaches the real internet")),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_http_address_is_rendered() {
        for url in ["file:///etc/hosts", "javascript:alert(1)", "about:blank", "ftp://acme.example/", ""] {
            assert!(start(url, 5).is_err(), "{url}");
        }
        assert!(start(&format!("https://acme.example/{}", "a".repeat(2048)), 5).is_err());
    }

    #[test]
    fn a_huge_dom_is_refused_and_long_html_cut() {
        let g = Gate::start(Net::system(), gate::CAPS, Instant::now() + Duration::from_secs(1)).unwrap();
        let snap = |v: Value| serde_json::to_string(&v.to_string()).unwrap();
        assert!(answer(&snap(json!({"elements": 300_000})), "https://acme.example/", &g).unwrap_err().to_string().contains("300000 elements"));
        let long = "é".repeat(MAX_HTML / 2 + 10);
        let v = answer(&snap(json!({"elements": 3, "html": long})), "https://acme.example/", &g).unwrap();
        assert_eq!(v["truncated"], true);
        assert!(v["html"].as_str().unwrap().len() <= MAX_HTML);
        assert!(answer("\"not json\"", "https://acme.example/", &g).is_err());
    }
}

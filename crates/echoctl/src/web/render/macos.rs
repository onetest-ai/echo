//! macOS: the page in a hidden WKWebView (no Dock icon, no window shown).
//!
//! - **Store:** a non-persistent data store.
//! - **Proxy:** the gate is the store's only proxy (`proxyConfigurations`), with the render's token and failover to a
//!   direct connection off.
//! - **Rule list:** Network.framework sends this computer's own addresses (loopback, its interfaces' addresses)
//!   around any proxy. So a content rule list blocks every address written as one that is on this computer or the
//!   user's network, `localhost`, `.local` and single-label names, and `file:`.
//! - **WebRTC, workers:** off in every frame, by WebKit's own setting and by the script run at document start.
//! - **Media:** none loaded at all (the rule list).
//! - **Denied:** downloads (cancelled before they start), popups and permissions.
//! - **Navigation:** the main frame goes only to http(s) (and about:) addresses, `MAX_MOVES` times at most.

use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::net::Ipv4Addr;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_foundation::{ns_string, NSArray, NSError, NSNumber, NSObject, NSObjectNSKeyValueCoding, NSString, NSURL};
use objc2_web_kit::{WKContentRuleList, WKContentRuleListStore, WKWebViewConfiguration, WKWebsiteDataStore};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tao::window::{Window, WindowBuilder};
use wry::{BackgroundThrottlingPolicy, NewWindowResponse, PageLoadEvent, PermissionResponse, WebView, WebViewBuilder, WebViewBuilderExtMacos};

use super::{answer, fail, Render, INIT_JS, MAX_MOVES, QUIET, SETTLE_MOST, SNAPSHOT_JS};
use crate::web::gate::{self, Gate};

#[link(name = "Network", kind = "framework")]
extern "C" {
    fn nw_endpoint_create_host(host: *const c_char, port: *const c_char) -> *mut NSObject;
    fn nw_proxy_config_create_http_connect(endpoint: *mut NSObject, tls_options: *mut NSObject) -> *mut NSObject;
    fn nw_proxy_config_set_username_and_password(config: *mut NSObject, user: *const c_char, password: *const c_char);
    fn nw_proxy_config_set_failover_allowed(config: *mut NSObject, allowed: bool);
}

enum Msg {
    /// The main frame committed a navigation.
    Started,
    /// The main frame finished loading.
    Loaded,
    /// The page's DOM, as `SNAPSHOT_JS` returned it.
    Dom(String),
}

type Compiled = Rc<RefCell<Option<std::result::Result<Retained<WKContentRuleList>, String>>>>;

/// The page's WebContent process, once known: stopped with the render (`stop`), never left running a page's loop.
static WEB_PID: AtomicI32 = AtomicI32::new(0);

pub(super) fn run(r: Render) -> Result<()> {
    let mtm = MainThreadMarker::new().ok_or_else(|| anyhow!("a page is rendered on the main thread"))?;
    sweep(&std::env::temp_dir());
    let rules_dir = std::env::temp_dir().join(format!("{RULES_DIR}{}", std::process::id()));
    let mut event_loop = EventLoopBuilder::<Msg>::with_user_event().build();
    event_loop.set_activation_policy(ActivationPolicy::Prohibited);
    let proxy = event_loop.create_proxy();
    let compiled: Compiled = Rc::default();
    let mut rule_store: Option<Retained<WKContentRuleListStore>> = None;
    let mut waiting: Option<(Window, Retained<WKWebViewConfiguration>)> = None;
    let mut view: Option<(WebView, Window)> = None;
    let (mut moves, mut committed, mut loaded_at, mut asked) = (0usize, false, None::<Instant>, false);
    // The DOM is read back by this time even if the page never finished loading; the render ends at `until`.
    let read_by = r.until - Duration::from_secs(2);
    event_loop.run(move |event, target, flow| {
        *flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));
        match event {
            Event::NewEvents(StartCause::Init) => {
                let ready = WindowBuilder::new().with_visible(false).build(target).map_err(anyhow::Error::from).and_then(|w| {
                    let config = configuration(mtm, &r.gate)?;
                    rule_store = Some(compile_rules(mtm, &rules_dir, compiled.clone())?);
                    Ok((w, config))
                });
                match ready {
                    Ok(w) => waiting = Some(w),
                    Err(e) => stop(&rules_dir, fail(&format!("{e:#}"))),
                }
            }
            Event::UserEvent(Msg::Started) => {
                if let Some((v, _)) = &view {
                    WEB_PID.store(web_process(v), Ordering::SeqCst);
                }
                committed = true;
                moves += 1;
                if moves > MAX_MOVES + 1 {
                    stop(&rules_dir, fail(&format!("the page moved more than {MAX_MOVES} times")));
                }
            }
            Event::UserEvent(Msg::Loaded) => {
                loaded_at.get_or_insert_with(Instant::now);
            }
            Event::UserEvent(Msg::Dom(dom)) => {
                let at = view.as_ref().and_then(|(v, _)| v.url().ok()).unwrap_or_default();
                let at = if at.starts_with("http://") || at.starts_with("https://") { at } else { r.url.clone() };
                match answer(&dom, &at, &r.gate) {
                    Ok(v) => {
                        println!("{v}");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        stop(&rules_dir, 0)
                    }
                    Err(e) => stop(&rules_dir, fail(&format!("{e:#}"))),
                }
            }
            _ => {}
        }
        if let Some(done) = compiled.borrow_mut().take() {
            let Some((window, config)) = waiting.take() else { return };
            let list = match done {
                Ok(list) => list,
                Err(e) => stop(&rules_dir, fail(&format!("the render's rules couldn't be set: {e}"))),
            };
            unsafe { config.userContentController().addContentRuleList(&list) };
            match build(&r, config, &window, proxy.clone()) {
                Ok(v) => view = Some((v, window)),
                Err(e) => stop(&rules_dir, fail(&format!("the page's view couldn't be made: {e:#}"))),
            }
        }
        let now = Instant::now();
        let settled = loaded_at.is_some_and(|t| (now - t >= QUIET && r.gate.last_connection().elapsed() >= QUIET) || now - t >= SETTLE_MOST);
        if !asked && (settled || (committed && now >= read_by)) {
            if let Some((v, _)) = &view {
                asked = true;
                let p = proxy.clone();
                if v.evaluate_script_with_callback(SNAPSHOT_JS, move |dom| drop(p.send_event(Msg::Dom(dom)))).is_err() {
                    stop(&rules_dir, fail("the page's DOM couldn't be read back"));
                }
            }
        }
        if now >= r.until {
            stop(&rules_dir, fail(&format!("the page didn't render within {} s", super::MOST_SECS.min((r.until - now).as_secs().max(1)))));
        }
    })
}

/// The render's end: its rule list's folder removed, the page's WebContent process stopped (WebKit would end it with
/// this process, but a page in a loop is not left to that), this process gone. On a failure the WebContent process is
/// named on stderr's second line (the tests check it is gone).
fn stop(rules_dir: &Path, code: i32) -> ! {
    let _ = std::fs::remove_dir_all(rules_dir);
    let pid = WEB_PID.load(Ordering::SeqCst);
    if pid > 0 {
        unsafe { libc::kill(pid, libc::SIGKILL) };
        if code != 0 {
            eprintln!("web process {pid} stopped");
        }
    }
    std::process::exit(code)
}

/// The view's WebContent process (WKWebView's private `_webProcessIdentifier`), 0 when it can't be told.
fn web_process(view: &WebView) -> i32 {
    use wry::WebViewExtMacOS;
    let wk = view.webview();
    let sel = objc2::sel!(_webProcessIdentifier);
    let ok: bool = unsafe { objc2::msg_send![&*wk, respondsToSelector: sel] };
    if ok {
        unsafe { objc2::msg_send![&*wk, _webProcessIdentifier] }
    } else {
        0
    }
}

/// What a render's rule list folder is called (its process id after it).
const RULES_DIR: &str = "echo-render-";

/// Rule list folders in `tmp` left by renders that were killed (their process is gone): removed.
fn sweep(tmp: &Path) {
    for e in std::fs::read_dir(tmp).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.strip_prefix(RULES_DIR).and_then(|p| p.parse::<i32>().ok()) else { continue };
        let gone = unsafe { libc::kill(pid, 0) } != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        if gone {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

/// The view, loading the page, with every way out denied.
fn build(r: &Render, config: Retained<WKWebViewConfiguration>, window: &Window, proxy: tao::event_loop::EventLoopProxy<Msg>) -> Result<WebView> {
    let p = proxy.clone();
    let view = WebViewBuilder::new()
        .with_webview_configuration(config)
        .with_initialization_script_for_main_only(INIT_JS, false)
        .with_navigation_handler(|to| ["https://", "http://", "about:"].iter().any(|s| to.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s))))
        // No download handler: wry then cancels a download before it starts. With one, WebKit opens the download's
        // connection before the handler can refuse it, outside the rule list (a loopback canary saw it).
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_permission_handler(|_| PermissionResponse::Deny)
        .with_on_page_load_handler(move |e, _| {
            let _ = p.send_event(match e {
                PageLoadEvent::Started => Msg::Started,
                PageLoadEvent::Finished => Msg::Loaded,
            });
        })
        .with_background_throttling(BackgroundThrottlingPolicy::Disabled)
        .with_devtools(false)
        .with_url(&r.url)
        .build(window)?;
    Ok(view)
}

/// The view's configuration: a store of its own in memory, reaching the web only through the gate, and WebRTC off.
fn configuration(mtm: MainThreadMarker, gate: &Gate) -> Result<Retained<WKWebViewConfiguration>> {
    let port = CString::new(gate.port.to_string())?;
    let user = CString::new(gate::USER)?;
    let token = CString::new(gate.token.as_str())?;
    unsafe {
        let config = WKWebViewConfiguration::new(mtm);
        let store = WKWebsiteDataStore::nonPersistentDataStore(mtm);
        let endpoint = nw_endpoint_create_host(c"127.0.0.1".as_ptr(), port.as_ptr());
        if endpoint.is_null() {
            bail!("the render's proxy couldn't be set");
        }
        let proxy = nw_proxy_config_create_http_connect(endpoint, std::ptr::null_mut());
        if proxy.is_null() {
            bail!("the render's proxy couldn't be set");
        }
        nw_proxy_config_set_username_and_password(proxy, user.as_ptr(), token.as_ptr());
        nw_proxy_config_set_failover_allowed(proxy, false);
        let proxies: Retained<NSArray<NSObject>> = NSArray::arrayWithObject(&*proxy);
        store.setValue_forKey(Some(&proxies), ns_string!("proxyConfigurations"));
        config.setWebsiteDataStore(&store);
        // WebRTC off in every frame: WKPreferences' own setting (private: `_setPeerConnectionEnabled:`), read back.
        let prefs = config.preferences();
        let off = NSNumber::numberWithBool(false);
        objc2::exception::catch(std::panic::AssertUnwindSafe(|| prefs.setValue_forKey(Some(&off), ns_string!("peerConnectionEnabled")))).map_err(|_| anyhow!("WebRTC can't be switched off in this macOS's webview"))?;
        let now = objc2::exception::catch(std::panic::AssertUnwindSafe(|| prefs.valueForKey(ns_string!("peerConnectionEnabled")))).ok().flatten();
        if now.and_then(|v| v.downcast::<NSNumber>().ok()).is_none_or(|n| n.boolValue()) {
            bail!("WebRTC can't be switched off in this macOS's webview");
        }
        Ok(config)
    }
}

/// The rule list compiled into a store of its own in `dir` (removed when the render ends); `compiled` gets it.
fn compile_rules(mtm: MainThreadMarker, dir: &Path, compiled: Compiled) -> Result<Retained<WKContentRuleListStore>> {
    std::fs::create_dir_all(dir)?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&dir.to_string_lossy()));
    let store = unsafe { WKContentRuleListStore::storeWithURL(Some(&url), mtm) }.ok_or_else(|| anyhow!("the render's rules couldn't be set"))?;
    let done = block2::RcBlock::new(move |list: *mut WKContentRuleList, err: *mut NSError| {
        let got = unsafe { Retained::retain(list) }.ok_or_else(|| unsafe { err.as_ref() }.map_or_else(|| "no reason given".to_string(), |e| e.localizedDescription().to_string()));
        *compiled.borrow_mut() = Some(got);
    });
    let rules = rules(&own_ipv4()?);
    unsafe { store.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(Some(ns_string!("echo-render")), Some(&NSString::from_str(&rules)), Some(&done)) };
    Ok(store)
}

/// What an address written in a URL may not start with: this computer and the user's own network (loopback,
/// unspecified, private, link-local, shared address space), multicast and reserved ranges. WebKit's rule syntax has no
/// `|`, so one rule each.
const PRIVATE_V4: [&str; 17] = [
    "127\\.", "0\\.", "10\\.", "192\\.168\\.", "169\\.254\\.", "172\\.1[6-9]\\.", "172\\.2[0-9]\\.", "172\\.3[01]\\.", "100\\.6[4-9]\\.", "100\\.[7-9][0-9]\\.", "100\\.1[01][0-9]\\.", "100\\.12[0-7]\\.",
    "22[4-9]\\.", "23[0-9]\\.", "24[0-9]\\.", "25[0-5]\\.", "198\\.1[89]\\.",
];

/// The content rule list (WebKit's JSON): blocked are `file:`, every IPv6 address written as one, the private IPv4
/// ranges above and this computer's own addresses (`own`), `localhost` and its subdomains, `.local` and single-label
/// names; for any load (the page itself, a frame, a fetch, a WebSocket, a picture, a form). And every media load.
fn rules(own: &[Ipv4Addr]) -> String {
    let host = "^[a-z][a-z0-9+.-]*://([^/]*@)?";
    let mut filters = vec!["^file:".to_string(), format!("{host}\\[")];
    filters.extend(PRIVATE_V4.iter().map(|p| format!("{host}{p}[0-9.]*[:/]")));
    filters.extend(own.iter().map(|ip| format!("{host}{}[:/]", ip.to_string().replace('.', "\\."))));
    filters.push(format!("{host}([^/@]*\\.)?localhost\\.?[:/]"));
    filters.push(format!("{host}[^/@]*\\.local\\.?[:/]"));
    filters.push(format!("{host}[^./:@]+\\.?[:/]"));
    let mut list: Vec<serde_json::Value> = filters.into_iter().map(|f| serde_json::json!({"trigger": {"url-filter": f}, "action": {"type": "block"}})).collect();
    // No media at all: a page's text needs none, and AVFoundation may fetch some of it with its own network stack.
    list.push(serde_json::json!({"trigger": {"url-filter": ".*", "resource-type": ["media"]}, "action": {"type": "block"}}));
    serde_json::Value::Array(list).to_string()
}

/// This computer's IPv4 addresses (every interface's); an error when they can't be listed.
fn own_ipv4() -> Result<Vec<Ipv4Addr>> {
    Ok(crate::netif::addresses()?
        .into_iter()
        .filter_map(|a| match a {
            std::net::IpAddr::V4(v4) => Some(v4),
            std::net::IpAddr::V6(_) => None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rules_name_every_private_range_and_this_computers_own_addresses() {
        let v: serde_json::Value = serde_json::from_str(&rules(&["203.0.113.9".parse().unwrap()])).unwrap();
        let filters: Vec<&str> = v.as_array().unwrap().iter().map(|r| {
            assert_eq!(r["action"]["type"], "block");
            r["trigger"]["url-filter"].as_str().unwrap()
        }).collect();
        assert!(filters.iter().all(|f| !f.contains('|')), "WebKit's syntax: no alternation");
        assert!(filters.iter().filter(|f| **f != ".*").all(|f| f.starts_with('^')), "each address rule anchored");
        for want in ["^file:", "@)?\\[", "@)?127\\.", "@)?10\\.", "@)?192\\.168\\.", "@)?169\\.254\\.", "@)?203\\.0\\.113\\.9[:/]", "localhost", "\\.local\\.?"] {
            assert!(filters.iter().any(|f| f.contains(want)), "{want}: {filters:?}");
        }
        assert!(own_ipv4().unwrap().contains(&Ipv4Addr::LOCALHOST));
        assert!(v.as_array().unwrap().iter().any(|r| r["trigger"]["resource-type"] == serde_json::json!(["media"])), "every media load blocked");
    }

    #[test]
    fn a_killed_renders_rule_folder_is_swept_next_time() {
        let tmp = std::env::temp_dir().join(format!("echo-render-sweep-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        // A process id no process has (above macOS's limit), and this one's.
        let (dead, live) = (tmp.join(format!("{RULES_DIR}99999999")), tmp.join(format!("{RULES_DIR}{}", std::process::id())));
        for d in [&dead, &live] {
            std::fs::create_dir_all(d).unwrap();
        }
        sweep(&tmp);
        assert!(!dead.exists() && live.exists());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

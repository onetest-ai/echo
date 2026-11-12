//! GitHub's device sign-in (OAuth 2.0 device authorization grant, RFC 8628), run by Echo for a plugin whose manifest
//! names it (`uses.signin.device.<host>`: the OAuth app's public client id and the scopes — no client secret, there
//! is none). `start` asks `https://<host>/login/device/code` for a code, which the plugin shows with the address
//! `https://<host>/login/device` (Echo's own, never one from the answer); Echo then asks
//! `https://<host>/login/oauth/access_token` every `interval` seconds until the user confirms, refuses or the code
//! runs out, and keeps the sign-in (`plugins::signin::save`, 0600). The device code stays in Echo's memory: the plugin
//! never sees it, and gets the token only through `ctx.signin.get`. One sign-in at a time per host; a new `start`
//! replaces the one waiting. It runs in the process that started it: a terminal's `echoctl plugin call` that ends
//! ends it too.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};

use crate::plugins;

/// One sign-in on its way, or how the last one ended.
struct Flow {
    n: u64,
    code: String,
    url: String,
    until: Instant,
    /// waiting, done, denied, expired or failed
    state: &'static str,
    error: String,
}

fn flows() -> &'static Mutex<HashMap<String, Flow>> {
    static FLOWS: OnceLock<Mutex<HashMap<String, Flow>>> = OnceLock::new();
    FLOWS.get_or_init(Default::default)
}

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Ask `host` for a code: `{code, url, expires_in}`; the sign-in is waited for in the background.
pub(super) fn start(host: &str, client_id: &str, scopes: &str) -> Result<Value> {
    let (status, v) = post(host, "login/device/code", &[("client_id", client_id), ("scope", scopes)])?;
    let device = v["device_code"].as_str().filter(|d| plain(d, 200)).ok_or_else(|| refused(host, status, &v))?;
    let code = v["user_code"].as_str().filter(|c| plain(c, 16)).ok_or_else(|| refused(host, status, &v))?.to_string();
    let url = format!("https://{host}/login/device");
    let expires = v["expires_in"].as_u64().unwrap_or(900).clamp(60, 1800);
    let interval = v["interval"].as_u64().unwrap_or(5).clamp(1, 60);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let until = Instant::now() + Duration::from_secs(expires);
    flows().lock().unwrap_or_else(|e| e.into_inner()).insert(host.into(), Flow { n, code: code.clone(), url: url.clone(), until, state: "waiting", error: String::new() });
    let (host, client_id, scopes, device) = (host.to_string(), client_id.to_string(), scopes.to_string(), device.to_string());
    std::thread::Builder::new().name("signin-device".into()).spawn(move || wait(&host, &client_id, &scopes, &device, interval, n))?;
    Ok(json!({"code": code, "url": url, "expires_in": expires}))
}

/// Ask until the user confirms, refuses or the code runs out — or this sign-in is replaced or cancelled.
fn wait(host: &str, client_id: &str, scopes: &str, device: &str, mut interval: u64, n: u64) {
    let grant = "urn:ietf:params:oauth:grant-type:device_code";
    loop {
        std::thread::sleep(Duration::from_secs(interval));
        let Some(until) = flows().lock().unwrap_or_else(|e| e.into_inner()).get(host).filter(|f| f.n == n && f.state == "waiting").map(|f| f.until) else { return };
        if Instant::now() >= until {
            return end(host, n, "expired", "");
        }
        // Not reached this time (offline a moment): asked again at the next turn, until the code runs out.
        let Ok((status, v)) = post(host, "login/oauth/access_token", &[("client_id", client_id), ("device_code", device), ("grant_type", grant)]) else { continue };
        match v["error"].as_str() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval = v["interval"].as_u64().unwrap_or(interval + 5).clamp(interval + 1, 60),
            Some("expired_token") => return end(host, n, "expired", ""),
            Some("access_denied") => return end(host, n, "denied", ""),
            Some(_) => return end(host, n, "failed", &said(host, status, &v)),
            None => {
                let Some(token) = v["access_token"].as_str() else { return end(host, n, "failed", &said(host, status, &v)) };
                // The scopes GitHub granted (the user may have been given fewer than asked).
                let granted = v["scope"].as_str().filter(|s| plain(s, 400)).map(|s| s.replace(',', " ")).unwrap_or_else(|| scopes.to_string());
                // Kept only if this sign-in is still the one asked for (not cancelled meanwhile).
                let mut all = flows().lock().unwrap_or_else(|e| e.into_inner());
                let Some(f) = all.get_mut(host).filter(|f| f.n == n && f.state == "waiting") else { return };
                match plugins::signin::save(host, token, &granted, client_id) {
                    Ok(()) => f.state = "done",
                    Err(e) => (f.state, f.error) = ("failed", format!("{e:#}")),
                }
                return;
            }
        }
    }
}

fn end(host: &str, n: u64, state: &'static str, error: &str) {
    if let Some(f) = flows().lock().unwrap_or_else(|e| e.into_inner()).get_mut(host).filter(|f| f.n == n && f.state == "waiting") {
        (f.state, f.error) = (state, error.to_string());
    }
}

/// `{state: none | waiting | done | denied | expired | failed, code?, url?, error?, signed_in}`: the code and address
/// only while waiting; `signed_in`: Echo keeps a sign-in of its own to `host`.
pub(super) fn status(host: &str) -> Value {
    let signed_in = plugins::signin::saved(host).is_some();
    let all = flows().lock().unwrap_or_else(|e| e.into_inner());
    let Some(f) = all.get(host) else { return json!({"state": "none", "signed_in": signed_in}) };
    let waiting = f.state == "waiting" && Instant::now() < f.until;
    let mut v = json!({"state": if f.state == "waiting" && !waiting { "expired" } else { f.state }, "signed_in": signed_in});
    if waiting {
        v["code"] = json!(f.code);
        v["url"] = json!(f.url);
        v["expires_in"] = json!(f.until.saturating_duration_since(Instant::now()).as_secs());
    }
    if !f.error.is_empty() {
        v["error"] = json!(f.error);
    }
    v
}

/// Stop waiting for the sign-in asked for (the code shown is then of no use to Echo).
pub(super) fn cancel(host: &str) -> bool {
    flows().lock().unwrap_or_else(|e| e.into_inner()).remove(host).is_some()
}

/// Sign out: stop any sign-in waiting and forget Echo's own one (`{forgot}`: there was one).
pub(super) fn forget(host: &str) -> Result<Value> {
    cancel(host);
    Ok(json!({"forgot": plugins::signin::forget_saved(host)?}))
}

/// A value from the answer: 1 to `max` characters, no space or control character.
fn plain(v: &str, max: usize) -> bool {
    !v.is_empty() && v.len() <= max && v.bytes().all(|b| b.is_ascii_graphic())
}

fn said(host: &str, status: u16, v: &Value) -> String {
    let what = v["error_description"].as_str().or(v["error"].as_str()).or(v["message"].as_str()).unwrap_or("no reason given");
    format!("{host} said {status}: {}", what.chars().filter(|c| !c.is_control()).take(200).collect::<String>())
}

fn refused(host: &str, status: u16, v: &Value) -> anyhow::Error {
    anyhow!("{} (is device sign-in on for this app?)", said(host, status, v))
}

/// POST a form to `https://<host>/<path>` and read its JSON answer: the real host, or in a development build the fake
/// `ECHOCTL_TEST_HOSTS` maps it to (`test_hosts`), or in Echo's tests [`FAKE`].
fn post(host: &str, path: &str, form: &[(&str, &str)]) -> Result<(u16, Value)> {
    let url = format!("https://{host}/{path}");
    #[cfg(test)]
    let fake = FAKE.lock().unwrap_or_else(|e| e.into_inner()).clone().map(|b| format!("{b}/{path}"));
    #[cfg(not(test))]
    let fake = super::test_hosts::test_host(&url, &[host.to_string()])?;
    let mut cfg = super::curl_head(fake.as_deref().unwrap_or(&url), "POST", fake.is_some());
    cfg.push_str(&format!("header = {}\n", super::curl_quote("Accept: application/json")));
    let body: Vec<String> = form.iter().map(|(k, v)| format!("{k}={}", encode(v))).collect();
    cfg.push_str(&format!("data-raw = {}\n", super::curl_quote(&body.join("&"))));
    let (status, text) = super::curl(&cfg, host)?;
    let v = serde_json::from_str(&text).unwrap_or(Value::Null);
    if !v.is_object() {
        bail!("{host} answered {status} with something other than JSON");
    }
    Ok((status, v))
}

/// `application/x-www-form-urlencoded`: what isn't a letter, digit or `-._~` as `%XX`.
fn encode(v: &str) -> String {
    v.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Echo's tests: the fake GitHub's base address (`http://127.0.0.1:<port>`).
#[cfg(test)]
pub(crate) static FAKE: Mutex<Option<String>> = Mutex::new(None);

#[cfg(test)]
/// Echo's tests: a made-up GitHub: a code, then "pending" `pending` times, then `last` (an access token or an error).
pub(crate) fn fake_github(pending: usize, last: Value) -> (String, std::sync::Arc<Mutex<Vec<String>>>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        let mut polls = 0;
        for conn in l.incoming().flatten() {
            use std::io::{BufRead, BufReader, Read, Write};
        let mut r = BufReader::new(&conn);
            let (mut line, mut len, mut h) = (String::new(), 0usize, String::new());
            r.read_line(&mut line).unwrap();
            while r.read_line(&mut h).is_ok_and(|n| n > 2) {
                if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                h.clear();
            }
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
            log.lock().unwrap().push(format!("{path} {}", String::from_utf8_lossy(&body)));
            let answer = if path == "/login/device/code" {
                json!({"device_code": "dev-123", "user_code": "ABCD-1234", "verification_uri": "https://evil.example/", "expires_in": 900, "interval": 1})
            } else if polls < pending {
                polls += 1;
                json!({"error": "authorization_pending"})
            } else {
                last.clone()
            };
            let a = answer.to_string();
            let mut out = &conn;
            let _ = write!(out, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{a}", a.len());
        }
    });
    (base, seen)
}
#[cfg(test)]
mod tests {

    use super::*;

    fn settled(host: &str) -> Value {
        (0..100)
            .find_map(|_| {
                let s = status(host);
                (s["state"] != "waiting").then_some(s).or_else(|| {
                    std::thread::sleep(Duration::from_millis(100));
                    None
                })
            })
            .expect("the sign-in ends")
    }

    #[test]
    fn device_sign_in_keeps_echos_own_sign_in_and_forgets_it() {
        let (_env, root) = crate::paths::testenv::folders("signin-device");
        let host = "device-ok.example";
        let (base, seen) = fake_github(1, json!({"access_token": "gho_madeupdevice", "token_type": "bearer", "scope": "repo,read:org"}));
        *FAKE.lock().unwrap() = Some(base);
        assert_eq!(status(host), json!({"state": "none", "signed_in": false}));
        let started = start(host, "Ov23liMADEUP", "repo read:org").unwrap();
        // The address is Echo's own, never the one in the answer.
        assert_eq!(started, json!({"code": "ABCD-1234", "url": "https://device-ok.example/login/device", "expires_in": 900}));
        assert_eq!(status(host)["code"], "ABCD-1234");
        assert_eq!(settled(host), json!({"state": "done", "signed_in": true}), "no code once done");
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen[0], "/login/device/code client_id=Ov23liMADEUP&scope=repo%20read%3Aorg");
        assert_eq!(seen[1], "/login/oauth/access_token client_id=Ov23liMADEUP&device_code=dev-123&grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code");
        assert_eq!(seen.len(), 3, "pending once, then the token");
        // Echo's own sign-in comes first, from a file only the user may read.
        assert_eq!(plugins::signin::get(host, false, true), Some(json!({"token": "gho_madeupdevice", "from": "echo", "scopes": "repo read:org"})));
        let f = root.join("config/signin").join(format!("{host}.json"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert!(!std::fs::read_to_string(&f).unwrap().contains("dev-123"), "the device code is never kept");
        // A plugin whose sheet didn't name Echo's own sign-in never gets it.
        assert_eq!(plugins::signin::get(host, true, false), None);
        assert_eq!(forget(host).unwrap(), json!({"forgot": true}));
        assert!(!f.exists());
        assert_eq!(plugins::signin::saved(host), None);
        assert_eq!(forget(host).unwrap(), json!({"forgot": false}));
        *FAKE.lock().unwrap() = None;
    }

    #[test]
    fn a_refused_cancelled_or_broken_device_sign_in_keeps_nothing() {
        let (_env, _root) = crate::paths::testenv::folders("signin-device-no");
        let host = "device-no.example";
        // The user refuses on GitHub.
        let (base, _) = fake_github(0, json!({"error": "access_denied"}));
        *FAKE.lock().unwrap() = Some(base);
        start(host, "Ov23liMADEUP", "repo").unwrap();
        assert_eq!(settled(host), json!({"state": "denied", "signed_in": false}));
        // GitHub says something else: its words, without control characters.
        let (base, _) = fake_github(0, json!({"error": "incorrect_client_credentials", "error_description": "The client_id\nis wrong"}));
        *FAKE.lock().unwrap() = Some(base);
        start(host, "Ov23liMADEUP", "repo").unwrap();
        assert_eq!(settled(host), json!({"state": "failed", "signed_in": false, "error": "device-no.example said 200: The client_idis wrong"}));
        // Cancelled while waiting: the token that comes after is not kept.
        let (base, _) = fake_github(2, json!({"access_token": "gho_madeuplate", "scope": "repo"}));
        *FAKE.lock().unwrap() = Some(base);
        start(host, "Ov23liMADEUP", "repo").unwrap();
        assert!(cancel(host));
        std::thread::sleep(Duration::from_millis(3500));
        assert_eq!(status(host), json!({"state": "none", "signed_in": false}));
        assert_eq!(plugins::signin::saved(host), None);
        *FAKE.lock().unwrap() = None;
    }
}

//! Microsoft 365 calendar, read by the app itself (no msgraph skill, no Node).
//!
//! Sign-in is OAuth 2.0 authorization code + PKCE through the **system browser** with a
//! `http://localhost:<port>` redirect (RFC 8252 loopback). The system browser matters in
//! managed tenants: the Enterprise SSO plug-in (macOS) / WAM (Windows) attach the device claim
//! that Conditional Access requires, which an embedded login window would not.
//!
//! The app registration is a setting (`config.graph_client_id`, default CodeMie's public
//! client, consented at Contoso; another org registers its own public client with redirect
//! `http://localhost` and delegated `Calendars.Read`). Only read-only scopes are requested.
//! Tokens live in `<config dir>/graph.json` (mode 0600) — like `gh` and the msgraph skill; the
//! Keychain would re-prompt after every re-signed build.

use crate::app::NoWindow;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::paths;

/// CodeMie (Contoso) public client — the default for the Contoso beta.
pub const DEFAULT_CLIENT_ID: &str = "3d7688c6-f449-4d04-8b0d-57d94818e922";
pub const DEFAULT_TENANT: &str = "common";
/// Read-only: the calendar, and the chats the user is in (the meeting chat during a call).
const SCOPES: &str = "openid profile offline_access User.Read Calendars.Read Chat.Read";
const GRAPH: &str = "https://graph.microsoft.com/v1.0";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Serialize, Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: String,
    /// unix seconds
    expires_at: i64,
    username: String,
    /// the registration these tokens belong to (a changed client id needs a new sign-in)
    client_id: String,
}

fn token_file() -> PathBuf {
    paths::config_dir().join("graph.json")
}

/// (client id, tenant) from the settings.
pub fn app_registration() -> (String, String) {
    let cfg = Config::load().unwrap_or_default();
    (
        cfg.graph_client_id.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_CLIENT_ID.into()),
        cfg.graph_tenant.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_TENANT.into()),
    )
}

fn load() -> Option<Tokens> {
    let t: Tokens = serde_json::from_slice(&std::fs::read(token_file()).ok()?).ok()?;
    (t.client_id == app_registration().0).then_some(t)
}

fn save(t: &Tokens) -> Result<()> {
    std::fs::create_dir_all(paths::config_dir())?;
    let tmp = token_file().with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600); // never readable by others, even briefly
    opts.open(&tmp)?.write_all(&serde_json::to_vec_pretty(t)?)?;
    std::fs::rename(tmp, token_file())?;
    Ok(())
}

/// The signed-in account, if any (for the current app registration).
pub fn account() -> Option<String> {
    load().map(|t| t.username)
}

pub fn logout() -> Result<()> {
    let _ = std::fs::remove_file(token_file());
    Ok(())
}

// ── PKCE helpers ───────────────────────────────────────────────────────────────────────────

fn b64url(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 2);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..=c.len() {
            out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

fn random(n: usize) -> Result<Vec<u8>> {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).map_err(|e| anyhow!("no secure random source: {e}"))?;
    Ok(b)
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", urlencode(v))).collect::<Vec<_>>().join("&")
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| {
            let v = v.replace('+', " ");
            let mut out = vec![];
            let b = v.as_bytes();
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'%' && i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() {
                    let hex = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
                    out.push(hex(b[i + 1]) << 4 | hex(b[i + 2]));
                    i += 3;
                    continue;
                }
                out.push(b[i]);
                i += 1;
            }
            String::from_utf8_lossy(&out).into_owned()
        })
    })
}

// ── HTTP via curl (tokens never on the command line: curl reads its config from stdin) ─────

fn curl(config: &str) -> Result<Value> {
    let mut child = Command::new("curl").no_window()
        .args(["-sS", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running curl")?;
    child.stdin.take().unwrap().write_all(config.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("network error: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    serde_json::from_slice(&out.stdout).context("unexpected reply from Microsoft")
}

fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn token_request(tenant: &str, body: &str) -> Result<Value> {
    let url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token");
    let v = curl(&format!("url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/x-www-form-urlencoded\"\ndata = {}\n", quote(&url), quote(body)))?;
    if let Some(e) = v["error"].as_str() {
        bail!("{e}: {}", v["error_description"].as_str().unwrap_or("").lines().next().unwrap_or(""));
    }
    Ok(v)
}

fn graph_get(token: &str, url: &str) -> Result<Value> {
    let v = curl(&format!(
        "url = {}\nheader = {}\nheader = \"Prefer: outlook.timezone=\\\"UTC\\\"\"\n",
        quote(url),
        quote(&format!("Authorization: Bearer {token}"))
    ))?;
    if let Some(e) = v.get("error") {
        bail!("Graph: {} {}", e["code"].as_str().unwrap_or(""), e["message"].as_str().unwrap_or(""));
    }
    Ok(v)
}

// ── sign-in ─────────────────────────────────────────────────────────────────────────────────

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(url).status();
    #[cfg(windows)]
    let _ = Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).status();
}

/// Wait for the browser's redirect on the loopback listener; reply with a small page.
fn wait_for_code(listener: &TcpListener, state: &str) -> Result<String> {
    listener.set_nonblocking(true)?;
    let t0 = Instant::now();
    while t0.elapsed() < LOGIN_TIMEOUT {
        match listener.accept() {
            Ok((stream, _)) => match handle_redirect(stream, state) {
                Ok(Some(code)) => return Ok(code),
                Ok(None) => {}
                // A preconnected socket that never sends a request just times out: keep waiting.
                Err(e) if e.downcast_ref::<std::io::Error>().is_some() => {}
                Err(e) => return Err(e),
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.into()),
        }
    }
    bail!("sign-in timed out (5 min)")
}

fn handle_redirect(mut stream: TcpStream, state: &str) -> Result<Option<String>> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let path = line.split_whitespace().nth(1).unwrap_or("/");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    // Only a redirect carrying our state counts; anything else (favicon, a stray request) is
    // ignored rather than ending the sign-in.
    if query_param(query, "state").as_deref() != Some(state) {
        return Ok(None);
    }
    let (ok, body) = match query_param(query, "code") {
        Some(code) => (Some(code), "Signed in to Echo. You can close this tab."),
        None => (None, "Sign-in failed — see Echo for details. You can close this tab."),
    };
    let html = format!("<!doctype html><meta charset=utf-8><title>Echo</title><body style=\"font:16px system-ui;margin:3em\">{body}</body>");
    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}", html.len());
    let _ = stream.flush();
    let mut sink = [0u8; 1024];
    let _ = stream.read(&mut sink);
    match ok {
        Some(code) => Ok(Some(code)),
        None => bail!("{}", query_param(query, "error_description").unwrap_or_default().lines().next().unwrap_or("sign-in failed")),
    }
}

/// `echoctl calendar login`: browser sign-in; returns the account name.
pub fn login() -> Result<String> {
    let (client_id, tenant) = app_registration();
    let verifier = b64url(&random(48)?);
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let state = b64url(&random(16)?);
    let listener = TcpListener::bind("127.0.0.1:0").context("opening the local sign-in port")?;
    let port = listener.local_addr()?.port();
    // Registered as http://localhost (loopback: any port). Browsers may try ::1 for
    // "localhost" first, so listen there too when available.
    let v6 = TcpListener::bind(("::1", port)).ok();
    let redirect = format!("http://localhost:{port}");
    let auth = format!(
        "https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize?{}",
        form(&[
            ("client_id", &client_id),
            ("response_type", "code"),
            ("redirect_uri", &redirect),
            ("response_mode", "query"),
            ("scope", SCOPES),
            ("state", &state),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("prompt", "select_account"),
        ])
    );
    eprintln!("opening the browser to sign in to Microsoft 365 (calendar, read-only)…");
    open_browser(&auth);
    let code = match &v6 {
        Some(l6) => {
            // Poll both listeners.
            listener.set_nonblocking(true)?;
            l6.set_nonblocking(true)?;
            let t0 = Instant::now();
            loop {
                if t0.elapsed() > LOGIN_TIMEOUT {
                    bail!("sign-in timed out (5 min)");
                }
                let mut got = None;
                for l in [&listener, l6] {
                    if let Ok((s, _)) = l.accept() {
                        got = match handle_redirect(s, &state) {
                            Ok(code) => code,
                            Err(e) if e.downcast_ref::<std::io::Error>().is_some() => None, // idle preconnect
                            Err(e) => return Err(e),
                        };
                    }
                    if got.is_some() {
                        break;
                    }
                }
                if let Some(c) = got {
                    break c;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        None => wait_for_code(&listener, &state)?,
    };
    let v = token_request(
        &tenant,
        &form(&[
            ("client_id", &client_id),
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &redirect),
            ("code_verifier", &verifier),
            ("scope", SCOPES),
        ]),
    )?;
    let access = v["access_token"].as_str().ok_or_else(|| anyhow!("no access token in the reply"))?.to_string();
    let me = graph_get(&access, &format!("{GRAPH}/me?$select=userPrincipalName,mail,displayName"))?;
    let username = me["mail"].as_str().or(me["userPrincipalName"].as_str()).unwrap_or("").to_string();
    save(&Tokens {
        access_token: access,
        refresh_token: v["refresh_token"].as_str().unwrap_or("").to_string(),
        expires_at: Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(3600),
        username: username.clone(),
        client_id,
    })?;
    // Who "Me" is, for colleagues vs external in notes — only if the user hasn't set it.
    if let Ok(mut cfg) = Config::load().map_err(|e| eprintln!("settings not updated: {e:#}")) {
        if cfg.me.is_none() {
            cfg.me = Some(crate::meeting::Person { name: me["displayName"].as_str().unwrap_or("").to_string(), email: Some(username.clone()) });
            let _ = cfg.save();
        }
    }
    Ok(username)
}

/// A valid access token, refreshing it when it is about to expire.
fn access_token() -> Result<String> {
    let mut t = load().ok_or_else(|| anyhow!("not signed in to Microsoft 365 — `echoctl calendar login`"))?;
    if t.expires_at - 120 > Utc::now().timestamp() {
        return Ok(t.access_token);
    }
    let (client_id, tenant) = app_registration();
    let v = token_request(
        &tenant,
        &form(&[("client_id", &client_id), ("grant_type", "refresh_token"), ("refresh_token", &t.refresh_token), ("scope", SCOPES)]),
    )
    .context("the Microsoft 365 sign-in expired — sign in again")?;
    t.access_token = v["access_token"].as_str().ok_or_else(|| anyhow!("no access token in the refresh reply"))?.to_string();
    if let Some(r) = v["refresh_token"].as_str() {
        t.refresh_token = r.to_string();
    }
    t.expires_at = Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(3600);
    save(&t)?;
    Ok(t.access_token)
}

/// Events between `from` and `to` (Graph shape, times in UTC), following pagination.
pub fn calendar_view(from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Value>> {
    let token = access_token()?;
    let fmt = |d: DateTime<Utc>| d.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let select = "id,subject,start,end,organizer,attendees,isOnlineMeeting,onlineMeeting,onlineMeetingProvider,\
                  isAllDay,isCancelled,responseStatus,bodyPreview,location,seriesMasterId,type,webLink";
    let mut url = Some(format!(
        "{GRAPH}/me/calendarView?startDateTime={}&endDateTime={}&$top=50&$orderby=start/dateTime&$select={select}",
        fmt(from),
        fmt(to)
    ));
    let mut events = vec![];
    while let Some(u) = url.take() {
        let v = graph_get(&token, &u)?;
        events.extend(v["value"].as_array().cloned().unwrap_or_default());
        url = v["@odata.nextLink"].as_str().map(str::to_owned).filter(|_| events.len() < 500);
    }
    Ok(events)
}

/// The meeting chat's id from a Teams join link: `/l/meetup-join/19%3ameeting_…%40thread.v2/…`.
pub fn chat_id_from_join_url(url: &str) -> Option<String> {
    let rest = url.split("/meetup-join/").nth(1)?;
    let raw = rest.split('/').next()?;
    let id = query_param(&format!("x={raw}"), "x")?;
    (id.starts_with("19:") && id.ends_with("@thread.v2")).then_some(id)
}

/// Plain text from a Teams message body (HTML): tags dropped, common entities decoded.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for c in html.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let t = tag.to_lowercase();
                if t.starts_with("br") || t.starts_with("/p") || t.starts_with("/div") || t.starts_with("/li") {
                    out.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) => out.push(c),
        }
    }
    let out = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    out.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn created_at(m: &Value) -> Option<DateTime<Utc>> {
    m["createdDateTime"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc))
}

/// All messages of a chat (user messages and system events) created in [since, until],
/// oldest first, ≤ 400.
fn chat_items(token: &str, chat_id: &str, since: DateTime<Utc>, until: DateTime<Utc>) -> Result<Vec<Value>> {
    // Newest *created* first, so paging can stop at the first message older than `since` (the
    // default order is by last modification: an old message edited today would end it early).
    let mut url = Some(format!("{GRAPH}/chats/{}/messages?$top=50&$orderby=createdDateTime%20desc", urlencode(chat_id)));
    let mut out = vec![];
    'pages: while let Some(u) = url.take() {
        let v = graph_get(token, &u)?;
        for m in v["value"].as_array().cloned().unwrap_or_default() {
            match created_at(&m) {
                Some(c) if c < since => break 'pages, // newest first: the rest is older
                Some(c) if c > until => continue,
                _ => out.push(m),
            }
        }
        url = v["@odata.nextLink"].as_str().map(str::to_owned).filter(|_| out.len() < 400);
    }
    out.reverse();
    Ok(out)
}

/// What people wrote in a chat during [since, until], oldest first (no system events).
pub fn chat_messages(chat_id: &str, since: DateTime<Utc>, until: DateTime<Utc>) -> Result<Vec<Value>> {
    let token = access_token()?;
    Ok(chat_items(&token, chat_id, since, until)?
        .into_iter()
        .filter(|m| m["messageType"] == "message" && m["deletedDateTime"].is_null())
        .collect())
}

fn call_event(m: &Value) -> Option<&'static str> {
    let kind = m["eventDetail"]["@odata.type"].as_str()?;
    if kind.ends_with("callStartedEventMessageDetail") {
        Some("started")
    } else if kind.ends_with("callEndedEventMessageDetail") {
        Some("ended")
    } else {
        None
    }
}

/// Call start/end events in recently active chats since `since` (diagnostics):
/// (chat topic or members, event, time).
pub fn recent_call_events(since: DateTime<Utc>) -> Result<Vec<(String, &'static str, DateTime<Utc>)>> {
    let token = access_token()?;
    let chats = graph_get(&token, &format!("{GRAPH}/me/chats?$top=20&$orderby=lastMessagePreview/createdDateTime%20desc&$expand=members"))?;
    let mut out = vec![];
    for c in chats["value"].as_array().into_iter().flatten() {
        let Some(id) = c["id"].as_str() else { continue };
        let name = c["topic"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| {
            c["members"].as_array().into_iter().flatten().filter_map(|m| m["displayName"].as_str()).take(3).collect::<Vec<_>>().join(", ")
        });
        for m in chat_items(&token, id, since, Utc::now())? {
            if let (Some(ev), Some(t)) = (call_event(&m), created_at(&m)) {
                out.push((format!("{name} [{}]", c["chatType"].as_str().unwrap_or("?")), ev, t));
            }
        }
    }
    out.sort_by_key(|(_, _, t)| *t);
    Ok(out)
}

/// A chat: its id, topic (group chats) and members.
pub type ChatInfo = (String, Option<String>, Vec<crate::meeting::Person>);

/// The chat an ad-hoc call was started from, by the name Teams gives the call (`ax::call_name`):
/// among the recently active chats, a group chat with that topic or a 1:1 chat with that
/// person. Calls from a chat post no "call started" event, so `find_call_chat` can't see them.
pub fn chat_named(name: &str) -> Result<Option<ChatInfo>> {
    let token = access_token()?;
    let me = account().unwrap_or_default();
    let chats = graph_get(&token, &format!("{GRAPH}/me/chats?$top=20&$orderby=lastMessagePreview/createdDateTime%20desc&$expand=members"))?;
    for c in chats["value"].as_array().into_iter().flatten() {
        let Some(id) = c["id"].as_str() else { continue };
        let members: Vec<crate::meeting::Person> = c["members"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| {
                let name = m["displayName"].as_str()?.trim().to_string();
                let email = m["email"].as_str().filter(|e| !e.is_empty()).map(str::to_owned);
                Some(crate::meeting::Person { name, email })
            })
            .collect();
        let topic = c["topic"].as_str().map(str::trim).filter(|t| !t.is_empty());
        let others: Vec<&crate::meeting::Person> = members.iter().filter(|p| !p.email.as_deref().is_some_and(|e| e.eq_ignore_ascii_case(&me))).collect();
        let matches = match (c["chatType"].as_str(), topic) {
            (_, Some(t)) => t.eq_ignore_ascii_case(name),
            (Some("oneOnOne"), None) => others.len() == 1 && others[0].name.eq_ignore_ascii_case(name),
            _ => false,
        };
        if matches {
            return Ok(Some((id.to_string(), topic.map(str::to_owned), members)));
        }
    }
    Ok(None)
}

/// The chat whose call is in progress at `recording_start`: an ad-hoc call ("Meet now", a call
/// from a group or 1:1 chat) has no join link, but Teams posts "call started" / "call ended"
/// events into its chat. Picks the chat with the latest call start before the recording (with
/// a little slack) and no end in between. Returns (chat id, call start).
pub fn find_call_chat(recording_start: DateTime<Utc>) -> Result<Option<(String, DateTime<Utc>)>> {
    let token = access_token()?;
    let window = recording_start - chrono::Duration::hours(4);
    let slack = recording_start + chrono::Duration::minutes(3);
    let chats = graph_get(&token, &format!("{GRAPH}/me/chats?$top=20&$orderby=lastMessagePreview/createdDateTime%20desc&$expand=lastMessagePreview"))?;
    let mut best: Option<(String, DateTime<Utc>)> = None;
    for c in chats["value"].as_array().into_iter().flatten() {
        let Some(id) = c["id"].as_str() else { continue };
        // Chats quiet for hours can't hold a call that started just now.
        if c["lastMessagePreview"]["createdDateTime"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| t.with_timezone(&Utc) < window) {
            continue;
        }
        let items = chat_items(&token, id, window, slack)?;
        let mut started: Option<DateTime<Utc>> = None;
        for m in &items {
            match (call_event(m), created_at(m)) {
                (Some("started"), Some(t)) => started = Some(t),
                (Some("ended"), Some(t)) if t <= recording_start => started = None,
                _ => {}
            }
        }
        if let Some(t) = started {
            if best.as_ref().is_none_or(|(_, b)| t > *b) {
                best = Some((id.to_string(), t));
            }
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meeting_chat_id_comes_from_the_join_link() {
        let url = "https://teams.microsoft.com/l/meetup-join/19%3ameeting_ZmM3ZGMz%40thread.v2/0?context=%7b%22Tid%22%7d";
        assert_eq!(chat_id_from_join_url(url).as_deref(), Some("19:meeting_ZmM3ZGMz@thread.v2"));
        assert_eq!(chat_id_from_join_url("https://zoom.us/j/1"), None);
    }

    #[test]
    fn message_html_becomes_text() {
        let html = "<p>Here is the <a href=\"x\">deck</a> &amp; notes</p><p><at id=\"0\">Daniel</at> please check</p>";
        assert_eq!(html_to_text(html), "Here is the deck & notes\nDaniel please check");
    }

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        // RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(b64url(&Sha256::digest(verifier.as_bytes())), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(b64url(b"f"), "Zg");
        assert_eq!(b64url(b"foob"), "Zm9vYg");
    }

    #[test]
    fn redirect_query_is_decoded() {
        let q = "code=0.AbC%2Bd-e&state=xyz&session_state=1";
        assert_eq!(query_param(q, "code").as_deref(), Some("0.AbC+d-e"));
        assert_eq!(query_param(q, "state").as_deref(), Some("xyz"));
        assert_eq!(query_param(q, "missing"), None);
    }
}

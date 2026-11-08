//! Microsoft 365 calendar, read by the app itself (no msgraph skill, no Node).
//!
//! Sign-in is OAuth 2.0 authorization code + PKCE through the **system browser** with a
//! `http://localhost:<port>` redirect (RFC 8252 loopback). The system browser matters in
//! managed tenants: the Enterprise SSO plug-in (macOS) / WAM (Windows) attach the device claim
//! that Conditional Access requires, which an embedded login window would not.
//!
//! The app registration is a setting (`config.graph_client_id`, default CodeMie's public
//! client, which a tenant's admin may have to approve; an organisation can register its own public client with redirect
//! `http://localhost` and delegated `Calendars.Read`). Only read-only scopes are requested; the token carries every
//! scope the tenant consented for the registration, which `m365.rs` (mail, Teams, OneDrive) relies on.
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
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::paths;

/// CodeMie's public client (an open project's app registration) — the default.
pub const DEFAULT_CLIENT_ID: &str = "3d7688c6-f449-4d04-8b0d-57d94818e922";
pub const DEFAULT_TENANT: &str = "common";
/// Read-only: the calendar, and the chats the user is in (the meeting chat during a call).
const SCOPES: &str = "openid profile offline_access User.Read Calendars.Read Chat.Read People.Read";
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
    // A newline would end the option and start another (curl reads its config line by line).
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r"))
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
/// What the user's directory says about a person (the People API, `People.Read`: works for
/// colleagues; someone outside the organisation comes back with a name only, or not at all). Found
/// by their address among the person's addresses; `None` when nobody matches.
pub fn person(email: &str) -> Result<Option<Value>> {
    let token = access_token()?;
    let email = email.trim().to_lowercase();
    let url = format!(
        "{GRAPH}/me/people?$search={}&$top=5&$select=displayName,jobTitle,department,companyName,officeLocation,phones,scoredEmailAddresses,userPrincipalName",
        urlencode(&format!("\"{email}\""))
    );
    let v = graph_get(&token, &url)?;
    let hit = v["value"].as_array().into_iter().flatten().find(|p| {
        p["userPrincipalName"].as_str().is_some_and(|u| u.eq_ignore_ascii_case(&email))
            || p["scoredEmailAddresses"].as_array().into_iter().flatten().any(|a| a["address"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(&email)))
    });
    Ok(hit.map(|p| {
        let phones = p["phones"].as_array().cloned().unwrap_or_default();
        let phone = ["business", "mobile"].iter().find_map(|t| phones.iter().find(|x| x["type"] == *t).and_then(|x| x["number"].as_str())).or_else(|| phones.first().and_then(|x| x["number"].as_str()));
        json!({"name": p["displayName"], "title": p["jobTitle"], "department": p["department"], "company": p["companyName"],
               "office": p["officeLocation"].as_str().map(|o| o.replace('\\', ", ")), "phone": phone})
    }))
}

// ── Teams channels and mail: thread sources for the Brain (read only; nothing is kept but its index) ──

/// GET with the body asked as text (mail) besides the usual headers.
fn graph_get_text(token: &str, url: &str) -> Result<Value> {
    let v = curl(&format!(
        "url = {}\nheader = {}\nheader = \"Prefer: outlook.timezone=\\\"UTC\\\", outlook.body-content-type=\\\"text\\\"\"\n",
        quote(url),
        quote(&format!("Authorization: Bearer {token}"))
    ))?;
    if let Some(e) = v.get("error") {
        bail!("Graph: {} {}", e["code"].as_str().unwrap_or(""), e["message"].as_str().unwrap_or(""));
    }
    Ok(v)
}

/// The teams the user is in: `[{id, name}]`.
pub fn teams() -> Result<Vec<Value>> {
    let token = access_token()?;
    let v = graph_get(&token, &format!("{GRAPH}/me/joinedTeams?$select=id,displayName"))?;
    Ok(v["value"].as_array().into_iter().flatten().map(|t| json!({"id": t["id"], "name": t["displayName"]})).collect())
}

/// A team's channels: `[{id, name}]`.
pub fn channels(team: &str) -> Result<Vec<Value>> {
    let token = access_token()?;
    let v = graph_get(&token, &format!("{GRAPH}/teams/{}/channels?$select=id,displayName,membershipType", urlencode(team)))?;
    Ok(v["value"].as_array().into_iter().flatten().map(|c| json!({"id": c["id"], "name": c["displayName"], "membership": c["membershipType"]})).collect())
}

/// A channel's messages (with their replies) created or changed since `since`, newest pages first,
/// at most `cap` root messages: `[{id, at, from, text, url, replies: [{at, from, text}]}]`.
pub fn channel_messages(team: &str, channel: &str, since: DateTime<Utc>, cap: usize) -> Result<Vec<Value>> {
    let token = access_token()?;
    let mut url = Some(format!("{GRAPH}/teams/{}/channels/{}/messages?$top=50&$expand=replies", urlencode(team), urlencode(channel)));
    let mut out = vec![];
    let msg = |m: &Value| -> Option<Value> {
        if !m["deletedDateTime"].is_null() || m["messageType"].as_str().is_some_and(|t| t != "message") {
            return None;
        }
        let text = html_to_text(m["body"]["content"].as_str().unwrap_or(""));
        let text = text.trim().to_string();
        (!text.is_empty()).then(|| json!({"id": m["id"], "at": m["createdDateTime"], "from": m["from"]["user"]["displayName"].as_str().or(m["from"]["application"]["displayName"].as_str()).unwrap_or("?"),
                                          "from_id": m["from"]["user"]["id"].as_str().map(|i| format!("teams:{i}")).or_else(|| m["from"]["application"]["id"].as_str().map(|i| format!("app:{i}"))),
                                          "text": text, "url": m["webUrl"]}))
    };
    while let Some(u) = url.take() {
        let v = graph_get(&token, &u)?;
        let mut older = 0;
        for m in v["value"].as_array().into_iter().flatten() {
            let changed = m["lastModifiedDateTime"].as_str().or(m["createdDateTime"].as_str()).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc));
            let replies: Vec<Value> = m["replies"].as_array().into_iter().flatten().filter_map(msg).collect();
            let newest_reply = replies.iter().filter_map(|r| r["at"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok())).map(|t| t.with_timezone(&Utc)).max();
            if changed.max(newest_reply).is_none_or(|t| t < since) {
                older += 1;
                continue;
            }
            if let Some(mut root) = msg(m) {
                root["replies"] = json!(replies);
                out.push(root);
            }
        }
        // Pages come newest first: a page all older than `since` ends it.
        if out.len() < cap && older < v["value"].as_array().map(|a| a.len()).unwrap_or(0) {
            url = v["@odata.nextLink"].as_str().map(str::to_owned);
        }
    }
    out.truncate(cap);
    Ok(out)
}

/// Mail matching a KQL query received since `since` (newest first, at most `cap`): `[{id,
/// conversation, subject, from, to, at, text, url}]`.
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub fn mail(kql: &str, since: DateTime<Utc>, cap: usize) -> Result<Vec<Value>> {
    let token = access_token()?;
    let q = format!("{kql} received>={}", since.format("%Y-%m-%d"));
    let mut url = Some(format!(
        "{GRAPH}/me/messages?$search={}&$top=50&$select=id,conversationId,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,body,webLink,internetMessageHeaders",
        urlencode(&format!("\"{}\"", q.replace('"', "'")))
    ));
    let mut out = vec![];
    while let Some(u) = url.take() {
        let v = graph_get_text(&token, &u)?;
        for m in v["value"].as_array().into_iter().flatten() {
            out.push(mail_item(m));
        }
        if out.len() < cap {
            url = v["@odata.nextLink"].as_str().map(str::to_owned);
        }
    }
    out.truncate(cap);
    Ok(out)
}

/// One message by its Graph id, as `mail` gives each.
pub fn message(id: &str) -> Result<Value> {
    let token = access_token()?;
    let m = graph_get_text(&token, &format!(
        "{GRAPH}/me/messages/{}?$select=id,conversationId,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,body,webLink,internetMessageHeaders",
        urlencode(id)
    ))?;
    Ok(mail_item(&m))
}

/// Every message of one conversation (oldest first).
pub fn conversation(id: &str) -> Result<Vec<Value>> {
    let token = access_token()?;
    let v = graph_get_text(&token, &format!(
        "{GRAPH}/me/messages?$filter={}&$top=50&$select=id,conversationId,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,body,webLink,internetMessageHeaders",
        urlencode(&format!("conversationId eq '{}'", id.replace('\'', "''")))
    ))?;
    let mut out: Vec<Value> = v["value"].as_array().into_iter().flatten().map(mail_item).collect();
    out.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    Ok(out)
}

/// How a mail's sender was authenticated, from Exchange's headers: `internal` (sent inside the
/// organisation: `X-MS-Exchange-Organization-AuthAs: Internal`), `pass` (Exchange's own verdict — the
/// topmost `Authentication-Results`, which it prepends; any lower one could be the sender's — says
/// DMARC or composite authentication passed for the From domain itself), `fail`, or `none`. SPF and
/// DKIM alone don't count: they can pass for the attacker's own domain behind a forged From.
pub fn mail_auth(m: &Value) -> &'static str {
    let from = m["from"]["emailAddress"]["address"].as_str().and_then(|a| a.rsplit_once('@')).map(|(_, d)| d.trim().to_lowercase()).unwrap_or_default();
    let mut authas = String::new();
    let mut first: Option<String> = None;
    for h in m["internetMessageHeaders"].as_array().into_iter().flatten() {
        let (n, v) = (h["name"].as_str().unwrap_or("").to_lowercase(), h["value"].as_str().unwrap_or("").to_lowercase());
        if n == "x-ms-exchange-organization-authas" {
            authas = v;
        } else if n == "authentication-results" && first.is_none() {
            first = Some(v);
        }
    }
    if authas.trim() == "internal" {
        return "internal";
    }
    let Some(r) = first else { return "none" };
    if r.contains("dmarc=fail") || r.contains("compauth=fail") {
        return "fail";
    }
    let header_from = r.split("header.from=").nth(1).map(|x| x.split([';', ' ', ')']).next().unwrap_or("").trim().to_string()).unwrap_or_default();
    let aligned = !from.is_empty() && header_from == from;
    if aligned && (r.contains("dmarc=pass") || r.contains("compauth=pass")) {
        "pass"
    } else {
        "none"
    }
}

/// The organisation's own mail domains (colleagues), from the tenant's verified domains.
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub fn org_domains() -> Result<Vec<String>> {
    let token = access_token()?;
    let v = graph_get(&token, &format!("{GRAPH}/organization?$select=verifiedDomains"))?;
    Ok(v["value"][0]["verifiedDomains"].as_array().into_iter().flatten().filter_map(|d| d["name"].as_str().map(str::to_lowercase)).collect())
}

/// A Teams author's mail address: the People API searched by their name, kept only when the person
/// found is that author (the same directory id) — a name alone can be anyone's.
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub fn person_email(name: &str, id: &str) -> Result<Option<String>> {
    let token = access_token()?;
    let url = format!("{GRAPH}/me/people?$search={}&$top=5&$select=id,displayName,scoredEmailAddresses,userPrincipalName", urlencode(&format!("\"{}\"", name.replace('"', ""))));
    let v = graph_get(&token, &url)?;
    Ok(v["value"].as_array().into_iter().flatten().find(|p| p["id"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(id))).and_then(|p| {
        p["userPrincipalName"].as_str().or_else(|| p["scoredEmailAddresses"].as_array().and_then(|a| a.first()).and_then(|a| a["address"].as_str())).map(str::to_lowercase)
    }))
}

fn mail_item(m: &Value) -> Value {
    let who = |r: &Value| -> Value { json!({"name": r["emailAddress"]["name"], "email": r["emailAddress"]["address"]}) };
    // The new part of a reply: up to the quoted mail ("From: …", "-----Original Message-----", "On … wrote:").
    let body = m["body"]["content"].as_str().unwrap_or("");
    let mut text = String::new();
    for l in body.lines() {
        let t = l.trim();
        if t.starts_with("From:") || t.starts_with("-----Original") || (t.starts_with("On ") && t.ends_with("wrote:")) || t.starts_with("________________") {
            break;
        }
        text.push_str(l);
        text.push('\n');
    }
    // Calendar invites and their answers (`eventMessage…`): the meeting itself is the call.
    let invite = m["@odata.type"].as_str().is_some_and(|t| t.contains("eventMessage"));
    json!({"id": m["id"], "internet_id": m["internetMessageId"], "conversation": m["conversationId"], "subject": m["subject"], "from": who(&m["from"]), "invite": invite, "auth": mail_auth(m),
           "to": m["toRecipients"].as_array().into_iter().flatten().chain(m["ccRecipients"].as_array().into_iter().flatten()).map(who).collect::<Vec<_>>(),
           "at": m["receivedDateTime"], "text": text.trim(), "url": m["webLink"]})
}

pub fn calendar_view(from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<Value>> {
    let token = access_token()?;
    let fmt = |d: DateTime<Utc>| d.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut url = Some(format!(
        "{GRAPH}/me/calendarView?startDateTime={}&endDateTime={}&$top=50&$orderby=start/dateTime&$select={EVENT_FIELDS}",
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

/// The calendar event fields Echo reads (the calendar view's, by id or in a range).
const EVENT_FIELDS: &str = "id,subject,start,end,organizer,attendees,isOnlineMeeting,onlineMeeting,onlineMeetingProvider,\
                            isAllDay,isCancelled,responseStatus,bodyPreview,location,seriesMasterId,type,webLink";

/// One calendar event (an occurrence's id works too).
pub fn event(id: &str) -> Result<Value> {
    let token = access_token()?;
    graph_get(&token, &format!("{GRAPH}/me/events/{}?$select={EVENT_FIELDS}", urlencode(id)))
}

/// What Teams kept of a meeting's transcription (`OnlineMeetingTranscript.Read.All`).
pub enum Transcripts {
    /// Each transcript `{id, created, ended, vtt}`, oldest first (a restarted transcription is several).
    Found(Vec<Value>),
    /// Teams has the meeting but nobody transcribed it.
    None,
    /// The meeting isn't given to this account (an outside organiser's, or not a Teams meeting).
    NotGiven(String),
}

/// The transcripts of the Teams meeting behind a join link: `/me/onlineMeetings?$filter=JoinWebUrl eq
/// '…'` → its transcripts created within `window` (a series shares one online meeting: the
/// occurrence's, `teams_transcript::occurrence_window`) → (`content`) each one's text as WebVTT
/// (`text/plain` is refused).
pub fn meeting_transcripts(join_url: &str, window: (DateTime<Utc>, DateTime<Utc>), content: bool) -> Result<Transcripts> {
    let token = access_token()?;
    let filter = format!("JoinWebUrl eq '{}'", join_url.replace('\'', "''"));
    let (code, body) = graph_raw(&token, &format!("{GRAPH}/me/onlineMeetings?$filter={}", urlencode(&filter)))?;
    let v: Value = serde_json::from_str(&body).unwrap_or_default();
    if code != 200 {
        return Ok(Transcripts::NotGiven(graph_error(code, &v)));
    }
    let Some(id) = v["value"][0]["id"].as_str().map(str::to_owned) else {
        return Ok(Transcripts::NotGiven("Teams doesn't give this meeting to your account".into()));
    };
    let list = graph_get(&token, &format!("{GRAPH}/me/onlineMeetings/{}/transcripts", urlencode(&id)))?;
    let out = transcripts_in(&list, window, content, |tid| {
        graph_raw(&token, &format!("{GRAPH}/me/onlineMeetings/{}/transcripts/{}/content?$format=text/vtt", urlencode(&id), urlencode(tid)))
    })?;
    Ok(if out.is_empty() { Transcripts::None } else { Transcripts::Found(out) })
}

/// The transcripts of a `/transcripts` list created within `window`, oldest first; with `content`,
/// each one's text from `fetch` (status, body). Only those are fetched: Teams expires old ones (their
/// content answers 404), and one from months ago must not fail this occurrence's import.
fn transcripts_in(list: &Value, (from, to): (DateTime<Utc>, DateTime<Utc>), content: bool, mut fetch: impl FnMut(&str) -> Result<(u16, String)>) -> Result<Vec<Value>> {
    let mut out = vec![];
    for t in list["value"].as_array().into_iter().flatten() {
        let Some(tid) = t["id"].as_str() else { continue };
        let created = t["createdDateTime"].as_str().and_then(|c| DateTime::parse_from_rfc3339(c).ok()).map(|c| c.with_timezone(&Utc));
        if !created.is_some_and(|c| c >= from && c <= to) {
            continue;
        }
        let mut item = json!({"id": tid, "created": t["createdDateTime"], "ended": t["endDateTime"]});
        if content {
            let (code, vtt) = fetch(tid)?;
            if code == 404 {
                bail!("the transcript can't be read: it is no longer available (Teams expired it)");
            }
            if code != 200 {
                bail!("the transcript can't be read: {}", graph_error(code, &serde_json::from_str(&vtt).unwrap_or_default()));
            }
            item["vtt"] = json!(vtt);
        }
        out.push(item);
    }
    out.sort_by(|a, b| a["created"].as_str().cmp(&b["created"].as_str()));
    Ok(out)
}

fn graph_error(code: u16, v: &Value) -> String {
    let m = v["error"]["message"].as_str().unwrap_or("").trim();
    match code {
        401 | 403 => format!("Teams refused it (HTTP {code}{}) — an outside organiser's meeting, or the sign-in lacks OnlineMeetingTranscript.Read.All", if m.is_empty() { String::new() } else { format!(": {m}") }),
        404 => "Teams doesn't give this meeting to your account".into(),
        _ => format!("HTTP {code}{}", if m.is_empty() { String::new() } else { format!(": {m}") }),
    }
}

/// GET whose answer isn't JSON (a transcript): its status and body.
fn graph_raw(token: &str, url: &str) -> Result<(u16, String)> {
    let mut child = Command::new("curl").no_window()
        .args(["-sS", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running curl")?;
    let config = format!("url = {}\nheader = {}\nwrite-out = \"\\n%{{http_code}}\"\n", quote(url), quote(&format!("Authorization: Bearer {token}")));
    child.stdin.take().unwrap().write_all(config.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("network error: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    Ok((code.trim().parse().unwrap_or(0), body.to_string()))
}

/// POST a JSON body to Graph (`path` after `/v1.0`): the answer's JSON (`{}` for an empty one), or the
/// error in words. Only what Echo sends the user themselves (`deliver.rs`): their own chat, their mailbox.
pub fn post(path: &str, body: &Value) -> Result<Value> {
    let token = access_token()?;
    let url = format!("https://graph.microsoft.com/v1.0{path}");
    let mut child = Command::new("curl").no_window()
        .args(["-sS", "-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running curl")?;
    let config = format!(
        "url = {}\nrequest = \"POST\"\nheader = {}\nheader = \"Content-Type: application/json\"\ndata-binary = {}\nwrite-out = \"\\n%{{http_code}}\"\n",
        quote(&url),
        quote(&format!("Authorization: Bearer {token}")),
        quote(&body.to_string())
    );
    child.stdin.take().unwrap().write_all(config.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("network error: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    let code: u16 = code.trim().parse().unwrap_or(0);
    let v: Value = if body.trim().is_empty() { json!({}) } else { serde_json::from_str(body).unwrap_or_else(|_| json!({"raw": body.chars().take(300).collect::<String>()})) };
    if !(200..300).contains(&code) {
        let m = v["error"]["message"].as_str().unwrap_or("").trim();
        bail!("Microsoft 365 refused it (HTTP {code}{})", if m.is_empty() { String::new() } else { format!(": {m}") });
    }
    Ok(v)
}

/// What a `request` sends: nothing, JSON, or a file's bytes.
pub enum Body<'a> {
    None,
    Json(&'a Value),
    File(&'a std::path::Path),
}

/// Any Graph call (`m365.rs`): `method` on `path` (after `/v1.0`, or a full `https://graph.microsoft.com/…` URL —
/// a `@odata.nextLink`), with `body`; `save_to` writes the answer's bytes there instead (a download — redirects
/// followed, the token not sent on to the storage host). The answer's JSON (`{}` for an empty one; `{saved}` for a
/// download), or the error in words. `headers`: extra ones (`ConsistencyLevel: eventual`).
pub fn request(method: &str, path: &str, body: Body, save_to: Option<&std::path::Path>, headers: &[&str]) -> Result<Value> {
    let token = access_token()?;
    let url = if path.starts_with("https://graph.microsoft.com/") { path.to_string() } else { format!("{GRAPH}{path}") };
    let mut config = format!(
        "url = {}
request = {}
header = {}
write-out = \"\\n%{{http_code}}\"\n",
        quote(&url),
        quote(method),
        quote(&format!("Authorization: Bearer {token}"))
    );
    for h in headers {
        config.push_str(&format!("header = {}\n", quote(h)));
    }
    match body {
        Body::None => {}
        Body::Json(v) => config.push_str(&format!("header = \"Content-Type: application/json\"\ndata-binary = {}\n", quote(&v.to_string()))),
        Body::File(f) => config.push_str(&format!("header = \"Content-Type: application/octet-stream\"\ndata-binary = {}\n", quote(&format!("@{}", f.display())))),
    }
    if let Some(out) = save_to {
        // curl drops the Authorization header on a redirect to another host (the download URL is pre-signed).
        config.push_str(&format!("location\noutput = {}\n", quote(&out.display().to_string())));
    }
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
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let (body, code) = text.rsplit_once('\n').unwrap_or(("", &text));
    let code: u16 = code.trim().parse().unwrap_or(0);
    if let Some(out) = save_to {
        // `out` is the caller's own scratch file (m365 renames it into place): an error's body is read from it, and it goes.
        if !(200..300).contains(&code) {
            let err = std::fs::read_to_string(out).unwrap_or_default();
            let _ = std::fs::remove_file(out);
            let v: Value = serde_json::from_str(&err).unwrap_or_default();
            bail!("Microsoft 365 refused it (HTTP {code}{})", v["error"]["message"].as_str().map(|m| format!(": {m}")).unwrap_or_default());
        }
        return Ok(json!({"saved": out}));
    }
    let v: Value = if body.trim().is_empty() { json!({}) } else { serde_json::from_str(body).unwrap_or_else(|_| json!({"raw": body.chars().take(300).collect::<String>()})) };
    if !(200..300).contains(&code) {
        let m = v["error"]["message"].as_str().unwrap_or("").trim();
        bail!("Microsoft 365 refused it (HTTP {code}{})", if m.is_empty() { String::new() } else { format!(": {m}") });
    }
    Ok(v)
}

/// The signed-in user's own mail address (`/me`: `mail`, else the sign-in name).
pub fn my_address() -> Result<String> {
    let token = access_token()?;
    let v = graph_get(&token, "https://graph.microsoft.com/v1.0/me?$select=mail,userPrincipalName")?;
    v["mail"].as_str().or_else(|| v["userPrincipalName"].as_str()).filter(|m| m.contains('@')).map(str::to_owned).ok_or_else(|| anyhow!("Microsoft 365 gives no address for this account"))
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
    chat_items_upto(token, chat_id, since, until, 400)
}

fn chat_items_upto(token: &str, chat_id: &str, since: DateTime<Utc>, until: DateTime<Utc>, max: usize) -> Result<Vec<Value>> {
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
        url = v["@odata.nextLink"].as_str().map(str::to_owned).filter(|_| out.len() < max);
    }
    out.reverse();
    Ok(out)
}

/// What people wrote in a chat during [since, until], oldest first (no system events), up to `max`
/// (the newest kept): a thread reading its meeting chat between calls.
pub fn chat_history(chat_id: &str, since: DateTime<Utc>, until: DateTime<Utc>, max: usize) -> Result<Vec<Value>> {
    let token = access_token()?;
    Ok(chat_items_upto(&token, chat_id, since, until, max)?
        .into_iter()
        .filter(|m| m["messageType"] == "message" && m["deletedDateTime"].is_null())
        .collect())
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

/// How far a chat's "call started" may be from the start Teams' call timer gives.
const CALL_START_SLACK: chrono::Duration = chrono::Duration::minutes(3);

/// The chat whose call is in progress at `recording_start`: an ad-hoc call ("Meet now", a call
/// from a group or 1:1 chat) has no join link, but Teams posts "call started" / "call ended"
/// events into its chat. Picks the chat with the latest call start before the recording (with
/// a little slack) and no end in between. Returns (chat id, call start).
///
/// `began`: when the call began by Teams' own timer. When known, only a start within
/// `CALL_START_SLACK` of it counts: a meeting left hours ago whose "call ended" never came (a
/// big meeting goes on without you) otherwise looks like a call still running, and its chat
/// was taken for a 1:1 call that posts no events at all (2026-10-15). No timer (the window
/// unreadable): `left` — when you last left a call before this one — bounds it instead: a call
/// that started before you left one isn't this one (rejoining it after a while finds no chat:
/// none is better than another meeting's).
pub fn find_call_chat(recording_start: DateTime<Utc>, began: Option<DateTime<Utc>>, left: Option<DateTime<Utc>>) -> Result<Option<(String, DateTime<Utc>)>> {
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
        if let Some(t) = started.filter(|t| match began {
            Some(b) => (*t - b).abs() <= CALL_START_SLACK,
            None => left.is_none_or(|l| *t > l),
        }) {
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
    fn only_the_occurrences_transcripts_are_fetched() {
        // A bi-weekly series: two old transcripts Teams expired (404), this occurrence's readable.
        let list = json!({"value": [
            {"id": "may", "createdDateTime": "2026-05-20T13:31:00Z"},
            {"id": "oct", "createdDateTime": "2026-10-21T13:32:00Z", "endDateTime": "2026-10-21T14:01:00Z"},
            {"id": "jul", "createdDateTime": "2026-07-29T13:30:00Z"},
        ]});
        let at = |s: &str| DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc);
        let window = (at("2026-10-21T13:00:00Z"), at("2026-10-21T14:30:00Z"));
        let mut asked = vec![];
        let got = transcripts_in(&list, window, true, |id| {
            asked.push(id.to_string());
            Ok(if id == "oct" { (200, "WEBVTT\n".to_string()) } else { (404, "{}".to_string()) })
        })
        .unwrap();
        assert_eq!(asked, ["oct"]);
        assert_eq!(got.len(), 1);
        assert_eq!((got[0]["id"].as_str(), got[0]["vtt"].as_str()), (Some("oct"), Some("WEBVTT\n")));
        // Without content nothing is fetched, and the list is the occurrence's too.
        let listed = transcripts_in(&list, window, false, |_| panic!("no content asked")).unwrap();
        assert_eq!(listed.iter().map(|t| t["id"].as_str().unwrap()).collect::<Vec<_>>(), ["oct"]);
        // This occurrence's own transcript expired: said so, not "doesn't give this meeting".
        let window = (at("2026-07-29T13:00:00Z"), at("2026-07-29T14:30:00Z"));
        let e = transcripts_in(&list, window, true, |_| Ok((404, "{}".to_string()))).unwrap_err();
        assert!(format!("{e:#}").contains("no longer available (Teams expired it)"), "{e:#}");
    }

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

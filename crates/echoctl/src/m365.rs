//! Microsoft 365 for Claude sessions, on Echo's own sign-in (`echoctl m365 …`, the `echo:microsoft-365` skill): search
//! across mail, Teams chats and files; mail (read, send, reply, draft); Teams (chats, read, send — a person by address
//! through their 1:1 chat —, channels, post); people; OneDrive (list, download, upload); and any read-only Graph GET.
//! No other tool or connector: the registration's token already carries the tenant's consented scopes (`graph.rs`).
//!
//! **Nothing leaves without `--yes`**: a send, a reply, a post or an upload without it only says what would go where
//! (`{sent: false, would}`) — the session shows that to the user and runs it again with `--yes` after their yes. A run
//! Echo started (minutes, a thread's sync, a routine) can't send or draft (`ECHOCTL_HEADLESS`). Both are conventions the
//! session follows, not a lock: `--yes` is a flag a model can pass, and a session allowed every command (the folder
//! session with no permission prompts, a routine in skip mode) could unset the variable. The real gate is Claude
//! Code's own permission prompt on the command — what a session reads (mail, chats) is others' words.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};

use crate::graph::{self, Body};

/// Search's kinds, as the CLI names them, and the Search API's entity type for each.
pub const KINDS: [(&str, &str); 3] = [("mail", "message"), ("chats", "chatMessage"), ("files", "driveItem")];

/// A path segment or query value, percent-encoded. A segment of only dots is encoded too: curl would resolve `..`.
pub(crate) fn enc(s: &str) -> String {
    if !s.is_empty() && s.bytes().all(|b| b == b'.') {
        return "%2E".repeat(s.len());
    }
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A OneDrive path (`Documents/plan.docx`), each segment encoded, the slashes kept.
fn enc_path(p: &str) -> String {
    p.trim_matches('/').split('/').map(enc).collect::<Vec<_>>().join("/")
}

/// Refused inside a run Echo started: those read others' words, and nothing they read may send for them.
fn not_in_run() -> Result<()> {
    if std::env::var_os("ECHOCTL_HEADLESS").is_some() {
        bail!("refused inside a run Echo started: only the user's own sessions send through Microsoft 365");
    }
    Ok(())
}

/// The Search API's highlights (`<c0>word</c0>`, `<ddd/>`) as plain text.
fn summary(s: &str) -> String {
    s.replace("<c0>", "").replace("</c0>", "").replace("<ddd/>", "…").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn person(v: &Value) -> Value {
    let e = if v["emailAddress"].is_object() { &v["emailAddress"] } else { &v["user"] };
    json!({"name": e["name"].as_str().or(e["displayName"].as_str()), "email": e["address"].as_str().or(e["userPrincipalName"].as_str())})
}

/// One search hit as the CLI gives it: the same fields whatever its kind, plus what each needs to be opened.
fn hit(kind: &str, h: &Value) -> Value {
    let r = &h["resource"];
    let mut v = match kind {
        "mail" => json!({"id": r["id"], "subject": r["subject"], "from": person(&r["from"]), "at": r["receivedDateTime"].as_str().or(r["sentDateTime"].as_str()),
                          "conversation": r["conversationId"], "url": r["webLink"]}),
        // A chat message has its chat; a channel post its team and channel (`teams channel <team> <channel>`).
        "chats" => json!({"id": r["id"], "chat": r["chatId"], "team": r["channelIdentity"]["teamId"], "channel": r["channelIdentity"]["channelId"],
                           "from": person(&r["from"]), "at": r["createdDateTime"], "url": r["webUrl"].as_str().or(r["webLink"].as_str())}),
        _ => json!({"id": r["id"], "name": r["name"], "url": r["webUrl"], "at": r["lastModifiedDateTime"], "by": r["lastModifiedBy"]["user"]["displayName"],
                     "ref": r["parentReference"]["driveId"].as_str().zip(r["id"].as_str()).map(|(d, i)| format!("drives/{d}/items/{i}"))}),
    };
    // Mail hits carry their id in the hit, not the resource.
    if v["id"].is_null() {
        v["id"] = h["hitId"].clone();
    }
    v["kind"] = json!(kind);
    v["summary"] = json!(summary(h["summary"].as_str().unwrap_or("")));
    v
}

/// The Search API over `kinds` (mail, chats, files): what Microsoft 365 ranks most relevant first, each kind its own
/// request (the API takes one entity type of these at a time). `from` narrows mail and chats to a sender, `days` to
/// the last N days. KQL in `query` passes through (`subject:`, `filetype:`, quotes).
pub fn search(query: &str, kinds: &[String], limit: usize, from: Option<&str>, days: Option<i64>) -> Result<Value> {
    let since = days.map(|d| Utc::now() - Duration::days(d));
    let mut out = vec![];
    let mut errors = serde_json::Map::new();
    for kind in kinds {
        let Some((_, entity)) = KINDS.iter().find(|(k, _)| k == kind) else { bail!("no kind {kind}: mail, chats or files") };
        let mut q = query.trim().to_string();
        if let (Some(f), true) = (from, *entity != "driveItem") {
            q = format!("{q} from:\"{}\"", f.replace('"', ""));
        }
        // The date in the query, so the ranking is over that window (measured: received / sent / LastModifiedTime).
        if let Some(s) = since {
            let field = match *entity { "message" => "received", "chatMessage" => "sent", _ => "LastModifiedTime" };
            q = format!("{q} {field}>={}", s.format("%Y-%m-%d"));
        }
        let body = json!({"requests": [{"entityTypes": [entity], "query": {"queryString": q}, "from": 0, "size": limit.clamp(1, 50)}]});
        match graph::request("POST", "/search/query", Body::Json(&body), None, &[]) {
            Ok(v) => {
                for h in v["value"][0]["hitsContainers"][0]["hits"].as_array().into_iter().flatten() {
                    let x = hit(kind, h);
                    let at = x["at"].as_str().and_then(|a| DateTime::parse_from_rfc3339(a).ok()).map(|a| a.with_timezone(&Utc));
                    if since.is_none_or(|s| at.is_none_or(|a| a >= s)) {
                        out.push(x);
                    }
                }
            }
            Err(e) => {
                errors.insert(kind.clone(), json!(format!("{e:#}")));
            }
        }
    }
    if out.is_empty() && !errors.is_empty() && errors.len() == kinds.len() {
        bail!("search failed: {}", Value::Object(errors));
    }
    Ok(json!({"query": query, "results": out, "errors": if errors.is_empty() { Value::Null } else { Value::Object(errors) }}))
}

// ── mail ─────────────────────────────────────────────────────────────────────────────────────

/// One mail (its new part), or `thread`: every mail of its conversation, oldest first.
pub fn mail_read(id: &str, thread: bool) -> Result<Value> {
    let m = graph::message(id)?;
    if !thread {
        return Ok(m);
    }
    let conv = m["conversation"].as_str().ok_or_else(|| anyhow!("that mail has no conversation"))?;
    Ok(json!({"subject": m["subject"], "messages": graph::conversation(conv)?}))
}

fn recipients(list: &[String]) -> Result<Vec<Value>> {
    list.iter()
        .flat_map(|a| a.split(','))
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(|a| if crate::deliver::valid_address(a) { Ok(json!({"emailAddress": {"address": a}})) } else { Err(anyhow!("not a mail address: {a}")) })
        .collect()
}

/// What a model wrote says so, in every channel (`deliver::ai_notice`): `ai` = the model's name.
fn with_notice(html: String, ai: Option<&str>) -> String {
    match ai.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => format!("{html}<p><em>{}</em></p>", crate::deliver::ai_notice(m).replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")),
        None => html,
    }
}

fn markdown_file(file: &Path) -> Result<(String, String)> {
    let md = std::fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    if md.trim().is_empty() {
        bail!("{} is empty", file.display());
    }
    let html = crate::minutes::markdown_html(&md);
    Ok((md, html))
}

/// A new mail from the user's account (kept in Sent): `to`, `cc`, `subject`, the Markdown `file` as its body.
pub fn mail_send(to: &[String], cc: &[String], subject: &str, file: &Path, ai: Option<&str>, yes: bool) -> Result<Value> {
    let (to, cc) = (recipients(to)?, recipients(cc)?);
    if to.is_empty() {
        bail!("no recipient (--to)");
    }
    let (md, html) = markdown_file(file)?;
    let html = with_notice(html, ai);
    let would = json!({"to": to, "cc": cc, "subject": subject, "text": md});
    if !yes {
        return Ok(not_sent(would));
    }
    not_in_run()?;
    graph::request("POST", "/me/sendMail", Body::Json(&json!({"message": {"subject": subject, "body": {"contentType": "HTML", "content": html}, "toRecipients": to, "ccRecipients": cc}, "saveToSentItems": true})), None, &[])?;
    Ok(json!({"sent": true, "to": would["to"], "subject": subject}))
}

/// A reply to mail `id` (`all`: to everyone on it), the Markdown `file` above the quoted mail.
pub fn mail_reply(id: &str, all: bool, file: &Path, ai: Option<&str>, yes: bool) -> Result<Value> {
    let (md, _) = markdown_file(file)?;
    mail_reply_text(id, all, &md, ai, yes)
}

/// A reply in the user's own words (Today's Read and reply: their click on Reply is the yes).
pub fn mail_reply_text(id: &str, all: bool, md: &str, ai: Option<&str>, yes: bool) -> Result<Value> {
    anyhow::ensure!(!md.trim().is_empty(), "the reply is empty");
    let m = graph::message(id)?;
    let html = crate::minutes::markdown_html(md);
    let html = with_notice(html, ai);
    let would = json!({"reply_to": {"subject": m["subject"], "from": m["from"], "at": m["at"]}, "to": if all { "everyone on it" } else { "its sender" }, "text": md});
    if !yes {
        return Ok(not_sent(would));
    }
    not_in_run()?;
    graph::request("POST", &format!("/me/messages/{}/{}", enc(id), if all { "replyAll" } else { "reply" }), Body::Json(&json!({"comment": html})), None, &[])?;
    Ok(json!({"sent": true, "reply_to": m["subject"]}))
}

/// A draft in the user's Drafts (nothing is sent): its link, to open and send from Outlook.
pub fn mail_draft(to: &[String], cc: &[String], subject: &str, file: &Path, ai: Option<&str>) -> Result<Value> {
    not_in_run()?;
    let (_, html) = markdown_file(file)?;
    let html = with_notice(html, ai);
    let v = graph::request("POST", "/me/messages", Body::Json(&json!({"subject": subject, "body": {"contentType": "HTML", "content": html}, "toRecipients": recipients(to)?, "ccRecipients": recipients(cc)?})), None, &[])?;
    Ok(json!({"draft": true, "id": v["id"], "url": v["webLink"]}))
}

fn not_sent(would: Value) -> Value {
    json!({"sent": false, "would": would, "next": "Nothing was sent. Show the user exactly this; run the same command with --yes only after they agree."})
}

// ── Teams ────────────────────────────────────────────────────────────────────────────────────

/// The user's chats, most recently active first: kind, topic, members, the last message. `with` keeps those whose
/// topic or a member's name or address contains it.
pub fn chats(with: Option<&str>, limit: usize) -> Result<Value> {
    let w = with.map(|w| w.to_lowercase());
    let mut url = Some("/me/chats?$expand=members,lastMessagePreview&$orderby=lastMessagePreview/createdDateTime%20desc&$top=50".to_string());
    let mut out = vec![];
    let mut pages = 0;
    while let Some(u) = url.take() {
        let v = graph::request("GET", &u, Body::None, None, &[])?;
        for c in v["value"].as_array().into_iter().flatten() {
            let members: Vec<Value> = c["members"].as_array().into_iter().flatten().map(|m| json!({"name": m["displayName"], "email": m["email"]})).collect();
            let hay = format!("{} {}", c["topic"].as_str().unwrap_or(""), members.iter().map(|m| format!("{} {}", m["name"].as_str().unwrap_or(""), m["email"].as_str().unwrap_or(""))).collect::<Vec<_>>().join(" ")).to_lowercase();
            if w.as_ref().is_some_and(|w| !hay.contains(w.as_str())) {
                continue;
            }
            let last = &c["lastMessagePreview"];
            out.push(json!({"id": c["id"], "kind": c["chatType"], "topic": c["topic"], "members": members, "url": c["webUrl"],
                            "last": {"at": last["createdDateTime"], "from": last["from"]["user"]["displayName"], "text": graph::html_to_text(last["body"]["content"].as_str().unwrap_or("")).chars().take(200).collect::<String>()}}));
            if out.len() >= limit {
                return Ok(json!(out));
            }
        }
        pages += 1;
        // Looking for someone: a few pages back; a plain list: the first page is the recent ones.
        if w.is_some() && pages < 6 {
            url = v["@odata.nextLink"].as_str().map(str::to_owned);
        }
    }
    Ok(json!(out))
}

/// A chat's messages of the last `days` (oldest first, at most `limit`, the newest kept): who, when, the text, files.
pub fn chat_read(chat: &str, days: i64, limit: usize) -> Result<Value> {
    let now = Utc::now();
    let msgs = graph::chat_history(chat, now - Duration::days(days), now, limit)?;
    // It reads whole pages: the newest `limit` of them.
    let msgs = &msgs[msgs.len().saturating_sub(limit.max(1))..];
    Ok(json!(msgs.iter().map(|m| json!({
        "id": m["id"], "at": m["createdDateTime"], "from": m["from"]["user"]["displayName"],
        "text": graph::html_to_text(m["body"]["content"].as_str().unwrap_or("")),
        "files": m["attachments"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).collect::<Vec<_>>(),
    })).collect::<Vec<_>>()))
}

/// The 1:1 chat with `email` (Teams gives the existing one, or makes it): no directory read needed — the person is
/// bound by address (`users('<address>')`).
fn one_on_one(email: &str) -> Result<String> {
    let me = graph::request("GET", "/me?$select=id", Body::None, None, &[])?;
    let my_id = me["id"].as_str().ok_or_else(|| anyhow!("Microsoft 365 gives no id for this account"))?;
    let member = |who: &str| json!({"@odata.type": "#microsoft.graph.aadUserConversationMember", "roles": ["owner"], "user@odata.bind": format!("https://graph.microsoft.com/v1.0/users('{}')", who.replace('\'', "''"))});
    let v = graph::request("POST", "/chats", Body::Json(&json!({"chatType": "oneOnOne", "members": [member(my_id), member(email)]})), None, &[])
        .with_context(|| format!("no Teams chat with {email}"))?;
    v["id"].as_str().map(str::to_owned).ok_or_else(|| anyhow!("Teams gave no chat for {email}"))
}

/// A message in a chat: `chat` by its id, or `to` a person by address (their 1:1 chat). Its text: the Markdown `file`.
pub fn chat_send(chat: Option<&str>, to: Option<&str>, file: &Path, ai: Option<&str>, yes: bool) -> Result<Value> {
    let (md, html) = markdown_file(file)?;
    let html = with_notice(html, ai);
    let id = match (chat, to) {
        (Some(c), None) => c.to_string(),
        // A person: the preview names them; their 1:1 chat is opened (made, if new — visible to them) only to send.
        (None, Some(t)) if crate::deliver::valid_address(t) => {
            if !yes {
                return Ok(not_sent(json!({"to": t, "kind": "oneOnOne", "text": md})));
            }
            not_in_run()?;
            one_on_one(t)?
        }
        (None, Some(t)) => bail!("not a mail address: {t} (find it with `m365 people`)"),
        _ => bail!("one of --chat or --to"),
    };
    // The chat with yourself (`48:notes`) has no members to show: Graph doesn't take it as a chat thread.
    let would = if id == "48:notes" {
        json!({"chat": id, "kind": "yourself", "text": md})
    } else {
        let info = graph::request("GET", &format!("/chats/{}?$expand=members", enc(&id)), Body::None, None, &[])?;
        let members: Vec<Value> = info["members"].as_array().into_iter().flatten().map(|m| json!(m["displayName"])).collect();
        json!({"chat": id, "kind": info["chatType"], "topic": info["topic"], "members": members, "text": md})
    };
    if !yes {
        return Ok(not_sent(would));
    }
    not_in_run()?;
    let v = graph::request("POST", &format!("/chats/{}/messages", enc(&id)), Body::Json(&json!({"body": {"contentType": "html", "content": html}})), None, &[])?;
    Ok(json!({"sent": true, "chat": id, "id": v["id"], "url": v["webUrl"]}))
}

/// A message the user sent by a click in the window (Won't do's draft): `md` into the chat `chat`, at once.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn chat_send_text(chat: &str, md: &str, ai: Option<&str>) -> Result<Value> {
    anyhow::ensure!(!md.trim().is_empty(), "the message is empty");
    anyhow::ensure!(!chat.trim().is_empty(), "no Teams chat to send it to");
    not_in_run()?;
    let html = with_notice(crate::minutes::markdown_html(md), ai);
    let v = graph::request("POST", &format!("/chats/{}/messages", enc(chat)), Body::Json(&json!({"body": {"contentType": "html", "content": html}})), None, &[])?;
    Ok(json!({"sent": true, "chat": chat, "id": v["id"], "url": v["webUrl"]}))
}

/// The user's teams, or (`team`) a team's channels.
pub fn teams(team: Option<&str>) -> Result<Value> {
    Ok(json!(match team {
        Some(t) => graph::channels(t)?,
        None => graph::teams()?,
    }))
}

/// A channel's posts of the last `days` (with their replies).
pub fn channel_read(team: &str, channel: &str, days: i64, limit: usize) -> Result<Value> {
    let posts = graph::channel_messages(team, channel, Utc::now() - Duration::days(days), limit)?;
    Ok(json!(posts.iter().map(|m| json!({
        "id": m["id"], "at": m["createdDateTime"], "from": m["from"]["user"]["displayName"], "subject": m["subject"],
        "text": graph::html_to_text(m["body"]["content"].as_str().unwrap_or("")),
        "replies": m["replies"].as_array().into_iter().flatten().map(|r| json!({"at": r["createdDateTime"], "from": r["from"]["user"]["displayName"], "text": graph::html_to_text(r["body"]["content"].as_str().unwrap_or(""))})).collect::<Vec<_>>(),
    })).collect::<Vec<_>>()))
}

/// A new post in a channel (`subject` optional), the Markdown `file` as its text.
pub fn channel_post(team: &str, channel: &str, subject: Option<&str>, file: &Path, ai: Option<&str>, yes: bool) -> Result<Value> {
    let (md, html) = markdown_file(file)?;
    let html = with_notice(html, ai);
    let would = json!({"team": team, "channel": channel, "subject": subject, "text": md});
    if !yes {
        return Ok(not_sent(would));
    }
    not_in_run()?;
    let mut body = json!({"body": {"contentType": "html", "content": html}});
    if let Some(s) = subject {
        body["subject"] = json!(s);
    }
    let v = graph::request("POST", &format!("/teams/{}/channels/{}/messages", enc(team), enc(channel)), Body::Json(&body), None, &[])?;
    Ok(json!({"sent": true, "id": v["id"], "url": v["webUrl"]}))
}

// ── people, OneDrive, anything else ──────────────────────────────────────────────────────────

/// People the user works with, by name or address (the People API: colleagues with title and department; outsiders
/// they mailed, with what Microsoft 365 knows).
pub fn people(query: &str, limit: usize) -> Result<Value> {
    let v = graph::request("GET", &format!("/me/people?$search={}&$top={}&$select=displayName,scoredEmailAddresses,userPrincipalName,jobTitle,department,companyName,officeLocation",
        enc(&format!("\"{}\"", query.replace('"', ""))), limit.clamp(1, 25)), Body::None, None, &[])?;
    Ok(json!(v["value"].as_array().into_iter().flatten().map(|p| json!({
        "name": p["displayName"], "emails": p["scoredEmailAddresses"].as_array().into_iter().flatten().filter_map(|e| e["address"].as_str()).collect::<Vec<_>>(),
        "title": p["jobTitle"], "department": p["department"], "company": p["companyName"], "office": p["officeLocation"],
    })).collect::<Vec<_>>()))
}

fn drive_item(i: &Value) -> Value {
    json!({"id": i["id"], "name": i["name"], "folder": i["folder"].is_object(), "size": i["size"], "at": i["lastModifiedDateTime"], "url": i["webUrl"],
           "ref": i["parentReference"]["driveId"].as_str().zip(i["id"].as_str()).map(|(d, x)| format!("drives/{d}/items/{x}"))})
}

/// What a OneDrive folder holds (`path` from the root; none: the root).
pub fn drive_list(path: Option<&str>) -> Result<Value> {
    let p = match path.map(|p| p.trim_matches('/')).filter(|p| !p.is_empty()) {
        Some(p) => format!("/me/drive/root:/{}:/children", enc_path(p)),
        None => "/me/drive/root/children".into(),
    };
    let v = graph::request("GET", &format!("{p}?$top=200&$select=id,name,size,folder,file,lastModifiedDateTime,webUrl,parentReference"), Body::None, None, &[])?;
    Ok(json!(v["value"].as_array().into_iter().flatten().map(drive_item).collect::<Vec<_>>()))
}

/// Where an item's bytes are: a search hit's `ref` (`drives/<d>/items/<i>`, SharePoint too), a OneDrive path
/// (`Documents/plan.docx`), or an item id of the user's own drive.
fn content_path(item: &str) -> String {
    if item.starts_with("drives/") {
        format!("/{}/content", item.split('/').map(enc).collect::<Vec<_>>().join("/"))
    } else if looks_like_item_id(item) {
        format!("/me/drive/items/{}/content", enc(item))
    } else {
        format!("/me/drive/root:/{}:/content", enc_path(item))
    }
}

/// A drive item's id rather than a name: personal OneDrive's `<hex>!<n>`, or a business id (`01` + 32 capitals and
/// digits). Anything else is a path, a root file without an extension (`README`) too.
fn looks_like_item_id(s: &str) -> bool {
    !s.contains('/')
        && (s.split_once('!').is_some_and(|(a, b)| !a.is_empty() && a.bytes().all(|c| c.is_ascii_hexdigit()) && !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()))
            || (s.len() >= 30 && s.starts_with("01") && s.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())))
}

/// A file's bytes into `out` (a folder: its name kept).
pub fn drive_download(item: &str, out: &Path) -> Result<Value> {
    if out.as_os_str() == "-" {
        bail!("--out is a file or a folder (not stdout)");
    }
    let target = if out.is_dir() {
        // Its own name, from its metadata (the same path without /content).
        let path = content_path(item);
        let meta = graph::request("GET", &format!("{}?$select=name", path.trim_end_matches("/content")), Body::None, None, &[])?;
        let name = meta["name"].as_str().filter(|n| !n.contains(['/', '\\']) && !n.starts_with('.')).unwrap_or("download");
        out.join(name)
    } else {
        out.to_path_buf()
    };
    // Into a file beside it, renamed over the target only once the whole answer came: a refused or broken download
    // never replaces (or removes) what was there.
    let part = target.with_file_name(format!(".{}.echo-part", target.file_name().and_then(|n| n.to_str()).unwrap_or("download")));
    let got = graph::request("GET", &content_path(item), Body::None, Some(&part), &[]);
    if let Err(e) = got {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    std::fs::rename(&part, &target).with_context(|| format!("saving {}", target.display()))?;
    Ok(json!({"saved": target, "bytes": std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0)}))
}

/// A local file into OneDrive at `path` (replacing one of that name), up to 250 MB.
pub fn drive_upload(local: &Path, path: &str, yes: bool) -> Result<Value> {
    let size = std::fs::metadata(local).with_context(|| format!("reading {}", local.display()))?.len();
    if size > 250 * 1024 * 1024 {
        bail!("{} is over 250 MB", local.display());
    }
    let would = json!({"file": local, "bytes": size, "to": format!("OneDrive/{}", path.trim_matches('/'))});
    if !yes {
        return Ok(not_sent(would));
    }
    not_in_run()?;
    let v = graph::request("PUT", &format!("/me/drive/root:/{}:/content", enc_path(path)), Body::File(local), None, &[])?;
    Ok(json!({"sent": true, "item": drive_item(&v)}))
}

/// Any Graph GET (`path` after `/v1.0`), read only: what the commands above don't cover (calendar details, a site,
/// a task list).
pub fn get(path: &str) -> Result<Value> {
    if !path.starts_with('/') || path.contains("..") {
        bail!("a Graph path after /v1.0, starting with / (for example /me/todo/lists)");
    }
    // Advanced queries ($search, $count on directory objects) need it; elsewhere Graph may refuse it.
    let advanced = path.contains("$search") || path.contains("$count");
    graph::request("GET", path, Body::None, None, if advanced { &["ConsistencyLevel: eventual"] } else { &[] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_read_the_same_whatever_their_kind() {
        let h = json!({"summary": "the <c0>pilot</c0> moves<ddd/>", "hitId": "m1", "resource": {"subject": "Pilot", "from": {"emailAddress": {"name": "Jane", "address": "jane@acme.example"}},
                       "receivedDateTime": "2026-11-03T10:00:00Z", "conversationId": "c1", "webLink": "https://outlook/m1"}});
        let m = hit("mail", &h);
        assert_eq!(m["summary"], "the pilot moves…");
        assert_eq!(m["from"]["email"], "jane@acme.example");
        assert_eq!(m["kind"], "mail");
        let f = hit("files", &json!({"summary": "", "resource": {"id": "i1", "name": "plan.docx", "parentReference": {"driveId": "d1"}}}));
        assert_eq!(f["ref"], "drives/d1/items/i1");
    }

    #[test]
    fn drive_paths_and_items_are_told_apart_and_encoded() {
        assert_eq!(content_path("drives/d!1/items/x"), "/drives/d%211/items/x/content");
        assert_eq!(content_path("Documents/Q4 plan.docx"), "/me/drive/root:/Documents/Q4%20plan.docx:/content");
        assert_eq!(content_path("01ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"), "/me/drive/items/01ABCDEFGHIJKLMNOPQRSTUVWXYZ234567/content");
        assert_eq!(content_path("D4648F06C91D9D3D!54927"), "/me/drive/items/D4648F06C91D9D3D%2154927/content");
        assert_eq!(content_path("README"), "/me/drive/root:/README:/content", "a root file with no extension is a path");
        assert_eq!(content_path("../../x"), "/me/drive/root:/%2E%2E/%2E%2E/x:/content", "dot segments never resolve");
        assert_eq!(enc(".."), "%2E%2E");
    }

    #[test]
    fn nothing_is_sent_without_yes() {
        let f = std::env::temp_dir().join(format!("echo-test-m365-{}.md", std::process::id()));
        std::fs::write(&f, "Hi **Bob**").unwrap();
        // No sign-in is touched: the preview comes first.
        let v = mail_send(&["bob@acme.example".into()], &[], "Hello", &f, Some("sonnet"), false).unwrap();
        assert_eq!(v["sent"], false);
        assert_eq!(v["would"]["to"][0]["emailAddress"]["address"], "bob@acme.example");
        assert!(mail_send(&["not an address".into()], &[], "x", &f, None, false).is_err());
        assert!(with_notice("<p>x</p>".into(), Some("sonnet")).contains("sonnet"), "a model's text says so");
        assert_eq!(with_notice("<p>x</p>".into(), None), "<p>x</p>");
        let u = drive_upload(&f, "Documents/x.md", false).unwrap();
        assert_eq!(u["sent"], false);
        let _ = std::fs::remove_file(&f);
    }

    #[test]
    fn get_takes_a_graph_path_only() {
        assert!(get("me").is_err());
        assert!(get("/me/../x").is_err());
    }
}

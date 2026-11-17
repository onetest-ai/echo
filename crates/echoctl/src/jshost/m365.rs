//! A plugin's Microsoft 365, read-only, through Echo's own sign-in (`ctx.m365`, `uses.m365.read`: `mail`, `chats`):
//! search, a mail's thread with its attachments' names, a chat's messages over some days. Nothing here sends, and no
//! plugin signs in by itself: without Echo's sign-in each says so.

use anyhow::{anyhow, bail, Result};
use chrono::{Local, NaiveDate, TimeZone, Utc};
use serde_json::{json, Value};

use crate::graph;

/// What `uses.m365.read` may name: search and read mail, search and read Teams chats.
pub(super) const READS: [&str; 2] = ["mail", "chats"];

fn granted(id: &str) -> Result<Vec<String>> {
    Ok(super::uses(id, "m365")?["read"].as_array().into_iter().flatten().filter_map(|k| k.as_str()).filter(|k| READS.contains(k)).map(str::to_owned).collect())
}

fn may(id: &str, what: &str) -> Result<()> {
    if !granted(id)?.iter().any(|k| k == what) {
        bail!("{id} didn't ask to read your Microsoft 365 {what} when it was installed");
    }
    signed_in()
}

fn signed_in() -> Result<()> {
    if graph::account().is_none() {
        bail!("Microsoft 365 isn't connected: sign in to it in Echo's Settings → Calendar & chat");
    }
    Ok(())
}

/// `ctx.m365.search(query, {kinds, days, limit})`: Microsoft 365's search over the kinds it asked for (`mail`,
/// `chats`; all of its own by default) in the last `days` (90 by default, 1–365), `limit` a kind (25, 1–50).
pub(super) fn search(id: &str, query: &str, opts: &Value) -> Result<Value> {
    let mine = granted(id)?;
    let kinds: Vec<String> = match opts["kinds"].as_array() {
        Some(a) => a.iter().map(|k| k.as_str().unwrap_or("").to_string()).collect(),
        None => mine.clone(),
    };
    if kinds.is_empty() {
        bail!("{id} didn't ask to read your Microsoft 365 mail or chats when it was installed");
    }
    if let Some(k) = kinds.iter().find(|k| !mine.contains(k)) {
        bail!("{id} didn't ask to read your Microsoft 365 {k} when it was installed");
    }
    signed_in()?;
    if query.trim().is_empty() {
        return Ok(json!({"query": query, "results": [], "errors": null}));
    }
    let days = opts["days"].as_i64().unwrap_or(90).clamp(1, 365);
    let limit = opts["limit"].as_u64().unwrap_or(25).clamp(1, 50) as usize;
    crate::m365::search(query, &kinds, limit, None, Some(days))
}

/// `ctx.m365.mailThread(message)`: the thread of a mail (its Graph id), oldest first, each message with its
/// attachments' names (never their contents).
pub(super) fn mail_thread(id: &str, message: &str) -> Result<Value> {
    may(id, "mail")?;
    let picked = graph::message(message)?;
    let conv = picked["conversation"].as_str().ok_or_else(|| anyhow!("that mail has no thread"))?;
    Ok(json!({"subject": picked["subject"], "picked": picked["id"], "messages": graph::conversation_with_files(conv)?}))
}

/// `ctx.m365.chat(chat, {since, until})`: what was written in a Teams chat on those days (local, both included;
/// `until` today and `since` six days before by default), oldest first, about 400 messages at most: `{id, topic, members,
/// url, since, until, messages: [{id, at, from, text, files}]}`.
pub(super) fn chat(id: &str, chat: &str, opts: &Value) -> Result<Value> {
    may(id, "chats")?;
    if chat.trim().is_empty() {
        bail!("which chat? (its id, from search or a mail's thread)");
    }
    let day = |k: &str| -> Result<Option<NaiveDate>> {
        opts[k].as_str().filter(|s| !s.is_empty()).map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| anyhow!("{k}: “{s}” isn't a day (YYYY-MM-DD)"))).transpose()
    };
    let until = day("until")?.unwrap_or_else(|| Local::now().date_naive());
    let since = day("since")?.unwrap_or(until - chrono::Duration::days(6));
    if since > until {
        bail!("the days end before they start");
    }
    let start = |d: NaiveDate| Local.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default()).earliest().map(|t| t.with_timezone(&Utc)).ok_or_else(|| anyhow!("{d} isn't a day here"));
    let (from, to) = (start(since)?, start(until + chrono::Duration::days(1))?);
    let info = graph::request("GET", &format!("/chats/{}?$expand=members", crate::m365::enc(chat)), graph::Body::None, None, &[])?;
    let members: Vec<Value> = info["members"].as_array().into_iter().flatten().map(|m| json!({"name": m["displayName"], "email": m["email"]})).collect();
    let messages: Vec<Value> = graph::chat_messages(chat, from, to)?
        .iter()
        .map(|m| {
            let content = m["body"]["content"].as_str().unwrap_or("");
            let text = if m["body"]["contentType"].as_str().is_some_and(|t| t.eq_ignore_ascii_case("text")) { content.trim().to_string() } else { graph::html_to_text(content) };
            json!({"id": m["id"], "at": m["createdDateTime"], "from": m["from"]["user"]["displayName"].as_str().or(m["from"]["application"]["displayName"].as_str()), "text": text,
                   "files": m["attachments"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).collect::<Vec<_>>()})
        })
        .collect();
    Ok(json!({"id": chat, "topic": info["topic"], "members": members, "url": info["webUrl"], "since": since.to_string(), "until": until.to_string(), "messages": messages}))
}

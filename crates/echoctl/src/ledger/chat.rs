//! The ledger: the meeting chat between calls (split out of `ledger.rs`, moved as it was).

use super::*;

// ── the meeting chat between calls ───────────────────────────────────────────────────────────

/// How far back a chat's first read goes, and how far each later read looks again (a sender trusted
/// meanwhile, a message edited): its days are rebuilt and an item that changed is read again.
pub(super) const CHAT_FIRST_DAYS: i64 = 30;
pub(super) const CHAT_AGAIN_DAYS: i64 = 7;
/// Messages taken at most a read (the newest).
pub(super) const CHAT_MAX: usize = 2000;
/// Messages this close to one of this thread's calls on the same chat belong to that call's item.
pub(super) const CALL_MARGIN_MIN: i64 = 2;
/// A chat is read at most this often (the sync looks every 5 minutes).
pub const CHAT_EVERY_MIN: i64 = 30;

pub(super) fn chats_file(project: &str) -> PathBuf {
    crate::projects::dir().join(project).join("ledger").join("chats.json")
}

/// Per chat: `{read_at, found, days, error, checked_at}` (the Sources chip shows it).
pub fn chat_state(project: &str) -> Value {
    fs::read(chats_file(project)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

pub(super) fn short(id: &str) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(id.as_bytes())[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// Whether one of the project's meeting chats is due a read.
pub fn chat_due(p: &crate::projects::Project) -> bool {
    let st = chat_state(&p.id);
    p.sources.iter().filter(|s| s.kind == "teams_chat").any(|s| {
        st[&s.id]["checked_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_none_or(|t| Local::now().signed_duration_since(t) >= chrono::Duration::minutes(CHAT_EVERY_MIN))
    })
}

/// Who may speak into the work through a chat: the user and colleagues, and senders trusted for the
/// thread's client (the Brain's list); the others wait in that list. A Teams author not looked up yet
/// holds the day back. Without the Brain built in, everyone in the chat (as for a call's own chat).
pub(super) struct ChatTrust {
    #[cfg(feature = "brain")]
    snd: crate::brain::senders::Senders,
    #[cfg(feature = "brain")]
    lookups: usize,
    #[cfg(feature = "brain")]
    client: String,
    #[cfg(feature = "brain")]
    name: String,
}

#[derive(PartialEq)]
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub(super) enum Said {
    Read,
    Skip,
    Unknown,
}

impl ChatTrust {
    #[allow(unused_variables)]
    fn new(p: &crate::projects::Project, name: &str) -> ChatTrust {
        #[cfg(feature = "brain")]
        {
            let mut snd = crate::brain::senders::load();
            snd.refresh_org();
            ChatTrust { snd, lookups: 0, client: p.client.clone().unwrap_or_else(|| "-".into()), name: name.to_string() }
        }
        #[cfg(not(feature = "brain"))]
        ChatTrust {}
    }

    #[allow(unused_variables)]
    fn judge(&mut self, m: &Value) -> Said {
        #[cfg(feature = "brain")]
        {
            use crate::brain::senders::Verdict;
            let Some(id) = m["from"]["user"]["id"].as_str() else { return Said::Skip }; // bots, apps
            let key = format!("teams:{id}");
            let who = m["from"]["user"]["displayName"].as_str().unwrap_or("?");
            if self.snd.unknown_author(&key) && self.lookups < 40 {
                self.lookups += 1;
                self.snd.learn_author(&key, who);
            }
            if self.snd.unknown_author(&key) {
                return Said::Unknown;
            }
            match self.snd.teams(&self.client, &key) {
                Verdict::Trusted => Said::Read,
                Verdict::Ignored => Said::Skip,
                Verdict::Waiting | Verdict::Unverified => {
                    let at = m["createdDateTime"].as_str().unwrap_or("");
                    self.snd.wait(&self.client, &key, who, "teams", m["id"].as_str().unwrap_or(""), at, &format!("💬 {}", self.name), false, None);
                    Said::Skip
                }
            }
        }
        #[cfg(not(feature = "brain"))]
        Said::Read
    }

    fn save(self) {
        #[cfg(feature = "brain")]
        {
            let mut s = self.snd;
            let _ = crate::brain::senders::save_merged(&mut s);
        }
    }
}

/// The thread's calls on this chat: when each was, widened by the margin (their messages are theirs).
pub(super) fn call_windows(p: &crate::projects::Project, chat: &str) -> Vec<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> {
    let t = |v: &Value| v.as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&chrono::Utc));
    crate::projects::calls_of(p)
        .unwrap_or_default()
        .into_iter()
        .filter(|(d, _)| fs::read_to_string(d.join("chat.id")).is_ok_and(|c| c.trim() == chat))
        .filter_map(|(d, _)| {
            let m = calls::read_json(&d.join("meta.json"));
            let a = t(&m["started_at"])?;
            let b = t(&m["ended_at"]).unwrap_or(a + chrono::Duration::hours(3));
            Some((a - chrono::Duration::minutes(CALL_MARGIN_MIN), b + chrono::Duration::minutes(CALL_MARGIN_MIN)))
        })
        .collect()
}

/// A day of a meeting chat between the thread's calls as one evidence item (`chat-<chat>-<day>`):
/// turns at their local time, files named in the text.
pub(super) fn chat_item(chat: &str, name: &str, day: chrono::NaiveDate, msgs: &[&Value]) -> Option<Value> {
    let local = |m: &Value| m["createdDateTime"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|d| d.with_timezone(&Local));
    let mut people: Vec<String> = vec![];
    let mut files: Vec<String> = vec![];
    let turns: Vec<Value> = msgs
        .iter()
        .filter_map(|m| {
            let text = crate::graph::html_to_text(m["body"]["content"].as_str().unwrap_or(""));
            let att: Vec<String> = m["attachments"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str().map(str::to_owned)).collect();
            if text.trim().is_empty() && att.is_empty() {
                return None;
            }
            let who = m["from"]["user"]["displayName"].as_str().unwrap_or("?").to_string();
            if !people.contains(&who) {
                people.push(who.clone());
            }
            files.extend(att.iter().map(|a| format!("{who} shared {a}")));
            let text = if text.trim().is_empty() { format!("(shared {})", att.join(", ")) } else { text };
            Some(json!({"ts": local(m)?.format("%H:%M:%S").to_string(), "speaker": who, "text": text.trim()}))
        })
        .collect();
    if turns.is_empty() {
        return None;
    }
    let first = msgs.iter().find_map(|m| local(m))?;
    let text = if files.is_empty() { String::new() } else { format!("## Files shared\n{}", files.join("\n")) };
    Some(json!({
        "id": format!("chat-{}-{day}", short(chat)), "kind": "chat", "source": "teams_chat", "chat": chat,
        "at": first.to_rfc3339(), "date": day.to_string(), "title": format!("{name} · chat on {day}"),
        "people": people, "turns": turns, "text": text,
    }))
}

/// Read the project's meeting chats (`teams_chat` sources) between its calls into evidence: one item
/// a day, only the trusted senders' messages, a day taken once the chat has been quiet for
/// `projectsync::SETTLE` (today's grows: an item that changed is read again). Returns the ids
/// written; a chat that can't be read keeps its error in `chats.json` and doesn't stop the others.
pub fn chat_sync(project: &str) -> Result<Vec<String>> {
    let p = crate::projects::load(project)?;
    let chats: Vec<crate::projects::Source> = p.sources.iter().filter(|s| s.kind == "teams_chat").cloned().collect();
    if chats.is_empty() {
        return Ok(vec![]);
    }
    let l = Ledger::of(project)?;
    let mut state = chat_state(project);
    let now = chrono::Utc::now();
    let mut written = vec![];
    for s in &chats {
        let name = s.name.clone().unwrap_or_else(|| "Meeting chat".into());
        let first = state[&s.id]["read_at"].is_null();
        let mut since = now - chrono::Duration::days(if first { CHAT_FIRST_DAYS } else { CHAT_AGAIN_DAYS });
        if let Some(t) = p.since.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).and_then(|d| d.and_hms_opt(0, 0, 0)).and_then(|d| d.and_local_timezone(Local).single()) {
            since = since.max(t.with_timezone(&chrono::Utc));
        }
        state[&s.id]["checked_at"] = json!(Local::now().to_rfc3339());
        let msgs = match crate::graph::chat_history(&s.id, since, now, CHAT_MAX) {
            Ok(m) => m,
            Err(e) => {
                state[&s.id]["error"] = json!(format!("{e:#}"));
                continue;
            }
        };
        let windows = call_windows(&p, &s.id);
        let mut trust = ChatTrust::new(&p, &name);
        let mut days: BTreeMap<chrono::NaiveDate, Vec<&Value>> = BTreeMap::new();
        let mut held: std::collections::BTreeSet<chrono::NaiveDate> = Default::default();
        for m in &msgs {
            let Some(at) = m["createdDateTime"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|d| d.with_timezone(&chrono::Utc)) else { continue };
            if windows.iter().any(|(a, b)| at >= *a && at <= *b) {
                continue; // the call's own item has it
            }
            let day = at.with_timezone(&Local).date_naive();
            match trust.judge(m) {
                Said::Read => days.entry(day).or_default().push(m),
                Said::Skip => {}
                Said::Unknown => {
                    held.insert(day);
                }
            }
        }
        trust.save();
        let settle = crate::projectsync::SETTLE;
        let _lock = l.lock()?;
        let done = processed(&l.events()?);
        let mut requeue = vec![];
        let mut found = 0;
        for (day, ms) in &days {
            let last = ms.iter().filter_map(|m| m["createdDateTime"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())).max();
            if held.contains(day) || last.is_some_and(|t| now.signed_duration_since(t.with_timezone(&chrono::Utc)) < settle) {
                continue; // a sender not looked up yet, or the chat still going: next time
            }
            let Some(item) = chat_item(&s.id, &name, *day, ms) else { continue };
            let id = item["id"].as_str().unwrap_or("").to_string();
            let path = l.dir.join("evidence").join(format!("{id}.json"));
            let body = serde_json::to_vec_pretty(&item)?;
            if fs::read(&path).ok().as_deref() == Some(body.as_slice()) {
                continue;
            }
            fs::create_dir_all(l.dir.join("evidence"))?;
            fs::write(l.evidence_md(&id), render_md(&item))?;
            crate::paths::write_atomic(&path, &body)?;
            if done.contains(&id) {
                requeue.push(json!({"type": "reprocess", "evidence": id, "why": "the chat changed", "at": Local::now().to_rfc3339()}));
            }
            found += 1;
            written.push(id);
        }
        l.append(&requeue)?;
        state[&s.id]["read_at"] = json!(Local::now().to_rfc3339());
        state[&s.id]["found"] = json!(found);
        state[&s.id]["days"] = json!(days.len());
        if let Some(o) = state[&s.id].as_object_mut() {
            o.remove("error");
        }
    }
    // Chats no longer watched forget their state (a source added back reads its 30 days again).
    if let Some(o) = state.as_object_mut() {
        o.retain(|k, _| chats.iter().any(|s| &s.id == k));
    }
    fs::create_dir_all(l.dir.clone())?;
    crate::paths::write_atomic(&chats_file(project), serde_json::to_string_pretty(&state)?.as_bytes())?;
    Ok(written)
}

/// The meeting chats of a thread's calls it doesn't read yet: `[{id, name, calls}]`, the most used first.
pub fn chat_suggestions(p: &crate::projects::Project) -> Vec<Value> {
    let mut by: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (d, _) in crate::projects::calls_of(p).unwrap_or_default() {
        let Some(c) = fs::read_to_string(d.join("chat.id")).ok().map(|c| c.trim().to_string()).filter(|c| c.starts_with("19:")) else { continue };
        if p.sources.iter().any(|s| s.kind == "teams_chat" && s.id == c) {
            continue;
        }
        let title = calls::read_json(&d.join("meta.json"))["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("Meeting chat").to_string();
        let e = by.entry(c).or_insert((title, 0));
        e.1 += 1;
    }
    let mut v: Vec<Value> = by.into_iter().map(|(id, (name, n))| json!({"id": id, "name": name, "calls": n})).collect();
    v.sort_by_key(|x| std::cmp::Reverse(x["calls"].as_u64().unwrap_or(0)));
    v
}

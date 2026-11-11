//! The ledger: mail, the sync and its pending evidence (split out of `ledger.rs`, moved as it was).

use super::*;

// ── mail ────────────────────────────────────────────────────────────────────────────────────────

/// A mail as one evidence item (`mail-<sha8 of its Internet message id>`: Graph's own id changes when
/// the mail moves to another folder): the new part of its text as one turn at the local time it
/// came, the sender as the speaker, and its link in Outlook (`url`).
pub(super) fn mail_item(m: &Value) -> Option<Value> {
    if m["invite"] == true {
        return None; // a meeting's invite: the meeting is the call
    }
    let key = m["internet_id"].as_str().or(m["id"].as_str())?;
    let at = m["at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())?.with_timezone(&Local);
    let text = m["text"].as_str().map(str::trim).filter(|t| !t.is_empty())?;
    let me = mail_me();
    // The user is "Me", as in a call; names as people read them ("Morgan, Alex" → "Alex Morgan").
    let name = |p: &Value| {
        let email = p["email"].as_str().unwrap_or("").to_lowercase();
        if !email.is_empty() && me(&email) {
            return "Me".to_string();
        }
        let n = p["name"].as_str().map(crate::people::display).filter(|n| !n.trim().is_empty());
        n.unwrap_or_else(|| p["email"].as_str().unwrap_or("?").trim().to_string())
    };
    let from = name(&m["from"]);
    let mut people = vec![from.clone()];
    for p in m["to"].as_array().into_iter().flatten().map(name) {
        if !people.contains(&p) {
            people.push(p);
        }
    }
    let subject = m["subject"].as_str().map(str::trim).filter(|t| !t.is_empty()).unwrap_or("(no subject)");
    Some(json!({
        "id": format!("mail-{}", short(key)), "kind": "mail", "source": "mail", "message": m["id"], "conversation": m["conversation"],
        "at": at.to_rfc3339(), "date": at.date_naive().to_string(), "title": subject, "from": m["from"], "to": m["to"], "auth": m["auth"],
        "url": m["url"].as_str().filter(|u| u.starts_with("https://")),
        "people": people, "turns": [{"ts": at.format("%H:%M:%S").to_string(), "speaker": from, "text": text}], "text": "",
    }))
}

/// A reply to mail the user sent from an item, as evidence that says which item it answers (`answers`): the sync is
/// told, and clears or updates what the item waits for.
pub(super) fn answering(mut item: Value, node: &str, title: &str) -> Value {
    item["answers"] = json!({"node": node, "title": title});
    item
}

/// Whether an address is the user's: the one in Settings → About me, or (with the Brain) the signed-in
/// account's local part at any of their domains.
pub(super) fn mail_me() -> impl Fn(&str) -> bool {
    let mine = crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| m.email).map(|e| e.to_lowercase());
    #[cfg(feature = "brain")]
    let local = crate::brain::sources::me_local();
    #[cfg(not(feature = "brain"))]
    let local: Option<String> = None;
    move |e: &str| Some(e) == mine.as_deref() || local.as_deref().is_some_and(|l| e.split('@').next() == Some(l))
}

/// Which of a read's mails are about the thread, by its source's subjects: a mail whose subject contains one, and
/// any other mail of a conversation such a mail is in (a reply that changed the subject), or one the thread already
/// holds mail of (`known`). A source naming no subjects keeps them all.
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub(super) fn about_thread(found: &[Value], src: &crate::projects::Source, known: &BTreeSet<String>) -> Vec<bool> {
    if src.subject.is_empty() {
        return vec![true; found.len()];
    }
    let mut convs = known.clone();
    convs.extend(found.iter().filter(|m| src.about(m["subject"].as_str().unwrap_or(""))).filter_map(|m| m["conversation"].as_str().map(str::to_owned)));
    found.iter().map(|m| src.about(m["subject"].as_str().unwrap_or("")) || m["conversation"].as_str().is_some_and(|c| convs.contains(c))).collect()
}

/// How far back a mail source's first read goes, and how far each later read looks again.
#[cfg(feature = "brain")]
pub(super) const MAIL_FIRST_DAYS: i64 = 30;
#[cfg(feature = "brain")]
pub(super) const MAIL_AGAIN_DAYS: i64 = 7;
/// Mails taken at most a read (the newest).
#[cfg(feature = "brain")]
pub(super) const MAIL_MAX: usize = 200;

pub(super) fn mails_file(project: &str) -> PathBuf {
    crate::projects::dir().join(project).join("ledger").join("mails.json")
}

/// Each ✉ Mail source's read into the work: `{<rule>: {read_at, checked_at, found, error}}`.
pub fn mail_state(project: &str) -> Value {
    calls::read_json(&mails_file(project)).as_object().map(|o| Value::Object(o.clone())).unwrap_or_else(|| json!({}))
}

/// How long a mail sent from an item is followed for its replies.
pub const SENT_FOLLOW_DAYS: i64 = 30;

/// `<thread>/<node>` (`m365 mail send|reply --task`): the thread and its item, checked to be there.
pub fn task_ref(r: &str) -> Result<(String, String)> {
    let (p, n) = r.split_once('/').filter(|(p, n)| !p.is_empty() && !n.is_empty()).ok_or_else(|| anyhow!("--task takes <thread>/<item>, e.g. acme-rollout/T003"))?;
    let t = fold(&Ledger::of(p)?)?;
    let id = t.nodes.get(n).map(|x| x.id.clone()).ok_or_else(|| anyhow!("no item {n} in thread {p}"))?;
    Ok((p.to_string(), id))
}

/// Mail the user sent from an item (#92): its conversation is followed, its replies come back to the item.
pub fn mail_sent(project: &str, node: &str, conversation: &str, subject: &str, to: &[String]) -> Result<()> {
    anyhow::ensure!(!conversation.trim().is_empty(), "Microsoft 365 gave no conversation to follow");
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    anyhow::ensure!(fold(&l)?.nodes.contains_key(node), "no item {node} in thread {project}");
    l.append(&[json!({"type": "sent", "node": node, "conversation": conversation, "subject": subject, "to": to, "at": Local::now().to_rfc3339()})])
}

/// The conversations the thread follows for replies: its open items' sent mails of the last `SENT_FOLLOW_DAYS`, one
/// a conversation — (state key, item, its title, conversation, when it was sent).
pub(super) fn followed(project: &str) -> Vec<(String, String, String, String, String)> {
    let Ok(t) = Ledger::of(project).and_then(|l| fold(&l)) else { return vec![] };
    let since = Local::now() - chrono::Duration::days(SENT_FOLLOW_DAYS);
    let mut out: Vec<(String, String, String, String, String)> = vec![];
    for x in t.nodes.values().filter(|x| !closed(&x.state)) {
        for m in &x.sent {
            let (Some(c), Some(at)) = (m["conversation"].as_str(), m["at"].as_str()) else { continue };
            let recent = chrono::DateTime::parse_from_rfc3339(at).is_ok_and(|t| t > since);
            if recent && !out.iter().any(|f| f.3 == c) {
                out.push((format!("sent:{}", short(c)), x.id.clone(), x.title.clone(), c.to_string(), at.to_string()));
            }
        }
    }
    out
}

/// The replies in a followed conversation: its mails after the user's went, none of them the user's, no invites.
#[cfg_attr(not(feature = "brain"), allow(dead_code))]
pub(super) fn replies<'a>(msgs: &'a [Value], at: &str, mine: &dyn Fn(&str) -> bool) -> Vec<&'a Value> {
    let Ok(sent) = chrono::DateTime::parse_from_rfc3339(at) else { return vec![] };
    msgs.iter()
        .filter(|m| m["invite"] != true)
        .filter(|m| m["at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_some_and(|t| t > sent))
        .filter(|m| !mine(&m["from"]["email"].as_str().unwrap_or("").to_lowercase()))
        .collect()
}

/// A mail source not checked for `CHAT_EVERY_MIN` (or never).
pub fn mail_due(p: &crate::projects::Project) -> bool {
    // Without the Brain built in mail isn't read: never due (it would be every tick).
    if !cfg!(feature = "brain") {
        return false;
    }
    let st = mail_state(&p.id);
    let due = |k: &str| st[k]["checked_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_none_or(|t| Local::now().signed_duration_since(t) >= chrono::Duration::minutes(CHAT_EVERY_MIN));
    p.sources.iter().filter(|s| s.kind == "mail").any(|s| due(&s.id)) || followed(&p.id).iter().any(|f| due(&f.0))
}

/// Read the project's ✉ Mail sources (`domain:` / `person:` rules, as the Brain reads them) into its
/// evidence: one item a mail, only from the user, colleagues and senders trusted for the thread's
/// client — a trusted outside sender's only when Exchange says it is really theirs (DMARC); the
/// others wait in the Brain's senders list (Needs you), never read meanwhile. The sync runs Claude
/// Code with tools on what it reads, hence the same rules as the Brain's quarantine. Returns the ids
/// written; a source that can't be read keeps its error in `mails.json`.
#[cfg(feature = "brain")]
pub fn mail_sync(project: &str) -> Result<Vec<String>> {
    use crate::brain::senders::{self, Verdict};
    use crate::brain::sources::{me_local, not_me, MailRule, MAX_PEOPLE};
    let p = crate::projects::load(project)?;
    let rules: Vec<crate::projects::Source> = p.sources.iter().filter(|s| s.kind == "mail").cloned().collect();
    let follow = followed(project);
    if rules.is_empty() && follow.is_empty() {
        return Ok(vec![]);
    }
    let l = Ledger::of(project)?;
    let mut state = mail_state(project);
    let now = chrono::Utc::now();
    let mut snd = senders::load();
    snd.refresh_org();
    let mut items: Vec<Value> = vec![];
    // The conversations the thread already holds mail of: their replies are its, whatever their subject.
    let known: BTreeSet<String> = l.evidence().values().filter(|e| e["kind"] == "mail").filter_map(|e| e["conversation"].as_str().map(str::to_owned)).collect();
    if snd.org_domains.is_empty() {
        // Nobody can be told a colleague: nothing is judged, every source reads this window next time.
        for k in rules.iter().map(|s| s.id.as_str()).chain(follow.iter().map(|f| f.0.as_str())) {
            state[k]["checked_at"] = json!(Local::now().to_rfc3339());
            state[k]["error"] = json!("couldn't read your organisation's domains: mail waits");
        }
    } else {
        let me = me_local();
        let client = p.client.clone().unwrap_or_else(|| "-".into());
        for s in &rules {
            state[&s.id]["checked_at"] = json!(Local::now().to_rfc3339());
            let Some(rule) = MailRule::parse(&s.id) else {
                state[&s.id]["error"] = json!(format!("not a mail rule: {}", s.id));
                continue;
            };
            let first = state[&s.id]["read_at"].is_null();
            let mut since = now - chrono::Duration::days(if first { MAIL_FIRST_DAYS } else { MAIL_AGAIN_DAYS });
            if let Some(t) = p.since.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).and_then(|d| d.and_hms_opt(0, 0, 0)).and_then(|d| d.and_local_timezone(Local).single()) {
                since = since.max(t.with_timezone(&chrono::Utc));
            }
            let found = match crate::graph::mail(&rule.kql(&s.subject), since, MAIL_MAX) {
                Ok(f) => f,
                Err(e) => {
                    state[&s.id]["error"] = json!(format!("{e:#}"));
                    continue;
                }
            };
            let mut n = 0;
            let about = about_thread(&found, s, &known);
            for (m, _) in found.iter().zip(&about).filter(|(_, a)| **a) {
                let people = not_me(std::iter::once(&m["from"]).chain(m["to"].as_array().into_iter().flatten()), me.as_deref());
                // Graph's search is fuzzy: only mail that really has these people; a broadcast isn't a thread's.
                if m["invite"] == true || people.len() > MAX_PEOPLE || !rule.matches(&people) {
                    continue;
                }
                let from = m["from"]["email"].as_str().unwrap_or("").to_lowercase();
                let who = m["from"]["name"].as_str().unwrap_or(&from).to_string();
                let (auth, id, at, subject) = (m["auth"].as_str().unwrap_or("none"), m["id"].as_str().unwrap_or(""), m["at"].as_str().unwrap_or(""), m["subject"].as_str().unwrap_or("(no subject)"));
                match snd.mail(&client, &from, auth) {
                    Verdict::Trusted => {
                        if let Some(item) = mail_item(m) {
                            n += 1;
                            items.push(item);
                        }
                    }
                    Verdict::Ignored => {}
                    Verdict::Unverified => snd.note_unverified(&client, &from, &who, id, at, subject, auth),
                    Verdict::Waiting => snd.wait(&client, &from, &who, "mail", id, at, subject, auth == "pass", None),
                }
            }
            state[&s.id]["read_at"] = json!(Local::now().to_rfc3339());
            state[&s.id]["found"] = json!(n);
            if let Some(o) = state[&s.id].as_object_mut() {
                o.remove("error");
            }
        }
        // Mail the user sent from an item: every reply in its conversation, whatever its subject or address (no
        // source rule: the user started it), told which item it answers. Who may speak is judged as above (R8).
        let mine = mail_me();
        for (key, node, title, conv, at) in &follow {
            state[key]["checked_at"] = json!(Local::now().to_rfc3339());
            let msgs = match crate::graph::conversation(conv) {
                Ok(m) => m,
                Err(e) => {
                    state[key]["error"] = json!(format!("{e:#}"));
                    continue;
                }
            };
            let mut n = 0;
            for m in replies(&msgs, at, &mine) {
                let from = m["from"]["email"].as_str().unwrap_or("").to_lowercase();
                let who = m["from"]["name"].as_str().unwrap_or(&from).to_string();
                let (auth, id, mat, subject) = (m["auth"].as_str().unwrap_or("none"), m["id"].as_str().unwrap_or(""), m["at"].as_str().unwrap_or(""), m["subject"].as_str().unwrap_or("(no subject)"));
                match snd.mail(&client, &from, auth) {
                    Verdict::Trusted => {
                        if let Some(item) = mail_item(m) {
                            n += 1;
                            items.push(answering(item, node, title));
                        }
                    }
                    Verdict::Ignored => {}
                    Verdict::Unverified => snd.note_unverified(&client, &from, &who, id, mat, subject, auth),
                    Verdict::Waiting => snd.wait(&client, &from, &who, "mail", id, mat, subject, auth == "pass", None),
                }
            }
            state[key]["read_at"] = json!(Local::now().to_rfc3339());
            state[key]["found"] = json!(n);
            state[key]["node"] = json!(node);
            if let Some(o) = state[key].as_object_mut() {
                o.remove("error");
            }
        }
        let _ = senders::save_merged(&mut snd);
    }
    // Sources no longer watched forget their state (one added back reads its 30 days again).
    if let Some(o) = state.as_object_mut() {
        o.retain(|k, _| rules.iter().any(|s| &s.id == k) || follow.iter().any(|f| &f.0 == k));
    }
    let _lock = l.lock()?;
    ensure_first_epic(&l)?;
    let mut written = vec![];
    for item in &items {
        let id = item["id"].as_str().unwrap_or("").to_string();
        let path = l.dir.join("evidence").join(format!("{id}.json"));
        if path.exists() || written.contains(&id) {
            continue; // a mail doesn't change; one the user added by hand is already there
        }
        fs::write(l.evidence_md(&id), render_md(item))?;
        crate::paths::write_atomic(&path, &serde_json::to_vec_pretty(item)?)?;
        written.push(id);
    }
    crate::paths::write_atomic(&mails_file(project), serde_json::to_string_pretty(&state)?.as_bytes())?;
    Ok(written)
}

/// A sender the user trusted for `client` (or whose mail they read anyway): that client's open threads
/// read their mail and meeting chats from the start again at the next sync (due at once) — what was
/// held back comes in; items already there stay as they are.
#[cfg(feature = "brain")]
pub fn reread_sources(client: &str) -> Result<()> {
    for p in crate::projects::list().into_iter().filter(|p| !p.stage.closed() && p.client.as_deref().unwrap_or("-") == client) {
        for f in [mails_file(&p.id), chats_file(&p.id)] {
            let mut st = calls::read_json(&f);
            let Some(o) = st.as_object_mut() else { continue };
            for v in o.values_mut().filter_map(Value::as_object_mut) {
                v.remove("read_at");
                v.remove("checked_at");
            }
            crate::paths::write_atomic(&f, serde_json::to_string_pretty(&st)?.as_bytes())?;
        }
    }
    Ok(())
}

/// A mail source whose subjects changed: the thread reads its first window again at the next sync, by the new words
/// (Recall too: `brain::sources::read_again`). Items already in the thread stay.
pub fn read_mail_again(project: &str, rule: &str) -> Result<()> {
    let f = mails_file(project);
    let mut st = calls::read_json(&f);
    if let Some(o) = st.as_object_mut().and_then(|o| o.get_mut(rule)).and_then(Value::as_object_mut) {
        o.remove("read_at");
        o.remove("checked_at");
        crate::paths::write_atomic(&f, serde_json::to_string_pretty(&st)?.as_bytes())?;
    }
    #[cfg(feature = "brain")]
    crate::brain::sources::read_again("mail", rule)?;
    Ok(())
}

/// Without the Brain built in there is no senders list to trust anyone by: mail isn't read.
#[cfg(not(feature = "brain"))]
pub fn mail_sync(_project: &str) -> Result<Vec<String>> {
    Ok(vec![])
}

/// Mails the user picked (Graph message ids; `conversation`: every message of their conversations
/// too) as evidence of the project: one item a mail, written once — the next sync reads it as it
/// reads a call. The user's own choice is the trust here (a thread's ✉ Mail sources, read by
/// themselves, will take only trusted senders). Returns the items, new or already there.
pub fn add_mail(project: &str, ids: &[String], conversation: bool) -> Result<Vec<Value>> {
    let l = Ledger::of(project)?;
    let mut mails: Vec<Value> = vec![];
    for id in ids {
        let m = crate::graph::message(id).with_context(|| format!("mail {id}"))?;
        match m["conversation"].as_str().filter(|_| conversation) {
            Some(c) => mails.extend(crate::graph::conversation(c)?),
            None => mails.push(m),
        }
    }
    let _lock = l.lock()?;
    ensure_first_epic(&l)?;
    let mut out = vec![];
    for m in &mails {
        let Some(item) = mail_item(m) else { continue };
        let id = item["id"].as_str().unwrap_or("").to_string();
        if out.iter().any(|x: &Value| x["id"] == id.as_str()) {
            continue;
        }
        let path = l.dir.join("evidence").join(format!("{id}.json"));
        let new = !path.exists();
        if new {
            fs::write(l.evidence_md(&id), render_md(&item))?;
            crate::paths::write_atomic(&path, &serde_json::to_vec_pretty(&item)?)?;
        }
        out.push(json!({"id": id, "title": item["title"], "date": item["date"], "from": item["turns"][0]["speaker"], "new": new}));
    }
    Ok(out)
}

/// Bring the project's evidence up to date with its calls: new ones written, changed ones written
/// again (and queued again when they were processed). `limit`: only the newest calls (the backfill).
/// Returns the ids written.
pub fn sync(project: &str, limit: Option<usize>) -> Result<Vec<String>> {
    let l = Ledger::of(project)?;
    let p = crate::projects::load(project)?;
    // Strict routing: the sync runs on the thread's AI profile; a call pinned to another profile never
    // becomes its evidence (nor reaches its people pass).
    let profile = p.ai_profile.clone().unwrap_or_else(crate::ai::default_id);
    let mut dirs: Vec<PathBuf> = crate::projects::calls_of(&p)?
        .into_iter()
        .map(|(d, _)| d)
        .filter(|d| crate::ai::call_profile_id(d).ok().as_deref() == Some(profile.as_str()))
        .collect();
    if let Some(n) = limit {
        dirs = dirs.split_off(dirs.len().saturating_sub(n));
    }
    let _lock = l.lock()?;
    ensure_first_epic(&l)?;
    let evs = l.events()?;
    let done = processed(&evs);
    let mut written = vec![];
    let mut requeue = vec![];
    for d in dirs {
        let Some(item) = call_item(&d, Some(project)) else { continue };
        let id = item["id"].as_str().unwrap_or("").to_string();
        let path = l.dir.join("evidence").join(format!("{id}.json"));
        let body = serde_json::to_vec_pretty(&item)?;
        if fs::read(&path).ok().as_deref() == Some(body.as_slice()) {
            continue;
        }
        fs::write(l.evidence_md(&id), render_md(&item))?;
        crate::paths::write_atomic(&path, &body)?;
        if done.contains(&id) {
            requeue.push(json!({"type": "reprocess", "evidence": id, "why": "the call changed", "at": Local::now().to_rfc3339()}));
        }
        written.push(id);
    }
    l.append(&requeue)?;
    Ok(written)
}

pub(crate) fn ensure_first_epic(l: &Ledger) -> Result<()> {
    if l.events()?.iter().any(|e| e["type"] == "create" && e["kind"] == "epic") {
        return Ok(());
    }
    l.append(&[json!({"type": "create", "mid": mid_of(&["plan", FIRST_EPIC]), "id": "E01", "kind": "epic", "by": "plan",
        "evidence": null, "date": null, "fields": {"title": FIRST_EPIC, "plan": FIRST_EPIC}})])
}

/// Evidence not processed yet, oldest first, with the file the agent reads.
pub fn pending(project: &str) -> Result<Vec<Value>> {
    let l = Ledger::of(project)?;
    let ev = l.evidence();
    let done = processed(&l.events()?);
    Ok(order(&ev)
        .into_iter()
        .filter(|i| !done.contains(i))
        .map(|i| json!({"id": i, "date": ev[&i]["date"], "title": ev[&i]["title"], "file": l.evidence_md(&i).display().to_string()}))
        .collect())
}

pub fn mark_processed(project: &str, evid: &str, note: &str) -> Result<()> {
    let l = Ledger::of(project)?;
    if !l.evidence().contains_key(evid) {
        bail!("unknown evidence {evid:?}");
    }
    l.append(&[json!({"type": "processed", "evidence": evid, "by": "agent", "note": note, "at": Local::now().to_rfc3339()})])
}

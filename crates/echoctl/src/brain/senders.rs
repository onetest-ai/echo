//! Who may speak into the Brain through mail and Teams channels. Per client (threads without one: `-`):
//! senders the user trusted, ignored, and those waiting for a decision. Trusted without asking: the user,
//! and colleagues — mail sent inside the organisation (Exchange's `AuthAs: Internal`) from one of its
//! verified domains; a Teams author whose address (the People API, by name) is in those domains. Anyone
//! else waits: their messages aren't read until the user trusts them. A trusted outside sender's mail is
//! read only when it passed authentication (DMARC, composite, or DKIM + SPF): the From line alone is
//! easy to forge.

use crate::paths;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// An address (lowercase) or `teams:<user id>`.
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub at: String,
    /// A trusted sender's mail that carries no authentication verdict (a mailing list, a domain without
    /// DMARC): `true` read anyway, `false` kept out without asking again; none = the user is asked.
    /// Mail that failed authentication is never read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unverified: Option<bool>,
}

/// A sender waiting for the user, with what they sent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Waiting {
    pub key: String,
    #[serde(default)]
    pub name: String,
    /// `mail` | `teams`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub last_at: String,
    /// The last subject or channel (others' words: shown escaped, never sent to a model).
    #[serde(default)]
    pub last_about: String,
    /// The last mail passed authentication.
    #[serde(default)]
    pub verified: bool,
    /// Why Echo thinks they belong (in the thread's People, in its calls, the client's domain).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Messages counted (the last 50), so a conversation read again isn't counted twice.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub seen: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct List {
    #[serde(default)]
    pub trusted: Vec<Entry>,
    #[serde(default)]
    pub ignored: Vec<Entry>,
    #[serde(default)]
    pub waiting: Vec<Waiting>,
    /// Trusted senders whose mail came without authentication, until the user says read it or not.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unverified: Vec<Waiting>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Senders {
    #[serde(default)]
    pub clients: BTreeMap<String, List>,
    /// The organisation's verified domains (colleagues), and when they were read.
    #[serde(default)]
    pub org_domains: Vec<String>,
    #[serde(default)]
    pub org_at: String,
    /// Teams authors looked up by name: id → address ("" when none was found).
    #[serde(default)]
    pub teams_known: BTreeMap<String, String>,
}

fn path() -> std::path::PathBuf {
    paths::home().join("brain").join("senders.json")
}

pub fn load() -> Senders {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// Save what a sync learned without undoing a decision the user made meanwhile: the trusted and
/// ignored lists on disk win, and a decided sender leaves the waiting list.
pub fn save_merged(s: &mut Senders) -> Result<()> {
    let disk = load();
    for (c, l) in &disk.clients {
        let mine = s.clients.entry(c.clone()).or_default();
        mine.trusted = l.trusted.clone();
        mine.ignored = l.ignored.clone();
    }
    for l in s.clients.values_mut() {
        let decided: Vec<String> = l.trusted.iter().chain(&l.ignored).map(|e| e.key.clone()).collect();
        l.waiting.retain(|w| !decided.contains(&w.key));
        let answered: Vec<String> = l.trusted.iter().filter(|e| e.unverified.is_some()).map(|e| e.key.clone()).collect();
        let trusted: Vec<String> = l.trusted.iter().map(|e| e.key.clone()).collect();
        l.unverified.retain(|w| trusted.contains(&w.key) && !answered.contains(&w.key));
    }
    save(s)
}

pub fn save(s: &Senders) -> Result<()> {
    std::fs::create_dir_all(path().parent().expect("a folder"))?;
    paths::write_atomic(&path(), serde_json::to_string_pretty(s)?.as_bytes())?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// Read.
    Trusted,
    /// Never read.
    Ignored,
    /// Not read; the sender waits for the user.
    Waiting,
    /// Trusted, but this mail didn't pass authentication: not read.
    Unverified,
}

/// The clients a document's threads belong to (`-` for a thread with none): a sender must be trusted
/// under each, since each client's sessions get what is read.
pub fn clients_of(threads: &[Value]) -> Vec<String> {
    let mut v: Vec<String> = threads.iter().map(|t| t["client"].as_str().unwrap_or("-").to_string()).collect();
    v.sort();
    v.dedup();
    v
}

/// One verdict from each client's: ignored anywhere → ignored; trusted everywhere → trusted; else
/// unverified when only authentication is missing, else waiting.
pub fn combine(vs: &[Verdict]) -> Verdict {
    if vs.contains(&Verdict::Ignored) {
        Verdict::Ignored
    } else if vs.iter().all(|v| *v == Verdict::Trusted) {
        Verdict::Trusted
    } else if vs.contains(&Verdict::Waiting) {
        Verdict::Waiting
    } else {
        Verdict::Unverified
    }
}

impl Senders {
    fn me(&self) -> Option<String> {
        crate::graph::account().map(|a| a.to_lowercase())
    }

    fn colleague_address(&self, addr: &str) -> bool {
        addr.rsplit_once('@').is_some_and(|(_, d)| self.org_domains.iter().any(|o| o == d || d.ends_with(&format!(".{o}"))))
    }

    fn listed(&self, client: &str, key: &str) -> Option<bool> {
        let l = self.clients.get(client)?;
        if l.ignored.iter().any(|e| e.key == key) {
            return Some(false);
        }
        l.trusted.iter().any(|e| e.key == key).then_some(true)
    }

    /// A mail's verdict: from its sender's address and Exchange's authentication of it.
    pub fn mail(&self, client: &str, from: &str, auth: &str) -> Verdict {
        let from = from.to_lowercase();
        // The user and colleagues only when Exchange says it came from inside: a From line is easy to forge.
        if auth == "internal" && (self.me().as_deref() == Some(from.as_str()) || self.colleague_address(&from)) {
            return Verdict::Trusted;
        }
        let read_anyway = self.clients.get(client).and_then(|l| l.trusted.iter().find(|e| e.key == from)).is_some_and(|e| e.unverified == Some(true));
        match self.listed(client, &from) {
            Some(false) => Verdict::Ignored,
            Some(true) if auth == "pass" || auth == "internal" || (auth == "none" && read_anyway) => Verdict::Trusted,
            Some(true) => Verdict::Unverified,
            None => Verdict::Waiting,
        }
    }

    /// A channel post's verdict: a colleague (their address in the organisation's domains), else the list.
    pub fn teams(&self, client: &str, key: &str) -> Verdict {
        if self.teams_known.get(key).is_some_and(|a| !a.is_empty() && (self.colleague_address(a) || self.me().as_deref() == Some(a.as_str()))) {
            return Verdict::Trusted;
        }
        match self.listed(client, key) {
            Some(true) => Verdict::Trusted,
            Some(false) => Verdict::Ignored,
            None => Verdict::Waiting,
        }
    }

    /// Note a sender whose message wasn't read.
    #[allow(clippy::too_many_arguments)]
    pub fn wait(&mut self, client: &str, key: &str, name: &str, kind: &str, msg: &str, at: &str, about: &str, verified: bool, hint: Option<String>) {
        let l = self.clients.entry(client.to_string()).or_default();
        if key.is_empty() || l.trusted.iter().chain(&l.ignored).any(|e| e.key == key) {
            return;
        }
        match l.waiting.iter_mut().find(|w| w.key == key) {
            Some(w) => {
                if w.seen.iter().any(|s| s == msg) {
                    return;
                }
                w.seen.push(msg.to_string());
                if w.seen.len() > 50 {
                    w.seen.remove(0);
                }
                w.count += 1;
                if at > w.last_at.as_str() {
                    w.last_at = at.to_string();
                    w.last_about = about.chars().take(80).collect();
                    w.verified = verified;
                }
                if w.hint.is_none() {
                    w.hint = hint;
                }
            }
            None => l.waiting.push(Waiting { key: key.into(), name: name.into(), kind: kind.into(), count: 1, last_at: at.into(), last_about: about.chars().take(80).collect(), verified, hint, seen: vec![msg.to_string()] }),
        }
    }

    /// Note a trusted sender's mail that wasn't read for want of authentication — unless the user
    /// already said to keep it out; a failed check (`fail`: likely forged) is counted but not offered.
    #[allow(clippy::too_many_arguments)]
    pub fn note_unverified(&mut self, client: &str, key: &str, name: &str, msg: &str, at: &str, about: &str, auth: &str) {
        let Some(l) = self.clients.get_mut(client) else { return };
        if auth != "none" || l.trusted.iter().find(|e| e.key == key).is_none_or(|e| e.unverified.is_some()) {
            return;
        }
        match l.unverified.iter_mut().find(|w| w.key == key) {
            Some(w) => {
                if !w.seen.iter().any(|s| s == msg) {
                    w.seen.push(msg.to_string());
                    if w.seen.len() > 50 {
                        w.seen.remove(0);
                    }
                    w.count += 1;
                    if at > w.last_at.as_str() {
                        w.last_at = at.to_string();
                        w.last_about = about.chars().take(80).collect();
                    }
                }
            }
            None => l.unverified.push(Waiting { key: key.into(), name: name.into(), kind: "mail".into(), count: 1, last_at: at.into(), last_about: about.chars().take(80).collect(), verified: false, hint: None, seen: vec![msg.to_string()] }),
        }
    }

    /// The colleagues' domains, read again daily (and the first time).
    pub fn refresh_org(&mut self) {
        let fresh = chrono::DateTime::parse_from_rfc3339(&self.org_at).is_ok_and(|t| chrono::Utc::now().signed_duration_since(t) < chrono::Duration::days(1));
        if fresh && !self.org_domains.is_empty() {
            return;
        }
        if let Ok(d) = crate::graph::org_domains() {
            self.org_domains = d;
            self.org_at = chrono::Utc::now().to_rfc3339();
        }
    }

    /// A Teams author's address, looked up once (a lookup that failed is tried again next time).
    pub fn learn_author(&mut self, key: &str, name: &str) {
        let Some(id) = key.strip_prefix("teams:").filter(|_| !self.teams_known.contains_key(key)) else { return };
        if let Ok(a) = crate::graph::person_email(name, id) {
            self.teams_known.insert(key.to_string(), a.unwrap_or_default());
        }
    }

    /// A Teams author not looked up yet: their posts wait for the lookup, not for the user.
    pub fn unknown_author(&self, key: &str) -> bool {
        key.starts_with("teams:") && !self.teams_known.contains_key(key)
    }
}

/// The user's decision on a sender: trust, ignore, or forget (back to waiting next time); for a
/// trusted sender's unauthenticated mail, `read_unverified` or `keep_unverified_out`.
pub fn decide(client: &str, key: &str, decision: &str) -> Result<()> {
    let key = if key.starts_with("teams:") || key.starts_with("app:") { key.trim().to_string() } else { key.trim().to_lowercase() };
    let key = key.as_str();
    let mut s = load();
    let l = s.clients.entry(client.to_string()).or_default();
    if let Some(read) = match decision {
        "read_unverified" => Some(true),
        "keep_unverified_out" => Some(false),
        _ => None,
    } {
        let e = l.trusted.iter_mut().find(|e| e.key == key).ok_or_else(|| anyhow::anyhow!("{key} isn't trusted for this client"))?;
        e.unverified = Some(read);
        l.unverified.retain(|w| w.key != key);
        save(&s)?;
        if read {
            super::sources::request_reread(client)?;
        }
        return Ok(());
    }
    let name = l.waiting.iter().find(|w| w.key == key).map(|w| w.name.clone()).or_else(|| l.trusted.iter().chain(&l.ignored).find(|e| e.key == key).map(|e| e.name.clone())).unwrap_or_default();
    l.trusted.retain(|e| e.key != key);
    l.ignored.retain(|e| e.key != key);
    let e = Entry { key: key.to_string(), name, at: chrono::Utc::now().to_rfc3339(), unverified: None };
    l.unverified.retain(|w| w.key != key);
    match decision {
        "trust" => l.trusted.push(e),
        "ignore" => l.ignored.push(e),
        "forget" => {}
        d => anyhow::bail!("trust, ignore or forget (not {d:?})"),
    }
    l.waiting.retain(|w| w.key != key);
    save(&s)?;
    // That client's sources are read again from the start at the next sync (the reader's cache keeps
    // it cheap): a trusted sender's messages come in, an ignored one's go.
    super::sources::request_reread(client)?;
    Ok(())
}

/// For the window and the CLI.
pub fn view() -> Value {
    let s = load();
    json!({"clients": s.clients, "org_domains": s.org_domains})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exchanges_aligned_verdict_passes() {
        let mail = |from: &str, hs: &[(&str, &str)]| json!({"from": {"emailAddress": {"address": from}}, "internetMessageHeaders": hs.iter().map(|(n, v)| json!({"name": n, "value": v})).collect::<Vec<_>>()});
        let g = crate::graph::mail_auth;
        // The attacker's own domain passes SPF and DKIM behind a forged From: not authenticated.
        assert_eq!(g(&mail("ana@acme.com", &[("Authentication-Results", "spf=pass smtp.mailfrom=evil.com; dkim=pass header.d=evil.com; dmarc=fail action=none header.from=acme.com; compauth=fail")])), "fail");
        assert_eq!(g(&mail("ana@acme.com", &[("Authentication-Results", "spf=pass smtp.mailfrom=evil.com; dkim=pass header.d=evil.com; dmarc=none header.from=acme.com")])), "none");
        assert_eq!(g(&mail("ana@acme.com", &[("Authentication-Results", "spf=pass; dkim=pass header.d=acme.com; dmarc=pass action=none header.from=acme.com;compauth=pass reason=100")])), "pass");
        // A pass the sender wrote lower down doesn't count; nor one for another From domain.
        assert_eq!(g(&mail("ana@acme.com", &[("Authentication-Results", "dmarc=none header.from=acme.com"), ("Authentication-Results", "dmarc=pass header.from=acme.com")])), "none");
        assert_eq!(g(&mail("ana@acme.com", &[("Authentication-Results", "dmarc=pass header.from=evil.com")])), "none");
        assert_eq!(g(&mail("bo@contoso.com", &[("X-MS-Exchange-Organization-AuthAs", "Internal")])), "internal");
    }

    #[test]
    fn verdicts() {
        let mut s = Senders { org_domains: vec!["contoso.com".into()], ..Default::default() };
        s.clients.insert("acme".into(), List { trusted: vec![Entry { key: "ana@acme.com".into(), ..Default::default() }], ignored: vec![Entry { key: "spam@acme.com".into(), ..Default::default() }], ..Default::default() });
        // A colleague's mail inside the organisation; the same address from outside isn't a colleague.
        assert_eq!(s.mail("acme", "Bo@contoso.com", "internal"), Verdict::Trusted);
        assert_eq!(combine(&[Verdict::Trusted, Verdict::Waiting]), Verdict::Waiting);
        assert_eq!(combine(&[Verdict::Trusted, Verdict::Ignored]), Verdict::Ignored);
        assert_eq!(combine(&[Verdict::Trusted, Verdict::Trusted]), Verdict::Trusted);
        assert_eq!(s.mail("acme", "bo@contoso.com", "none"), Verdict::Waiting);
        // Not even the user's own address from outside.
        assert_ne!(s.mail("acme", "me@nowhere.example", "fail"), Verdict::Trusted);
        // A trusted outside sender: read only authenticated.
        assert_eq!(s.mail("acme", "ana@acme.com", "pass"), Verdict::Trusted);
        assert_eq!(s.mail("acme", "ana@acme.com", "none"), Verdict::Unverified);
        // Offered once; read anyway → read, but a failed check never is.
        s.note_unverified("acme", "ana@acme.com", "Ana", "m9", "2026-10-03T10:00:00Z", "News", "none");
        s.note_unverified("acme", "ana@acme.com", "Ana", "m9", "2026-10-03T10:00:00Z", "News", "none");
        s.note_unverified("acme", "ana@acme.com", "Ana", "m8", "2026-10-03T10:00:00Z", "Forged", "fail");
        assert_eq!(s.clients["acme"].unverified[0].count, 1);
        s.clients.get_mut("acme").unwrap().trusted[0].unverified = Some(true);
        assert_eq!(s.mail("acme", "ana@acme.com", "none"), Verdict::Trusted);
        assert_eq!(s.mail("acme", "ana@acme.com", "fail"), Verdict::Unverified);
        s.clients.get_mut("acme").unwrap().trusted[0].unverified = None;
        assert_eq!(s.mail("acme", "spam@acme.com", "pass"), Verdict::Ignored);
        // Trust is per client.
        assert_eq!(s.mail("other", "ana@acme.com", "pass"), Verdict::Waiting);
        s.teams_known.insert("teams:1".into(), "karen@contoso.com".into());
        s.teams_known.insert("teams:2".into(), String::new());
        assert_eq!(s.teams("acme", "teams:1"), Verdict::Trusted);
        assert_eq!(s.teams("acme", "teams:2"), Verdict::Waiting);
        s.wait("acme", "x@acme.com", "X", "mail", "m1", "2026-10-01T10:00:00Z", "Hello", true, None);
        s.wait("acme", "x@acme.com", "X", "mail", "m2", "2026-10-02T10:00:00Z", "Again", true, Some("in People".into()));
        s.wait("acme", "x@acme.com", "X", "mail", "m2", "2026-10-02T10:00:00Z", "Again", true, None);
        let w = &s.clients["acme"].waiting[0];
        assert_eq!((w.count, w.last_about.as_str(), w.hint.as_deref()), (2, "Again", Some("in People")));
    }
}

//! People and organisations around threads and clients (`docs/projects-design.md`, People): a
//! directory of people (`projects/people.json`: what the user or Microsoft 365 said about each,
//! with its source and history), each project's placements (`Member`, `OrgPlace`), and the facts
//! read from its calls — recomputed every time, so new people appear and quiet ones show by
//! themselves.

// Its edits come from the window (`window::project_op`).
#![cfg_attr(not(feature = "tray"), allow(dead_code))]

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::calls;
use crate::projects::{self, Client, Project};

const FILE: &str = "people.json";
/// First seen this recently: new. Not seen for this long while the calls went on: gone quiet.
const NEW_DAYS: i64 = 14;
const QUIET_DAYS: i64 = 42;
/// Fields a person's directory entry takes from the user (`set`).
pub const FIELDS: [&str; 12] = ["name", "title", "org", "group", "reports_to", "matters", "notes", "phone", "department", "company", "office", "about"];
/// Who someone is, in a few sentences: the people pass writes it (source `echo`) until the user
/// writes theirs (source `you`: kept, never rewritten).
const MAX_ABOUT: usize = 2000;
const MAX_AGENT_ABOUT: usize = 800;
/// A fact about someone: one line, its words quoted from one evidence item.
const MAX_FACT: usize = 240;
/// Facts kept per person (the oldest go first).
const MAX_FACTS: usize = 300;
/// The fields Microsoft 365 fills (the People API).
const M365_FIELDS: [&str; 5] = ["title", "department", "company", "office", "phone"];
/// A person is looked up again after this many days.
const M365_DAYS: i64 = 7;
/// At most this many lookups per refresh.
const M365_BATCH: usize = 40;
pub const MATTERS: [&str; 4] = ["critical", "high", "normal", "irrelevant"];

/// A person in the directory: what the user (or later Microsoft 365) said, each field with its
/// source, and a history of changes. `id` = their email, lower case (else `n:<name>`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emails: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<Value>,
    /// When Microsoft 365 was last asked about them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub m365_at: Option<String>,
    /// Where a source disagrees with what the user set: `{field, yours, theirs, source, at}`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<Value>,
    /// A source's value the user chose to keep theirs over (not asked again for that value).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub kept: BTreeMap<String, String>,
    /// Addresses (or `n:` names) the user said are someone else: never merged with this person.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_same: Vec<String>,
    /// What the calls, chats and mails of the threads say about them, each with its words and where
    /// they were said: `{id, text, quote, date, thread, evidence, kind, call, title, url, ts, added}`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<Value>,
}

/// Every address the directory knows for a person → their key.
fn canon(dir_people: &[Person]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for p in dir_people {
        for e in p.emails.iter().chain(std::iter::once(&p.id)) {
            out.insert(e.to_lowercase(), p.id.clone());
        }
    }
    out
}

/// Did the user say these two are different people?
fn apart(dir_people: &[Person], a: &str, b: &str) -> bool {
    dir_people.iter().any(|p| (p.id == a || p.emails.iter().any(|e| e == a)) && p.not_same.iter().any(|x| x == b))
        || dir_people.iter().any(|p| (p.id == b || p.emails.iter().any(|e| e == b)) && p.not_same.iter().any(|x| x == a))
}

/// Merge `key` into `into`: one person with both's addresses, names and fields (`into`'s win).
pub fn merge(key: &str, key_name: &str, key_emails: &[String], into: &str, into_name: &str, into_email: Option<&str>) -> Result<()> {
    if key == into {
        bail!("that is the same person");
    }
    projects::with_lock(|| {
        let mut all = directory()?;
        let old = all.iter().position(|p| p.id == key).map(|i| all.remove(i));
        let i = match all.iter().position(|p| p.id == into) {
            Some(i) => i,
            None => {
                all.push(Person { id: into.into(), name: display(into_name), emails: into_email.map(|e| vec![e.to_lowercase()]).unwrap_or_default(), ..Default::default() });
                all.len() - 1
            }
        };
        let p = &mut all[i];
        let mut add = key_emails.iter().map(|e| e.to_lowercase()).collect::<Vec<_>>();
        if key.contains('@') {
            add.push(key.to_string());
        }
        for e in add {
            p.not_same.retain(|x| x != &e);
            if !p.emails.contains(&e) {
                p.emails.push(e);
            }
        }
        for n in [key_name.to_string()].into_iter().chain(old.iter().flat_map(|o| o.aliases.clone())) {
            if !n.is_empty() && n != p.name && !p.aliases.contains(&n) {
                p.aliases.push(n);
            }
        }
        if let Some(o) = old {
            for (k, v) in o.fields {
                if !p.fields.contains_key(&k) {
                    p.sources.insert(k.clone(), o.sources.get(&k).cloned().unwrap_or_default());
                    p.fields.insert(k, v);
                }
            }
            p.history.extend(o.history);
        }
        p.history.push(json!({"field": "merged", "from": key, "to": into, "source": "you", "at": chrono::Utc::now().to_rfc3339()}));
        projects::write_json(&file(), &all)
    })
}

/// Split an address off a person: it is someone else, and won't be merged with them again.
pub fn split(key: &str, name: &str, email: &str) -> Result<()> {
    let email = email.to_lowercase();
    projects::with_lock(|| {
        let mut all = directory()?;
        let i = match all.iter().position(|p| p.id == key) {
            Some(i) => i,
            None => {
                all.push(Person { id: key.into(), name: display(name), emails: if key.contains('@') { vec![key.into()] } else { vec![] }, ..Default::default() });
                all.len() - 1
            }
        };
        let p = &mut all[i];
        p.emails.retain(|e| e != &email);
        if !p.not_same.contains(&email) {
            p.not_same.push(email.clone());
        }
        p.history.push(json!({"field": "not the same", "from": email, "to": null, "source": "you", "at": chrono::Utc::now().to_rfc3339()}));
        projects::write_json(&file(), &all)
    })
}

fn file() -> PathBuf {
    projects::dir().join(FILE)
}

pub fn directory() -> Result<Vec<Person>> {
    match std::fs::read(file()) {
        Ok(b) => Ok(serde_json::from_slice(&b)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}

/// Who is a colleague on any of their addresses (#40: a colleague invited on a client address
/// counted as the client): the user's own mail domain and internal organisations' domains, and every
/// address of a directory person known as a colleague — one of their addresses is in those domains,
/// or Microsoft 365's People API knows them in the tenant (their title, department or office come
/// from it: outside people come back with a name or nothing).
#[derive(Debug, Default)]
pub struct Colleagues {
    pub domains: BTreeSet<String>,
    pub addresses: BTreeSet<String>,
}

impl Colleagues {
    pub fn domain(&self, d: &str) -> bool {
        let d = d.to_lowercase();
        self.domains.iter().any(|x| d == *x || d.ends_with(&format!(".{x}")))
    }

    pub fn address(&self, e: &str) -> bool {
        let e = e.trim().to_lowercase();
        self.addresses.contains(&e) || domain(Some(&e)).is_some_and(|d| self.domain(&d))
    }

    /// The addresses (lowercase) among these people that are colleagues': a colleague's address, or any
    /// address of a name that also comes with one here (the same person invited twice).
    pub fn among<'a>(&self, people: impl IntoIterator<Item = (&'a str, Option<&'a str>)>) -> BTreeSet<String> {
        let people: Vec<(String, String)> = people.into_iter().filter_map(|(n, e)| Some((norm(n), e?.trim().to_lowercase()))).collect();
        let names: BTreeSet<&String> = people.iter().filter(|(n, e)| n.split(' ').count() >= 2 && self.address(e)).map(|(n, _)| n).collect();
        people.iter().filter(|(n, e)| self.address(e) || names.contains(n)).map(|(_, e)| e.clone()).collect()
    }
}

/// The colleagues' book, read again only when `clients.json`, the directory or the settings change.
pub fn colleagues() -> std::sync::Arc<Colleagues> {
    type Cached = Option<(Vec<Option<std::time::SystemTime>>, std::sync::Arc<Colleagues>)>;
    static CACHE: std::sync::Mutex<Cached> = std::sync::Mutex::new(None);
    let stamps: Vec<Option<std::time::SystemTime>> = [projects::dir().join("clients.json"), file(), crate::paths::state_dir().join("config.json")]
        .iter()
        .map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .collect();
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    // Tests write these files within the same second: never cached there.
    if let Some((_, c)) = g.as_ref().filter(|(s, _)| *s == stamps && !cfg!(test)) {
        return c.clone();
    }
    let mut c = Colleagues::default();
    c.domains.extend(crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| domain(m.email.as_deref())));
    c.domains.extend(projects::orgs().unwrap_or_default().into_iter().filter(|o| o.internal).flat_map(|o| o.domains).map(|d| d.trim().trim_start_matches('@').to_lowercase()).filter(|d| !d.is_empty()));
    for p in directory().unwrap_or_default() {
        let tenant = ["title", "department", "office"].iter().any(|f| p.sources.get(*f).is_some_and(|s| s == "m365"));
        if tenant || p.emails.iter().any(|e| domain(Some(e)).is_some_and(|d| c.domain(&d))) {
            c.addresses.extend(p.emails.iter().map(|e| e.trim().to_lowercase()));
        }
    }
    let c = std::sync::Arc::new(c);
    *g = Some((stamps, c.clone()));
    c
}

/// A person's key: their email, else their name.
pub fn key_of(name: &str, email: Option<&str>) -> String {
    match email.map(|e| e.trim().to_lowercase()).filter(|e| e.contains('@')) {
        Some(e) => e,
        None => format!("n:{}", norm(name)),
    }
}

/// A display name as people read it: "(Partner)", "(external)", "[EXT]" and the like dropped, and
/// a directory's "Last, First" turned round. "Varga, Lena (Partner)" → "Lena Varga".
pub fn display(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in name.chars() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth = (depth - 1).max(0),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let out = out.trim_matches(|c: char| c == ',' || c == '-' || c.is_whitespace()).to_string();
    match out.split_once(',') {
        Some((last, first)) if !first.trim().is_empty() && !last.trim().is_empty() && !first.contains(',') => format!("{} {}", first.trim(), last.trim()),
        _ => out,
    }
}

/// Accents folded, for comparing names only (Inés = Ines).
fn fold(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' => 'a',
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ě' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
            'ú' | 'ù' | 'û' | 'ü' | 'ů' => 'u',
            'ñ' | 'ń' => 'n',
            'ç' | 'č' | 'ć' => 'c',
            'š' | 'ś' => 's',
            'ž' | 'ź' | 'ż' => 'z',
            'ł' => 'l',
            'ý' => 'y',
            'ř' => 'r',
            c => c,
        })
        .collect()
}

/// A name for matching: displayed, lower case, accents folded.
fn norm(name: &str) -> String {
    fold(&display(name).to_lowercase())
}

/// Words that tag an address or a name, not a person ("partner.lena.varga@…").
const TAGS: [&str; 8] = ["partner", "ext", "external", "guest", "contractor", "contr", "admin", "adm"];

/// The name words of a person: from their name, else their address's local part.
fn tokens(text: &str) -> BTreeSet<String> {
    fold(&text.to_lowercase())
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= 2 && !TAGS.contains(w))
        .map(str::to_owned)
        .collect()
}

/// Set one field of a person (the user's word: source `you`), creating the entry; `""` clears it.
/// Every change is kept in its history.
pub fn set(key: &str, name: &str, email: Option<&str>, field: &str, value: &str, source: &str) -> Result<Person> {
    if !FIELDS.contains(&field) {
        bail!("a person's fields are {FIELDS:?}");
    }
    let value = value.trim();
    if field == "matters" && !value.is_empty() && !MATTERS.contains(&value) {
        bail!("how much someone matters is one of {MATTERS:?}");
    }
    if field == "about" && value.chars().count() > MAX_ABOUT {
        bail!("who someone is takes at most {MAX_ABOUT} characters");
    }
    projects::with_lock(|| {
        let mut all = directory()?;
        let i = match all.iter().position(|p| p.id == key) {
            Some(i) => i,
            None => {
                all.push(Person { id: key.into(), name: name.trim().into(), emails: email.map(|e| vec![e.trim().to_lowercase()]).unwrap_or_default(), ..Default::default() });
                all.len() - 1
            }
        };
        let p = &mut all[i];
        let before = if field == "name" { Some(p.name.clone()) } else { p.fields.get(field).cloned() };
        if before.as_deref().unwrap_or("") == value {
            return Ok(p.clone());
        }
        if field == "name" {
            if value.is_empty() {
                bail!("a person needs a name");
            }
            p.name = value.into();
        } else if value.is_empty() {
            p.fields.remove(field);
        } else {
            p.fields.insert(field.into(), value.into());
        }
        p.sources.insert(field.into(), source.into());
        if field == "about" {
            p.fields.remove("about_thread");
        }
        p.history.push(json!({"field": field, "from": before, "to": (!value.is_empty()).then_some(value), "source": source, "at": chrono::Utc::now().to_rfc3339()}));
        let p = p.clone();
        projects::write_json(&file(), &all)?;
        Ok(p)
    })
}

/// Merge what a source says into a person: a field nobody set, or the same source set, takes the
/// new value (with history); a field the user set and the source contradicts becomes a question —
/// unless the user already kept theirs over that very value. Returns whether anything changed.
fn merge_source(p: &mut Person, said: &[(&str, String)], source: &str, now: &str) -> bool {
    let mut changed = false;
    for (field, theirs) in said {
        let theirs = theirs.trim();
        if theirs.is_empty() {
            continue;
        }
        let ours = p.fields.get(*field).cloned();
        if ours.as_deref() == Some(theirs) {
            continue;
        }
        if p.sources.get(*field).map(String::as_str) == Some("you") && ours.is_some() {
            if p.kept.get(*field).map(String::as_str) != Some(theirs) && !p.questions.iter().any(|q| q["field"] == *field && q["theirs"] == theirs) {
                p.questions.retain(|q| q["field"] != *field);
                p.questions.push(json!({"field": field, "yours": ours, "theirs": theirs, "source": source, "at": now}));
                changed = true;
            }
            continue;
        }
        p.history.push(json!({"field": field, "from": ours, "to": theirs, "source": source, "at": now}));
        p.fields.insert((*field).into(), theirs.into());
        p.sources.insert((*field).into(), source.into());
        changed = true;
    }
    changed
}

/// Ask Microsoft 365 about people (`(name, email)`) not asked for a week, at most `M365_BATCH`; each
/// answer is merged as `m365` (`merge_source`). Without a Microsoft 365 sign-in, nothing.
pub fn refresh_m365(who: &[(String, String)]) -> Result<usize> {
    if crate::graph::account().is_none() {
        return Ok(0);
    }
    let now = chrono::Utc::now();
    let fresh = |p: &Person| p.m365_at.as_deref().and_then(|a| chrono::DateTime::parse_from_rfc3339(a).ok()).is_some_and(|a| (now - a.with_timezone(&chrono::Utc)).num_days() < M365_DAYS);
    let dir = directory()?;
    let todo: Vec<&(String, String)> = who
        .iter()
        .filter(|(_, e)| e.contains('@'))
        .filter(|(_, e)| !dir.iter().any(|p| (p.id == e.to_lowercase() || p.emails.contains(&e.to_lowercase())) && fresh(p)))
        .take(M365_BATCH)
        .collect();
    // Ask first, outside the lock (the network is slow); then merge every answer under it.
    let mut answers = vec![];
    for (name, email) in todo {
        match crate::graph::person(email) {
            Ok(a) => answers.push((name.clone(), email.to_lowercase(), a)),
            Err(e) => {
                eprintln!("echoctl: Microsoft 365 about {email}: {e:#}");
                break;
            }
        }
    }
    let n = answers.len();
    if n == 0 {
        return Ok(0);
    }
    projects::with_lock(|| {
        let mut all = directory()?;
        let at = now.to_rfc3339();
        for (name, email, a) in answers {
            let i = match all.iter().position(|p| p.id == email || p.emails.contains(&email)) {
                Some(i) => i,
                None => {
                    all.push(Person { id: email.clone(), name: name.clone(), emails: vec![email.clone()], ..Default::default() });
                    all.len() - 1
                }
            };
            let p = &mut all[i];
            p.m365_at = Some(at.clone());
            if let Some(a) = a {
                let said: Vec<(&str, String)> = M365_FIELDS.iter().filter_map(|f| a[*f].as_str().map(|v| (*f, v.to_string()))).collect();
                merge_source(p, &said, "m365", &at);
            }
        }
        projects::write_json(&file(), &all)
    })?;
    Ok(n)
}

/// The user's answer to a question: `use` takes the source's value, `keep` keeps theirs (and isn't
/// asked about that value again).
pub fn answer(key: &str, field: &str, use_theirs: bool) -> Result<()> {
    projects::with_lock(|| {
        let mut all = directory()?;
        let p = all.iter_mut().find(|p| p.id == key).ok_or_else(|| anyhow::anyhow!("no such person"))?;
        let q = p.questions.iter().position(|q| q["field"] == field).ok_or_else(|| anyhow::anyhow!("no question about {field}"))?;
        let q = p.questions.remove(q);
        let theirs = q["theirs"].as_str().unwrap_or("").to_string();
        let now = chrono::Utc::now().to_rfc3339();
        if use_theirs {
            p.history.push(json!({"field": field, "from": q["yours"], "to": theirs, "source": q["source"], "at": now}));
            p.fields.insert(field.into(), theirs);
            p.sources.insert(field.into(), q["source"].as_str().unwrap_or("").into());
        } else {
            p.kept.insert(field.into(), theirs);
        }
        projects::write_json(&file(), &all)
    })
}

/// What the calls say about one person.
#[derive(Debug, Default, Clone)]
struct Seen {
    name: String,
    email: Option<String>,
    calls: BTreeSet<String>,
    invited: usize,
    spoke: f64,
    chat: usize,
    first: String,
    last: String,
    /// Other addresses the same person was invited with (a guest account).
    also: Vec<String>,
}

/// One person invited from two accounts (their own and a guest one) shows as one: the same full
/// name (two words or more) with different emails merges into the one seen in more calls.
fn merge_same_names(seen: &mut BTreeMap<String, Seen>, dir_people: &[Person]) {
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (k, s) in seen.iter() {
        let n = norm(&s.name);
        if n.split(' ').count() >= 2 && s.email.is_some() {
            by_name.entry(n).or_default().push(k.clone());
        }
    }
    for keys in by_name.into_values().filter(|k| k.len() > 1) {
        let mut keys = keys;
        keys.sort_by_key(|k| std::cmp::Reverse(seen[k].calls.len()));
        let (main, rest) = keys.split_first().unwrap();
        for k in rest.iter().filter(|k| !apart(dir_people, main, k)) {
            let o = seen.remove(k).unwrap();
            let m = seen.get_mut(main).unwrap();
            m.calls.extend(o.calls);
            m.invited += o.invited;
            m.spoke += o.spoke;
            m.chat += o.chat;
            if m.first.is_empty() || (!o.first.is_empty() && o.first < m.first) {
                m.first = o.first;
            }
            m.last = m.last.clone().max(o.last);
            for e in o.email.into_iter().chain(o.also) {
                if !m.also.contains(&e) && m.email.as_ref() != Some(&e) {
                    m.also.push(e);
                }
            }
        }
    }
}

/// The user, as calls show them (left out of every list).
struct MeIs(Option<String>, Option<String>);

impl MeIs {
    fn now() -> MeIs {
        let me = crate::config::Config::load().ok().and_then(|c| c.me);
        MeIs(me.as_ref().map(|m| norm(&m.name)), me.and_then(|m| m.email).map(|e| e.to_lowercase()))
    }
    fn is(&self, name: &str, email: Option<&str>) -> bool {
        email.map(str::to_lowercase).is_some_and(|e| Some(&e) == self.1.as_ref()) || self.0.as_deref() == Some(norm(name).as_str())
    }
}

/// A generated name ("Speaker 2", "Remote") is nobody in particular.
fn anonymous(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    // "Speaker 2" (detected voices), "@1" (a Teams transcript's unnamed speaker), "someone" (a chat
    // message with no user: a bot or an app) are nobody.
    n.is_empty() || n == "remote" || n == "me" || n == "unknown" || n == "someone"
        || (n.starts_with("speaker ") && n[8..].chars().all(|c| c.is_ascii_digit()))
        || (n.len() > 1 && n.starts_with('@') && n[1..].chars().all(|c| c.is_ascii_digit()))
}

/// Everyone the calls show: invitees and organisers, named speakers (and how long each spoke),
/// chat posters — by email where any call gives one for that name. Returns them and the total
/// speech in seconds.
fn gather(dirs: &[PathBuf], me: &MeIs, dir_people: &[Person]) -> (BTreeMap<String, Seen>, f64) {
    // Names → emails from every invite and the directory's aliases (speakers and chat carry names only).
    let mut emails: BTreeMap<String, String> = BTreeMap::new();
    let meetings: Vec<Option<crate::meeting::MeetingContext>> = dirs.iter().map(|d| crate::meeting::MeetingContext::load(d).ok().flatten()).collect();
    for m in meetings.iter().flatten() {
        for p in m.organizer.iter().chain(&m.attendees) {
            if let Some(e) = p.email.as_deref().filter(|e| e.contains('@')) {
                emails.entry(norm(&p.name)).or_insert_with(|| e.to_lowercase());
            }
        }
    }
    for p in dir_people {
        if let Some(e) = p.emails.first() {
            for n in p.aliases.iter().chain(std::iter::once(&p.name)) {
                emails.entry(norm(n)).or_insert_with(|| e.clone());
            }
        }
    }
    let canon = canon(dir_people);
    let mut out: BTreeMap<String, Seen> = BTreeMap::new();
    let mut total = 0.0;
    for (d, m) in dirs.iter().zip(&meetings) {
        let meta = calls::read_json(&d.join("meta.json"));
        let date = meta["started_at"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|t| t.with_timezone(&chrono::Local).date_naive().to_string()).unwrap_or_default();
        let stamp = calls::stamp_of(d);
        let mut touch = |name: &str, email: Option<&str>, f: &mut dyn FnMut(&mut Seen)| {
            if anonymous(name) && email.is_none() {
                return;
            }
            let email = email.map(str::to_lowercase).or_else(|| emails.get(&norm(name)).cloned());
            if me.is(name, email.as_deref()) {
                return;
            }
            let key = email.as_ref().and_then(|e| canon.get(e).cloned()).unwrap_or_else(|| key_of(name, email.as_deref()));
            let s = out.entry(key).or_default();
            let shown = display(name);
            if s.name.is_empty() || (s.name.contains('@') && !shown.contains('@')) {
                s.name = shown;
            }
            match (&s.email, email) {
                (None, e) => s.email = e,
                (Some(a), Some(e)) if *a != e && !s.also.contains(&e) => s.also.push(e),
                _ => {}
            }
            if s.calls.insert(stamp.clone()) {
                if s.first.is_empty() || date < s.first {
                    s.first = date.clone();
                }
                if date > s.last {
                    s.last = date.clone();
                }
            }
            f(s);
        };
        if let Some(m) = m {
            for p in m.organizer.iter().chain(&m.attendees) {
                let n = p.name.trim().trim_end_matches("(external)").trim();
                touch(if n.is_empty() { p.email.as_deref().unwrap_or("") } else { n }, p.email.as_deref(), &mut |s| s.invited += 1);
            }
        }
        if let Some(segs) = calls::read_json(&d.join("segments.json")).as_array() {
            for s in segs {
                let secs = (s["end"].as_f64().unwrap_or(0.0) - s["start"].as_f64().unwrap_or(0.0)).max(0.0);
                total += secs;
                if s["speaker"] == "remote" {
                    if let Some(n) = s["name"].as_str() {
                        touch(n, None, &mut |x| x.spoke += secs);
                    }
                }
            }
        }
        if let Some(chat) = calls::read_json(&d.join("chat.json")).as_array() {
            for c in chat.iter().filter(|c| c["bot"] != true) {
                if let Some(n) = c["from"].as_str() {
                    touch(n, None, &mut |x| x.chat += 1);
                }
            }
        }
    }
    merge_same_names(&mut out, dir_people);
    (out, total)
}

fn domain(email: Option<&str>) -> Option<String> {
    email.and_then(|e| e.rsplit_once('@')).map(|(_, d)| d.to_lowercase())
}

/// The organisation a mail domain belongs to (a client's domain or one under it).
fn org_of<'a>(clients: &'a [Client], dom: Option<&str>) -> Option<&'a Client> {
    let dom = dom?;
    clients.iter().find(|c| c.domains.iter().any(|d| dom == d || dom.ends_with(&format!(".{d}"))))
}

/// Open tasks a person owns, the late ones, and the ones waiting on them, by name (first names too).
fn work_of(projects: &[&Project], today: chrono::NaiveDate) -> Vec<(String, usize, usize, usize)> {
    let mut out: BTreeMap<String, (usize, usize, usize)> = BTreeMap::new();
    for p in projects {
        let Ok(t) = crate::ledger::Ledger::of(&p.id).and_then(|l| crate::ledger::fold(&l)) else { continue };
        for n in t.nodes.values().filter(|n| n.kind == "task" && matches!(n.state.as_str(), "open" | "in_progress" | "blocked")) {
            for o in [&n.internal_owner, &n.client_owner].into_iter().flatten() {
                let e = out.entry(norm(o)).or_default();
                e.0 += 1;
                if n.due.as_deref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).is_some_and(|d| d < today) {
                    e.1 += 1;
                }
            }
            if let Some(w) = n.waiting_for.as_ref().and_then(|w| w["who"].as_str()) {
                out.entry(norm(w)).or_default().2 += 1;
            }
        }
    }
    out.into_iter().map(|(k, (a, b, c))| (k, a, b, c)).collect()
}

/// The People tab for some projects (one thread, or a client's threads): every person their calls
/// show or the user placed, with what Echo saw, the directory's fields and each project's placement;
/// and the organisations involved, with their relationships and placements.
pub fn view(threads: &[&Project], today: chrono::NaiveDate) -> Result<Value> {
    // Every organisation, accounts or not: whose people they are.
    let clients = projects::orgs()?;
    let dir_people = directory()?;
    let me = MeIs::now();
    let my_domain = crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| domain(m.email.as_deref()));
    let mut dirs: Vec<PathBuf> = vec![];
    // Each call's threads (by stamp), for the threads someone was seen in.
    let mut call_threads: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in threads {
        for (d, _) in projects::calls_of(p)? {
            call_threads.entry(calls::stamp_of(&d)).or_default().push(p.id.clone());
            if !dirs.contains(&d) {
                dirs.push(d);
            }
        }
    }
    let (mut seen, total) = gather(&dirs, &me, &dir_people);
    let ids: BTreeSet<&str> = threads.iter().map(|p| p.id.as_str()).collect();
    // Facts said in these threads put someone in them too (a mail's sender who was in no call).
    for p in &dir_people {
        if !seen.contains_key(&p.id) && p.facts.iter().any(|f| f["thread"].as_str().is_some_and(|t| ids.contains(t))) {
            seen.insert(p.id.clone(), Seen { name: p.name.clone(), email: p.emails.first().cloned(), ..Default::default() });
        }
    }
    // Who is in calls with whom: for each person, the three they shared most calls with (two or more).
    let mut with: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    {
        let keys: Vec<&String> = seen.keys().collect();
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                let n = seen[*a].calls.intersection(&seen[*b].calls).count();
                if n >= 2 {
                    with.entry((*a).clone()).or_default().push((n, (*b).clone()));
                    with.entry((*b).clone()).or_default().push((n, (*a).clone()));
                }
            }
        }
        for v in with.values_mut() {
            v.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
            v.truncate(3);
        }
    }
    let canon = canon(&dir_people);
    let member_key = |m: &projects::Member| m.email.as_ref().map(|e| e.to_lowercase()).and_then(|e| canon.get(&e).cloned()).unwrap_or_else(|| key_of(&m.name, m.email.as_deref()));
    // People placed by hand who no call shows (yet).
    for p in threads {
        for m in &p.people {
            seen.entry(member_key(m)).or_insert_with(|| Seen { name: display(&m.name), email: m.email.clone(), ..Default::default() });
        }
    }
    let work = work_of(threads, today);
    let last_call = dirs.iter().filter_map(|d| calls::read_json(&d.join("meta.json"))["started_at"].as_str().map(|s| s.get(..10).unwrap_or("").to_string())).max().unwrap_or_default();
    let days = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok().map(|x| (today - x).num_days());
    let book = colleagues();
    let mut people: Vec<Value> = seen
        .into_iter()
        .map(|(key, mut s)| {
            let entry = dir_people.iter().find(|p| p.id == key || s.email.as_ref().is_some_and(|e| p.emails.contains(e)));
            let f = |k: &str| entry.and_then(|p| p.fields.get(k).cloned());
            // A colleague shown on their own address; a client address they were invited on stays in `also`.
            if let Some(i) = s.also.iter().position(|e| book.address(e)).filter(|_| !s.email.as_deref().is_some_and(|e| book.address(e))) {
                let own = s.also.remove(i);
                s.also.extend(s.email.replace(own));
            }
            let colleague = s.email.as_deref().is_some_and(|e| book.address(e)) || entry.is_some_and(|p| p.emails.iter().any(|e| book.address(e)));
            let dom = domain(s.email.as_deref());
            let org = f("org").and_then(|o| clients.iter().find(|c| c.id == o)).or_else(|| org_of(&clients, dom.as_deref())).filter(|c| !colleague || c.internal || f("org").is_some());
            let side = match (org, &dom) {
                (Some(c), _) if c.internal => "yours",
                (None, _) if colleague => "yours",
                (Some(_), _) => "org",
                (None, Some(d)) if Some(d) == my_domain.as_ref() => "yours",
                (None, Some(_)) => "other",
                (None, None) => "unknown",
            };
            let places: Vec<Value> = threads
                .iter()
                .filter_map(|p| {
                    let m = p.people.iter().find(|m| member_key(m) == key || m.email.as_ref().is_some_and(|e| s.also.contains(&e.to_lowercase())) || (m.email.is_none() && s.email.is_some() && norm(&m.name) == norm(&s.name)))?;
                    Some(json!({"thread": p.id, "role": m.role, "stance": m.place.stance, "influence": m.place.influence, "interest": m.place.interest}))
                })
                .collect();
            let n = norm(entry.map(|p| p.name.as_str()).unwrap_or(&s.name));
            let first_name = n.split(' ').next().unwrap_or("").to_string();
            let (owes, late, waiting) = work.iter().filter(|(w, ..)| *w == n || *w == first_name).fold((0, 0, 0), |a, (_, x, y, z)| (a.0 + x, a.1 + y, a.2 + z));
            let in_thread = !places.is_empty();
            let new = in_thread && !s.first.is_empty() && days(&s.first).is_some_and(|d| d <= NEW_DAYS);
            let quiet = in_thread && !s.last.is_empty() && days(&s.last).is_some_and(|d| d > QUIET_DAYS) && last_call > s.last;
            // What the threads' words say about them, newest first; and where they were seen.
            let mut facts: Vec<Value> = entry.map(|p| p.facts.iter().filter(|f| f["thread"].as_str().is_some_and(|t| ids.contains(t))).cloned().collect()).unwrap_or_default();
            facts.reverse();
            let mut seen_in: BTreeSet<String> = s.calls.iter().flat_map(|c| call_threads.get(c).cloned().unwrap_or_default()).collect();
            seen_in.extend(places.iter().filter_map(|p| p["thread"].as_str().map(str::to_owned)));
            seen_in.extend(facts.iter().filter_map(|f| f["thread"].as_str().map(str::to_owned)));
            let touches = s.calls.iter().cloned().chain(facts.iter().filter_map(|f| f["evidence"].as_str().map(str::to_owned))).collect::<BTreeSet<_>>().len();
            let with_them: Vec<Value> = with.get(&key).into_iter().flatten().map(|(n, k)| json!({"key": k, "calls": n})).collect();
            let addresses: Vec<Value> = s.email.iter().chain(&s.also).map(|e| {
                let d = domain(Some(e));
                json!({"email": e, "org": org_of(&clients, d.as_deref()).map(|c| c.name.clone()), "domain": d})
            }).collect();
            json!({
                "key": key, "name": entry.map(|p| p.name.clone()).unwrap_or(s.name), "email": s.email, "also": s.also, "addresses": addresses, "domain": dom,
                "org": org.map(|c| json!({"id": c.id, "name": c.name})), "side": side,
                "title": f("title"), "department": f("department"), "company": f("company"), "office": f("office"), "questions": entry.map(|p| json!(p.questions)).unwrap_or(json!([])), "group": f("group"), "reports_to": f("reports_to"), "matters": f("matters"), "notes": f("notes"), "phone": f("phone"),
                "sources": entry.map(|p| json!(p.sources)).unwrap_or(json!({})), "history": entry.map(|p| json!(p.history)).unwrap_or(json!([])),
                "calls": s.calls.len(), "of_calls": dirs.len(), "invited": s.invited, "chat": s.chat,
                "share": if total > 0.0 { (s.spoke / total * 100.0).round() } else { 0.0 },
                "first": s.first, "last": s.last, "new": new, "quiet": quiet,
                "owes": owes, "late": late, "waiting": waiting,
                "places": places, "in_thread": in_thread,
                "about": f("about"), "about_by": entry.and_then(|p| p.sources.get("about").cloned()), "about_thread": f("about_thread"),
                "facts": facts, "threads": seen_in, "touches": touches, "with": with_them,
            })
        })
        .collect();
    // Maybe the same person: names (or addresses) that share a first and a last name, unless the user
    // said they aren't. A suggestion in the panel, never merged by itself.
    let words: Vec<(String, BTreeSet<String>)> = people
        .iter()
        .map(|p| {
            let mut t = tokens(&display(p["name"].as_str().unwrap_or("")));
            if t.len() < 2 {
                t.extend(p["email"].as_str().and_then(|e| e.split('@').next()).map(tokens).unwrap_or_default());
            }
            (p["key"].as_str().unwrap_or("").to_string(), t)
        })
        .collect();
    let mut maybe: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (i, (ka, ta)) in words.iter().enumerate() {
        for (kb, tb) in words.iter().skip(i + 1) {
            if ta.len() >= 2 && tb.len() >= 2 && ta.intersection(tb).count() >= 2 && !apart(&dir_people, ka, kb) {
                let a = &people[i];
                let b = people.iter().find(|p| p["key"] == kb.as_str()).unwrap();
                maybe.entry(ka.clone()).or_default().push(json!({"key": kb, "name": b["name"], "email": b["email"]}));
                maybe.entry(kb.clone()).or_default().push(json!({"key": ka, "name": a["name"], "email": a["email"]}));
            }
        }
    }
    for p in people.iter_mut() {
        p["maybe_same"] = json!(maybe.remove(p["key"].as_str().unwrap_or("")).unwrap_or_default());
    }
    people.sort_by(|a, b| b["in_thread"].as_bool().cmp(&a["in_thread"].as_bool()).then(b["calls"].as_u64().cmp(&a["calls"].as_u64())).then(a["name"].as_str().cmp(&b["name"].as_str())));
    // Organisations: the threads' clients, the ones their people come from, and those related to them.
    let mut ids: BTreeSet<String> = threads.iter().filter_map(|p| p.client.clone()).collect();
    ids.extend(people.iter().filter(|p| p["in_thread"] == true).filter_map(|p| p["org"]["id"].as_str().map(str::to_owned)));
    ids.extend(threads.iter().flat_map(|p| p.orgs.iter().map(|o| o.client.clone())));
    let related: Vec<String> = clients.iter().filter(|c| ids.contains(&c.id)).flat_map(|c| c.parent.iter().cloned().chain(c.relations.iter().map(|r| r.to.clone()))).collect();
    ids.extend(related);
    let orgs: Vec<Value> = clients
        .iter()
        .filter(|c| ids.contains(&c.id))
        .map(|c| {
            let places: Vec<Value> = threads
                .iter()
                .filter_map(|p| p.orgs.iter().find(|o| o.client == c.id).map(|o| json!({"thread": p.id, "role": o.role, "stance": o.place.stance, "influence": o.place.influence, "interest": o.place.interest})))
                .collect();
            json!({"id": c.id, "name": c.name, "internal": c.internal, "account": c.account, "parent": c.parent, "relations": c.relations, "groups": c.groups,
                   "domains": c.domains, "logo_v": projects::client_logo_v(&c.id), "places": places,
                   "people": people.iter().filter(|p| p["org"]["id"] == c.id.as_str()).count()})
        })
        .collect();
    let pending: Vec<Value> = threads.iter().flat_map(|p| suggestions(&p.id).into_iter().filter(|x| x["state"] == "pending").map(move |mut x| { x["thread"] = json!(p.id); x })).collect();
    Ok(json!({"people": people, "orgs": orgs, "calls": dirs.len(), "suggestions": pending, "threads": threads.iter().map(|p| json!({"id": p.id, "name": p.name})).collect::<Vec<_>>()}))
}

/// One thread's People tab.
pub fn of_thread(id: &str, today: chrono::NaiveDate) -> Result<Value> {
    let p = projects::load(id)?;
    view(&[&p], today)
}

/// A client's People tab: across its threads.
pub fn of_client(id: &str, today: chrono::NaiveDate) -> Result<Value> {
    let ps: Vec<Project> = projects::list().into_iter().filter(|p| p.client.as_deref() == Some(id)).collect();
    view(&ps.iter().collect::<Vec<_>>(), today)
}

/// What the people view is built from, as file times: the directory, clients, each thread's profile,
/// suggestions and ledger events, the calls folder (a call added or removed), the user's settings. Any
/// change — an edit here, a sync, a new call — makes a new one.
fn inputs(ps: &[Project]) -> String {
    let t = |p: PathBuf| std::fs::metadata(&p).and_then(|m| m.modified()).ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).unwrap_or(0);
    let mut out = vec![t(file()), t(projects::dir().join("clients.json")), t(crate::paths::calls_dir()), t(crate::paths::state_dir().join("config.json"))];
    for p in ps {
        let d = projects::dir().join(&p.id);
        out.extend([t(d.join("profile.json")), t(suggestions_file(&p.id)), t(d.join("ledger").join("events.jsonl"))]);
    }
    format!("{}|{}|{out:?}", projects::dir().display(), ps.iter().map(|p| p.id.as_str()).collect::<Vec<_>>().join(","))
}

/// The last view built, by its inputs (and the day: new and quiet are counted from it).
static VIEW: std::sync::Mutex<Option<(String, Value)>> = std::sync::Mutex::new(None);

/// `view`, built again only when its inputs changed.
fn view_cached(ps: &[Project], today: chrono::NaiveDate) -> Result<Value> {
    // And every 10 minutes regardless: a call's speakers named later change none of these times.
    let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 600).unwrap_or(0);
    let key = format!("{today}|{tick}|{}", inputs(ps));
    if let Some((k, v)) = VIEW.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        if *k == key {
            return Ok(v.clone());
        }
    }
    let v = view(&ps.iter().collect::<Vec<_>>(), today)?;
    *VIEW.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, v.clone()));
    Ok(v)
}

/// Everyone across every thread: the Brain's People.
pub fn everyone(today: chrono::NaiveDate) -> Result<Value> {
    view_cached(&projects::list(), today)
}

/// The people of some threads (what a Brain reader may see), with their threads named.
fn of_threads(ids: &[String]) -> Result<(Vec<Value>, BTreeMap<String, String>)> {
    let ps: Vec<Project> = projects::list().into_iter().filter(|p| ids.contains(&p.id)).collect();
    let names = ps.iter().map(|p| (p.id.clone(), p.name.clone())).collect();
    if ps.is_empty() {
        return Ok((vec![], names));
    }
    let v = view_cached(&ps, chrono::Local::now().date_naive())?;
    Ok((v["people"].as_array().cloned().unwrap_or_default(), names))
}

/// A person as a reader sees them in a list: who, where from, which threads and their role there.
fn brief(x: &Value, names: &BTreeMap<String, String>) -> Value {
    let role = |t: &str| x["places"].as_array().into_iter().flatten().find(|p| p["thread"] == t).and_then(|p| p["role"].as_str().map(str::to_owned));
    let threads: Vec<Value> = x["threads"].as_array().into_iter().flatten().filter_map(Value::as_str).map(|t| json!({"id": t, "name": names.get(t), "role": role(t)})).collect();
    json!({"key": x["key"], "name": x["name"], "email": x["email"], "organisation": x["org"]["name"].as_str().or(match x["side"].as_str() { Some("yours") => Some("the user's company"), _ => None }),
           "title": x["title"], "group": x["group"], "threads": threads, "calls": x["calls"], "last_seen": x["last"], "about": about_for(x, names)})
}

/// Who someone is, for a reader of these threads: the user's own words, or Echo's when a pass on one
/// of them wrote it (another thread's may rest on what this reader may not see).
fn about_for(x: &Value, threads: &BTreeMap<String, String>) -> Value {
    let ok = x["about_by"] == "you" || x["about_thread"].as_str().is_some_and(|t| threads.contains_key(t));
    if ok { x["about"].clone() } else { Value::Null }
}

/// `find_people`: the people of these threads whose name, address, organisation, title, group or role
/// has every word of the query; the most seen first.
pub fn find(threads: &[String], query: &str, limit: usize) -> Result<Value> {
    let (people, names) = of_threads(threads)?;
    let words: Vec<String> = fold(&query.to_lowercase()).split_whitespace().map(str::to_owned).collect();
    let mut seen: Vec<&Value> = people.iter().filter(|x| x["touches"].as_u64().unwrap_or(0) > 0 || x["in_thread"] == true).collect();
    seen.sort_by_key(|x| std::cmp::Reverse(x["touches"].as_u64().unwrap_or(0)));
    let mut hits: Vec<Value> = seen
        .into_iter()
        .map(|x| brief(x, &names))
        .filter(|b| {
            // The values only (a field's name isn't a match).
            let mut hay: Vec<&str> = ["name", "email", "organisation", "title", "group", "about"].iter().filter_map(|k| b[*k].as_str()).collect();
            hay.extend(b["threads"].as_array().into_iter().flatten().flat_map(|t| [t["name"].as_str(), t["role"].as_str()]).flatten());
            let hay = fold(&hay.join(" ").to_lowercase());
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect();
    let total = hits.len();
    hits.truncate(limit);
    Ok(json!({"people": hits, "total": total, "guidance": "get_person gives one person's facts with their sources. Cite them; say 'not modeled' for what they don't say."}))
}

/// `get_person`: one person of these threads (by key, address or name) with the facts said about them
/// there, each with its words and a link (a call's moment, the mail).
pub fn card(threads: &[String], who: &str, max_facts: usize) -> Result<Value> {
    let (people, names) = of_threads(threads)?;
    let w = who.trim().to_lowercase();
    let x = people.iter().find(|x| x["key"] == w.as_str()).or_else(|| find_person(&people, who));
    let Some(x) = x else { return Ok(json!({"status": "not_found", "reason": format!("nobody called {who:?} in the threads this session may read")})) };
    let name_of = |k: &str| people.iter().find(|y| y["key"] == k).map(|y| y["name"].clone()).unwrap_or(json!(k));
    let facts: Vec<Value> = x["facts"].as_array().into_iter().flatten().take(max_facts).map(|f| {
        let link = match (f["kind"].as_str(), f["call"].as_str()) {
            (Some("mail"), _) => f["url"].as_str().map(str::to_owned),
            (_, Some(call)) => Some(match f["ts"].as_str().map(|t| t.split(':').fold(0u64, |a, x| a * 60 + x.parse::<u64>().unwrap_or(0))) {
                Some(t) => format!("echo://call/{call}?t={t}"),
                None => format!("echo://call/{call}"),
            }),
            _ => None,
        };
        json!({"fact": f["text"], "quote": f["quote"], "date": f["date"], "thread": names.get(f["thread"].as_str().unwrap_or("")), "source": f["title"], "kind": f["kind"], "link": link})
    }).collect();
    let mut v = brief(x, &names);
    v["places"] = json!(x["places"].as_array().into_iter().flatten().map(|p| json!({"thread": names.get(p["thread"].as_str().unwrap_or("")), "role": p["role"], "stance": p["stance"], "influence": p["influence"], "interest": p["interest"]})).collect::<Vec<_>>());
    v["reports_to"] = x["reports_to"].as_str().map(name_of).unwrap_or(Value::Null);
    v["first_seen"] = x["first"].clone();
    v["share_of_talking"] = x["share"].clone();
    v["tasks"] = json!({"owes": x["owes"], "late": x["late"], "waiting_on_them": x["waiting"]});
    v["often_with"] = json!(x["with"].as_array().into_iter().flatten().map(|y| json!({"name": name_of(y["key"].as_str().unwrap_or("")), "calls_together": y["calls"]})).collect::<Vec<_>>());
    v["facts"] = json!(facts);
    v["guidance"] = json!("Each fact has the words it rests on and a link: cite them. `about` is Echo's summary (or the user's own words).");
    Ok(v)
}

/// Place a person in a thread (adding them if they aren't in it): role, stance, influence, interest
/// — each only when given (`""` clears it).
pub fn place(thread: &str, name: &str, email: Option<&str>, v: &Value) -> Result<()> {
    let key = key_of(name, email);
    projects::update(thread, |p| {
        let i = match p.people.iter().position(|m| key_of(&m.name, m.email.as_deref()) == key) {
            Some(i) => i,
            None => {
                p.people.push(projects::Member { name: name.trim().into(), email: email.map(|e| e.trim().to_lowercase()), ..Default::default() });
                p.people.len() - 1
            }
        };
        let m = &mut p.people[i];
        set_place(&mut m.role, &mut m.place, v)
    })?;
    Ok(())
}

/// A thread's counterpart organisation: link one (`client`, an organisation's id) or unlink it.
pub fn link_org(thread: &str, client: &str, on: bool) -> Result<()> {
    if on && !projects::orgs()?.iter().any(|c| c.id == client) {
        bail!("no organisation {client:?}");
    }
    projects::update(thread, |p| {
        p.orgs.retain(|o| o.client != client || on);
        if on && !p.orgs.iter().any(|o| o.client == client) {
            p.orgs.push(projects::OrgPlace { client: client.into(), ..Default::default() });
        }
        Ok(())
    })?;
    Ok(())
}

/// "Name this organisation": a new organisation (not an account) for the people of a mail domain no
/// organisation has, made the thread's counterpart when one is given.
pub fn name_org(name: &str, domain: &str, thread: Option<&str>) -> Result<projects::Client> {
    let d = domain.trim().trim_start_matches('@').to_lowercase();
    if d.is_empty() || !d.contains('.') {
        bail!("a mail domain, like acme.example");
    }
    let o = projects::add_org(name, &[d], false, false)?;
    if let Some(t) = thread {
        link_org(t, &o.id, true)?;
    }
    Ok(o)
}

/// Place an organisation in a thread, as `place` does a person.
pub fn place_org(thread: &str, client: &str, v: &Value) -> Result<()> {
    if !projects::orgs()?.iter().any(|c| c.id == client) {
        bail!("no organisation {client:?}");
    }
    projects::update(thread, |p| {
        let i = match p.orgs.iter().position(|o| o.client == client) {
            Some(i) => i,
            None => {
                p.orgs.push(projects::OrgPlace { client: client.into(), ..Default::default() });
                p.orgs.len() - 1
            }
        };
        let o = &mut p.orgs[i];
        set_place(&mut o.role, &mut o.place, v)
    })?;
    Ok(())
}

fn set_place(role: &mut Option<String>, place: &mut projects::Place, v: &Value) -> Result<()> {
    let text = |k: &str| v.get(k).map(|x| x.as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned));
    let level = |k: &str| v.get(k).map(|x| x.as_u64().map(|n| n as u8).or_else(|| x.as_str().and_then(|s| s.parse().ok())));
    if let Some(r) = text("role") {
        *role = r;
    }
    if let Some(s) = text("stance") {
        place.stance = s;
    }
    if let Some(n) = level("influence") {
        place.influence = n;
    }
    if let Some(n) = level("interest") {
        place.interest = n;
    }
    place.check()
}

// ── suggestions from the sync's people pass ───────────────────────────────────────────────
// The agent proposes, with the words; the CLI checks the words and the person; the user decides.
// Nothing here changes a person, a placement or an organisation until the user says yes.

/// What a proposal may say about a person (in this thread, or in the directory) or an organisation.
pub const SUGGEST_FIELDS: [&str; 9] = ["role", "stance", "influence", "interest", "title", "group", "reports_to", "left", "relation"];

fn suggestions_file(project: &str) -> PathBuf {
    projects::dir().join(project).join("people-suggestions.json")
}

pub fn suggestions(project: &str) -> Vec<Value> {
    std::fs::read(suggestions_file(project)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// What the people pass reads: the thread's people and organisations as they stand, the calls of
/// this run (their readable files), and what was already proposed or turned down.
pub fn context(project: &str, items: &[String]) -> Result<Value> {
    let v = of_thread(project, chrono::Local::now().date_naive())?;
    let l = crate::ledger::Ledger::of(project)?;
    let name_of = |k: &str| v["people"].as_array().into_iter().flatten().find(|x| x["key"] == k).and_then(|x| x["name"].as_str().map(str::to_owned));
    let people: Vec<Value> = v["people"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["touches"].as_u64().unwrap_or(0) > 0 || x["in_thread"] == true)
        .map(|x| {
            let pl = x["places"].get(0).cloned().unwrap_or(json!({}));
            json!({"name": x["name"], "email": x["email"], "organisation": x["org"]["name"].as_str().unwrap_or(match x["side"].as_str() { Some("yours") => "the user's company", _ => "" }),
                   "in_thread": x["in_thread"], "title": x["title"], "group": x["group"], "reports_to": x["reports_to"].as_str().and_then(name_of),
                   "role": pl["role"], "stance": pl["stance"], "influence": pl["influence"], "interest": pl["interest"], "calls": x["calls"],
                   "about": if x["about_by"] == "you" || x["about_thread"] == project { x["about"].clone() } else { Value::Null }, "about_is_users": x["about_by"] == "you",
                   "facts": x["facts"].as_array().into_iter().flatten().take(12).map(|f| json!({"date": f["date"], "fact": f["text"]})).collect::<Vec<_>>()})
        })
        .collect();
    let orgs: Vec<Value> = v["orgs"].as_array().into_iter().flatten().map(|o| json!({"name": o["name"], "relations": o["relations"], "parent": o["parent"]})).collect();
    let known = projects::orgs()?.into_iter().map(|c| c.name).collect::<Vec<_>>();
    let said: Vec<Value> = suggestions(project).into_iter().map(|x| json!({"who": x["who"], "field": x["field"], "value": x["value"], "state": x["state"]})).collect();
    Ok(json!({
        "project": project, "people": people, "organisations": orgs, "known_organisations": known,
        "me_profile": crate::calls::me_profile(),
        "items": items.iter().map(|i| json!({"id": i, "file": l.evidence_md(i).display().to_string().replace('\\', "/")})).collect::<Vec<_>>(),
        "already_proposed": said,
        "fields": SUGGEST_FIELDS.iter().chain(&["fact", "about"]).collect::<Vec<_>>(), "stances": projects::STANCES, "relations": projects::RELATIONS,
    }))
}

/// Find who a proposal is about among the thread's people: by address, by cleaned full name, else a
/// first name only one of them has.
fn find_person<'a>(people: &'a [Value], who: &str) -> Option<&'a Value> {
    let w = who.trim().to_lowercase();
    if w.contains('@') {
        return people.iter().find(|x| x["email"].as_str().is_some_and(|e| e == w) || x["also"].as_array().into_iter().flatten().any(|e| e == w.as_str()));
    }
    let n = norm(who);
    if let Some(x) = people.iter().find(|x| norm(x["name"].as_str().unwrap_or("")) == n) {
        return Some(x);
    }
    let first: Vec<&Value> = people.iter().filter(|x| norm(x["name"].as_str().unwrap_or("")).split(' ').next() == Some(n.as_str())).collect();
    (first.len() == 1).then(|| first[0])
}

/// Check proposals for one evidence item and keep the good ones as suggestions: the words must be
/// one span of the item, the person (or organisation) known, the value valid and new. Returns
/// `{added, rejected: [{i, why}]}`.
pub fn apply_suggestions(project: &str, evid: &str, proposals: &[Value]) -> Result<Value> {
    let v = of_thread(project, chrono::Local::now().date_naive())?;
    let people = v["people"].as_array().cloned().unwrap_or_default();
    let clients = projects::orgs()?;
    let dir_people = directory()?;
    let me = MeIs::now();
    // Facts and who someone is go in at once (their words are checked; nothing to decide).
    let mut facts: Vec<(String, String, Option<String>, Value)> = vec![];
    let mut abouts: Vec<(String, String, Option<String>, String)> = vec![];
    let org_named = |n: &str| clients.iter().find(|c| norm(&c.name) == norm(n) || c.aliases.iter().any(|a| norm(a) == norm(n)) || c.id == n);
    let mut all = suggestions(project);
    let (mut added, mut rejected) = (0, vec![]);
    for (i, pr) in proposals.iter().enumerate() {
        let field = pr["field"].as_str().unwrap_or("");
        let value = match &pr["value"] {
            Value::String(x) => x.trim().to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        };
        let who = pr["who"].as_str().unwrap_or("");
        let why: Option<String> = (|| {
            if field == "fact" || field == "about" {
                let (key, name, email) = match find_person(&people, who) {
                    Some(x) => (x["key"].as_str().unwrap_or("").to_string(), x["name"].as_str().unwrap_or("").to_string(), x["email"].as_str().map(str::to_owned)),
                    // A mail's sender or recipient by address, not in a call yet — only one of this mail's.
                    None if field == "fact" && who.contains('@') && !who.contains(char::is_whitespace) => {
                        let e = who.trim().to_lowercase();
                        let item = crate::ledger::Ledger::of(project).ok().and_then(|l| l.evidence().get(evid).cloned()).unwrap_or_default();
                        let addrs: Vec<String> = std::iter::once(&item["from"]).chain(item["to"].as_array().into_iter().flatten()).filter_map(|p| p["email"].as_str().map(str::to_lowercase)).collect();
                        if !addrs.contains(&e) {
                            return Some(format!("{who:?} isn't among the thread's people nor this mail's sender or recipients"));
                        }
                        let name = pr["name"].as_str().map(display).filter(|n| !n.is_empty()).unwrap_or_else(|| e.clone());
                        (canon(&dir_people).get(&e).cloned().unwrap_or_else(|| e.clone()), name, Some(e))
                    }
                    None => return Some(format!("{who:?} isn't among the thread's people (use a name or an address from people-context)")),
                };
                if me.is(&name, email.as_deref()) {
                    return Some("nothing about the user".into());
                }
                let n = value.chars().count();
                if field == "about" {
                    if n == 0 || n > MAX_AGENT_ABOUT {
                        return Some(format!("who someone is: 1 to {MAX_AGENT_ABOUT} characters"));
                    }
                    if dir_people.iter().any(|p| p.id == key && p.sources.get("about").map(String::as_str) == Some("you")) {
                        return Some("the user wrote who they are: leave it".into());
                    }
                    abouts.push((key, name, email, value.clone()));
                    return None;
                }
                if n == 0 || n > MAX_FACT {
                    return Some(format!("a fact is one line of 1 to {MAX_FACT} characters"));
                }
                let quote = pr["quote"].as_str().unwrap_or("");
                let at = match crate::ledger::quote_in(project, evid, quote) {
                    Ok(Some(at)) => at,
                    Ok(None) => return Some("quote not verbatim in this evidence item (one contiguous span, 3 words or more)".into()),
                    Err(e) => return Some(format!("{e:#}")),
                };
                let id = format!("{:x}", { use sha2::Digest as _; let d = sha2::Sha256::digest(format!("{key}\0{evid}\0{}", norm(quote)).as_bytes()); u64::from_be_bytes(d[..8].try_into().unwrap()) });
                facts.push((key, name, email, json!({"id": id, "text": value, "quote": quote, "date": at["date"], "thread": project, "evidence": evid,
                    "kind": at["kind"], "call": at["call"], "title": at["title"], "url": at["url"], "ts": at["ts"], "added": chrono::Utc::now().to_rfc3339()})));
                return None;
            }
            if !SUGGEST_FIELDS.contains(&field) {
                return Some(format!("field is one of {SUGGEST_FIELDS:?}, fact or about"));
            }
            let at = match crate::ledger::quote_in(project, evid, pr["quote"].as_str().unwrap_or("")) {
                Ok(Some(at)) => at,
                Ok(None) => return Some("quote not verbatim in this evidence item (one contiguous span, 3 words or more)".into()),
                Err(e) => return Some(format!("{e:#}")),
            };
            let (target, current): (Value, Option<String>) = if field == "relation" {
                let (Some(a), Some(b)) = (org_named(who), org_named(pr["to"].as_str().unwrap_or(""))) else {
                    return Some("a relation names two known organisations: `who` and `to`".into());
                };
                if !projects::RELATIONS.contains(&value.as_str()) {
                    return Some(format!("a relation is one of {:?}", projects::RELATIONS));
                }
                let cur = a.relations.iter().find(|r| r.to == b.id).map(|r| r.kind.clone());
                (json!({"org": a.id, "name": a.name, "to": b.id, "to_name": b.name}), cur)
            } else {
                let Some(x) = find_person(&people, who) else { return Some(format!("{who:?} isn't among the thread's people (use a name or an address from people-context)")) };
                let pl = x["places"].get(0).cloned().unwrap_or(json!({}));
                let cur = match field {
                    "role" | "stance" => pl[field].as_str().map(str::to_owned),
                    "influence" | "interest" => pl[field].as_u64().map(|n| n.to_string()),
                    "left" => (x["in_thread"] != true).then(|| "true".to_string()),
                    "reports_to" => x["reports_to"].as_str().map(str::to_owned),
                    f => x[f].as_str().map(str::to_owned),
                };
                (json!({"key": x["key"], "name": x["name"], "email": x["email"]}), cur)
            };
            let value_ok = match field {
                "stance" => projects::STANCES.contains(&value.as_str()),
                "influence" | "interest" => matches!(value.as_str(), "1" | "2" | "3"),
                "left" => value == "true",
                "reports_to" => find_person(&people, &value).is_some(),
                _ => !value.is_empty() && value.chars().count() <= 80,
            };
            if !value_ok {
                return Some(format!("bad value {value:?} for {field}"));
            }
            let value = if field == "reports_to" { find_person(&people, &value).and_then(|x| x["key"].as_str()).unwrap_or("").to_string() } else { value.clone() };
            if current.as_deref() == Some(value.as_str()) {
                return Some("already so".into());
            }
            let tkey = target["key"].as_str().or(target["org"].as_str()).unwrap_or("").to_string();
            if all.iter().any(|x| x["target_key"] == tkey.as_str() && x["field"] == field && x["value"] == value.as_str() && x["state"] != "accepted") {
                return Some("already proposed (or turned down)".into());
            }
            let sid = format!("{:x}", { use sha2::Digest as _; let d = sha2::Sha256::digest(format!("{tkey}\0{field}\0{value}\0{evid}").as_bytes()); u64::from_be_bytes(d[..8].try_into().unwrap()) });
            all.push(json!({"sid": sid, "who": who, "target": target, "target_key": tkey, "field": field, "value": value,
                            "value_name": if field == "reports_to" { find_person(&people, pr["value"].as_str().unwrap_or("")).map(|x| x["name"].clone()) } else { None },
                            "quote": pr["quote"], "note": pr["note"], "at": at, "state": "pending", "proposed": chrono::Utc::now().to_rfc3339()}));
            added += 1;
            None
        })();
        if let Some(w) = why {
            rejected.push(json!({"i": i, "why": w}));
        }
    }
    projects::with_lock(|| projects::write_json(&suggestions_file(project), &all))?;
    let (n_facts, n_about) = (facts.len(), abouts.len());
    if n_facts + n_about > 0 {
        projects::with_lock(|| {
            let mut dir = directory()?;
            let now = chrono::Utc::now().to_rfc3339();
            for (key, name, email, f) in facts {
                let p = entry(&mut dir, &key, &name, email.as_deref());
                if !p.facts.iter().any(|x| x["id"] == f["id"]) {
                    p.facts.push(f);
                    p.facts.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
                    let over = p.facts.len().saturating_sub(MAX_FACTS);
                    p.facts.drain(..over);
                }
            }
            for (key, name, email, text) in abouts {
                let p = entry(&mut dir, &key, &name, email.as_deref());
                if p.sources.get("about").map(String::as_str) != Some("you") && p.fields.get("about") != Some(&text) {
                    p.history.push(json!({"field": "about", "from": null, "to": null, "source": "echo", "at": now}));
                    p.fields.insert("about".into(), text);
                    p.fields.insert("about_thread".into(), project.into());
                    p.sources.insert("about".into(), "echo".into());
                }
            }
            projects::write_json(&file(), &dir)
        })?;
    }
    Ok(json!({"added": added, "facts": n_facts, "about": n_about, "rejected": rejected}))
}

/// A person's directory entry (by key or one of their addresses), made when there is none.
fn entry<'a>(all: &'a mut Vec<Person>, key: &str, name: &str, email: Option<&str>) -> &'a mut Person {
    let i = match all.iter().position(|p| p.id == key || email.is_some_and(|e| p.emails.iter().any(|x| x == e))) {
        Some(i) => i,
        None => {
            all.push(Person { id: key.into(), name: name.trim().into(), emails: email.map(|e| vec![e.trim().to_lowercase()]).unwrap_or_default(), ..Default::default() });
            all.len() - 1
        }
    };
    &mut all[i]
}

/// The user's answer to a suggestion: yes applies it as theirs (a placement, a directory field, a
/// relationship, or taking someone out of the thread); no keeps it turned down (not proposed again).
/// `value` = the user's own version instead of the proposed one ("Change…"): checked as a proposal
/// is, applied as theirs, and the proposed value stays turned down.
pub fn decide(project: &str, sid: &str, yes: bool, value: Option<&str>) -> Result<()> {
    let mut all = suggestions(project);
    let x = all.iter_mut().find(|x| x["sid"] == sid).ok_or_else(|| anyhow::anyhow!("no such suggestion"))?;
    if x["state"] != "pending" {
        bail!("already decided");
    }
    let field = x["field"].as_str().unwrap_or("").to_string();
    let own = match value.map(str::trim) {
        Some(v) if yes => Some(corrected(project, &field, v)?),
        _ => None,
    };
    if yes {
        let t = &x["target"];
        let field = field.as_str();
        let value = own.as_deref().unwrap_or(x["value"].as_str().unwrap_or(""));
        let (name, email) = (t["name"].as_str().unwrap_or(""), t["email"].as_str());
        match field {
            "role" | "stance" => place(project, name, email, &json!({field: value}))?,
            "influence" | "interest" => place(project, name, email, &json!({field: value.parse::<u8>().unwrap_or(0)}))?,
            "left" => {
                let key = t["key"].as_str().unwrap_or("").to_string();
                projects::update(project, |p| {
                    p.people.retain(|m| key_of(&m.name, m.email.as_deref()) != key);
                    Ok(())
                })?;
            }
            "relation" => {
                let (org, to) = (t["org"].as_str().unwrap_or(""), t["to"].as_str().unwrap_or("").to_string());
                projects::update_client(org, |c| {
                    c.relations.retain(|r| r.to != to);
                    c.relations.push(projects::Relation { to: to.clone(), kind: value.into(), note: None });
                    Ok(())
                })?;
            }
            f => {
                set(t["key"].as_str().unwrap_or(""), name, email, f, value, "you")?;
            }
        }
    }
    x["state"] = json!(if own.is_some() { "corrected" } else if yes { "accepted" } else { "rejected" });
    if let Some(v) = &own {
        x["chosen"] = json!(v);
    }
    x["decided"] = json!(chrono::Utc::now().to_rfc3339());
    projects::with_lock(|| projects::write_json(&suggestions_file(project), &all))
}

/// The user's own value for a suggestion's field, checked as a proposal's is (`reports_to`: a
/// person of the thread, by key, name or address → their key). Whether someone left is yes or no.
fn corrected(project: &str, field: &str, v: &str) -> Result<String> {
    let ok = match field {
        "stance" => projects::STANCES.contains(&v),
        "relation" => projects::RELATIONS.contains(&v),
        "influence" | "interest" => matches!(v, "1" | "2" | "3"),
        "left" => bail!("whether someone left is a yes or a no"),
        "reports_to" => {
            let people = of_thread(project, chrono::Local::now().date_naive())?["people"].as_array().cloned().unwrap_or_default();
            let x = people.iter().find(|x| x["key"] == v).or_else(|| find_person(&people, v));
            return x.and_then(|x| x["key"].as_str()).map(str::to_owned).ok_or_else(|| anyhow::anyhow!("{v:?} isn't among the thread's people"));
        }
        _ => !v.is_empty() && v.chars().count() <= 80,
    };
    if !ok {
        bail!("bad value {v:?} for {field}");
    }
    Ok(v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(root: &std::path::Path, name: &str, started: &str, meeting: Value, segs: Value) -> PathBuf {
        let _ = root;
        let d = crate::paths::calls_dir().join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), json!({"started_at": started, "title": name}).to_string()).unwrap();
        std::fs::write(d.join("meeting.json"), meeting.to_string()).unwrap();
        std::fs::write(d.join("segments.json"), segs.to_string()).unwrap();
        d
    }

    #[test]
    fn calls_show_who_was_there_and_who_spoke() {
        let (_env, root) = crate::paths::testenv::folders("people-gather");
        let m = json!({"subject": "Aurora weekly", "organizer": {"name": "Dana Kowal", "email": "Dana@Northwind.example"},
                       "attendees": [{"name": "Ravi Mehta", "email": "ravi@northwind.example"}, {"name": "Me Myself", "email": "me@us.example"}]});
        let a = call(&root, "2026-10-01_100000-a", "2026-10-01T10:00:00+00:00", m.clone(),
            json!([{"speaker": "remote", "name": "Dana Kowal", "start": 0.0, "end": 30.0}, {"speaker": "remote", "name": "Speaker 2", "start": 30.0, "end": 40.0}, {"speaker": "me", "start": 40.0, "end": 60.0}]));
        let b = call(&root, "2026-10-20_100000-b", "2026-10-20T10:00:00+00:00", json!({"attendees": [{"name": "Tomás Ruiz", "email": "tomas@northwind.example"}]}),
            json!([{"speaker": "remote", "name": "Dana Kowal", "start": 0.0, "end": 20.0}]));
        let me = MeIs(Some("me myself".into()), Some("me@us.example".into()));
        let (seen, total) = gather(&[a, b], &me, &[]);
        assert_eq!(total, 80.0);
        let dana = &seen["dana@northwind.example"];
        assert_eq!((dana.calls.len(), dana.invited, dana.spoke), (2, 1, 50.0), "named speaker joined to her invite's email");
        assert_eq!((dana.first.as_str(), dana.last.as_str()), ("2026-10-01", "2026-10-20"));
        assert!(!seen.contains_key("me@us.example"), "the user is left out");
        assert!(!seen.keys().any(|k| k.contains("speaker")), "a generated name is nobody");
        assert_eq!(seen.len(), 3);

        // The same full name from a guest account is one person.
        let c = call(&root, "2026-10-21_100000-c", "2026-10-21T10:00:00+00:00", json!({"attendees": [{"name": "Dana Kowal", "email": "dana_kowal@us.example"}, {"name": "Dana", "email": "dana@other.example"}]}), json!([]));
        let (seen, _) = gather(&[c.clone(), crate::paths::calls_dir().join("2026-10-01_100000-a"), crate::paths::calls_dir().join("2026-10-20_100000-b")], &me, &[]);
        let danas: Vec<&Seen> = seen.values().filter(|s| s.name == "Dana Kowal").collect();
        assert_eq!(danas.len(), 1, "one person");
        assert_eq!((danas[0].calls.len(), danas[0].also.len()), (3, 1), "her calls together, the other address kept");
        assert!(seen.contains_key("dana@other.example"), "a first name alone never merges");
    }

    #[test]
    fn the_people_pass_proposes_and_the_user_decides() {
        let (_env, _root) = crate::paths::testenv::folders("people-suggest");
        let p = projects::create("Aurora", None, None).unwrap();
        place(&p.id, "Ravi Mehta", Some("ravi@northwind.example"), &json!({"role": "security"})).unwrap();
        place(&p.id, "Dana Kowal", Some("dana@northwind.example"), &json!({})).unwrap();
        let l = crate::ledger::Ledger::of(&p.id).unwrap();
        std::fs::create_dir_all(l.dir.join("evidence")).unwrap();
        let ev = "2026-10-28_110000";
        std::fs::write(l.dir.join(format!("evidence/{ev}.json")), json!({"id": ev, "date": "2026-10-28", "title": "Aurora weekly", "turns": [
            {"ts": "00:12:21", "speaker": "Ravi Mehta", "text": "I can't sign this off until we see the data-flow diagram."},
            {"ts": "00:14:00", "speaker": "Dana Kowal", "text": "Ravi reports to me now, since the reorg last month."}]}).to_string()).unwrap();
        let r = apply_suggestions(&p.id, ev, &[
            json!({"who": "Ravi Mehta", "field": "stance", "value": "sceptic", "quote": "I can't sign this off until we see the data-flow diagram"}),
            json!({"who": "Ravi", "field": "reports_to", "value": "Dana Kowal", "quote": "Ravi reports to me now"}),
            json!({"who": "Ravi Mehta", "field": "stance", "value": "sceptic", "quote": "words nobody said in the call"}),
            json!({"who": "Lena Fischer", "field": "title", "value": "CIO", "quote": "Ravi reports to me now"}),
            json!({"who": "Ravi Mehta", "field": "stance", "value": "grumpy", "quote": "I can't sign this off"}),
            json!({"who": "Ravi Mehta", "field": "role", "value": "security", "quote": "I can't sign this off"}),
        ]).unwrap();
        assert_eq!(r["added"], 2, "{r}");
        let why: Vec<&str> = r["rejected"].as_array().unwrap().iter().map(|x| x["why"].as_str().unwrap()).collect();
        assert!(why[0].contains("verbatim") && why[1].contains("isn't among") && why[2].contains("bad value") && why[3].contains("already so"), "{why:?}");
        assert_eq!(apply_suggestions(&p.id, ev, &[json!({"who": "Ravi Mehta", "field": "stance", "value": "sceptic", "quote": "I can't sign this off"})]).unwrap()["added"], 0, "proposed once");
        let all = suggestions(&p.id);
        assert_eq!(all[0]["at"]["ts"], "00:12:21");
        // Nothing changed until the user says so.
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let ravi = |v: &Value| v["people"].as_array().unwrap().iter().find(|x| x["name"] == "Ravi Mehta").unwrap().clone();
        assert!(ravi(&of_thread(&p.id, today).unwrap())["places"][0]["stance"].is_null());
        assert_eq!(of_thread(&p.id, today).unwrap()["suggestions"].as_array().unwrap().len(), 2);
        decide(&p.id, all[0]["sid"].as_str().unwrap(), true, None).unwrap();
        decide(&p.id, all[1]["sid"].as_str().unwrap(), false, None).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        assert_eq!(ravi(&v)["places"][0]["stance"], "sceptic");
        assert!(ravi(&v)["reports_to"].is_null(), "turned down");
        assert_eq!(v["suggestions"].as_array().unwrap().len(), 0);
        assert_eq!(apply_suggestions(&p.id, ev, &[json!({"who": "Ravi", "field": "reports_to", "value": "Dana Kowal", "quote": "Ravi reports to me now"})]).unwrap()["added"], 0, "a no stays no");
        // Not quite: the user's own version is applied, the proposed one stays turned down.
        apply_suggestions(&p.id, ev, &[json!({"who": "Ravi Mehta", "field": "role", "value": "approver", "quote": "I can't sign this off"})]).unwrap();
        let sid = suggestions(&p.id).iter().find(|x| x["field"] == "role" && x["state"] == "pending").unwrap()["sid"].as_str().unwrap().to_string();
        assert!(decide(&p.id, &sid, true, Some("grumpy")).is_ok(), "a role is free text");
        assert!(decide(&p.id, &sid, true, Some("x")).unwrap_err().to_string().contains("already decided"));
        apply_suggestions(&p.id, ev, &[json!({"who": "Ravi Mehta", "field": "stance", "value": "blocker", "quote": "I can't sign this off"})]).unwrap();
        let sid = suggestions(&p.id).iter().find(|x| x["field"] == "stance" && x["state"] == "pending").unwrap()["sid"].as_str().unwrap().to_string();
        assert!(decide(&p.id, &sid, true, Some("grumpy")).unwrap_err().to_string().contains("bad value"), "checked as a proposal is");
        decide(&p.id, &sid, true, Some("neutral")).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        assert_eq!(ravi(&v)["places"][0]["stance"], "neutral");
        assert_eq!(ravi(&v)["places"][0]["role"], "grumpy");
        let s = suggestions(&p.id);
        let x = s.iter().find(|x| x["sid"] == sid.as_str()).unwrap();
        assert_eq!((x["state"].as_str(), x["value"].as_str(), x["chosen"].as_str()), (Some("corrected"), Some("blocker"), Some("neutral")));
        assert_eq!(apply_suggestions(&p.id, ev, &[json!({"who": "Ravi Mehta", "field": "stance", "value": "blocker", "quote": "I can't sign this off"})]).unwrap()["added"], 0, "the proposed value stays turned down");
    }

    #[test]
    fn what_is_said_about_someone_is_kept_with_its_words() {
        let (_env, _root) = crate::paths::testenv::folders("people-facts");
        let p = projects::create("Aurora", None, None).unwrap();
        let q = projects::create("Borealis", None, None).unwrap();
        place(&p.id, "Ravi Mehta", Some("ravi@northwind.example"), &json!({"role": "security"})).unwrap();
        let l = crate::ledger::Ledger::of(&p.id).unwrap();
        std::fs::create_dir_all(l.dir.join("evidence")).unwrap();
        let ev = "2026-10-28_110000";
        std::fs::write(l.dir.join(format!("evidence/{ev}.json")), json!({"id": ev, "date": "2026-10-28", "title": "Aurora weekly", "call": "2026-10-28_110000-aurora", "turns": [
            {"ts": "00:12:21", "speaker": "Ravi Mehta", "text": "I own the SSO rollout and I want it done before the March freeze."},
            {"ts": "00:14:00", "speaker": "Lena Fischer", "text": "I'll send the vendor list on Friday, I run procurement now."}]}).to_string()).unwrap();
        let r = apply_suggestions(&p.id, ev, &[
            json!({"who": "Ravi Mehta", "field": "fact", "value": "Owns the SSO rollout; wants it before the March freeze.", "quote": "I own the SSO rollout and I want it done"}),
            json!({"who": "lena@northwind.example", "name": "Lena Fischer", "field": "fact", "value": "Runs procurement.", "quote": "I run procurement now"}),
            json!({"who": "Ravi Mehta", "field": "fact", "value": "Hates Mondays.", "quote": "words nobody said"}),
            json!({"who": "Ravi Mehta", "field": "about", "value": "Northwind's security lead; owns SSO."}),
        ]).unwrap();
        assert_eq!((r["facts"].as_u64(), r["about"].as_u64(), r["rejected"].as_array().unwrap().len()), (Some(1), Some(1), 2), "{r}");
        assert!(r["rejected"][0]["why"].as_str().unwrap().contains("sender or recipients"), "an address must be this item's: {r}");
        // A mail from her: her address is its sender's.
        let mail = "mail-0000aaaa";
        std::fs::write(l.dir.join(format!("evidence/{mail}.json")), json!({"id": mail, "kind": "mail", "date": "2026-10-29", "title": "Vendors",
            "from": {"name": "Lena Fischer", "email": "lena@northwind.example"}, "to": [{"name": "Me", "email": "me@us.example"}],
            "turns": [{"ts": "09:00:00", "speaker": "Lena Fischer", "text": "I run procurement now, so send the vendor list to me."}]}).to_string()).unwrap();
        let r = apply_suggestions(&p.id, mail, &[json!({"who": "lena@northwind.example", "name": "Lena Fischer", "field": "fact", "value": "Runs procurement.", "quote": "I run procurement now"})]).unwrap();
        assert_eq!(r["facts"], 1, "{r}");
        // Read again: nothing twice.
        apply_suggestions(&p.id, ev, &[json!({"who": "Ravi Mehta", "field": "fact", "value": "Owns SSO.", "quote": "i own the SSO rollout and I want it done"})]).unwrap();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        let who = |n: &str| v["people"].as_array().unwrap().iter().find(|x| x["name"] == n).unwrap().clone();
        let ravi = who("Ravi Mehta");
        assert_eq!(ravi["facts"].as_array().unwrap().len(), 1);
        assert_eq!((ravi["facts"][0]["ts"].as_str(), ravi["facts"][0]["thread"].as_str()), (Some("00:12:21"), Some(p.id.as_str())));
        assert_eq!((ravi["about"].as_str(), ravi["about_by"].as_str()), (Some("Northwind's security lead; owns SSO."), Some("echo")));
        assert_eq!(who("Lena Fischer")["threads"], json!([p.id]), "a mail's sender is in the thread by what was said");
        // The user's words stay theirs.
        set(ravi["key"].as_str().unwrap(), "Ravi Mehta", Some("ravi@northwind.example"), "about", "Ravi: careful, wants everything in writing.", "you").unwrap();
        let r = apply_suggestions(&p.id, ev, &[json!({"who": "Ravi Mehta", "field": "about", "value": "Something else."})]).unwrap();
        assert!(r["rejected"][0]["why"].as_str().unwrap().contains("the user wrote"), "{r}");
        // A reader sees the people of its threads only, with links to the moment.
        let c = card(std::slice::from_ref(&p.id), "ravi@northwind.example", 10).unwrap();
        assert_eq!(c["facts"][0]["link"], "echo://call/2026-10-28_110000-aurora?t=741");
        assert_eq!(c["about"], "Ravi: careful, wants everything in writing.");
        assert_eq!(find(std::slice::from_ref(&p.id), "procurement", 10).unwrap()["total"], 0, "facts aren't searched, only who they are");
        assert_eq!(find(std::slice::from_ref(&p.id), "ravi security", 10).unwrap()["total"], 1);
        assert_eq!(card(std::slice::from_ref(&q.id), "Ravi Mehta", 10).unwrap()["status"], "not_found");
        // Echo's summary from one thread isn't shown to a reader of another.
        let mut x = json!({"about": "secret", "about_by": "echo", "about_thread": p.id});
        assert!(about_for(&x, &[(q.id.clone(), "B".into())].into_iter().collect()).is_null());
        x["about_by"] = json!("you");
        assert_eq!(about_for(&x, &BTreeMap::new()), "secret");
    }

    #[test]
    fn names_as_directories_write_them() {
        assert_eq!(display("Varga, Lena (Partner)"), "Lena Varga");
        assert_eq!(display("Pat Novak (external)"), "Pat Novak");
        assert_eq!(display("[EXT] Dana Kowal"), "Dana Kowal");
        assert_eq!(display("Smith, Jr."), "Jr. Smith");
        assert_eq!(norm("Inés Peña Gómez"), "ines pena gomez");
        assert_eq!(tokens("partner.lena.varga"), ["lena", "varga"].map(String::from).into_iter().collect());
    }

    #[test]
    fn one_person_with_addresses_at_two_organisations() {
        let (_env, root) = crate::paths::testenv::folders("people-addresses");
        let a = call(&root, "2026-10-01_100000-a", "2026-10-01T10:00:00+00:00",
            json!({"attendees": [{"name": "Varga, Lena (Partner)", "email": "partner.lena.varga@initech.example"}]}), json!([]));
        let b = call(&root, "2026-10-02_100000-b", "2026-10-02T10:00:00+00:00",
            json!({"attendees": [{"name": "Lena V.", "email": "lena.varga@globex.example"}]}), json!([]));
        let p = projects::create("Globex", None, None).unwrap();
        projects::update(&p.id, |x| { x.pins = vec![calls::stamp_of(&a), calls::stamp_of(&b)]; Ok(()) }).unwrap();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        let lena = v["people"].as_array().unwrap().iter().find(|x| x["email"] == "partner.lena.varga@initech.example").unwrap().clone();
        assert_eq!(lena["name"], "Lena Varga", "the directory's \"Last, First (Partner)\" read as a name");
        assert_eq!(lena["maybe_same"][0]["email"], "lena.varga@globex.example", "the same first and last name in the other address");
        // Merged by the user: one person, both addresses, every call.
        merge("lena.varga@globex.example", "Lena V.", &[], "partner.lena.varga@initech.example", "Lena Varga", Some("partner.lena.varga@initech.example")).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        let ks: Vec<&Value> = v["people"].as_array().unwrap().iter().filter(|x| x["name"] == "Lena Varga").collect();
        assert_eq!(ks.len(), 1);
        assert_eq!((ks[0]["calls"].as_u64(), ks[0]["addresses"].as_array().unwrap().len()), (Some(2), 2));
        assert_eq!(ks[0]["maybe_same"].as_array().unwrap().len(), 0);
        // Split again: two people, never suggested as one.
        split("partner.lena.varga@initech.example", "Lena Varga", "lena.varga@globex.example").unwrap();
        let v = of_thread(&p.id, today).unwrap();
        assert_eq!(v["people"].as_array().unwrap().len(), 2);
        assert!(v["people"].as_array().unwrap().iter().all(|x| x["maybe_same"].as_array().unwrap().is_empty()));
    }

    #[test]
    fn placements_and_the_directory_are_kept_with_history() {
        let (_env, _root) = crate::paths::testenv::folders("people-place");
        let p = projects::create("Aurora", None, None).unwrap();
        place(&p.id, "Dana Kowal", Some("dana@northwind.example"), &json!({"stance": "champion", "influence": 3, "interest": "3", "role": "sponsor"})).unwrap();
        place(&p.id, "Dana Kowal", Some("DANA@northwind.example"), &json!({"stance": ""})).unwrap();
        let m = &projects::load(&p.id).unwrap().people[0];
        assert_eq!((m.role.as_deref(), m.place.stance.as_deref(), m.place.influence), (Some("sponsor"), None, Some(3)));
        assert!(place(&p.id, "Dana Kowal", None, &json!({"stance": "fan"})).is_err());
        assert!(place(&p.id, "Dana Kowal", None, &json!({"influence": 5})).is_err());
        let k = key_of("Dana Kowal", Some("dana@northwind.example"));
        set(&k, "Dana Kowal", Some("dana@northwind.example"), "title", "Head of IT", "you").unwrap();
        let d = set(&k, "Dana Kowal", None, "title", "CIO", "you").unwrap();
        assert_eq!(d.fields["title"], "CIO");
        assert_eq!(d.history.len(), 2);
        assert_eq!(d.history[1]["from"], "Head of IT");
        assert!(set(&k, "x", None, "matters", "very", "you").is_err());

        // Microsoft 365 fills what nobody set; where the user set something else, it asks.
        let mut d = directory().unwrap().into_iter().find(|p| p.id == k).unwrap();
        let said = [("title", "Head of IT".to_string()), ("department", "IT".to_string())];
        assert!(merge_source(&mut d, &said, "m365", "2026-10-30T00:00:00Z"));
        assert_eq!((d.fields["title"].as_str(), d.fields["department"].as_str(), d.sources["department"].as_str()), ("CIO", "IT", "m365"));
        assert_eq!(d.questions[0]["theirs"], "Head of IT");
        assert!(!merge_source(&mut d, &said, "m365", "2026-10-30T00:00:00Z"), "asked once");
        projects::with_lock(|| { let mut all = directory()?; *all.iter_mut().find(|p| p.id == k).unwrap() = d.clone(); projects::write_json(&file(), &all) }).unwrap();
        answer(&k, "title", false).unwrap();
        let mut d = directory().unwrap().into_iter().find(|p| p.id == k).unwrap();
        assert!(d.questions.is_empty());
        assert!(!merge_source(&mut d, &said, "m365", "2026-10-31T00:00:00Z"), "kept: not asked again for the same value");
        assert!(merge_source(&mut d, &[("title", "VP IT".to_string())], "m365", "2026-11-30T00:00:00Z"), "a new value asks again");
        let v = of_thread(&p.id, chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()).unwrap();
        assert_eq!(v["people"][0]["title"], "CIO");
        assert_eq!(v["people"][0]["places"][0]["role"], "sponsor");
        assert_eq!(v["people"][0]["in_thread"], true);
    }
}

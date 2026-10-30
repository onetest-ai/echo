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
pub const FIELDS: [&str; 11] = ["name", "title", "org", "group", "reports_to", "matters", "notes", "phone", "department", "company", "office"];
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

/// A person's key: their email, else their name.
pub fn key_of(name: &str, email: Option<&str>) -> String {
    match email.map(|e| e.trim().to_lowercase()).filter(|e| e.contains('@')) {
        Some(e) => e,
        None => format!("n:{}", norm(name)),
    }
}

/// A display name as people read it: "(Partner)", "(external)", "[EXT]" and the like dropped, and
/// a directory's "Last, First" turned round. "Ortiz, Karen (Partner)" → "Karen Ortiz".
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

/// Words that tag an address or a name, not a person ("partner.karen.ortiz@…").
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
    n.is_empty() || n == "remote" || n == "me" || n == "unknown" || (n.starts_with("speaker ") && n[8..].chars().all(|c| c.is_ascii_digit()))
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
            for c in chat {
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
    let clients = projects::clients()?;
    let dir_people = directory()?;
    let me = MeIs::now();
    let my_domain = crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| domain(m.email.as_deref()));
    let mut dirs: Vec<PathBuf> = vec![];
    for p in threads {
        for (d, _) in projects::calls_of(p)? {
            if !dirs.contains(&d) {
                dirs.push(d);
            }
        }
    }
    let (mut seen, total) = gather(&dirs, &me, &dir_people);
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
    let mut people: Vec<Value> = seen
        .into_iter()
        .map(|(key, s)| {
            let entry = dir_people.iter().find(|p| p.id == key || s.email.as_ref().is_some_and(|e| p.emails.contains(e)));
            let f = |k: &str| entry.and_then(|p| p.fields.get(k).cloned());
            let dom = domain(s.email.as_deref());
            let org = f("org").and_then(|o| clients.iter().find(|c| c.id == o)).or_else(|| org_of(&clients, dom.as_deref()));
            let side = match (org, &dom) {
                (Some(c), _) if c.internal => "yours",
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
            json!({"id": c.id, "name": c.name, "internal": c.internal, "parent": c.parent, "relations": c.relations, "groups": c.groups,
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

/// Place an organisation in a thread, as `place` does a person.
pub fn place_org(thread: &str, client: &str, v: &Value) -> Result<()> {
    if !projects::clients()?.iter().any(|c| c.id == client) {
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
        .filter(|x| x["calls"].as_u64().unwrap_or(0) > 0 || x["in_thread"] == true)
        .map(|x| {
            let pl = x["places"].get(0).cloned().unwrap_or(json!({}));
            json!({"name": x["name"], "email": x["email"], "organisation": x["org"]["name"].as_str().unwrap_or(match x["side"].as_str() { Some("yours") => "the user's company", _ => "" }),
                   "in_thread": x["in_thread"], "title": x["title"], "group": x["group"], "reports_to": x["reports_to"].as_str().and_then(name_of),
                   "role": pl["role"], "stance": pl["stance"], "influence": pl["influence"], "interest": pl["interest"], "calls": x["calls"]})
        })
        .collect();
    let orgs: Vec<Value> = v["orgs"].as_array().into_iter().flatten().map(|o| json!({"name": o["name"], "relations": o["relations"], "parent": o["parent"]})).collect();
    let known = projects::clients()?.into_iter().map(|c| c.name).collect::<Vec<_>>();
    let said: Vec<Value> = suggestions(project).into_iter().map(|x| json!({"who": x["who"], "field": x["field"], "value": x["value"], "state": x["state"]})).collect();
    Ok(json!({
        "project": project, "people": people, "organisations": orgs, "known_organisations": known,
        "items": items.iter().map(|i| json!({"id": i, "file": l.evidence_md(i).display().to_string().replace('\\', "/")})).collect::<Vec<_>>(),
        "already_proposed": said,
        "fields": SUGGEST_FIELDS, "stances": projects::STANCES, "relations": projects::RELATIONS,
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
    let clients = projects::clients()?;
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
            if !SUGGEST_FIELDS.contains(&field) {
                return Some(format!("field is one of {SUGGEST_FIELDS:?}"));
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
    Ok(json!({"added": added, "rejected": rejected}))
}

/// The user's answer to a suggestion: yes applies it as theirs (a placement, a directory field, a
/// relationship, or taking someone out of the thread); no keeps it turned down (not proposed again).
pub fn decide(project: &str, sid: &str, yes: bool) -> Result<()> {
    let mut all = suggestions(project);
    let x = all.iter_mut().find(|x| x["sid"] == sid).ok_or_else(|| anyhow::anyhow!("no such suggestion"))?;
    if x["state"] != "pending" {
        bail!("already decided");
    }
    if yes {
        let t = &x["target"];
        let field = x["field"].as_str().unwrap_or("");
        let value = x["value"].as_str().unwrap_or("");
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
    x["state"] = json!(if yes { "accepted" } else { "rejected" });
    x["decided"] = json!(chrono::Utc::now().to_rfc3339());
    projects::with_lock(|| projects::write_json(&suggestions_file(project), &all))
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
        decide(&p.id, all[0]["sid"].as_str().unwrap(), true).unwrap();
        decide(&p.id, all[1]["sid"].as_str().unwrap(), false).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        assert_eq!(ravi(&v)["places"][0]["stance"], "sceptic");
        assert!(ravi(&v)["reports_to"].is_null(), "turned down");
        assert_eq!(v["suggestions"].as_array().unwrap().len(), 0);
        assert_eq!(apply_suggestions(&p.id, ev, &[json!({"who": "Ravi", "field": "reports_to", "value": "Dana Kowal", "quote": "Ravi reports to me now"})]).unwrap()["added"], 0, "a no stays no");
    }

    #[test]
    fn names_as_directories_write_them() {
        assert_eq!(display("Ortiz, Karen (Partner)"), "Karen Ortiz");
        assert_eq!(display("Pat Novak (external)"), "Pat Novak");
        assert_eq!(display("[EXT] Dana Kowal"), "Dana Kowal");
        assert_eq!(display("Smith, Jr."), "Jr. Smith");
        assert_eq!(norm("Inés Peña Gómez"), "ines pena gomez");
        assert_eq!(tokens("partner.karen.ortiz"), ["ortiz", "karen"].map(String::from).into_iter().collect());
    }

    #[test]
    fn one_person_with_addresses_at_two_organisations() {
        let (_env, root) = crate::paths::testenv::folders("people-addresses");
        let a = call(&root, "2026-10-01_100000-a", "2026-10-01T10:00:00+00:00",
            json!({"attendees": [{"name": "Ortiz, Karen (Partner)", "email": "partner.karen.ortiz@initrode.example"}]}), json!([]));
        let b = call(&root, "2026-10-02_100000-b", "2026-10-02T10:00:00+00:00",
            json!({"attendees": [{"name": "Karen F.", "email": "karen.ortiz@northwind.example"}]}), json!([]));
        let p = projects::create("Northwind", None, None).unwrap();
        projects::update(&p.id, |x| { x.pins = vec![calls::stamp_of(&a), calls::stamp_of(&b)]; Ok(()) }).unwrap();
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        let karen = v["people"].as_array().unwrap().iter().find(|x| x["email"] == "partner.karen.ortiz@initrode.example").unwrap().clone();
        assert_eq!(karen["name"], "Karen Ortiz", "the directory's \"Last, First (Partner)\" read as a name");
        assert_eq!(karen["maybe_same"][0]["email"], "karen.ortiz@northwind.example", "the same first and last name in the other address");
        // Merged by the user: one person, both addresses, every call.
        merge("karen.ortiz@northwind.example", "Karen F.", &[], "partner.karen.ortiz@initrode.example", "Karen Ortiz", Some("partner.karen.ortiz@initrode.example")).unwrap();
        let v = of_thread(&p.id, today).unwrap();
        let ks: Vec<&Value> = v["people"].as_array().unwrap().iter().filter(|x| x["name"] == "Karen Ortiz").collect();
        assert_eq!(ks.len(), 1);
        assert_eq!((ks[0]["calls"].as_u64(), ks[0]["addresses"].as_array().unwrap().len()), (Some(2), 2));
        assert_eq!(ks[0]["maybe_same"].as_array().unwrap().len(), 0);
        // Split again: two people, never suggested as one.
        split("partner.karen.ortiz@initrode.example", "Karen Ortiz", "karen.ortiz@northwind.example").unwrap();
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

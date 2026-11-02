//! Clients and projects (`docs/projects-design.md`): their files under `<home>/projects/`, the
//! stages, and which calls belong to a project — by its rules (a meeting series, a client's people,
//! the Teams chat a call came from, words of the subject), pins and exclusions. All of it is
//! deterministic: a suggestion names a project for a call, only the user puts a call in one. Calls
//! are never written here: membership is computed from the rules each time.
//!
//! ```text
//! <home>/projects/clients.json          [{id, name, aliases, domains, internal}]
//! <home>/projects/<id>/profile.json     the project (below)
//! <home>/projects/<id>/brief.md         the user's brief
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::calls;
use crate::meeting::{MeetingContext, Person};
use crate::paths;
use crate::vocab;

const PROFILE: &str = "profile.json";
const BRIEF: &str = "brief.md";
const CLIENTS: &str = "clients.json";
/// Colours a new project gets, in turn (the window's swatches).
const COLORS: [&str; 8] = ["#9b78ff", "#5aa9ff", "#3ccf91", "#f0a24a", "#e8603c", "#d36bd1", "#4cc3c8", "#c9b458"];
/// A suggestion needs at least this share of a call's other people among a project's people.
const SUGGEST_SHARE: f64 = 0.5;

/// `<home>/projects`.
pub fn dir() -> PathBuf {
    paths::home().join("projects")
}

/// Where a project stands. The group (`pursuit`, `delivery`, `closed`) decides what Echo does with
/// it; the stage is for the user and for the sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Lead,
    Proposal,
    Negotiation,
    Kickoff,
    Active,
    OnHold,
    WrapUp,
    Done,
    Lost,
    Cancelled,
}

impl Stage {
    pub const ALL: [Stage; 10] = [
        Stage::Lead,
        Stage::Proposal,
        Stage::Negotiation,
        Stage::Kickoff,
        Stage::Active,
        Stage::OnHold,
        Stage::WrapUp,
        Stage::Done,
        Stage::Lost,
        Stage::Cancelled,
    ];

    pub fn group(self) -> &'static str {
        match self {
            Stage::Lead | Stage::Proposal | Stage::Negotiation => "pursuit",
            Stage::Kickoff | Stage::Active | Stage::OnHold | Stage::WrapUp => "delivery",
            Stage::Done | Stage::Lost | Stage::Cancelled => "closed",
        }
    }

    pub fn closed(self) -> bool {
        self.group() == "closed"
    }

    pub fn id(self) -> &'static str {
        match self {
            Stage::Lead => "lead",
            Stage::Proposal => "proposal",
            Stage::Negotiation => "negotiation",
            Stage::Kickoff => "kickoff",
            Stage::Active => "active",
            Stage::OnHold => "on_hold",
            Stage::WrapUp => "wrap_up",
            Stage::Done => "done",
            Stage::Lost => "lost",
            Stage::Cancelled => "cancelled",
        }
    }

    /// "on hold", "On-Hold", "on_hold" → OnHold.
    pub fn parse(s: &str) -> Result<Stage> {
        let k: String = s.trim().to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        Stage::ALL.into_iter().find(|st| st.id() == k).ok_or_else(|| {
            anyhow!("unknown stage {s:?} (one of: {})", Stage::ALL.map(|s| s.id()).join(", "))
        })
    }
}

/// An organisation the user works for or with. Its domains say whose mail and calls are its.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    /// The user's own company (its people are colleagues).
    #[serde(default)]
    pub internal: bool,
    /// The organisation it belongs to (a client id): Northwind Retail → Northwind Group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Its relationships to other organisations (partner, vendor, delivers for…).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relations: Vec<Relation>,
    /// Its groups (departments, teams), as people are placed in them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
}

/// Which calls belong to a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Rule {
    /// Every occurrence of a recurring meeting: its calendar series when the call knows it, else
    /// its subject ("Weekly sync #12" ~ "weekly sync").
    Series {
        subject: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        series_id: Option<String>,
    },
    /// At least half of a call's other people come from these domains (none: the client's).
    People {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        domains: Vec<String>,
    },
    /// The call's chat is this Teams chat (a meeting's chat, or the chat a call was started from).
    Chat {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    /// The subject contains these words (case and punctuation aside).
    Subject { contains: String },
}

impl Rule {
    pub fn kind(&self) -> &'static str {
        match self {
            Rule::Series { .. } => "series",
            Rule::People { .. } => "people",
            Rule::Chat { .. } => "chat",
            Rule::Subject { .. } => "subject",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageChange {
    pub stage: Stage,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Something a project watches besides its calls: a Teams chat (its calls join), a Teams channel,
/// mail, a folder (the Brain reads them: `brain::sources`, `brain::files`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// `teams_chat`, `teams_channel`, `mail`, `folder`.
    pub kind: String,
    /// The chat or channel id, the mail rule, the folder's absolute path.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A folder's globs of what not to read (`Archive/`, `*draft*`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Their role in this project ("sponsor", "approver").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(flatten)]
    pub place: Place,
}

/// Where a stakeholder (a person or an organisation) stands in a project: stance (champion,
/// supporter, neutral, sceptic, blocker), influence and interest (1 low … 3 high).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Place {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub influence: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interest: Option<u8>,
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub const STANCES: [&str; 5] = ["champion", "supporter", "neutral", "sceptic", "blocker"];

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
impl Place {
    pub fn check(&self) -> Result<()> {
        if let Some(s) = &self.stance {
            if !STANCES.contains(&s.as_str()) {
                bail!("a stance is one of {STANCES:?}");
            }
        }
        for v in [self.influence, self.interest].into_iter().flatten() {
            if !(1..=3).contains(&v) {
                bail!("influence and interest are 1 (low) to 3 (high)");
            }
        }
        Ok(())
    }
}

/// An organisation (a client) as a stakeholder of a project.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrgPlace {
    pub client: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(flatten)]
    pub place: Place,
}

/// How two organisations relate: `kind` from `this` organisation to `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub to: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub const RELATIONS: [&str; 6] = ["partner", "vendor", "delivers_for", "integrates_for", "customer", "competitor"];

/// A glossary entry: hints transcription on the project's calls.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Term {
    pub term: String,
    /// system, term, acronym, milestone, deliverable, person…
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A board column: its name and colour, and the state category its cards are in (`open`,
/// `in_progress`, `waiting` — items waiting on someone —, `blocked`, `done`), so the sync's states
/// still place every card whatever the columns are called. `limit`: how many cards it should hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub id: String,
    pub name: String,
    pub color: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    #[serde(default)]
    pub columns: Vec<Column>,
    /// What a card shows: story, kind, owner, due, waiting, holds_up, source.
    #[serde(default)]
    pub fields: Vec<String>,
    /// none | story | owner
    #[serde(default)]
    pub group: String,
    /// Saved views of the Work tab (tabs): each its layout, filters, grouping, card fields and
    /// hidden columns. Kept as the page writes them (free-form; the page reads what it knows).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<Value>,
}

pub const COLUMN_STATES: [&str; 5] = ["open", "in_progress", "waiting", "blocked", "done"];

fn check_board(b: &Board) -> Result<()> {
    if b.columns.is_empty() {
        bail!("a board needs a column");
    }
    let mut ids = BTreeSet::new();
    for c in &b.columns {
        if c.name.trim().is_empty() || !ids.insert(c.id.clone()) || !COLUMN_STATES.contains(&c.state.as_str()) {
            bail!("each column needs a name, its own id and one of {COLUMN_STATES:?}");
        }
    }
    if b.views.len() > 30 || b.views.iter().any(|v| !v.is_object() || v.to_string().len() > 4096) {
        bail!("views are objects (at most 30)");
    }
    for s in COLUMN_STATES {
        if !b.columns.iter().any(|c| c.state == s && !c.hidden) && s != "waiting" {
            bail!("keep a visible column for {s}: cards in it would have nowhere to go");
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub color: String,
    /// How it is drawn: none = an identicon from its id; a symbol's name (the window's set); a logo
    /// image in its folder wins over both (`logo`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub stage: Stage,
    /// Every stage it was in, oldest first (the first is where it started).
    #[serde(default)]
    pub stages: Vec<StageChange>,
    /// `YYYY-MM-DD`: calls before it don't belong.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// `YYYY-MM-DD`: its documents, posts and mail dated before it are history in the Brain (kept, not
    /// searched unless history is asked for; Brain's `supersede_before`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_before: Option<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Call stamps put in by hand, whatever the rules say.
    #[serde(default)]
    pub pins: Vec<String>,
    /// Call stamps taken out by hand ("Not this" on a suggestion too).
    #[serde(default)]
    pub exclusions: Vec<String>,
    /// Calendar events put in by hand (their ids): the call recorded from one belongs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default)]
    pub people: Vec<Member>,
    /// Organisations as its stakeholders (the client, its parent, partners).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub orgs: Vec<OrgPlace>,
    #[serde(default)]
    pub terms: Vec<Term>,
    /// What Claude should know about it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// The Work board, as this project set it (none: the default columns, fields, no grouping).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<Board>,
    /// The AI profile and model of its jobs (none: the default profile; its minutes model).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub created_at: String,
}

impl Project {
    /// When it closed (the last change into a closed stage), if it is closed.
    fn closed_at(&self) -> Option<DateTime<FixedOffset>> {
        if !self.stage.closed() {
            return None;
        }
        self.stages.iter().rev().find(|c| c.stage.closed()).and_then(|c| DateTime::parse_from_rfc3339(&c.at).ok())
    }
}

/// What membership looks at in a call.
#[derive(Debug, Clone, Default)]
pub struct CallFacts {
    pub stamp: String,
    pub started: Option<DateTime<FixedOffset>>,
    pub subject: Option<String>,
    pub series_id: Option<String>,
    /// The other people (email, else name; lowercase), not the user.
    pub others: BTreeSet<String>,
    /// The user's own mail domain: their colleagues aren't "outside people".
    pub own_domain: Option<String>,
    pub chat_id: Option<String>,
    /// The calendar event it was recorded from (or is: an upcoming one).
    pub event_id: Option<String>,
}

impl CallFacts {
    pub fn of(dir: &Path, me: Option<&Person>) -> CallFacts {
        let meta = calls::read_json(&dir.join("meta.json"));
        let meeting = MeetingContext::load(dir).ok().flatten();
        let mut f = meeting.as_ref().map(|m| CallFacts::of_meeting(m, me)).unwrap_or_default();
        f.stamp = calls::stamp_of(dir);
        f.started = meta["started_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).or(f.started);
        if f.subject.is_none() {
            f.subject = calls::previous_subject(dir);
        }
        f.chat_id = fs::read_to_string(dir.join("chat.id")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).or(f.chat_id);
        f
    }

    /// From a meeting alone (a recording that is starting: no folder facts yet).
    pub fn of_meeting(m: &MeetingContext, me: Option<&Person>) -> CallFacts {
        CallFacts {
            stamp: String::new(),
            started: m.start.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
            subject: m.subject.clone().filter(|s| !s.trim().is_empty()),
            series_id: m.series_id.clone(),
            others: vocab::others(m, me),
            own_domain: me.and_then(|p| p.email.as_deref()).and_then(domain_of).map(str::to_lowercase),
            chat_id: m.join_url.as_deref().and_then(crate::graph::chat_id_from_join_url),
            event_id: m.id.clone().filter(|i| !i.is_empty()),
        }
    }
}

fn domain_of(key: &str) -> Option<&str> {
    key.rsplit_once('@').map(|(_, d)| d)
}

/// The outside people of a call (their domains): not the user's colleagues (their own domain, or a
/// client marked as their own company), and only people known by their email.
fn outside<'a>(f: &'a CallFacts, clients: &[Client]) -> Vec<&'a str> {
    let internal: Vec<String> = clients.iter().filter(|c| c.internal).flat_map(|c| norm_domains(&c.domains)).collect();
    f.others
        .iter()
        .filter_map(|k| domain_of(k))
        .filter(|d| f.own_domain.as_deref() != Some(*d) && !internal.iter().any(|x| x == d))
        .collect()
}

fn norm_domains(ds: &[String]) -> Vec<String> {
    ds.iter().map(|d| d.trim().trim_start_matches('@').to_lowercase()).filter(|d| !d.is_empty()).collect()
}

/// Words of a subject, for "contains" (lowercase, letters and digits).
fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

fn contains_words(subject: &str, needle: &str) -> bool {
    let (s, n) = (words(subject), words(needle));
    !n.is_empty() && s.windows(n.len()).any(|w| w == n.as_slice())
}

/// Does a call belong to the project, and why: "pinned", or the kind of the first rule it meets.
pub fn matches(p: &Project, clients: &[Client], f: &CallFacts) -> Option<&'static str> {
    if !f.stamp.is_empty() && p.exclusions.contains(&f.stamp) {
        return None;
    }
    if !f.stamp.is_empty() && p.pins.contains(&f.stamp) {
        return Some("pinned");
    }
    if f.event_id.as_ref().is_some_and(|e| p.events.contains(e)) {
        return Some("pinned");
    }
    if let (Some(since), Some(started)) = (p.since.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()), f.started) {
        if started.date_naive() < since {
            return None;
        }
    }
    // A closed project keeps the calls it had; new ones don't join it.
    if let (Some(closed), Some(started)) = (p.closed_at(), f.started) {
        if started > closed {
            return None;
        }
    }
    let client_domains = p.client.as_deref().and_then(|c| clients.iter().find(|x| x.id == c)).map(|c| norm_domains(&c.domains)).unwrap_or_default();
    for r in &p.rules {
        let hit = match r {
            Rule::Series { subject, series_id } => {
                (series_id.is_some() && series_id == &f.series_id)
                    || f.subject.as_deref().is_some_and(|s| {
                        let s = vocab::norm_subject(s);
                        !s.is_empty() && s == vocab::norm_subject(subject)
                    })
            }
            // At least half of the call's outside people (colleagues don't count) are theirs.
            Rule::People { domains } => {
                let ds = if domains.is_empty() { client_domains.clone() } else { norm_domains(domains) };
                let out = outside(f, clients);
                let theirs = out.iter().filter(|d| ds.iter().any(|x| x == *d)).count();
                !ds.is_empty() && theirs > 0 && theirs * 2 >= out.len()
            }
            Rule::Chat { id, .. } => f.chat_id.as_deref() == Some(id.as_str()),
            Rule::Subject { contains } => f.subject.as_deref().is_some_and(|s| contains_words(s, contains)),
        };
        if hit {
            return Some(r.kind());
        }
    }
    None
}

// ── files ──────────────────────────────────────────────────────────────────────────────────────

/// Every write to the projects' files takes this (waits up to 10 s).
fn lock() -> Result<crate::app::FileLock> {
    fs::create_dir_all(dir())?;
    let path = dir().join(".lock");
    let t = std::time::Instant::now();
    loop {
        if let Some(l) = crate::app::FileLock::try_take(&path).with_context(|| format!("opening {}", path.display()))? {
            return Ok(l);
        }
        if t.elapsed() > std::time::Duration::from_secs(10) {
            bail!("the projects are being changed by another process");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Run `f` under the projects' lock (another file of theirs, as `people.json`).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(crate) fn with_lock<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    let _l = lock()?;
    f()
}

/// A client's logo version for the page (0: none).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub(crate) fn client_logo_v(id: &str) -> u64 {
    logo_v(client_logo_stem(id))
}

pub(crate) fn write_json<T: Serialize>(path: &Path, v: &T) -> Result<()> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d)?;
    }
    paths::write_atomic(path, &serde_json::to_vec_pretty(v)?).with_context(|| format!("writing {}", path.display()))
}

pub fn clients() -> Result<Vec<Client>> {
    let p = dir().join(CLIENTS);
    match fs::read(&p) {
        Ok(b) => serde_json::from_slice(&b).with_context(|| format!("reading {}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e).with_context(|| format!("reading {}", p.display())),
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 60 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') && !id.starts_with('-')
}

fn profile_path(id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        bail!("{id:?} is not a project id");
    }
    Ok(dir().join(id).join(PROFILE))
}

pub fn load(id: &str) -> Result<Project> {
    let p = profile_path(id)?;
    let b = fs::read(&p).map_err(|e| if e.kind() == std::io::ErrorKind::NotFound { anyhow!("no project {id:?}") } else { anyhow!("reading {}: {e}", p.display()) })?;
    let pr: Project = serde_json::from_slice(&b).with_context(|| format!("reading {}", p.display()))?;
    if pr.id != id {
        bail!("{} says it is {:?}", p.display(), pr.id);
    }
    Ok(pr)
}

/// Every project (unreadable ones left out, with a note on stderr), by client then name.
pub fn list() -> Vec<Project> {
    let mut out: Vec<Project> = fs::read_dir(dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().join(PROFILE).exists())
                .filter_map(|e| {
                    let id = e.file_name().to_string_lossy().into_owned();
                    load(&id).map_err(|err| eprintln!("echoctl: project {id}: {err:#}")).ok()
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| (a.client.as_deref().unwrap_or("~"), a.name.to_lowercase()).cmp(&(b.client.as_deref().unwrap_or("~"), b.name.to_lowercase())));
    out
}

pub fn brief(id: &str) -> Result<String> {
    let p = dir().join(id).join(BRIEF);
    profile_path(id)?;
    Ok(fs::read_to_string(p).unwrap_or_default())
}

pub fn set_brief(id: &str, text: &str) -> Result<()> {
    let _l = lock()?;
    load(id)?;
    paths::write_atomic(&dir().join(id).join(BRIEF), text.as_bytes())?;
    Ok(())
}

/// A client's description (the user's, Markdown): `clients/<id>.md`.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn client_brief(id: &str) -> Result<String> {
    let f = client_logo_stem(id)?.with_extension("md");
    Ok(fs::read_to_string(f).unwrap_or_default())
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn set_client_brief(id: &str, text: &str) -> Result<()> {
    let _l = lock()?;
    if !clients()?.iter().any(|c| c.id == id) {
        bail!("no client {id:?}");
    }
    let f = client_logo_stem(id)?.with_extension("md");
    fs::create_dir_all(f.parent().unwrap_or(Path::new(".")))?;
    paths::write_atomic(&f, text.as_bytes())?;
    Ok(())
}

fn check(p: &Project, clients: &[Client]) -> Result<()> {
    if p.name.trim().is_empty() {
        bail!("a project needs a name");
    }
    if let Some(c) = &p.client {
        if !clients.iter().any(|x| &x.id == c) {
            bail!("no client {c:?}");
        }
    }
    if let Some(b) = &p.board {
        check_board(b)?;
    }
    if let Some(s) = &p.history_before {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").with_context(|| format!("history before {s:?}: expected YYYY-MM-DD"))?;
    }
    if let Some(s) = &p.since {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").with_context(|| format!("since {s:?}: expected YYYY-MM-DD"))?;
    }
    for r in &p.rules {
        let empty = match r {
            Rule::Series { subject, .. } => vocab::norm_subject(subject).is_empty(),
            Rule::Chat { id, .. } => id.trim().is_empty(),
            Rule::Subject { contains } => words(contains).is_empty(),
            Rule::People { .. } => false,
        };
        if empty {
            bail!("a {} rule needs a value", r.kind());
        }
    }
    Ok(())
}

/// A new project (stage Active, unless given). Its id comes from its name, made unique.
pub fn create(name: &str, client: Option<&str>, stage: Option<Stage>) -> Result<Project> {
    let _l = lock()?;
    let base = crate::recorder::slugify(name);
    let base = if base.is_empty() { "project".to_string() } else { base };
    let id = std::iter::once(base.clone())
        .chain((2..).map(|n| format!("{base}-{n}")))
        .find(|id| !dir().join(id).exists())
        .unwrap_or(base);
    let now = Utc::now().to_rfc3339();
    let stage = stage.unwrap_or(Stage::Active);
    let n = fs::read_dir(dir()).map(|rd| rd.flatten().filter(|e| e.path().join(PROFILE).exists()).count()).unwrap_or(0);
    let p = Project {
        id: id.clone(),
        name: name.trim().to_string(),
        client: client.map(str::to_owned),
        color: COLORS[n % COLORS.len()].to_string(),
        icon: None,
        stage,
        stages: vec![StageChange { stage, at: now.clone(), reason: None }],
        since: None,
        history_before: None,
        rules: vec![],
        pins: vec![],
        exclusions: vec![],
        events: vec![],
        board: None,
        sources: vec![],
        people: vec![],
        orgs: vec![],
        terms: vec![],
        notes: String::new(),
        ai_profile: None,
        model: None,
        created_at: now,
    };
    check(&p, &clients()?)?;
    write_json(&dir().join(&id).join(PROFILE), &p)?;
    Ok(p)
}

/// Change a project under the lock, read inside it; checked before it is written.
pub fn update(id: &str, f: impl FnOnce(&mut Project) -> Result<()>) -> Result<Project> {
    let _l = lock()?;
    let mut p = load(id)?;
    let stage = p.stage;
    f(&mut p)?;
    p.id = id.to_string();
    if p.stage != stage {
        p.stages.push(StageChange { stage: p.stage, at: Utc::now().to_rfc3339(), reason: None });
    }
    check(&p, &clients()?)?;
    write_json(&profile_path(id)?, &p)?;
    Ok(p)
}

/// A logo: PNG, JPEG or WebP (checked by its first bytes), ≤ 512 KB — a project's as `logo.<ext>` in
/// its folder, a client's as `clients/<id>.<ext>`.
const LOGOS: [(&str, &str); 3] = [("png", "image/png"), ("jpg", "image/jpeg"), ("webp", "image/webp")];
const LOGO_MAX: usize = 512 * 1024;

fn logo_kind(b: &[u8]) -> Result<&'static str> {
    match b {
        _ if b.len() > LOGO_MAX => bail!("a logo is at most {} KB", LOGO_MAX / 1024),
        [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, ..] => Ok("png"),
        [0xFF, 0xD8, 0xFF, ..] => Ok("jpg"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Ok("webp"),
        _ => bail!("a logo is a PNG, JPEG or WebP image"),
    }
}

/// The logo file at `stem.<ext>`, its type, and its change time (for the page's cache).
fn logo_at(stem: &Path) -> Option<(PathBuf, &'static str, u64)> {
    LOGOS.iter().find_map(|(ext, mime)| {
        let p = stem.with_extension(ext);
        let t = p.metadata().ok().filter(|m| m.is_file())?.modified().ok()?;
        Some((p, *mime, t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)))
    })
}

fn write_logo(stem: &Path, bytes: Option<&[u8]>) -> Result<()> {
    let ext = bytes.map(logo_kind).transpose()?;
    if let Some(d) = stem.parent() {
        fs::create_dir_all(d)?;
    }
    for (e, _) in LOGOS {
        let _ = fs::remove_file(stem.with_extension(e));
    }
    if let (Some(ext), Some(b)) = (ext, bytes) {
        paths::write_atomic(&stem.with_extension(ext), b)?;
    }
    Ok(())
}

fn project_logo_stem(id: &str) -> Result<PathBuf> {
    profile_path(id)?;
    Ok(dir().join(id).join("logo"))
}

fn client_logo_stem(id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        bail!("{id:?} is not a client id");
    }
    Ok(dir().join("clients").join(id))
}

/// The project's logo file and its type, if it has one.
pub fn logo(id: &str) -> Option<(PathBuf, &'static str)> {
    logo_at(&project_logo_stem(id).ok()?).map(|(p, m, _)| (p, m))
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn client_logo(id: &str) -> Option<(PathBuf, &'static str)> {
    logo_at(&client_logo_stem(id).ok()?).map(|(p, m, _)| (p, m))
}

/// The logo's version for the page (0: none).
fn logo_v(stem: Result<PathBuf>) -> u64 {
    stem.ok().and_then(|s| logo_at(&s)).map(|(_, _, t)| t.max(1)).unwrap_or(0)
}

/// Give a project a logo (image bytes), or take it away (None).
pub fn set_logo_bytes(id: &str, bytes: Option<&[u8]>) -> Result<()> {
    let _l = lock()?;
    load(id)?;
    write_logo(&project_logo_stem(id)?, bytes)
}

/// Give a project a logo from an image file, or take it away (None).
pub fn set_logo(id: &str, file: Option<&Path>) -> Result<()> {
    let bytes = file.map(|f| fs::read(f).with_context(|| format!("reading {}", f.display()))).transpose()?;
    set_logo_bytes(id, bytes.as_deref())
}

/// Give a client a logo (image bytes), or take it away (None).
pub fn set_client_logo(id: &str, bytes: Option<&[u8]>) -> Result<()> {
    let _l = lock()?;
    if !clients()?.iter().any(|c| c.id == id) {
        bail!("no client {id:?}");
    }
    write_logo(&client_logo_stem(id)?, bytes)
}

/// Move a project to a stage (a dated event, with why).
pub fn set_stage(id: &str, stage: Stage, reason: Option<&str>) -> Result<Project> {
    let _l = lock()?;
    let mut p = load(id)?;
    if p.stage != stage {
        p.stage = stage;
        p.stages.push(StageChange { stage, at: Utc::now().to_rfc3339(), reason: reason.map(str::to_owned).filter(|r| !r.trim().is_empty()) });
        write_json(&profile_path(id)?, &p)?;
    }
    Ok(p)
}

/// Remove a project's folder (its brief and profile; later its ledger) into the trash.
pub fn delete(id: &str) -> Result<()> {
    let _l = lock()?;
    load(id)?;
    let trash = paths::state_dir().join("trash");
    fs::create_dir_all(&trash)?;
    let to = trash.join(format!("project-{id}-{}", Utc::now().format("%Y%m%d%H%M%S")));
    fs::rename(dir().join(id), &to).with_context(|| format!("moving the project to {}", to.display()))?;
    Ok(())
}

pub fn add_client(name: &str, domains: &[String], internal: bool) -> Result<Client> {
    let _l = lock()?;
    let mut all = clients()?;
    if name.trim().is_empty() {
        bail!("a client needs a name");
    }
    let base = crate::recorder::slugify(name);
    let base = if base.is_empty() { "client".to_string() } else { base };
    let id = std::iter::once(base.clone()).chain((2..).map(|n| format!("{base}-{n}"))).find(|id| !all.iter().any(|c| &c.id == id)).unwrap_or(base);
    let c = Client { id, name: name.trim().to_string(), domains: norm_domains(domains), internal, ..Default::default() };
    all.push(c.clone());
    write_json(&dir().join(CLIENTS), &all)?;
    Ok(c)
}

pub fn update_client(id: &str, f: impl FnOnce(&mut Client) -> Result<()>) -> Result<Client> {
    let _l = lock()?;
    let mut all = clients()?;
    let c = all.iter_mut().find(|c| c.id == id).ok_or_else(|| anyhow!("no client {id:?}"))?;
    f(c)?;
    c.id = id.to_string();
    c.domains = norm_domains(&c.domains);
    if c.name.trim().is_empty() {
        bail!("a client needs a name");
    }
    let c = c.clone();
    write_json(&dir().join(CLIENTS), &all)?;
    Ok(c)
}

/// A client without projects can go.
pub fn remove_client(id: &str) -> Result<()> {
    let _l = lock()?;
    let mut all = clients()?;
    if list().iter().any(|p| p.client.as_deref() == Some(id)) {
        bail!("{id} still has projects: move or delete them first");
    }
    let n = all.len();
    all.retain(|c| c.id != id);
    if all.len() == n {
        bail!("no client {id:?}");
    }
    write_logo(&client_logo_stem(id)?, None)?;
    write_json(&dir().join(CLIENTS), &all)
}

// ── calls ──────────────────────────────────────────────────────────────────────────────────────

fn me() -> Option<Person> {
    crate::config::Config::load().ok().and_then(|c| c.me)
}

/// The project's calls, oldest first, each with why it belongs.
pub fn calls_of(p: &Project) -> Result<Vec<(PathBuf, &'static str)>> {
    let clients = clients()?;
    let me = me();
    Ok(calls::all_calls().into_iter().filter_map(|d| matches(p, &clients, &CallFacts::of(&d, me.as_ref())).map(|w| (d, w))).collect())
}

/// The projects a call belongs to, with why.
pub fn of_call(dir: &Path) -> Result<Vec<(Project, &'static str)>> {
    let clients = clients()?;
    let f = CallFacts::of(dir, me().as_ref());
    Ok(list().into_iter().filter_map(|p| matches(&p, &clients, &f).map(|w| (p, w))).collect())
}

/// The projects a meeting that is starting belongs to by their rules (no pins: the call has no
/// stamp yet) — for its vocabulary and live notes.
pub fn of_meeting(m: &MeetingContext) -> Vec<Project> {
    let Ok(clients) = clients() else { return vec![] };
    let f = CallFacts::of_meeting(m, me().as_ref());
    list().into_iter().filter(|p| matches(p, &clients, &f).is_some()).collect()
}

/// The projects a calendar event belongs to (its rules, or added by hand), as chips — `pinned`:
/// added by hand to this very event.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn for_event(m: &MeetingContext) -> Vec<Value> {
    let Ok(clients) = clients() else { return vec![] };
    let f = CallFacts::of_meeting(m, me().as_ref());
    list().iter().filter(|p| !p.stage.closed()).filter_map(|p| matches(p, &clients, &f).map(|w| chip(p, w))).collect()
}

/// Put a calendar event in a project by hand, or take it out ("include" | "remove").
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn set_event(id: &str, event: &str, how: &str) -> Result<Project> {
    if event.trim().is_empty() {
        bail!("no event named");
    }
    update(id, |p| {
        p.events.retain(|e| e != event);
        match how {
            "include" => p.events.push(event.to_string()),
            "remove" => {}
            _ => bail!("include or remove"),
        }
        Ok(())
    })
}

/// Put a call in a project by hand, take it out, or leave it to the rules again.
pub fn set_membership(id: &str, call: &Path, how: &str) -> Result<Project> {
    let stamp = calls::stamp_of(call);
    update(id, |p| {
        p.pins.retain(|s| s != &stamp);
        p.exclusions.retain(|s| s != &stamp);
        match how {
            "include" => p.pins.push(stamp.clone()),
            "exclude" => p.exclusions.push(stamp.clone()),
            "rules" => {}
            _ => bail!("include, exclude or rules"),
        }
        Ok(())
    })
}

/// The people a project meets: its members and the other people of its calls (keys, lowercase).
fn people_of(p: &Project, calls: &[(PathBuf, &'static str)], me: Option<&Person>) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = p.people.iter().map(|m| m.email.as_deref().unwrap_or(&m.name).trim().to_lowercase()).collect();
    for (d, _) in calls {
        out.extend(CallFacts::of(d, me).others);
    }
    out
}

/// Projects a call isn't in but may belong to (never for a call excluded from one): half of its
/// other people are people the project meets, or its subject names the project or one of its
/// terms. `why` says which, in words.
pub fn suggest(call: &Path) -> Result<Vec<Value>> {
    let clients = clients()?;
    let me = me();
    let f = CallFacts::of(call, me.as_ref());
    let mut out = vec![];
    for p in list().into_iter().filter(|p| !p.stage.closed()) {
        if matches(&p, &clients, &f).is_some() || p.exclusions.contains(&f.stamp) {
            continue;
        }
        let pc = calls_of(&p)?;
        let known = people_of(&p, &pc, me.as_ref());
        let shared = f.others.iter().filter(|k| known.contains(*k)).count();
        let subject = f.subject.clone().unwrap_or_default();
        let named = std::iter::once(p.name.as_str()).chain(p.terms.iter().map(|t| t.term.as_str())).find(|t| contains_words(&subject, t));
        let why = if !f.others.is_empty() && shared as f64 >= SUGGEST_SHARE * f.others.len() as f64 {
            format!("{shared} of its {} people are in its calls", f.others.len())
        } else if let Some(t) = named {
            format!("its subject names “{t}”")
        } else {
            continue;
        };
        let mut c = chip(&p, "suggested");
        c["why"] = json!(why);
        out.push(c);
    }
    Ok(out)
}

// ── found for you ──────────────────────────────────────────────────────────────────────────────

/// How far back Found for you looks.
const FOUND_DAYS: i64 = 90;
/// Calls a candidate needs.
const FOUND_MIN_CALLS: usize = 2;
const FOUND: &str = "found.json";
/// Mail domains that say nothing about an organisation.
const PERSONAL_DOMAINS: [&str; 8] = ["gmail.com", "googlemail.com", "outlook.com", "hotmail.com", "live.com", "yahoo.com", "icloud.com", "me.com"];

#[derive(Debug, Default, Serialize, Deserialize)]
struct FoundState {
    /// Candidates the user rejected (their keys): never suggested again.
    #[serde(default)]
    rejected: Vec<String>,
}

fn found_state() -> FoundState {
    fs::read(dir().join(FOUND)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// "globex.example" → "Globex", "northwind-group.example" → "Northwind Group": a first guess at a client's name.
fn name_from_domain(d: &str) -> String {
    let label = d.split('.').next().unwrap_or(d);
    label.split(['-', '_']).filter(|w| !w.is_empty()).map(|w| {
        let mut c = w.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
    }).collect::<Vec<_>>().join(" ")
}

/// The organisation a call is with: the domain at least half of its outside people come from
/// (colleagues and personal mail aside), if any.
fn main_domain(f: &CallFacts, skip: &BTreeSet<String>) -> Option<String> {
    let mut n: BTreeMap<&str, usize> = BTreeMap::new();
    let mut total = 0;
    for d in f.others.iter().filter_map(|k| domain_of(k)).filter(|d| !skip.contains(*d) && f.own_domain.as_deref() != Some(*d)) {
        *n.entry(d).or_default() += 1;
        total += 1;
    }
    let (d, c) = n.into_iter().max_by_key(|(_, c)| *c)?;
    (c * 2 >= total).then(|| d.to_string())
}

/// Projects Echo would make from the calls of the last 90 days that are in none: a recurring meeting,
/// one outside organisation's calls, one Teams chat's calls — each with the rules it would get, its
/// calls and its client (one you have, or a new one for an unknown domain). Deterministic; rejected
/// ones never come back. Newest first.
pub fn found() -> Result<Vec<Value>> {
    let clients = clients()?;
    let projects = list();
    let me = me();
    let rejected = found_state().rejected;
    let since = Utc::now() - chrono::Duration::days(FOUND_DAYS);
    // Domains that don't make a client: the user's own, internal clients', personal mail.
    let mut skip: BTreeSet<String> = PERSONAL_DOMAINS.iter().map(|d| d.to_string()).collect();
    skip.extend(me.as_ref().and_then(|m| m.email.as_deref()).and_then(domain_of).map(str::to_lowercase));
    skip.extend(clients.iter().filter(|c| c.internal).flat_map(|c| norm_domains(&c.domains)));
    let free: Vec<(PathBuf, CallFacts)> = calls::all_calls()
        .into_iter()
        .map(|d| {
            let f = CallFacts::of(&d, me.as_ref());
            (d, f)
        })
        .filter(|(_, f)| f.started.is_none_or(|t| t.with_timezone(&Utc) >= since))
        .filter(|(_, f)| !projects.iter().any(|p| matches(p, &clients, f).is_some()))
        .collect();
    let client_of = |d: &str| clients.iter().find(|c| norm_domains(&c.domains).iter().any(|x| x == d));
    let mut out: Vec<Value> = vec![];
    let mut taken: BTreeSet<String> = BTreeSet::new(); // calls already in a candidate
    let mut push = |key: String, kind: &str, name: String, rules: Vec<Rule>, members: Vec<&(PathBuf, CallFacts)>, why: String, taken: &mut BTreeSet<String>| {
        if members.len() < FOUND_MIN_CALLS || rejected.contains(&key) {
            return;
        }
        // The organisation most of its calls are with.
        let mut doms: BTreeMap<String, usize> = BTreeMap::new();
        for (_, f) in &members {
            if let Some(d) = main_domain(f, &skip) {
                *doms.entry(d).or_default() += 1;
            }
        }
        let dom = doms.into_iter().max_by_key(|(_, n)| *n).map(|(d, _)| d);
        let (client, new_client) = match dom.as_deref() {
            Some(d) => match client_of(d) {
                Some(c) => (Some(c.id.clone()), Value::Null),
                None => (None, json!({"name": name_from_domain(d), "domain": d})),
            },
            None => (None, Value::Null),
        };
        let latest = members.iter().filter_map(|(_, f)| f.started).max();
        for (d, _) in &members {
            taken.insert(calls::stamp_of(d));
        }
        out.push(json!({
            "key": key, "kind": kind, "name": name, "why": why, "rules": rules,
            "client": client, "new_client": new_client, "latest": latest.map(|t| t.to_rfc3339()),
            "calls": members.iter().rev().map(|(d, _)| call_row(d, kind)).collect::<Vec<_>>(),
        }));
    };
    // 1. Recurring meetings: the same calendar series, else the same subject.
    let mut series: BTreeMap<String, Vec<&(PathBuf, CallFacts)>> = BTreeMap::new();
    for x in &free {
        if let Some(s) = x.1.subject.as_deref().map(vocab::norm_subject).filter(|s| s.chars().count() >= 3) {
            series.entry(s).or_default().push(x);
        }
    }
    for (norm, members) in series {
        let subject = members.iter().rev().find_map(|(_, f)| f.subject.clone()).unwrap_or(norm.clone());
        let series_id = members.iter().rev().find_map(|(_, f)| f.series_id.clone());
        let n = members.len();
        push(format!("series:{norm}"), "series", subject.trim().to_string(), vec![Rule::Series { subject: subject.trim().to_string(), series_id }], members, format!("a meeting that recurs: {n} calls"), &mut taken);
    }
    // 2. One outside organisation's calls (not a recurring meeting already).
    let mut by_dom: BTreeMap<String, Vec<&(PathBuf, CallFacts)>> = BTreeMap::new();
    for x in free.iter().filter(|(d, _)| !taken.contains(&calls::stamp_of(d))) {
        if let Some(d) = main_domain(&x.1, &skip) {
            by_dom.entry(d).or_default().push(x);
        }
    }
    for (d, members) in by_dom {
        let n = members.len();
        let who = client_of(&d).map(|c| c.name.clone()).unwrap_or_else(|| name_from_domain(&d));
        push(format!("domain:{d}"), "people", format!("{who} calls"), vec![Rule::People { domains: if client_of(&d).is_some() { vec![] } else { vec![d.clone()] } }], members, format!("{n} calls mostly with people from {d}"), &mut taken);
    }
    // 3. One Teams chat's calls.
    let mut by_chat: BTreeMap<String, Vec<&(PathBuf, CallFacts)>> = BTreeMap::new();
    for x in free.iter().filter(|(d, _)| !taken.contains(&calls::stamp_of(d))) {
        if let Some(c) = &x.1.chat_id {
            by_chat.entry(c.clone()).or_default().push(x);
        }
    }
    for (chat, members) in by_chat {
        let n = members.len();
        let name = members.iter().rev().find_map(|(_, f)| f.subject.clone()).unwrap_or_else(|| "Calls from one chat".into());
        push(format!("chat:{chat}"), "chat", name, vec![Rule::Chat { id: chat.clone(), name: None }], members, format!("{n} calls from the same Teams chat"), &mut taken);
    }
    out.sort_by(|a, b| b["latest"].as_str().cmp(&a["latest"].as_str()));
    Ok(out)
}

fn found_one(key: &str) -> Result<Value> {
    found()?.into_iter().find(|c| c["key"] == key).ok_or_else(|| anyhow!("that suggestion is gone (its calls may be in a project now)"))
}

/// Make the project a candidate proposes: `name` and `client` as the user left them (a client id,
/// "new" for the candidate's new client, "" for none).
pub fn found_accept(key: &str, name: Option<&str>, client: Option<&str>) -> Result<Project> {
    let c = found_one(key)?;
    let client = match client {
        Some("new") => {
            let nc = &c["new_client"];
            let name = nc["name"].as_str().ok_or_else(|| anyhow!("this suggestion has no new client"))?;
            Some(add_client(name, &[nc["domain"].as_str().unwrap_or("").to_string()], false)?.id)
        }
        Some("") => None,
        Some(id) => Some(id.to_string()),
        None => c["client"].as_str().map(str::to_owned),
    };
    let rules: Vec<Rule> = serde_json::from_value(c["rules"].clone())?;
    let name = name.map(str::trim).filter(|n| !n.is_empty()).unwrap_or(c["name"].as_str().unwrap_or("New project")).to_string();
    let p = create(&name, client.as_deref(), None)?;
    // A people rule for a domain that is now its client's: the client's domains say it.
    let rules = rules
        .into_iter()
        .map(|r| match r {
            Rule::People { domains } if client.is_some() && domains.len() == 1 && clients().ok().into_iter().flatten().any(|x| Some(&x.id) == client.as_ref() && norm_domains(&x.domains) == domains) => Rule::People { domains: vec![] },
            r => r,
        })
        .collect();
    update(&p.id, |p| {
        p.rules = rules;
        Ok(())
    })
}

/// Add a candidate's rules to a project the user has.
pub fn found_merge(key: &str, into: &str) -> Result<Project> {
    let c = found_one(key)?;
    let rules: Vec<Rule> = serde_json::from_value(c["rules"].clone())?;
    update(into, |p| {
        for r in rules {
            if !p.rules.contains(&r) {
                p.rules.push(r);
            }
        }
        Ok(())
    })
}

/// Not a project: never suggested again.
pub fn found_reject(key: &str) -> Result<()> {
    let _l = lock()?;
    let mut st = found_state();
    if !st.rejected.iter().any(|k| k == key) {
        st.rejected.push(key.to_string());
    }
    write_json(&dir().join(FOUND), &st)
}

// ── views (CLI and window) ─────────────────────────────────────────────────────────────────────

pub(crate) fn call_row(d: &Path, why: &str) -> Value {
    let meta = calls::read_json(&d.join("meta.json"));
    json!({
        "name": d.file_name().map(|n| n.to_string_lossy().into_owned()),
        "title": meta["title"],
        "started_at": meta["started_at"],
        "why": why,
        "minutes": d.join("notes.md").exists(),
    })
}

/// A project as the window and `project show` give it.
pub fn view(p: &Project, with_calls: bool) -> Result<Value> {
    let mut v = serde_json::to_value(p)?;
    v["group"] = json!(p.stage.group());
    let b = brief(&p.id)?;
    v["brief_html"] = json!(crate::minutes::markdown_html(&b));
    v["brief"] = json!(b);
    v["has_logo"] = json!(logo(&p.id).is_some());
    v["logo_v"] = json!(logo_v(project_logo_stem(&p.id)));
    v["client_logo_v"] = json!(p.client.as_deref().map(|c| logo_v(client_logo_stem(c))).unwrap_or(0));
    if let Some(c) = p.client.as_deref().and_then(|c| clients().ok()?.into_iter().find(|x| x.id == c)) {
        v["client_name"] = json!(c.name);
        v["client_domains"] = json!(c.domains);
    }
    let calls = calls_of(p)?;
    v["call_count"] = json!(calls.len());
    v["last_call"] = calls.last().map(|(d, _)| calls::read_json(&d.join("meta.json"))["started_at"].clone()).unwrap_or(Value::Null);
    if with_calls {
        v["calls"] = calls.iter().rev().map(|(d, w)| call_row(d, w)).collect();
        // Its meeting chats: how each was read, and its calls' chats it doesn't read yet.
        v["chat_state"] = crate::ledger::chat_state(&p.id);
        v["mail_state"] = crate::ledger::mail_state(&p.id);
        v["chat_suggestions"] = json!(crate::ledger::chat_suggestions(p));
    }
    Ok(v)
}

/// Everything the Projects list needs: clients, projects (no calls), stages.
pub fn overview() -> Result<Value> {
    let projects: Vec<Value> = list().iter().map(|p| view(p, false)).collect::<Result<_>>()?;
    let clients: Vec<Value> = clients()?
        .into_iter()
        .map(|c| {
            let v = logo_v(client_logo_stem(&c.id));
            let mut x = serde_json::to_value(c).unwrap_or_default();
            x["logo_v"] = json!(v);
            x
        })
        .collect();
    Ok(json!({
        "clients": clients,
        "projects": projects,
        "stages": Stage::ALL.map(|s| json!({"id": s.id(), "group": s.group()})),
    }))
}

/// A project as a call shows it (a chip): name, colour, icon, why it belongs.
pub fn chip(p: &Project, why: &str) -> Value {
    json!({
        "id": p.id, "name": p.name, "color": p.color, "icon": p.icon, "client": p.client,
        "has_logo": logo(&p.id).is_some(), "logo_v": logo_v(project_logo_stem(&p.id)),
        "client_logo_v": p.client.as_deref().map(|c| logo_v(client_logo_stem(c))).unwrap_or(0),
        "why": why, "closed": p.stage.closed(),
    })
}

/// Projects for each call name, for the calls list's chips (one pass over the calls).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn by_call() -> BTreeMap<String, Vec<Value>> {
    let (Ok(clients), me) = (clients(), me()) else { return BTreeMap::new() };
    let projects = list();
    let mut out = BTreeMap::new();
    if projects.is_empty() {
        return out;
    }
    for d in calls::all_calls() {
        let f = CallFacts::of(&d, me.as_ref());
        let hits: Vec<Value> = projects.iter().filter_map(|p| matches(p, &clients, &f).map(|w| chip(p, w))).collect();
        if !hits.is_empty() {
            out.insert(d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), hits);
        }
    }
    out
}

/// Earlier calls of a project a minutes run sees (their highlights, not their files).
const RECENT_CALLS: usize = 5;

/// What a minutes run should know of the call's projects (`echoctl context`): each one's brief,
/// glossary, people, notes for Claude, and the highlights of its last calls before this one (from
/// their minutes, deterministic: `calls::recap_from_notes`) — so the run needs no other call's files.
pub fn context_for(dir: &Path) -> Value {
    let Ok(projects) = of_call(dir) else { return json!([]) };
    let stamp = calls::stamp_of(dir);
    projects
        .iter()
        .map(|(p, why)| {
            let recent: Vec<Value> = calls_of(p)
                .unwrap_or_default()
                .into_iter()
                .filter(|(d, _)| calls::stamp_of(d) < stamp)
                .filter_map(|(d, _)| {
                    let notes = fs::read_to_string(d.join("notes.md")).ok()?;
                    let meta = calls::read_json(&d.join("meta.json"));
                    Some(json!({"title": meta["title"], "started_at": meta["started_at"], "highlights": calls::recap_from_notes(&notes)}))
                })
                .rev()
                .take(RECENT_CALLS)
                .collect();
            json!({
                "id": p.id, "name": p.name, "stage": p.stage.id(), "why": why,
                "brief": brief(&p.id).unwrap_or_default(),
                "notes": p.notes,
                "terms": p.terms.iter().map(|t| &t.term).collect::<Vec<_>>(),
                "people": p.people,
                "recent_calls": recent,
            })
        })
        .collect()
}

/// Glossary terms of the projects a meeting belongs to (for its transcription hint).
pub fn terms_for_meeting(m: &MeetingContext) -> Vec<String> {
    let ps = of_meeting(m);
    let mut out: Vec<String> = vec![];
    // The Brain's taxonomy (it supersedes the glossaries): terms pinned to or mentioned in these
    // threads, and the always-on ones; then any glossary term not moved in yet.
    #[cfg(feature = "brain")]
    let from_brain = if crate::brain::enabled() { crate::brain::taxonomy::terms_for_threads(&ps.iter().map(|p| p.id.clone()).collect::<Vec<_>>()) } else { vec![] };
    #[cfg(not(feature = "brain"))]
    let from_brain: Vec<String> = vec![];
    for t in from_brain.into_iter().chain(ps.into_iter().flat_map(|p| p.terms).map(|t| t.term)) {
        if !out.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(name: &str, email: &str) -> Person {
        Person { name: name.into(), email: Some(email.into()) }
    }

    fn call(name: &str, meeting: Option<MeetingContext>, chat: Option<&str>) -> PathBuf {
        let d = paths::calls_dir().join(name);
        fs::create_dir_all(&d).unwrap();
        let started = format!("{}-{}-{}T{}:{}:{}+02:00", &name[..4], &name[5..7], &name[8..10], &name[11..13], &name[13..15], &name[15..17]);
        fs::write(d.join("meta.json"), json!({"title": name, "started_at": started}).to_string()).unwrap();
        if let Some(m) = meeting {
            m.save(&d).unwrap();
        }
        if let Some(c) = chat {
            fs::write(d.join("chat.id"), c).unwrap();
        }
        d
    }

    fn meeting(subject: &str, people: &[Person]) -> MeetingContext {
        MeetingContext { subject: Some(subject.into()), attendees: people.to_vec(), ..Default::default() }
    }

    #[test]
    fn stages_parse_and_group() {
        assert_eq!(Stage::parse("On hold").unwrap(), Stage::OnHold);
        assert_eq!(Stage::parse("wrap-up").unwrap(), Stage::WrapUp);
        assert_eq!(Stage::Negotiation.group(), "pursuit");
        assert_eq!(Stage::Kickoff.group(), "delivery");
        assert!(Stage::Lost.closed() && !Stage::OnHold.closed());
        assert!(Stage::parse("won").is_err());
        assert_eq!(serde_json::to_value(Stage::WrapUp).unwrap(), json!("wrap_up"));
    }

    #[test]
    fn rules_pins_and_exclusions() {
        let (_env, _root) = paths::testenv::folders("projects-rules");
        let dana = person("Dana Kowal", "dana@northwind.example");
        let ravi = person("Ravi Mehta", "ravi@northwind.example");
        let lena = person("Lena Fischer", "lena@ours.example");
        let weekly = call("2026-10-22_110000-aurora-weekly", Some(meeting("Aurora weekly #12", &[dana.clone(), lena.clone()])), None);
        let review = call("2026-10-23_150000-security", Some(meeting("Security questions", std::slice::from_ref(&ravi))), None);
        let chat = call("2026-10-24_090000-ad-hoc", None, Some("19:abc@thread.v2"));
        let other = call("2026-10-25_100000-coffee", Some(meeting("Coffee", std::slice::from_ref(&lena))), None);

        let nw = add_client("Northwind", &["@Northwind.example".into()], false).unwrap();
        assert_eq!(nw.domains, vec!["northwind.example"]);
        let p = create("Aurora · agent hub", Some(&nw.id), None).unwrap();
        assert_eq!((p.id.as_str(), p.stage), ("aurora-agent-hub", Stage::Active));
        let p = update(&p.id, |p| {
            p.rules = vec![
                Rule::Series { subject: "aurora weekly".into(), series_id: None },
                Rule::People { domains: vec![] },
                Rule::Chat { id: "19:abc@thread.v2".into(), name: None },
            ];
            Ok(())
        })
        .unwrap();
        let why: Vec<(String, &str)> = calls_of(&p).unwrap().into_iter().map(|(d, w)| (calls::stamp_of(&d), w)).collect();
        assert_eq!(why, vec![("2026-10-22_110000".into(), "series"), ("2026-10-23_150000".into(), "people"), ("2026-10-24_090000".into(), "chat")]);

        // Pins win over the rules, exclusions over both.
        set_membership(&p.id, &other, "include").unwrap();
        let p = set_membership(&p.id, &review, "exclude").unwrap();
        let names: Vec<String> = calls_of(&p).unwrap().into_iter().map(|(d, _)| calls::stamp_of(&d)).collect();
        assert_eq!(names, vec!["2026-10-22_110000", "2026-10-24_090000", "2026-10-25_100000"]);
        let p = set_membership(&p.id, &review, "rules").unwrap();
        assert_eq!(calls_of(&p).unwrap().len(), 4);
        assert_eq!(of_call(&weekly).unwrap()[0].1, "series");
        assert!(of_call(&chat).unwrap().iter().any(|(x, _)| x.id == p.id));

        // `since` drops older calls; a closed project takes no calls started after it closed.
        let p = update(&p.id, |p| {
            p.since = Some("2026-10-23".into());
            Ok(())
        })
        .unwrap();
        assert_eq!(calls_of(&p).unwrap().len(), 3);
        let p = set_stage(&p.id, Stage::Done, Some("pilot live")).unwrap();
        assert_eq!(p.stages.last().unwrap().reason.as_deref(), Some("pilot live"));
        assert_eq!(calls_of(&p).unwrap().len(), 3, "its calls stay");
        let later = call("2099-01-01_100000-aurora-weekly", Some(meeting("Aurora weekly", &[dana])), None);
        assert!(of_call(&later).unwrap().is_empty());

        // A calendar event added by hand: the event, and the call later recorded from it, belong.
        let q = create("Quarterly", None, None).unwrap();
        let ev = MeetingContext { id: Some("AAMk-1".into()), subject: Some("Board prep".into()), ..Default::default() };
        assert!(for_event(&ev).iter().all(|c| c["id"] != q.id.as_str()));
        set_event(&q.id, "AAMk-1", "include").unwrap();
        assert_eq!(for_event(&ev).iter().find(|c| c["id"] == q.id.as_str()).unwrap()["why"], "pinned");
        let rec = call("2026-10-26_100000-board-prep", Some(ev.clone()), None);
        assert!(of_call(&rec).unwrap().iter().any(|(x, w)| x.id == q.id && *w == "pinned"));
        set_event(&q.id, "AAMk-1", "remove").unwrap();
        assert!(of_call(&rec).unwrap().iter().all(|(x, _)| x.id != q.id));
    }

    #[test]
    fn people_rule_needs_half_of_the_others() {
        let clients = vec![Client { id: "nw".into(), name: "Northwind".into(), domains: vec!["northwind.example".into()], ..Default::default() }];
        let mut p = Project {
            id: "x".into(),
            name: "X".into(),
            client: Some("nw".into()),
            color: COLORS[0].into(),
            icon: None,
            stage: Stage::Active,
            stages: vec![],
            since: None,
        history_before: None,
            rules: vec![Rule::People { domains: vec![] }],
            pins: vec![],
            exclusions: vec![],
            events: vec![],
            board: None,
            sources: vec![],
            people: vec![],
            orgs: vec![],
            terms: vec![],
            notes: String::new(),
            ai_profile: None,
            model: None,
            created_at: String::new(),
        };
        let f = |others: &[&str]| CallFacts { others: others.iter().map(|s| s.to_string()).collect(), ..Default::default() };
        assert_eq!(matches(&p, &clients, &f(&["a@northwind.example", "b@ours.example"])), Some("people"));
        assert_eq!(matches(&p, &clients, &f(&["a@northwind.example", "b@ours.example", "c@ours.example"])), None);
        // Colleagues don't count: the user's own domain, or a client marked as their company.
        let mut g = f(&["a@northwind.example", "b@ours.example", "c@ours.example", "d@contoso.example", "e@contoso.example"]);
        g.own_domain = Some("ours.example".into());
        assert_eq!(matches(&p, &clients, &g), None, "contoso is outside here");
        let mut with_contoso = clients.clone();
        with_contoso.push(Client { id: "contoso".into(), name: "Contoso".into(), domains: vec!["contoso.example".into()], internal: true, ..Default::default() });
        assert_eq!(matches(&p, &with_contoso, &g), Some("people"));
        assert_eq!(matches(&p, &with_contoso, &f(&["d@contoso.example"])), None, "only colleagues: nobody outside");
        assert_eq!(matches(&p, &clients, &f(&[])), None);
        p.client = None; // no client, no domains: the rule matches nothing
        assert_eq!(matches(&p, &clients, &f(&["a@northwind.example"])), None);
        p.rules = vec![Rule::Subject { contains: "Helios".into() }];
        let s = |subject: &str| CallFacts { subject: Some(subject.into()), ..Default::default() };
        assert_eq!(matches(&p, &clients, &s("Helios weekly — waves")), Some("subject"));
        assert_eq!(matches(&p, &clients, &s("Heliosphere")), None, "whole words only");
    }

    #[test]
    fn suggestions_and_bad_input() {
        let (_env, _root) = paths::testenv::folders("projects-suggest");
        let dana = person("Dana Kowal", "dana@northwind.example");
        let tomas = person("Tomás Ruiz", "tomas@ours.example");
        call("2026-10-22_110000-aurora-weekly", Some(meeting("Aurora weekly", &[dana.clone(), tomas.clone()])), None);
        let adhoc = call("2026-10-24_090000-follow-up", Some(meeting("Follow-up", &[dana.clone(), tomas.clone()])), None);
        let named = call("2026-10-25_090000-stargate", Some(meeting("Stargate migration plan", &[person("Kim", "kim@x.example")])), None);
        let p = create("Aurora", None, Some(Stage::Kickoff)).unwrap();
        update(&p.id, |p| {
            p.rules = vec![Rule::Series { subject: "Aurora weekly".into(), series_id: None }];
            p.terms = vec![Term { term: "Stargate".into(), kind: Some("system".into()), note: None }];
            Ok(())
        })
        .unwrap();
        let s = suggest(&adhoc).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0]["id"], "aurora");
        assert!(s[0]["why"].as_str().unwrap().contains("2 of its 2 people"), "{s:?}");
        assert!(suggest(&named).unwrap()[0]["why"].as_str().unwrap().contains("Stargate"));
        set_membership(&p.id, &adhoc, "exclude").unwrap();
        assert!(suggest(&adhoc).unwrap().is_empty(), "Not this is remembered");

        assert!(create("Aurora", None, None).unwrap().id == "aurora-2");
        assert!(update("aurora", |p| {
            p.client = Some("nobody".into());
            Ok(())
        })
        .is_err());
        assert!(update("aurora", |p| {
            p.rules.push(Rule::Subject { contains: "  ".into() });
            Ok(())
        })
        .is_err());
        assert!(load("../etc").is_err());
        // A board keeps a visible column for every state but waiting; names and ids are checked.
        let col = |id: &str, state: &str| Column { id: id.into(), name: id.into(), color: "#888888".into(), state: state.into(), limit: None, hidden: false };
        let good = Board { columns: vec![col("todo", "open"), col("doing", "in_progress"), col("stuck", "blocked"), col("done", "done")], fields: vec![], group: "none".into(), views: vec![json!({"id": "v1", "name": "Board", "layout": "board"})] };
        update("aurora", |p| {
            p.board = Some(good.clone());
            Ok(())
        })
        .unwrap();
        let mut no_done = good.clone();
        no_done.columns.pop();
        assert!(update("aurora", |p| {
            p.board = Some(no_done);
            Ok(())
        })
        .is_err());
        let mut twice = good.clone();
        twice.columns.push(col("todo", "open"));
        assert!(update("aurora", |p| {
            p.board = Some(twice);
            Ok(())
        })
        .is_err());
        // Logos: an image by its first bytes, one at a time; anything else refused.
        let png = std::env::temp_dir().join(format!("echo-logo-{}.png", std::process::id()));
        fs::write(&png, b"\x89PNG\r\n\x1a\nrest").unwrap();
        set_logo("aurora", Some(&png)).unwrap();
        assert_eq!(logo("aurora").unwrap().1, "image/png");
        fs::write(&png, b"<svg onload=alert(1)>").unwrap();
        assert!(set_logo("aurora", Some(&png)).is_err());
        assert!(logo("aurora").is_some(), "a refused file leaves the logo");
        set_logo("aurora", None).unwrap();
        assert!(logo("aurora").is_none());
        let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3];
        let nw = add_client("Northwind", &[], false).unwrap();
        set_client_logo(&nw.id, Some(&jpg)).unwrap();
        assert_eq!(client_logo(&nw.id).unwrap().1, "image/jpeg");
        assert!(set_client_logo("nobody", Some(&jpg)).is_err());
        assert!(set_client_logo(&nw.id, Some(&vec![0x89; LOGO_MAX + 1])).is_err());
        remove_client(&nw.id).unwrap();
        assert!(client_logo(&nw.id).is_none(), "a removed client's logo goes too");
        let _ = fs::remove_file(&png);
        assert!(remove_client("nobody").is_err());
        let c = add_client("Contoso", &[], false).unwrap();
        update("aurora", |p| {
            p.client = Some(c.id.clone());
            Ok(())
        })
        .unwrap();
        assert!(remove_client(&c.id).is_err(), "it still has a project");
        assert_eq!(terms_for_meeting(&meeting("Aurora weekly", &[dana])), vec!["Stargate"]);
    }

    #[test]
    fn found_for_you() {
        let (_env, _root) = paths::testenv::folders("projects-found");
        let now = chrono::Local::now();
        let at = |days: i64, h: u32| (now - chrono::Duration::days(days)).format(&format!("%Y-%m-%d_{h:02}0000")).to_string();
        let dana = person("Dana Kowal", "dana@northwind.example");
        let ravi = person("Ravi Mehta", "ravi@northwind.example");
        let kim = person("Kim Lee", "kim@contoso.example");
        let pat = person("Pat Ode", "pat@gmail.com");
        // A recurring meeting (3 calls), two calls with Contoso, one call alone, two with personal mail.
        for d in [1, 8, 15] {
            call(&format!("{}-aurora-weekly", at(d, 11)), Some(meeting(&format!("Aurora weekly #{d}"), std::slice::from_ref(&dana))), None);
        }
        call(&format!("{}-intro", at(2, 9)), Some(meeting("Intro", std::slice::from_ref(&kim))), None);
        call(&format!("{}-follow-up", at(3, 9)), Some(meeting("Follow-up on pricing", std::slice::from_ref(&kim))), None);
        call(&format!("{}-security", at(4, 15)), Some(meeting("Security questions", &[ravi])), None);
        call(&format!("{}-p1", at(5, 10)), Some(meeting("Catch-up A", std::slice::from_ref(&pat))), None);
        call(&format!("{}-p2", at(6, 10)), Some(meeting("Catch-up B", &[pat])), None);
        call(&format!("{}-old", at(200, 10)), Some(meeting("Intro", std::slice::from_ref(&kim))), None);
        let f = found().unwrap();
        let keys: Vec<&str> = f.iter().map(|c| c["key"].as_str().unwrap()).collect();
        assert!(keys.contains(&"series:aurora weekly"), "{keys:?}");
        assert!(keys.contains(&"domain:contoso.example"), "{keys:?}");
        assert!(!keys.iter().any(|k| k.contains("gmail")), "personal mail makes no client: {keys:?}");
        assert_eq!(keys.len(), 2, "one call alone is no project, a 200-day-old call is out: {keys:?}");
        let s = f.iter().find(|c| c["key"] == "series:aurora weekly").unwrap();
        assert_eq!(s["calls"].as_array().unwrap().len(), 3);
        assert_eq!(s["new_client"]["domain"], "northwind.example");
        assert_eq!(s["new_client"]["name"], "Northwind");

        // Accept with its new client: the project has the series rule and the client; its calls leave the list.
        let p = found_accept("series:aurora weekly", Some("Aurora"), Some("new")).unwrap();
        assert_eq!((p.name.as_str(), p.client.as_deref()), ("Aurora", Some("northwind")));
        assert_eq!(calls_of(&p).unwrap().len(), 3);
        // The client now known, Contoso's candidate is still there; reject it and it is gone for good.
        found_reject("domain:contoso.example").unwrap();
        assert!(found().unwrap().is_empty());
        assert!(found_accept("domain:contoso.example", None, None).is_err());
    }

    #[test]
    fn context_gives_the_projects_and_their_recent_calls() {
        let (_env, _root) = paths::testenv::folders("projects-context");
        let dana = person("Dana Kowal", "dana@northwind.example");
        let first = call("2026-10-15_110000-aurora-weekly", Some(meeting("Aurora weekly", std::slice::from_ref(&dana))), None);
        fs::write(first.join("notes.md"), "# Aurora weekly\n\n## Decisions\n- Pilot moves to 24 Nov\n").unwrap();
        call("2026-10-22_110000-aurora-weekly", Some(meeting("Aurora weekly", std::slice::from_ref(&dana))), None);
        let now = call("2026-10-29_110000-aurora-weekly", Some(meeting("Aurora weekly", &[dana])), None);
        let later = call("2026-11-05_110000-aurora-weekly", None, None);
        fs::write(later.join("notes.md"), "## Decisions\n- From the future\n").unwrap();
        let p = create("Aurora", None, None).unwrap();
        update(&p.id, |p| {
            p.rules = vec![Rule::Series { subject: "Aurora weekly".into(), series_id: None }];
            p.notes = "“the store” is Aurora".into();
            Ok(())
        })
        .unwrap();
        set_brief(&p.id, "Phase 2 of the store.").unwrap();
        let c = context_for(&now);
        assert_eq!(c[0]["brief"], "Phase 2 of the store.");
        assert_eq!(c[0]["why"], "series");
        let recent = c[0]["recent_calls"].as_array().unwrap();
        assert_eq!(recent.len(), 1, "only earlier calls with minutes: {recent:?}");
        assert!(recent[0]["highlights"].to_string().contains("24 Nov"));
    }
}

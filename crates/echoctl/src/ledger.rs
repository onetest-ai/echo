//! A project's work ledger (`docs/projects-design.md`, ported from taska): Workstream (epic) ›
//! Story › Task, plus decision, risk and question records — folded from an append-only event log.
//!
//! - **Evidence**: each of the project's calls becomes one item (`evidence/<stamp>.json` + a readable
//!   `.md`): its transcript as turns, its chat and the user's notes as text. A call that changed after
//!   it was processed is queued again.
//! - **Agents propose, the code disposes** (`apply`): every proposal quotes one contiguous span of
//!   its item, verbatim; a quote that isn't there, a broken level rule (a story needs 2–6 acceptance
//!   criteria, a task a story and an owner, a date or the type session/dependency) or a change to a
//!   node before it existed is refused. Status changes, criteria met, links and waits count only once
//!   an independent verifier says `supports` (`verify_prep` / `verify_apply`).
//! - **The user's edits win** (`human_event`): a field they set is pinned, and an agent's later
//!   evidence lands in the history without changing it. Every edit can be undone.
//!
//! ```text
//! <home>/projects/<id>/ledger/events.jsonl      every event, one per line, appended
//! <home>/projects/<id>/ledger/evidence/<id>.json|.md
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde::Serialize;
use serde_json::{json, Value};

use crate::calls;
use crate::evidence::{call_item, render_md};

mod apply;
mod chat;
mod edits;
mod mail;
pub use apply::*;
pub use chat::*;
pub use edits::*;
pub use mail::*;

const STATUS: [&str; 9] = ["held", "done", "progress", "blocked", "cancelled", "reopen", "due", "delayed", "reassign"];
const VERIFIED_OPS: [&str; 4] = ["ac_met", "link", "waiting", "answer"];
const KINDS: [&str; 6] = ["epic", "story", "task", "risk", "decision", "question"];
const TASK_TYPES: [&str; 3] = ["action", "session", "dependency"];
const CLOSED: [&str; 4] = ["done", "cancelled", "merged", "deleted"];
const HUMAN_FIELDS: [&str; 20] = [
    "title", "internal_owner", "client_owner", "due", "state", "parent", "type", "client_owed", "waiting_for", "column", "links", "description", "delegated_by",
    "decided_by", "likelihood", "impact", "mode", "how", "prompt", "skill",
];
/// A risk's likelihood and impact.
const LEVELS: [&str; 3] = ["low", "med", "high"];
/// Where a node came from outside the calls (a mail, a ticket): at most this many `{title, url}`, https only.
const MAX_LINKS: usize = 10;
/// A description (Markdown): the user's at most this many characters; an agent's, from what was said, shorter.
const MAX_DESC: usize = 4000;
const MAX_AGENT_DESC: usize = 1200;
const AGENT_OPS: [&str; 10] = ["create", "status", "revise", "ac_met", "link", "waiting", "answer", "suggest_move", "suggest_merge", "note"];
const HUMAN_OPS: [&str; 13] = ["edit", "ac_add", "ac_set", "link", "unlink", "merge", "delete", "accept", "reject", "undo", "create", "xref", "unxref"];
/// A link to an item in another thread (#20): one piece of work raised in both (`same`), or this one waits
/// on that one (`depends_on`; the other side records `blocks`).
const XREF_KINDS: [&str; 3] = ["same", "depends_on", "blocks"];
const HUMAN_STATES: [&str; 5] = ["open", "in_progress", "blocked", "done", "cancelled"];
const MAX_AC: usize = 6;
const HIST_KEYS: [&str; 34] = [
    "mid", "type", "op", "effect", "evidence", "date", "ts", "speaker", "quote", "note", "due_text", "due", "by", "kind", "set", "text", "met", "ac",
    "ac_text", "depends_on", "into", "title", "ac_add", "who", "what", "source", "thread", "target", "master", "internal_owner", "client_owner",
    "delegated_by", "at", "drop",
];
/// The workstream every project starts with.
const FIRST_EPIC: &str = "General";

fn parent_ok(kind: &str, parent_kind: Option<&str>) -> bool {
    match kind {
        "story" => parent_kind == Some("epic"),
        "task" => parent_kind == Some("story"),
        "risk" | "decision" | "question" => matches!(parent_kind, Some("epic") | Some("story")),
        "epic" => parent_kind.is_none(),
        _ => false,
    }
}

fn prefix(kind: &str) -> &'static str {
    match kind {
        "epic" => "E",
        "story" => "S",
        "task" => "T",
        "risk" => "R",
        "decision" => "D",
        _ => "Q",
    }
}

fn new_id(kind: &str, n: usize) -> String {
    if kind == "epic" {
        format!("E{n:02}")
    } else {
        format!("{}{n:03}", prefix(kind))
    }
}

// ── text ───────────────────────────────────────────────────────────────────────────────────────

/// Lower-case, punctuation-free, single-spaced: the form quotes are compared in.
pub fn norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars().flat_map(char::to_lowercase) {
        let c = match c {
            '‘' | '’' | '“' | '”' | '`' => '\'',
            c => c,
        };
        if c.is_alphanumeric() || c == '_' || c == '\'' {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        } else {
            space = true;
        }
    }
    out
}

fn mid_of(parts: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(parts.join("|").as_bytes());
    h.iter().take(5).map(|b| format!("{b:02x}")).collect()
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];
const WEEKDAYS: [&str; 7] = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

fn month_of(word: &str) -> Option<u32> {
    let w = word.trim_end_matches('.');
    let full = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
    if let Some(i) = full.iter().position(|m| *m == w) {
        return Some(i as u32 + 1);
    }
    if w == "sept" {
        return Some(9);
    }
    MONTHS.iter().position(|m| *m == w).map(|i| i as u32 + 1)
}

fn day_of(word: &str) -> Option<u32> {
    let d = word.trim_end_matches(|c: char| c.is_alphabetic());
    let tail = &word[d.len()..];
    if !(tail.is_empty() || ["st", "nd", "rd", "th"].contains(&tail)) {
        return None;
    }
    d.parse::<u32>().ok().filter(|d| (1..=31).contains(d))
}

/// A written-month date in a phrase: "March 14", "25 Aug", "14th of October", "25_Aug".
fn month_day(s: &str) -> Option<(u32, u32)> {
    let s = s.replace(['-', '_', ','], " ");
    let w: Vec<&str> = s.split_whitespace().collect();
    for i in 0..w.len() {
        if let Some(m) = month_of(w[i]) {
            if let Some(d) = w.get(i + 1).and_then(|x| day_of(x)) {
                return Some((m, d));
            }
            let before = if i >= 2 && w[i - 1] == "of" { w.get(i - 2) } else if i >= 1 { w.get(i - 1) } else { None };
            if let Some(d) = before.and_then(|x| day_of(x)) {
                return Some((m, d));
            }
        }
    }
    None
}

fn next_working(mut d: NaiveDate) -> NaiveDate {
    while matches!(d.weekday(), Weekday::Sat | Weekday::Sun) {
        d += Duration::days(1);
    }
    d
}

fn has_word(t: &str, w: &str) -> bool {
    t.split(|c: char| !c.is_alphanumeric()).any(|x| x == w)
}

/// "the eighteenth" → "the 18th", "twenty-first" → "21st": spelled ordinals as digits.
fn ordinals(t: &str) -> String {
    const ONES: [(&str, u32); 9] = [("first", 1), ("second", 2), ("third", 3), ("fourth", 4), ("fifth", 5), ("sixth", 6), ("seventh", 7), ("eighth", 8), ("ninth", 9)];
    const TEENS: [(&str, u32); 11] = [("tenth", 10), ("eleventh", 11), ("twelfth", 12), ("thirteenth", 13), ("fourteenth", 14), ("fifteenth", 15), ("sixteenth", 16), ("seventeenth", 17), ("eighteenth", 18), ("nineteenth", 19), ("twentieth", 20)];
    let t = t.replace("twenty-", "twenty ").replace("thirty-", "thirty ");
    let w: Vec<&str> = t.split_whitespace().collect();
    let mut out: Vec<String> = vec![];
    let mut i = 0;
    while i < w.len() {
        let tens = match w[i] {
            "twenty" => Some(20),
            "thirty" => Some(30),
            _ => None,
        };
        if let (Some(tn), Some(o)) = (tens, w.get(i + 1).and_then(|x| ONES.iter().find(|(n, _)| n == x))) {
            out.push(format!("{}th", tn + o.1));
            i += 2;
            continue;
        }
        if w[i] == "thirtieth" {
            out.push("30th".into());
        } else if let Some((_, n)) = ONES.iter().chain(TEENS.iter()).find(|(n, _)| *n == w[i]) {
            out.push(format!("{n}th"));
        } else {
            out.push(w[i].to_string());
        }
        i += 1;
    }
    out.join(" ")
}

/// Spoken date words → an ISO date, resolved against the day they were said; None when not a date.
/// ISO dates, "Oct 14" / "14th of October", today / eod, tomorrow (next working day), "the 12th",
/// "middle of next week" (its Wednesday), end of week, "next week" (its Friday), weekday names.
pub fn resolve_due(text: Option<&str>, said: &str) -> Option<String> {
    let t = ordinals(&text?.trim().to_lowercase());
    if t.is_empty() {
        return None;
    }
    let d = NaiveDate::parse_from_str(&said[..said.len().min(10)], "%Y-%m-%d").ok()?;
    for w in t.split(|c: char| !(c.is_ascii_digit() || c == '-')) {
        if w.len() == 10 && w.as_bytes()[4] == b'-' && w.as_bytes()[7] == b'-' {
            return NaiveDate::parse_from_str(w, "%Y-%m-%d").ok().map(|x| x.to_string());
        }
    }
    if let Some((m, day)) = month_day(&t) {
        let mut x = NaiveDate::from_ymd_opt(d.year(), m, day)?;
        if x < d - Duration::days(180) {
            x = NaiveDate::from_ymd_opt(d.year() + 1, m, day)?;
        }
        return Some(x.to_string());
    }
    if has_word(&t, "today") || has_word(&t, "eod") || t.contains("end of day") {
        return Some(d.to_string());
    }
    if has_word(&t, "tomorrow") {
        return Some(next_working(d + Duration::days(1)).to_string());
    }
    let words: Vec<&str> = t.split_whitespace().collect();
    if let Some(i) = words.iter().position(|w| *w == "the") {
        if let Some(day) = words.get(i + 1).and_then(|w| day_of(w.trim_end_matches(|c: char| !c.is_alphanumeric()))).filter(|_| words[i + 1].chars().any(char::is_alphabetic)) {
            let (mut y, mut mo) = (d.year(), d.month());
            if day < d.day() {
                (y, mo) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
            }
            return NaiveDate::from_ymd_opt(y, mo, day).map(|x| x.to_string());
        }
    }
    let next = has_word(&t, "next");
    let wd = d.weekday().num_days_from_monday() as i64;
    let week = |x: NaiveDate| x.iso_week().week();
    if (has_word(&t, "middle") || has_word(&t, "mid")) && t.contains("week") {
        let mut wed = d + Duration::days((2 - wd).rem_euclid(7));
        if next && week(wed) == week(d) {
            wed += Duration::days(7);
        }
        return Some(wed.to_string());
    }
    if t.contains("end of week") || t.contains("end of the week") || t.contains("this week") || has_word(&t, "eow") {
        return Some((d + Duration::days((4 - wd).rem_euclid(7))).to_string());
    }
    if next && t.contains("week") && !WEEKDAYS.iter().any(|w| t.contains(w)) {
        return Some((d + Duration::days((4 - wd).rem_euclid(7) + 7)).to_string());
    }
    for (i, w) in WEEKDAYS.iter().enumerate() {
        if t.contains(w) || has_word(&t, &w[..3]) {
            let mut delta = (i as i64 - wd).rem_euclid(7);
            if delta == 0 {
                delta = 7;
            }
            if next && delta < 7 && week(d + Duration::days(delta)) == week(d) {
                delta += 7;
            }
            return Some((d + Duration::days(delta)).to_string());
        }
    }
    None
}

// ── store ──────────────────────────────────────────────────────────────────────────────────────

pub struct Ledger {
    pub(crate) dir: PathBuf,
}

impl Ledger {
    pub fn of(project: &str) -> Result<Ledger> {
        crate::projects::load(project)?;
        Ok(Ledger { dir: crate::projects::dir().join(project).join("ledger") })
    }

    fn lock(&self) -> Result<crate::app::FileLock> {
        fs::create_dir_all(self.dir.join("evidence"))?;
        let path = self.dir.join(".lock");
        let t = std::time::Instant::now();
        loop {
            if let Some(l) = crate::app::FileLock::try_take(&path)? {
                return Ok(l);
            }
            if t.elapsed() > std::time::Duration::from_secs(30) {
                bail!("the project's ledger is busy (another sync?)");
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    pub fn events(&self) -> Result<Vec<Value>> {
        let p = self.dir.join("events.jsonl");
        let text = match fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e).with_context(|| format!("reading {}", p.display())),
        };
        // A line cut short (a crash mid-write) is skipped, never the whole log.
        Ok(text.lines().filter(|l| !l.trim().is_empty()).filter_map(|l| serde_json::from_str(l).ok()).collect())
    }

    fn append(&self, recs: &[Value]) -> Result<()> {
        if recs.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(&self.dir)?;
        let data: String = recs.iter().map(|r| format!("{r}\n")).collect();
        let mut f = fs::OpenOptions::new().create(true).append(true).open(self.dir.join("events.jsonl"))?;
        f.write_all(data.as_bytes())?; // one append: concurrent writers never interleave lines
        Ok(())
    }

    pub fn evidence(&self) -> BTreeMap<String, Value> {
        let mut out = BTreeMap::new();
        if let Ok(rd) = fs::read_dir(self.dir.join("evidence")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "json") {
                    if let Some(v) = fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
                        if let Some(id) = v["id"].as_str() {
                            out.insert(id.to_string(), v);
                        }
                    }
                }
            }
        }
        out
    }

    /// The readable copy of an item, for the agent.
    pub fn evidence_md(&self, id: &str) -> PathBuf {
        self.dir.join("evidence").join(format!("{id}.md"))
    }
}

fn order(ev: &BTreeMap<String, Value>) -> Vec<String> {
    let mut ids: Vec<&String> = ev.keys().collect();
    ids.sort_by(|a, b| (ev[*a]["at"].as_str().unwrap_or(""), *a).cmp(&(ev[*b]["at"].as_str().unwrap_or(""), *b)));
    ids.into_iter().cloned().collect()
}

/// Evidence processed and not queued again since.
/// Evidence the sync has read, oldest first (by the item's order).
pub fn processed_ids(project: &str) -> Result<Vec<String>> {
    let l = Ledger::of(project)?;
    let done = processed(&l.events()?);
    Ok(order(&l.evidence()).into_iter().filter(|e| done.contains(e)).collect())
}

fn processed(evs: &[Value]) -> BTreeSet<String> {
    let mut done = BTreeSet::new();
    for e in evs {
        let id = e["evidence"].as_str().unwrap_or("").to_string();
        match e["type"].as_str() {
            Some("processed") => {
                done.insert(id);
            }
            Some("reprocess") => {
                done.remove(&id);
            }
            _ => {}
        }
    }
    done
}

fn full_text(item: &Value) -> String {
    let mut parts: Vec<&str> = item["turns"].as_array().map(|a| a.iter().filter_map(|t| t["text"].as_str()).collect()).unwrap_or_default();
    if let Some(t) = item["text"].as_str() {
        parts.push(t);
    }
    parts.push(item["title"].as_str().unwrap_or(""));
    parts.join(" ")
}

/// Where a quote stands in an evidence item — one contiguous span of it (normalised, ≥ 3 words) —
/// with the item's day, call, and the turn's time and speaker. None: not verbatim, or no such item.
pub fn quote_in(project: &str, evid: &str, quote: &str) -> Result<Option<Value>> {
    let l = Ledger::of(project)?;
    let Some(item) = l.evidence().get(evid).cloned() else { return Ok(None) };
    let q = norm(quote);
    if q.split(' ').count() < 3 || !norm(&full_text(&item)).contains(&q) {
        return Ok(None);
    }
    let head: String = q.chars().take(40).collect();
    let turn = item["turns"].as_array().into_iter().flatten().find(|t| norm(t["text"].as_str().unwrap_or("")).contains(&head));
    Ok(Some(json!({"evidence": evid, "date": item["date"], "call": item["call"], "title": item["title"], "kind": item["kind"], "url": item["url"],
                   "ts": turn.map(|t| t["ts"].clone()), "speaker": turn.map(|t| t["speaker"].clone())})))
}

// ── fold ───────────────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct Ac {
    pub text: String,
    pub met: bool,
    pub met_by: Option<String>,
    pub source: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub human: bool,
    #[serde(skip)]
    dropped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub parent: Option<String>,
    #[serde(rename = "type")]
    pub task_type: Option<String>,
    pub internal_owner: Option<String>,
    pub client_owner: Option<String>,
    /// Who handed the task over and checks it ("Me": the user delegated it). Unset: whoever's words
    /// created it (`creator`) is taken for it in the Tasks piles.
    pub delegated_by: Option<String>,
    pub client_owed: bool,
    pub ac: Vec<Ac>,
    pub depends_on: Vec<String>,
    pub state: String,
    pub due: Option<String>,
    pub due_history: Vec<Value>,
    pub history: Vec<Value>,
    pub pinned: Vec<String>,
    pub suggestions: Vec<Value>,
    pub merged_into: Option<String>,
    pub waiting_for: Option<Value>,
    /// The board column the user put it in (a project's own column id).
    pub column: Option<String>,
    /// The user's links to where it came from (`{title, url}`): a mail, a ticket.
    pub links: Vec<Value>,
    /// What the work is, in Markdown: an agent's from the words that created it, else the user's (pinned).
    /// A record's substance: a question's answer, a decision's content (what, the options, why), a
    /// risk's response — an agent's answer or response only once the verifier says `supports`.
    pub description: Option<String>,
    /// Who took a decision.
    pub decided_by: Option<String>,
    /// A risk's likelihood and impact: low | med | high.
    pub likelihood: Option<String>,
    pub impact: Option<String>,
    /// Who does it (`taskmodes`): `claude` — Claude can, with `prompt` (what to hand back) and a `skill`; `manual` —
    /// the user, with `how`. Only the user's choice is here; Echo's guess is kept beside the ledger.
    pub mode: Option<String>,
    pub how: Option<String>,
    pub prompt: Option<String>,
    pub skill: Option<String>,
    #[serde(skip)]
    unlinked: Vec<String>,
    pub created: Option<String>,
    pub created_in: Option<String>,
    pub creator: Option<String>,
    pub by: String,
    pub children: Vec<String>,
    pub derived: Option<String>,
    pub waiting_on: Vec<String>,
    pub blocks: Vec<String>,
    /// Its links to items in other threads (#20): `{thread, node, kind, master: here | there}`.
    pub xrefs: Vec<Value>,
    /// A mirror (`same`, the master in another thread): that item — `{thread, node, title, history}`; its
    /// state, owners and due shown here are the master's (`fold_linked`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mirror_of: Option<Value>,
    /// The master of a `same` pair: the threads that raised it too (`{thread, node, title}`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub also_in: Vec<Value>,
    /// Open items in other threads this one waits on (`depends_on`): `{thread, node, title, owner, state}`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub xwaits: Vec<Value>,
}

/// The valid links of a value (an older or hand-written event may hold anything).
fn links_of(v: &Value) -> Vec<Value> {
    v.as_array()
        .map(|a| a.iter().filter(|l| link_ok(l)).take(MAX_LINKS).map(|l| json!({"title": l["title"], "url": l["url"]})).collect())
        .unwrap_or_default()
}

fn link_ok(l: &Value) -> bool {
    l["title"].as_str().is_some_and(|t| !t.trim().is_empty() && t.chars().count() <= 200)
        && l["url"].as_str().is_some_and(|u| (u.starts_with("https://") || crate::links::Link::parse(u).is_some()) && u.len() <= 2000 && !u.chars().any(char::is_whitespace))
}

/// A description as kept: trimmed, empty = none, cut at `MAX_DESC` characters.
fn desc_of(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|t| !t.is_empty()).map(|t| t.chars().take(MAX_DESC).collect())
}

fn level_of(v: &Value) -> Option<String> {
    v.as_str().filter(|l| LEVELS.contains(l)).map(str::to_owned)
}

/// A question with an answer, or closed as answered.
pub fn answered(n: &Node) -> bool {
    n.kind == "question" && (n.description.is_some() || n.state == "done")
}

/// A record still to act on: a question without an answer, a risk not closed (with a response it is
/// handled, still open), any other kind not closed.
pub fn record_open(n: &Node) -> bool {
    !closed(&n.state) && !answered(n)
}

fn check_links(v: &Value) -> Result<()> {
    if !(v.is_null() || v.as_array().is_some_and(|a| a.len() <= MAX_LINKS && a.iter().all(link_ok))) {
        bail!("links is a list of at most {MAX_LINKS} {{title, url}}, url https:// or an echo:// link");
    }
    Ok(())
}

fn s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn node_of(e: &Value) -> Node {
    let f = &e["fields"];
    Node {
        id: s(&e["id"]).unwrap_or_default(),
        kind: s(&e["kind"]).unwrap_or_default(),
        title: s(&f["title"]).unwrap_or_default(),
        parent: s(&f["parent"]),
        task_type: s(&f["type"]),
        internal_owner: s(&f["internal_owner"]),
        client_owner: s(&f["client_owner"]),
        delegated_by: s(&f["delegated_by"]),
        client_owed: f["client_owed"] == true,
        ac: f["ac"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).map(|t| Ac { text: t.into(), met: false, met_by: None, source: s(&e["by"]), human: false, dropped: false }).collect()).unwrap_or_default(),
        depends_on: f["depends_on"].as_array().map(|a| a.iter().filter_map(s).collect()).unwrap_or_default(),
        state: "open".into(),
        due: None,
        due_history: vec![],
        history: vec![],
        pinned: vec![],
        suggestions: vec![],
        merged_into: None,
        waiting_for: None,
        column: None,
        links: links_of(&f["links"]),
        description: desc_of(&f["description"]),
        decided_by: s(&f["decided_by"]),
        likelihood: level_of(&f["likelihood"]),
        impact: level_of(&f["impact"]),
        mode: None,
        how: None,
        prompt: None,
        skill: None,
        unlinked: vec![],
        created: s(&e["date"]),
        created_in: s(&e["evidence"]),
        creator: s(&e["speaker"]),
        by: s(&e["by"]).unwrap_or_else(|| "agent".into()),
        children: vec![],
        derived: None,
        waiting_on: vec![],
        blocks: vec![],
        xrefs: vec![],
        mirror_of: None,
        also_in: vec![],
        xwaits: vec![],
    }
}

fn needs_verdict(e: &Value) -> bool {
    e["type"] == "agent" && (VERIFIED_OPS.contains(&e["op"].as_str().unwrap_or("")) || (e["op"] == "status" && STATUS.contains(&e["effect"].as_str().unwrap_or(""))))
}

fn chase(n: &BTreeMap<String, Node>, id: &str) -> String {
    let mut seen = BTreeSet::new();
    let mut i = id.to_string();
    while let Some(m) = n.get(&i).and_then(|x| x.merged_into.clone()) {
        if !seen.insert(i.clone()) {
            break;
        }
        i = m;
    }
    i
}

/// Set a field unless the user pinned it (a user's own edit pins it).
fn settable(n: &mut Node, field: &str, human: bool) -> bool {
    if human {
        if !n.pinned.iter().any(|p| p == field) {
            n.pinned.push(field.into());
        }
        true
    } else {
        !n.pinned.iter().any(|p| p == field)
    }
}

fn set_field(n: &mut Node, field: &str, v: &Value, human: bool) -> bool {
    if !settable(n, field, human) {
        return false;
    }
    match field {
        "title" => n.title = s(v).unwrap_or_default(),
        "internal_owner" => n.internal_owner = s(v),
        "client_owner" => n.client_owner = s(v),
        "delegated_by" => n.delegated_by = s(v),
        "state" => n.state = s(v).unwrap_or_else(|| "open".into()),
        "type" => n.task_type = s(v),
        "client_owed" => n.client_owed = v == true,
        "waiting_for" => n.waiting_for = (!v.is_null()).then(|| v.clone()),
        "due" => n.due = s(v),
        "column" => n.column = s(v),
        "links" => n.links = links_of(v),
        "description" => n.description = desc_of(v),
        "decided_by" => n.decided_by = s(v),
        "likelihood" => n.likelihood = level_of(v),
        "impact" => n.impact = level_of(v),
        "mode" => n.mode = v.as_str().filter(|m| crate::taskmodes::MODES.contains(m)).map(str::to_owned),
        "how" | "prompt" | "skill" => {
            let t = v.as_str().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned);
            match field {
                "how" => n.how = t,
                "prompt" => n.prompt = t,
                _ => n.skill = t,
            }
        }
        _ => return false,
    }
    true
}

pub struct Tree {
    pub nodes: BTreeMap<String, Node>,
    pub notes: BTreeMap<String, Vec<Value>>,
    pub order: Vec<String>,
    pub evidence: BTreeMap<String, Value>,
}

/// The tree as the events say, up to the last evidence processed.
pub fn fold(l: &Ledger) -> Result<Tree> {
    fold_events(&l.events()?, l.evidence())
}

fn fold_events(evs: &[Value], ev: BTreeMap<String, Value>) -> Result<Tree> {
    let ord = order(&ev);
    let pos: BTreeMap<&str, usize> = ord.iter().enumerate().map(|(i, c)| (c.as_str(), i)).collect();
    let mut seen: BTreeSet<String> = evs.iter().filter(|e| e["by"] == "agent").filter_map(|e| s(&e["evidence"])).collect();
    seen.extend(processed(evs));
    let lim = seen.iter().filter_map(|c| pos.get(c.as_str()).copied()).max().map(|x| x as i64).unwrap_or(-1);
    let verdict: BTreeMap<String, String> = evs.iter().filter(|e| e["type"] == "verdict").filter_map(|e| Some((s(&e["mid"])?, s(&e["verdict"])?))).collect();
    let undone: BTreeSet<String> = evs.iter().filter(|e| e["type"] == "human" && e["op"] == "undo").filter_map(|e| s(&e["target"])).collect();
    let decided: BTreeMap<String, String> = evs
        .iter()
        .filter(|e| e["type"] == "human" && (e["op"] == "accept" || e["op"] == "reject") && !undone.contains(e["mid"].as_str().unwrap_or("")))
        .filter_map(|e| Some((s(&e["target"])?, s(&e["op"])?)))
        .collect();
    let mut n: BTreeMap<String, Node> = BTreeMap::new();
    let mut notes: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let hist = |node: &mut Node, e: &Value, applied: bool, extra: Value| {
        let mut h = serde_json::Map::new();
        for k in HIST_KEYS {
            if let Some(v) = e.get(k).filter(|v| !v.is_null()) {
                h.insert(k.into(), v.clone());
            }
        }
        h.insert("applied".into(), json!(applied));
        h.insert("verdict".into(), e["mid"].as_str().and_then(|m| verdict.get(m)).map(|v| json!(v)).unwrap_or(Value::Null));
        if let Value::Object(x) = extra {
            h.extend(x);
        }
        node.history.push(Value::Object(h));
    };
    for e in evs {
        let evid = e["evidence"].as_str();
        if let Some(c) = evid {
            if pos.get(c).map(|p| *p as i64 > lim).unwrap_or(e["by"] != "human") {
                continue;
            }
        }
        if e["mid"].as_str().is_some_and(|m| undone.contains(m)) {
            continue;
        }
        let human = e["by"] == "human";
        match e["type"].as_str().unwrap_or("") {
            "create" => {
                let mut x = node_of(e);
                if let Some(d) = s(&e["fields"]["due"]) {
                    x.due_history.push(json!({"due": d, "date": e["date"], "ts": e["ts"], "evidence": e["evidence"], "text": e["fields"]["due_text"]}));
                }
                if human {
                    x.pinned = vec!["title".into()];
                }
                hist(&mut x, e, true, json!({"op": "create"}));
                n.insert(x.id.clone(), x);
            }
            "note" => notes.entry(s(&e["node"]).unwrap_or_default()).or_default().push(e.clone()),
            "agent" => {
                let id = chase(&n, e["node"].as_str().unwrap_or(""));
                if !n.contains_key(&id) {
                    continue;
                }
                let op = e["op"].as_str().unwrap_or("");
                let mut ok = !needs_verdict(e) || e["mid"].as_str().and_then(|m| verdict.get(m)).map(String::as_str) == Some("supports");
                match op {
                    "status" => {
                        let x = n.get_mut(&id).unwrap();
                        if ok && !["merged", "deleted"].contains(&x.state.as_str()) {
                            ok = apply_status(x, e);
                        }
                        hist(x, e, ok, json!({}));
                    }
                    "revise" => {
                        let x = n.get_mut(&id).unwrap();
                        let t_ok = if e["title"].is_string() { set_field(x, "title", &e["title"], false) } else { true };
                        let d_ok = if e["description"].is_string() { set_field(x, "description", &e["description"], false) } else { true };
                        let added: Vec<Ac> = e["ac_add"].as_array().map(|a| a.iter().filter_map(|t| t.as_str()).map(|t| Ac { text: t.into(), met: false, met_by: None, source: Some("agent".into()), human: false, dropped: false }).collect()).unwrap_or_default();
                        let any = !added.is_empty();
                        x.ac.extend(added);
                        // Applied when any part of it was (a pinned description doesn't undo a new title).
                        hist(x, e, (e["title"].is_string() && t_ok) || (e["description"].is_string() && d_ok) || any, json!({}));
                    }
                    "ac_met" => {
                        let x = n.get_mut(&id).unwrap();
                        let vis: Vec<usize> = (0..x.ac.len()).filter(|i| !x.ac[*i].dropped).collect();
                        let k = match e["ac_text"].as_str() {
                            Some(t) => vis.iter().position(|i| x.ac[*i].text == t),
                            None => e["ac"].as_u64().and_then(|a| (a as usize).checked_sub(1)),
                        };
                        if let Some(&i) = k.and_then(|k| vis.get(k)) {
                            if ok && !x.ac[i].human {
                                x.ac[i].met = true;
                                x.ac[i].met_by = s(&e["mid"]);
                            }
                        }
                        hist(x, e, ok, json!({}));
                    }
                    "link" => {
                        let d = chase(&n, e["depends_on"].as_str().unwrap_or(""));
                        let exists = n.contains_key(&d);
                        let x = n.get_mut(&id).unwrap();
                        if ok && exists && !x.depends_on.contains(&d) && !x.unlinked.contains(&d) && d != x.id {
                            x.depends_on.push(d);
                        }
                        hist(x, e, ok, json!({}));
                    }
                    "waiting" => {
                        let x = n.get_mut(&id).unwrap();
                        if ok {
                            let v = if e["what"].as_str().is_some_and(|w| !w.is_empty()) { json!({"who": e["who"], "what": e["what"], "since": e["date"], "evidence": e["evidence"]}) } else { Value::Null };
                            ok = set_field(x, "waiting_for", &v, false);
                        }
                        hist(x, e, ok, json!({}));
                    }
                    "answer" => {
                        let x = n.get_mut(&id).unwrap();
                        if ok && !["merged", "deleted"].contains(&x.state.as_str()) {
                            ok = set_field(x, "description", &e["text"], false);
                            if ok && x.kind == "question" && !x.pinned.iter().any(|p| p == "state") {
                                x.state = "done".into();
                            }
                            if ok && x.kind == "risk" {
                                for k in ["internal_owner", "likelihood", "impact"] {
                                    if e[k].is_string() {
                                        set_field(x, k, &e[k], false);
                                    }
                                }
                            }
                        }
                        hist(x, e, ok, json!({}));
                    }
                    "suggest_move" | "suggest_merge" => {
                        let st = e["mid"].as_str().and_then(|m| decided.get(m)).cloned().unwrap_or_else(|| "pending".into());
                        let to = s(&e["parent"]).or_else(|| s(&e["into"])).unwrap_or_default();
                        n.get_mut(&id).unwrap().suggestions.push(json!({"mid": e["mid"], "op": op, "to": to, "quote": e["quote"], "note": e["note"], "status": st, "evidence": e["evidence"], "date": e["date"]}));
                        if st == "accept" {
                            structural(&mut n, &id, &op[8..], &to);
                        }
                        let x = n.get_mut(&id).unwrap();
                        hist(x, e, st == "accept", json!({"status": st}));
                    }
                    _ => {}
                }
            }
            "human" => {
                let op = e["op"].as_str().unwrap_or("");
                if ["undo", "accept", "reject"].contains(&op) {
                    continue;
                }
                let id = chase(&n, e["node"].as_str().unwrap_or(""));
                if !n.contains_key(&id) {
                    continue;
                }
                match op {
                    "edit" => {
                        if let Some(sets) = e["set"].as_object() {
                            for (k, v) in sets {
                                if !HUMAN_FIELDS.contains(&k.as_str()) {
                                    continue;
                                }
                                if k == "parent" {
                                    if let Some(to) = v.as_str() {
                                        structural(&mut n, &id, "move", to);
                                    }
                                    continue;
                                }
                                let x = n.get_mut(&id).unwrap();
                                if k == "due" {
                                    x.due_history.push(json!({"due": v, "date": e["date"], "ts": null, "evidence": e["evidence"], "text": "set by you"}));
                                    settable(x, "due", true);
                                } else if k == "waiting_for" && v.is_object() {
                                    let mut w = v.clone();
                                    if w["since"].is_null() {
                                        w["since"] = e["date"].clone();
                                    }
                                    set_field(x, k, &w, true);
                                } else {
                                    set_field(x, k, v, true);
                                }
                            }
                        }
                    }
                    "ac_add" => n.get_mut(&id).unwrap().ac.push(Ac { text: s(&e["text"]).unwrap_or_default(), met: false, met_by: None, source: Some("human".into()), human: false, dropped: false }),
                    "ac_set" => {
                        let x = n.get_mut(&id).unwrap();
                        let vis: Vec<usize> = (0..x.ac.len()).filter(|i| !x.ac[*i].dropped).collect();
                        if let Some(&i) = e["ac"].as_u64().and_then(|a| (a as usize).checked_sub(1)).and_then(|k| vis.get(k)) {
                            if let Some(t) = e["text"].as_str() {
                                x.ac[i].text = t.into();
                            }
                            if let Some(m) = e["met"].as_bool() {
                                x.ac[i].met = m;
                                x.ac[i].human = true;
                            }
                            if e["drop"] == true {
                                x.ac[i].dropped = true;
                            }
                        }
                    }
                    "link" => {
                        let d = chase(&n, e["depends_on"].as_str().unwrap_or(""));
                        let exists = n.contains_key(&d);
                        let x = n.get_mut(&id).unwrap();
                        if exists && !x.depends_on.contains(&d) && d != x.id {
                            x.depends_on.push(d.clone());
                        }
                        x.unlinked.retain(|u| u != &d);
                    }
                    "unlink" => {
                        let d = s(&e["depends_on"]).unwrap_or_default();
                        let x = n.get_mut(&id).unwrap();
                        x.depends_on.retain(|u| u != &d);
                        x.unlinked.push(d);
                    }
                    "merge" => structural(&mut n, &id, "merge", e["into"].as_str().unwrap_or("")),
                    "xref" | "unxref" => {
                        let (th, tg) = (s(&e["thread"]).unwrap_or_default(), s(&e["target"]).unwrap_or_default());
                        let x = n.get_mut(&id).unwrap();
                        x.xrefs.retain(|r| !(r["thread"] == th.as_str() && r["node"] == tg.as_str()));
                        if op == "xref" {
                            x.xrefs.push(json!({"thread": th, "node": tg, "kind": e["kind"], "master": e["master"]}));
                        }
                    }
                    "delete" => {
                        let x = n.get_mut(&id).unwrap();
                        x.state = "deleted".into();
                        x.pinned.push("state".into());
                    }
                    _ => {}
                }
                let x = n.get_mut(&id).unwrap();
                hist(x, e, true, json!({}));
            }
            _ => {}
        }
    }
    let ids: Vec<String> = n.keys().cloned().collect();
    for id in &ids {
        let deps: Vec<String> = n[id].depends_on.iter().map(|d| chase(&n, d)).collect();
        let x = n.get_mut(id).unwrap();
        let mut uniq: Vec<String> = vec![];
        for d in deps {
            if &d != id && !uniq.contains(&d) {
                uniq.push(d);
            }
        }
        x.depends_on = uniq;
        x.ac.retain(|a| !a.dropped);
        x.due = x.due_history.last().and_then(|h| s(&h["due"]));
    }
    derive(&mut n);
    let lim_n = (lim + 1).max(0) as usize;
    Ok(Tree { nodes: n, notes, order: ord.into_iter().take(lim_n).collect(), evidence: ev })
}

fn apply_status(x: &mut Node, e: &Value) -> bool {
    let eff = e["effect"].as_str().unwrap_or("");
    if eff == "mention" {
        return false;
    }
    let pinned = |x: &Node, f: &str| x.pinned.iter().any(|p| p == f);
    if pinned(x, "state") && !["due", "delayed", "reassign"].contains(&eff) {
        return false;
    }
    match eff {
        "held" | "done" => {
            x.state = "done".into();
            if !pinned(x, "waiting_for") {
                x.waiting_for = None;
            }
        }
        "progress" if x.state == "open" || x.state == "blocked" => x.state = "in_progress".into(),
        "blocked" | "cancelled" => x.state = eff.into(),
        "reopen" if x.state == "done" || x.state == "cancelled" => x.state = "in_progress".into(),
        "delayed" => {
            if pinned(x, "due") {
                return false;
            }
            x.due_history.push(json!({"due": null, "date": e["date"], "ts": e["ts"], "evidence": e["evidence"], "text": e["due_text"].as_str().unwrap_or("delayed, no new date")}));
        }
        // A hand-over is one change: if the user set an owner it names, none of it is applied (a Handed over by they set is kept).
        "reassign" => {
            let fields: Vec<_> = ["internal_owner", "client_owner", "delegated_by"].into_iter().filter(|k| e[*k].is_string()).collect();
            if fields.iter().any(|k| *k != "delegated_by" && pinned(x, k)) {
                return false;
            }
            for k in fields {
                set_field(x, k, &e[k], false);
            }
        }
        _ => {}
    }
    if eff != "delayed" && e["due"].is_string() {
        if pinned(x, "due") {
            return eff != "due";
        }
        if x.due_history.last().map(|h| &h["due"]) != Some(&e["due"]) {
            x.due_history.push(json!({"due": e["due"], "date": e["date"], "ts": e["ts"], "evidence": e["evidence"], "text": e["due_text"]}));
        }
    }
    true
}

fn structural(n: &mut BTreeMap<String, Node>, id: &str, op: &str, to: &str) {
    let to = chase(n, to);
    if !n.contains_key(&to) || to == id {
        return;
    }
    let tk = n[&to].kind.clone();
    if op == "move" {
        let x = n.get_mut(id).unwrap();
        if parent_ok(&x.kind, Some(&tk)) && settable(x, "parent", true) {
            x.parent = Some(to);
        }
    } else if op == "merge" && tk == n[id].kind {
        let src = n[id].clone();
        {
            let x = n.get_mut(id).unwrap();
            x.merged_into = Some(to.clone());
            x.state = "merged".into();
        }
        let t = n.get_mut(&to).unwrap();
        t.history.extend(src.history.iter().map(|h| {
            let mut h = h.clone();
            h["merged_from"] = json!(src.id);
            h
        }));
        t.history.sort_by(|a, b| (a["date"].as_str().unwrap_or(""), a["ts"].as_str().unwrap_or("")).cmp(&(b["date"].as_str().unwrap_or(""), b["ts"].as_str().unwrap_or(""))));
        for a in src.ac {
            if !t.ac.iter().any(|x| x.text == a.text) {
                t.ac.push(a);
            }
        }
        for d in src.depends_on {
            if !t.depends_on.contains(&d) && d != to {
                t.depends_on.push(d);
            }
        }
        for c in n.values_mut() {
            if c.parent.as_deref() == Some(id) {
                c.parent = Some(to.clone());
            }
        }
    }
}

fn closed(state: &str) -> bool {
    CLOSED.contains(&state)
}

fn derive(n: &mut BTreeMap<String, Node>) {
    let mut kids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for x in n.values() {
        if x.state != "merged" && x.state != "deleted" {
            if let Some(p) = &x.parent {
                kids.entry(p.clone()).or_default().push(x.id.clone());
            }
        }
    }
    for x in n.values_mut() {
        x.children = kids.remove(&x.id).unwrap_or_default();
        x.derived = None;
    }
    let ids: Vec<String> = n.keys().cloned().collect();
    for id in &ids {
        let x = &n[id];
        if x.kind != "story" || closed(&x.state) || x.pinned.iter().any(|p| p == "state") {
            continue;
        }
        let moved = x.children.iter().any(|c| n[c].kind == "task" && (n[c].state == "done" || n[c].state == "in_progress"));
        let all_met = !x.ac.is_empty() && x.ac.iter().all(|a| a.met);
        let x = n.get_mut(id).unwrap();
        if all_met {
            x.state = "done".into();
            x.derived = Some("all acceptance criteria met".into());
        } else if x.state == "open" && moved {
            x.state = "in_progress".into();
            x.derived = Some("a task moved".into());
        }
    }
    for id in &ids {
        let x = &n[id];
        let mut w: Vec<String> = x.depends_on.iter().filter(|d| n.get(*d).is_some_and(|d| !closed(&d.state))).cloned().collect();
        if x.kind == "story" {
            for c in &x.children {
                if n[c].kind == "task" && n[c].client_owed && !closed(&n[c].state) && !w.contains(c) {
                    w.push(c.clone());
                }
            }
        }
        n.get_mut(id).unwrap().waiting_on = w;
    }
    for id in &ids {
        let mut b: Vec<String> = n.values().filter(|m| m.waiting_on.contains(id) && !closed(&m.state)).map(|m| m.id.clone()).collect();
        b.sort();
        n.get_mut(id).unwrap().blocks = b;
    }
}

fn path(n: &BTreeMap<String, Node>, id: &str) -> String {
    let mut out = vec![];
    let mut i = Some(id.to_string());
    let mut guard = 0;
    while let Some(x) = i.and_then(|i| n.get(&i)) {
        out.push(x.title.clone());
        i = x.parent.clone();
        guard += 1;
        if guard > 20 {
            break;
        }
    }
    out.reverse();
    out.join(" › ")
}

// ── context: what the extractor sees ─────────────────────────────────────────────────────────────

pub fn context(project: &str) -> Result<Value> {
    let p = crate::projects::load(project)?;
    let t = fold_linked(project)?;
    let n = &t.nodes;
    fn view(n: &BTreeMap<String, Node>, x: &Node) -> Value {
        if closed(&x.state) {
            return json!({"id": x.id, "title": x.title, "state": x.state});
        }
        let mut d = json!({"id": x.id, "title": x.title, "state": x.state});
        if let Some(c) = &x.created {
            d["since"] = json!(c);
        }
        if x.kind == "story" {
            d["ac"] = json!(x.ac.iter().enumerate().map(|(i, a)| format!("{}. {} {}", i + 1, if a.met { "[x]" } else { "[ ]" }, a.text)).collect::<Vec<_>>());
        }
        if x.kind == "task" {
            d["type"] = json!(x.task_type);
        }
        if let Some(t) = &x.description {
            d["description"] = json!(if t.chars().count() > 400 { format!("{}…", t.chars().take(400).collect::<String>()) } else { t.clone() });
        }
        for (k, v) in [("internal_owner", &x.internal_owner), ("client_owner", &x.client_owner), ("delegated_by", &x.delegated_by), ("due", &x.due)] {
            if let Some(v) = v {
                d[k] = json!(v);
            }
        }
        if x.client_owed {
            d["client_owed"] = json!(true);
        }
        if !x.depends_on.is_empty() {
            d["waits_for"] = json!(x.depends_on.iter().filter_map(|i| n.get(i).map(|y| format!("{i} {}", y.title))).collect::<Vec<_>>());
        }
        if !x.blocks.is_empty() {
            d["holds_up"] = json!(x.blocks.iter().filter_map(|i| n.get(i).map(|y| format!("{i} {}", y.title))).collect::<Vec<_>>());
        }
        if let Some(w) = &x.waiting_for {
            d["waiting_for"] = json!(format!("{}: {} (since {})", w["who"].as_str().unwrap_or("?"), w["what"].as_str().unwrap_or(""), w["since"].as_str().unwrap_or("")));
        }
        if !x.pinned.is_empty() {
            d["pinned_by_human"] = json!(x.pinned);
        }
        // One piece of work raised in two threads (#20): known here — don't create it again; what is said
        // about it here still counts (a status change goes to it).
        if let Some(m) = &x.mirror_of {
            d["also_tracked_in"] = json!(format!("{} {} (thread {}) — the same work; its state is shown from there", m["node"].as_str().unwrap_or(""), m["title"].as_str().unwrap_or(""), m["thread"].as_str().unwrap_or("")));
        }
        if !x.also_in.is_empty() {
            d["also_raised_in"] = json!(x.also_in.iter().map(|a| format!("{} {} (thread {})", a["node"].as_str().unwrap_or(""), a["title"].as_str().unwrap_or(""), a["thread"].as_str().unwrap_or(""))).collect::<Vec<_>>());
        }
        if !x.xwaits.is_empty() {
            d["waits_for_other_threads"] = json!(x.xwaits.iter().map(|w| format!("{} {} (thread {})", w["node"].as_str().unwrap_or(""), w["title"].as_str().unwrap_or(""), w["thread"].as_str().unwrap_or(""))).collect::<Vec<_>>());
        }
        let pend: Vec<String> = x.history.iter().filter(|h| h["type"] == "agent" && needs_verdict(h) && h["verdict"].is_null()).map(|h| format!("{}@{}", h["effect"].as_str().or(h["op"].as_str()).unwrap_or(""), h["date"].as_str().unwrap_or(""))).collect();
        if !pend.is_empty() {
            d["unverified_claims"] = json!(pend[pend.len().saturating_sub(3)..]);
        }
        let sug: Vec<String> = x.suggestions.iter().filter(|s| s["status"] == "pending").map(|s| format!("{} -> {}", &s["op"].as_str().unwrap_or("suggest_?")[8..], s["to"].as_str().unwrap_or(""))).collect();
        if !sug.is_empty() {
            d["pending_suggestions"] = json!(sug);
        }
        if let Some(h) = x.history.iter().rev().find(|h| h["evidence"].is_string()) {
            d["last"] = h["date"].clone();
        }
        for (k, name) in [("story", "stories"), ("task", "tasks")] {
            let xs: Vec<Value> = x.children.iter().filter(|c| n[*c].kind == k).map(|c| view(n, &n[c])).collect();
            if !xs.is_empty() {
                d[name] = json!(xs);
            }
        }
        for (k, name) in [("risk", "risks"), ("decision", "decisions"), ("question", "questions")] {
            let xs: Vec<String> = x.children.iter().filter(|c| n[*c].kind == k).map(|c| record_line(&n[c])).collect();
            if !xs.is_empty() {
                d[name] = json!(xs);
            }
        }
        d
    }
    let epics: Vec<&Node> = n.values().filter(|x| x.kind == "epic" && x.state != "merged" && x.state != "deleted").collect();
    let clients = crate::projects::clients().unwrap_or_default();
    let client = p.client.as_deref().and_then(|c| clients.iter().find(|x| x.id == c));
    Ok(json!({
        "project": p.name, "stage": p.stage.id(), "client": client.map(|c| &c.name),
        "client_domains": client.map(|c| &c.domains), "notes": p.notes,
        // About me (the user's `me.md`): who "Me" is and how they work — defaults the words can override.
        "me_profile": crate::calls::me_profile(),
        "people": p.people,
        "evidence_processed": {"count": t.order.len(), "latest": t.order.iter().rev().take(5).rev().map(|c| format!("{} {}", t.evidence[c]["date"].as_str().unwrap_or(""), t.evidence[c]["title"].as_str().unwrap_or(""))).collect::<Vec<_>>()},
        "epics": epics.iter().filter(|e| !e.children.is_empty()).map(|e| view(n, e)).collect::<Vec<_>>(),
        "epics_without_work": epics.iter().filter(|e| e.children.is_empty()).map(|e| format!("{} {}", e.id, e.title)).collect::<Vec<_>>(),
    }))
}

/// A record as the extractor reads it: its id and title, then what it holds — a question's answer, a
/// decision's content and who took it, a risk's response, owner, likelihood and impact.
fn record_line(r: &Node) -> String {
    let cut = |t: &str, k: usize| if t.chars().count() > k { format!("{}…", t.chars().take(k).collect::<String>()) } else { t.to_string() };
    let mut s = format!("{} {}", r.id, cut(&r.title, 90));
    let what = r.description.as_deref().map(|d| cut(&d.split_whitespace().collect::<Vec<_>>().join(" "), 160));
    let mut more = vec![];
    match r.kind.as_str() {
        "question" => more.push(match &what {
            Some(a) => format!("answered: {a}"),
            None if r.state == "done" => "answered".into(),
            None => "no answer yet".into(),
        }),
        "decision" => {
            more.extend(what.map(|d| format!("decided: {d}")));
            more.extend(r.decided_by.as_ref().map(|b| format!("by {b}")));
        }
        "risk" => {
            more.push(what.map(|d| format!("response: {d}")).unwrap_or_else(|| "no response yet".into()));
            more.extend(r.internal_owner.as_ref().map(|o| format!("owner {o}")));
            more.extend(r.likelihood.as_ref().map(|l| format!("likelihood {l}")));
            more.extend(r.impact.as_ref().map(|i| format!("impact {i}")));
        }
        _ => {}
    }
    if closed(&r.state) && !(r.kind == "question" && r.state == "done") {
        more.push("closed".into());
    }
    if !r.pinned.is_empty() {
        more.push(format!("pinned_by_human: {}", r.pinned.join(", ")));
    }
    if !more.is_empty() {
        s.push_str(&format!(" ({})", more.join("; ")));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_day_is_one_item_of_turns_at_local_time() {
        let m = |id: &str, at: &str, who: &str, html: &str, files: &[&str]| json!({"id": id, "createdDateTime": at, "from": {"user": {"id": "u1", "displayName": who}},
            "body": {"content": html}, "attachments": files.iter().map(|f| json!({"name": f})).collect::<Vec<_>>()});
        let a = m("1", "2026-10-30T07:02:04Z", "Dana", "<p>The test server needs a <b>firewall rule</b></p>", &[]);
        let b = m("2", "2026-10-30T08:00:00Z", "Sam", "", &["plan.pptx"]);
        let c = m("3", "2026-10-30T08:01:00Z", "Sam", "", &[]);
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let item = chat_item("19:abc@thread.v2", "Weekly", day, &[&a, &b, &c]).unwrap();
        assert!(item["id"].as_str().unwrap().starts_with("chat-") && item["id"].as_str().unwrap().ends_with("-2026-10-30"));
        assert_eq!(item["kind"], "chat");
        let turns = item["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 2, "an empty message without files is left out");
        assert_eq!(turns[0]["text"], "The test server needs a firewall rule");
        assert_eq!(turns[1]["text"], "(shared plan.pptx)");
        assert_eq!(turns[0]["ts"].as_str().unwrap().len(), 8);
        assert!(item["text"].as_str().unwrap().contains("Sam shared plan.pptx"));
        assert!(chat_item("19:abc@thread.v2", "Weekly", day, &[&c]).is_none());
    }

    #[test]
    fn norm_and_dates() {
        assert_eq!(norm("  “Can you SEND it,” she said—this week? "), "'can you send it ' she said this week");
        let wed = "2026-10-28"; // a Wednesday
        assert_eq!(resolve_due(Some("by Friday"), wed).as_deref(), Some("2026-10-30"));
        assert_eq!(resolve_due(Some("next Friday"), wed).as_deref(), Some("2026-11-06"));
        assert_eq!(resolve_due(Some("tomorrow"), "2026-10-30").as_deref(), Some("2026-11-02"), "a Friday's tomorrow is Monday");
        assert_eq!(resolve_due(Some("end of the week"), wed).as_deref(), Some("2026-10-30"));
        assert_eq!(resolve_due(Some("next week"), wed).as_deref(), Some("2026-11-06"));
        assert_eq!(resolve_due(Some("middle of next week"), wed).as_deref(), Some("2026-11-04"));
        assert_eq!(resolve_due(Some("the 12th"), wed).as_deref(), Some("2026-11-12"));
        assert_eq!(resolve_due(Some("24th of November"), wed).as_deref(), Some("2026-11-24"));
        assert_eq!(resolve_due(Some("Jan 5"), "2026-12-10").as_deref(), Some("2027-01-05"));
        assert_eq!(resolve_due(Some("2026-11-24"), wed).as_deref(), Some("2026-11-24"));
        assert_eq!(resolve_due(Some("Decisions doc"), wed), None, "a word that starts like a month is no date");
        assert_eq!(resolve_due(Some("soon"), wed), None);
        assert_eq!(resolve_due(Some("by the eighteenth"), wed).as_deref(), Some("2026-11-18"));
        assert_eq!(resolve_due(Some("December eighteenth"), wed).as_deref(), Some("2026-12-18"));
        assert_eq!(resolve_due(Some("the twenty-first of December"), wed).as_deref(), Some("2026-12-21"));
        assert_eq!(resolve_due(Some("on the first"), wed).as_deref(), Some("2026-11-01"));
        assert_eq!(resolve_due(Some("the second phase"), wed).as_deref(), Some("2026-11-02"), "taska's rule; the prompt asks for dates");
    }

    fn setup(name: &str) -> (crate::paths::testenv::Env, Ledger, String) {
        let (env, _root) = crate::paths::testenv::folders(name);
        let p = crate::projects::create("Aurora", None, None).unwrap();
        crate::projects::update(&p.id, |p| {
            p.people = vec![crate::projects::Member { name: "Dana Kowal".into(), email: Some("dana@northwind.example".into()), role: None, ..Default::default() }];
            Ok(())
        })
        .unwrap();
        let l = Ledger::of(&p.id).unwrap();
        fs::create_dir_all(l.dir.join("evidence")).unwrap();
        let item = json!({"id": "2026-10-28_110000", "kind": "meeting", "source": "call", "at": "2026-10-28T11:00:00+02:00", "date": "2026-10-28", "title": "Aurora weekly",
            "turns": [
                {"ts": "00:12:05", "speaker": "Dana Kowal", "text": "Then SSO has to land first, let's say the pilot moves to the 24th of November."},
                {"ts": "00:12:21", "speaker": "Ravi Mehta", "text": "Our security review needs the data-flow diagram, can you send it this week?"},
                {"ts": "00:13:02", "speaker": "Me", "text": "I can have the SAML config ready by Friday."},
                {"ts": "00:20:10", "speaker": "Dana Kowal", "text": "The Berlin workshop happened yesterday and it went well."},
            ], "text": ""});
        fs::write(l.dir.join("evidence/2026-10-28_110000.json"), serde_json::to_vec(&item).unwrap()).unwrap();
        ensure_first_epic(&l).unwrap();
        (env, l, p.id)
    }

    #[test]
    fn what_a_call_changed_and_the_tasks_its_action_items_became() {
        let (_env, _l, id) = setup("ledger-call-changes");
        let ev = "2026-10-28_110000";
        assert_eq!(call_changes(&id, ev).unwrap()["queued"], true, "not read yet");
        apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "SSO for the Berlin pilot", "ac": ["SAML works", "Review passed"], "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "due_text": "Friday", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "status", "node": "E01", "effect": "mention", "quote": "Then SSO has to land first"}),
        ]).unwrap();
        mark_processed(&id, ev, "").unwrap();
        let c = call_changes(&id, ev).unwrap();
        // Still to do from that call: its open tasks (not the story, not a mention), each once, by its link.
        let todo = open_tasks_from(std::slice::from_ref(&id), ev);
        assert_eq!(todo.len(), 1, "{todo:?}");
        assert_eq!((todo[0]["task"].as_str(), todo[0]["link"].as_str()), (Some("SAML config"), Some(format!("echo://task/{id}/T001").as_str())));
        assert_eq!(c["read"], true);
        let ch = c["changes"].as_array().unwrap();
        assert_eq!(ch.len(), 2, "a mention changes nothing: {c}");
        assert_eq!(ch[0]["node"]["id"], "S001", "in the order said");
        assert_eq!(ch[1]["node"]["due"], "2026-10-30");

    }

    #[test]
    fn proposals_are_checked_against_the_words_and_the_rules() {
        let (_env, l, id) = setup("ledger-apply");
        let ev = "2026-10-28_110000";
        let r = apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "SSO for the Berlin pilot", "ac": ["SAML works for Berlin users", "Security review passed"], "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "due_text": "Friday", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "ref": "t2", "kind": "task", "parent": "s1", "title": "Data-flow diagram to Ravi", "type": "action", "due_text": "this week", "quote": "can you send it this week"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Workshop", "type": "session", "quote": "The Berlin workshop happened yesterday"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Invented", "type": "action", "internal_owner": "Me", "quote": "nobody said these words at all"}),
            json!({"op": "create", "kind": "story", "parent": "E01", "title": "No criteria", "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Chore", "type": "action", "quote": "Our security review needs"}),
            json!({"op": "create", "kind": "decision", "parent": "s1", "title": "SSO before the pilot", "quote": "SSO has to land first, let's say"}),
        ]).unwrap();
        assert_eq!(r["events"], 5, "{r}");
        let why: Vec<&str> = r["rejected"].as_array().unwrap().iter().map(|x| x["why"].as_str().unwrap()).collect();
        assert!(why[0].contains("not verbatim"), "{why:?}");
        assert!(why[1].contains("acceptance criteria"), "{why:?}");
        assert!(why[2].contains("admission bar"), "{why:?}");
        // Re-applying the same batch adds nothing.
        assert_eq!(apply(&id, ev, &[json!({"op": "create", "kind": "story", "parent": "E01", "title": "SSO for the Berlin pilot", "ac": ["x", "y"], "quote": "Then SSO has to land first"})]).unwrap()["events"], 0);

        let t = fold(&l).unwrap().nodes;
        assert_eq!(t["T001"].due.as_deref(), Some("2026-10-30"), "Friday, said on Wednesday");
        assert_eq!(t["T001"].internal_owner.as_deref(), Some("Me"));
        assert_eq!(t["T002"].due.as_deref(), Some("2026-10-30"), "this week");
        assert_eq!(t["S001"].children, vec!["D001", "T001", "T002", "T003"]);

        // A `due` without a readable date is refused, with what to write instead.
        let r = apply(&id, ev, &[json!({"op": "status", "node": "T001", "effect": "due", "due_text": "before the break", "quote": "I can have the SAML config ready by Friday"})]).unwrap();
        assert!(r["rejected"][0]["why"].as_str().unwrap().contains("no date can be read"), "{r}");
        // A status change counts only once a verifier supports it.
        apply(&id, ev, &[json!({"op": "status", "node": "T003", "effect": "held", "quote": "The Berlin workshop happened yesterday"}),
                         json!({"op": "link", "node": "S001", "depends_on": "T002", "quote": "Our security review needs the data-flow diagram"})]).unwrap();
        assert_eq!(fold(&l).unwrap().nodes["T003"].state, "open");
        let packets = verify_prep(&id, false).unwrap();
        assert_eq!(packets.len(), 2);
        assert!(packets[0]["context"].as_array().unwrap().len() >= 2);
        let v: Vec<Value> = packets.iter().map(|p| json!({"mid": p["mid"], "verdict": "supports"})).collect();
        assert_eq!(verify_apply(&id, &v).unwrap()["verdicts"]["supports"], 2);
        let t = fold(&l).unwrap().nodes;
        assert_eq!(t["T003"].state, "done");
        assert_eq!(t["S001"].state, "in_progress", "a task moved");
        assert_eq!(t["S001"].depends_on, vec!["T002"]);
        assert_eq!(t["T002"].blocks, vec!["S001"]);
        assert!(verify_prep(&id, false).unwrap().is_empty());
    }

    #[test]
    fn a_reassign_of_an_owner_the_user_set_is_kept_out_whole() {
        let (_env, l, id) = setup("ledger-reassign-pinned");
        let ev = "2026-10-28_110000";
        apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "SSO", "ac": ["a"], "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "quote": "I can have the SAML config ready by Friday"}),
        ]).unwrap();
        human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"internal_owner": "Alex Chen"}})).unwrap();
        // A call hands it to someone else: the user's owner stays, and nothing of the reassign is applied.
        apply(&id, ev, &[json!({"op": "status", "node": "T001", "effect": "reassign", "internal_owner": "Priya Nair", "delegated_by": "Me", "quote": "can you send it this week?"})]).unwrap();
        let p = verify_prep(&id, false).unwrap();
        verify_apply(&id, &[json!({"mid": p[0]["mid"], "verdict": "supports"})]).unwrap();
        let t = fold(&l).unwrap().nodes;
        assert_eq!((t["T001"].internal_owner.as_deref(), t["T001"].delegated_by.as_deref()), (Some("Alex Chen"), None));
        assert!(t["T001"].history.iter().any(|h| h["effect"] == "reassign" && h["applied"] == false), "{:?}", t["T001"].history);
    }

    #[test]
    fn a_reassign_is_applied_when_the_user_set_no_owner_and_keeps_their_handed_over_by() {
        let (_env, l, id) = setup("ledger-reassign-free");
        let ev = "2026-10-28_110000";
        apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "SSO", "ac": ["a"], "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "quote": "I can have the SAML config ready by Friday"}),
        ]).unwrap();
        human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"delegated_by": "Dana Kowal"}})).unwrap();
        apply(&id, ev, &[json!({"op": "status", "node": "T001", "effect": "reassign", "internal_owner": "Ravi Mehta", "delegated_by": "Me", "quote": "can you send it this week?"})]).unwrap();
        let p = verify_prep(&id, false).unwrap();
        verify_apply(&id, &[json!({"mid": p[0]["mid"], "verdict": "supports"})]).unwrap();
        let t = fold(&l).unwrap().nodes;
        assert_eq!((t["T001"].internal_owner.as_deref(), t["T001"].delegated_by.as_deref()), (Some("Ravi Mehta"), Some("Dana Kowal")));
        assert!(t["T001"].history.iter().any(|h| h["effect"] == "reassign" && h["applied"] == true));
    }

    #[test]
    fn the_users_edits_win_and_undo() {
        let (_env, l, id) = setup("ledger-human");
        let ev = "2026-10-28_110000";
        apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "SSO", "ac": ["a", "b"], "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "due_text": "Friday", "quote": "I can have the SAML config ready by Friday"}),
        ]).unwrap();
        let e = human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"due": "2026-11-03", "title": "SAML for Berlin"}})).unwrap();
        // The agent moves the date again: recorded, not applied (the user's date is pinned).
        apply(&id, ev, &[json!({"op": "status", "node": "T001", "effect": "due", "due_text": "next Friday", "quote": "I can have the SAML config ready by Friday"})]).unwrap();
        let p = verify_prep(&id, false).unwrap();
        verify_apply(&id, &[json!({"mid": p[0]["mid"], "verdict": "supports"})]).unwrap();
        let t = fold(&l).unwrap().nodes;
        assert_eq!((t["T001"].due.as_deref(), t["T001"].title.as_str()), (Some("2026-11-03"), "SAML for Berlin"));
        assert!(t["T001"].pinned.contains(&"due".to_string()));
        assert!(t["T001"].history.iter().any(|h| h["effect"] == "due" && h["applied"] == false));
        // Undo the edit: the agent's dates count again.
        human_event(&id, &json!({"op": "undo", "target": e["mid"]})).unwrap();
        let t = fold(&l).unwrap().nodes;
        assert_eq!(t["T001"].title, "SAML config");
        assert_eq!(t["T001"].due.as_deref(), Some("2026-11-06"));
        // Bad edits are refused when written, never stored.
        assert!(human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"due": "soon"}})).is_err());
        // A date no work item is due on: what a date box gives while its year is half typed (#303).
        for due in ["0002-12-12", "0202-12-12", "1999-12-31", "2101-01-01"] {
            let err = human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"due": due}})).unwrap_err();
            assert!(err.to_string().contains("between 2000 and 2100"), "{due}: {err}");
        }
        assert!(human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"state": "finished"}})).is_err());
        assert!(human_event(&id, &json!({"op": "edit", "node": "T999", "set": {"title": "x"}})).is_err());
        // An undo, accept or reject of an event that isn't there is refused, and nothing is written (#127).
        let before = l.events().unwrap().len();
        for op in ["undo", "accept", "reject"] {
            let err = human_event(&id, &json!({"op": op, "target": "0000000000"})).unwrap_err();
            assert!(err.to_string().contains("unknown event 0000000000"), "{op}: {err}");
        }
        assert_eq!(l.events().unwrap().len(), before);
        // Who does it: the user's choice on the task, pinned; Echo's guess never overwrites it.
        human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"mode": "claude", "prompt": "Draft the SAML runbook.", "skill": "follow-up-mail"}})).unwrap();
        let n = fold(&Ledger::of(&id).unwrap()).unwrap().nodes["T001"].clone();
        assert_eq!((n.mode.as_deref(), n.prompt.as_deref(), n.skill.as_deref()), (Some("claude"), Some("Draft the SAML runbook."), Some("follow-up-mail")));
        assert!(n.pinned.iter().any(|f| f == "mode"));
        let guess = serde_json::from_value(json!({format!("{id}/T001"): {"mode": "manual", "how": "x"}})).unwrap();
        assert_eq!(crate::taskmodes::of(&guess, &id, &n)["by"], "you", "the user's choice over Echo's guess");
        assert!(human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"mode": "robot"}})).is_err());
        assert!(human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"skill": "Rm -rf"}})).is_err());
        let c = human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "Book the room"})).unwrap();
        assert_eq!(c["id"], "T002");
        assert!(human_event(&id, &json!({"op": "create", "kind": "task", "parent": "E01", "title": "No story"})).is_err());
        let ctx = context(&id).unwrap();
        assert_eq!(ctx["epics"][0]["stories"][0]["tasks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn a_status_heard_in_a_mirrors_thread_goes_to_its_master() {
        let (_env, l, id) = setup("ledger-xref-forward");
        let ev = "2026-10-28_110000";
        apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "SSO", "ac": ["SAML works"], "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "SAML config", "type": "action", "internal_owner": "Me", "quote": "I can have the SAML config ready by Friday"}),
        ]).unwrap();
        let other = crate::projects::create("Identity programme", None, None).unwrap().id;
        ensure_first_epic(&Ledger::of(&other).unwrap()).unwrap();
        human_event(&other, &json!({"op": "create", "kind": "story", "parent": "E01", "title": "Single sign-on"})).unwrap();
        let master = human_event(&other, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "SAML configuration"})).unwrap()["id"].as_str().unwrap().to_string();
        // An earlier link to the same thread (a wait, read first) doesn't change how the mirror is read.
        let other_task = human_event(&other, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "IdP contract"})).unwrap()["id"].as_str().unwrap().to_string();
        human_event(&id, &json!({"op": "xref", "node": "S001", "thread": other, "target": other_task, "kind": "depends_on"})).unwrap();
        // Here is the mirror; the identity programme does the work.
        human_event(&id, &json!({"op": "xref", "node": "T001", "thread": other, "target": master, "kind": "same", "master": "there"})).unwrap();
        // This thread's call says it is done; verified, it reaches the master and shows here as the master's.
        apply(&id, ev, &[json!({"op": "status", "node": "T001", "effect": "done", "quote": "I can have the SAML config ready by Friday"})]).unwrap();
        let p = verify_prep(&id, false).unwrap();
        verify_apply(&id, &[json!({"mid": p[0]["mid"], "verdict": "supports"})]).unwrap();
        let m = &fold_linked(&other).unwrap().nodes[&master];
        assert_eq!(m.state, "done");
        assert!(m.history.iter().any(|h| h["from"]["thread"] == id.as_str()), "the master says where it was heard");
        assert_eq!(fold_linked(&id).unwrap().nodes["T001"].state, "done");
        // The mirror's own history keeps its evidence ("What this call changed" still lists it).
        assert!(fold(&l).unwrap().nodes["T001"].history.iter().any(|h| h["op"] == "status" && h["evidence"] == ev));
    }

    #[test]
    fn apply_names_what_it_made_and_offers_the_same_work_once() {
        let (_env, l, id) = setup("ledger-same-work");
        let ev = "2026-10-28_110000";
        let r = apply(&id, ev, &[
            json!({"op": "create", "ref": "s1", "kind": "story", "parent": "E01", "title": "Security review", "ac": ["Diagram sent"], "quote": "Our security review needs the data-flow diagram"}),
            json!({"op": "create", "kind": "task", "parent": "s1", "title": "Send the data-flow diagram", "type": "action", "internal_owner": "Me", "quote": "can you send it this week"}),
        ]).unwrap();
        assert_eq!(r["created"].as_array().unwrap().iter().map(|c| c["id"].as_str().unwrap()).collect::<Vec<_>>(), ["S001", "T001"]);
        // The same ask again (as from a second source): offered as the same work, never merged by itself.
        let r = apply(&id, ev, &[json!({"op": "create", "kind": "task", "parent": "S001", "title": "Send them the data-flow diagram", "type": "action", "internal_owner": "Me", "quote": "Our security review needs the data-flow diagram"})]).unwrap();
        let new = r["created"][0]["id"].as_str().unwrap_or_else(|| panic!("{r}")).to_string();
        let t = fold(&l).unwrap().nodes;
        let sug = &t[&new].suggestions;
        assert_eq!((sug.len(), sug[0]["op"].as_str(), sug[0]["to"].as_str(), sug[0]["status"].as_str()), (1, Some("suggest_merge"), Some("T001"), Some("pending")), "{sug:?}");
        assert!(t[&new].state == "open" && t["T001"].state == "open", "nothing merged");
        // Not every shared word makes the same work: a different object, or a short title inside a long one.
        let r = apply(&id, ev, &[
            json!({"op": "create", "kind": "task", "parent": "S001", "title": "Send the vendor contract", "type": "action", "internal_owner": "Me", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "ref": "s2", "kind": "story", "parent": "E01", "title": "Security review of the vendor contract", "ac": ["Signed"], "quote": "Then SSO has to land first"}),
        ]).unwrap();
        let t = fold(&l).unwrap().nodes;
        for c in r["created"].as_array().unwrap() {
            assert!(t[c["id"].as_str().unwrap()].suggestions.is_empty(), "{c}");
        }
    }

    #[test]
    fn a_revise_is_applied_when_any_part_of_it_is() {
        let (_env, _l, id) = setup("ledger-revise-part");
        let ev = "2026-10-28_110000";
        apply(&id, ev, &[json!({"op": "create", "kind": "story", "parent": "E01", "title": "Pilot", "ac": ["SSO works"], "quote": "Then SSO has to land first"})]).unwrap();
        human_event(&id, &json!({"op": "edit", "node": "S001", "set": {"description": "Mine."}})).unwrap();
        apply(&id, ev, &[json!({"op": "revise", "node": "S001", "title": "Pilot after SSO", "description": "Theirs.", "quote": "Then SSO has to land first"})]).unwrap();
        let claims: Vec<Value> = verify_prep(&id, false).unwrap().iter().map(|c| json!({"mid": c["mid"], "verdict": "supports"})).collect();
        verify_apply(&id, &claims).unwrap();
        let t = fold(&Ledger::of(&id).unwrap()).unwrap();
        let s = &t.nodes["S001"];
        assert_eq!((s.title.as_str(), s.description.as_deref()), ("Pilot after SSO", Some("Mine.")), "the title moved, the pinned description stayed");
        let h = s.history.iter().rev().find(|h| h["op"] == "revise").unwrap();
        assert_eq!(h["applied"], true, "{h}");
    }

    #[cfg(feature = "brain")]
    #[test]
    fn trusting_a_sender_reads_the_clients_sources_again() {
        let (_env, _root) = crate::paths::testenv::folders("ledger-reread");
        crate::projects::add_client("Northwind", &["northwind.example".into()], false).unwrap();
        let a = crate::projects::create("Aurora", Some("northwind"), None).unwrap();
        let b = crate::projects::create("Elsewhere", None, None).unwrap();
        for p in [&a.id, &b.id] {
            fs::create_dir_all(crate::projects::dir().join(p).join("ledger")).unwrap();
            let st = json!({"domain:northwind.example": {"read_at": "2026-10-01T00:00:00Z", "checked_at": "2026-10-01T00:00:00Z", "found": 3}});
            fs::write(mails_file(p), st.to_string()).unwrap();
            fs::write(chats_file(p), json!({"19:x": {"read_at": "2026-10-01T00:00:00Z", "checked_at": "2026-10-01T00:00:00Z"}}).to_string()).unwrap();
        }
        reread_sources("northwind").unwrap();
        let m = mail_state(&a.id);
        assert!(m["domain:northwind.example"]["read_at"].is_null() && m["domain:northwind.example"]["checked_at"].is_null(), "{m}");
        assert_eq!(m["domain:northwind.example"]["found"], 3, "what it found stays");
        assert!(calls::read_json(&chats_file(&a.id))["19:x"]["read_at"].is_null());
        assert!(!mail_state(&b.id)["domain:northwind.example"]["read_at"].is_null(), "another client's thread reads on as it was");
    }

    #[test]
    fn a_mail_is_evidence_its_history_cites() {
        let (_env, l, id) = setup("ledger-mail");
        let m = json!({"id": "AAMk1", "internet_id": "<abc@mail.example>", "conversation": "c1", "subject": "FW: Pathfinder demo", "auth": "internal",
            "from": {"name": "Dana Kowal", "email": "dana@northwind.example"}, "to": [{"name": "Me Myself", "email": "me@us.example"}],
            "at": "2026-10-30T09:15:00Z", "text": "Could you prepare the agents demo for Northwind by next Thursday? They want to see Pathfinder live.",
            "url": "https://outlook.office365.com/owa/?ItemID=AAMk1"});
        let item = mail_item(&m).unwrap();
        let ev = item["id"].as_str().unwrap().to_string();
        assert!(ev.starts_with("mail-") && item["kind"] == "mail" && item["turns"][0]["speaker"] == "Dana Kowal");
        assert_eq!(mail_item(&json!({"internet_id": "<x>", "id": "y", "text": "x", "at": "2026-10-30T09:15:00Z", "invite": true})), None, "an invite is the call's");
        let dir_name = mail_item(&json!({"id": "z", "text": "x", "at": "2026-10-30T09:15:00Z", "from": {"name": "Kowal, Dana (Partner)", "email": "dana@northwind.example"}})).unwrap();
        assert_eq!(dir_name["turns"][0]["speaker"], "Dana Kowal", "names as people read them");
        assert_eq!(mail_item(&json!({"id": "y", "text": "x", "at": "2026-10-30T09:15:00Z", "url": "javascript:x"})).unwrap()["url"], Value::Null);
        fs::write(l.dir.join(format!("evidence/{ev}.json")), serde_json::to_vec(&item).unwrap()).unwrap();
        let r = apply(&id, &ev, &[json!({"op": "create", "kind": "story", "parent": "E01", "ref": "s", "title": "Pathfinder demo", "ac": ["Demo shown to Northwind"],
                "description": "Northwind asked to **see Pathfinder live**.", "quote": "They want to see Pathfinder live"}),
            json!({"op": "create", "kind": "task", "type": "action", "parent": "s", "title": "Prepare the agents demo", "internal_owner": "Me",
                "due_text": "next Thursday", "quote": "Could you prepare the agents demo for Northwind by next Thursday?"})]).unwrap();
        assert_eq!(r["events"], 2, "{r}");
        mark_processed(&id, &ev, "").unwrap();
        let t = tree(&id).unwrap();
        let task = &t["nodes"]["T001"];
        assert_eq!(task["history"][0]["evidence"], ev.as_str());
        assert_eq!(task["history"][0]["source"], "mail");
        assert_eq!(task["history"][0]["speaker"], "Dana Kowal");
        assert_eq!(task["due"], "2026-11-05", "a spoken date read against the mail's day");
        assert_eq!(t["evidence"][&ev]["url"], "https://outlook.office365.com/owa/?ItemID=AAMk1");
        assert!(t["nodes"]["S001"]["description_html"].as_str().unwrap().contains("<strong>see Pathfinder live</strong>"));
        assert_eq!(quote_in(&id, &ev, "prepare the agents demo").unwrap().unwrap()["url"], "https://outlook.office365.com/owa/?ItemID=AAMk1");
    }

    #[test]
    fn a_mail_source_with_subjects_reads_its_mail_and_the_rest_of_their_conversations() {
        let src = |subject: &[&str]| crate::projects::Source { kind: "mail".into(), id: "person:jane@acme.example".into(), subject: subject.iter().map(|w| w.to_string()).collect(), ..Default::default() };
        let m = |subject: &str, conv: &str| json!({"subject": subject, "conversation": conv});
        let found = [m("Acme Platform status", "c1"), m("Quick question", "c1"), m("Lunch on Friday", "c2"), m("RE: dates", "c3"), m("Acme platform: APX cut-over", "c4")];
        let none = BTreeSet::new();
        assert_eq!(about_thread(&found, &src(&["Acme Platform"]), &none), [true, true, false, false, true], "its subject, or its conversation's");
        let known: BTreeSet<String> = ["c3".to_string()].into();
        assert_eq!(about_thread(&found, &src(&["Acme Platform"]), &known), [true, true, false, true, true], "a conversation the thread already holds");
        assert_eq!(about_thread(&found, &src(&[]), &none), [true; 5], "no subjects: every mail of its people");
    }

    #[test]
    fn new_subjects_read_the_sources_window_again() {
        let (_env, _l, id) = setup("ledger-mail-again");
        let f = mails_file(&id);
        fs::write(&f, json!({"person:jane@acme.example": {"read_at": "2026-11-10T09:00:00+00:00", "checked_at": "2026-11-10T09:00:00+00:00", "found": 3}, "domain:acme.example": {"read_at": "x"}}).to_string()).unwrap();
        read_mail_again(&id, "person:jane@acme.example").unwrap();
        let st = mail_state(&id);
        assert!(st["person:jane@acme.example"]["read_at"].is_null() && st["person:jane@acme.example"]["checked_at"].is_null(), "{st}");
        assert_eq!(st["domain:acme.example"]["read_at"], "x", "the other sources as they were");
    }

    #[cfg(feature = "brain")]
    #[test]
    fn mail_waits_while_nobody_can_be_told_a_colleague() {
        let (_env, l, id) = setup("ledger-mail-sync");
        crate::projects::update(&id, |p| {
            p.sources.push(crate::projects::Source { kind: "mail".into(), id: "domain:northwind.example".into(), ..Default::default() });
            Ok(())
        })
        .unwrap();
        let p = crate::projects::load(&id).unwrap();
        assert!(mail_due(&p), "never checked");
        // Not signed in: the organisation's domains are unknown, so nothing is judged or read.
        assert!(mail_sync(&id).unwrap().is_empty());
        let st = mail_state(&id);
        assert!(st["domain:northwind.example"]["error"].as_str().unwrap().contains("mail waits"));
        assert!(st["domain:northwind.example"]["read_at"].is_null(), "the window is read again next time");
        assert!(!mail_due(&p), "checked just now");
        assert!(!l.evidence().keys().any(|k| k.starts_with("mail-")));
    }

    #[test]
    fn a_description_the_user_wrote_stays_theirs() {
        let (_env, l, id) = setup("ledger-description");
        let ev = "2026-10-28_110000";
        apply(&id, ev, &[json!({"op": "create", "kind": "story", "parent": "E01", "title": "Security review", "ac": ["Diagram sent"], "description": "Ravi's review.",
            "quote": "Our security review needs the data-flow diagram"})]).unwrap();
        mark_processed(&id, ev, "").unwrap();
        assert_eq!(fold(&l).unwrap().nodes["S001"].description.as_deref(), Some("Ravi's review."));
        let long = "x".repeat(MAX_AGENT_DESC + 1);
        let r = apply(&id, ev, &[json!({"op": "revise", "node": "S001", "description": long, "quote": "Our security review needs the data-flow diagram"})]).unwrap();
        assert_eq!(r["events"], 0, "an agent's description is short");
        human_event(&id, &json!({"op": "edit", "node": "S001", "set": {"description": "Mine: the **diagram** first."}})).unwrap();
        apply(&id, ev, &[json!({"op": "revise", "node": "S001", "description": "The agent's.", "quote": "can you send it this week?"})]).unwrap();
        let n = fold(&l).unwrap().nodes["S001"].clone();
        assert_eq!(n.description.as_deref(), Some("Mine: the **diagram** first."), "pinned");
        assert!(n.history.iter().any(|h| h["op"] == "revise" && h["applied"] == false));
        assert!(human_event(&id, &json!({"op": "edit", "node": "S001", "set": {"description": "y".repeat(MAX_DESC + 1)}})).is_err());
        human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "Send it", "description": "By hand."})).unwrap();
        assert_eq!(fold(&l).unwrap().nodes["T001"].description.as_deref(), Some("By hand."));
    }

    #[test]
    fn records_keep_their_substance() {
        let (_env, l, id) = setup("ledger-records");
        let ev = "2026-10-28_110000";
        let r = apply(&id, ev, &[
            json!({"op": "create", "ref": "q", "kind": "question", "parent": "E01", "title": "When does the pilot start?", "quote": "Then SSO has to land first"}),
            json!({"op": "create", "kind": "decision", "parent": "E01", "title": "Pilot after SSO", "description": "SSO first; the pilot moves to **24 Nov**.",
                   "decided_by": "Dana Kowal", "quote": "let's say the pilot moves to the 24th of November"}),
            json!({"op": "create", "ref": "r", "kind": "risk", "parent": "E01", "title": "Security review holds the pilot", "impact": "high", "quote": "Our security review needs the data-flow diagram"}),
            json!({"op": "answer", "node": "q", "text": "On 24 November, once SSO lands (Dana).", "quote": "the pilot moves to the 24th of November"}),
            json!({"op": "answer", "node": "r", "text": "Send the data-flow diagram this week.", "internal_owner": "Me", "likelihood": "med", "quote": "can you send it this week?"}),
            // Refused: an answer on a task, a level that isn't one, a risk's response as a description.
            json!({"op": "answer", "node": "E01", "text": "x", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "kind": "risk", "parent": "E01", "title": "SAML late", "likelihood": "very", "quote": "I can have the SAML config ready by Friday"}),
            json!({"op": "create", "kind": "risk", "parent": "E01", "title": "SAML late", "description": "Watch it.", "quote": "I can have the SAML config ready by Friday"}),
        ]).unwrap();
        assert_eq!(r["rejected"].as_array().unwrap().len(), 3, "{r}");
        // The refusal names the field to drop and where the response goes, or the model keeps retrying it (#137).
        let why = r["rejected"][2]["why"].as_str().unwrap();
        assert!(why.contains("without `description`") && why.contains("op 'answer'") && why.contains("`text`"), "{why}");
        mark_processed(&id, ev, "").unwrap();
        let t = fold(&l).unwrap();
        let (q, d, k) = (&t.nodes["Q001"], &t.nodes["D001"], &t.nodes["R001"]);
        assert_eq!((d.description.as_deref(), d.decided_by.as_deref()), (Some("SSO first; the pilot moves to **24 Nov**."), Some("Dana Kowal")), "a decision's content goes in at once");
        assert!(q.description.is_none() && q.state == "open" && record_open(q), "an answer waits for the verifier");
        assert!(k.description.is_none() && k.impact.as_deref() == Some("high") && k.likelihood.is_none());
        let packets = verify_prep(&id, false).unwrap();
        assert_eq!(packets.len(), 2);
        assert!(packets.iter().any(|p| p["claimed"].as_str().unwrap().starts_with("this question is answered: On 24 November")));
        assert!(packets.iter().any(|p| p["claimed"].as_str().unwrap().starts_with("what is done about this risk: Send the data-flow")));
        verify_apply(&id, &packets.iter().map(|p| json!({"mid": p["mid"], "verdict": "supports"})).collect::<Vec<_>>()).unwrap();
        let t = fold(&l).unwrap();
        let (q, k) = (&t.nodes["Q001"], &t.nodes["R001"]);
        assert!(q.state == "done" && answered(q) && !record_open(q), "answered and closed");
        assert_eq!(q.description.as_deref(), Some("On 24 November, once SSO lands (Dana)."));
        assert_eq!((k.description.as_deref(), k.internal_owner.as_deref(), k.likelihood.as_deref()), (Some("Send the data-flow diagram this week."), Some("Me"), Some("med")));
        assert!(record_open(k), "a handled risk is still open");
        let ctx = context(&id).unwrap().to_string();
        assert!(ctx.contains("Q001 When does the pilot start? (answered: On 24 November"), "{ctx}");
        assert!(ctx.contains("R001 Security review holds the pilot (response: Send the data-flow diagram this week.; owner Me; likelihood med; impact high)"), "{ctx}");
        assert!(ctx.contains("D001 Pilot after SSO (decided: SSO first; the pilot moves to **24 Nov**.; by Dana Kowal)"), "{ctx}");
        // The user's: an answer on an open question answers it; levels are checked; a record's text is theirs.
        human_event(&id, &json!({"op": "create", "kind": "question", "parent": "E01", "title": "Who presents?"})).unwrap();
        human_event(&id, &json!({"op": "edit", "node": "Q002", "set": {"description": "Dana."}})).unwrap();
        assert!(answered(&fold(&l).unwrap().nodes["Q002"]));
        assert!(human_event(&id, &json!({"op": "edit", "node": "R001", "set": {"impact": "huge"}})).is_err());
        human_event(&id, &json!({"op": "edit", "node": "R001", "set": {"impact": "low", "description": "Mine."}})).unwrap();
        apply(&id, ev, &[json!({"op": "answer", "node": "R001", "text": "Theirs.", "quote": "Our security review needs the data-flow diagram"})]).unwrap();
        let p = verify_prep(&id, false).unwrap();
        verify_apply(&id, &[json!({"mid": p[0]["mid"], "verdict": "supports"})]).unwrap();
        let k = fold(&l).unwrap().nodes["R001"].clone();
        assert_eq!((k.description.as_deref(), k.impact.as_deref()), (Some("Mine."), Some("low")), "pinned by the user");
    }

    #[test]
    fn links_to_where_it_came_from() {
        let (_env, l, id) = setup("ledger-links");
        let mail = json!({"title": "FW: Pathfinder", "url": "https://outlook.office365.com/owa/?ItemID=x"});
        human_event(&id, &json!({"op": "create", "kind": "story", "parent": "E01", "title": "Demo"})).unwrap();
        human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "Agents", "links": [mail]})).unwrap();
        assert_eq!(fold(&l).unwrap().nodes["T001"].links, vec![mail.clone()]);
        let two = json!([mail, {"title": "Ticket", "url": "https://jira.example.com/T-1"}]);
        human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"links": two}})).unwrap();
        assert_eq!(fold(&l).unwrap().nodes["T001"].links.len(), 2);
        // Only https, with a title: refused when written.
        for bad in [json!([{"title": "x", "url": "javascript:alert(1)"}]), json!([{"title": "", "url": "https://a.b"}]), json!("https://a.b")] {
            assert!(human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"links": bad}})).is_err());
            assert!(human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "y", "links": bad})).is_err());
        }
    }
}

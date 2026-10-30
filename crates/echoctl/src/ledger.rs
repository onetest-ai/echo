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
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, Weekday};
use serde::Serialize;
use serde_json::{json, Value};

use crate::calls;

const STATUS: [&str; 9] = ["held", "done", "progress", "blocked", "cancelled", "reopen", "due", "delayed", "reassign"];
const VERIFIED_OPS: [&str; 3] = ["ac_met", "link", "waiting"];
const KINDS: [&str; 6] = ["epic", "story", "task", "risk", "decision", "question"];
const TASK_TYPES: [&str; 3] = ["action", "session", "dependency"];
const CLOSED: [&str; 4] = ["done", "cancelled", "merged", "deleted"];
const HUMAN_FIELDS: [&str; 10] = ["title", "internal_owner", "client_owner", "due", "state", "parent", "type", "client_owed", "waiting_for", "column"];
const AGENT_OPS: [&str; 9] = ["create", "status", "revise", "ac_met", "link", "waiting", "suggest_move", "suggest_merge", "note"];
const HUMAN_OPS: [&str; 11] = ["edit", "ac_add", "ac_set", "link", "unlink", "merge", "delete", "accept", "reject", "undo", "create"];
const HUMAN_STATES: [&str; 5] = ["open", "in_progress", "blocked", "done", "cancelled"];
const MAX_AC: usize = 6;
const HIST_KEYS: [&str; 26] = [
    "mid", "type", "op", "effect", "evidence", "date", "ts", "speaker", "quote", "note", "due_text", "due", "by", "kind", "set", "text", "met", "ac",
    "ac_text", "depends_on", "into", "title", "ac_add", "who", "what", "source",
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
    Ok(Some(json!({"evidence": evid, "date": item["date"], "call": item["call"], "title": item["title"],
                   "ts": turn.map(|t| t["ts"].clone()), "speaker": turn.map(|t| t["speaker"].clone())})))
}

fn render_md(x: &Value) -> String {
    let mut head = vec![format!("# {}", x["title"].as_str().unwrap_or("")), format!("evidence: {} · {} · {}", x["id"].as_str().unwrap_or(""), x["date"].as_str().unwrap_or(""), x["kind"].as_str().unwrap_or(""))];
    if let Some(p) = x["people"].as_array().filter(|p| !p.is_empty()) {
        head.push(format!("people: {}", p.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
    }
    let mut body: Vec<String> = x["turns"].as_array().map(|a| a.iter().map(|t| format!("[{}] {}: {}", t["ts"].as_str().unwrap_or(""), t["speaker"].as_str().unwrap_or("?"), t["text"].as_str().unwrap_or(""))).collect()).unwrap_or_default();
    if let Some(t) = x["text"].as_str().filter(|t| !t.is_empty()) {
        body.push(t.to_string());
    }
    format!("{}\n\n{}\n", head.join("\n"), body.join("\n\n"))
}

fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A call as an evidence item: its transcript (turns), chat and the user's notes (text).
pub fn call_item(dir: &Path) -> Option<Value> {
    let meta = calls::read_json(&dir.join("meta.json"));
    let started = meta["started_at"].as_str()?.to_string();
    let date = chrono::DateTime::parse_from_rfc3339(&started).ok()?.with_timezone(&Local).date_naive().to_string();
    let segs = calls::read_json(&dir.join("segments.json"));
    let turns: Vec<Value> = segs
        .as_array()?
        .iter()
        .filter(|s| s["text"].as_str().is_some_and(|t| !t.trim().is_empty()))
        .map(|s| {
            let who = match s["speaker"].as_str() {
                Some("me") | Some("mic") => "Me".to_string(),
                _ => s["name"].as_str().unwrap_or("Remote").to_string(),
            };
            json!({"ts": clock(s["start"].as_f64().unwrap_or(0.0)), "speaker": who, "text": s["text"].as_str().unwrap_or("").trim()})
        })
        .collect();
    if turns.is_empty() {
        return None;
    }
    let mut text = vec![];
    if let Some(chat) = calls::read_json(&dir.join("chat.json")).as_array().filter(|c| !c.is_empty()) {
        text.push("## Chat during the call".to_string());
        for m in chat {
            text.push(format!("{}: {}", m["from"].as_str().unwrap_or("?"), m["text"].as_str().unwrap_or("")));
        }
    }
    if let Ok(n) = fs::read_to_string(dir.join(calls::USER_NOTES)) {
        if !n.trim().is_empty() {
            text.push(format!("## The user's own notes\n{}", n.trim()));
        }
    }
    let mut people: Vec<String> = vec![];
    for p in crate::meeting::MeetingContext::load(dir).ok().flatten().map(|m| m.organizer.into_iter().chain(m.attendees).collect::<Vec<_>>()).unwrap_or_default() {
        let name = p.name.trim().trim_end_matches("(external)").trim().to_string();
        if !name.is_empty() && !people.contains(&name) {
            people.push(name);
        }
    }
    let stamp = calls::stamp_of(dir);
    Some(json!({
        "id": stamp, "kind": "meeting", "source": "call", "at": started, "date": date,
        "title": meta["title"].as_str().unwrap_or(&stamp), "call": dir.file_name().map(|n| n.to_string_lossy().into_owned()),
        "people": people, "turns": turns, "text": text.join("\n"),
    }))
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
        let Some(item) = call_item(&d) else { continue };
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
        unlinked: vec![],
        created: s(&e["date"]),
        created_in: s(&e["evidence"]),
        creator: s(&e["speaker"]),
        by: s(&e["by"]).unwrap_or_else(|| "agent".into()),
        children: vec![],
        derived: None,
        waiting_on: vec![],
        blocks: vec![],
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
        "state" => n.state = s(v).unwrap_or_else(|| "open".into()),
        "type" => n.task_type = s(v),
        "client_owed" => n.client_owed = v == true,
        "waiting_for" => n.waiting_for = (!v.is_null()).then(|| v.clone()),
        "due" => n.due = s(v),
        "column" => n.column = s(v),
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
                        let added: Vec<Ac> = e["ac_add"].as_array().map(|a| a.iter().filter_map(|t| t.as_str()).map(|t| Ac { text: t.into(), met: false, met_by: None, source: Some("agent".into()), human: false, dropped: false }).collect()).unwrap_or_default();
                        let any = !added.is_empty();
                        x.ac.extend(added);
                        hist(x, e, t_ok || any, json!({}));
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
        "reassign" => {
            for k in ["internal_owner", "client_owner"] {
                if e[k].is_string() {
                    set_field(x, k, &e[k], false);
                }
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
    let l = Ledger::of(project)?;
    let p = crate::projects::load(project)?;
    let t = fold(&l)?;
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
        for (k, v) in [("internal_owner", &x.internal_owner), ("client_owner", &x.client_owner), ("due", &x.due)] {
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
            let xs: Vec<String> = x.children.iter().filter(|c| n[*c].kind == k).map(|c| format!("{c} {}{}", n[c].title.chars().take(90).collect::<String>(), if closed(&n[c].state) { " (closed)" } else { "" })).collect();
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
        "people": p.people,
        "evidence_processed": {"count": t.order.len(), "latest": t.order.iter().rev().take(5).rev().map(|c| format!("{} {}", t.evidence[c]["date"].as_str().unwrap_or(""), t.evidence[c]["title"].as_str().unwrap_or(""))).collect::<Vec<_>>()},
        "epics": epics.iter().filter(|e| !e.children.is_empty()).map(|e| view(n, e)).collect::<Vec<_>>(),
        "epics_without_work": epics.iter().filter(|e| e.children.is_empty()).map(|e| format!("{} {}", e.id, e.title)).collect::<Vec<_>>(),
    }))
}

// ── apply: agent proposals ───────────────────────────────────────────────────────────────────────

/// Owners as the project knows them: a member's name for their name, email, or unique first name.
fn canonical(p: &crate::projects::Project, who: Option<&str>) -> Option<String> {
    let w = who?.trim();
    if w.is_empty() {
        return None;
    }
    let lw = w.to_lowercase();
    if let Some(m) = p.people.iter().find(|m| m.name.to_lowercase() == lw || m.email.as_deref().is_some_and(|e| e.to_lowercase() == lw)) {
        return Some(m.name.clone());
    }
    let first: Vec<&crate::projects::Member> = p.people.iter().filter(|m| m.name.split_whitespace().next().is_some_and(|f| f.to_lowercase() == lw)).collect();
    if first.len() == 1 {
        return Some(first[0].name.clone());
    }
    Some(w.to_string())
}

fn key_str(v: &Value) -> String {
    if v.is_null() { "None".into() } else { v.to_string() }
}

/// Apply one evidence item's proposals: each checked against the item's words and the tree's rules.
/// Returns `{events, by_op, rejected}`.
pub fn apply(project: &str, evid: &str, proposals: &[Value]) -> Result<Value> {
    let l = Ledger::of(project)?;
    let p = crate::projects::load(project)?;
    let _lock = l.lock()?;
    let ev = l.evidence();
    let item = ev.get(evid).ok_or_else(|| anyhow!("unknown evidence {evid:?}"))?.clone();
    let full = norm(&full_text(&item));
    let evs = l.events()?;
    let mut n = fold_events(&evs, ev.clone())?.nodes;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &evs {
        if e["type"] == "create" {
            *counts.entry(s(&e["kind"]).unwrap_or_default()).or_default() += 1;
        }
    }
    let mut seen: BTreeSet<String> = evs.iter().filter_map(|e| s(&e["mid"])).collect();
    let mut made: BTreeMap<String, String> = evs.iter().filter(|e| e["type"] == "create").filter_map(|e| Some((s(&e["mid"])?, s(&e["id"])?))).collect();
    let mut handles: BTreeMap<String, String> = BTreeMap::new();
    let (mut out, mut rej): (Vec<Value>, Vec<Value>) = (vec![], vec![]);
    let date = s(&item["date"]).unwrap_or_default();
    let turns = item["turns"].as_array().cloned().unwrap_or_default();
    let locate = |q: &str| -> (Value, Value) {
        let head: String = q.chars().take(40).collect();
        for t in &turns {
            if norm(t["text"].as_str().unwrap_or("")).contains(&head) {
                return (t["ts"].clone(), json!(canonical(&p, t["speaker"].as_str())));
            }
        }
        (Value::Null, Value::Null)
    };
    let base = |pr: &Value, q: &str, key: &[String]| -> Value {
        let (ts, sp) = locate(q);
        let mut parts: Vec<&str> = vec![evid, q];
        parts.extend(key.iter().map(String::as_str));
        json!({"mid": mid_of(&parts), "evidence": evid, "date": date, "source": item["source"], "kind": item["kind"], "ts": ts, "speaker": sp,
               "quote": pr["quote"], "quote_norm": q, "note": pr["note"], "by": "agent"})
    };
    for (i, pr) in proposals.iter().enumerate() {
        let r = |x: &Value, h: &BTreeMap<String, String>| x.as_str().map(|v| h.get(v).cloned().unwrap_or_else(|| v.to_string()));
        let q = norm(pr["quote"].as_str().unwrap_or(""));
        let op = pr["op"].as_str().unwrap_or("");
        let why = if q.split(' ').count() < 3 {
            Some("quote too short".to_string())
        } else if !full.contains(&q) {
            Some("quote not verbatim in this evidence item (one contiguous span)".to_string())
        } else if !AGENT_OPS.contains(&op) {
            Some(format!("bad op {op:?}"))
        } else {
            None
        };
        if let Some(w) = why {
            rej.push(json!({"i": i, "why": w, "quote": pr["quote"].as_str().unwrap_or("").chars().take(80).collect::<String>()}));
            continue;
        }
        if op == "create" {
            let k = pr["kind"].as_str().unwrap_or("");
            let par = r(&pr["parent"], &handles);
            let pk = par.as_deref().and_then(|x| n.get(x)).map(|x| x.kind.clone());
            let ty = pr["type"].as_str().unwrap_or("");
            let ac = pr["ac"].as_array().cloned().unwrap_or_default();
            let title = pr["title"].as_str().unwrap_or("").trim().to_string();
            let why = if !KINDS.contains(&k) {
                Some(format!("bad kind {k:?}"))
            } else if k == "epic" && pr["plan"] != "Off-plan" {
                Some("agents create workstreams (epics) only for off-plan work: plan must be 'Off-plan'".into())
            } else if title.is_empty() {
                Some("a create needs a title".into())
            } else if k != "epic" && !parent_ok(k, pk.as_deref()) {
                Some(format!("a {k} needs a parent of the right kind, got {par:?} ({pk:?})"))
            } else if k == "story" && ac.is_empty() {
                Some("a story needs acceptance criteria (ac: [...]): what must be true for it to be done".into())
            } else if k == "story" && ac.len() > MAX_AC {
                Some(format!("more than {MAX_AC} acceptance criteria: split it into two stories and link them"))
            } else if k == "task" && !TASK_TYPES.contains(&ty) {
                Some(format!("task type must be one of {TASK_TYPES:?}"))
            } else if k == "task" && !(pr["internal_owner"].is_string() || pr["client_owner"].is_string() || pr["due_text"].is_string() || ty == "session" || ty == "dependency") {
                Some("admission bar: a task needs an owner, a date, or to be a session/dependency; else op 'note'".into())
            } else {
                None
            };
            if let Some(w) = why {
                rej.push(json!({"i": i, "why": w, "title": pr["title"]}));
                continue;
            }
            let cmid = mid_of(&[evid, &q, "create", k, par.as_deref().unwrap_or("None"), &title]);
            let handle = pr["ref"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{i}"));
            if let Some(id) = made.get(&cmid) {
                handles.insert(handle, id.clone());
                continue;
            }
            let c = counts.entry(k.to_string()).or_default();
            *c += 1;
            let nid = new_id(k, *c);
            handles.insert(handle, nid.clone());
            let mut e = base(pr, &q, &["create".into(), nid.clone()]);
            e["mid"] = json!(cmid);
            e["type"] = json!("create");
            e["id"] = json!(nid);
            e["kind"] = json!(k);
            e["fields"] = json!({
                "title": title, "parent": if k == "epic" { None } else { par.clone() }, "type": pr["type"],
                "plan": if k == "epic" { Some("Off-plan") } else { None },
                "internal_owner": canonical(&p, pr["internal_owner"].as_str()), "client_owner": canonical(&p, pr["client_owner"].as_str()),
                "client_owed": pr["client_owed"] == true, "ac": ac,
                "due": resolve_due(pr["due_text"].as_str(), &date), "due_text": pr["due_text"],
            });
            n.insert(nid.clone(), node_of(&e));
            made.insert(cmid, nid.clone());
            out.push(e);
            for d in pr["depends_on"].as_array().cloned().unwrap_or_default() {
                let d = r(&d, &handles).unwrap_or_default();
                if n.get(&d).is_some_and(|x| x.kind == "story" || x.kind == "task") && (k == "story" || k == "task") {
                    let mut le = base(pr, &q, &["link".into(), nid.clone(), d.clone()]);
                    le["type"] = json!("agent");
                    le["op"] = json!("link");
                    le["node"] = json!(nid);
                    le["depends_on"] = json!(d);
                    out.push(le);
                }
            }
            continue;
        }
        let node = r(&pr["node"], &handles).unwrap_or_default();
        if op == "note" {
            if !n.contains_key(&node) {
                rej.push(json!({"i": i, "why": format!("note needs a known node, got {node:?}")}));
                continue;
            }
            if pr["note"].as_str().is_none_or(|t| t.trim().is_empty()) {
                rej.push(json!({"i": i, "why": "note needs its text in `note`"}));
                continue;
            }
            let mut e = base(pr, &q, &["note".into(), node.clone()]);
            e["type"] = json!("note");
            e["node"] = json!(node);
            e["text"] = pr["note"].clone();
            out.push(e);
            continue;
        }
        let Some(x) = n.get(&node) else {
            rej.push(json!({"i": i, "why": format!("unknown node {node:?}")}));
            continue;
        };
        let later: Vec<String> = [Some(node.clone()), r(&pr["depends_on"], &handles), r(&pr["into"], &handles)]
            .into_iter()
            .flatten()
            .filter(|y| n.get(y).is_some_and(|z| z.created.as_deref().unwrap_or("") > date.as_str()))
            .collect();
        if !later.is_empty() {
            rej.push(json!({"i": i, "why": format!("{later:?} did not exist yet on {date}")}));
            continue;
        }
        let mut key: Vec<String> = ["effect", "depends_on", "into", "parent", "ac", "what"].iter().map(|k| key_str(&pr[*k])).collect();
        if op == "revise" {
            key.push(key_str(&pr["title"]));
            key.push(key_str(&pr["ac_add"]));
        }
        let mut k2 = vec![op.to_string(), node.clone()];
        k2.extend(key);
        let mut e = base(pr, &q, &k2);
        e["type"] = json!("agent");
        e["op"] = json!(op);
        e["node"] = json!(node);
        let mid = s(&e["mid"]).unwrap_or_default();
        if seen.contains(&mid) {
            continue;
        }
        let bad = |w: String, rej: &mut Vec<Value>| rej.push(json!({"i": i, "why": w}));
        match op {
            "status" => {
                let eff = pr["effect"].as_str().unwrap_or("");
                if !STATUS.contains(&eff) && eff != "mention" {
                    bad(format!("bad effect {eff:?}"), &mut rej);
                    continue;
                }
                if eff == "due" && resolve_due(pr["due_text"].as_str(), &date).is_none() {
                    bad(format!("no date can be read from due_text {:?}: write it as a date (\"December 18\", \"next Friday\", \"2026-12-18\")", pr["due_text"].as_str().unwrap_or("")), &mut rej);
                    continue;
                }
                e["effect"] = json!(eff);
                e["due_text"] = pr["due_text"].clone();
                e["due"] = if eff == "delayed" { Value::Null } else { json!(resolve_due(pr["due_text"].as_str(), &date)) };
                e["internal_owner"] = json!(canonical(&p, pr["internal_owner"].as_str()));
                e["client_owner"] = json!(canonical(&p, pr["client_owner"].as_str()));
            }
            "revise" => {
                let add = pr["ac_add"].as_array().map(|a| a.len()).unwrap_or(0);
                if !pr["title"].is_string() && add == 0 {
                    bad("revise needs a new title and/or ac_add".into(), &mut rej);
                    continue;
                }
                if x.ac.len() + add > MAX_AC {
                    bad(format!("{node} would have more than {MAX_AC} criteria: create a second story"), &mut rej);
                    continue;
                }
                e["title"] = pr["title"].clone();
                e["ac_add"] = pr["ac_add"].clone();
            }
            "ac_met" => {
                let a = pr["ac"].as_u64().unwrap_or(0) as usize;
                if x.kind != "story" || a == 0 || a > x.ac.len() {
                    bad("ac_met needs a story node and the 1-based number of one of its criteria".into(), &mut rej);
                    continue;
                }
                let live = x.ac[a - 1].text.clone();
                if let Some(saw) = pr["ac_text"].as_str() {
                    if norm(saw) != norm(&live) {
                        bad(format!("criterion {a} of {node} is now “{live}”, not “{saw}”: the criteria changed since you read context; re-read it"), &mut rej);
                        continue;
                    }
                }
                e["ac"] = json!(a);
                e["ac_text"] = json!(live);
            }
            "link" => {
                let d = r(&pr["depends_on"], &handles).unwrap_or_default();
                let ok = n.get(&d).is_some_and(|y| y.kind == "story" || y.kind == "task") && (x.kind == "story" || x.kind == "task");
                if !ok {
                    bad(format!("link joins stories/tasks; got {node} -> {d}"), &mut rej);
                    continue;
                }
                e["depends_on"] = json!(d);
            }
            "waiting" => {
                if x.kind != "story" && x.kind != "task" {
                    bad("waiting applies to a story or task".into(), &mut rej);
                    continue;
                }
                e["who"] = json!(canonical(&p, pr["who"].as_str()));
                e["what"] = pr["what"].clone();
            }
            "suggest_move" => {
                let to = r(&pr["parent"], &handles).unwrap_or_default();
                if !n.get(&to).is_some_and(|y| parent_ok(&x.kind, Some(&y.kind))) {
                    bad(format!("cannot move {node} under {to}"), &mut rej);
                    continue;
                }
                e["parent"] = json!(to);
            }
            "suggest_merge" => {
                let to = r(&pr["into"], &handles).unwrap_or_default();
                if n.get(&to).is_none_or(|y| y.kind != x.kind) || to == node {
                    bad(format!("merge needs two nodes of one kind; got {node} -> {to}"), &mut rej);
                    continue;
                }
                e["into"] = json!(to);
            }
            _ => {}
        }
        seen.insert(mid);
        out.push(e);
    }
    l.append(&out)?;
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for e in &out {
        let k = if e["type"] == "create" { format!("create_{}", e["kind"].as_str().unwrap_or("")) } else { e["op"].as_str().or(e["type"].as_str()).unwrap_or("").to_string() };
        *by.entry(k).or_default() += 1;
    }
    Ok(json!({"events": out.len(), "by_op": by, "rejected": rej}))
}

// ── verify ──────────────────────────────────────────────────────────────────────────────────────

/// The claims a verifier must judge (each with its words in context). `weak`: also those judged weak.
pub fn verify_prep(project: &str, weak: bool) -> Result<Vec<Value>> {
    let l = Ledger::of(project)?;
    let evs = l.events()?;
    let ev = l.evidence();
    let latest: BTreeMap<String, String> = evs.iter().filter(|e| e["type"] == "verdict").filter_map(|e| Some((s(&e["mid"])?, s(&e["verdict"])?))).collect();
    let n = fold_events(&evs, ev.clone())?.nodes;
    let mut out = vec![];
    for e in evs.iter().filter(|e| needs_verdict(e)) {
        let mid = s(&e["mid"]).unwrap_or_default();
        if let Some(v) = latest.get(&mid) {
            if !(weak && v == "weak") {
                continue;
            }
        }
        let item = e["evidence"].as_str().and_then(|i| ev.get(i)).cloned().unwrap_or(Value::Null);
        let turns = item["turns"].as_array().cloned().unwrap_or_default();
        let head: String = e["quote_norm"].as_str().unwrap_or("").chars().take(40).collect();
        let k = turns.iter().position(|t| norm(t["text"].as_str().unwrap_or("")).contains(&head));
        let ctx: Vec<String> = match k {
            Some(k) => turns[k.saturating_sub(2)..(k + 3).min(turns.len())].iter().map(|t| format!("[{}] {}: {}", t["ts"].as_str().unwrap_or(""), t["speaker"].as_str().unwrap_or(""), t["text"].as_str().unwrap_or(""))).collect(),
            None => vec![item["text"].as_str().unwrap_or("").chars().take(1200).collect()],
        };
        let node = e["node"].as_str().unwrap_or("");
        let x = n.get(node);
        let claim = match e["op"].as_str().unwrap_or("") {
            "ac_met" => format!("acceptance criterion {} met: {}", e["ac"], e["ac_text"].as_str().unwrap_or("?")),
            "link" => {
                let d = e["depends_on"].as_str().unwrap_or("");
                format!("cannot finish until “{}” ({d}) does", n.get(d).map(|y| y.title.as_str()).unwrap_or(d))
            }
            "waiting" => match e["what"].as_str() {
                Some(w) if !w.is_empty() => format!("is waiting on {}: {w}", e["who"].as_str().unwrap_or("someone")),
                _ => "is no longer waiting on anyone (what it waited for arrived or was dropped)".into(),
            },
            _ => e["effect"].as_str().unwrap_or("").to_string(),
        };
        out.push(json!({"mid": mid, "node": node, "kind": x.map(|y| &y.kind), "path": path(&n, node), "claimed": claim, "due": e["due"],
                        "quote": e["quote"], "evidence": format!("{} {}", e["date"].as_str().unwrap_or(""), item["title"].as_str().unwrap_or("")), "context": ctx}));
    }
    Ok(out)
}

pub fn verify_apply(project: &str, verdicts: &[Value]) -> Result<Value> {
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    let mids: BTreeSet<String> = l.events()?.iter().filter(|e| needs_verdict(e)).filter_map(|e| s(&e["mid"])).collect();
    let (mut out, mut rej) = (vec![], vec![]);
    for v in verdicts {
        let ok = v["mid"].as_str().is_some_and(|m| mids.contains(m)) && ["supports", "weak", "wrong"].contains(&v["verdict"].as_str().unwrap_or(""));
        if !ok {
            rej.push(v.clone());
            continue;
        }
        out.push(json!({"type": "verdict", "mid": v["mid"], "verdict": v["verdict"], "reason": v["reason"], "by": "verifier"}));
    }
    l.append(&out)?;
    let mut c: BTreeMap<String, usize> = BTreeMap::new();
    for v in &out {
        *c.entry(s(&v["verdict"]).unwrap_or_default()).or_default() += 1;
    }
    Ok(json!({"verdicts": c, "rejected": rej}))
}

// ── the user's edits ─────────────────────────────────────────────────────────────────────────────

fn check_edit(sets: &Value, n: &BTreeMap<String, Node>) -> Result<()> {
    let Some(m) = sets.as_object() else { bail!("set must be an object of {HUMAN_FIELDS:?}") };
    for (k, v) in m {
        match k.as_str() {
            "due" if !v.is_null() => {
                NaiveDate::parse_from_str(v.as_str().unwrap_or(""), "%Y-%m-%d").map_err(|_| anyhow!("due must be YYYY-MM-DD or null"))?;
            }
            "due" => {}
            "state" if !HUMAN_STATES.contains(&v.as_str().unwrap_or("")) => bail!("state must be one of {HUMAN_STATES:?}"),
            "waiting_for" if !v.is_null() && !(v.is_object() && (v["what"].is_string() || v["who"].is_string())) => bail!("waiting_for is null or {{who, what}}"),
            "parent" if !v.as_str().is_some_and(|p| n.contains_key(p)) => bail!("unknown parent {v}"),
            "client_owed" if !v.is_boolean() => bail!("client_owed is true or false"),
            "column" if !(v.is_null() || v.as_str().is_some_and(|c| !c.is_empty() && c.len() <= 40)) => bail!("column is a column id"),
            "title" if v.as_str().is_none_or(|t| t.trim().is_empty()) => bail!("title cannot be empty"),
            "internal_owner" | "client_owner" | "type" if !(v.is_null() || v.is_string()) => bail!("{k} must be text"),
            k if !HUMAN_FIELDS.contains(&k) => bail!("editable fields: {HUMAN_FIELDS:?}"),
            _ => {}
        }
    }
    Ok(())
}

/// An edit the user made (`{op, node, set|text|ac|met|drop|depends_on|into|target, …}` or a create).
pub fn human_event(project: &str, body: &Value) -> Result<Value> {
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    ensure_first_epic(&l)?;
    let evs = l.events()?;
    let t = fold_events(&evs, l.evidence())?;
    let n = &t.nodes;
    let op = body["op"].as_str().unwrap_or("");
    if !HUMAN_OPS.contains(&op) {
        bail!("bad op {op:?}");
    }
    let last = t.order.last().cloned();
    let now = Local::now().to_rfc3339();
    let date = last.as_ref().and_then(|c| s(&t.evidence[c]["date"])).unwrap_or_else(|| now[..10].to_string());
    let mut e = json!({"type": "human", "op": op, "by": "human", "evidence": last, "date": date, "at": now});
    if op == "create" {
        let k = body["kind"].as_str().unwrap_or("");
        let par = body["parent"].as_str();
        if !KINDS.contains(&k) || (k != "epic" && !parent_ok(k, par.and_then(|p| n.get(p)).map(|x| x.kind.as_str()))) {
            bail!("cannot create a {k} under {par:?}");
        }
        let c = evs.iter().filter(|x| x["type"] == "create" && x["kind"] == k).count() + 1;
        e = json!({"type": "create", "id": new_id(k, c), "kind": k, "by": "human", "evidence": last, "date": date, "at": now,
                   "fields": {"title": body["title"].as_str().filter(|t| !t.trim().is_empty()).unwrap_or("Untitled"),
                              "parent": if k == "epic" { None } else { par }, "type": body["type"].as_str().or(if k == "task" { Some("action") } else { None }),
                              "ac": body["ac"].as_array().cloned().unwrap_or_default(), "plan": if k == "epic" { Some("Off-plan") } else { None },
                              "internal_owner": body["internal_owner"], "due": body["due"]}});
    } else if ["undo", "accept", "reject"].contains(&op) {
        let target = body["target"].as_str().ok_or_else(|| anyhow!("target mid required"))?;
        e["target"] = json!(target);
    } else {
        let node = body["node"].as_str().filter(|x| n.contains_key(*x)).ok_or_else(|| anyhow!("unknown node {}", body["node"]))?;
        e["node"] = json!(node);
        for k in ["set", "text", "ac", "met", "drop", "depends_on", "into"] {
            if !body[k].is_null() {
                e[k] = body[k].clone();
            }
        }
        if op == "edit" {
            check_edit(&e["set"], n)?;
        }
        if op == "ac_set" && !body["ac"].is_u64() {
            bail!("ac must be a 1-based number");
        }
        if (op == "link" || op == "unlink") && !body["depends_on"].as_str().is_some_and(|d| n.contains_key(d)) {
            bail!("unknown node {}", body["depends_on"]);
        }
        if op == "merge" && !body["into"].as_str().is_some_and(|d| n.contains_key(d)) {
            bail!("unknown node {}", body["into"]);
        }
    }
    e["mid"] = json!(mid_of(&["human", &now, &body.to_string()]));
    l.append(std::slice::from_ref(&e))?;
    Ok(e)
}

/// The tree for the window and `ledger tree`: every node, the workstreams first.
/// What one call changed in a project's work (the call's Actions tab): every history entry whose
/// evidence is this call, by node, in the order said — new nodes, status changes, dates, links,
/// waits, criteria met — with the words, the verifier's mark, and the node as it is now. Mentions
/// are left out (they change nothing). `read`: the sync has read the call; `queued`: it will.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn call_changes(project: &str, stamp: &str) -> Result<Value> {
    let l = Ledger::of(project)?;
    let t = fold(&l)?;
    let read = t.order.iter().any(|e| e == stamp);
    let queued = !read && pending(project)?.iter().any(|e| e["id"] == stamp);
    let mut out: Vec<Value> = vec![];
    for n in t.nodes.values().filter(|n| n.state != "merged" && n.state != "deleted" && n.kind != "epic") {
        for h in n.history.iter().filter(|h| h["evidence"] == stamp && h["by"] != "human" && h["effect"] != "mention") {
            let mut c = h.clone();
            c["node"] = json!({"id": n.id, "kind": n.kind, "type": n.task_type, "title": n.title, "state": n.state, "due": n.due,
                               "owner": n.internal_owner.as_ref().or(n.client_owner.as_ref())});
            if let Some(d) = h["depends_on"].as_str().and_then(|d| t.nodes.get(d)) {
                c["depends_on_title"] = json!(d.title);
            }
            out.push(c);
        }
    }
    out.sort_by(|a, b| a["ts"].as_str().unwrap_or("").cmp(b["ts"].as_str().unwrap_or("")));
    Ok(json!({"read": read, "queued": queued, "changes": out}))
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
fn words(s: &str) -> BTreeSet<String> {
    norm(s).split_whitespace().filter(|w| w.len() > 2).map(str::to_owned).collect()
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
fn hms(ts: &str) -> Option<f64> {
    let p: Vec<f64> = ts.split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    Some(p.iter().fold(0.0, |a, x| a * 60.0 + x))
}

/// The task an action item became, if any: one created from this call within a minute of the item's
/// moment whose words share a third of the item's (or, with no moment, half). Deterministic.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn match_actions(actions: &[Value], changes: &[(String, Value)]) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let mut used = BTreeSet::new();
    for a in actions {
        let (Some(id), Some(text)) = (a["id"].as_str(), a["task"].as_str()) else { continue };
        let aw = words(text);
        if aw.is_empty() {
            continue;
        }
        let best = changes
            .iter()
            .filter(|(p, c)| c["op"] == "create" && c["node"]["kind"] == "task" && !used.contains(&format!("{p}/{}", c["node"]["id"])))
            .filter_map(|(p, c)| {
                let tw = words(&format!("{} {}", c["node"]["title"].as_str().unwrap_or(""), c["quote"].as_str().unwrap_or("")));
                let share = aw.intersection(&tw).count() as f64 / aw.len() as f64;
                let near = a["t"].as_f64().zip(c["ts"].as_str().and_then(hms)).map(|(x, y)| (x - y).abs() <= 60.0);
                let ok = match near {
                    Some(true) => share >= 0.34,
                    Some(false) => share >= 0.75,
                    None => share >= 0.5,
                };
                ok.then_some((share + if near == Some(true) { 1.0 } else { 0.0 }, p, c))
            })
            .max_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((_, p, c)) = best {
            used.insert(format!("{p}/{}", c["node"]["id"]));
            out.insert(id.to_owned(), json!({"project": p, "id": c["node"]["id"], "title": c["node"]["title"], "state": c["node"]["state"]}));
        }
    }
    out
}

pub fn tree(project: &str) -> Result<Value> {
    let l = Ledger::of(project)?;
    let t = fold(&l)?;
    let pend = pending(project)?.len();
    Ok(json!({
        "nodes": t.nodes, "notes": t.notes,
        "processed": t.order.len(), "pending": pend,
        // Each evidence item's call: history entries name the call they came from.
        "evidence": t.evidence.iter().map(|(k, v)| (k.clone(), json!({"title": v["title"], "call": v["call"], "date": v["date"]}))).collect::<serde_json::Map<String, Value>>(),
        "roots": t.nodes.values().filter(|x| x.kind == "epic" && x.state != "merged" && x.state != "deleted").map(|x| &x.id).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(c["read"], true);
        let ch = c["changes"].as_array().unwrap();
        assert_eq!(ch.len(), 2, "a mention changes nothing: {c}");
        assert_eq!(ch[0]["node"]["id"], "S001", "in the order said");
        assert_eq!(ch[1]["node"]["due"], "2026-10-30");

        let all: Vec<(String, Value)> = ch.iter().map(|x| (id.clone(), x.clone())).collect();
        let m = match_actions(&[
            json!({"id": "a1", "task": "Have the SAML config ready by Friday", "t": 790.0}),
            json!({"id": "a2", "task": "Book the Berlin flights", "t": 790.0}),
            json!({"id": "a3", "task": "SAML config ready", "t": 3000.0}),
        ], &all);
        assert_eq!(m["a1"]["id"], "T001");
        assert!(!m.contains_key("a2") && !m.contains_key("a3"), "unrelated words; a task taken once: {m:?}");
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
        assert!(human_event(&id, &json!({"op": "edit", "node": "T001", "set": {"state": "finished"}})).is_err());
        assert!(human_event(&id, &json!({"op": "edit", "node": "T999", "set": {"title": "x"}})).is_err());
        let c = human_event(&id, &json!({"op": "create", "kind": "task", "parent": "S001", "title": "Book the room"})).unwrap();
        assert_eq!(c["id"], "T002");
        assert!(human_event(&id, &json!({"op": "create", "kind": "task", "parent": "E01", "title": "No story"})).is_err());
        let ctx = context(&id).unwrap();
        assert_eq!(ctx["epics"][0]["stories"][0]["tasks"].as_array().unwrap().len(), 2);
    }
}

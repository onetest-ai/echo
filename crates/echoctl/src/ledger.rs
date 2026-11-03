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
const VERIFIED_OPS: [&str; 4] = ["ac_met", "link", "waiting", "answer"];
const KINDS: [&str; 6] = ["epic", "story", "task", "risk", "decision", "question"];
const TASK_TYPES: [&str; 3] = ["action", "session", "dependency"];
const CLOSED: [&str; 4] = ["done", "cancelled", "merged", "deleted"];
const HUMAN_FIELDS: [&str; 16] = [
    "title", "internal_owner", "client_owner", "due", "state", "parent", "type", "client_owed", "waiting_for", "column", "links", "description", "delegated_by",
    "decided_by", "likelihood", "impact",
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
const HIST_KEYS: [&str; 33] = [
    "mid", "type", "op", "effect", "evidence", "date", "ts", "speaker", "quote", "note", "due_text", "due", "by", "kind", "set", "text", "met", "ac",
    "ac_text", "depends_on", "into", "title", "ac_add", "who", "what", "source", "thread", "target", "master", "internal_owner", "client_owner",
    "delegated_by", "at",
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

// ── the meeting chat between calls ───────────────────────────────────────────────────────────

/// How far back a chat's first read goes, and how far each later read looks again (a sender trusted
/// meanwhile, a message edited): its days are rebuilt and an item that changed is read again.
const CHAT_FIRST_DAYS: i64 = 30;
const CHAT_AGAIN_DAYS: i64 = 7;
/// Messages taken at most a read (the newest).
const CHAT_MAX: usize = 2000;
/// Messages this close to one of this thread's calls on the same chat belong to that call's item.
const CALL_MARGIN_MIN: i64 = 2;
/// A chat is read at most this often (the sync looks every 5 minutes).
pub const CHAT_EVERY_MIN: i64 = 30;

fn chats_file(project: &str) -> PathBuf {
    crate::projects::dir().join(project).join("ledger").join("chats.json")
}

/// Per chat: `{read_at, found, days, error, checked_at}` (the Sources chip shows it).
pub fn chat_state(project: &str) -> Value {
    fs::read(chats_file(project)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}))
}

fn short(id: &str) -> String {
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
struct ChatTrust {
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
enum Said {
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
fn call_windows(p: &crate::projects::Project, chat: &str) -> Vec<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> {
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
fn chat_item(chat: &str, name: &str, day: chrono::NaiveDate, msgs: &[&Value]) -> Option<Value> {
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

// ── mail ────────────────────────────────────────────────────────────────────────────────────────

/// A mail as one evidence item (`mail-<sha8 of its Internet message id>`: Graph's own id changes when
/// the mail moves to another folder): the new part of its text as one turn at the local time it
/// came, the sender as the speaker, and its link in Outlook (`url`).
fn mail_item(m: &Value) -> Option<Value> {
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

/// Whether an address is the user's: the one in Settings → About me, or (with the Brain) the signed-in
/// account's local part at any of their domains.
fn mail_me() -> impl Fn(&str) -> bool {
    let mine = crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| m.email).map(|e| e.to_lowercase());
    #[cfg(feature = "brain")]
    let local = crate::brain::sources::me_local();
    #[cfg(not(feature = "brain"))]
    let local: Option<String> = None;
    move |e: &str| Some(e) == mine.as_deref() || local.as_deref().is_some_and(|l| e.split('@').next() == Some(l))
}

/// How far back a mail source's first read goes, and how far each later read looks again.
#[cfg(feature = "brain")]
const MAIL_FIRST_DAYS: i64 = 30;
#[cfg(feature = "brain")]
const MAIL_AGAIN_DAYS: i64 = 7;
/// Mails taken at most a read (the newest).
#[cfg(feature = "brain")]
const MAIL_MAX: usize = 200;

fn mails_file(project: &str) -> PathBuf {
    crate::projects::dir().join(project).join("ledger").join("mails.json")
}

/// Each ✉ Mail source's read into the work: `{<rule>: {read_at, checked_at, found, error}}`.
pub fn mail_state(project: &str) -> Value {
    calls::read_json(&mails_file(project)).as_object().map(|o| Value::Object(o.clone())).unwrap_or_else(|| json!({}))
}

/// A mail source not checked for `CHAT_EVERY_MIN` (or never).
pub fn mail_due(p: &crate::projects::Project) -> bool {
    // Without the Brain built in mail isn't read: never due (it would be every tick).
    if !cfg!(feature = "brain") {
        return false;
    }
    let st = mail_state(&p.id);
    p.sources.iter().filter(|s| s.kind == "mail").any(|s| {
        st[&s.id]["checked_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).is_none_or(|t| Local::now().signed_duration_since(t) >= chrono::Duration::minutes(CHAT_EVERY_MIN))
    })
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
    if rules.is_empty() {
        return Ok(vec![]);
    }
    let l = Ledger::of(project)?;
    let mut state = mail_state(project);
    let now = chrono::Utc::now();
    let mut snd = senders::load();
    snd.refresh_org();
    let mut items: Vec<Value> = vec![];
    if snd.org_domains.is_empty() {
        // Nobody can be told a colleague: nothing is judged, every source reads this window next time.
        for s in &rules {
            state[&s.id]["checked_at"] = json!(Local::now().to_rfc3339());
            state[&s.id]["error"] = json!("couldn't read your organisation's domains: mail waits");
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
            let found = match crate::graph::mail(&rule.kql(), since, MAIL_MAX) {
                Ok(f) => f,
                Err(e) => {
                    state[&s.id]["error"] = json!(format!("{e:#}"));
                    continue;
                }
            };
            let mut n = 0;
            for m in &found {
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
        let _ = senders::save_merged(&mut snd);
    }
    // Sources no longer watched forget their state (one added back reads its 30 days again).
    if let Some(o) = state.as_object_mut() {
        o.retain(|k, _| rules.iter().any(|s| &s.id == k));
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
        && l["url"].as_str().is_some_and(|u| u.starts_with("https://") && u.len() <= 2000 && !u.chars().any(char::is_whitespace))
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
        bail!("links is a list of at most {MAX_LINKS} {{title, url}}, url https://");
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
        "reassign" => {
            for k in ["internal_owner", "client_owner", "delegated_by"] {
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
            } else if pr["description"].as_str().is_some_and(|t| t.chars().count() > MAX_AGENT_DESC) {
                Some(format!("description: at most {MAX_AGENT_DESC} characters"))
            } else if (k == "question" || k == "risk") && pr["description"].as_str().is_some_and(|t| !t.trim().is_empty()) {
                Some(format!("a {k}'s {} is checked by the verifier: create it without one, then op 'answer' on its ref", if k == "question" { "answer" } else { "response" }))
            } else if let Some(f) = ["likelihood", "impact"].into_iter().find(|f| !pr[*f].is_null() && (k != "risk" || level_of(&pr[*f]).is_none())) {
                Some(format!("{f} is for a risk: one of {LEVELS:?}"))
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
                "delegated_by": canonical(&p, pr["delegated_by"].as_str()),
                "client_owed": pr["client_owed"] == true, "ac": ac,
                "due": resolve_due(pr["due_text"].as_str(), &date), "due_text": pr["due_text"],
                "description": pr["description"].as_str().map(str::trim).filter(|t| !t.is_empty()),
                "decided_by": if k == "decision" { canonical(&p, pr["decided_by"].as_str()) } else { None },
                "likelihood": pr["likelihood"], "impact": pr["impact"],
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
        if op == "answer" {
            key.push(key_str(&pr["text"]));
        }
        if op == "revise" {
            key.push(key_str(&pr["title"]));
            key.push(key_str(&pr["ac_add"]));
            key.push(key_str(&pr["description"]));
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
                e["delegated_by"] = json!(canonical(&p, pr["delegated_by"].as_str()));
            }
            "revise" => {
                let add = pr["ac_add"].as_array().map(|a| a.len()).unwrap_or(0);
                let desc = pr["description"].as_str().map(str::trim).filter(|t| !t.is_empty());
                if !pr["title"].is_string() && add == 0 && desc.is_none() {
                    bad("revise needs a new title, ac_add and/or description".into(), &mut rej);
                    continue;
                }
                if desc.is_some_and(|t| t.chars().count() > MAX_AGENT_DESC) {
                    bad(format!("description: at most {MAX_AGENT_DESC} characters"), &mut rej);
                    continue;
                }
                if desc.is_some() && (x.kind == "question" || x.kind == "risk") {
                    bad(format!("a {}'s answer or response is op 'answer' (the verifier checks it)", x.kind), &mut rej);
                    continue;
                }
                if x.ac.len() + add > MAX_AC {
                    bad(format!("{node} would have more than {MAX_AC} criteria: create a second story"), &mut rej);
                    continue;
                }
                e["title"] = pr["title"].clone();
                e["ac_add"] = pr["ac_add"].clone();
                e["description"] = json!(desc);
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
            "answer" => {
                let text = pr["text"].as_str().map(str::trim).filter(|t| !t.is_empty());
                if x.kind != "question" && x.kind != "risk" {
                    bad(format!("answer is for a question (its answer) or a risk (its response); {node} is a {}", x.kind), &mut rej);
                    continue;
                }
                let Some(text) = text else {
                    bad("answer needs its text: the answer, or what is done about the risk".into(), &mut rej);
                    continue;
                };
                if text.chars().count() > MAX_AGENT_DESC {
                    bad(format!("text: at most {MAX_AGENT_DESC} characters"), &mut rej);
                    continue;
                }
                if let Some(f) = ["likelihood", "impact"].into_iter().find(|f| !pr[*f].is_null() && (x.kind != "risk" || level_of(&pr[*f]).is_none())) {
                    bad(format!("{f} is for a risk: one of {LEVELS:?}"), &mut rej);
                    continue;
                }
                e["text"] = json!(text);
                if x.kind == "risk" {
                    e["internal_owner"] = json!(canonical(&p, pr["internal_owner"].as_str()));
                    e["likelihood"] = pr["likelihood"].clone();
                    e["impact"] = pr["impact"].clone();
                }
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
            "answer" if x.is_some_and(|y| y.kind == "risk") => format!("what is done about this risk: {}", e["text"].as_str().unwrap_or("")),
            "answer" => format!("this question is answered: {}", e["text"].as_str().unwrap_or("")),
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
            "internal_owner" | "client_owner" | "delegated_by" | "decided_by" | "type" if !(v.is_null() || v.is_string()) => bail!("{k} must be text"),
            "likelihood" | "impact" if !(v.is_null() || level_of(v).is_some()) => bail!("{k} is null or one of {LEVELS:?}"),
            "links" => check_links(v)?,
            "description" if !(v.is_null() || v.as_str().is_some_and(|t| t.chars().count() <= MAX_DESC)) => bail!("description is text of at most {MAX_DESC} characters"),
            k if !HUMAN_FIELDS.contains(&k) => bail!("editable fields: {HUMAN_FIELDS:?}"),
            _ => {}
        }
    }
    Ok(())
}

/// An edit the user made (`{op, node, set|text|ac|met|drop|depends_on|into|target, …}` or a create). A link
/// to an item in another thread is written in both ledgers (`link_threads`), and so is its removal — an
/// undo of one included.
pub fn human_event(project: &str, body: &Value) -> Result<Value> {
    match body["op"].as_str().unwrap_or("") {
        "xref" => {
            let master_here = body["master"] == "here";
            return link_threads(project, body["node"].as_str().unwrap_or(""), body["thread"].as_str().unwrap_or(""), body["target"].as_str().unwrap_or(""), body["kind"].as_str().unwrap_or("same"), master_here);
        }
        "unxref" => return unlink_threads(project, body["node"].as_str().unwrap_or(""), body["thread"].as_str().unwrap_or(""), body["target"].as_str().unwrap_or("")),
        "undo" => {
            let evs = Ledger::of(project)?.events()?;
            if let Some(x) = evs.iter().find(|e| e["mid"] == body["target"] && e["type"] == "human" && (e["op"] == "xref" || e["op"] == "unxref")) {
                let (node, thread, target) = (x["node"].as_str().unwrap_or(""), x["thread"].as_str().unwrap_or(""), x["target"].as_str().unwrap_or(""));
                // The pair's latest link event: undoing an older one would undo something else.
                let latest = evs.iter().rev().find(|e| e["type"] == "human" && (e["op"] == "xref" || e["op"] == "unxref") && e["node"] == node && e["thread"] == thread && e["target"] == target);
                if latest.is_some_and(|l| l["mid"] != x["mid"]) {
                    bail!("this link was changed since: undo the newer change, or link / unlink it again");
                }
                if x["op"] == "xref" {
                    return unlink_threads(project, node, thread, target);
                }
                // Undo an unlink: link them again as they were.
                let before = evs.iter().rev().find(|e| e["type"] == "human" && e["op"] == "xref" && e["node"] == node && e["thread"] == thread && e["target"] == target).ok_or_else(|| anyhow!("nothing to link again"))?;
                return link_threads(project, node, thread, target, before["kind"].as_str().unwrap_or("same"), before["master"] == "here");
            }
        }
        "edit" => {
            // A mirror's state, owners and due are its master's (#20): changed there, or they would never show.
            let node = body["node"].as_str().unwrap_or("");
            let raw = fold(&Ledger::of(project)?)?;
            let master = raw.nodes.get(node).and_then(|x| x.xrefs.iter().find(|r| r["kind"] == "same" && r["master"] == "there").cloned());
            if let (Some(m), Some(set)) = (master, body["set"].as_object()) {
                let (theirs, mine): (serde_json::Map<String, Value>, serde_json::Map<String, Value>) =
                    set.clone().into_iter().partition(|(k, _)| MIRRORED.contains(&k.as_str()));
                let mut out = Value::Null;
                if !theirs.is_empty() {
                    out = human_event_here(m["thread"].as_str().unwrap_or(""), &json!({"op": "edit", "node": m["node"], "set": theirs}))?;
                }
                if !mine.is_empty() {
                    out = human_event_here(project, &json!({"op": "edit", "node": node, "set": mine}))?;
                }
                return Ok(out);
            }
        }
        _ => {}
    }
    human_event_here(project, body)
}

/// A mirror's fields that are its master's (#20).
const MIRRORED: [&str; 6] = ["state", "internal_owner", "client_owner", "due", "waiting_for", "delegated_by"];

/// Link `node` here to `target` in `thread` (#20), in both ledgers. `same`: one piece of work, its master
/// here or there; `depends_on`: this one waits on that one (the other records `blocks`).
pub fn link_threads(project: &str, node: &str, thread: &str, target: &str, kind: &str, master_here: bool) -> Result<Value> {
    if thread == project {
        bail!("a link to an item in the same thread is a dependency (link), not a cross-thread link");
    }
    if !["same", "depends_on"].contains(&kind) {
        bail!("kind is same or depends_on");
    }
    // Both items must be there before either side is written.
    for (p, id) in [(project, node), (thread, target)] {
        let t = fold(&Ledger::of(p)?)?;
        if t.nodes.get(id).is_none_or(|x| CLOSED.contains(&x.state.as_str())) {
            bail!("{p} has no open item {id}");
        }
    }
    let (here, there) = match kind {
        "same" => (if master_here { "here" } else { "there" }, if master_here { "there" } else { "here" }),
        _ => ("there", "there"),
    };
    let back = if kind == "depends_on" { "blocks" } else { "same" };
    let e = human_event_here(project, &json!({"op": "xref", "node": node, "thread": thread, "target": target, "kind": kind, "master": here}))?;
    if let Err(err) = human_event_here(thread, &json!({"op": "xref", "node": target, "thread": project, "target": node, "kind": back, "master": there})) {
        // Never a link on one side only.
        let _ = human_event_here(project, &json!({"op": "unxref", "node": node, "thread": thread, "target": target}));
        return Err(err.context(format!("{thread} couldn't take the link")));
    }
    Ok(e)
}

/// Take a cross-thread link away, in both ledgers: each item is on its own again, with its own last values.
pub fn unlink_threads(project: &str, node: &str, thread: &str, target: &str) -> Result<Value> {
    let e = human_event_here(project, &json!({"op": "unxref", "node": node, "thread": thread, "target": target}))?;
    if Ledger::of(thread).is_ok() {
        let _ = human_event_here(thread, &json!({"op": "unxref", "node": target, "thread": project, "target": node}));
    }
    Ok(e)
}

fn human_event_here(project: &str, body: &Value) -> Result<Value> {
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
        check_links(&body["links"])?;
        for f in ["likelihood", "impact"] {
            if !(body[f].is_null() || (k == "risk" && level_of(&body[f]).is_some())) {
                bail!("{f} is for a risk: one of {LEVELS:?}");
            }
        }
        if !(body["description"].is_null() || body["description"].as_str().is_some_and(|t| t.chars().count() <= MAX_DESC)) {
            bail!("description is text of at most {MAX_DESC} characters");
        }
        let c = evs.iter().filter(|x| x["type"] == "create" && x["kind"] == k).count() + 1;
        e = json!({"type": "create", "id": new_id(k, c), "kind": k, "by": "human", "evidence": last, "date": date, "at": now,
                   "fields": {"title": body["title"].as_str().filter(|t| !t.trim().is_empty()).unwrap_or("Untitled"),
                              "parent": if k == "epic" { None } else { par }, "type": body["type"].as_str().or(if k == "task" { Some("action") } else { None }),
                              "ac": body["ac"].as_array().cloned().unwrap_or_default(), "plan": if k == "epic" { Some("Off-plan") } else { None },
                              "internal_owner": body["internal_owner"], "delegated_by": body["delegated_by"], "due": body["due"], "links": body["links"], "description": body["description"],
                              "decided_by": body["decided_by"], "likelihood": body["likelihood"], "impact": body["impact"]}});
    } else if ["undo", "accept", "reject"].contains(&op) {
        let target = body["target"].as_str().ok_or_else(|| anyhow!("target mid required"))?;
        e["target"] = json!(target);
    } else {
        let node = body["node"].as_str().filter(|x| n.contains_key(*x)).ok_or_else(|| anyhow!("unknown node {}", body["node"]))?;
        e["node"] = json!(node);
        for k in ["set", "text", "ac", "met", "drop", "depends_on", "into", "thread", "target", "kind", "master"] {
            if !body[k].is_null() {
                e[k] = body[k].clone();
            }
        }
        if (op == "xref" || op == "unxref") && !(body["thread"].as_str().is_some_and(|t| !t.is_empty() && t != project) && body["target"].is_string()) {
            bail!("{op} needs another thread and its item (thread, target)");
        }
        if op == "xref" && !(XREF_KINDS.contains(&body["kind"].as_str().unwrap_or("")) && ["here", "there"].contains(&body["master"].as_str().unwrap_or("there"))) {
            bail!("kind is one of {XREF_KINDS:?}, master here or there");
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

/// Status changes an agent made from a mirror's own evidence (#20): applied, verified, in its history — the
/// ones that move work along (not mentions).
fn status_moves(x: &Node) -> Vec<Value> {
    x.history.iter().filter(|h| h["type"] == "agent" && h["op"] == "status" && h["applied"] == true && h["verdict"] != "unsupported").cloned().collect()
}

/// When a history entry happened, comparable across entries: a call's day and time for an agent's, the
/// moment the user acted for theirs.
fn when(h: &Value) -> String {
    match h["at"].as_str().filter(|_| h["by"] == "human") {
        Some(at) => at.chars().take(19).collect(),
        None => format!("{}T{}", h["date"].as_str().unwrap_or(""), h["ts"].as_str().unwrap_or("00:00:00")),
    }
}

/// A ledger folded with its master items taking the status changes their mirrors' threads heard (one
/// level: the mirrors' own ledgers as they are) — newer than the master's own last word on its state.
fn fold_forwarded(project: &str) -> Result<Tree> {
    let mut t = fold(&Ledger::of(project)?)?;
    let mut mirrors: BTreeMap<String, Option<Tree>> = BTreeMap::new();
    let ids: Vec<String> = t.nodes.keys().cloned().collect();
    let mut moved = false;
    for id in ids {
        let refs: Vec<Value> = t.nodes[&id].xrefs.iter().filter(|r| r["kind"] == "same" && r["master"] == "here").cloned().collect();
        for r in refs {
            let (th, node) = (r["thread"].as_str().unwrap_or("").to_string(), r["node"].as_str().unwrap_or("").to_string());
            let other = mirrors.entry(th.clone()).or_insert_with(|| Ledger::of(&th).and_then(|l| fold(&l)).ok());
            let Some(m) = other.as_ref().and_then(|o| o.nodes.get(&node)).cloned() else { continue };
            let x = t.nodes.get_mut(&id).unwrap();
            x.also_in.push(json!({"thread": th, "node": node, "title": m.title}));
            // A merged or deleted master takes nothing.
            if x.state == "merged" || x.state == "deleted" {
                continue;
            }
            let last_here = x.history.iter().filter(|h| h["op"] == "status" || (h["op"] == "edit" && h["set"]["state"].is_string())).map(when).max().unwrap_or_default();
            for h in status_moves(&m).into_iter().filter(|h| when(h) > last_here) {
                if apply_status(x, &h) {
                    let mut h = h.clone();
                    h["from"] = json!({"thread": th, "node": node});
                    x.history.push(h);
                    moved = true;
                }
            }
            x.due = x.due_history.last().and_then(|h| s(&h["due"]));
        }
    }
    if moved {
        derive(&mut t.nodes);
    }
    Ok(t)
}

/// A ledger as the window, Tasks and the extractor read it (#20): masters with their mirrors' status
/// changes; each mirror showing its master's state, owners, due and last history entries; each item that
/// waits on an open item of another thread with it in `xwaits`.
pub fn fold_linked(project: &str) -> Result<Tree> {
    let mut t = fold_forwarded(project)?;
    let mut others: BTreeMap<String, Option<Tree>> = BTreeMap::new();
    let mut get = |th: &str, node: &str| -> Option<Node> { others.entry(th.to_string()).or_insert_with(|| fold_forwarded(th).ok()).as_ref().and_then(|t| t.nodes.get(node).cloned()) };
    let ids: Vec<String> = t.nodes.keys().cloned().collect();
    for id in &ids {
        let refs = t.nodes[id].xrefs.clone();
        for r in refs {
            let (th, node, kind) = (r["thread"].as_str().unwrap_or("").to_string(), r["node"].as_str().unwrap_or("").to_string(), r["kind"].as_str().unwrap_or(""));
            let Some(mut o) = get(&th, &node) else { continue };
            // An item that is itself a mirror stands for its master (one more step).
            if let Some(m) = o.xrefs.iter().find(|x| x["kind"] == "same" && x["master"] == "there").cloned() {
                if let Some(mm) = get(m["thread"].as_str().unwrap_or(""), m["node"].as_str().unwrap_or("")) {
                    o.state = mm.state;
                    o.internal_owner = mm.internal_owner;
                    o.client_owner = mm.client_owner;
                    o.due = mm.due;
                }
            }
            let x = t.nodes.get_mut(id).unwrap();
            match kind {
                "same" if r["master"] == "there" => {
                    x.state = o.state.clone();
                    x.internal_owner = o.internal_owner.clone();
                    x.client_owner = o.client_owner.clone();
                    x.delegated_by = o.delegated_by.clone();
                    x.due = o.due.clone();
                    x.waiting_for = o.waiting_for.clone();
                    let last: Vec<Value> = o.history.iter().rev().take(3).rev().cloned().collect();
                    // A master in a closed thread isn't on Tasks: the mirror is shown there instead.
                    let closed_thread = crate::projects::load(&th).map(|p| p.stage.closed()).unwrap_or(true);
                    x.mirror_of = Some(json!({"thread": th, "node": node, "title": o.title, "history": last, "closed_thread": closed_thread}));
                }
                "depends_on" if !closed(&o.state) => {
                    let owner = o.internal_owner.clone().or(o.client_owner.clone());
                    x.xwaits.push(json!({"thread": th, "node": node, "title": o.title, "owner": owner, "state": o.state}));
                }
                _ => {}
            }
        }
    }
    // A local wait on a mirror is a wait on its master's work (Tasks shows the master, not the mirror).
    for id in &ids {
        let waits: Vec<String> = t.nodes[id].waiting_on.clone();
        for w in waits {
            let Some(m) = t.nodes.get(&w).and_then(|y| y.mirror_of.clone()).filter(|m| m["closed_thread"] != true) else { continue };
            let (state, owner) = (t.nodes[&w].state.clone(), t.nodes[&w].internal_owner.clone());
            let x = t.nodes.get_mut(id).unwrap();
            x.waiting_on.retain(|v| v != &w);
            if !closed(&state) {
                x.xwaits.push(json!({"thread": m["thread"], "node": m["node"], "title": m["title"], "owner": owner, "state": state}));
            }
        }
    }
    Ok(t)
}

/// Open stories, tasks and questions of every open thread (but `not`) whose title has every word of `q`
/// (no model; at most 20, the threads' newest first): what an item can be linked to (#20).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn search_open(q: &str, not: Option<&str>) -> Vec<Value> {
    let words: Vec<String> = norm(q).split_whitespace().map(str::to_owned).collect();
    if words.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for p in crate::projects::list().into_iter().filter(|p| !p.stage.closed() && Some(p.id.as_str()) != not) {
        let Ok(t) = Ledger::of(&p.id).and_then(|l| fold(&l)) else { continue };
        for x in t.nodes.values().filter(|x| ["story", "task", "question"].contains(&x.kind.as_str()) && !closed(&x.state)) {
            let title = norm(&x.title);
            if words.iter().all(|w| title.contains(w.as_str())) || x.id.eq_ignore_ascii_case(q.trim()) {
                out.push(json!({"thread": p.id, "thread_name": p.name, "id": x.id, "title": x.title, "kind": x.kind, "state": x.state, "owner": x.internal_owner.as_ref().or(x.client_owner.as_ref())}));
            }
        }
        if out.len() >= 20 {
            break;
        }
    }
    out.truncate(20);
    out
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
    let t = fold_linked(project)?;
    let pend = pending(project)?.len();
    let mut nodes = serde_json::to_value(&t.nodes)?;
    for (_, x) in nodes.as_object_mut().into_iter().flatten() {
        if let Some(d) = x["description"].as_str().map(crate::minutes::markdown_html) {
            x["description_html"] = json!(d);
        }
    }
    Ok(json!({
        "nodes": nodes, "notes": t.notes,
        "processed": t.order.len(), "pending": pend,
        // Each evidence item's call: history entries name the call they came from.
        "evidence": t.evidence.iter().map(|(k, v)| (k.clone(), json!({"title": v["title"], "call": v["call"], "date": v["date"], "kind": v["kind"], "url": v["url"], "from": v["turns"][0]["speaker"].as_str().filter(|_| v["kind"] == "mail")}))).collect::<serde_json::Map<String, Value>>(),
        "roots": t.nodes.values().filter(|x| x.kind == "epic" && x.state != "merged" && x.state != "deleted").map(|x| &x.id).collect::<Vec<_>>(),
    }))
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

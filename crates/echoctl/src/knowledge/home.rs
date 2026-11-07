//! Home (spec-2 K11, D20): a vault's first tab — what's new, what waits, how healthy the vault is, and asking right
//! there. Drawn only from what Echo already holds (ruling R48): the calendar events it cached, Health's cached checks,
//! the ingest ledger, `activity.jsonl` and the conversations — never Microsoft 365, the network or a model. git is only
//! read (`rev-parse`, `log`, `diff`, `ls-files`, `symbolic-ref`), on a thread of its own: the first answer never waits
//! for it, and the window asks again until git's part is there (K11-10). Home writes nothing to the vault.
//!
//! *What's new* (K11-04) is the last [`DAYS`] days: Echo's own lines first (minutes, ingest, Curate, Health, edits in
//! Echo, the starter), then git's commits Echo didn't make and its uncommitted changes Echo didn't write, then — in a
//! vault git can't answer for — the index's times Echo's lines don't explain. A page changed no later than
//! [`SLACK_MS`] after Echo's last line for it is Echo's own write (the line is written after the file).

#![cfg_attr(not(feature = "tray"), allow(dead_code))] // the window's route reads it (`tray`); the tests always

use anyhow::Result;
use chrono::{DateTime, FixedOffset, Local, NaiveDate};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::activity::{self, Entry};
use super::health::{self, By, Finding, Group};
use super::scan::{self, Index, Page};
use super::{checks, fm, git, ingest, links, sources, starter, Vault};

/// What's new looks this many days back.
pub const DAYS: i64 = 14;
/// What's new shows this many rows (per filter).
pub const ROWS: usize = 12;
/// Commits git log reads at most: a long history isn't read past them.
const COMMITS: usize = 200;
/// A page changed this soon after Echo's line for it is Echo's own write.
const SLACK_MS: i64 = 2000;
/// git's answer is used this long before it is read again (in the background).
const GIT_FRESH: Duration = Duration::from_secs(20);
/// Conversations under *Continue* (K11-03).
const CONTINUE: usize = 3;

/// A calendar event as Home reads it.
#[derive(Debug, Clone)]
pub struct Event {
    pub title: String,
    pub start: DateTime<FixedOffset>,
    /// The attendees' and the organizer's addresses.
    pub emails: Vec<String>,
}

impl Event {
    fn of(m: &crate::meeting::MeetingContext) -> Option<Event> {
        let start = DateTime::parse_from_rfc3339(m.start.as_deref()?).ok()?;
        let emails = m.attendees.iter().chain(m.organizer.iter()).filter_map(|p| p.email.clone()).collect();
        Some(Event { title: m.subject.clone().unwrap_or_default(), start, emails })
    }
}

/// The calendar events Echo holds now — today's agenda and, in the window, its calendar's last fetch; never a new one.
pub fn calendar() -> Vec<Event> {
    #[allow(unused_mut)] // without the window there is only the agenda
    let mut held = crate::watch::agenda();
    #[cfg(feature = "tray")]
    held.extend(crate::window::cached_meetings());
    let mut seen = HashSet::new();
    held.iter().filter(|m| seen.insert((m.id.clone(), m.start.clone()))).filter_map(Event::of).collect()
}

/// A row of What's new.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// Vault-relative; empty for the whole vault (the starter's row).
    pub path: String,
    pub title: String,
    /// `new` or `edit`.
    pub kind: String,
    /// Lines added and removed; none when only the file's time says it changed.
    pub plus: Option<usize>,
    pub minus: Option<usize>,
    /// When (RFC 3339).
    pub at: String,
    /// `minutes`, `ingest`, `ask`, `health`, `you`, `starter`, `outside` or `commit`.
    pub by: String,
    /// Who, in words: Echo's minutes, Ingest, Ask (Curate), Health fix, You in Echo, Echo, Changed outside Echo — or a
    /// commit's author.
    pub who: String,
    /// What it came from, in words — others' words among them (a commit's message, a source's name).
    pub detail: String,
    #[serde(skip)]
    t: i64,
}

// ── git, read in the background ──

/// A commit of the last days, as git log read it.
#[derive(Debug, Clone)]
pub(super) struct Commit {
    id: String,
    short: String,
    author: String,
    at: String,
    t: i64,
    subject: String,
    /// Vault-relative paths, lines added and removed (none for a binary file), and whether the commit made the file.
    files: Vec<(String, Option<usize>, Option<usize>, bool)>,
}

/// What git says of a vault: its last commits (newest first, at most [`COMMITS`]), the files changed and not committed,
/// the branch and the last commit.
#[derive(Debug, Clone, Default)]
pub(super) struct GitFacts {
    commits: Vec<Commit>,
    /// Vault-relative, lines added and removed (none: git can't count them), and whether git doesn't know the file yet.
    dirty: Vec<(String, Option<usize>, Option<usize>, bool)>,
    branch: Option<String>,
    /// (short id, message, when).
    last: Option<(String, String, String)>,
}

/// `added<TAB>removed<TAB>path` (`-` for a binary file). A quoted path (a tab, a newline or a quote in its name) is
/// left out.
fn numstat(line: &str) -> Option<(String, Option<usize>, Option<usize>)> {
    let mut f = line.splitn(3, '\t');
    let (a, d, p) = (f.next()?, f.next()?, f.next()?);
    let n = |x: &str| if x == "-" { Some(None) } else { x.parse().ok().map(Some) };
    (!p.is_empty() && !p.starts_with('"')).then_some(())?;
    Some((p.to_string(), n(a)?, n(d)?))
}

/// One commit of `git log`'s answer: its line of fields, its `--numstat` lines (relative to the vault) and its
/// `--summary` lines (relative to the repository's top: `prefix` is the vault's folder there).
fn commit_of(chunk: &str, prefix: &str) -> Option<Commit> {
    let mut lines = chunk.lines();
    let head: Vec<&str> = lines.next()?.splitn(5, '\u{1f}').collect();
    let [id, short, author, at, subject] = head[..] else { return None };
    let t = DateTime::parse_from_rfc3339(at).ok()?.timestamp_millis();
    let (mut files, mut made) = (vec![], HashSet::new());
    for l in lines {
        if let Some(rest) = l.strip_prefix(" create mode ") {
            if let Some(p) = rest.split_once(' ').and_then(|(_, p)| p.strip_prefix(prefix)) {
                made.insert(p.to_string());
            }
        } else if let Some(f) = numstat(l) {
            files.push(f);
        }
    }
    let files = files.into_iter().map(|(p, a, d)| { let new = made.contains(&p); (p, a, d, new) }).collect();
    Some(Commit { id: id.into(), short: short.into(), author: author.into(), at: at.into(), t, subject: subject.into(), files })
}

/// What git says of the vault in `root` — reads only; `None` when it isn't in a repository git can read.
pub(super) fn read_git(root: &Path, days: i64) -> Option<GitFacts> {
    #[cfg(test)]
    std::thread::sleep(*lock(&DELAY));
    let prefix = git::out(root, &["rev-parse", "--show-prefix"])?;
    let (since, most) = (format!("--since={days}.days"), format!("-n{COMMITS}"));
    let log = git::out(root, &["-c", "core.quotepath=off", "log", &since, &most, "--numstat", "--summary", "--no-renames", "--relative", "--format=%x1e%H%x1f%h%x1f%an%x1f%cI%x1f%s", "--", "."]).unwrap_or_default();
    let commits = log.split('\u{1e}').filter_map(|c| commit_of(c, &prefix)).collect();
    // Against the last commit (a repository without one: every file is git's news).
    let diff = git::out(root, &["-c", "core.quotepath=off", "diff", "--numstat", "--no-renames", "--relative", "HEAD", "--", "."]).unwrap_or_default();
    let mut dirty: Vec<_> = diff.lines().filter_map(numstat).map(|(p, a, d)| (p, a, d, false)).collect();
    let others = git::run(root, &["ls-files", "--others", "--exclude-standard", "-z", "--", "."]).filter(|o| o.status.success()).map(|o| o.stdout).unwrap_or_default();
    for p in String::from_utf8_lossy(&others).split('\0').filter(|p| !p.is_empty() && p.ends_with(".md")) {
        let lines = scan::read_capped(&super::join(root, p)).map(|t| t.lines().count());
        dirty.push((p.to_string(), lines, lines.map(|_| 0), true));
    }
    let branch = git::out(root, &["symbolic-ref", "--short", "-q", "HEAD"]).filter(|b| !b.is_empty());
    let last = git::out(root, &["log", "-1", "--format=%h%x1f%s%x1f%cI", "--", "."]).and_then(|l| {
        let f: Vec<&str> = l.splitn(3, '\u{1f}').collect();
        (f.len() == 3).then(|| (f[0].to_string(), f[1].to_string(), f[2].to_string()))
    });
    Some(GitFacts { commits, dirty, branch, last })
}

/// What git said of a vault (by its folder and id): for which revision of its pages, when, and whether a read goes.
struct Held {
    rev: String,
    at: Instant,
    got: Option<Got>,
    reading: bool,
}

#[derive(Clone)]
enum Got {
    Repo(Arc<GitFacts>),
    NotRepo,
}

type Reads = HashMap<(PathBuf, String), Held>;
static GIT: Mutex<Option<Reads>> = Mutex::new(None);
#[cfg(test)]
static DELAY: Mutex<Duration> = Mutex::new(Duration::ZERO);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A folder git may answer for: git is there, and the folder or one above it has a `.git` (nothing is run to know).
fn in_repo(root: &Path) -> bool {
    git::program().is_some() && root.ancestors().any(|d| d.join(".git").exists())
}

/// What git said of the vault, never waiting for it: `ready` (read for these pages within [`GIT_FRESH`]), `pending` (a
/// read goes on its own thread; what it read before, if anything, meanwhile) or `none` (no repository).
fn git_now(v: &Vault, ix: &Index) -> (Option<Arc<GitFacts>>, &'static str) {
    if !in_repo(&v.root) {
        return (None, "none");
    }
    let key = (v.root.clone(), v.id.clone());
    let mut all = lock(&GIT);
    let h = all.get_or_insert_with(HashMap::new).entry(key.clone()).or_insert_with(|| Held { rev: String::new(), at: Instant::now(), got: None, reading: false });
    let fresh = h.got.is_some() && h.rev == ix.rev && h.at.elapsed() < GIT_FRESH;
    if !fresh && !h.reading {
        h.reading = true;
        let (root, rev) = (v.root.clone(), ix.rev.clone());
        std::thread::spawn(move || {
            // Its end is kept even when the read panics: the next ask starts another.
            struct Done((PathBuf, String), String, Option<Got>);
            impl Drop for Done {
                fn drop(&mut self) {
                    if let Some(h) = lock(&GIT).as_mut().and_then(|a| a.get_mut(&self.0)) {
                        h.reading = false;
                        if let Some(g) = self.2.take() {
                            *h = Held { rev: std::mem::take(&mut self.1), at: Instant::now(), got: Some(g), reading: false };
                        }
                    }
                }
            }
            let mut done = Done(key, rev, None);
            done.2 = Some(read_git(&root, DAYS).map_or(Got::NotRepo, |f| Got::Repo(Arc::new(f))));
        });
    }
    match &h.got {
        Some(Got::NotRepo) => (None, "none"),
        Some(Got::Repo(f)) => (Some(f.clone()), if fresh { "ready" } else { "pending" }),
        None => (None, "pending"),
    }
}

/// (tests) Until the guard drops, every read of git waits `by` first: the first answer must not wait for it.
#[cfg(test)]
pub(crate) fn delay_git(by: Duration) -> DelayGuard {
    *lock(&DELAY) = by;
    DelayGuard
}

#[cfg(test)]
pub(crate) struct DelayGuard;

#[cfg(test)]
impl Drop for DelayGuard {
    fn drop(&mut self) {
        *lock(&DELAY) = Duration::ZERO;
    }
}

// ── What's new ──

/// An ingest run's sources by their file names, for its rows.
fn run_sources(v: &Vault) -> HashMap<String, Vec<String>> {
    let Ok(l) = sources::ledger(v) else { return HashMap::new() };
    l.runs
        .iter()
        .filter_map(|r| {
            let names = r["sources"].as_array()?.iter().filter_map(Value::as_str).map(|s| s.rsplit('/').next().unwrap_or(s).to_string()).collect();
            Some((r["id"].as_str()?.to_string(), names))
        })
        .collect()
}

/// Echo's line `e` as a row's `by`, who and what it came from.
fn echo_who(e: &Entry, runs: &HashMap<String, Vec<String>>) -> (&'static str, &'static str, String) {
    match e.by.as_str() {
        "minutes" => ("minutes", "Echo's minutes", String::new()),
        "ingest" => {
            let from = e.r#ref.as_ref().and_then(|r| runs.get(r)).filter(|s| !s.is_empty()).map(|s| match s.len() {
                1 | 2 => format!("from {}", s.join(", ")),
                n => format!("from {}, {} and {} more", s[0], s[1], n - 2),
            });
            ("ingest", "Ingest", from.unwrap_or_default())
        }
        "ask" => ("ask", "Ask (Curate)", String::new()),
        "health" => ("health", "Health fix", String::new()),
        _ => ("you", "You in Echo", String::new()),
    }
}

/// What's new from Echo's lines `acts`, git's facts (when the vault is a repository) and — `mtimes` — the index's times,
/// newest first: the newest `max` rows, and the newest `max` new pages and notes too, so each of the window's filters
/// has its own (All · New pages · Notes).
fn rows(v: &Vault, ix: &Index, acts: &[Entry], git: Option<&GitFacts>, mtimes: bool, since: i64, max: usize) -> Vec<Row> {
    let page = |rel: &str| scan::page_rel(v, rel).and_then(|p| ix.pages.get(&p));
    let ms = |at: &str| DateTime::parse_from_rfc3339(at).ok().map(|t| t.timestamp_millis());
    let local = |t: i64| DateTime::from_timestamp_millis(t).map(|d| d.with_timezone(&Local).to_rfc3339()).unwrap_or_default();
    // When Echo last wrote each page; the starter wrote them all.
    let (mut wrote, mut whole) = (HashMap::new(), None::<i64>);
    for e in acts {
        let Some(t) = ms(&e.at) else { continue };
        if e.path.is_empty() {
            whole = whole.max(Some(t));
        } else {
            let w = wrote.entry(e.path.as_str()).or_insert(t);
            *w = (*w).max(t);
        }
    }
    let echos = |rel: &str, mtime: i64| wrote.get(rel).copied().max(whole).is_some_and(|w| mtime <= w + SLACK_MS);
    let runs = run_sources(v);
    let mut out = vec![];
    for e in acts {
        // A source isn't a page: Coming in says what came in.
        let Some(t) = ms(&e.at).filter(|t| *t >= since && e.by != "source") else { continue };
        if e.path.is_empty() {
            let commit = e.commit.as_ref().map(|c| format!(" · the first commit {c}")).unwrap_or_default();
            let pages = starter::FILES.iter().filter(|(f, _)| scan::is_page_path(v, f)).count();
            out.push(Row { path: String::new(), title: "The vault".into(), kind: "new".into(), plus: None, minus: None, at: e.at.clone(), by: "starter".into(), who: "Echo".into(), detail: format!("{pages} pages · from Echo's twin starter{commit}"), t });
            continue;
        }
        let Some(p) = page(&e.path) else { continue };
        let (by, who, detail) = echo_who(e, &runs);
        out.push(Row { path: e.path.clone(), title: p.title.clone(), kind: e.kind.clone(), plus: Some(e.plus), minus: Some(e.minus), at: e.at.clone(), by: by.into(), who: who.into(), detail, t });
    }
    if let Some(g) = git {
        // Echo's own commits are its lines' already.
        let echo: Vec<&str> = acts.iter().filter_map(|e| e.commit.as_deref()).filter(|c| !c.is_empty()).collect();
        for c in g.commits.iter().filter(|c| c.t >= since && !echo.iter().any(|e| c.id.starts_with(e))) {
            for (path, plus, minus, new) in &c.files {
                let Some(p) = page(path) else { continue };
                out.push(Row { path: path.clone(), title: p.title.clone(), kind: if *new { "new" } else { "edit" }.into(), plus: *plus, minus: *minus, at: c.at.clone(), by: "commit".into(), who: c.author.clone(), detail: format!("commit {} · “{}”", c.short, c.subject), t: c.t });
            }
        }
        for (path, plus, minus, new) in &g.dirty {
            let Some(p) = page(path).filter(|p| p.mtime >= since && !echos(path, p.mtime)) else { continue };
            out.push(Row { path: path.clone(), title: p.title.clone(), kind: if *new { "new" } else { "edit" }.into(), plus: *plus, minus: *minus, at: local(p.mtime), by: "outside".into(), who: "Changed outside Echo".into(), detail: "not committed".into(), t: p.mtime });
        }
    }
    if mtimes {
        for p in ix.pages.values().filter(|p| p.mtime >= since) {
            let rel = scan::vault_rel(v, &p.path);
            if !echos(&rel, p.mtime) {
                out.push(Row { path: rel, title: p.title.clone(), kind: "edit".into(), plus: None, minus: None, at: local(p.mtime), by: "outside".into(), who: "Changed outside Echo".into(), detail: String::new(), t: p.mtime });
            }
        }
    }
    out.sort_by(|a, b| b.t.cmp(&a.t).then_with(|| a.path.cmp(&b.path)));
    let notes = v.path("notes").filter(|n| !n.is_empty()).map(|n| format!("{n}/"));
    let filters: [&dyn Fn(&Row) -> bool; 3] = [&|_| true, &|r| r.kind == "new" && !r.path.is_empty(), &|r| notes.as_deref().is_some_and(|n| r.path.starts_with(n))];
    let mut keep = vec![false; out.len()];
    for f in filters {
        for (i, _) in out.iter().enumerate().filter(|(_, r)| f(r)).take(max) {
            keep[i] = true;
        }
    }
    out.into_iter().zip(keep).filter_map(|(r, k)| k.then_some(r)).collect()
}

/// What's new (K11-04): the last `days` days, newest first, at most `max` rows a filter — git read here and now, as the
/// tests ask it; the window's route reads git in the background instead (`view`).
#[allow(dead_code)]
pub fn whats_new(v: &Vault, days: i64, max: usize) -> Vec<Row> {
    let Ok(ix) = scan::index(v) else { return vec![] };
    let since = (Local::now() - chrono::Duration::days(days)).timestamp_millis();
    let acts = activity::recent(v, "1970-01-01T00:00:00+00:00");
    let git = if in_repo(&v.root) { read_git(&v.root, days) } else { None };
    rows(v, &ix, &acts, git.as_ref(), git.is_none(), since, max)
}

// ── the Ask box's suggestions ──

/// A client page — in a `clients` folder or tagged `client` — with what a meeting is matched by: its names (title,
/// file name, aliases), its domains (frontmatter `domain`, `domains`, `website`) and its file name.
struct Client<'a> {
    page: &'a Page,
    names: Vec<String>,
    domains: Vec<String>,
    stem: String,
}

/// `needle` in `hay` on word boundaries.
fn has_phrase(hay: &str, needle: &str) -> bool {
    !needle.is_empty()
        && hay.match_indices(needle).any(|(i, _)| {
            let before = hay[..i].chars().next_back();
            let after = hay[i + needle.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
}

/// `https://www.acme.example/` → `acme.example`.
fn bare_domain(d: &str) -> String {
    let d = d.trim().to_lowercase();
    let d = d.trim_start_matches("https://").trim_start_matches("http://").trim_start_matches('@').trim_start_matches("www.");
    d.split('/').next().unwrap_or("").to_string()
}

impl Client<'_> {
    /// Is the meeting about this client: its title names it, or someone in it writes from the client's domain (or a
    /// domain named as the page is: `sam@acme.example` for `clients/acme.md`).
    fn about(&self, e: &Event) -> bool {
        let title = e.title.to_lowercase();
        self.names.iter().any(|n| has_phrase(&title, n))
            || e.emails.iter().filter_map(|m| m.rsplit_once('@').map(|(_, d)| d.to_lowercase())).any(|d| {
                let label = d.split('.').next().unwrap_or("");
                self.domains.iter().any(|x| d == *x || d.ends_with(&format!(".{x}"))) || (label.chars().count() >= 3 && (label == self.stem || self.stem.split(['-', '_', ' ']).next() == Some(label)))
            })
    }
}

fn clients<'a>(ix: &'a Index, in_scope: &impl Fn(&str) -> bool) -> Vec<Client<'a>> {
    let client = |p: &Page| p.path.rsplit('/').skip(1).any(|f| f.eq_ignore_ascii_case("clients")) || p.tags.iter().any(|t| t.eq_ignore_ascii_case("client"));
    ix.pages
        .values()
        .filter(|p| in_scope(&p.path) && client(p))
        .map(|p| {
            let stem = p.path.rsplit('/').next().unwrap_or(&p.path).trim_end_matches(".md").to_lowercase();
            let mut names = vec![p.title.to_lowercase(), stem.replace(['-', '_'], " ")];
            names.extend(fm::get(&p.props, "aliases").map(|a| a.items()).unwrap_or_default().iter().map(|a| a.to_lowercase()));
            let mut names: Vec<String> = names.into_iter().map(|n| n.trim().to_string()).filter(|n| n.chars().count() >= 3).collect();
            names.dedup();
            let domains = ["domain", "domains", "website"].iter().filter_map(|k| fm::get(&p.props, k)).flat_map(|x| x.items()).map(|d| bare_domain(&d)).filter(|d| d.contains('.')).collect();
            Client { page: p, names, domains, stem }
        })
        .collect()
}

/// A note's day: its `date`, else the date its file name starts with.
fn note_day(p: &Page) -> Option<NaiveDate> {
    let day = |s: &str| s.get(..10).and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    fm::get(&p.props, "date").and_then(|d| day(d.text().trim())).or_else(|| day(p.path.rsplit('/').next().unwrap_or("")))
}

/// The newest note of the notes folder (by its day, then its time).
fn newest_note<'a>(v: &Vault, ix: &'a Index, in_scope: &impl Fn(&str) -> bool) -> Option<&'a Page> {
    let folder = v.path("notes").and_then(|n| scan::page_rel(v, &n)).filter(|n| !n.is_empty())?;
    let prefix = format!("{folder}/");
    ix.pages.values().filter(|p| p.path.starts_with(&prefix) && in_scope(&p.path)).max_by(|a, b| (note_day(a), a.mtime, &a.path).cmp(&(note_day(b), b.mtime, &b.path)))
}

/// A folder named action-items (any case, `_` or a space for the dash) and how many pages it holds, if it holds any.
fn action_items(ix: &Index, in_scope: &impl Fn(&str) -> bool) -> Option<(String, usize)> {
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for p in ix.pages.keys().filter(|p| in_scope(p)) {
        let parts: Vec<&str> = p.split('/').collect();
        if let Some(i) = parts[..parts.len() - 1].iter().position(|f| f.to_lowercase().replace(['_', ' '], "-") == "action-items") {
            *by.entry(parts[..=i].join("/")).or_default() += 1;
        }
    }
    by.into_iter().next()
}

/// A day as Home says it: today, yesterday, 3 Jan (2025 when not this year's).
fn day_word(d: NaiveDate, today: NaiveDate) -> String {
    if d == today {
        "today".into()
    } else if today.pred_opt() == Some(d) {
        "yesterday".into()
    } else if d.format("%Y").to_string() == today.format("%Y").to_string() {
        d.format("%-d %b").to_string()
    } else {
        d.format("%-d %b %Y").to_string()
    }
}

fn suggest(v: &Vault, ix: &Index, calendar: &[Event], findings: &[Finding], now: DateTime<Local>) -> Vec<Value> {
    let in_scope = checks::in_scope(v);
    let page = |p: &Page| json!({ "path": scan::vault_rel(v, &p.path), "title": p.title });
    let today = now.date_naive();
    let mut out = vec![];
    // The next meeting today or tomorrow that a client page is about.
    let clients = clients(ix, &in_scope);
    let mut soon: Vec<(DateTime<Local>, &Event)> = calendar.iter().map(|e| (e.start.with_timezone(&Local), e)).filter(|(d, e)| *d >= now && (d.date_naive() == today || today.succ_opt() == Some(d.date_naive())) && !e.title.trim().is_empty()).collect();
    soon.sort_by_key(|(d, _)| *d);
    if let Some((d, e, c)) = soon.iter().find_map(|(d, e)| clients.iter().find(|c| c.about(e)).map(|c| (d, e, c))) {
        let day = if d.date_naive() == today { "today" } else { "tomorrow" };
        out.push(json!({ "kind": "meeting", "text": format!("Brief me for {} at {}", e.title.trim(), d.format("%H:%M")), "why": format!("{day} · matches {}", c.page.path), "pages": [page(c.page)] }));
    }
    if let Some(n) = newest_note(v, ix, &in_scope) {
        let day = note_day(n).or_else(|| DateTime::from_timestamp_millis(n.mtime).map(|t| t.with_timezone(&Local).date_naive())).unwrap_or(today);
        out.push(json!({ "kind": "note", "text": format!("What did we decide in {}?", n.title), "why": format!("the newest note · {}", day_word(day, today)), "pages": [page(n)] }));
    }
    if let Some((folder, n)) = action_items(ix, &in_scope) {
        out.push(json!({ "kind": "actions", "text": "Which action items are still open?", "why": format!("{folder}/ · {n} {}", if n == 1 { "page" } else { "pages" }), "pages": [] }));
    }
    if let Some(f) = findings.iter().find(|f| f.kind == "contradiction") {
        let mut seen = HashSet::new();
        let pages: Vec<Value> = f.pages.iter().filter(|(p, _)| seen.insert(p.as_str())).map(|(p, _)| json!({ "path": p, "title": scan::page_rel(v, p).and_then(|r| ix.pages.get(&r)).map(|x| x.title.clone()) })).collect();
        out.push(json!({ "kind": "contradiction", "text": format!("{} — which is right?", f.title), "why": "Health found two pages that disagree", "pages": pages }));
    }
    out
}

/// The Ask box's suggestions (K11-02), at most four, in this order and each only when its source is there: the next
/// meeting today or tomorrow a client page is about, the newest note, an action-items folder with pages, a
/// contradiction Health holds. None is invented.
pub fn suggestions(v: &Vault, calendar: &[Event], findings: &[Finding]) -> Vec<Value> {
    scan::index(v).map(|ix| suggest(v, &ix, calendar, findings, Local::now())).unwrap_or_default()
}

// ── the cards ──

/// Ask's last conversations in the vault (K11-03): title, the mode Ask last ran it in, when, and whether the terminal
/// panel has it (open there now, or the panel's own conversation). In the window's build only (ruling R49).
#[cfg(feature = "tray")]
fn conversations(v: &Vault) -> Vec<Value> {
    let list = super::chat::list_value(v);
    list.as_array()
        .map(|a| {
            a.iter()
                .take(CONTINUE)
                .map(|c| {
                    let id = c["id"].as_str().unwrap_or("");
                    let panel = c["live"] != true && (c["current"] == true || crate::terminal::live_session(id));
                    json!({ "id": id, "title": c["title"], "at": c["at"], "mode": mode_of(v, id), "terminal": panel })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(feature = "tray"))]
fn conversations(_v: &Vault) -> Vec<Value> {
    vec![]
}

/// The mode Ask last ran conversation `id` in (its guard's scope, in Echo's folder of it), if Ask ran it.
fn mode_of(v: &Vault, id: &str) -> Option<String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return None;
    }
    let f = super::cache_dir(&v.id).join("chat").join(id).join("settings.json");
    let j: Value = serde_json::from_slice(&std::fs::read(f).ok()?).ok()?;
    j["env"][crate::guard::RUN_CHAT].as_str().map(str::to_owned)
}

/// The Health card (K11-05): the score, the last 30 days, whether only code has checked, the first three of *Needs
/// you*, and what code can fix by kind (most first). With the findings shown, for the suggestions.
fn health_card(v: &Vault, ix: &Index) -> (Value, Vec<Finding>) {
    let (shown, _, score, state) = health::scored(v, ix);
    let needs: Vec<&Finding> = shown.iter().filter(|f| f.group == Group::Needs).collect();
    let mut kinds: Vec<(String, usize)> = vec![];
    for f in shown.iter().filter(|f| f.by == By::Code && f.fixable) {
        match kinds.iter_mut().find(|(k, _)| *k == f.kind) {
            Some(k) => k.1 += 1,
            None => kinds.push((f.kind.clone(), 1)),
        }
    }
    kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
    let from = (Local::now().date_naive() - chrono::Duration::days(29)).format("%Y-%m-%d").to_string();
    let history: Vec<&health::Point> = state.history.iter().filter(|p| p.day >= from).collect();
    let top: Vec<Value> = needs.iter().take(3).map(|f| json!({ "key": f.key, "title": f.title, "kind": f.kind, "by": f.by, "pages": f.pages })).collect();
    let card = json!({
        "score": score.total, "code_only": score.code_only, "history": history, "ai": state.ai.as_ref().map(|a| &a.at),
        "needs": needs.len(), "top": top, "fixable": kinds.iter().map(|(_, n)| n).sum::<usize>(), "fixable_kinds": kinds,
    });
    (card, shown)
}

/// Coming in (K11-06): the changes waiting for review with their sources, the sources not processed by kind (Echo's
/// order), the run going with its step now.
fn coming_in(v: &Vault, ing: &Result<Value>) -> Value {
    let d = match ing {
        Ok(d) => d,
        Err(e) => return json!({ "error": format!("{e:#}") }),
    };
    let none = vec![];
    let srcs = d["sources"].as_array().unwrap_or(&none);
    let title = |p: &str| srcs.iter().find(|s| s["path"] == p).and_then(|s| s["title"].as_str()).unwrap_or_else(|| p.rsplit('/').next().unwrap_or(p)).to_string();
    let titles = |r: &Value| r["sources"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(title).collect::<Vec<_>>()).unwrap_or_default();
    let runs = d["runs"].as_array().unwrap_or(&none);
    let review: Vec<Value> = runs.iter().filter(|r| r["state"] == "review" && r["to_answer"].as_u64().unwrap_or(0) > 0).map(|r| json!({ "id": r["id"], "to_answer": r["to_answer"], "sources": titles(r) })).collect();
    let waiting: u64 = review.iter().filter_map(|r| r["to_answer"].as_u64()).sum();
    let kinds: Vec<(&str, usize)> = ["echo", "mail", "teams", "web", "file"].iter().map(|k| (*k, srcs.iter().filter(|s| s["state"] == "new" && s["kind"] == *k).count())).filter(|(_, n)| *n > 0).collect();
    let running = d["running"].as_str().map(|id| {
        let r = runs.iter().find(|r| r["id"] == id).cloned().unwrap_or_default();
        json!({ "id": id, "step": ingest::step_now(v, id), "waiting": r["waiting"], "sources": titles(&r) })
    });
    json!({ "waiting": waiting, "runs": review, "new": kinds.iter().map(|(_, n)| n).sum::<usize>(), "kinds": kinds, "running": running, "offer": d["offer"] })
}

/// Most linked (K11-07): the index with the pages it links to, and the four pages most pages link to (hubs left out).
fn hubs(v: &Vault, ix: &Index) -> Value {
    let index = v.path("index").and_then(|i| scan::page_rel(v, &i)).filter(|i| ix.pages.contains_key(i));
    let g = links::graph(ix, index.as_deref(), "links");
    let hubs: HashSet<&str> = g["hubs"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let (mut into, mut listed) = (HashMap::<&str, usize>::new(), 0);
    for e in g["edges"].as_array().into_iter().flatten() {
        let (Some(a), Some(b)) = (e[0].as_str(), e[1].as_str()) else { continue };
        *into.entry(b).or_default() += 1;
        if Some(a) == index.as_deref() {
            listed += 1;
        }
    }
    let mut top: Vec<(&str, usize)> = into.into_iter().filter(|(p, _)| !hubs.contains(p)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let pages: Vec<Value> = top.into_iter().take(4).map(|(p, n)| json!({ "path": scan::vault_rel(v, p), "title": ix.pages.get(p).map(|x| x.title.clone()), "in": n })).collect();
    json!({ "index": index.as_deref().map(|i| json!({ "path": scan::vault_rel(v, i), "links": listed })), "pages": pages })
}

/// The vault card (K11-08): where it is, its index and size, the AI profile, what it holds — and git's branch and last
/// commit once git answered.
fn vault_card(v: &Vault, ix: &Index, git: Option<&GitFacts>) -> Value {
    let index = v.path("index");
    let bytes = index.as_ref().and_then(|i| std::fs::metadata(super::join(&v.root, i)).ok()).map(|m| m.len());
    let tags = checks::tag_taxonomy(v).and_then(|t| std::fs::read_to_string(super::join(&v.root, &t)).ok()).map(|t| checks::tags_in_order(&t).len())
        .unwrap_or_else(|| ix.pages.values().flat_map(|p| p.tags.iter()).collect::<HashSet<_>>().len());
    json!({
        "root": v.root, "pages_root": v.path("pages"), "obsidian": v.detected.obsidian.is_some(), "index": index, "index_bytes": bytes,
        "profile": v.profile, "profile_name": super::profile_name(&v.profile), "pages": ix.pages.len(), "tags": tags, "skills": v.detected.skills.len(),
        "git": git.map(|g| json!({ "branch": g.branch, "last": g.last.as_ref().map(|(id, subject, at)| json!({ "id": id, "subject": subject, "at": at })) })),
    })
}

/// A vault Echo just made from its starter and nothing since (K11-09): only the starter's line in its activity, no
/// sources, every page one of the starter's and unchanged — Home shows its three steps.
fn fresh(v: &Vault, ix: &Index, acts: &[Entry], ing: &Result<Value>) -> bool {
    if v.template.as_deref() != Some(starter::TEMPLATE) || acts.iter().any(|e| e.by != "starter") {
        return false;
    }
    let Some(made) = acts.iter().find_map(|e| DateTime::parse_from_rfc3339(&e.at).ok()).map(|t| t.timestamp_millis()) else { return false };
    let no_sources = ing.as_ref().is_ok_and(|d| d["sources"].as_array().is_some_and(|s| s.is_empty()));
    no_sources && ix.pages.values().all(|p| p.mtime <= made + SLACK_MS && starter::FILES.iter().any(|(f, _)| *f == scan::vault_rel(v, &p.path)))
}

/// Home as the window shows it (K11): `{rev, fresh, suggestions, continue, whats_new: {rows, git, notes}, health,
/// coming_in, hubs, vault}` — from Echo's caches, never waiting for git (`whats_new.git`: `ready`, `pending` while it
/// reads, or `none`).
pub fn view(v: &Vault) -> Result<Value> {
    let ix = scan::index(v)?;
    let now = Local::now();
    let since = (now - chrono::Duration::days(DAYS)).timestamp_millis();
    let acts = activity::recent(v, "1970-01-01T00:00:00+00:00");
    let (git, state) = git_now(v, &ix);
    let news = rows(v, &ix, &acts, git.as_deref(), state == "none", since, ROWS);
    let (health, shown) = health_card(v, &ix);
    let ing = ingest::value(v);
    Ok(json!({
        "rev": ix.rev,
        "fresh": fresh(v, &ix, &acts, &ing),
        "suggestions": suggestions(v, &calendar(), &shown),
        "continue": conversations(v),
        "whats_new": { "rows": news, "git": state, "notes": v.path("notes") },
        "health": health,
        "coming_in": coming_in(v, &ing),
        "hubs": hubs(v, &ix),
        "vault": vault_card(v, &ix, git.as_deref()),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::health::{By, Group};
    use crate::knowledge::{activity, diff, fixture, git, AddOpts};
    use chrono::{Duration, Local};
    use std::process::Command;
    use std::time::{Instant, SystemTime};

    fn vault(root: &Path) -> Vault {
        let t = fixture::twin(&root.join("twin"));
        crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
    }

    /// Every file under `dir` last changed `days` days ago (a fixture written just now would look changed today).
    fn age(dir: &Path, days: i64) {
        let when = SystemTime::now() - std::time::Duration::from_secs(days as u64 * 86_400);
        fn walk(d: &Path, when: SystemTime) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if e.file_type().unwrap().is_dir() {
                    if e.file_name() != ".git" {
                        walk(&p, when);
                    }
                } else {
                    std::fs::File::options().write(true).open(&p).unwrap().set_modified(when).unwrap();
                }
            }
        }
        walk(dir, when);
    }

    fn set_mtime(p: &Path, at: DateTime<Local>) {
        std::fs::File::options().write(true).open(p).unwrap().set_modified(SystemTime::from(at)).unwrap();
    }

    /// Echo wrote `rel` at `at` (the file, then its activity line — as the savers do).
    fn echo_wrote(v: &Vault, rel: &str, text: &str, by: &str, r#ref: Option<&str>, at: DateTime<Local>) {
        let p = crate::knowledge::join(&v.root, rel);
        let old = std::fs::read_to_string(&p).ok();
        fixture::write(&v.root, rel, text);
        set_mtime(&p, at);
        let (plus, minus) = diff::counts(&diff::lines(old.as_deref().unwrap_or(""), text));
        let e = activity::Entry { at: at.to_rfc3339(), path: rel.into(), kind: if old.is_some() { "edit" } else { "new" }.into(), by: by.into(), r#ref: r#ref.map(str::to_owned), plus, minus, commit: None };
        activity::record(v, e).unwrap();
    }

    /// git in `dir`, as `who` at `at` (author and committer), with the test's config only.
    fn git_as(dir: &Path, who: (&str, &str), at: DateTime<Local>, args: &[&str]) {
        let date = at.to_rfc3339();
        let o = Command::new(git::program().unwrap())
            .arg("-C")
            .arg(dir)
            .args(["-c", &format!("user.name={}", who.0), "-c", &format!("user.email={}", who.1)])
            .args(args)
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    }

    fn finding(kind: &str, title: &str, pages: &[&str]) -> Finding {
        Finding {
            key: format!("{kind}:{}", pages.join(",")),
            kind: kind.into(),
            group: Group::Needs,
            by: By::Ai,
            pages: pages.iter().map(|p| (p.to_string(), Some(3))).collect(),
            title: title.into(),
            why: "Two pages state different facts about the same thing.".into(),
            fixable: false,
            fix_all: false,
            subject: Default::default(),
        }
    }

    fn texts(s: &[Value]) -> Vec<String> {
        s.iter().map(|x| x["text"].as_str().unwrap_or("").to_string()).collect()
    }

    // K11-T1
    #[test]
    fn suggestions_come_only_from_what_is_there() {
        let (_env, root) = crate::paths::testenv::folders("kn-home-sugg");
        let v = vault(&root);
        fixture::write(&v.root, "knowledge/action-items/README.md", "# Action items\n");
        fixture::write(&v.root, "knowledge/action-items/send-the-plan.md", "---\ntitle: Send the plan\n---\n# Send the plan\n");
        fixture::write(&v.root, "knowledge/action-items/book-the-room.md", "---\ntitle: Book the room\n---\n# Book the room\n");
        let soon = Local::now() + Duration::hours(1);
        let ev = |title: &str, start: DateTime<Local>, emails: &[&str]| Event { title: title.into(), start: start.fixed_offset(), emails: emails.iter().map(|e| e.to_string()).collect() };
        let calendar = [
            ev("Dentist", soon - Duration::minutes(30), &["front-desk@smiles.example"]),
            ev("Acme Corp · Sprint review", soon, &["alex@example.com"]),
            ev("Acme Corp · Retro", soon + Duration::hours(2), &[]),
        ];
        let found = [finding("contradiction", "Acme go-live: 12 Dec or 9 Jan", &["knowledge/clients/acme.md", "knowledge/notes/2026-01-02-b.md"])];
        let s = suggestions(&v, &calendar, &found);
        let kinds: Vec<&str> = s.iter().map(|x| x["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["meeting", "note", "actions", "contradiction"], "{s:?}");
        assert_eq!(texts(&s), [
            format!("Brief me for Acme Corp · Sprint review at {}", soon.format("%H:%M")),
            "What did we decide in Third sync?".to_string(),
            "Which action items are still open?".to_string(),
            "Acme go-live: 12 Dec or 9 Jan — which is right?".to_string(),
        ]);
        assert!(s[0]["why"].as_str().unwrap().ends_with("matches clients/acme.md"), "{}", s[0]["why"]);
        assert_eq!(s[0]["pages"][0]["path"], "knowledge/clients/acme.md");
        assert_eq!(s[1]["pages"][0]["path"], "knowledge/notes/2026-01-03-c.md", "the newest note by its date");
        assert_eq!(s[2]["why"], "action-items/ · 2 pages", "its README isn't one");
        assert_eq!(s[3]["pages"].as_array().unwrap().len(), 2);

        // By an attendee's domain: a meeting whose title names no client.
        let by_domain = [ev("Weekly sync", soon, &["sam@acme.example"])];
        assert_eq!(suggestions(&v, &by_domain, &[])[0]["text"], format!("Brief me for Weekly sync at {}", soon.format("%H:%M")));
        // A meeting past, in three days, or matching no client page: none for it.
        let none = [ev("Acme Corp · Kickoff", Local::now() - Duration::hours(2), &[]), ev("Acme Corp · Planning", Local::now() + Duration::days(3), &[]), ev("Dentist", soon, &["a@smiles.example"])];
        assert_eq!(suggestions(&v, &none, &[])[0]["kind"], "note", "no meeting suggestion");

        // A vault with none of them: nothing is invented.
        let bare = root.join("bare");
        fixture::write(&bare, "Hello.md", "# Hello\n");
        let b = crate::knowledge::add(&bare, AddOpts { name: Some("Bare".into()), ..Default::default() }).unwrap();
        assert_eq!(suggestions(&b, &[], &[]), Vec::<Value>::new());
        assert_eq!(suggestions(&b, &calendar, &[finding("stale", "An old line", &["Hello.md"])]), Vec::<Value>::new(), "no client page, no contradiction");
    }

    /// A row as (path, by, who, plus, minus).
    type Who = (String, String, String, Option<usize>, Option<usize>);

    fn whos(rows: &[Row]) -> Vec<Who> {
        rows.iter().map(|r| (r.path.clone(), r.by.clone(), r.who.clone(), r.plus, r.minus)).collect()
    }

    // K11-T2
    #[test]
    fn whats_new_says_who_changed_what() {
        let (_env, root) = crate::paths::testenv::folders("kn-home-who");
        let _iso = git::tests::isolated(&root, true);
        if git::program().is_none() {
            return;
        }
        let v = vault(&root);
        age(&v.root, 30);
        let ago = |m: i64| Local::now() - Duration::minutes(m);
        assert!(git::run(&v.root, &["init", "-q", "-b", "main"]).unwrap().status.success());
        git_as(&v.root, ("Test User", "test@example.com"), Local::now() - Duration::days(30), &["add", "-A"]);
        git_as(&v.root, ("Test User", "test@example.com"), Local::now() - Duration::days(30), &["commit", "-q", "-m", "first"]);
        echo_wrote(&v, "knowledge/notes/2026-11-05-weekly-sync.md", "---\ntitle: Weekly sync\n---\n# Weekly sync\n\nWe agreed.\n", "minutes", Some("2026-11-05_090012-weekly-sync"), ago(50));
        echo_wrote(&v, "knowledge/clients/acme.md", "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp\n\nSee [[2026-01-01-a]].\n\nPilot moves to 18 November.\n", "ingest", Some("20261107-0951"), ago(40));
        echo_wrote(&v, "knowledge/notes/2026-01-03-c.md", "---\ntitle: Third sync\ntags: [meeting]\n---\n# Third\n\n- Fix forward.\n- No rollback.\n", "ask", Some("6f2c9e1a-1b2c-4d5e-8f90-123456789abc"), ago(30));
        fixture::write(&v.root, "knowledge/topics/pricing.md", "---\ntitle: Pricing\n---\n# Pricing\n\nPer stream.\nPer seat.\n");
        git_as(&v.root, ("Dana Reyes", "dana@northwind.example"), ago(20), &["add", "--", "knowledge/topics/pricing.md"]);
        git_as(&v.root, ("Dana Reyes", "dana@northwind.example"), ago(20), &["commit", "-q", "-m", "pricing draft"]);
        let outside = crate::knowledge::join(&v.root, "knowledge/notes/2026-01-02-b.md");
        std::fs::write(&outside, "---\ntitle: Second sync\ntags: [meeting]\n---\n# Second sync, edited in Obsidian\n").unwrap();
        set_mtime(&outside, ago(10));

        let rows = whats_new(&v, 14, 12);
        assert_eq!(whos(&rows), [
            ("knowledge/notes/2026-01-02-b.md".into(), "outside".into(), "Changed outside Echo".into(), Some(1), Some(1)),
            ("knowledge/topics/pricing.md".into(), "commit".into(), "Dana Reyes".into(), Some(7), Some(0)),
            ("knowledge/notes/2026-01-03-c.md".into(), "ask".into(), "Ask (Curate)".into(), Some(3), Some(0)),
            ("knowledge/clients/acme.md".into(), "ingest".into(), "Ingest".into(), Some(2), Some(0)),
            ("knowledge/notes/2026-11-05-weekly-sync.md".into(), "minutes".into(), "Echo's minutes".into(), Some(6), Some(0)),
        ], "{rows:#?}");
        assert_eq!([rows[1].kind.as_str(), rows[4].kind.as_str(), rows[3].kind.as_str()], ["new", "new", "edit"]);
        assert_eq!(rows[1].title, "Pricing");
        assert!(rows[1].detail.contains("pricing draft"), "the commit's message: {}", rows[1].detail);
        assert_eq!(rows[0].detail, "not committed");
    }

    // K11-T3
    #[test]
    fn a_vault_without_git_says_what_echo_did_and_what_changed_outside() {
        let (_env, root) = crate::paths::testenv::folders("kn-home-nogit");
        let v = vault(&root);
        age(&v.root, 30);
        let ago = |m: i64| Local::now() - Duration::minutes(m);
        echo_wrote(&v, "knowledge/notes/2026-11-05-weekly-sync.md", "---\ntitle: Weekly sync\n---\n# Weekly sync\n", "minutes", None, ago(30));
        echo_wrote(&v, "knowledge/notes/2026-01-02-b.md", "---\ntitle: Second sync\ntags: [meeting]\n---\n# Second sync\n\nSee [[acme]].\n", "health", None, ago(20));
        let outside = crate::knowledge::join(&v.root, "knowledge/clients/acme.md");
        std::fs::write(&outside, "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp, edited elsewhere\n").unwrap();
        set_mtime(&outside, ago(10));
        let rows = whats_new(&v, 14, 12);
        assert_eq!(whos(&rows), [
            ("knowledge/clients/acme.md".into(), "outside".into(), "Changed outside Echo".into(), None, None),
            ("knowledge/notes/2026-01-02-b.md".into(), "health".into(), "Health fix".into(), Some(2), Some(0)),
            ("knowledge/notes/2026-11-05-weekly-sync.md".into(), "minutes".into(), "Echo's minutes".into(), Some(4), Some(0)),
        ], "{rows:#?}");
    }

    #[test]
    fn whats_new_reads_a_bounded_log() {
        let (_env, root) = crate::paths::testenv::folders("kn-home-bounded");
        let _iso = git::tests::isolated(&root, true);
        if git::program().is_none() {
            return;
        }
        let v = vault(&root);
        assert!(git::run(&v.root, &["init", "-q", "-b", "main"]).unwrap().status.success());
        git_as(&v.root, ("Test User", "test@example.com"), Local::now() - Duration::hours(4), &["add", "-A"]);
        git_as(&v.root, ("Test User", "test@example.com"), Local::now() - Duration::hours(4), &["commit", "-q", "-m", "the twin"]);
        // Then 10,000 commits by git fast-import, each changing one of 50 notes, the newest a minute ago.
        let now = Local::now().timestamp() - 60;
        let mut stream = String::new();
        for i in 0..10_000i64 {
            let body = format!("---\ntitle: Note {}\n---\n# Note {}\n\nVersion {i}.\n", i % 50, i % 50);
            let msg = format!("note {i}");
            let from = if i == 0 { "from refs/heads/main^0\n" } else { "" };
            stream.push_str(&format!("commit refs/heads/main\ncommitter Dana Reyes <dana@northwind.example> {} +0000\ndata {}\n{msg}\n{from}", now - (10_000 - i), msg.len()));
            stream.push_str(&format!("M 100644 inline knowledge/notes/n{}.md\ndata {}\n{body}\n", i % 50, body.len()));
        }
        let mut c = Command::new(git::program().unwrap()).arg("-C").arg(&v.root).args(["fast-import", "--quiet"]).stdin(std::process::Stdio::piped()).spawn().unwrap();
        use std::io::Write as _;
        c.stdin.take().unwrap().write_all(stream.as_bytes()).unwrap();
        assert!(c.wait().unwrap().success());
        for n in 0..50 {
            let last = 10_000 - 50 + n;
            fixture::write(&v.root, &format!("knowledge/notes/n{}.md", last % 50), &format!("---\ntitle: Note {}\n---\n# Note {}\n\nVersion {last}.\n", last % 50, last % 50));
        }
        assert!(git::run(&v.root, &["add", "-A", "--", "knowledge/notes"]).unwrap().status.success());

        let t = Instant::now();
        let facts = read_git(&v.root, 14).expect("a repository");
        let took = t.elapsed();
        assert_eq!(facts.commits.len(), 200, "git log -n 200: the rest isn't read");
        assert!(took <= std::time::Duration::from_millis(300), "read in {took:?}");
        let rows = whats_new(&v, 14, 12);
        assert_eq!(rows.len(), 12, "the newest 12");
        assert_eq!(rows[0].path, "knowledge/notes/n49.md");
        assert_eq!(rows[0].who, "Dana Reyes");
    }

    #[test]
    fn home_draws_from_what_echo_holds() {
        let (_env, root) = crate::paths::testenv::folders("kn-home-view");
        let v = vault(&root);
        let h = view(&v).unwrap();
        for k in ["rev", "fresh", "suggestions", "continue", "whats_new", "health", "coming_in", "hubs", "vault"] {
            assert!(h.get(k).is_some(), "{k}: {h}");
        }
        assert_eq!(h["fresh"], false, "a connected vault isn't a new one");
        assert_eq!(h["whats_new"]["git"], "none");
        assert_eq!(h["health"]["code_only"], true);
        assert_eq!(h["hubs"]["index"]["path"], "knowledge/CATALOG.md");
        assert_eq!(h["hubs"]["pages"][0]["path"], "knowledge/clients/acme.md", "the most linked page, the index left out");
        assert_eq!(h["vault"]["pages"], 7);
        assert_eq!(h["coming_in"]["new"], 1, "the deck in raw_inbox/");
        assert_eq!(h["coming_in"]["kinds"], json!([["file", 1]]));
    }

    #[test]
    fn a_new_vaults_home_is_its_three_steps() {
        let (_env, root) = crate::paths::testenv::folders("kn-home-new");
        let v = crate::knowledge::starter::create("My twin", Some(root.join("my-twin")), false, "auto").unwrap().vault;
        let before = fixture::snapshot(&v.root);
        let h = view(&v).unwrap();
        assert_eq!(h["fresh"], true);
        assert_eq!(h["suggestions"], json!([]), "nothing to suggest yet");
        let rows = h["whats_new"]["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!((rows[0]["title"].as_str(), rows[0]["by"].as_str()), (Some("The vault"), Some("starter")));
        assert_eq!(fixture::snapshot(&v.root), before, "Home writes nothing to the vault");
        // A page changed outside Echo: it is the user's vault now.
        let p = crate::knowledge::join(&v.root, "knowledge/profile/DIGITAL_PROFILE.md");
        std::fs::write(&p, "# Me\n").unwrap();
        set_mtime(&p, Local::now() + Duration::seconds(5));
        assert_eq!(view(&v).unwrap()["fresh"], false);
    }
}

//! Health (spec K16, §5.7): what the code checks found, as findings whose keys stay the same across scans; the score;
//! fixes shown as diffs before anything is written, each written only over the text it was computed on (X-T8); the
//! findings the user ignored; one score point a day. Code checks run when Health, Home or the CLI asks, from Echo's
//! index, cached per revision; nothing here starts a model.
//!
//! `<cache>/health.json` — `{history: [{day, score, categories}], ignored: [key], ai: {at, run, cost}}` — is changed
//! under `<cache>/health.lock`, re-read first, written atomically.

#![cfg_attr(not(feature = "tray"), allow(dead_code))] // most of it serves the window's routes and actions (`tray`)

use anyhow::{anyhow, bail, Result};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::links::Resolver;
use super::scan::{self, Index, RawLink};
use super::{activity, catalog, checks, diff, git, page, Vault};

/// Where a finding is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    /// Only the user can decide.
    Needs,
    /// Most fixed by code.
    Should,
    /// When there is time.
    Tidy,
}

/// Who found it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum By {
    Code,
    Ai,
}

/// What a finding is about, for its fixes.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum Subject {
    #[default]
    None,
    /// A link as written (a broken link, an index row).
    Link(RawLink),
    /// A tag or a property's key.
    Word(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// `kind:` + its pages, sorted, + what it is about (the link as written, the tag, the key): the same across scans.
    pub key: String,
    pub kind: String,
    pub group: Group,
    pub by: By,
    /// Vault-relative, with the line when there is one.
    pub pages: Vec<(String, Option<u32>)>,
    pub title: String,
    /// What it means, in one sentence.
    pub why: String,
    /// Echo has a fix to show without asking anything.
    pub fixable: bool,
    /// *Fix all* applies that fix: it is no guess (a broken link's near name is offered only on its own finding).
    #[serde(default)]
    pub fix_all: bool,
    #[serde(skip)]
    pub(super) subject: Subject,
}

/// The code checks' kinds, in the order they are listed: the group and what a finding of the kind means.
const CODE: [(&str, Group, &str); 10] = [
    ("not_in_index", Group::Should, "The index is how you and agents find pages, and it doesn't list this one."),
    ("broken", Group::Should, "A link that leads nowhere loses the reader and hides a relation from the graph."),
    ("isolated", Group::Tidy, "Nothing links to this page and it links nowhere, so it is easy to miss."),
    ("no_props", Group::Should, "Without properties a page has no title, date or tags for the index and search."),
    ("unknown_tag", Group::Needs, "A tag the taxonomy doesn't allow splits one subject into several."),
    ("meeting_tag_missing", Group::Needs, "Echo tags meeting notes with it, and the taxonomy doesn't allow it."),
    ("dead_row", Group::Should, "The index lists a page that isn't there."),
    ("usual_prop", Group::Tidy, "Most pages in this folder have this property, and this one doesn't."),
    ("duplicate_title", Group::Needs, "Pages with one title are hard to tell apart in links and search."),
    ("oversized_index", Group::Should, "Agents read the index first; past 150 KB or 1,500 rows it costs more than it helps."),
];

/// The AI checks' kinds (spec K16-05; found by `lint`).
pub const AI_KINDS: [&str; 5] = ["contradiction", "possible_duplicate", "stale", "link_to_add", "missing_summary"];

/// §5.7: the categories, their weights and the kinds each counts. Isolated pages, links worth adding and missing
/// summaries count in none.
const CATEGORIES: [(&str, u32, &[&str]); 5] = [
    ("Links", 25, &["broken", "dead_row"]),
    ("Index", 20, &["not_in_index", "oversized_index"]),
    ("Properties", 15, &["no_props", "usual_prop"]),
    ("Consistency", 25, &["unknown_tag", "meeting_tag_missing", "duplicate_title", "contradiction", "possible_duplicate"]),
    ("Freshness", 15, &["stale"]),
];

/// Days of score points kept.
const HISTORY_DAYS: i64 = 90;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Score {
    /// 0–100.
    pub total: u32,
    /// (name, points, weight) of each category counted, in §5.7's order.
    pub categories: Vec<(String, f64, u32)>,
    /// No AI check has run: Freshness is left out.
    pub code_only: bool,
}

// ── findings ──

fn made(kind: &str, pages: Vec<(String, Option<u32>)>, about: Option<String>, title: String, fixable: bool, subject: Subject) -> Finding {
    made_sure(kind, pages, about, title, fixable, fixable, subject)
}

/// `fix_all`: *Fix all* may apply its fix (it is no guess).
fn made_sure(kind: &str, pages: Vec<(String, Option<u32>)>, about: Option<String>, title: String, fixable: bool, fix_all: bool, subject: Subject) -> Finding {
    let (group, why) = CODE.iter().find(|(k, ..)| *k == kind).map_or((Group::Tidy, ""), |(_, g, w)| (*g, *w));
    let mut names: Vec<&str> = pages.iter().map(|(p, _)| p.as_str()).collect();
    names.sort();
    names.dedup();
    let mut key = format!("{kind}:{}", names.join(","));
    if let Some(a) = about {
        key.push(':');
        key.push_str(&a);
    }
    Finding { key, kind: kind.into(), group, by: By::Code, pages, title, why: why.into(), fixable, fix_all, subject }
}

/// A link as its key writes it: `[[…]]` (`![[…]]` for an embed) or `(…)`.
fn written(l: &RawLink) -> String {
    match (l.wiki, l.embed) {
        (true, false) => format!("[[{}]]", l.target),
        (true, true) => format!("![[{}]]", l.target),
        _ => format!("({})", l.target),
    }
}

/// A link as a finding's title names it.
fn shown(l: &RawLink) -> String {
    if l.wiki { format!("[[{}]]", l.target) } else { Resolver::bare(l) }
}

/// The same link written more than once on a page is one finding, with each line.
fn grouped(links: impl Iterator<Item = (String, RawLink)>) -> Vec<(String, Vec<u32>, RawLink)> {
    let mut by: BTreeMap<(String, String), (Vec<u32>, RawLink)> = BTreeMap::new();
    for (p, l) in links {
        let e = by.entry((p, written(&l))).or_insert_with(|| (vec![], l.clone()));
        let line = l.line as u32;
        if !e.0.contains(&line) {
            e.0.push(line);
        }
    }
    by.into_iter().map(|((p, ..), (lines, l))| (p, lines, l)).collect()
}

fn compute(v: &Vault, ix: &Index) -> Vec<Finding> {
    let c = checks::run(v, ix);
    let at = |p: &str| scan::vault_rel(v, p);
    let title = |p: &str| ix.pages.get(p).map_or_else(|| p.to_string(), |x| x.title.clone());
    let index = v.path("index").unwrap_or_default();
    let index_name = index.rsplit('/').next().unwrap_or(&index).to_string();
    let mut out = vec![];
    for p in &c.not_in_index {
        out.push(made("not_in_index", vec![(at(p), None)], None, format!("{} isn't in {index_name}", title(p)), true, Subject::None));
    }
    for (p, lines, l) in grouped(c.broken_links.iter().cloned()) {
        let near = closest(ix, Some(p.as_str()), &l, true);
        let (fixable, sure) = (near.is_some(), near.is_some_and(|n| !n.guess));
        out.push(made_sure("broken", lines.iter().map(|n| (at(&p), Some(*n))).collect(), Some(written(&l)), format!("{} leads to no page", shown(&l)), fixable, sure, Subject::Link(l)));
    }
    for p in &c.isolated {
        out.push(made("isolated", vec![(at(p), None)], None, format!("{} has no links", title(p)), false, Subject::None));
    }
    for p in &c.no_props {
        out.push(made("no_props", vec![(at(p), None)], None, format!("{} has no properties", title(p)), true, Subject::None));
    }
    for (p, t) in &c.unknown_tags {
        out.push(made("unknown_tag", vec![(at(p), None)], Some(format!("#{t}")), format!("#{t} isn't in the taxonomy"), false, Subject::Word(t.clone())));
    }
    if let (Some(t), Some(tax)) = (&c.meeting_tag_missing, checks::tag_taxonomy(v)) {
        out.push(made("meeting_tag_missing", vec![(tax, None)], Some(format!("#{t}")), format!("The taxonomy doesn't allow #{t}, the tag of meeting notes"), false, Subject::Word(t.clone())));
    }
    for (_, lines, l) in grouped(c.dead_rows.iter().map(|l| (index.clone(), l.clone()))) {
        out.push(made("dead_row", lines.iter().map(|n| (index.clone(), Some(*n))).collect(), Some(written(&l)), format!("{index_name} lists {}, which isn't there", shown(&l)), true, Subject::Link(l)));
    }
    for (p, k) in &c.usual_props {
        out.push(made("usual_prop", vec![(at(p), None)], Some(k.clone()), format!("{} has no {k}", title(p)), false, Subject::Word(k.clone())));
    }
    for ps in &c.duplicate_titles {
        out.push(made("duplicate_title", ps.iter().map(|p| (at(p), None)).collect(), None, format!("“{}” is the title of {} pages", title(&ps[0]), ps.len()), false, Subject::None));
    }
    if let Some(o) = &c.oversized_index {
        out.push(made("oversized_index", vec![(index.clone(), None)], None, format!("{index_name} is {} KB with {} rows", o.bytes / 1024, o.rows), false, Subject::None));
    }
    let rank = |f: &Finding| CODE.iter().position(|(k, ..)| *k == f.kind);
    out.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.key.cmp(&b.key)));
    out
}

type Held = HashMap<(PathBuf, String), (String, Arc<Vec<Finding>>)>;
static CACHE: Mutex<Option<Held>> = Mutex::new(None);

/// What the code checks read: the pages (the index's revision), the index file and the tag taxonomy (either may sit
/// outside the pages root), and the vault's settings.
fn revision(v: &Vault, ix: &Index) -> String {
    let stat = |rel: Option<String>| rel.and_then(|r| std::fs::metadata(super::join(&v.root, &r)).ok()).map(|m| (m.len(), m.modified().ok()));
    let all = format!("{}\n{:?}\n{:?}\n{}", ix.rev, stat(v.path("index")), stat(checks::tag_taxonomy(v)), serde_json::to_string(v).unwrap_or_default());
    page::sha(all.as_bytes())
}

/// The code checks' findings, computed once per revision of the vault.
pub(super) fn cached(v: &Vault, ix: &Index) -> Arc<Vec<Finding>> {
    let rev = revision(v, ix);
    let id = (v.root.clone(), v.id.clone());
    let held = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|c| c.get(&id).filter(|(r, _)| *r == rev).map(|(_, f)| f.clone()));
    if let Some(f) = held {
        return f;
    }
    let f = Arc::new(compute(v, ix));
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(id, (rev, f.clone()));
    f
}

#[allow(dead_code)] // read by Home and the AI checks (later tasks)
/// Every finding of the code checks, the ignored ones too.
pub fn findings(v: &Vault, ix: &Index) -> Vec<Finding> {
    (*cached(v, ix)).clone()
}

fn finding(v: &Vault, ix: &Index, key: &str) -> Result<Finding> {
    cached(v, ix).iter().find(|f| f.key == key).cloned().ok_or_else(|| anyhow!("that finding isn't there any more"))
}

/// The pages Health judges (as slice 1's checks: not the index, a taxonomy or a folder's README).
fn scope_pages(v: &Vault, ix: &Index) -> usize {
    let in_scope = checks::in_scope(v);
    ix.pages.keys().filter(|p| in_scope(p)).count()
}

// ── the score ──

/// §5.7: each category's points from the share of pages its findings touch; Freshness (AI kinds only) only once an AI
/// check has run; the total rounded half up.
pub fn score(findings: &[Finding], scope_pages: usize, ai_ran: bool) -> Score {
    let (mut categories, mut points, mut weights) = (vec![], 0.0, 0u32);
    for (name, weight, kinds) in CATEGORIES {
        if !ai_ran && kinds.iter().all(|k| AI_KINDS.contains(k)) {
            continue;
        }
        let touched: BTreeSet<&str> = findings.iter().filter(|f| kinds.contains(&f.kind.as_str())).flat_map(|f| f.pages.iter().map(|(p, _)| p.as_str())).collect();
        let p = f64::from(weight) * (1.0 - 5.0 * touched.len() as f64 / scope_pages.max(1) as f64).max(0.0);
        points += p;
        weights += weight;
        categories.push((name.to_string(), (p * 100.0).round() / 100.0, weight));
    }
    // The margin keeps a .5 that floating point holds as .4999… rounding up.
    let total = if weights == 0 { 100 } else { (100.0 * points / f64::from(weights) + 0.5 + 1e-9).floor() as u32 };
    Score { total: total.min(100), categories, code_only: !ai_ran }
}

// ── fixes ──

/// What a fix does to its file's text. Each finds its place in the text it is given, so several fixes of one file
/// apply one after another.
#[derive(Debug, Clone, PartialEq)]
enum Op {
    /// The link written to lead elsewhere: `to` is its new target as written (inside `[[…]]`, or the destination).
    Relink { link: RawLink, to: String },
    /// The link replaced by its text.
    Unlink(RawLink),
    /// The table row holding the link removed (an index row); elsewhere the link replaced by its text.
    DropRow(RawLink),
    /// Text put before the first line (properties).
    Prepend(String),
    /// A tag replaced on the page.
    Retag { from: String, to: String },
    /// A row put in the index: its folder, the page, its description and date (as `catalog::insert` takes them).
    Row(String, String, String, String),
}

/// A finding's fix, computed against its file as it is now: the file (vault-relative), that text's sha256 (`base`), the
/// line diff and the file's text after it.
#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub key: String,
    pub choice: String,
    pub path: String,
    pub base: String,
    pub diff: Vec<(char, String)>,
    pub text: String,
    pub guess: bool,
    ops: Vec<Op>,
}

impl Fix {
    /// As the window shows it: the changes with 3 lines around them (`hunks`, as permission cards), the counts, whether
    /// it rests on a guessed page, and the sha256 of the text it writes (`sha`), which the page sends back with `base`
    /// so that only the change it showed is written (ruling R37).
    pub fn value(&self) -> Value {
        let (plus, minus) = diff::counts(&self.diff);
        let line = |(k, l): &(char, String)| json!([k.to_string(), l]);
        let hunks: Vec<Vec<Value>> = diff::hunks(&self.diff, 3).iter().map(|h| h.iter().map(line).collect()).collect();
        json!({ "key": self.key, "choice": self.choice, "path": self.path, "base": self.base, "sha": page::sha(self.text.as_bytes()), "guess": self.guess, "hunks": hunks, "plus": plus, "minus": minus })
    }
}

/// One way to fix a finding.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    /// `to`, `remove`, `create`, `props`, `row`, `tag:<tag>` or `taxonomy`.
    pub choice: String,
    pub label: String,
    /// The page a link would lead to (vault-relative), and why Echo thinks it is the one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

fn strip_md(name: &str) -> &str {
    name.strip_suffix(".md").or_else(|| name.strip_suffix(".MD")).unwrap_or(name)
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// A name as compared for "case or spacing": lower case, spaces and underscores as dashes, runs of dashes as one.
fn norm_name(s: &str) -> String {
    let mut out = String::new();
    for c in super::links::percent_decode(s).to_lowercase().chars() {
        let c = if c.is_whitespace() || c == '_' { '-' } else { c };
        if !(c == '-' && out.ends_with('-')) {
            out.push(c);
        }
    }
    out.trim_matches('-').to_string()
}

/// Edits from `a` to `b` (insert, delete, replace one character).
fn distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The page a link that leads nowhere most likely means.
struct Near {
    /// Pages-root-relative.
    to: String,
    why: &'static str,
    /// A name some edits away, not the same name: offered on its own finding, never by *Fix all* (ruling R36 — Echo's
    /// dated notes are a few edits from each other).
    guess: bool,
}

/// The page a link that leads nowhere most likely means: the same name in another folder, the same name but for case
/// or spacing, or — with `guesses` — a name at most 2 edits away (and at most a third of it changed, so a short name
/// never matches anything). The nearest wins, then the shallowest and shortest path. `from`: the linking page, never
/// its own answer.
fn closest(ix: &Index, from: Option<&str>, link: &RawLink, guesses: bool) -> Option<Near> {
    let bare = Resolver::bare(link);
    let name = strip_md(bare.rsplit('/').next().unwrap_or(&bare)).trim().to_string();
    if name.is_empty() {
        return None;
    }
    let norm = norm_name(&name);
    let want: Vec<char> = norm.chars().collect();
    // (how near: 0 the same name, 1 case or spacing, 2 edits; the edits; the path's depth; its length)
    type Rank = (u8, usize, usize, usize);
    let mut best: Option<(Rank, &String, &'static str)> = None;
    for p in ix.pages.keys().filter(|p| Some(p.as_str()) != from) {
        let stem = strip_md(p.rsplit('/').next().unwrap_or(p));
        let (rank, why) = if stem == name {
            ((0, 0), "the same name in another folder")
        } else if stem.eq_ignore_ascii_case(&name) || norm_name(stem) == norm {
            ((1, 0), "the same name but for case or spacing")
        } else {
            let have: Vec<char> = norm_name(stem).chars().collect();
            if !guesses || have.len().abs_diff(want.len()) > 2 {
                continue;
            }
            match distance(&want, &have) {
                1 if want.len() >= 3 => ((2, 1), "one character apart"),
                2 if want.len() >= 6 => ((2, 2), "two characters apart"),
                _ => continue,
            }
        };
        let k = (rank.0, rank.1, p.matches('/').count(), p.len());
        if best.as_ref().is_none_or(|(b, bp, _)| (k, p) < (*b, *bp)) {
            best = Some((k, p, why));
        }
    }
    best.map(|((tier, ..), p, why)| Near { to: p.clone(), why, guess: tier == 2 })
}

/// `to` (vault-relative) from folder `from_dir` (vault-relative).
fn relative(from_dir: &str, to: &str) -> String {
    let a: Vec<&str> = from_dir.split('/').filter(|s| !s.is_empty()).collect();
    let b: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let mut parts = vec![".."; a.len() - common];
    parts.extend(&b[common..]);
    parts.join("/")
}

/// A wikilink's text as the reader sees it: its alias, else its name.
fn wiki_text(inner: &str) -> String {
    match inner.split_once('|') {
        Some((_, alias)) => alias.trim().to_string(),
        None => inner.split('#').next().unwrap_or(inner).trim().to_string(),
    }
}

/// `link`, in the file at `path` (vault-relative), written to lead to page `to` (pages-root-relative): a wikilink by
/// the page's path with what it showed as its alias; a Markdown link by its folder as written with the name corrected
/// when that leads there, else by the path from the file's folder. A `#heading` stays.
fn relinked(v: &Vault, ix: &Index, path: &str, link: &RawLink, to: &str) -> String {
    if link.wiki {
        let before = link.target.split('|').next().unwrap_or("");
        let anchor = before.find('#').map_or("", |i| &before[i..]);
        return format!("{}{anchor}|{}", strip_md(to), wiki_text(&link.target));
    }
    let raw = link.target.as_str();
    let (angle, inner) = raw.strip_prefix('<').and_then(|r| r.strip_suffix('>')).map_or((false, raw), |i| (true, i));
    let (dest, anchor) = inner.find('#').map_or((inner, ""), |i| (&inner[..i], &inner[i..]));
    let enc = |s: &str| if angle { s.to_string() } else { s.replace(' ', "%20") };
    let name = to.rsplit('/').next().unwrap_or(to);
    let same = dest.rsplit_once('/').map_or_else(|| enc(name), |(d, _)| format!("{d}/{}", enc(name)));
    let probe = RawLink { target: same.clone(), line: 0, wiki: false, embed: false };
    let leads = scan::page_rel(v, path).and_then(|from| Resolver::new(ix).resolve(&from, &probe)).as_deref() == Some(to);
    let new = if leads { same } else { enc(&relative(dir_of(path), &scan::vault_rel(v, to))) };
    if angle { format!("<{new}{anchor}>") } else { format!("{new}{anchor}") }
}

/// The `[[…]]` in a line whose inside, trimmed, is `target` (with its `!` for an embed): byte ranges.
fn wiki_spans(line: &str, target: &str, embed: bool) -> Vec<(usize, usize)> {
    let mut out = vec![];
    let mut at = 0;
    while let Some(i) = line[at..].find("[[") {
        let s = at + i;
        let Some(j) = line[s + 2..].find("]]") else { break };
        let e = s + 2 + j + 2;
        let bang = s > 0 && line.as_bytes()[s - 1] == b'!';
        if line[s + 2..e - 2].trim() == target && bang == embed {
            out.push((if bang { s - 1 } else { s }, e));
        }
        at = e;
    }
    out
}

/// The Markdown links in a line whose destination is `target`: the whole `[text](target …)` (with an image's `!`),
/// its text, its destination — byte ranges.
fn md_spans(line: &str, target: &str) -> Vec<[(usize, usize); 3]> {
    let mut out = vec![];
    let mut at = 0;
    while let Some(i) = line[at..].find("](") {
        let m = at + i;
        at = m + 2;
        let d = line.len() - line[m + 2..].trim_start().len();
        let end = d + target.len();
        if !line[d..].starts_with(target) || !line[end..].starts_with([')', ' ', '\t', '"']) {
            continue;
        }
        let (Some(c), Some(o)) = (line[end..].find(')'), line[..m].rfind('[')) else { continue };
        let start = if o > 0 && line.as_bytes()[o - 1] == b'!' { o - 1 } else { o };
        out.push([(start, end + c + 1), (o + 1, m), (d, end)]);
    }
    out
}

fn replaced(line: &str, mut spans: Vec<((usize, usize), String)>) -> String {
    let mut s = line.to_string();
    spans.sort_by_key(|((a, _), _)| std::cmp::Reverse(*a));
    for ((a, b), new) in spans {
        s.replace_range(a..b, &new);
    }
    s
}

/// `text` with each line holding `link` changed by `edit` (the line without its ending → the new line, or `None` to
/// drop it). The link must still be there.
fn on_link_lines(text: &str, link: &RawLink, edit: impl Fn(&str) -> Option<String>) -> Result<String> {
    let lines: BTreeSet<usize> = scan::page("page.md", text, 0, 0).links.iter().filter(|l| l.target == link.target && l.wiki == link.wiki && l.embed == link.embed).map(|l| l.line).collect();
    if lines.is_empty() {
        bail!("the link isn't there any more");
    }
    let mut out = String::with_capacity(text.len());
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if !lines.contains(&(i + 1)) {
            out.push_str(l);
            continue;
        }
        let body = l.trim_end_matches(['\n', '\r']);
        if let Some(new) = edit(body) {
            out.push_str(&new);
            out.push_str(&l[body.len()..]);
        }
    }
    Ok(out)
}

fn is_heading(t: &str) -> bool {
    let h = t.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&h) && t[h..].starts_with(' ')
}

/// A line with inline `#from` (a whole tag) as `#to`, if it has one.
fn inline_retag(line: &str, from: &str, to: &str) -> Option<String> {
    let tag_char = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '/');
    let (mut out, mut changed, mut prev) = (String::new(), false, None::<char>);
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        if c == '#' && prev.is_none_or(|p| p.is_whitespace() || p == '(') {
            let word: String = rest[1..].chars().take_while(|c| tag_char(*c)).collect();
            if word.eq_ignore_ascii_case(from) {
                out.push('#');
                out.push_str(to);
                rest = &rest[1 + word.len()..];
                prev = to.chars().last();
                changed = true;
                continue;
            }
        }
        out.push(c);
        prev = Some(c);
        rest = &rest[c.len_utf8()..];
    }
    changed.then_some(out)
}

/// `text` with tag `from` replaced by `to`: in the properties' `tags` (an inline or a block list; dropped when `to` is
/// there already) and as an inline `#tag` outside code and headings. It must still be there.
fn retag(text: &str, from: &str, to: &str) -> Result<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let bare = |i: usize| lines[i].trim_end_matches(['\n', '\r']);
    let eol = |i: usize| &lines[i][bare(i).len()..];
    let tag_of = |item: &str| item.trim().trim_start_matches('-').trim().trim_matches(['"', '\'']).trim_start_matches('#').to_lowercase();
    let (is_from, is_to) = (|item: &str| tag_of(item) == from.to_lowercase(), |item: &str| tag_of(item) == to.to_lowercase());
    // The properties are lines 1..end (between the `---` lines).
    let end = (!lines.is_empty() && bare(0) == "---").then(|| (1..lines.len()).find(|&i| bare(i) == "---" || bare(i) == "...")).flatten();
    let mut out: Vec<Option<String>> = lines.iter().map(|l| Some(l.to_string())).collect();
    let mut changed = false;
    for i in 1..end.unwrap_or(1) {
        let Some((k, value)) = bare(i).split_once(':').filter(|(k, _)| !k.starts_with([' ', '\t']) && k.trim().eq_ignore_ascii_case("tags")) else { continue };
        let value = value.trim();
        if !value.is_empty() {
            let (open, inner, close) = value.strip_prefix('[').and_then(|x| x.strip_suffix(']')).map_or(("", value, ""), |x| ("[", x, "]"));
            let items: Vec<&str> = inner.split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
            if items.iter().any(|x| is_from(x)) {
                let mut new: Vec<String> = vec![];
                for x in &items {
                    let item = if is_from(x) { to.to_string() } else { x.to_string() };
                    if !(is_to(&item) && new.iter().any(|n| is_to(n))) {
                        new.push(item);
                    }
                }
                out[i] = Some(format!("{k}: {open}{}{close}{}", new.join(", "), eol(i)));
                changed = true;
            }
            continue;
        }
        let items: Vec<usize> = (i + 1..end.unwrap_or(0)).take_while(|&j| bare(j).trim_start().starts_with('-')).collect();
        let there = items.iter().any(|&j| is_to(bare(j)));
        for &j in &items {
            if is_from(bare(j)) {
                let indent = &bare(j)[..bare(j).len() - bare(j).trim_start().len()];
                out[j] = (!there).then(|| format!("{indent}- {to}{}", eol(j)));
                changed = true;
            }
        }
    }
    let mut fence = false;
    for (i, slot) in out.iter_mut().enumerate().skip(end.map_or(0, |e| e + 1)) {
        let t = bare(i).trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence || is_heading(t) {
            continue;
        }
        if let Some(new) = inline_retag(bare(i), from, to) {
            *slot = Some(format!("{new}{}", eol(i)));
            changed = true;
        }
    }
    if !changed {
        bail!("#{from} isn't on the page any more");
    }
    Ok(out.into_iter().flatten().collect())
}

/// A line of a table, whose cells `|` separates.
fn table_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

/// Each `|` written `\|` (those escaped already kept).
fn escape_pipes(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 1);
    let mut prev = None;
    for c in s.chars() {
        if c == '|' && prev != Some('\\') {
            out.push('\\');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// `line` with `link` replaced by its text.
fn unlinked(line: &str, link: &RawLink) -> String {
    if link.wiki {
        replaced(line, wiki_spans(line, &link.target, link.embed).into_iter().map(|r| (r, wiki_text(&link.target))).collect())
    } else {
        replaced(line, md_spans(line, &link.target).into_iter().map(|[whole, t, _]| (whole, line[t.0..t.1].to_string())).collect())
    }
}

fn run(op: &Op, text: &str) -> Result<String> {
    match op {
        Op::Relink { link, to } if link.wiki => on_link_lines(text, link, |l| {
            // On a table row the alias's `|` would split the row's cells.
            let to = if table_row(l) { escape_pipes(to) } else { to.clone() };
            Some(replaced(l, wiki_spans(l, &link.target, link.embed).into_iter().map(|r| (r, format!("{}[[{to}]]", if link.embed { "!" } else { "" }))).collect()))
        }),
        Op::Relink { link, to } => on_link_lines(text, link, |l| Some(replaced(l, md_spans(l, &link.target).into_iter().map(|[_, _, dest]| (dest, to.clone())).collect()))),
        Op::Unlink(link) => on_link_lines(text, link, |l| Some(unlinked(l, link))),
        Op::DropRow(link) => on_link_lines(text, link, |l| if table_row(l) { None } else { Some(unlinked(l, link)) }),
        Op::Prepend(s) => Ok(format!("{s}{text}")),
        Op::Retag { from, to } => retag(text, from, to),
        Op::Row(folder, note, desc, date) => match catalog::insert(text, folder, note, desc, date, &chrono::Local::now().format("%Y-%m-%d").to_string()) {
            Ok(Some(new)) => Ok(new),
            Ok(None) => bail!("the index lists it already"),
            Err(why) => bail!("{why}"),
        },
    }
}

/// A page's first properties: `title` (its H1, else its name), `date` (from its name, else its first commit) and
/// `tags: []`, in the file's own line endings.
fn properties(v: &Vault, path: &str, text: &str) -> String {
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let title = scan::page(path, text, 0, 0).title;
    let name = path.rsplit('/').next().unwrap_or(path);
    let date = name.get(..10).filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()).map(str::to_owned).or_else(|| {
        let log = git::out(&v.root, &["log", "--diff-filter=A", "--follow", "--format=%aI", "--", path])?;
        log.lines().rfind(|l| !l.trim().is_empty()).and_then(|l| l.get(..10)).map(str::to_owned)
    });
    let mut s = format!("---{nl}title: {}{nl}", super::export::yaml(&title));
    if let Some(d) = date {
        s.push_str(&format!("date: {d}{nl}"));
    }
    s.push_str(&format!("tags: []{nl}---{nl}"));
    s
}

/// The tag taxonomy's spelling of `tag`, if it allows it.
fn allowed(v: &Vault, tag: &str) -> Result<String> {
    let text = checks::tag_taxonomy(v).and_then(|t| std::fs::read_to_string(super::join(&v.root, &t)).ok()).ok_or_else(|| anyhow!("the vault has no tag taxonomy"))?;
    checks::tags_in_order(&text).into_iter().find(|t| t.eq_ignore_ascii_case(tag.trim_start_matches('#'))).ok_or_else(|| anyhow!("#{tag} isn't in the taxonomy"))
}

/// The file a finding's fix changes (vault-relative).
fn file_of(v: &Vault, f: &Finding) -> Result<String> {
    if f.kind == "not_in_index" {
        return v.path("index").ok_or_else(|| anyhow!("the vault has no index"));
    }
    f.pages.first().map(|(p, _)| p.clone()).ok_or_else(|| anyhow!("Echo has no fix for this"))
}

/// What `choice` (`None`: the fix Echo would make by itself) does to `path`, whose text is `text`.
/// The choice made, its operations, and whether it rests on a guessed page.
fn plan(v: &Vault, ix: &Index, f: &Finding, choice: Option<&str>, path: &str, text: &str) -> Result<(String, Vec<Op>, bool)> {
    let link = || match &f.subject {
        Subject::Link(l) => Ok(l.clone()),
        _ => Err(anyhow!("Echo has no fix for this")),
    };
    // An index row is pointed only at the same name (R36); a broken link may be pointed at a near one, flagged.
    let guesses = f.kind == "broken";
    let near = |l: &RawLink| closest(ix, scan::page_rel(v, path).as_deref(), l, guesses).map(|n| (Op::Relink { link: l.clone(), to: relinked(v, ix, path, l, &n.to) }, n.guess));
    let choice = match (f.kind.as_str(), choice) {
        (_, Some(c)) => c.to_string(),
        ("broken", None) => "to".into(),
        ("dead_row", None) => String::from(if near(&link()?).is_some() { "to" } else { "remove" }),
        ("no_props", None) => "props".into(),
        ("not_in_index", None) => "row".into(),
        ("unknown_tag", None) => bail!("Pick a tag the taxonomy allows"),
        _ => bail!("Echo has no fix for this"),
    };
    let mut guess = false;
    let ops = match (f.kind.as_str(), choice.as_str()) {
        ("broken" | "dead_row", "to") => {
            let (op, g) = near(&link()?).ok_or_else(|| anyhow!("Nothing close to point it to"))?;
            guess = g;
            vec![op]
        }
        ("broken", "remove") => vec![Op::Unlink(link()?)],
        ("dead_row", "remove") => vec![Op::DropRow(link()?)],
        ("broken", "create") => bail!("A new page is made with New page"),
        ("no_props", "props") => vec![Op::Prepend(properties(v, path, text))],
        ("not_in_index", "row") => {
            let (folder, note, desc, date) = page::row_for(v, &f.pages[0].0)?;
            vec![Op::Row(folder, note, desc, date)]
        }
        ("unknown_tag", c) if c.starts_with("tag:") => match &f.subject {
            Subject::Word(from) => vec![Op::Retag { from: from.clone(), to: allowed(v, &c[4..])? }],
            _ => bail!("Echo has no fix for this"),
        },
        ("unknown_tag", "taxonomy") => bail!("The taxonomy opens in Pages"),
        _ => bail!("“{choice}” isn't a fix for this"),
    };
    Ok((choice, ops, guess))
}

/// The fix of finding `key` by `choice` (`None`: the one Echo would make by itself), against its file as it is now.
/// Nothing is written.
pub fn fix(v: &Vault, key: &str, choice: Option<&str>) -> Result<Fix> {
    let ix = scan::index(v)?;
    let f = finding(v, &ix, key)?;
    let path = file_of(v, &f)?;
    let full = page::inside(v, &path)?;
    let bytes = std::fs::read(&full).map_err(|e| anyhow!("{path}: {e}"))?;
    if bytes.len() as u64 > scan::READ_MAX {
        bail!("{path} is too large for Echo to change");
    }
    let old = std::str::from_utf8(&bytes).map_err(|_| anyhow!("{path} isn't UTF-8 text, so Echo doesn't change it"))?;
    let (choice, ops, guess) = plan(v, &ix, &f, choice, &path, old)?;
    let new = ops.iter().try_fold(old.to_string(), |t, op| run(op, &t))?;
    Ok(Fix { key: key.into(), choice, path, base: page::sha(&bytes), diff: diff::lines(old, &new), text: new, guess, ops })
}

/// The ways finding `f` can be fixed, Echo's own suggestion first.
fn choices_of(v: &Vault, ix: &Index, f: &Finding) -> Vec<Choice> {
    let c = |choice: &str, label: String| Choice { choice: choice.into(), label, to: None, why: None };
    match f.kind.as_str() {
        "broken" | "dead_row" => {
            let mut out = vec![];
            if let Subject::Link(l) = &f.subject {
                if let Some(n) = closest(ix, f.pages.first().and_then(|(p, _)| scan::page_rel(v, p)).as_deref(), l, f.kind == "broken") {
                    let to = scan::vault_rel(v, &n.to);
                    out.push(Choice { choice: "to".into(), label: format!("Point it to {to}"), to: Some(to), why: Some(n.why.into()) });
                }
            }
            if f.kind == "broken" {
                out.extend([c("remove", "Remove the link, keep its text".into()), c("create", "Create the page".into())]);
            } else {
                out.push(c("remove", "Remove the row".into()));
            }
            out
        }
        "no_props" => vec![c("props", "Add a title, a date and tags".into())],
        "not_in_index" => vec![c("row", "Add its row to the index".into())],
        "unknown_tag" => {
            let tags = checks::tag_taxonomy(v).and_then(|t| std::fs::read_to_string(super::join(&v.root, &t)).ok()).map(|t| checks::tags_in_order(&t)).unwrap_or_default();
            let mut out: Vec<Choice> = tags.iter().map(|t| c(&format!("tag:{t}"), format!("Use #{t}"))).collect();
            out.push(c("taxonomy", "Open the taxonomy to add it".into()));
            out
        }
        _ => vec![],
    }
}

/// A fix not written, and why (it had nothing left to do, it isn't there any more, it was a guess…).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skipped {
    pub key: String,
    pub why: String,
}

/// What came of writing fixes.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Applied {
    /// Files written (vault-relative).
    pub written: Vec<String>,
    /// Files that changed since their fix was computed: left as they are.
    pub changed_since: Vec<String>,
    /// Fixes not made, and why: in a file written, one with nothing left to do there (another fix of the file did it).
    pub skipped: Vec<Skipped>,
    /// The commit's label (`<id>`, `off`, `skipped: …`).
    pub commit: String,
}

/// Write fixes: each file only if it is still the text its fixes were computed on (several fixes of one file go in
/// together), through `write_atomic`, and slice 1's `log.md` line when rows went into the index; then one commit of
/// the written files that had no changes of the user's (when the vault commits) and an activity line per page.
pub fn apply(v: &Vault, fixes: &[Fix]) -> Result<Applied> {
    super::refuse_headless()?;
    if !super::available(v) {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    let mut files: Vec<(&str, Vec<&Fix>)> = vec![];
    for f in fixes {
        match files.iter_mut().find(|(p, _)| *p == f.path) {
            Some((_, fs)) => fs.push(f),
            None => files.push((f.path.as_str(), vec![f])),
        }
    }
    let full: Vec<PathBuf> = files.iter().map(|(p, _)| page::inside(v, p)).collect::<Result<_>>()?;
    let is_row = |o: &Op| matches!(o, Op::Row(..));
    let log = v.detected.log.clone().filter(|_| fixes.iter().any(|f| f.ops.iter().any(is_row)));
    // git is asked first: between a file's check and its write nothing slow runs.
    let targets: Vec<PathBuf> = full.iter().cloned().chain(log.iter().map(|l| super::join(&v.root, l))).collect();
    let dirty = if v.git.commit { git::dirty(&v.root, &targets) } else { vec![] };
    let mut out = Applied::default();
    let (mut done, mut written, mut rows) = (vec![], vec![], 0);
    for ((path, fs), full) in files.iter().zip(&full) {
        let now = std::fs::read(full).ok();
        let base = now.as_deref().map(page::sha);
        let (fresh, stale): (Vec<&Fix>, Vec<&Fix>) = fs.iter().copied().partition(|f| base.as_deref() == Some(f.base.as_str()));
        if !stale.is_empty() {
            out.changed_since.push(path.to_string());
        }
        let Some(old) = now.filter(|_| !fresh.is_empty()).and_then(|b| String::from_utf8(b).ok()) else { continue };
        let (mut new, mut rows_here) = (old.clone(), 0);
        for f in fresh {
            match f.ops.iter().try_fold(new.clone(), |t, op| run(op, &t)) {
                Ok(t) => {
                    new = t;
                    rows_here += f.ops.iter().filter(|o| is_row(o)).count();
                }
                Err(e) => out.skipped.push(Skipped { key: f.key.clone(), why: format!("{e:#}") }),
            }
        }
        if new != old {
            crate::paths::write_atomic(full, new.as_bytes())?;
            out.written.push(path.to_string());
            written.push(full.clone());
            done.push((path.to_string(), full, old, new));
            rows += rows_here;
        }
    }
    if let (Some(index), Some(log), true) = (v.path("index"), &log, rows > 0) {
        if let Some(l) = page::log_rows(v, &index, rows)? {
            out.written.push(log.clone());
            written.push(l);
        }
    }
    out.commit = match written.len() {
        0 => git::Commit::Off.label(),
        1 => page::commit_one(v, &written, &dirty, &format!("Echo: Health fix in {}", out.written[0])),
        n => page::commit_one(v, &written, &dirty, &format!("Echo: Health fixes in {n} files")),
    };
    // A file the user had changed isn't in the commit, so its line names none.
    for (path, full, old, new) in &done {
        activity::changed(v, path, "health", None, Some(old), new, activity::commit_id(&out.commit).filter(|_| !dirty.contains(full)));
    }
    Ok(out)
}

// ── health.json ──

/// One day's score: the day's last.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub day: String,
    pub score: u32,
    pub categories: Vec<(String, f64, u32)>,
}

/// The last AI check (written by `lint`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRun {
    pub at: String,
    pub run: String,
    #[serde(default)]
    pub cost: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Oldest first, at most one a day, the last [`HISTORY_DAYS`] days.
    #[serde(default)]
    pub history: Vec<Point>,
    /// Keys of the findings the user ignored, kept when they are no longer found.
    #[serde(default)]
    pub ignored: Vec<String>,
    #[serde(default)]
    pub ai: Option<AiRun>,
}

fn file(v: &Vault) -> PathBuf {
    super::cache_dir(&v.id).join("health.json")
}

/// Only a file that isn't there is empty; any other read error is one, so it is never rewritten from nothing.
fn read(v: &Vault) -> Result<State> {
    match std::fs::read(file(v)) {
        Ok(b) => serde_json::from_slice(&b).map_err(|e| anyhow!("{}: {e}", file(v).display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(e) => Err(e.into()),
    }
}

/// `health.json` as it is (empty when it can't be read).
pub fn load(v: &Vault) -> State {
    read(v).unwrap_or_default()
}

/// Change `health.json` under its lock, re-read first (rust-conventions R1, R3).
fn update<T>(v: &Vault, f: impl FnOnce(&mut State) -> T) -> Result<T> {
    let d = super::cache_dir(&v.id);
    std::fs::create_dir_all(&d)?;
    let t = Instant::now();
    let _held = loop {
        if let Some(l) = crate::app::FileLock::try_take(&d.join("health.lock"))? {
            break l;
        }
        if t.elapsed() > Duration::from_secs(20) {
            bail!("Health's file is busy");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut s = read(v)?;
    let out = f(&mut s);
    crate::paths::write_atomic(&file(v), serde_json::to_string_pretty(&s)?.as_bytes())?;
    Ok(out)
}

/// Ignore finding `key` (`on`), or list it again; the keys ignored now.
pub fn ignore(v: &Vault, key: &str, on: bool) -> Result<Vec<String>> {
    super::refuse_headless()?;
    if key.is_empty() || key.len() > 2000 || key.chars().any(char::is_control) {
        bail!("not a finding");
    }
    update(v, |s| {
        s.ignored.retain(|k| k != key);
        if on {
            s.ignored.push(key.to_string());
            s.ignored.sort();
        }
        s.ignored.clone()
    })
}

/// Keep `score` as `day`'s point (the day's last), and the last [`HISTORY_DAYS`] days. Unchanged: nothing written.
fn note_on(v: &Vault, day: NaiveDate, score: &Score) -> Result<()> {
    let p = Point { day: day.format("%Y-%m-%d").to_string(), score: score.total, categories: score.categories.clone() };
    let noted = |s: &State| s.history.contains(&p);
    if noted(&load(v)) {
        return Ok(());
    }
    let first = day - chrono::Duration::days(HISTORY_DAYS - 1);
    update(v, |s| {
        if noted(s) {
            return;
        }
        s.history.retain(|x| x.day != p.day && NaiveDate::parse_from_str(&x.day, "%Y-%m-%d").is_ok_and(|d| d >= first));
        s.history.push(p.clone());
        s.history.sort_by(|a, b| a.day.cmp(&b.day));
    })
}

/// The findings shown and those ignored, the score of the shown ones, and `health.json` once today's point is noted
/// (spec K16-09: when the checks are first computed on a day, and whenever the score changes).
fn scored(v: &Vault, ix: &Index) -> (Vec<Finding>, Vec<Finding>, Score, State) {
    let state = load(v);
    let (ignored, shown): (Vec<Finding>, Vec<Finding>) = cached(v, ix).iter().cloned().partition(|f| state.ignored.contains(&f.key));
    let s = score(&shown, scope_pages(v, ix), state.ai.is_some());
    if let Err(e) = note_on(v, chrono::Local::now().date_naive(), &s) {
        eprintln!("knowledge: the health score of {}: {e:#}", v.id);
    }
    (shown, ignored, s, load(v))
}

// ── what the window and the CLI ask ──

/// Health as the window shows it: the score, the findings (and the ignored ones apart), the history, the last AI
/// check. `fixes_of`: a kind whose findings *Fix all* fixes, with their fixes (its sheet).
pub fn view(v: &Vault, fixes_of: Option<&str>) -> Result<Value> {
    let ix = scan::index(v)?;
    let (shown, ignored, s, state) = scored(v, &ix);
    let mut j = json!({ "score": s, "findings": shown, "ignored": ignored, "history": state.history, "ai": state.ai, "pages": scope_pages(v, &ix) });
    if let Some(kind) = fixes_of {
        let fixes: Vec<Value> = shown.iter().filter(|f| f.kind == kind && f.fix_all).map(|f| fix(v, &f.key, None).map_or_else(|e| json!({ "key": f.key, "error": format!("{e:#}") }), |x| x.value())).collect();
        j["fixes"] = json!(fixes);
    }
    Ok(j)
}

/// A link's name, for *Create the page*.
fn link_name(l: &RawLink) -> String {
    let bare = Resolver::bare(l);
    strip_md(bare.rsplit('/').next().unwrap_or(&bare)).to_string()
}

/// One finding: itself, whether it is ignored, its choices and the fix of `choice` (`None`: Echo's own) — or why
/// there is none.
pub fn detail(v: &Vault, key: &str, choice: Option<&str>) -> Result<Value> {
    let ix = scan::index(v)?;
    let f = finding(v, &ix, key)?;
    let shown = match fix(v, key, choice) {
        Ok(x) => x.value(),
        Err(e) => json!({ "choice": choice, "error": format!("{e:#}") }),
    };
    let mut j = json!({ "finding": f, "ignored": load(v).ignored.iter().any(|k| k == key), "choices": choices_of(v, &ix, &f), "fix": shown });
    if let (Subject::Link(l), "broken") = (&f.subject, f.kind.as_str()) {
        j["new_page"] = json!({ "folder": dir_of(&f.pages[0].0), "name": link_name(l) });
    }
    Ok(j)
}

/// `echoctl knowledge check`: slice 1's keys as they were, the new kinds, and the score.
pub fn check(v: &Vault) -> Result<Value> {
    let ix = scan::index(v)?;
    let mut j = serde_json::to_value(checks::run(v, &ix))?;
    let (_, _, s, _) = scored(v, &ix);
    j["score"] = serde_json::to_value(s)?;
    Ok(j)
}

#[cfg(test)]
mod tests;

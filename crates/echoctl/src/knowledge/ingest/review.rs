//! A run's changes (spec K15-07…K15-10, D27, D28, D42, D43, X-T8, X-T9): what the run created or modified in its copy
//! — compared with `bases.json` when it ended —, each one's diff and the source words it came from; the user's answers
//! (`review.json`); and the approved ones written into the vault: an index's added rows into its tables as they are
//! now and a log's added lines at its end (D42), every other file only over its base. Paths come from Echo's own walk
//! of the copy, never from the run's notes, whose paths only name the changes they describe.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use super::super::{activity, catalog, diff, evidence, export, git, page, scan, sources, Vault};
use super::{effective, find_run, run_dir, update_run, NOTES};
use crate::paths;

/// The most a change's edited text may be.
const EDIT_MAX: usize = 5 << 20;
/// The run's notes (`changes.json`) and a source checked for quotes are read up to this much.
const NOTES_MAX: u64 = 4 << 20;
const SOURCE_MAX: u64 = 16 << 20;

/// What the run changed and what the user said: `review.json` (lock + re-read + `write_atomic`).
#[derive(Debug, Default, Serialize, Deserialize)]
struct Review {
    /// Found when the run ended: each file it created (`new`) or modified, outside `.echo-ingest/`.
    found: Vec<Found>,
    #[serde(default)]
    answers: BTreeMap<String, Answer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Found {
    path: String,
    new: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Answer {
    /// `approved`, `rejected`, `written` or `changed` (the vault's file changed since the run: not written).
    state: String,
    /// The user's own text (*Edit*).
    #[serde(default)]
    text: Option<String>,
    /// *Take the run's*: written over the vault's file as it is now.
    #[serde(default)]
    force: bool,
}

/// A quote Echo found in its source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Quote {
    pub source: String,
    pub text: String,
}

/// One file the run created or changed, as the review shows it.
#[derive(Debug, Clone, Serialize)]
pub struct Change {
    /// Vault-relative.
    pub path: String,
    /// `new` (a new page), `edit`, `index`, `log`, `taxonomy` or `other`.
    pub kind: &'static str,
    /// An index or log change that only adds lines (D42): written as its added lines onto the file as it is then.
    pub adds_only: bool,
    pub plus: usize,
    pub minus: usize,
    /// Hunks with 3 lines of context: `[[["=" | "-" | "+", line], …], …]`.
    pub diff: Vec<Vec<(char, String)>>,
    /// Why, in the run's words (others' words: shown as text).
    pub why: Option<String>,
    /// The quotes found in their sources (D27); `invented`: how many weren't (counted, never shown).
    pub quotes: Vec<Quote>,
    pub invented: usize,
    /// "no quote found", or "Echo can't check quotes in this kind of file" (D43).
    pub quote_note: Option<String>,
    /// Added lines left out because they link to a rejected new page (D28), and the change saying so.
    pub dropped: Vec<String>,
    pub note: Option<String>,
    /// `undecided`, `approved`, `rejected`, `written`, `changed` (in the vault since the run) or `blocked` (Echo can't
    /// write it: `note` says why).
    pub state: String,
    /// The user's text, not the run's.
    pub edited: bool,
    #[serde(skip)]
    new_file: bool,
    #[serde(skip)]
    approved: bool,
    #[serde(skip)]
    force: bool,
    /// The file's sha256 as it was copied.
    #[serde(skip)]
    base_sha: Option<String>,
    /// The file as the run found it (`None` for a new one, or when Echo kept no copy of it).
    #[serde(skip)]
    base: Option<String>,
    /// The diff's left side: the base, else (or for a changed one) the vault's file now.
    #[serde(skip)]
    left: String,
    /// What would be written.
    #[serde(skip)]
    text: Vec<u8>,
    /// *Adds only*: the added lines (an index row with its table's heading).
    #[serde(skip)]
    rows: Vec<(Option<String>, String)>,
}

impl Change {
    /// Its diff and counts, from `left` to `text`.
    fn fill(&mut self) {
        let d = diff::lines(&self.left, &String::from_utf8_lossy(&self.text));
        (self.plus, self.minus) = diff::counts(&d);
        self.diff = diff::hunks(&d, 3);
    }
}

fn read_review(dir: &Path) -> Result<Review> {
    match std::fs::read(dir.join("review.json")) {
        Ok(b) => Ok(serde_json::from_slice(&b)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Review::default()),
        Err(e) => Err(e.into()),
    }
}

fn update_review<T>(dir: &Path, f: impl FnOnce(&mut Review) -> Result<T>) -> Result<T> {
    let _held = export::hold(&dir.join("review.lock"), 20, "this run's review is busy")?;
    let mut r = read_review(dir)?;
    let out = f(&mut r)?;
    paths::write_atomic(&dir.join("review.json"), &serde_json::to_vec_pretty(&r)?)?;
    Ok(out)
}

fn sha(b: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// A path a run may have written that Echo writes back: never an instruction file, the vault's tools' own files or a
/// source (the guard's rules, again).
fn writable(v: &Vault, rel: &str) -> bool {
    let p = Path::new(rel);
    let folder = sources::folder(v);
    !guard_refuses(p) && rel != folder && !rel.starts_with(&format!("{folder}/"))
}

fn guard_refuses(p: &Path) -> bool {
    crate::guard::instruction_file(p) || crate::guard::machinery(p)
}

/// The files under `root` (vault-relative, `/`), links never followed.
pub(super) fn files_under(root: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push((e.path(), r)),
                Ok(t) if t.is_file() => out.push(r),
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// When a run ends: the files of its copy that it created or changed (`review.json`) — how many, and how many of them
/// Echo can't write into the vault. The vault's file as it was copied is kept (`base/`) for each modified one still so
/// now, so its diff and activity line stay the run's own however the vault changes before the write.
pub(super) fn find(v: &Vault, dir: &Path) -> Result<(usize, usize)> {
    let bases: Value = serde_json::from_slice(&std::fs::read(dir.join("bases.json"))?)?;
    let copy = dir.join("vault");
    let (mut found, mut blocked) = (vec![], 0);
    for rel in files_under(&copy) {
        if rel == NOTES || rel.starts_with(&format!("{NOTES}/")) || !writable(v, &rel) {
            continue;
        }
        let base = bases["files"][&rel].as_str();
        if base.is_some_and(|b| sources::file_sha(&super::super::join(&copy, &rel)).is_ok_and(|now| now == b)) {
            continue;
        }
        match page::inside(v, &rel) {
            Err(_) => blocked += 1,
            Ok(full) => {
                let kept = super::super::join(&dir.join("base"), &rel);
                let still = base.is_some_and(|b| std::fs::read(&full).is_ok_and(|now| sha(&now) == b));
                if still && !kept.exists() {
                    std::fs::create_dir_all(kept.parent().expect("a parent"))?;
                    std::fs::copy(&full, &kept)?;
                }
            }
        }
        found.push(Found { path: rel, new: base.is_none() });
    }
    let n = found.len();
    update_review(dir, |r| {
        r.found = found;
        Ok(())
    })?;
    Ok((n, blocked))
}

/// A change's kind (K15-07).
fn kind(v: &Vault, rel: &str, new: bool) -> &'static str {
    if v.path("index").as_deref() == Some(rel) {
        "index"
    } else if v.detected.log.as_deref() == Some(rel) {
        "log"
    } else if v.detected.taxonomies.iter().any(|t| t == rel) {
        "taxonomy"
    } else if scan::is_page_path(v, rel) {
        if new { "new" } else { "edit" }
    } else {
        "other"
    }
}

/// An index or log change that only adds lines (D42), and those lines: an index's must each be a row of a table (its
/// heading kept), its *Last update* line aside (row insertion sets it anyway); blank lines don't count.
fn additions(kind: &str, d: &[(char, String)], new_text: &str) -> Option<Vec<(Option<String>, String)>> {
    let (mut at, mut rows) = (0usize, vec![]);
    for (k, line) in d {
        let aside = line.trim().is_empty() || (kind == "index" && line.contains("Last update:"));
        match k {
            '=' => at += 1,
            '-' if aside => {}
            '-' => return None,
            _ => {
                if !aside {
                    let heading = if kind == "index" { Some(catalog::table_of_row(new_text, at)?) } else { None };
                    rows.push((heading, line.clone()));
                }
                at += 1;
            }
        }
    }
    Some(rows)
}

/// The run's notes (`.echo-ingest/changes.json`) by the path each one names: why, and the quotes `(source, text)`.
type Notes = HashMap<String, (Option<String>, Vec<(String, String)>)>;

/// A path as the run wrote it: relative to the copy, with `/`.
fn rel_in(copy: &Path, p: &str) -> String {
    let p = p.replace('\\', "/");
    let roots = [Some(copy.to_path_buf()), dunce::canonicalize(copy).ok()];
    let stripped = roots.iter().flatten().find_map(|r| p.strip_prefix(r.to_string_lossy().replace('\\', "/").trim_end_matches('/')).map(str::to_owned));
    stripped.unwrap_or(p).trim_start_matches("./").trim_start_matches('/').to_string()
}

fn one_line(s: &str, max: usize) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(max).collect()
}

fn read_capped(p: &Path, max: u64) -> Option<String> {
    use std::io::Read as _;
    let mut b = vec![];
    std::fs::File::open(p).ok()?.take(max).read_to_end(&mut b).ok()?;
    Some(String::from_utf8_lossy(&b).into_owned())
}

fn notes(copy: &Path) -> Notes {
    let mut out = Notes::new();
    let Some(text) = read_capped(&copy.join(NOTES).join("changes.json"), NOTES_MAX) else { return out };
    let Ok(j) = serde_json::from_str::<Value>(&text) else { return out };
    for c in j["changes"].as_array().into_iter().flatten() {
        let Some(path) = c["path"].as_str() else { continue };
        let e = out.entry(rel_in(copy, path)).or_default();
        if let Some(why) = c["why"].as_str().map(|w| one_line(w, 500)).filter(|w| !w.is_empty()) {
            e.0 = Some(why);
        }
        for q in c["quotes"].as_array().into_iter().flatten() {
            if let (Some(s), Some(t)) = (q["source"].as_str(), q["text"].as_str()) {
                e.1.push((rel_in(copy, s), t.to_string()));
            }
        }
    }
    out
}

/// The quotes of a change that stand in their sources (D27, D43): only in the run's own sources, only those Echo reads
/// as text; `(found, invented, unchecked)`.
fn checked(copy: &Path, run_sources: &[String], given: &[(String, String)], texts: &mut HashMap<String, Option<String>>) -> (Vec<Quote>, usize, usize) {
    let (mut found, mut invented, mut unchecked) = (vec![], 0, 0);
    let mut seen = BTreeSet::new();
    for (source, text) in given {
        if !seen.insert((source, text)) {
            continue;
        }
        if !run_sources.contains(source) {
            invented += 1;
        } else if !evidence::text_source(Path::new(source)) {
            unchecked += 1;
        } else {
            let t = texts.entry(source.clone()).or_insert_with(|| read_capped(&super::super::join(copy, source), SOURCE_MAX));
            if t.as_deref().is_some_and(|t| evidence::found(t, text)) {
                found.push(Quote { source: source.clone(), text: text.chars().take(2000).collect() });
            } else {
                invented += 1;
            }
        }
    }
    (found, invented, unchecked)
}

/// Does `line` link to `page` (vault-relative) — by its path from the vault's root or from `dir` (the linking file's
/// folder), spaces encoded or not, or by a wikilink to its name — with nothing of another path stuck to either end.
fn mentions(line: &str, page: &str, dir: &str) -> bool {
    let rel = if dir.is_empty() { page } else { page.strip_prefix(&format!("{dir}/")).unwrap_or(page) };
    let stem = |p: &str| p.strip_suffix(".md").unwrap_or(p).to_string();
    let open = |c: Option<char>| c.is_none_or(|c| "(<[ `|\"'".contains(c));
    let close = |c: Option<char>| c.is_none_or(|c| ")>] `|\"',;#".contains(c));
    let has = |needle: &str, before: &dyn Fn(Option<char>) -> bool, after: &dyn Fn(Option<char>) -> bool| {
        line.match_indices(needle).any(|(i, m)| before(line[..i].chars().next_back()) && after(line[i + m.len()..].chars().next()))
    };
    let wiki = |c: Option<char>| c.is_some_and(|c| "]|#".contains(c));
    [page, rel].iter().any(|p| has(p, &open, &close) || has(&p.replace(' ', "%20"), &open, &close))
        || [stem(page), stem(rel), stem(page.rsplit('/').next().unwrap_or(page))].iter().any(|s| has(&format!("[[{s}"), &|_| true, &wiki))
}

/// D28: a rejected new page takes the index rows and log lines added for it along, on the changes that added them.
fn drop_rejected(changes: &mut [Change]) {
    let rejected: Vec<String> = changes.iter().filter(|c| c.kind == "new" && c.state == "rejected").map(|c| c.path.clone()).collect();
    if rejected.is_empty() {
        return;
    }
    for c in changes.iter_mut().filter(|c| matches!(c.kind, "index" | "log") && !c.edited && !matches!(c.state.as_str(), "written" | "blocked")) {
        let dir = c.path.rsplit_once('/').map_or("", |(d, _)| d).to_string();
        let proposed = String::from_utf8_lossy(&c.text).into_owned();
        let (mut kept, mut pages) = (vec![], BTreeSet::new());
        for (k, line) in diff::lines(&c.left, &proposed) {
            let to: Vec<&String> = rejected.iter().filter(|p| k == '+' && mentions(&line, p, &dir)).collect();
            match k {
                '-' => {}
                _ if !to.is_empty() => {
                    pages.extend(to.into_iter().cloned());
                    c.dropped.push(line);
                }
                _ => kept.push(line),
            }
        }
        if c.dropped.is_empty() {
            continue;
        }
        let mut text = kept.join("\n");
        if proposed.ends_with('\n') && !text.is_empty() {
            text.push('\n');
        }
        c.text = text.into_bytes();
        let dropped = c.dropped.clone();
        c.rows.retain(|(_, l)| !dropped.contains(l));
        let n = c.dropped.len();
        let pages: Vec<String> = pages.into_iter().collect();
        c.note = Some(format!("{n} line{} left out: {} to {}, which you rejected", if n == 1 { "" } else { "s" }, if n == 1 { "it links" } else { "they link" }, pages.join(", ")));
        c.fill();
    }
}

/// A run's changes as the review shows them (K15-07, -08).
pub fn changes(v: &Vault, run: &str) -> Result<Vec<Change>> {
    let dir = run_dir(v, run)?;
    if !dir.is_dir() {
        bail!("this run's changes are gone (written or discarded)");
    }
    let r = find_run(v, run)?;
    let review = read_review(&dir)?;
    let bases: Value = serde_json::from_slice(&std::fs::read(dir.join("bases.json"))?)?;
    let copy = dir.join("vault");
    let notes = notes(&copy);
    let mut texts = HashMap::new();
    let mut out = vec![];
    for f in &review.found {
        // A path of Echo's own walk of the copy (no link followed): read as it is, whatever its name.
        let run_bytes = std::fs::read(super::super::join(&copy, &f.path))?;
        let a = review.answers.get(&f.path).cloned().unwrap_or_default();
        let base_sha = bases["files"][&f.path].as_str().map(str::to_owned);
        // A path the vault can't take (a name Windows can't, a folder that is a link out): shown, never written.
        let blocked = page::inside(v, &f.path).err().map(|e| format!("can't be written: {e:#}"));
        let now = if blocked.is_none() { std::fs::read(super::super::join(&v.root, &f.path)).ok() } else { None };
        let kept = super::super::join(&dir.join("base"), &f.path);
        let base: Option<String> = if f.new {
            Some(String::new())
        } else if kept.is_file() {
            std::fs::read(&kept).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
        } else {
            now.as_ref().filter(|b| base_sha.as_deref() == Some(sha(b).as_str())).map(|b| String::from_utf8_lossy(b).into_owned())
        };
        let kind = kind(v, &f.path, f.new);
        let text = a.text.clone().map(String::into_bytes).unwrap_or(run_bytes);
        let proposed = String::from_utf8_lossy(&text).into_owned();
        let rows = (a.text.is_none() && base.is_some() && matches!(kind, "index" | "log")).then(|| additions(kind, &diff::lines(base.as_deref().unwrap_or(""), &proposed), &proposed)).flatten();
        // Changed in the vault since the run: not as it was copied, and not as the run would write it.
        let same = now.as_deref() == Some(text.as_slice());
        let moved = if f.new { now.is_some() } else { now.as_deref().map(sha) != base_sha };
        let changed = blocked.is_none() && rows.is_none() && !same && moved;
        let mut state = if a.state.is_empty() { "undecided".to_string() } else { a.state.clone() };
        if blocked.is_some() {
            state = "blocked".into();
        } else if changed && !a.force && matches!(state.as_str(), "undecided" | "approved") {
            state = "changed".into();
        }
        // Compare (K6's) for a changed one: from the vault's file now; else the run's own diff.
        let left = if state == "changed" { now.as_ref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default() } else { base.clone().or_else(|| now.as_ref().map(|b| String::from_utf8_lossy(b).into_owned())).unwrap_or_default() };
        let (why, given) = notes.get(&f.path).cloned().unwrap_or_default();
        let (quotes, invented, unchecked) = checked(&copy, &r.sources, &given, &mut texts);
        let quote_note = match (quotes.is_empty(), unchecked > 0) {
            (_, true) => Some("Echo can't check quotes in this kind of file".to_string()),
            (true, false) => Some("no quote found".to_string()),
            (false, false) => None,
        };
        let mut c = Change {
            path: f.path.clone(),
            kind,
            adds_only: rows.is_some(),
            plus: 0,
            minus: 0,
            diff: vec![],
            why,
            quotes,
            invented,
            quote_note,
            dropped: vec![],
            note: blocked.clone(),
            edited: a.text.is_some(),
            new_file: f.new,
            approved: a.state == "approved" && blocked.is_none(),
            force: a.force,
            base_sha,
            base: if f.new { None } else { base },
            left,
            text,
            rows: rows.unwrap_or_default(),
            state,
        };
        c.fill();
        out.push(c);
    }
    drop_rejected(&mut out);
    Ok(out)
}

/// The review as the page shows it: the run, its changes and their counts.
pub fn changes_value(v: &Vault, run: &str) -> Result<Value> {
    let r = effective(v, find_run(v, run)?);
    let all = if r.state == "review" { changes(v, run)? } else { vec![] };
    let count = |s: &str| all.iter().filter(|c| c.state == s).count();
    Ok(json!({
        "run": r,
        "changes": all,
        "counts": { "all": all.len(), "undecided": count("undecided"), "approved": count("approved"), "rejected": count("rejected"), "written": count("written"), "changed": count("changed"), "blocked": count("blocked") },
    }))
}

/// A change's whole proposed text, for *Edit* (K15-08): the run's (with D28's lines left out), or the user's edit.
pub fn proposed(v: &Vault, run: &str, path: &str) -> Result<Value> {
    let c = changes(v, run)?.into_iter().find(|c| c.path == path).ok_or_else(|| anyhow!("this run has no change to {path}"))?;
    Ok(json!({ "path": c.path, "text": String::from_utf8_lossy(&c.text), "edited": c.edited, "state": c.state }))
}

/// An answer to a change (K15-08, -09).
pub enum Decision {
    Approve,
    Reject,
    /// The user's own text for the file (no longer *adds only*).
    Edit(String),
    /// *Take the run's* for a change made in the vault since the run: written over the vault's file as it is.
    Take,
}

/// A run waiting for review, and its folder.
fn reviewing(v: &Vault, run: &str) -> Result<PathBuf> {
    super::super::refuse_headless()?;
    let dir = run_dir(v, run)?;
    let r = effective(v, find_run(v, run)?);
    if r.state != "review" || !dir.is_dir() {
        bail!("This run's changes aren't waiting for review");
    }
    Ok(dir)
}

/// Answer the change to `path` (one of the run's own, by its path).
pub fn decide(v: &Vault, run: &str, path: &str, d: Decision) -> Result<()> {
    let dir = reviewing(v, run)?;
    let now = changes(v, run)?.into_iter().find(|c| c.path == path).ok_or_else(|| anyhow!("this run has no change to {path}"))?;
    if now.state == "blocked" {
        bail!("{path} {}", now.note.unwrap_or_default());
    }
    if matches!(d, Decision::Take) && now.state != "changed" {
        bail!("Take the run's is for a change made in the vault since the run");
    }
    update_review(&dir, |r| {
        if !r.found.iter().any(|f| f.path == path) {
            bail!("this run has no change to {path}");
        }
        let a = r.answers.entry(path.to_string()).or_default();
        if a.state == "written" {
            bail!("{path} is written already");
        }
        match d {
            Decision::Approve => a.state = "approved".into(),
            Decision::Reject => *a = Answer { state: "rejected".into(), text: None, force: false },
            Decision::Edit(text) if text.len() > EDIT_MAX => bail!("the text is too long"),
            Decision::Edit(text) => {
                a.state = "approved".into();
                a.text = Some(text);
            }
            Decision::Take => {
                a.state = "approved".into();
                a.force = true;
            }
        }
        Ok(())
    })
}

/// *Approve the rest*: every change not answered yet; how many.
pub fn approve_rest(v: &Vault, run: &str) -> Result<usize> {
    let dir = reviewing(v, run)?;
    let blocked: Vec<String> = changes(v, run)?.into_iter().filter(|c| c.state == "blocked").map(|c| c.path).collect();
    update_review(&dir, |r| {
        let open: Vec<String> = r.found.iter().filter(|f| !r.answers.contains_key(&f.path) && !blocked.contains(&f.path)).map(|f| f.path.clone()).collect();
        for p in &open {
            r.answers.insert(p.clone(), Answer { state: "approved".into(), ..Default::default() });
        }
        Ok(open.len())
    })
}

/// What *Write N approved* did (K15-09).
#[derive(Debug, Clone, Default, Serialize)]
pub struct Written {
    /// Vault-relative.
    pub written: Vec<String>,
    /// "changed in the vault since the run": not written.
    pub changed_since: Vec<String>,
    /// The commit's id, `off` or `skipped: <why>`.
    pub commit: String,
    /// The written files the commit took: not those with changes of the user's before (X-G2).
    pub committed: Vec<String>,
    /// Changes written of the run's so far, of how many.
    pub kept: usize,
    pub of: usize,
    /// Every change is written or rejected: the run is done and its copy gone.
    pub finished: bool,
}

enum Out {
    Write { old: Option<Vec<u8>>, new: Vec<u8> },
    /// Nothing to do: the vault has it already.
    Same,
    Stale,
}

/// What writing `c` over the vault's file as it is `now` comes to.
fn outcome(c: &Change, now: Option<Vec<u8>>, today: &str) -> Out {
    // *Take the run's* takes the run's whole text, an index's or a log's too.
    if c.adds_only && !c.force {
        let Some(cur) = now.as_ref().and_then(|b| String::from_utf8(b.clone()).ok()) else { return Out::Stale };
        let mut t = cur.clone();
        for (heading, line) in &c.rows {
            let line = line.trim_end_matches('\r');
            match heading {
                Some(h) => match catalog::insert_row(&t, h, line, today) {
                    Ok(Some(next)) => t = next,
                    Ok(None) => {}
                    Err(_) => return Out::Stale,
                },
                None if t.lines().any(|l| l.trim_end_matches('\r') == line) => {}
                None => {
                    if !t.is_empty() && !t.ends_with('\n') {
                        t.push('\n');
                    }
                    t.push_str(line);
                    t.push('\n');
                }
            }
        }
        return if t == cur { Out::Same } else { Out::Write { old: now, new: t.into_bytes() } };
    }
    let fresh = c.force || if c.new_file { now.is_none() } else { now.as_deref().map(sha) == c.base_sha };
    match now {
        // Already as the run would write it (a write that stopped midway, written again): written, not changed.
        Some(b) if b == c.text => Out::Same,
        _ if !fresh => Out::Stale,
        old => Out::Write { old, new: c.text.clone() },
    }
}

/// The commit message (K15-09): `Ingest: <N> sources (<first title>)`.
fn message(v: &Vault, run_sources: &[String]) -> String {
    let first = run_sources.first().map(String::as_str).unwrap_or("");
    let title = sources::queue(v).ok().and_then(|q| q.into_iter().find(|s| s.path == first).map(|s| s.title)).unwrap_or_else(|| first.rsplit('/').next().unwrap_or(first).to_string());
    let n = run_sources.len();
    format!("Ingest: {n} source{} ({title})", if n == 1 { "" } else { "s" })
}

/// *Write N approved* (K15-09): each approved change written into the vault — an *adds only* index or log change as
/// its added lines onto the file as it is now, every other only over its base (or where there still is no file, for a
/// new one), else "changed in the vault since the run"; then one commit of the written files that were clean before
/// (when the vault commits), the activity lines, the run's sources Done (kept N of M); the copy goes once nothing is
/// left to decide.
pub fn write(v: &Vault, run: &str, only: &[String]) -> Result<Written> {
    let dir = reviewing(v, run)?;
    if !super::super::available(v) {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    let r = find_run(v, run)?;
    let _save = export::vault_lock(v)?;
    let all = changes(v, run)?;
    if let Some(p) = only.iter().find(|p| !all.iter().any(|c| &c.path == *p)) {
        bail!("this run has no change to {p}");
    }
    let todo: Vec<&Change> = all.iter().filter(|c| c.approved && (only.is_empty() || only.contains(&c.path))).collect();
    let blocked: Vec<&str> = all.iter().filter(|c| c.state == "blocked").map(|c| c.path.as_str()).collect();
    let full: Vec<PathBuf> = todo.iter().map(|c| page::inside(v, &c.path)).collect::<Result<_>>()?;
    // git is asked first: between a file's check and its write nothing slow runs.
    let dirty = if v.git.commit { git::dirty(&v.root, &full) } else { vec![] };
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut out = Written { of: all.len(), ..Default::default() };
    let (mut done, mut written) = (vec![], vec![]);
    for (c, full) in todo.iter().zip(&full) {
        let now = match std::fs::read(full) {
            Ok(b) => Some(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(anyhow!("{}: {e}", c.path)),
        };
        match outcome(c, now, &today) {
            Out::Stale => out.changed_since.push(c.path.clone()),
            Out::Same => {
                out.written.push(c.path.clone());
                // Echo's change is in the file (written before, by a write that stopped midway): its line and commit.
                if c.base.as_deref().map(str::as_bytes) != Some(c.text.as_slice()) && !c.adds_only {
                    written.push(full.clone());
                    done.push((c.path.clone(), full.clone(), c.base.clone().map(String::into_bytes), c.text.clone()));
                }
            }
            Out::Write { old, new } => {
                std::fs::create_dir_all(full.parent().expect("a parent"))?;
                paths::write_atomic(full, &new)?;
                out.written.push(c.path.clone());
                written.push(full.clone());
                done.push((c.path.clone(), full.clone(), old, new));
            }
        }
    }
    out.commit = if written.is_empty() { git::Commit::Off.label() } else { page::commit_one(v, &written, &dirty, &message(v, &r.sources)) };
    let commit = activity::commit_id(&out.commit);
    if commit.is_some() {
        out.committed = done.iter().filter(|(_, full, ..)| !dirty.contains(full)).map(|(p, ..)| p.clone()).collect();
    }
    // A file the user had changed isn't in the commit, so its line names none.
    for (path, full, old, new) in &done {
        let old = old.as_ref().map(|b| String::from_utf8_lossy(b).into_owned());
        activity::changed(v, path, "ingest", Some(run), old.as_deref(), &String::from_utf8_lossy(new), commit.clone().filter(|_| !dirty.contains(full)));
    }
    let (kept, finished) = update_review(&dir, |rv| {
        for p in &out.written {
            rv.answers.insert(p.clone(), Answer { state: "written".into(), ..Default::default() });
        }
        for p in &out.changed_since {
            if let Some(a) = rv.answers.get_mut(p) {
                a.state = "changed".into();
            }
        }
        let kept = rv.answers.values().filter(|a| a.state == "written").count();
        let finished = rv.found.iter().all(|f| blocked.contains(&f.path.as_str()) || rv.answers.get(&f.path).is_some_and(|a| matches!(a.state.as_str(), "written" | "rejected")));
        Ok((kept, finished))
    })?;
    (out.kept, out.finished) = (kept, finished);
    update_run(v, run, |l, r| {
        r.kept = Some(kept);
        if finished {
            r.state = "done".into();
        }
        for e in l.sources.values_mut().filter(|e| e.run.as_deref() == Some(run)) {
            e.state = "done".into();
            e.kept = Some(kept);
        }
        Ok(())
    })?;
    if finished {
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(out)
}

/// *Discard the run* (K15-08): its copy and answers go; a run waiting for review leaves its sources Done — nothing
/// kept (K15-10; *Prepare again* puts them back).
pub fn discard(v: &Vault, run: &str) -> Result<()> {
    super::super::refuse_headless()?;
    let dir = run_dir(v, run)?;
    let r = effective(v, find_run(v, run)?);
    match r.state.as_str() {
        "running" => bail!("Stop the run first"),
        "done" => bail!("This run is done: its changes are written"),
        _ => {}
    }
    let _ = std::fs::remove_dir_all(&dir);
    update_run(v, run, |l, r| {
        r.state = "discarded".into();
        super::release(l, run, "done", Some(0));
        Ok(())
    })
}

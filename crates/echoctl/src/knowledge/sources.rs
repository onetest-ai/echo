//! Ingest's sources (spec K14, D29, D30, D36): what the user adds to a vault's sources folder exactly as it came —
//! files byte for byte, a call's or memo's minutes, a mail thread, a Teams chat, a web page as Markdown — and Echo's
//! own ledger of them, `<cache>/ingest.json` (never in the vault). Echo writes a source only when the user adds it, and
//! never edits or deletes one (D29). A source is done when Echo applied or skipped it, or when the vault's `log.md`
//! has a `processed · <path>` line, the starter's format (D30). Adding processes nothing.

use anyhow::{anyhow, bail, Context as _, Result};
use chrono::{DateTime, Local, NaiveDate, TimeZone as _, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use super::{export, git, page, Vault};

/// The largest file Echo copies in.
pub const MAX_FILE: u64 = 50 << 20;

/// The ledger, `<cache>/ingest.json`: changed under `ingest.lock`, re-read first, written atomically (rust-conventions
/// R1). Runs (`runs`) are Ingest's own (spec K15).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    /// The first open's answer for the files already there (K14-09).
    #[serde(default)]
    pub baseline: Option<Baseline>,
    /// By vault-relative path.
    #[serde(default)]
    pub sources: BTreeMap<String, Entry>,
    #[serde(default)]
    pub runs: Vec<Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    pub at: String,
    /// *Mark them done* (else *Treat them as new*).
    pub done: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub sha256: String,
    /// When Echo learned of it (RFC 3339).
    #[serde(default)]
    pub added: String,
    /// How: `file`, `call`, `mail`, `teams`, `web` (added by Echo), `baseline` or `folder` (found there).
    #[serde(default)]
    pub by: String,
    /// `new`, `waiting` (a run's changes wait for review), `done` or `skipped`.
    #[serde(default)]
    pub state: String,
    /// The run that took it.
    #[serde(default)]
    pub run: Option<String>,
    /// How many of that run's changes were kept.
    #[serde(default)]
    pub kept: Option<usize>,
    /// Why it was skipped, in the user's words.
    #[serde(default)]
    pub note: Option<String>,
    /// A call on another AI profile than the vault's, added after *Add anyway* (X-P3).
    #[serde(default)]
    pub anyway: bool,
}

/// What a source is, by where it sits: `echo/`, `mail/`, `teams/`, `web/`, else a file as it came.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Echo,
    Mail,
    Teams,
    Web,
    File,
}

/// One file of the sources folder, as the queue shows it.
#[derive(Debug, Clone, Serialize)]
pub struct Source {
    /// Vault-relative.
    pub path: String,
    pub kind: Kind,
    /// A Markdown source's own title, else the file's name.
    pub title: String,
    /// Its size, when it came, how, and the ledger's note, run, kept and anyway.
    pub meta: Value,
    /// `new`, `waiting`, `done` or `skipped`.
    pub state: String,
}

/// What an add wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Added {
    /// Vault-relative.
    pub path: String,
    /// The commit's id, `off` or `skipped: <why>`.
    pub committed: String,
}

/// What a call's source holds besides the minutes (both off unless ticked).
#[derive(Debug, Clone, Copy, Default)]
pub struct Include {
    pub transcript: bool,
    pub chat: bool,
}

/// A call on another AI profile than the vault's, added without *Add anyway* (X-P3).
#[derive(Debug)]
pub struct Mismatch {
    /// The profiles' ids: the call's, the vault's.
    pub call: String,
    pub vault: String,
    /// Their names, as the user knows them.
    pub call_name: String,
    pub vault_name: String,
}

impl std::fmt::Display for Mismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "this call's text goes only to the {} profile; this vault is read by the {} profile", self.call_name, self.vault_name)
    }
}

impl std::error::Error for Mismatch {}

/// A Teams chat, as its source names it.
pub struct Chat {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
}

fn now() -> String {
    Local::now().to_rfc3339()
}

/// `s` on one line: each run of whitespace and control characters made one space.
fn one_line(s: &str) -> String {
    s.split(|c: char| c.is_whitespace() || c.is_control()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// A profile's name as the user knows it (its id when it can't be found).
fn profile_name(id: &str) -> String {
    crate::ai::Settings::load().ok().and_then(|s| s.get(id).ok()).map(|p| p.name).unwrap_or_else(|| id.to_string())
}

fn file_sha(p: &Path) -> Result<String> {
    use sha2::Digest as _;
    use std::io::Read as _;
    let mut f = std::fs::File::open(p)?;
    let mut h = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 << 10];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|x| format!("{x:02x}")).collect())
}

// ── the ledger ──

fn ledger_file(v: &Vault) -> PathBuf {
    super::cache_dir(&v.id).join("ingest.json")
}

/// The vault's ledger; none yet reads as empty. One that can't be read is an error, never taken for empty.
pub fn ledger(v: &Vault) -> Result<Ledger> {
    match std::fs::read(ledger_file(v)) {
        Ok(b) => serde_json::from_slice(&b).with_context(|| format!("{} can't be read", ledger_file(v).display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::default()),
        Err(e) => Err(e.into()),
    }
}

/// Change the ledger under its lock, re-read first, so two writers never undo each other.
fn update_ledger<T>(v: &Vault, f: impl FnOnce(&mut Ledger) -> Result<T>) -> Result<T> {
    let dir = super::cache_dir(&v.id);
    std::fs::create_dir_all(&dir)?;
    let _held = export::hold(&dir.join("ingest.lock"), 20, "the ingest ledger is busy")?;
    let mut l = ledger(v)?;
    let out = f(&mut l)?;
    crate::paths::write_atomic(&ledger_file(v), serde_json::to_string_pretty(&l)?.as_bytes())?;
    Ok(out)
}

// ── the folder and its queue ──

/// The vault's sources folder, vault-relative: the one found or chosen, else `raw_inbox` (made by the first add).
pub fn folder(v: &Vault) -> String {
    v.path("sources").filter(|s| !s.is_empty()).unwrap_or_else(|| "raw_inbox".into())
}

fn folder_there(v: &Vault) -> bool {
    page::inside(v, &folder(v)).is_ok_and(|p| p.is_dir())
}

/// The starter's `raw_inbox/README.md`, written when Echo makes the folder (K14-01).
fn readme() -> &'static str {
    super::starter::FILES.iter().find(|(p, _)| *p == "raw_inbox/README.md").map(|(_, t)| *t).unwrap_or("")
}

/// Every file of the sources folder, vault-relative: its own `README.md`, dot-files, dot-folders and links left out.
fn files(v: &Vault) -> Vec<String> {
    let rel = folder(v);
    let Ok(dir) = page::inside(v, &rel) else { return vec![] };
    if !dir.is_dir() {
        return vec![];
    }
    super::detect::walk(&dir, &[], |r| !r.rsplit('/').next().unwrap_or(r).starts_with('.') && !r.eq_ignore_ascii_case("README.md"))
        .into_iter()
        .map(|r| format!("{rel}/{r}"))
        .collect()
}

/// The paths `log.md` says were processed: `- <date> · processed · <path> → <page>` (the starter's format).
fn processed(v: &Vault) -> HashSet<String> {
    let Some(log) = v.detected.log.as_ref() else { return HashSet::new() };
    let text = std::fs::read_to_string(super::join(&v.root, log)).unwrap_or_default();
    text.lines()
        .filter_map(|l| l.split_once("· processed ·").map(|(_, rest)| rest))
        .map(|rest| {
            let p = rest.split('→').next().unwrap_or("");
            p.split(" -> ").next().unwrap_or(p).trim().trim_matches('`').trim().to_string()
        })
        .filter(|p| !p.is_empty())
        .collect()
}

fn kind_of(inner: &str) -> Kind {
    match inner.split_once('/').map(|(d, _)| d) {
        Some("echo") => Kind::Echo,
        Some("mail") => Kind::Mail,
        Some("teams") => Kind::Teams,
        Some("web") => Kind::Web,
        _ => Kind::File,
    }
}

/// A Markdown source's own title (its frontmatter's, else its first `# ` heading), else the file's name.
fn title_of(full: &Path, name: &str) -> String {
    use std::io::Read as _;
    if name.to_ascii_lowercase().ends_with(".md") {
        let mut head = vec![];
        if std::fs::File::open(full).and_then(|f| f.take(64 << 10).read_to_end(&mut head)).is_ok() {
            let text = String::from_utf8_lossy(&head);
            let (fm, body) = super::fm::split(&text);
            let from_fm = fm.map(super::fm::parse).and_then(|p| super::fm::get(&p, "title").map(|t| one_line(&t.text())));
            if let Some(t) = from_fm.filter(|t| !t.is_empty()).or_else(|| body.lines().find_map(|l| l.strip_prefix("# ")).map(one_line).filter(|t| !t.is_empty())) {
                return t;
            }
        }
    }
    name.to_string()
}

fn state_of(e: Option<&Entry>, processed: bool) -> &'static str {
    match e.map(|e| e.state.as_str()) {
        Some("skipped") => "skipped",
        Some("done") => "done",
        _ if processed => "done",
        Some("waiting") => "waiting",
        _ => "new",
    }
}

/// The queue (K14-08): every file of the sources folder, newest first (when Echo learned of it, else when it changed).
pub fn queue(v: &Vault) -> Result<Vec<Source>> {
    let l = ledger(v)?;
    let done = processed(v);
    let base = folder(v);
    let mut out: Vec<(Option<DateTime<chrono::FixedOffset>>, Source)> = files(v)
        .into_iter()
        .map(|rel| {
            let full = super::join(&v.root, &rel);
            let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();
            let e = l.sources.get(&rel);
            let m = std::fs::metadata(&full).ok();
            let modified = m.as_ref().and_then(|m| m.modified().ok()).map(|t| DateTime::<Local>::from(t).to_rfc3339());
            let when = e.map(|e| e.added.clone()).filter(|a| !a.is_empty()).or(modified.clone()).and_then(|w| DateTime::parse_from_rfc3339(&w).ok());
            let meta = json!({
                "size": m.map(|m| m.len()), "modified": modified, "markdown": name.to_ascii_lowercase().ends_with(".md"),
                "added": e.map(|e| e.added.clone()), "by": e.map(|e| e.by.clone()), "note": e.and_then(|e| e.note.clone()),
                "run": e.and_then(|e| e.run.clone()), "kept": e.and_then(|e| e.kept), "anyway": e.is_some_and(|e| e.anyway),
            });
            let s = Source { kind: kind_of(&rel[base.len() + 1..]), title: title_of(&full, &name), state: state_of(e, done.contains(&rel)).into(), meta, path: rel };
            (when, s)
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    Ok(out.into_iter().map(|(_, s)| s).collect())
}

/// The first open's question (K14-09) while the vault has no ledger yet: how many files are there that `log.md`
/// doesn't call processed.
pub fn offer(v: &Vault) -> Result<Option<usize>> {
    if ledger_file(v).exists() {
        return Ok(None);
    }
    let done = processed(v);
    let n = files(v).iter().filter(|p| !done.contains(*p)).count();
    Ok((n > 0).then_some(n))
}

/// The first open's answer: the files already there that nothing calls done yet become Done (*Mark them done*) or
/// New (*Treat them as new*), and the question isn't asked again. How many were marked.
pub fn baseline(v: &Vault, done: bool) -> Result<usize> {
    super::refuse_headless()?;
    let processed = processed(v);
    // Read before the lock: a folder of large files takes a while.
    let found: Vec<(String, String)> = files(v).into_iter().filter(|p| !processed.contains(p)).map(|p| {
        let sha = file_sha(&super::join(&v.root, &p)).unwrap_or_default();
        (p, sha)
    }).collect();
    update_ledger(v, |l| {
        let at = now();
        let mut n = 0;
        for (p, sha256) in found {
            if let std::collections::btree_map::Entry::Vacant(slot) = l.sources.entry(p) {
                slot.insert(Entry { sha256, added: at.clone(), by: "baseline".into(), state: if done { "done" } else { "new" }.into(), ..Default::default() });
                n += 1;
            }
        }
        l.baseline = Some(Baseline { at, done });
        Ok(n)
    })
}

/// `path` from the page or the CLI as one of the sources folder's files (checked as any vault path, then found among
/// the folder's files: never its README, a dot-file or anything outside it).
fn source_path(v: &Vault, path: &str) -> Result<String> {
    page::inside(v, path)?;
    let rel = path.trim().to_string();
    if !files(v).contains(&rel) {
        bail!("{rel} isn't a source in {}/", folder(v));
    }
    Ok(rel)
}

/// *Skip*: done without a run, with the user's note (K14-10).
pub fn skip(v: &Vault, path: &str, note: Option<&str>) -> Result<()> {
    super::refuse_headless()?;
    let rel = source_path(v, path)?;
    let sha = file_sha(&super::join(&v.root, &rel))?;
    let note = note.map(one_line).filter(|n| !n.is_empty()).map(|n| n.chars().take(500).collect::<String>());
    update_ledger(v, |l| {
        let e = l.sources.entry(rel).or_insert_with(|| Entry { sha256: sha, added: now(), by: "folder".into(), ..Default::default() });
        e.state = "skipped".into();
        e.note = note;
        Ok(())
    })
}

/// What the Ingest view shows of the sources: the folder (and that Echo will make it), the first open's question,
/// the queue and its counts, the ledger's baseline and runs, and who reads what is added (X-P6).
pub fn value(v: &Vault) -> Result<Value> {
    let folder = folder(v);
    let there = folder_there(v);
    let l = ledger(v)?;
    let q = queue(v)?;
    let count = |s: &str| q.iter().filter(|x| x.state == s).count();
    Ok(json!({
        "folder": folder,
        "exists": there,
        "create": (!there).then(|| format!("Echo will create {folder}/ with a README")),
        "offer": offer(v)?.map(|n| json!({ "count": n, "folder": folder })),
        "counts": { "all": q.len(), "new": count("new"), "waiting": count("waiting"), "done": count("done"), "skipped": count("skipped") },
        "sources": q,
        "baseline": l.baseline,
        "runs": l.runs,
        "profile": v.profile,
        "profile_name": profile_name(&v.profile),
        "m365": crate::graph::account().is_some(),
        "max_file": MAX_FILE,
    }))
}

// ── adding ──

/// A file name safe on macOS and Windows: no `\ / : * ? " < > |` or control characters, no dot or space at either
/// end, not a Windows device name, its stem at most 120 characters.
fn safe_name(name: &str) -> String {
    let s: String = name.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '-' } else { c }).collect();
    let s = s.trim_matches(|c: char| c.is_whitespace() || c == '.');
    let (stem, ext) = match s.rfind('.') {
        Some(i) if i > 0 && s.len() - i <= 12 => (&s[..i], &s[i..]),
        _ => (s, ""),
    };
    let stem: String = stem.chars().take(120).collect();
    let stem = stem.trim_end_matches(|c: char| c.is_whitespace() || c == '.');
    let stem = if stem.is_empty() { "file" } else { stem };
    let first = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    let device = matches!(first.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (first.len() == 4 && (first.starts_with("COM") || first.starts_with("LPT")) && first.as_bytes()[3].is_ascii_digit());
    format!("{}{stem}{ext}", if device { "_" } else { "" })
}

/// `<date>-<slug>.md`, the slug of `title` (letters and digits of any script, lowercased, at most 60), else `fallback`.
fn dated_name(date: &str, title: &str, fallback: &str) -> String {
    let slug = crate::recorder::slugify(title);
    format!("{date}-{}.md", if slug.is_empty() { fallback } else { &slug })
}

/// `dir/name`, else `name (2)`, `name (3)`… — the first not there (the folder's own README is always taken).
fn free_name(v: &Vault, dir: &str, name: &str) -> Result<String> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    let top = dir == folder(v);
    for n in 1..1000 {
        let candidate = if n == 1 { name.to_string() } else { format!("{stem} ({n}){ext}") };
        let rel = format!("{dir}/{candidate}");
        let full = page::inside(v, &rel)?;
        if !(top && candidate.eq_ignore_ascii_case("README.md")) && std::fs::symlink_metadata(&full).is_err() {
            return Ok(rel);
        }
    }
    bail!("no free name for {name} in {dir}/")
}

/// What a source holds: text Echo wrote, or a file copied as it is.
enum Body<'a> {
    Text(String),
    File(&'a Path),
}

struct New<'a> {
    /// The folder in the sources folder (`echo`, `mail`, `teams`, `web`), or `""` for a file.
    sub: &'a str,
    name: String,
    title: String,
    by: &'a str,
    /// What it came from, for the activity line: a call, a message, a chat, a page.
    from: Option<String>,
    anyway: bool,
    body: Body<'a>,
}

/// `src` byte for byte at `to`, through a hidden file beside it (a reader never sees half a file).
fn copy_in(src: &Path, to: &Path) -> Result<()> {
    let name = to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = to.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    if let Err(e) = std::fs::copy(src, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!("copying {}: {e}", src.display()));
    }
    std::fs::rename(&tmp, to).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

/// Write one new source (and the README when Echo makes the folder), record it as New in the ledger and in the
/// activity log, and commit just those files when the vault commits (`Add source: <title>`, X-G1…G5).
fn write_source(v: &Vault, n: New) -> Result<Added> {
    super::refuse_headless()?;
    if !super::available(v) {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    let cache = super::cache_dir(&v.id);
    std::fs::create_dir_all(&cache)?;
    // One add at a time into a vault: a free name is chosen and taken together.
    let _held = export::hold(&cache.join("sources.lock"), 60, &format!("{} is busy adding a source", v.name))?;
    let base = folder(v);
    let there = folder_there(v);
    let dir = if n.sub.is_empty() { base.clone() } else { format!("{base}/{}", n.sub) };
    let rel = free_name(v, &dir, &n.name)?;
    let full = page::inside(v, &rel)?;
    let readme_rel = format!("{base}/README.md");
    let readme_full = if there { None } else { Some(page::inside(v, &readme_rel)?) };
    let mut targets = vec![full.clone()];
    targets.extend(readme_full.clone());
    // git first: which of these are changed already (none should be; one git can't answer for is, R30).
    let dirty = if v.git.commit { git::dirty(&v.root, &targets) } else { vec![] };
    std::fs::create_dir_all(full.parent().ok_or_else(|| anyhow!("{rel} has no folder"))?)?;
    let mut written: Vec<(String, PathBuf, Option<String>)> = vec![];
    if let Some(r) = readme_full.filter(|r| !r.exists()) {
        crate::paths::write_atomic(&r, readme().as_bytes())?;
        written.push((readme_rel, r, Some(readme().to_string())));
    }
    let text = match &n.body {
        Body::Text(t) => {
            crate::paths::write_atomic(&full, t.as_bytes())?;
            Some(t.clone())
        }
        Body::File(src) => {
            copy_in(src, &full)?;
            None
        }
    };
    written.push((rel.clone(), full.clone(), text));
    if v.path("sources").is_none() {
        super::found_sources(&v.id, &base)?;
    }
    let sha256 = file_sha(&full)?;
    update_ledger(v, |l| {
        l.sources.insert(rel.clone(), Entry { sha256, added: now(), by: n.by.into(), state: "new".into(), anyway: n.anyway, ..Default::default() });
        Ok(())
    })?;
    let title: String = n.title.chars().take(80).collect();
    let paths: Vec<PathBuf> = written.iter().map(|(_, p, _)| p.clone()).collect();
    let committed = page::commit_one(v, &paths, &dirty, &format!("Add source: {title}"));
    let commit = super::activity::commit_id(&committed);
    for (r, _, text) in &written {
        match text {
            Some(t) => super::activity::changed(v, r, "source", n.from.as_deref(), None, t, commit.clone()),
            None => {
                let e = super::activity::Entry { at: now(), path: r.clone(), kind: "new".into(), by: "source".into(), r#ref: n.from.clone(), plus: 0, minus: 0, commit: commit.clone() };
                if let Err(err) = super::activity::record(v, e) {
                    eprintln!("knowledge: activity of {r} in {}: {err:#}", v.id);
                }
            }
        }
    }
    Ok(Added { path: rel, committed })
}

/// Files the user picked in the system's dialog, each copied byte for byte into the sources folder as it is named
/// (a taken name gets ` (2)`); over 50 MB refused with the reason. Each with its own answer.
pub fn add_files(v: &Vault, picked: &[PathBuf]) -> Vec<(String, Result<Added>)> {
    picked.iter().map(|p| {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let r = add_file(v, p, &name);
        (name, r)
    }).collect()
}

fn add_file(v: &Vault, src: &Path, name: &str) -> Result<Added> {
    let m = std::fs::metadata(src).with_context(|| format!("{name} can't be read"))?;
    if !m.is_file() {
        bail!("{name} isn't a file");
    }
    if m.len() > MAX_FILE {
        bail!("{name} is {} MB — Echo adds files up to 50 MB", (m.len() as f64 / (1 << 20) as f64).round());
    }
    let safe = safe_name(name);
    write_source(v, New { sub: "", name: safe.clone(), title: safe, by: "file", from: None, anyway: false, body: Body::File(src) })
}

/// A call's or memo's minutes as a source (K14-04, §5.5): `echo/<date>-<slug>.md`, the transcript and chat only when
/// ticked, `note:` when the call has a note in this vault. A call on another profile than the vault's needs `anyway`.
pub fn add_call(v: &Vault, call: &Path, include: Include, anyway: bool) -> Result<Added> {
    super::refuse_headless()?;
    let f = export::facts(call)?;
    let profile = f.profile.clone().ok_or_else(|| anyhow!("this call's AI profile can't be read"))?;
    let across = profile != v.profile;
    if across && !anyway {
        return Err(Mismatch { call_name: profile_name(&profile), vault_name: profile_name(&v.profile), call: profile, vault: v.profile.clone() }.into());
    }
    let memo = crate::memo::is_memo(call);
    let title = Some(one_line(&f.title)).filter(|t| !t.is_empty()).unwrap_or_else(|| "Call".into());
    let mut fm = vec![format!("title: {}", export::yaml(&title)), format!("date: {}", f.date), format!("kind: {}", if memo { "memo" } else { "call" })];
    if !f.participants.is_empty() {
        fm.push(format!("participants: {}", export::yaml(&one_line(&f.participants.join(", ")))));
    }
    if f.minutes > 0 {
        fm.push(format!("duration: {}m", f.minutes));
    }
    fm.push(format!("source: echo://call/{}", f.stamp));
    if let Some(n) = export::note_in(call, v) {
        fm.push(format!("note: {}", export::yaml(&n)));
    }
    fm.push("origin: echo".into());
    let mut text = format!("---\n{}\n---\n\n# {title} — minutes\n\n{}\n", fm.join("\n"), f.notes.trim_end());
    if include.transcript {
        if let Ok(t) = std::fs::read_to_string(call.join("transcript.md")) {
            text.push_str(&format!("\n## Transcript\n\n{}\n", t.trim()));
        }
    }
    if include.chat {
        if let Some(c) = export::chat_text(call) {
            text.push_str(&format!("\n## Chat\n\n{}\n", c.trim()));
        }
    }
    let name = dated_name(&f.date, &title, if memo { "memo" } else { "call" });
    let from = call.file_name().map(|n| n.to_string_lossy().into_owned());
    write_source(v, New { sub: "echo", name, title, by: "call", from, anyway: across, body: Body::Text(text) })
}

/// A local time as a section shows it.
fn local_time(at: &str) -> String {
    DateTime::parse_from_rfc3339(at).map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_else(|_| at.to_string())
}

fn local_date(at: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(at).ok().map(|t| t.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

/// The mail thread of message `id`, from Echo's Microsoft 365 sign-in (K14-05).
pub fn add_mail(v: &Vault, id: &str) -> Result<Added> {
    super::refuse_headless()?;
    let picked = crate::graph::message(id)?;
    let conv = picked["conversation"].as_str().ok_or_else(|| anyhow!("that mail has no thread"))?;
    let messages = crate::graph::conversation(conv)?;
    add_thread(v, &picked, &messages)
}

/// A thread (`graph::mail_item`s, oldest first) as `mail/<date>-<subject>.md`: one `### <name> · <date time>` section
/// per message, its text without tags, its attachments named (never downloaded). `source:` is the picked message's link.
pub(crate) fn add_thread(v: &Vault, picked: &Value, messages: &[Value]) -> Result<Added> {
    if messages.is_empty() {
        bail!("that thread has no messages Echo can read");
    }
    let subject = one_line(messages[0]["subject"].as_str().or(picked["subject"].as_str()).unwrap_or(""));
    let title = if subject.is_empty() { "(no subject)".to_string() } else { subject.clone() };
    let newest = messages.iter().filter_map(|m| m["at"].as_str()).filter_map(|a| DateTime::parse_from_rfc3339(a).ok()).max();
    let date = newest.and_then(|t| local_date(&t.to_rfc3339())).unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());
    let name_of = |p: &Value| one_line(p["name"].as_str().filter(|n| !n.trim().is_empty()).or(p["email"].as_str()).unwrap_or(""));
    let mut people: Vec<String> = vec![];
    for m in messages {
        for p in std::iter::once(&m["from"]).chain(m["to"].as_array().into_iter().flatten()) {
            let n = name_of(p);
            if !n.is_empty() && !people.contains(&n) {
                people.push(n);
            }
        }
    }
    let mut fm = vec![format!("title: {}", export::yaml(&title)), format!("date: {date}"), "kind: mail".to_string()];
    if !people.is_empty() {
        fm.push(format!("people: {}", export::yaml(&people.join(", "))));
    }
    if let Some(u) = picked["url"].as_str().map(one_line).filter(|u| !u.is_empty()) {
        fm.push(format!("source: {}", export::yaml(&u)));
    }
    fm.push("origin: echo".into());
    let mut text = format!("---\n{}\n---\n\n# {title}\n", fm.join("\n"));
    for m in messages {
        let who = Some(name_of(&m["from"])).filter(|n| !n.is_empty()).unwrap_or_else(|| "Someone".into());
        text.push_str(&format!("\n### {who} · {}\n\n", local_time(m["at"].as_str().unwrap_or(""))));
        let body = m["text"].as_str().unwrap_or("").trim();
        if !body.is_empty() {
            text.push_str(body);
            text.push('\n');
        }
        let files: Vec<String> = m["files"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(one_line).filter(|f| !f.is_empty()).collect();
        if !files.is_empty() {
            text.push_str(&format!("\nAttachments: {}\n", files.join(", ")));
        }
    }
    let from = picked["id"].as_str().map(str::to_owned);
    write_source(v, New { sub: "mail", name: dated_name(&date, &subject, "mail"), title, by: "mail", from, anyway: false, body: Body::Text(text) })
}

/// A Teams chat's messages from `since` to `until` (local days, both included), from Echo's Microsoft 365 sign-in.
pub fn add_chat(v: &Vault, chat: &str, since: NaiveDate, until: NaiveDate) -> Result<Added> {
    super::refuse_headless()?;
    if since > until {
        bail!("the days end before they start");
    }
    let info = crate::graph::request("GET", &format!("/chats/{}?$expand=members", crate::m365::enc(chat)), crate::graph::Body::None, None, &[])?;
    let me = crate::graph::account().unwrap_or_default().to_lowercase();
    let members: Vec<String> = info["members"].as_array().into_iter().flatten()
        .filter(|m| !m["email"].as_str().is_some_and(|e| !me.is_empty() && e.to_lowercase() == me))
        .filter_map(|m| m["displayName"].as_str().map(one_line)).filter(|n| !n.is_empty()).collect();
    let name = info["topic"].as_str().map(one_line).filter(|t| !t.is_empty()).unwrap_or_else(|| if members.is_empty() { "Teams chat".into() } else { members.join(", ") });
    let day = |d: NaiveDate| Local.from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default()).earliest().map(|t| t.with_timezone(&Utc));
    let from = day(since).ok_or_else(|| anyhow!("{since} isn't a day here"))?;
    let to = day(until + chrono::Duration::days(1)).ok_or_else(|| anyhow!("{until} isn't a day here"))?;
    let messages = crate::graph::chat_messages(chat, from, to)?;
    add_chat_messages(v, &Chat { id: chat.to_string(), name, url: info["webUrl"].as_str().map(str::to_owned) }, since, until, &messages)
}

/// A chat's messages (Graph's, oldest first) as `teams/<until>-<chat>.md`: the chat's name and days, one
/// `### <name> · <date time>` section per message (its text through `graph::html_to_text`), attachments named.
pub(crate) fn add_chat_messages(v: &Vault, chat: &Chat, since: NaiveDate, until: NaiveDate, messages: &[Value]) -> Result<Added> {
    if since > until {
        bail!("the days end before they start");
    }
    if messages.is_empty() {
        bail!("nothing was written in {} from {since} to {until}", one_line(&chat.name));
    }
    let title = Some(one_line(&chat.name)).filter(|t| !t.is_empty()).unwrap_or_else(|| "Teams chat".into());
    let who = |m: &Value| one_line(m["from"]["user"]["displayName"].as_str().or(m["from"]["application"]["displayName"].as_str()).unwrap_or(""));
    let mut people: Vec<String> = vec![];
    for m in messages {
        let n = who(m);
        if !n.is_empty() && !people.contains(&n) {
            people.push(n);
        }
    }
    let mut fm = vec![format!("title: {}", export::yaml(&title)), format!("date: {until}"), "kind: teams".to_string(), format!("since: {since}"), format!("until: {until}")];
    if !people.is_empty() {
        fm.push(format!("people: {}", export::yaml(&people.join(", "))));
    }
    if let Some(u) = chat.url.as_deref().map(one_line).filter(|u| !u.is_empty()) {
        fm.push(format!("source: {}", export::yaml(&u)));
    }
    fm.push("origin: echo".into());
    let mut text = format!("---\n{}\n---\n\n# {title}\n", fm.join("\n"));
    for m in messages {
        let name = Some(who(m)).filter(|n| !n.is_empty()).unwrap_or_else(|| "Someone".into());
        text.push_str(&format!("\n### {name} · {}\n\n", local_time(m["createdDateTime"].as_str().unwrap_or(""))));
        let content = m["body"]["content"].as_str().unwrap_or("");
        let body = if m["body"]["contentType"].as_str().is_some_and(|t| t.eq_ignore_ascii_case("text")) { content.trim().to_string() } else { crate::graph::html_to_text(content) };
        if !body.is_empty() {
            text.push_str(&body);
            text.push('\n');
        }
        let files: Vec<String> = m["attachments"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).map(one_line).filter(|f| !f.is_empty()).collect();
        if !files.is_empty() {
            text.push_str(&format!("\nAttachments: {}\n", files.join(", ")));
        }
    }
    let name = dated_name(&until.to_string(), &chat.name, "chat");
    write_source(v, New { sub: "teams", name, title, by: "teams", from: Some(chat.id.clone()), anyway: false, body: Body::Text(text) })
}

/// A web page by its https address (K14-06): fetched with `curl`, its main text as `web/<date>-<slug>.md`.
pub fn add_web(v: &Vault, url: &str) -> Result<Added> {
    super::refuse_headless()?;
    let url = url.trim();
    let (title, md) = super::extract::fetch(url)?;
    let title = Some(one_line(&title)).filter(|t| !t.is_empty()).unwrap_or_else(|| url.to_string());
    let date = Local::now().format("%Y-%m-%d").to_string();
    let fm = [format!("title: {}", export::yaml(&title)), format!("date: {date}"), "kind: web".into(), format!("url: {}", export::yaml(url)), format!("fetched: {}", now()), "origin: echo".into()];
    let text = format!("---\n{}\n---\n\n# {title}\n\n{}\n", fm.join("\n"), md.trim());
    write_source(v, New { sub: "web", name: dated_name(&date, &title, "page"), title, by: "web", from: Some(url.to_string()), anyway: false, body: Body::Text(text) })
}

// ── what the sheet lists ──

/// Recent calls and memos with minutes, newest first, whose title, people or subject hold every word of `q` (at most
/// `limit`): each with its date, people, AI profile and whether that is the vault's, and its note in this vault.
pub fn calls(v: &Vault, q: &str, limit: usize) -> Vec<Value> {
    let words: Vec<String> = q.to_lowercase().split_whitespace().map(str::to_owned).collect();
    let mut out = vec![];
    for d in crate::calls::all_calls().into_iter().rev() {
        if out.len() >= limit {
            break;
        }
        if !d.join("notes.md").is_file() {
            continue;
        }
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if !words.is_empty() {
            let meta = crate::calls::read_json(&d.join("meta.json"));
            let meeting = crate::calls::read_json(&d.join("meeting.json"));
            let people: Vec<&str> = meeting["attendees"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str()).collect();
            let hay = format!("{} {} {} {}", meta["title"].as_str().unwrap_or(""), name.replace('-', " "), meeting["subject"].as_str().unwrap_or(""), people.join(" ")).to_lowercase();
            if !words.iter().all(|w| hay.contains(w.as_str())) {
                continue;
            }
        }
        let Ok(f) = export::facts(&d) else { continue };
        out.push(json!({
            "name": name, "title": f.title, "date": f.date, "people": f.participants, "memo": crate::memo::is_memo(&d),
            "profile": f.profile, "profile_name": f.profile.as_deref().map(profile_name),
            "profile_ok": f.profile.as_deref() == Some(v.profile.as_str()),
            "transcript": d.join("transcript.md").is_file(), "chat": d.join("chat.json").is_file(), "note": export::note_in(&d, v),
        }));
    }
    out
}

/// Mail and chats matching `q` in the last 90 days, from Echo's Microsoft 365 sign-in; without one, only that.
pub fn m365_search(q: &str) -> Result<Value> {
    if crate::graph::account().is_none() {
        return Ok(json!({ "signed_in": false, "results": [] }));
    }
    if q.trim().is_empty() {
        return Ok(json!({ "signed_in": true, "results": [] }));
    }
    let r = crate::m365::search(q, &["mail".to_string(), "chats".to_string()], 25, None, Some(90))?;
    // A chat's messages, not a channel's posts: a chat is what can be added.
    let results: Vec<Value> = r["results"].as_array().into_iter().flatten().filter(|h| h["kind"] == "mail" || h["chat"].is_string()).cloned().collect();
    Ok(json!({ "signed_in": true, "results": results, "errors": r["errors"] }))
}

#[cfg(test)]
mod tests;

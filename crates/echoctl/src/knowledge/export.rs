//! A call's minutes as a page of a vault (spec K3): written by Echo from `notes.md` and the call's facts — no model —
//! in the vault's notes folder and naming, listed in its index, logged when the vault keeps a `log.md`, committed only
//! where the vault allows it. A note the user changed after Echo wrote it is never written over. What was saved where
//! is the call's `knowledge.json`.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::{catalog, checks, git, scan, Vault};

/// The record of a call's saves, `<call>/knowledge.json`.
const RECORD: &str = "knowledge.json";

/// How a save was asked for.
#[derive(Debug, Clone, Default)]
pub struct Manual {
    /// A new note beside one the user changed.
    pub new: bool,
    /// Write over the note even though the user changed it.
    pub replace: bool,
    /// The user agreed to save across AI profiles.
    pub anyway: bool,
    /// The file's name without `.md`.
    pub stem: Option<String>,
    pub transcript: Option<bool>,
    pub chat: Option<bool>,
}

#[derive(Debug, Clone)]
pub enum How {
    /// By the vault's rule, after the minutes were written.
    Auto,
    /// The user asked (a call's ⋯ → Save to Knowledge, the CLI).
    Manual(Manual),
}

#[cfg(test)]
impl How {
    pub fn manual() -> How {
        How::Manual(Manual::default())
    }
    fn map(self, f: impl FnOnce(&mut Manual)) -> How {
        match self {
            How::Manual(mut m) => {
                f(&mut m);
                How::Manual(m)
            }
            auto => auto,
        }
    }
    pub fn new_note(self) -> How {
        self.map(|m| m.new = true)
    }
    pub fn replace(self) -> How {
        self.map(|m| m.replace = true)
    }
    pub fn anyway(self) -> How {
        self.map(|m| m.anyway = true)
    }
}

/// What a save did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Saved {
    pub vault: String,
    /// Vault-relative.
    pub path: String,
    /// `saved` (a new note), `updated`, `unchanged`, `kept` (the user changed it; not written), `skipped`.
    pub state: String,
    /// `added`, `listed`, or `skipped: <why>`.
    pub index: String,
    /// The commit's id, `skipped: <why>` or `off`.
    pub commit: String,
}

/// A save into a vault read by another AI profile than the call's, not agreed to.
#[derive(Debug)]
pub struct ProfileMismatch {
    pub call: String,
    pub vault: String,
}

impl std::fmt::Display for ProfileMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "this call's text goes only to the {} profile; this vault is read by the {} profile", self.call, self.vault)
    }
}

impl std::error::Error for ProfileMismatch {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Entry {
    vault: String,
    path: String,
    sha256: String,
    at: String,
    /// `auto` or `manual`.
    how: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    index: String,
    #[serde(default)]
    commit: String,
    #[serde(default)]
    anyway: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Failure {
    vault: String,
    at: String,
    error: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Record {
    /// `none` (don't save this call), a vault id (only there), or nothing (the vaults' rules).
    #[serde(default)]
    choice: Option<String>,
    #[serde(default)]
    saved: Vec<Entry>,
    #[serde(default)]
    failed: Vec<Failure>,
}

fn load(call: &Path) -> Record {
    std::fs::read(call.join(RECORD)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The lock at `path`, waited for up to `secs`; `busy` says what is busy when it isn't free by then.
fn hold(path: &Path, secs: u64, busy: &str) -> Result<crate::app::FileLock> {
    let t = std::time::Instant::now();
    loop {
        if let Some(l) = crate::app::FileLock::try_take(path)? {
            return Ok(l);
        }
        if t.elapsed() > std::time::Duration::from_secs(secs) {
            bail!("{busy} — try again in a moment");
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}

/// One save at a time into a vault — the note, its index row, the log line and the call's record together — across
/// Echo's window, its CLI and its threads (else two saves read the same index and one row is lost).
fn vault_lock(v: &Vault) -> Result<crate::app::FileLock> {
    let dir = super::cache_dir(&v.id);
    std::fs::create_dir_all(&dir)?;
    hold(&dir.join("save.lock"), 60, &format!("{} is busy saving", v.name))
}

/// Change the call's record under its lock, re-read first.
fn update<T>(call: &Path, f: impl FnOnce(&mut Record) -> T) -> Result<T> {
    let _held = hold(&call.join(".knowledge.lock"), 20, "this call's record is busy")?;
    let mut r = load(call);
    let out = f(&mut r);
    crate::paths::write_atomic(&call.join(RECORD), serde_json::to_string_pretty(&r)?.as_bytes())?;
    Ok(out)
}

/// The call's record as clients show it.
#[cfg(any(test, feature = "tray"))]
pub fn record(call: &Path) -> Value {
    serde_json::to_value(load(call)).unwrap_or_default()
}

fn sha(b: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// A YAML scalar as written in Echo's notes: plain when that reads back the same, else double-quoted.
pub(super) fn yaml(s: &str) -> String {
    let plain = !s.is_empty()
        && !s.starts_with(|c: char| "[]{}&*!|>'\"%@`#,?:-".contains(c) || c.is_whitespace())
        && !s.ends_with(char::is_whitespace)
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.ends_with(':')
        && !s.contains(['\n', '"', '\''])
        && !["true", "false", "yes", "no", "null", "on", "off", "~"].contains(&s.to_ascii_lowercase().as_str());
    if plain { s.to_string() } else { quoted(s) }
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " "))
}

/// A call's facts the note is written from.
struct Facts {
    title: String,
    date: String,
    minutes: i64,
    meeting: String,
    participants: Vec<String>,
    client: Option<String>,
    threads: Vec<String>,
    link: String,
    stamp: String,
    profile: Option<String>,
    notes: String,
}

fn facts(call: &Path) -> Result<Facts> {
    let notes = std::fs::read_to_string(call.join("notes.md")).map_err(|_| anyhow!("this call has no minutes yet"))?;
    let meta = crate::calls::read_json(&call.join("meta.json"));
    let name = call.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let title = meta["title"].as_str().map(str::trim).filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| "Call".into());
    let start = meta["started_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
    let end = meta["ended_at"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
    let date = start.map(|s| s.with_timezone(&chrono::Local).format("%Y-%m-%d").to_string()).or_else(|| name.get(..10).map(str::to_owned)).unwrap_or_else(today);
    let minutes = match (start, end) {
        (Some(s), Some(e)) => ((e - s).num_seconds() as f64 / 60.0).round() as i64,
        _ => 0,
    };
    let memo = crate::memo::is_memo(call);
    let platform = match meta["app"].as_str().unwrap_or("") {
        "teams" => Some("MS Teams"),
        "zoom" => Some("Zoom"),
        "meet" | "google-meet" => Some("Google Meet"),
        "slack" => Some("Slack"),
        "webex" => Some("Webex"),
        _ => None,
    };
    let meeting = if memo { "Memo".to_string() } else { platform.map(|p| format!("{title} ({p})")).unwrap_or_else(|| title.clone()) };
    let m = crate::calls::read_json(&call.join("meeting.json"));
    let mut participants: Vec<String> = m["attendees"].as_array().into_iter().flatten().filter_map(|a| a["name"].as_str().or(a.as_str())).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    participants.dedup();
    let (mut client, mut threads) = (None, vec![]);
    if let Ok(ps) = crate::projects::of_call(call) {
        let clients = crate::projects::clients().unwrap_or_default();
        for (p, _) in &ps {
            threads.push(p.name.clone());
            if client.is_none() {
                client = p.client.as_ref().and_then(|c| clients.iter().find(|x| &x.id == c)).map(|c| c.name.clone());
            }
        }
    }
    Ok(Facts {
        title,
        date,
        minutes,
        meeting,
        participants,
        client,
        threads,
        link: format!("echo://call/{name}"),
        stamp: crate::calls::stamp_of(call),
        profile: crate::ai::call_profile_id(call).ok(),
        notes,
    })
}

/// The first paragraph — or the first list, items joined with "; " — of the section headed Summary, else of the first
/// `##` section. At most 300 characters, one line.
pub fn summary(notes: &str) -> String {
    let lines: Vec<&str> = notes.lines().collect();
    let heads: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.starts_with("## ")).map(|(i, _)| i).collect();
    let Some(&start) = heads.iter().find(|&&i| lines[i][3..].trim().eq_ignore_ascii_case("summary")).or_else(|| heads.first()) else { return String::new() };
    let mut items: Vec<String> = vec![];
    let mut para: Vec<String> = vec![];
    for l in &lines[start + 1..] {
        let t = l.trim();
        if t.starts_with('#') {
            break;
        }
        if t.is_empty() {
            if !para.is_empty() || !items.is_empty() {
                break;
            }
            continue;
        }
        let item = t.strip_prefix("- [ ] ").or_else(|| t.strip_prefix("- [x] ")).or_else(|| t.strip_prefix("- ")).or_else(|| t.strip_prefix("* "));
        match item {
            Some(i) if para.is_empty() => items.push(i.trim().to_string()),
            _ if !items.is_empty() => break,
            _ => para.push(t.to_string()),
        }
    }
    let text = if items.is_empty() { para.join(" ") } else { items.join("; ") };
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() > 300 {
        format!("{}…", text.chars().take(299).collect::<String>().trim_end())
    } else {
        text
    }
}

/// `notes.md` without its title and the date / participants line that opens the minutes (the note's properties carry
/// both); every other line — a lead paragraph, the blank lines between paragraphs — as written.
fn body(notes: &str) -> String {
    let mut lines: Vec<&str> = notes.lines().collect();
    if lines.first().is_some_and(|l| l.starts_with("# ")) {
        lines.remove(0);
    }
    let dated = |l: &str| l.len() >= 10 && l.as_bytes()[..10].iter().enumerate().all(|(i, b)| if i == 4 || i == 7 { *b == b'-' } else { b.is_ascii_digit() });
    if let Some(i) = lines.iter().position(|l| !l.trim().is_empty()).filter(|&i| dated(lines[i].trim()) && !lines[i].starts_with('#')) {
        lines.remove(i);
    }
    lines.join("\n").trim().to_string()
}

/// A file name for the note in the vault's naming.
fn file_name(naming: &str, date: &str, title: &str, stem: Option<&str>) -> String {
    if let Some(s) = stem.map(str::trim).filter(|s| !s.is_empty()) {
        let clean: String = s.chars().map(|c| if "/\\:*?\"<>|#".contains(c) || c.is_control() { '-' } else { c }).collect();
        return format!("{}.md", clean.trim_matches(|c: char| c == '.' || c.is_whitespace()).trim_end_matches(".md"));
    }
    if naming == super::detect::DATE_TITLE {
        let t: String = title.chars().map(|c| if "/\\:*?\"<>|#".contains(c) || c.is_control() { '-' } else { c }).take(80).collect();
        return format!("{date} {}.md", t.trim().trim_matches('.'));
    }
    let slug = crate::recorder::slugify(title);
    format!("{date}-{}.md", if slug.is_empty() { "call" } else { &slug })
}

/// The vault's page whose title, file name or aliases equal `name`, as a wikilink target.
fn page_link(v: &Vault, name: &str) -> Option<String> {
    let ix = scan::index(v).ok()?;
    let lc = name.to_lowercase();
    let p = ix.pages.values().find(|p| {
        let stem = p.path.rsplit('/').next().unwrap_or(&p.path).trim_end_matches(".md").to_lowercase();
        p.title.to_lowercase() == lc || stem == lc || super::fm::get(&p.props, "aliases").is_some_and(|a| a.items().iter().any(|x| x.to_lowercase() == lc))
    })?;
    let stem = p.path.rsplit('/').next().unwrap_or(&p.path).trim_end_matches(".md").to_string();
    Some(if stem == name { format!("[[{name}]]") } else { format!("[[{stem}|{name}]]") })
}

/// The note's text.
fn note_text(v: &Vault, f: &Facts, transcript: Option<&str>, chat: Option<&str>) -> String {
    let mut fm: Vec<String> = vec![
        format!("title: {}", quoted(&f.title)),
        format!("date: {}", f.date),
        format!("meeting: {}", quoted(&f.meeting)),
        format!("tags: [{}]", yaml(&checks::meeting_tag(v))),
    ];
    if !f.participants.is_empty() {
        fm.push(format!("participants: {}", yaml(&f.participants.join(", "))));
    }
    if f.minutes > 0 {
        fm.push(format!("duration: {}m", f.minutes));
    }
    if let Some(c) = &f.client {
        fm.push(format!("client: {}", quoted(&page_link(v, c).unwrap_or_else(|| c.clone()))));
    }
    if !f.threads.is_empty() {
        fm.push(format!("threads: [{}]", f.threads.iter().map(|t| quoted(t)).collect::<Vec<_>>().join(", ")));
    }
    fm.push(format!("source: {}", f.link));
    fm.push("origin: echo".into());
    fm.push(format!("echo_call: {}", f.stamp));
    fm.push(format!("summary: {}", quoted(&summary(&f.notes))));
    let mut text = format!("---\n{}\n---\n\n# {} — {}\n\n{}\n", fm.join("\n"), f.title, f.date, body(&f.notes));
    if let Some(t) = transcript {
        let t = t.lines().skip_while(|l| l.starts_with("# ") || l.starts_with("- **") || l.trim().is_empty()).collect::<Vec<_>>().join("\n");
        text.push_str(&format!("\n## Transcript\n\n{}\n", t.trim()));
    }
    if let Some(c) = chat {
        text.push_str(&format!("\n## Chat\n\n{}\n", c.trim()));
    }
    text
}

fn chat_text(call: &Path) -> Option<String> {
    let v = crate::calls::read_json(&call.join("chat.json"));
    let lines: Vec<String> = v.as_array()?.iter().filter_map(|m| {
        let text = m["text"].as_str()?.trim();
        (!text.is_empty()).then(|| format!("- **{}**: {}", m["from"].as_str().unwrap_or("Someone"), text.replace('\n', " ")))
    }).collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// `b` relative to the folder of `a` (both vault-relative).
fn relative_to(a: &str, b: &str) -> String {
    let dir: Vec<&str> = a.rsplit_once('/').map(|(d, _)| d.split('/').collect()).unwrap_or_default();
    let target: Vec<&str> = b.split('/').collect();
    let common = dir.iter().zip(&target).take_while(|(x, y)| x == y).count();
    let mut parts: Vec<&str> = std::iter::repeat_n("..", dir.len() - common).collect();
    parts.extend(&target[common..]);
    parts.join("/")
}

/// Does the vault's rule take this call (its mode aside)?
fn in_scope(v: &Vault, call: &Path, memo: bool) -> bool {
    match v.minutes.scope.as_str() {
        "calls" => !memo,
        "some" => {
            let ps = crate::projects::of_call(call).unwrap_or_default();
            let clients = crate::projects::clients().unwrap_or_default();
            let lc = |s: &str| s.trim().to_lowercase();
            ps.iter().any(|(p, _)| {
                v.minutes.threads.iter().any(|t| lc(t) == lc(&p.id) || lc(t) == lc(&p.name))
                    || p.client.as_ref().is_some_and(|cid| {
                        let cname = clients.iter().find(|c| &c.id == cid).map(|c| c.name.clone()).unwrap_or_default();
                        v.minutes.clients.iter().any(|c| lc(c) == lc(cid) || lc(c) == lc(&cname))
                    })
            })
        }
        _ => true,
    }
}

/// The vaults that take this call's minutes by themselves and whose folder is there — what the call's chip promises.
pub fn targets(call: &Path) -> Vec<Vault> {
    by_rule(call).into_iter().filter(super::available).collect()
}

/// The vaults that take this call's minutes by themselves: the one chosen for it, else every vault whose rule is
/// `auto` and takes it — always on the call's own AI profile. A vault whose folder is gone stays in: its save is
/// tried, so the failure is recorded and shown.
fn by_rule(call: &Path) -> Vec<Vault> {
    let rec = load(call);
    if rec.choice.as_deref() == Some("none") {
        return vec![];
    }
    let Ok(profile) = crate::ai::call_profile_id(call) else { return vec![] };
    let memo = crate::memo::is_memo(call);
    super::list()
        .unwrap_or_default()
        .into_iter()
        .filter(|v| v.profile == profile)
        .filter(|v| match &rec.choice {
            Some(id) => &v.id == id,
            None => v.minutes.mode == "auto" && in_scope(v, call, memo),
        })
        .collect()
}

/// For this call only: `Some("none")` — don't save it; `Some(id)` — save it only there; `None` — the vaults' rules.
pub fn choose(call: &Path, choice: Option<&str>) -> Result<()> {
    super::refuse_headless()?;
    if let Some(id) = choice.filter(|c| *c != "none") {
        super::get(id)?;
    }
    update(call, |r| r.choice = choice.map(str::to_owned))
}

/// Save the minutes into every vault that takes them by itself (after they are written). Failures are recorded on the
/// call (the Inbox shows them); the minutes are never touched.
pub fn save_by_rules(call: &Path) -> Vec<Saved> {
    let rec = load(call);
    let mut out = vec![];
    for v in by_rule(call) {
        // A note saved by hand isn't updated by itself (D17); a note the user deleted isn't put back.
        if let Some(e) = rec.saved.iter().find(|e| e.vault == v.id) {
            if e.how != "auto" || !super::join(&v.root, &e.path).exists() {
                continue;
            }
        }
        match save(call, &v.id, How::Auto) {
            Ok(s) => out.push(s),
            Err(e) => eprintln!("knowledge: minutes of {} → {}: {e:#}", call.display(), v.id),
        }
    }
    out
}

/// After a call's minutes are written (an Echo run or a session's `/echo:process-call`): the names of the vaults that
/// will take them, saved on a thread of their own.
pub fn minutes_written(call: &Path) -> Vec<String> {
    let names: Vec<String> = targets(call).into_iter().map(|v| v.name).collect();
    if !by_rule(call).is_empty() {
        let call = call.to_path_buf();
        std::thread::spawn(move || {
            save_by_rules(&call);
            crate::attention::stale();
        });
    }
    names
}

/// Vaults the call was saved to by hand whose note is now older than the minutes (offered: Update).
#[cfg(any(test, feature = "tray"))]
pub fn stale(call: &Path) -> Vec<String> {
    let rec = load(call);
    let Ok(f) = facts(call) else { return vec![] };
    rec.saved
        .iter()
        .filter(|e| e.how == "manual")
        .filter_map(|e| {
            let v = super::get(&e.vault).ok()?;
            let now = std::fs::read(super::join(&v.root, &e.path)).ok()?;
            // Unchanged by the user, and different from what the minutes would make now.
            (sha(&now) == e.sha256 && sha(note_text(&v, &f, None, None).as_bytes()) != e.sha256).then(|| e.vault.clone())
        })
        .collect()
}

/// The last failed save of this call, if it wasn't followed by a good one: (vault id, vault name, why).
pub fn failure(call: &Path) -> Option<(String, String, String)> {
    let f = load(call).failed.into_iter().last()?;
    let name = super::get(&f.vault).map(|v| v.name).unwrap_or_else(|_| f.vault.clone());
    Some((f.vault, name, f.error))
}

fn fail(call: &Path, vault: &str, e: &anyhow::Error) {
    let error = format!("{e:#}");
    let _ = update(call, |r| {
        r.failed.retain(|f| f.vault != vault);
        r.failed.push(Failure { vault: vault.into(), at: chrono::Local::now().to_rfc3339(), error });
    });
}

/// Save a call's minutes into vault `id`.
pub fn save(call: &Path, id: &str, how: How) -> Result<Saved> {
    let v = super::get(id)?;
    let out = vault_lock(&v).and_then(|_held| save_into(call, &v, how, true).map(|(s, _)| s));
    match &out {
        Ok(_) => {
            let _ = update(call, |r| r.failed.retain(|f| f.vault != v.id));
        }
        Err(e) if e.downcast_ref::<ProfileMismatch>().is_none() => fail(call, &v.id, e),
        Err(_) => {}
    }
    out
}

/// The work of a save; `commit: false` leaves the commit to the caller (past minutes, one commit for all), returning the
/// paths written that may be committed.
fn save_into(call: &Path, v: &Vault, how: How, commit: bool) -> Result<(Saved, Vec<PathBuf>)> {
    if !super::available(v) {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    let f = facts(call)?;
    let manual = match &how {
        How::Manual(m) => Some(m.clone()),
        How::Auto => None,
    };
    let anyway = manual.as_ref().is_some_and(|m| m.anyway);
    if f.profile.as_deref() != Some(v.profile.as_str()) {
        let mismatch = ProfileMismatch { call: f.profile.clone().unwrap_or_else(|| "unknown".into()), vault: v.profile.clone() };
        if manual.is_none() || !anyway || f.profile.is_none() {
            return Err(mismatch.into());
        }
    }
    let rec = load(call);
    let entry = rec.saved.iter().find(|e| e.vault == v.id).cloned();
    let notes_dir = notes_dir_of(v);
    let naming = v.path("naming").unwrap_or_else(|| super::detect::DATE_SLUG.into());
    let transcript = manual.as_ref().and_then(|m| m.transcript).unwrap_or(v.minutes.transcript).then(|| std::fs::read_to_string(call.join("transcript.md")).ok()).flatten();
    let chat = manual.as_ref().and_then(|m| m.chat).unwrap_or(v.minutes.chat).then(|| chat_text(call)).flatten();
    let text = note_text(v, &f, transcript.as_deref(), chat.as_deref());
    let new_note = manual.as_ref().is_some_and(|m| m.new);
    let replace = manual.as_ref().is_some_and(|m| m.replace);
    // git is asked first: from the check that the note is as Echo left it to the write, nothing slow runs.
    let index_rel = v.path("index");
    let log_rel = v.detected.log.clone();
    let candidates: Vec<PathBuf> = [entry.as_ref().filter(|_| !new_note).map(|e| super::join(&v.root, &e.path)), index_rel.as_ref().map(|i| super::join(&v.root, i)), log_rel.as_ref().map(|l| super::join(&v.root, l))].into_iter().flatten().collect();
    let dirty_before = if v.git.commit { git::dirty(&v.root, &candidates) } else { vec![] };
    // Where: the note this call already has here (unless a new one is asked for), else a free name.
    let (rel, state) = match entry.as_ref().filter(|_| !new_note) {
        Some(e) if super::join(&v.root, &e.path).exists() => {
            let now = std::fs::read(super::join(&v.root, &e.path))?;
            if sha(&now) == sha(text.as_bytes()) {
                return Ok((Saved { vault: v.id.clone(), path: e.path.clone(), state: "unchanged".into(), index: "listed".into(), commit: e.commit.clone() }, vec![]));
            }
            if sha(&now) != e.sha256 && !replace {
                let _ = update(call, |r| {
                    if let Some(x) = r.saved.iter_mut().find(|x| x.vault == v.id) {
                        x.state = "kept".into();
                    }
                });
                return Ok((Saved { vault: v.id.clone(), path: e.path.clone(), state: "kept".into(), index: "listed".into(), commit: "off".into() }, vec![]));
            }
            (e.path.clone(), "updated")
        }
        _ => {
            let stem = manual.as_ref().and_then(|m| m.stem.clone());
            let name = file_name(&naming, &f.date, &f.title, stem.as_deref());
            (free_name(v, &format!("{notes_dir}/{name}"))?, "saved")
        }
    };
    let note_abs = super::join(&v.root, &rel);
    std::fs::create_dir_all(note_abs.parent().expect("a parent"))?;
    crate::paths::write_atomic(&note_abs, text.as_bytes())?;
    let mut written = vec![note_abs.clone()];
    // The index row, the index read just now.
    let index = match &index_rel {
        None => "skipped: the vault has no index".to_string(),
        Some(i) => {
            let ipath = super::join(&v.root, i);
            match std::fs::read_to_string(&ipath) {
                Err(e) => format!("skipped: {e}"),
                Ok(old) => {
                    let notes_from_index = relative_to(i, &notes_dir);
                    let note_from_index = relative_to(i, &rel);
                    match catalog::insert(&old, &notes_from_index, &note_from_index, &row_desc(&f), &f.date, &today()) {
                        Ok(Some(new)) => {
                            crate::paths::write_atomic(&ipath, new.as_bytes())?;
                            written.push(ipath);
                            "added".into()
                        }
                        Ok(None) => "listed".into(),
                        Err(why) => format!("skipped: {why}"),
                    }
                }
            }
        }
    };
    // The vault's log, when it keeps one.
    if let Some(l) = &log_rel {
        let lpath = super::join(&v.root, l);
        if let Ok(mut old) = std::fs::read_to_string(&lpath) {
            if !old.ends_with('\n') {
                old.push('\n');
            }
            let verb = if state == "updated" { "changed" } else { "added" };
            old.push_str(&format!("- {} · {verb} · {rel} · minutes of \"{}\" from Echo\n", today(), f.title));
            crate::paths::write_atomic(&lpath, old.as_bytes())?;
            written.push(lpath);
        }
    }
    let committable: Vec<PathBuf> = written.into_iter().filter(|p| !dirty_before.contains(p)).collect();
    let commit_label = if !v.git.commit {
        git::Commit::Off.label()
    } else if !commit {
        "pending".into()
    } else {
        git::commit_paths(&v.root, &committable, &format!("Echo: minutes of \"{}\" ({}) → {notes_dir}/", f.title, f.date)).label()
    };
    let hash = sha(text.as_bytes());
    let how_label = if manual.is_some() { "manual" } else { "auto" };
    let entry_out = Entry { vault: v.id.clone(), path: rel.clone(), sha256: hash, at: chrono::Local::now().to_rfc3339(), how: how_label.into(), state: state.into(), index: index.clone(), commit: commit_label.clone(), anyway: anyway || entry.as_ref().is_some_and(|e| e.anyway) };
    update(call, |r| {
        // A new note beside a kept one becomes the call's note here.
        r.saved.retain(|e| e.vault != v.id);
        r.saved.push(entry_out);
    })?;
    Ok((Saved { vault: v.id.clone(), path: rel, state: state.into(), index, commit: commit_label }, if v.git.commit { committable } else { vec![] }))
}

/// Where the vault keeps meeting notes: the folder found or chosen, else `notes/` in its pages.
fn notes_dir_of(v: &Vault) -> String {
    v.path("notes").or_else(|| v.path("pages").map(|p| if p.is_empty() { "notes".into() } else { format!("{p}/notes") })).unwrap_or_else(|| "notes".into())
}

/// A new note's path: `base`, else the next free `-2`, `-3`, …
fn free_name(v: &Vault, base: &str) -> Result<String> {
    let free = |p: &str| !super::join(&v.root, p).exists();
    if free(base) {
        return Ok(base.to_string());
    }
    let (s, ext) = base.rsplit_once('.').unwrap_or((base, "md"));
    (2..1000).map(|n| format!("{s}-{n}.{ext}")).find(|p| free(p)).ok_or_else(|| anyhow!("no free file name for {base}"))
}

/// The index row's description: `46m Title — the summary`, at most 300 characters.
fn row_desc(f: &Facts) -> String {
    let s = summary(&f.notes);
    let head = if f.minutes > 0 { format!("{}m {}", f.minutes, f.title) } else { f.title.clone() };
    let d = if s.is_empty() { head } else { format!("{head} — {s}") };
    if d.chars().count() > 300 { format!("{}…", d.chars().take(299).collect::<String>().trim_end()) } else { d }
}

/// Calls whose minutes exist, that this vault's rule takes, on its profile, not yet saved here.
fn past_calls(v: &Vault) -> Vec<PathBuf> {
    crate::calls::all_calls()
        .into_iter()
        .filter(|d| d.join("notes.md").is_file())
        .filter(|d| crate::ai::call_profile_id(d).ok().as_deref() == Some(v.profile.as_str()))
        .filter(|d| in_scope(v, d, crate::memo::is_memo(d)))
        .filter(|d| load(d).choice.as_deref() != Some("none") && !load(d).saved.iter().any(|e| e.vault == v.id))
        .collect()
}

/// How many past calls *Save past minutes* would save, and the first rows.
pub fn past(id: &str) -> Result<Value> {
    let v = super::get(id)?;
    let calls = past_calls(&v);
    let first: Vec<Value> = calls.iter().rev().take(5).filter_map(|d| facts(d).ok()).map(|f| json!({ "title": f.title, "date": f.date })).collect();
    Ok(json!({ "count": calls.len(), "first": first }))
}

/// Save every past call's minutes the vault's rule takes: one save each, one commit for all.
pub fn save_past(id: &str) -> Result<Value> {
    super::refuse_headless()?;
    let v = super::get(id)?;
    let _held = vault_lock(&v)?;
    let (mut saved, mut skipped, mut paths) = (0, 0, vec![]);
    for d in past_calls(&v) {
        match save_into(&d, &v, How::Auto, false) {
            Ok((s, p)) if s.state == "saved" => {
                saved += 1;
                paths.extend(p);
            }
            _ => skipped += 1,
        }
    }
    paths.sort();
    paths.dedup();
    let commit = if v.git.commit && !paths.is_empty() { git::commit_paths(&v.root, &paths, &format!("Echo: minutes of {saved} past calls")).label() } else { git::Commit::Off.label() };
    Ok(json!({ "saved": saved, "skipped": skipped, "commit": commit }))
}

/// What a client shows on a call: its record, the vaults that will take it, and those offering an update.
#[cfg(any(test, feature = "tray"))]
pub fn call_value(call: &Path) -> Value {
    let vaults = super::list().unwrap_or_default();
    json!({
        "record": record(call),
        "targets": targets(call).iter().map(|v| json!({ "id": v.id, "name": v.name })).collect::<Vec<_>>(),
        "stale": stale(call),
        "vaults": vaults.iter().map(|v| json!({ "id": v.id, "name": v.name, "profile": v.profile, "mode": v.minutes.mode, "available": super::available(v) })).collect::<Vec<_>>(),
        "failure": failure(call).map(|(id, name, error)| json!({ "vault": id, "name": name, "error": error })),
        "profile": crate::ai::call_profile_id(call).ok(),
    })
}

/// The note the minutes would make in vault `id`, and its index row — nothing written (the save sheet's preview).
pub fn preview(call: &Path, id: &str, stem: Option<&str>) -> Result<Value> {
    let v = super::get(id)?;
    let f = facts(call)?;
    let notes_dir = notes_dir_of(&v);
    let naming = v.path("naming").unwrap_or_else(|| super::detect::DATE_SLUG.into());
    // As the save decides: the call's note here when it has one, else a free name.
    let mine = load(call).saved.into_iter().find(|e| e.vault == v.id).map(|e| e.path).filter(|p| super::join(&v.root, p).exists());
    let rel = match mine {
        Some(p) => p,
        None => free_name(&v, &format!("{notes_dir}/{}", file_name(&naming, &f.date, &f.title, stem)))?,
    };
    let index = v.path("index");
    // The row as the save would write it into the index as it is now.
    let row = index.as_ref().and_then(|i| {
        let old = std::fs::read_to_string(super::join(&v.root, i)).ok()?;
        let new = catalog::insert(&old, &relative_to(i, &notes_dir), &relative_to(i, &rel), &row_desc(&f), &f.date, &today()).ok()??;
        let before: std::collections::HashSet<&str> = old.lines().collect();
        new.lines().find(|l| !before.contains(l) && l.trim_start().starts_with('|')).map(str::to_owned)
    });
    Ok(json!({
        "vault": v.id, "path": rel, "stem": rel.rsplit('/').next().unwrap_or("").trim_end_matches(".md"),
        "index": index, "row": row, "commit": v.git.commit, "branch": git::info(&v.root).and_then(|g| g.branch),
        "profile_ok": f.profile.as_deref() == Some(v.profile.as_str()), "call_profile": f.profile, "vault_profile": v.profile,
        "transcript": v.minutes.transcript, "chat": v.minutes.chat, "exists": super::join(&v.root, &rel).exists(),
        "note": note_text(&v, &f, None, None), "title": f.title,
    }))
}

/// The preview of the newest call with minutes on the vault's own profile — how a call lands here (Settings →
/// Meeting minutes → Preview); `null` before there is one. Nothing is written.
pub fn sample(id: &str) -> Result<Value> {
    let v = super::get(id)?;
    let newest = crate::calls::all_calls().into_iter().filter(|d| d.join("notes.md").is_file() && crate::ai::call_profile_id(d).ok().as_deref() == Some(v.profile.as_str())).max_by_key(|d| d.file_name().map(|n| n.to_os_string()));
    match newest {
        Some(d) => preview(&d, id, None),
        None => Ok(Value::Null),
    }
}

//! Post-call helpers that give Claude deterministic facts to work from: `context` (the input
//! bundle for notes), `title`, `note` (the user's own notes during a call) and `previous`
//! (the last occurrence of a recurring meeting).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, FixedOffset, Local};
use serde_json::{json, Value};

use crate::config::Config;
use crate::meeting::MeetingContext;
use crate::paths;

/// Below these, a call is too thin to summarize.
const MIN_SUMMARY_MINUTES: f64 = 3.0;
const MIN_SUMMARY_CHARS: usize = 1_500;
pub const USER_NOTES: &str = "my-notes.md";
/// In a trashed call: when it was deleted (its mtime ages the trash entry).
const DELETED_AT: &str = ".deleted_at";

/// All recorded calls, oldest first.
pub fn all_calls() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(paths::calls_dir())
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.join("meta.json").exists()).collect())
        .unwrap_or_default();
    dirs.sort();
    dirs
}

/// Resolve `latest`, a path, or a folder name under calls/ to a call directory.
pub fn resolve_call(arg: &str) -> Result<PathBuf> {
    if arg == "latest" {
        return all_calls().pop().ok_or_else(|| anyhow!("no recorded calls in {}", paths::calls_dir().display()));
    }
    for p in [PathBuf::from(arg), paths::calls_dir().join(arg)] {
        if p.join("meta.json").exists() {
            return Ok(p);
        }
    }
    bail!("{arg} is not a call folder (expected meta.json inside)")
}

fn read_json(p: &Path) -> Value {
    fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

fn subject_of(dir: &Path) -> Option<String> {
    MeetingContext::load(dir)
        .ok()
        .flatten()
        .and_then(|m| m.subject)
        .or_else(|| read_json(&dir.join("meta.json"))["title"].as_str().map(str::to_owned))
}

/// Heading budget for notes by transcript length (chars): short calls get fewer sections.
pub fn heading_budget(chars: usize) -> usize {
    match chars {
        c if c < 8_000 => 4,
        c if c < 20_000 => 5,
        c if c < 80_000 => c / 10_000 + 1,
        _ => 8,
    }
}

fn domain(email: &str) -> Option<String> {
    email.rsplit_once('@').map(|(_, d)| d.to_lowercase())
}

/// The latest earlier call with the same subject that has notes (recurring meetings).
/// "Last time" for a recurring meeting: up to 3 short bullets from the previous occurrence's
/// notes — its decisions first, else its summary, else its first bullets. Deterministic.
pub fn recap(subject: &str) -> Vec<String> {
    let Some(dir) = previous_with_notes(subject, None) else { return vec![] };
    recap_from_notes(&fs::read_to_string(dir.join("notes.md")).unwrap_or_default())
}

pub fn recap_from_notes(md: &str) -> Vec<String> {
    let clean = |l: &str| -> String {
        let t = l.trim().trim_start_matches(['-', '*', '•']).trim().replace("**", "");
        // drop [mm:ss] anchors
        let t = t.split(" [").next().unwrap_or(&t).trim().to_string();
        if t.chars().count() > 88 { format!("{}…", t.chars().take(87).collect::<String>()) } else { t }
    };
    let section = |want: &[&str]| -> Vec<String> {
        let mut inside = false;
        let mut out = vec![];
        for l in md.lines() {
            if let Some(h) = l.strip_prefix("## ") {
                inside = want.iter().any(|w| h.to_lowercase().contains(w));
                continue;
            }
            if inside && (l.trim_start().starts_with("- ") || l.trim_start().starts_with("* ")) {
                out.push(clean(l));
            } else if inside && !l.trim().is_empty() && out.is_empty() && !l.starts_with('#') {
                // a summary paragraph: its first sentence
                out.push(clean(l.split(". ").next().unwrap_or(l)));
            }
        }
        out
    };
    let mut got = section(&["decision"]);
    if got.is_empty() {
        got = section(&["summary"]);
    }
    if got.is_empty() {
        got = md.lines().filter(|l| l.trim_start().starts_with("- ")).map(clean).collect();
    }
    got.into_iter().filter(|b| !b.is_empty()).take(3).collect()
}

pub fn previous_with_notes(subject: &str, before: Option<&Path>) -> Option<PathBuf> {
    let want = subject.trim().to_lowercase();
    all_calls()
        .into_iter()
        .rev()
        .filter(|d| before.is_none_or(|b| d.as_path() < b))
        .find(|d| d.join("notes.md").exists() && subject_of(d).is_some_and(|s| s.trim().to_lowercase() == want))
}

/// Where note templates live: the user's (`.echo/templates`, first) then the plugin's.
/// Built-in note templates (compiled in, so the CLI needs no plugin files).
const BUILTIN_TEMPLATES: &[(&str, &str)] = &[
    ("default", include_str!("../templates/default.md")),
    ("discussion", include_str!("../templates/discussion.md")),
    ("status-sync", include_str!("../templates/status-sync.md")),
];

#[derive(Debug, Clone, serde::Serialize)]
pub struct Template {
    pub name: String,
    pub path: String,
    pub sections: Vec<Value>,
    #[serde(skip)]
    keywords: Vec<String>,
}

fn parse_template(text: &str, stem: &str, path: String) -> Option<Template> {
    let (front, body) = text.strip_prefix("---\n")?.split_once("\n---\n")?;
    let mut name = stem.to_string();
    let mut keywords = vec![];
    for line in front.lines() {
        if let Some(v) = line.strip_prefix("name:") {
            name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("match:") {
            keywords = v
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|k| k.trim().trim_matches('"').to_lowercase())
                .filter(|k| !k.is_empty())
                .collect();
        }
    }
    let mut sections = vec![];
    for chunk in body.split("\n## ").map(|c| c.trim_start_matches("## ")) {
        let (heading, instruction) = chunk.split_once('\n').unwrap_or((chunk, ""));
        if !heading.trim().is_empty() {
            sections.push(json!({ "heading": heading.trim(), "instruction": instruction.trim() }));
        }
    }
    Some(Template { name, path, sections, keywords })
}

/// The template for a meeting title: the first whose keyword appears in it (user templates
/// first), else `default`.
pub fn pick_template(title: Option<&str>) -> Option<Template> {
    let mut all = vec![];
    let mut seen = std::collections::HashSet::new();
    // User templates (<home>/.echo/templates/*.md) override built-ins of the same name.
    let mut files: Vec<PathBuf> = fs::read_dir(paths::state_dir().join("templates"))
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "md")).collect())
        .unwrap_or_default();
    files.sort();
    let user = files.iter().filter_map(|f| {
        let text = fs::read_to_string(f).ok()?;
        parse_template(&text, &f.file_stem()?.to_string_lossy(), f.display().to_string())
    });
    let builtin = BUILTIN_TEMPLATES.iter().filter_map(|(stem, text)| parse_template(text, stem, format!("builtin:{stem}")));
    for t in user.chain(builtin) {
        if seen.insert(t.name.clone()) {
            all.push(t);
        }
    }
    let t = title.unwrap_or("").to_lowercase();
    all.iter()
        .find(|tpl| tpl.keywords.iter().any(|k| t.contains(k.as_str())))
        .or_else(|| all.iter().find(|tpl| tpl.name == "default"))
        .cloned()
}

/// `echoctl context <call>`: the input bundle for writing notes, as JSON.
pub fn context(call: &str) -> Result<()> {
    let dir = resolve_call(call)?;
    let meta = read_json(&dir.join("meta.json"));
    let meeting = MeetingContext::load(&dir)?;
    let cfg = Config::load()?;
    let segs = read_json(&dir.join("segments.json"));
    let segs = segs.as_array().cloned().unwrap_or_default();

    let ts = |k: &str| meta[k].as_str().and_then(|v| DateTime::parse_from_rfc3339(v).ok());
    let duration_min = match (ts("started_at"), ts("ended_at")) {
        (Some(a), Some(b)) => (b - a).num_seconds() as f64 / 60.0,
        _ => 0.0,
    };
    let mut chars = 0usize;
    let mut speech_s = 0f64;
    let mut by_lang: serde_json::Map<String, Value> = Default::default();
    let mut by_speaker: serde_json::Map<String, Value> = Default::default();
    for s in &segs {
        let n = s["text"].as_str().map(|t| t.chars().count()).unwrap_or(0);
        chars += n;
        speech_s += (s["end"].as_f64().unwrap_or(0.0) - s["start"].as_f64().unwrap_or(0.0)).max(0.0);
        for (map, key) in [(&mut by_lang, "lang"), (&mut by_speaker, "speaker")] {
            let k = s[key].as_str().unwrap_or("?").to_string();
            let v = map.entry(k).or_insert(json!(0));
            *v = json!(v.as_u64().unwrap_or(0) + n as u64);
        }
    }
    let dominant_language = by_lang.iter().max_by_key(|(_, v)| v.as_u64().unwrap_or(0)).map(|(k, _)| k.clone());

    // Colleagues vs external attendees, by the user's email domain.
    let my_domain = cfg.me.as_ref().and_then(|p| p.email.as_deref()).and_then(domain);
    let (mut colleagues, mut external) = (vec![], vec![]);
    if let Some(m) = &meeting {
        for p in m.organizer.iter().chain(m.attendees.iter()) {
            let is_me = cfg.me.as_ref().is_some_and(|me| me.email.is_some() && me.email == p.email);
            if is_me {
                continue;
            }
            let label = match &p.email {
                Some(e) => format!("{} <{e}>", p.name),
                None => p.name.clone(),
            };
            match (p.email.as_deref().and_then(domain), &my_domain) {
                (Some(d), Some(mine)) if &d != mine => external.push(label),
                _ => colleagues.push(label),
            }
        }
        colleagues.dedup();
        external.dedup();
    }

    let user_notes = dir.join(USER_NOTES);
    let user_headings: Vec<String> = fs::read_to_string(&user_notes)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix('#').map(|h| h.trim_start_matches('#').trim().to_string()))
        .filter(|h| !h.is_empty())
        .collect();

    // Shared screens make a call worth notes however short the talk: the slides are the content.
    let frames = frames_of(&dir);
    let summarize = if segs.is_empty() {
        Some("no transcript yet — run `echoctl transcribe` first")
    } else if !frames.is_empty() {
        None
    } else if duration_min < MIN_SUMMARY_MINUTES {
        Some("shorter than 3 minutes")
    } else if chars < MIN_SUMMARY_CHARS {
        Some("less than 1,500 characters of transcript")
    } else {
        None
    };
    let subject = subject_of(&dir);
    let previous = subject.as_deref().and_then(|s| previous_with_notes(s, Some(&dir)));

    let out = json!({
        "dir": dir.display().to_string(),
        "title": subject,
        "started_at": meta["started_at"],
        "duration_min": (duration_min * 10.0).round() / 10.0,
        "speech_min": (speech_s / 6.0).round() / 10.0,
        "me": cfg.me,
        "meeting": meeting,
        "colleagues": colleagues,
        "external": external,
        "transcript_chars": chars,
        "chars_by_speaker": by_speaker,
        "chars_by_language": by_lang,
        "dominant_language": dominant_language,
        "user_notes": user_notes.exists().then(|| user_notes.display().to_string()),
        "user_headings": user_headings,
        "heading_budget": heading_budget(chars),
        "summarize": summarize.is_none(),
        "skip_reason": summarize,
        "previous_notes": previous.map(|p| p.join("notes.md").display().to_string()),
        "template": pick_template(subject_of(&dir).as_deref()),
        "frames": frames,
        "chat": fs::read(dir.join("chat.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(json!([])),
        "me_profile": fs::read_to_string(me_profile_path()).ok().filter(|t| !t.trim().is_empty() && t.trim() != ME_TEMPLATE.trim()),
        "vocabulary": cfg.vocabulary,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

/// A Claude session took this call for notes (`claim`); no other session will.
pub const CLAIMED: &str = ".claimed";
/// Calls older than this are not offered for automatic notes (catch-up after a long gap would
/// flood a session); the pane's Generate button still works for them.
const PENDING_MAX_AGE_H: i64 = 48;

/// Calls transcribed but without notes, worth summarizing, unclaimed, from the last 48 h —
/// what a Claude session picks up when it opens (the app writes no notes by itself).
pub fn pending_calls() -> Vec<PathBuf> {
    let now = chrono::Utc::now();
    all_calls()
        .into_iter()
        .filter(|d| {
            let meta = read_json(&d.join("meta.json"));
            let t = |k: &str| meta[k].as_str().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok());
            let (Some(a), Some(b)) = (t("started_at"), t("ended_at")) else { return false };
            let long_enough = (b - a).num_seconds() as f64 / 60.0 >= MIN_SUMMARY_MINUTES;
            let recent = (now - b.with_timezone(&chrono::Utc)).num_hours() < PENDING_MAX_AGE_H;
            let big_enough = fs::metadata(d.join("transcript.md")).is_ok_and(|m| m.len() as usize >= MIN_SUMMARY_CHARS);
            long_enough && recent && big_enough
                && !d.join("notes.md").exists()
                && !d.join(CLAIMED).exists()
                && !d.join(crate::transcribe_marker()).exists()
        })
        .collect()
}

/// Refuse automatic notes for a session outside `config.notes_in`.
fn session_allowed(session: Option<&Path>) -> Result<bool> {
    let cfg = crate::config::Config::load()?;
    Ok(session.is_none_or(|cwd| cfg.notes_allowed(cwd)))
}

/// `echoctl pending [--for <session cwd>]`: those calls as JSON (`[{dir, name}]`); empty for
/// a session that isn't the one chosen for notes.
pub fn pending(session: Option<&Path>) -> Result<()> {
    // Inside a run Echo started for one call, the mod must not pick up others.
    let calls = if session_allowed(session)? && !crate::notes::headless() { pending_calls() } else { vec![] };
    let out: Vec<Value> = calls
        .iter()
        .map(|d| json!({ "dir": d.display().to_string(), "name": d.file_name().map(|n| n.to_string_lossy().into_owned()) }))
        .collect();
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

/// `echoctl claim <call>`: atomically take a call for notes. Fails if another session has it,
/// so several open Claude sessions never process the same call twice.
pub fn claim(call: &str, session: Option<&Path>) -> Result<()> {
    if crate::notes::headless() {
        bail!("this is a run Echo started for one call; it was claimed already");
    }
    if !session_allowed(session)? {
        bail!("notes are written only in sessions opened in the folder set by `echoctl config --notes-in`");
    }
    let dir = resolve_call(call)?;
    let marker = dir.join(CLAIMED);
    match fs::OpenOptions::new().write(true).create_new(true).open(&marker) {
        Ok(mut f) => {
            use std::io::Write;
            let _ = writeln!(f, "{}", chrono::Local::now().to_rfc3339());
            println!("{}", dir.display());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => bail!("already claimed by another session"),
        Err(e) => Err(e.into()),
    }
}

/// The user's own profile, read by every notes / follow-up step (`<home>/.echo/me.md`).
pub fn me_profile_path() -> PathBuf {
    paths::state_dir().join("me.md")
}

pub const ME_TEMPLATE: &str = "# About me\n\n\
Name:\nRole:\nCompany / team:\nWhat I work on now:\nPeople I work with often (name — role):\n\
Terms and names used in my calls:\nHow I like notes and follow-ups written (tone, length, language):\n";

/// Create `me.md` from the template if missing; returns its path.
pub fn ensure_me_profile() -> Result<PathBuf> {
    let p = me_profile_path();
    if !p.exists() {
        fs::create_dir_all(paths::state_dir())?;
        fs::write(&p, ME_TEMPLATE)?;
    }
    Ok(p)
}

/// `echoctl compose`: open a pre-filled email in the default mail app (never sends).
pub fn compose(to: &[String], cc: &[String], subject: &str, body_file: &Path) -> Result<()> {
    let body = fs::read_to_string(body_file).with_context(|| format!("reading {}", body_file.display()))?;
    let enc = |s: &str| -> String {
        s.bytes()
            .map(|b| match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'@' => (b as char).to_string(),
                _ => format!("%{b:02X}"),
            })
            .collect()
    };
    let mut url = format!("mailto:{}?subject={}&body={}", to.iter().map(|t| enc(t)).collect::<Vec<_>>().join(","), enc(subject), enc(&body));
    if !cc.is_empty() {
        url.push_str(&format!("&cc={}", cc.iter().map(|t| enc(t)).collect::<Vec<_>>().join(",")));
    }
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&url).status()?;
    #[cfg(windows)]
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).status()?;
    println!("opened a draft to {} in the mail app — review and send it there", to.join(", "));
    Ok(())
}

/// `echoctl frames <call> --keep a.jpg,b.jpg | none`: keep only the frames the notes use,
/// delete the rest (talking heads, Teams UI, near-duplicates) and rewrite `frames.jsonl`.
pub fn prune_frames(call: &str, keep: &[String]) -> Result<()> {
    let dir = resolve_call(call)?;
    let name = |p: &str| Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let keep: std::collections::HashSet<String> = keep.iter().filter(|k| k.as_str() != "none").map(|k| name(k)).collect();
    // Never deleted outright: what is pruned goes to `.echo/trash/<call>.frames/` (with its
    // index lines) of the data folder the call is in, purged after 30 days like deleted calls
    // — a wrong prune can be undone.
    let home_trash = dir.parent().and_then(Path::parent).map(|home| home.join(".echo").join("trash"));
    let bin = home_trash.unwrap_or_else(trash_dir).join(format!("{}.frames", dir.file_name().unwrap().to_string_lossy()));
    let (mut kept, mut removed) = (0, 0);
    if let Ok(rd) = fs::read_dir(dir.join("frames")) {
        for f in rd.filter_map(|e| e.ok().map(|e| e.path())) {
            if keep.contains(&name(&f.to_string_lossy())) {
                kept += 1;
            } else if fs::create_dir_all(&bin).is_ok() && fs::rename(&f, bin.join(name(&f.to_string_lossy()))).is_ok() {
                removed += 1;
            }
        }
    }
    let lines: Vec<String> = fs::read_to_string(dir.join("frames.jsonl")).unwrap_or_default().lines().map(str::to_owned).collect();
    let kept_line = |l: &String| serde_json::from_str::<Value>(l).ok().and_then(|v| v["file"].as_str().map(name)).is_some_and(|f| keep.contains(&f));
    let (index, pruned): (Vec<String>, Vec<String>) = lines.into_iter().partition(kept_line);
    if removed > 0 {
        let mut f = fs::OpenOptions::new().create(true).append(true).open(bin.join("frames.jsonl"))?;
        for l in &pruned {
            use std::io::Write;
            writeln!(f, "{l}")?;
        }
        fs::write(bin.join(DELETED_AT), chrono::Local::now().to_rfc3339())?;
    }
    if index.is_empty() {
        let _ = fs::remove_file(dir.join("frames.jsonl"));
        let _ = fs::remove_dir(dir.join("frames"));
    } else {
        fs::write(dir.join("frames.jsonl"), index.join("\n") + "\n")?;
    }
    if let Some(trash) = bin.parent() {
        purge_trash_in(trash);
    }
    if removed > 0 {
        println!("frames: kept {kept}, moved {removed} to {} (30 days)", bin.display());
    } else {
        println!("frames: kept {kept}");
    }
    Ok(())
}

/// Key frames saved during the call (`frames.jsonl`): `[{t, file, change}]`, file absolute.
fn frames_of(dir: &Path) -> Vec<Value> {
    fs::read_to_string(dir.join("frames.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .map(|mut f| {
            if let Some(file) = f["file"].as_str() {
                f["file"] = json!(dir.join(file).display().to_string());
            }
            f
        })
        .collect()
}

/// Clamp a generated title: one line, ≤ 100 chars (cut at a word boundary after 50).
pub fn clamp_title(raw: &str) -> Result<String> {
    let t = raw.trim();
    if t.contains('\n') {
        bail!("a title must be a single line");
    }
    if t.chars().count() <= 100 {
        return Ok(t.to_string());
    }
    let head: String = t.chars().take(100).collect();
    let cut = head.char_indices().filter(|(i, c)| *i > 50 && *c == ' ').map(|(i, _)| i).next_back().unwrap_or(head.len());
    Ok(head[..cut].trim_end_matches(|c: char| c.is_ascii_punctuation() || c == ' ').to_string())
}

/// `echoctl title <call> <title>`: set a call's title and rename its folder to match.
pub fn set_title(call: &str, title: &str) -> Result<()> {
    let dir = resolve_call(call)?;
    let title = clamp_title(title)?;
    // While recording, meta.json is rewritten from memory at the stop: a title set now would be lost.
    if read_json(&dir.join("meta.json"))["ended_at"].is_null() {
        bail!("{} is still being recorded — set the title after it stops", dir.display());
    }
    let mut meta = read_json(&dir.join("meta.json"));
    meta["title"] = json!(title);
    fs::write(dir.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
    // The title is what counts; the folder name only follows it. Windows refuses to rename a
    // folder with a file open inside (a notes run's own claude.log): the run renames it when
    // it ends.
    match rename_to_title(&dir) {
        Ok(d) => println!("{}", d.display()),
        Err(e) => {
            eprintln!("title set; the folder is renamed to match when files in it are no longer open ({e:#})");
            println!("{}", dir.display());
        }
    }
    Ok(())
}

/// Rename a call's folder to `<stamp>-<slug of its title>` (not while it is recording).
/// Returns the folder's path afterwards.
pub fn rename_to_title(dir: &Path) -> Result<PathBuf> {
    let Some(title) = read_json(&dir.join("meta.json"))["title"].as_str().map(str::to_owned) else {
        return Ok(dir.to_path_buf());
    };
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let stamp = name.get(..17).unwrap_or(&name); // "YYYY-MM-DD_HHMMSS"
    let slug = crate::recorder::slugify(&title);
    let new_dir = dir.with_file_name(if slug.is_empty() { stamp.to_string() } else { format!("{stamp}-{slug}") });
    let recording = crate::watch::running_watcher().and_then(|w| w.recording).is_some_and(|r| Path::new(&r) == dir);
    if new_dir == dir || recording {
        return Ok(dir.to_path_buf());
    }
    fs::rename(dir, &new_dir).with_context(|| format!("renaming {}", dir.display()))?;
    Ok(new_dir)
}

/// `echoctl note <text>`: append the user's own note to the call being recorded, stamped
/// with the time into the call. Lines starting with `#` are headings that shape the notes.
pub fn note(text: &str) -> Result<()> {
    let w = crate::watch::running_watcher().ok_or_else(|| anyhow!("no watcher running"))?;
    let dir = PathBuf::from(w.recording.ok_or_else(|| anyhow!("nothing is being recorded right now"))?);
    let meta = read_json(&dir.join("meta.json"));
    let started: Option<DateTime<FixedOffset>> =
        meta["started_at"].as_str().and_then(|v| DateTime::parse_from_rfc3339(v).ok());
    let at = started
        .map(|s| {
            let secs = (Local::now().fixed_offset() - s).num_seconds().max(0);
            format!("{:02}:{:02}", secs / 60, secs % 60)
        })
        .unwrap_or_default();
    let line = if text.trim_start().starts_with('#') { format!("\n{}\n", text.trim()) } else { format!("- [{at}] {}\n", text.trim()) };
    let mut f = OpenOptions::new().create(true).append(true).open(dir.join(USER_NOTES))?;
    f.write_all(line.as_bytes())?;
    println!("noted in {}", dir.join(USER_NOTES).display());
    Ok(())
}

/// `echoctl previous --subject S`: notes of the last call with that subject, if any.
pub fn previous(subject: &str) -> Result<()> {
    if let Some(d) = previous_with_notes(subject, None) {
        println!("{}", d.join("notes.md").display());
    }
    Ok(())
}

/// Deleted calls wait here this long before being purged.
const TRASH_DAYS: u64 = 30;

fn trash_dir() -> PathBuf {
    paths::state_dir().join("trash")
}

/// `echoctl delete <call>`: move a call to `.echo/trash/` (recoverable for 30 days).
pub fn delete(call: &str) -> Result<()> {
    let dir = resolve_call(call)?;
    let recording = crate::watch::running_watcher()
        .and_then(|w| w.recording)
        .is_some_and(|r| Path::new(&r) == dir || Path::new(&r).ends_with(dir.file_name().unwrap_or_default()));
    if recording {
        bail!("{} is being recorded — stop the recording first", dir.display());
    }
    fs::create_dir_all(trash_dir())?;
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let mut dest = trash_dir().join(&name);
    let mut n = 1;
    while dest.exists() {
        dest = trash_dir().join(format!("{name}.{n}"));
        n += 1;
    }
    fs::rename(&dir, &dest).with_context(|| format!("moving {} to the trash", dir.display()))?;
    // The 30 days count from now, not from when the call's files last changed.
    fs::write(dest.join(DELETED_AT), chrono::Local::now().to_rfc3339())?;
    purge_trash();
    println!("{}", json!({ "deleted": name, "trash": dest.display().to_string(), "restore": format!("echoctl restore {}", dest.file_name().unwrap().to_string_lossy()) }));
    Ok(())
}

/// `echoctl restore <name>`: bring a deleted call back from the trash.
pub fn restore(name: &str) -> Result<()> {
    let src = trash_dir().join(name);
    if !src.join("meta.json").exists() {
        bail!("{name} is not in {}", trash_dir().display());
    }
    // A numbered duplicate ("<call>.1") goes back under its original name if that is free.
    let base = name.rsplit_once('.').filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit())).map(|(b, _)| b).unwrap_or(name);
    let mut dest = paths::calls_dir().join(base);
    if dest.exists() {
        dest = paths::calls_dir().join(name);
    }
    fs::create_dir_all(paths::calls_dir())?;
    fs::rename(&src, &dest)?;
    let _ = fs::remove_file(dest.join(DELETED_AT));
    println!("{}", dest.display());
    Ok(())
}

fn purge_trash() {
    purge_trash_in(&trash_dir());
}

/// Entries of a trash folder deleted more than 30 days ago (calls, pruned frames).
fn purge_trash_in(trash: &Path) {
    let Ok(rd) = fs::read_dir(trash) else { return };
    let max_age = std::time::Duration::from_secs(TRASH_DAYS * 24 * 3600);
    for e in rd.flatten() {
        let stamp = e.path().join(DELETED_AT);
        let when = if stamp.exists() { fs::metadata(&stamp) } else { e.metadata() };
        let old = when.and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > max_age);
        if old {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_budget_scales_with_length() {
        assert_eq!(heading_budget(3_000), 4);
        assert_eq!(heading_budget(15_000), 5);
        assert_eq!(heading_budget(45_000), 5);
        assert_eq!(heading_budget(79_999), 8);
        assert_eq!(heading_budget(200_000), 8);
    }

    #[test]
    fn templates_are_picked_by_title_keyword() {
        let d = parse_template("---\nname: default\nmatch: []\n---\n## Summary\nshort\n\n## Decisions\nlist\n", "default", "t".into()).unwrap();
        assert_eq!(d.sections.len(), 2);
        assert_eq!(d.sections[1]["heading"], "Decisions");
        let s = parse_template("---\nname: status-sync\nmatch: [daily, standup]\n---\n## Progress\nper person\n", "sync", "t".into()).unwrap();
        assert_eq!(s.keywords, vec!["daily", "standup"]);
    }

    #[test]
    fn recap_prefers_decisions() {
        let md = "# T\n\n## Summary\nWe met. More.\n\n## Decisions\n- Keep **X** out [02:31]\n- Meet weekly\n";
        assert_eq!(recap_from_notes(md), vec!["Keep X out", "Meet weekly"]);
        let md = "# T\n\n## Summary\nWe agreed on the plan. Then more.\n";
        assert_eq!(recap_from_notes(md), vec!["We agreed on the plan"]);
    }

    #[test]
    fn frames_are_pruned_to_the_ones_kept() {
        let home = std::env::temp_dir().join(format!("echo-frames-{}", std::process::id()));
        let dir = home.join("calls").join("call");
        let bin = home.join(".echo").join("trash").join("call.frames");
        fs::create_dir_all(dir.join("frames")).unwrap();
        fs::write(dir.join("meta.json"), "{}").unwrap();
        for f in ["00m10s.jpg", "02m00s.jpg", "05m30s.jpg"] {
            fs::write(dir.join("frames").join(f), b"x").unwrap();
        }
        let index = ["00m10s.jpg", "02m00s.jpg", "05m30s.jpg"].map(|f| format!("{{\"t\":1,\"file\":\"frames/{f}\"}}")).join("\n");
        fs::write(dir.join("frames.jsonl"), index).unwrap();
        prune_frames(dir.to_str().unwrap(), &["02m00s.jpg".into(), dir.join("frames/05m30s.jpg").display().to_string()]).unwrap();
        assert!(!dir.join("frames/00m10s.jpg").exists());
        assert!(dir.join("frames/02m00s.jpg").exists() && dir.join("frames/05m30s.jpg").exists());
        assert_eq!(fs::read_to_string(dir.join("frames.jsonl")).unwrap().lines().count(), 2);
        // Pruned, not deleted: in the call's own data folder's trash, with its index line.
        assert!(bin.join("00m10s.jpg").exists());
        assert_eq!(fs::read_to_string(bin.join("frames.jsonl")).unwrap().lines().count(), 1);
        prune_frames(dir.to_str().unwrap(), &["none".into()]).unwrap();
        assert!(!dir.join("frames").exists() && !dir.join("frames.jsonl").exists());
        assert!(bin.join("02m00s.jpg").exists() && bin.join("05m30s.jpg").exists());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn builtin_templates_parse() {
        for (stem, text) in BUILTIN_TEMPLATES {
            let t = parse_template(text, stem, String::new()).unwrap_or_else(|| panic!("{stem} does not parse"));
            assert!(!t.sections.is_empty(), "{stem} has no sections");
        }
    }

    #[test]
    fn titles_are_clamped_to_one_short_line() {
        assert!(clamp_title("a\nb").is_err());
        assert_eq!(clamp_title("  Weekly sync  ").unwrap(), "Weekly sync");
        let long = "Discussion of the migration plan for the internal operations platform and the rollout schedule across regions";
        let t = clamp_title(long).unwrap();
        assert!(t.len() <= 100 && t.len() > 50 && !t.ends_with(' '), "{t}");
    }
}

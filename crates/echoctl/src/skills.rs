//! Settings → Templates → Skills: Agent Skills (agentskills.io) for the work a task asks for — each a folder:
//! `SKILL.md` (frontmatter `name`, `description`…, then what to do) and, as it needs, `scripts/`, `references/`,
//! `assets/`. Echo keeps the catalogue — what there is, whether it is valid, on or off — and shows it; their content is
//! written elsewhere (VS Code, Claude: Ask Claude, Open in VS Code). Echo's own are built in, read-only until
//! Customized (copied to the user's); the user's are `<home>/.echo/skills/<id>/`. Each time the Claude panel's session
//! starts, Echo copies the valid skills that are on into a plugin, `my-skills` (`<config>/my-skills/`): only their
//! descriptions sit in its context until one is used. Send to Claude names one (`/my-skills:<id>`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{json, Value};

use crate::paths;

/// The plugin's name: a skill is `/my-skills:<id>` in the session.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub const PLUGIN: &str = "my-skills";

/// Echo's own skills: their files, as they ship.
const BUILTIN: &[(&str, &[(&str, &str)])] = &[("follow-up-mail", &[("SKILL.md", include_str!("../skills/follow-up-mail/SKILL.md"))])];

fn dir() -> PathBuf {
    paths::state_dir().join("skills")
}

fn off_file() -> PathBuf {
    paths::state_dir().join("skills-off.json")
}

fn off() -> Vec<String> {
    fs::read(off_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// A `SKILL.md`'s frontmatter (top-level `key: value` lines, values unquoted; a block value — `>` or `|` and the
/// indented lines under it — folded to one line or kept as lines) and body.
fn split(text: &str) -> Option<(Vec<(String, String)>, String)> {
    // As Claude Code reads it: a byte-order mark and Windows line ends are fine.
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let (front, body) = text.strip_prefix("---\n").and_then(|t| t.split_once("\n---"))?;
    let mut fields: Vec<(String, String)> = vec![];
    let mut block: Option<(usize, &str)> = None; // the field a block value goes to, and how it joins
    for l in front.lines() {
        let indented = l.starts_with(' ') || l.starts_with('\t');
        if let (true, Some((i, join))) = (indented || l.trim().is_empty(), block) {
            let v = &mut fields[i].1;
            if !v.is_empty() && !l.trim().is_empty() {
                v.push_str(join);
            }
            v.push_str(l.trim());
            continue;
        }
        block = None;
        if indented {
            continue;
        }
        if let Some((k, v)) = l.split_once(':') {
            let v = v.trim();
            // Nothing after the colon: a plain value on the indented lines under it, folded as `>` is.
            if matches!(v, "" | ">" | ">-" | ">+" | "|" | "|-" | "|+") {
                block = Some((fields.len(), if v.starts_with('|') { "\n" } else { " " }));
                fields.push((k.trim().to_string(), String::new()));
            } else {
                fields.push((k.trim().to_string(), v.trim_matches('"').trim_matches('\'').to_string()));
            }
        }
    }
    Some((fields, body.trim_start_matches('\n').trim().to_string()))
}

/// A skill's name as the spec takes it: lower-case letters, digits and single hyphens, at most 64.
pub fn valid_id(n: &str) -> bool {
    !n.is_empty() && n.len() <= 64 && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !n.starts_with('-') && !n.ends_with('-') && !n.contains("--")
}

/// What the spec asks of a skill named `id` with this `SKILL.md`: the problems (none: it can be used).
fn problems(id: &str, text: Option<&str>) -> Vec<String> {
    let Some(text) = text else { return vec!["no SKILL.md".into()] };
    let Some((fields, body)) = split(text) else { return vec!["SKILL.md has no frontmatter (--- … ---) at its top".into()] };
    let get = |k: &str| fields.iter().find(|(f, _)| f == k).map(|(_, v)| v.as_str());
    let mut out = vec![];
    match get("name") {
        None | Some("") => out.push("no name in its frontmatter".into()),
        Some(n) => {
            if !valid_id(n) {
                out.push(format!("name \"{n}\": lower-case letters, digits and single hyphens, at most 64"));
            } else if n != id {
                out.push(format!("name \"{n}\" isn't its folder's name, {id}"));
            }
        }
    }
    match get("description") {
        None | Some("") => out.push("no description — Claude picks a skill by it".into()),
        Some(d) if d.chars().count() > 1024 => out.push("description longer than 1,024 characters".into()),
        _ => {}
    }
    if get("compatibility").is_some_and(|c| c.chars().count() > 500) {
        out.push("compatibility longer than 500 characters".into());
    }
    if body.lines().count() > 500 {
        out.push("SKILL.md over 500 lines — move detail into references/".into());
    }
    out
}

/// What a skill folder holds besides `SKILL.md`: scripts, references, assets, other files.
fn contents(d: &Path) -> Value {
    let count = |sub: &str| walk(&d.join(sub)).len();
    let other = fs::read_dir(d).into_iter().flatten().flatten().filter(|e| !["SKILL.md", "scripts", "references", "assets"].contains(&e.file_name().to_string_lossy().as_ref())).count();
    json!({"scripts": count("scripts"), "references": count("references"), "assets": count("assets"), "other": other})
}

/// Every file under `d`, relative to it.
fn walk(d: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![d.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in fs::read_dir(&p).into_iter().flatten().flatten() {
            let path = e.path();
            // A link is left out: one pointing back up would never end, one pointing out would copy what isn't the skill.
            // So is what is hidden (`.git`, `.DS_Store`, an editor's swap file): not the skill's, and it changes by itself.
            if e.file_type().is_ok_and(|t| t.is_symlink()) || e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(r) = path.strip_prefix(d) {
                out.push(r.to_path_buf());
            }
        }
    }
    out.sort();
    out
}

/// Every skill — Echo's first (as Customized, if so), then the user's by name — with what the window shows: its
/// description, kind (builtin / customized / yours), on, problems, what it holds, its folder, its SKILL.md.
pub fn list() -> Vec<Value> {
    let offs = off();
    let mut out = vec![];
    let item = |id: &str, kind: &str, text: Option<String>, folder: Option<PathBuf>, files: Value| {
        let desc = text.as_deref().and_then(split).and_then(|(f, _)| f.into_iter().find(|(k, _)| k == "description").map(|(_, v)| v)).unwrap_or_default();
        json!({"id": id, "kind": kind, "description": desc, "on": !offs.iter().any(|o| o == id), "problems": problems(id, text.as_deref()),
               "files": files, "folder": folder, "text": text})
    };
    for (id, files) in BUILTIN {
        let mine = dir().join(id);
        if mine.is_dir() {
            out.push(item(id, "customized", fs::read_to_string(mine.join("SKILL.md")).ok(), Some(mine.clone()), contents(&mine)));
        } else {
            let text = files.iter().find(|(f, _)| *f == "SKILL.md").map(|(_, t)| t.to_string());
            let n = |sub: &str| files.iter().filter(|(f, _)| f.starts_with(&format!("{sub}/"))).count();
            out.push(item(id, "builtin", text, None, json!({"scripts": n("scripts"), "references": n("references"), "assets": n("assets"), "other": 0})));
        }
    }
    let mut mine: Vec<String> = fs::read_dir(dir()).into_iter().flatten().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    mine.sort();
    for id in mine.into_iter().filter(|i| !BUILTIN.iter().any(|(b, _)| b == i) && !i.starts_with('.')) {
        let d = dir().join(&id);
        out.push(item(&id, "yours", fs::read_to_string(d.join("SKILL.md")).ok(), Some(d.clone()), contents(&d)));
    }
    out
}

fn known(id: &str) -> Option<Value> {
    list().into_iter().find(|s| s["id"] == id)
}

/// A new skill of the user's: its folder with a `SKILL.md` to fill in (the window opens it in VS Code).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn create(name: &str) -> Result<PathBuf> {
    // Claude Code's rule for a skill's name: latin lower-case letters, digits and single hyphens.
    let id = crate::recorder::slugify(name).chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect::<String>().split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let id = id.chars().take(64).collect::<String>().trim_end_matches('-').to_string();
    anyhow::ensure!(!id.is_empty(), "give the skill a name in latin letters (Claude Code's rule for skill names) — its text can be in any language");
    anyhow::ensure!(known(&id).is_none() && !dir().join(&id).exists(), "a skill named {id} exists already");
    let d = dir().join(&id);
    fs::create_dir_all(&d)?;
    let text = format!(
        "---\nname: {id}\ndescription: Use when … — say what it does and when, with the words a task would use.\n---\n\n# {}\n\n1. Read the task (`echoctl read <its echo:// link>`).\n2. …\n3. Show the user the result before anything goes to anyone.\n",
        name.trim()
    );
    paths::write_atomic(&d.join("SKILL.md"), text.as_bytes())?;
    Ok(d)
}

/// An Echo skill made the user's: its files copied to their folder (from then on theirs to change).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn customize(id: &str) -> Result<PathBuf> {
    let (_, files) = BUILTIN.iter().find(|(b, _)| *b == id).ok_or_else(|| anyhow::anyhow!("{id} isn't one of Echo's skills"))?;
    let d = dir().join(id);
    anyhow::ensure!(!d.exists(), "{id} is customized already");
    for (f, text) in *files {
        let p = d.join(f);
        fs::create_dir_all(p.parent().unwrap_or(&d))?;
        fs::write(&p, text)?;
    }
    Ok(d)
}

/// Take the user's folder away — a customized skill is Echo's own again, one of theirs is gone (kept in the trash).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn remove(id: &str) -> Result<()> {
    let s = known(id).ok_or_else(|| anyhow::anyhow!("there is no skill {id}"))?;
    anyhow::ensure!(s["kind"] != "builtin", "{id} is Echo's own: nothing to take away");
    let bin = paths::state_dir().join("trash").join("skills");
    fs::create_dir_all(&bin)?;
    fs::rename(dir().join(id), trash_path(&bin, id, chrono::Utc::now()))?;
    // A new skill of the same name later starts on.
    crate::calls::update_state(&off_file(), |offs: &mut Vec<String>| {
        offs.retain(|o| o != id);
        Ok(())
    })
}

/// Where a skill taken away goes in the trash `bin`: `<id>-<time>`, or `<id>-<time>-2`, `-3`… when that is taken (one
/// made again and taken away again within the same millisecond).
fn trash_path(bin: &Path, id: &str, now: chrono::DateTime<chrono::Utc>) -> PathBuf {
    let base = format!("{id}-{}", now.format("%Y%m%d%H%M%S%3f"));
    (1..).map(|n| bin.join(if n == 1 { base.clone() } else { format!("{base}-{n}") })).find(|p| !p.exists()).unwrap_or_else(|| bin.join(base))
}

/// Switch a skill on or off: off, it stays out of the Claude panel's session.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn set_on(id: &str, on: bool) -> Result<()> {
    anyhow::ensure!(known(id).is_some(), "there is no skill {id}");
    crate::calls::update_state(&off_file(), |offs: &mut Vec<String>| {
        offs.retain(|o| o != id);
        if !on {
            offs.push(id.to_string());
        }
        Ok(())
    })
}

/// A skill's folder (the user's), for VS Code or Finder.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn folder(id: &str) -> Option<PathBuf> {
    known(id).and_then(|s| s["folder"].as_str().map(PathBuf::from))
}

/// What the panel's session would load now (the skills that are on and valid, every file): when it differs from what
/// it started with, the panel says "Restart to update" (`terminal::loaded_now`).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn fingerprint() -> (String, Option<std::time::SystemTime>) {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    let mut at = None;
    for s in list().into_iter().filter(|s| s["on"] == true && s["problems"].as_array().is_some_and(|p| p.is_empty())) {
        h.update(s["id"].as_str().unwrap_or("").as_bytes());
        h.update(s["text"].as_str().unwrap_or("").as_bytes());
        // Each file by its name, size and time — not its bytes: a folder of assets is read every poll otherwise.
        if let Some(d) = s["folder"].as_str() {
            for f in walk(Path::new(d)) {
                h.update(f.to_string_lossy().as_bytes());
                let m = fs::metadata(Path::new(d).join(&f)).ok();
                h.update(m.as_ref().map(|m| m.len()).unwrap_or(0).to_le_bytes());
                let t = m.and_then(|m| m.modified().ok());
                h.update(format!("{t:?}").as_bytes());
                at = at.max(t);
            }
        }
    }
    (h.finalize().iter().take(8).map(|x| format!("{x:02x}")).collect(), at)
}

/// The `my-skills` plugin as the skills are now (those on and valid, every file of each): one folder per version
/// (`<config>/my-skills/<fingerprint>/`), written whole under another name and renamed into place, never changed
/// after — a session or a Yolo run that loaded one keeps it while another is made. Versions a week old go.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn plugin() -> Result<PathBuf> {
    let base = paths::config_dir().join(PLUGIN);
    let done = base.join(fingerprint().0);
    // Each use is marked (`.used`): the sweep goes by the last use, not by when a version was made.
    let used = |d: &Path| fs::write(d.join(".used"), b"");
    if done.join(".claude-plugin/plugin.json").exists() {
        used(&done)?;
        return Ok(done);
    }
    fs::create_dir_all(&base)?;
    let root = base.join(format!(".new-{}-{}", std::process::id(), chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)));
    let built = build_plugin(&root);
    if built.is_ok() && fs::rename(&root, &done).is_ok() {
        used(&done)?;
        sweep_plugins(&base, &done);
        return Ok(done);
    }
    // Failed, or another start made the same version first.
    let _ = fs::remove_dir_all(&root);
    built?;
    anyhow::ensure!(done.join(".claude-plugin/plugin.json").exists(), "the skills plugin couldn't be put in place");
    used(&done)?;
    Ok(done)
}

/// Old versions of the plugin (unused for a week), and half-made ones left by a crash.
fn sweep_plugins(base: &Path, keep: &Path) {
    let week = std::time::Duration::from_secs(7 * 24 * 3600);
    for e in fs::read_dir(base).into_iter().flatten().flatten() {
        let p = e.path();
        let last = fs::metadata(p.join(".used")).or_else(|_| e.metadata());
        let old = last.and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a > week);
        if p != keep && old && p.is_dir() {
            let _ = fs::remove_dir_all(&p);
        }
    }
}

fn build_plugin(root: &Path) -> Result<()> {
    let skills = root.join("skills");
    fs::create_dir_all(root.join(".claude-plugin"))?;
    paths::write_atomic(
        &root.join(".claude-plugin/plugin.json"),
        &serde_json::to_vec_pretty(&json!({"name": PLUGIN, "version": "1.0.0", "description": "Your skills from Echo (Settings → Templates → Skills)."}))?,
    )?;
    for s in list().into_iter().filter(|s| s["on"] == true && s["problems"].as_array().is_some_and(|p| p.is_empty())) {
        let id = s["id"].as_str().unwrap_or_default();
        let to = skills.join(id);
        match s["folder"].as_str() {
            Some(from) => {
                for f in walk(Path::new(from)) {
                    let p = to.join(&f);
                    fs::create_dir_all(p.parent().unwrap_or(&to))?;
                    fs::copy(Path::new(from).join(&f), &p)?;
                }
            }
            None => {
                for (f, text) in BUILTIN.iter().find(|(b, _)| *b == id).map(|(_, f)| *f).unwrap_or(&[]) {
                    let p = to.join(f);
                    fs::create_dir_all(p.parent().unwrap_or(&to))?;
                    fs::write(&p, text)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_skill_taken_away_twice_in_the_same_millisecond_keeps_both_in_the_trash() {
        // Made again and taken away again at once (#251's third flaky test): the second move must not hit the first's
        // name, a folder already there.
        let bin = std::env::temp_dir().join(format!("echo-skills-trash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&bin);
        fs::create_dir_all(&bin).unwrap();
        let now = chrono::Utc::now();
        let first = trash_path(&bin, "status-report", now);
        fs::create_dir_all(first.join("scripts")).unwrap();
        let second = trash_path(&bin, "status-report", now);
        assert_ne!(first, second);
        assert!(!second.exists() && second.file_name().unwrap().to_string_lossy().starts_with("status-report-"));
        let _ = fs::remove_dir_all(&bin);
    }
    use super::*;

    #[test]
    fn skills_are_folders_checked_by_the_spec_and_make_a_plugin() {
        let (_env, _) = paths::testenv::folders("skills");
        let s = &list()[0];
        assert_eq!((s["id"].as_str(), s["kind"].as_str(), s["on"].as_bool()), (Some("follow-up-mail"), Some("builtin"), Some(true)));
        assert!(s["problems"].as_array().unwrap().is_empty(), "{s}");
        assert!(s["description"].as_str().unwrap().starts_with("Use when"));
        // Customized: Echo's files become the user's; Reset takes them back.
        let d = customize("follow-up-mail").unwrap();
        assert!(d.join("SKILL.md").is_file());
        assert_eq!(list()[0]["kind"], "customized");
        // One of theirs, with scripts and references; the spec's rules on it.
        let mine = create("Status report").unwrap();
        assert!(mine.ends_with("status-report"));
        fs::create_dir_all(mine.join("scripts")).unwrap();
        fs::write(mine.join("scripts/collect.sh"), "echo hi").unwrap();
        fs::create_dir_all(mine.join("references")).unwrap();
        fs::write(mine.join("references/FORMAT.md"), "# Format").unwrap();
        let st = list().into_iter().find(|s| s["id"] == "status-report").unwrap();
        assert_eq!(st["files"], json!({"scripts": 1, "references": 1, "assets": 0, "other": 0}));
        assert!(st["problems"].as_array().unwrap().is_empty(), "{st}");
        let bad = dir().join("Bad_Skill");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("SKILL.md"), "---\nname: other\n---\nx").unwrap();
        let b = list().into_iter().find(|s| s["id"] == "Bad_Skill").unwrap();
        let p: Vec<&str> = b["problems"].as_array().unwrap().iter().filter_map(|x| x.as_str()).collect();
        assert!(p.iter().any(|x| x.contains("isn't its folder's name")) && p.iter().any(|x| x.contains("no description")), "{p:?}");
        // The plugin: valid skills that are on, every file of each.
        set_on("status-report", false).unwrap();
        let root = plugin().unwrap();
        assert!(root.join("skills/follow-up-mail/SKILL.md").is_file());
        assert!(!root.join("skills/status-report").exists(), "off");
        assert!(!root.join("skills/Bad_Skill").exists(), "invalid");
        set_on("status-report", true).unwrap();
        let first = root;
        let root = plugin().unwrap();
        assert_ne!(root, first, "a new version beside the old one");
        assert!(first.join("skills/follow-up-mail/SKILL.md").is_file(), "the old one stays whole for a session that loaded it");
        assert_eq!(plugin().unwrap(), root, "unchanged skills: the same version");
        assert_eq!(fs::read_to_string(root.join("skills/status-report/scripts/collect.sh")).unwrap(), "echo hi");
        assert!(root.join("skills/status-report/references/FORMAT.md").is_file());
        // Taken away: theirs to the trash, Echo's back to built in; one made again later starts on.
        set_on("status-report", false).unwrap();
        remove("status-report").unwrap();
        create("Status report").unwrap();
        assert_eq!(list().into_iter().find(|s| s["id"] == "status-report").unwrap()["on"], true);
        remove("status-report").unwrap();
        remove("follow-up-mail").unwrap();
        assert_eq!(list()[0]["kind"], "builtin");
        assert!(remove("follow-up-mail").is_err());
        assert!(create("Follow up mail").is_err(), "Echo's name");
        assert!(create("Звіт").is_err(), "a name Claude Code wouldn't take");
        assert_eq!(create("Звіт for Acme").unwrap().file_name().unwrap(), "for-acme");
        // Windows line ends and a byte-order mark read as Claude Code reads them.
        let crlf = dir().join("crlf-skill");
        fs::create_dir_all(&crlf).unwrap();
        fs::write(crlf.join("SKILL.md"), "\u{feff}---\r\nname: crlf-skill\r\ndescription: Use when x.\r\n---\r\nbody\r\n").unwrap();
        assert!(list().into_iter().find(|s| s["id"] == "crlf-skill").unwrap()["problems"].as_array().unwrap().is_empty());
        // A description written as a YAML block: its lines, folded.
        let folded = dir().join("folded-skill");
        fs::create_dir_all(&folded).unwrap();
        fs::write(folded.join("SKILL.md"), "---\nname: folded-skill\ndescription: >\n  Use when a weekly\n  report is due.\nlicense: MIT\n---\nbody\n").unwrap();
        let f = list().into_iter().find(|s| s["id"] == "folded-skill").unwrap();
        assert_eq!(f["description"], "Use when a weekly report is due.");
        fs::write(folded.join("SKILL.md"), "---\nname: folded-skill\ndescription:\n  Use when a weekly\n  report is due.\n---\nbody\n").unwrap();
        assert_eq!(list().into_iter().find(|s| s["id"] == "folded-skill").unwrap()["description"], "Use when a weekly report is due.", "a plain value on the lines under");
        // What is hidden is not the skill's: Finder's .DS_Store changes nothing.
        let before = fingerprint().0;
        fs::write(folded.join(".DS_Store"), "x").unwrap();
        assert_eq!(fingerprint().0, before);
        assert!(f["problems"].as_array().unwrap().is_empty(), "{f}");
        // What the session would load changes with a skill switched off.
        let before = fingerprint().0;
        set_on("crlf-skill", false).unwrap();
        assert_ne!(fingerprint().0, before);
    }
}

//! Settings → Templates → Skills: Agent Skills (agentskills.io) for the work a task asks for — each a folder:
//! `SKILL.md` (frontmatter `name`, `description`…, then what to do) and, as it needs, `scripts/`, `references/`,
//! `assets/`. Echo keeps the catalogue — what there is, whether it is valid, on or off — and shows it; their content is
//! written elsewhere (VS Code, Claude: Ask Claude, Open in VS Code). Echo's own are built in, read-only until
//! Customized (copied to the user's); the user's are `<home>/.echo/skills/<id>/`. Each time the Claude panel's session
//! starts, Echo copies the valid skills that are on into a plugin, `my-skills` (`<config>/my-skills/`): only their
//! descriptions sit in its context until one is used. Send to Claude names one (`/my-skills:<id>`).

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
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

/// A `SKILL.md`'s frontmatter (one `key: value` a line, values unquoted) and body.
fn split(text: &str) -> Option<(Vec<(String, String)>, String)> {
    let (front, body) = text.strip_prefix("---\n").and_then(|t| t.split_once("\n---"))?;
    let fields = front
        .lines()
        .filter(|l| !l.starts_with(' ') && !l.starts_with('\t'))
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().trim_matches('"').trim_matches('\'').to_string()))
        .collect();
    Some((fields, body.trim_start_matches('\n').trim().to_string()))
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
            let ok = n.len() <= 64 && n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !n.starts_with('-') && !n.ends_with('-') && !n.contains("--");
            if !ok {
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
        let body = text.as_deref().and_then(split).map(|(_, b)| b).unwrap_or_default();
        json!({"id": id, "kind": kind, "description": desc, "on": !offs.iter().any(|o| o == id), "problems": problems(id, text.as_deref()),
               "files": files, "folder": folder, "body": body})
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
    let id = crate::recorder::slugify(name).chars().take(64).collect::<String>().trim_end_matches('-').to_string();
    anyhow::ensure!(!id.is_empty(), "give the skill a name");
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
    fs::rename(dir().join(id), bin.join(format!("{id}-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"))))?;
    Ok(())
}

/// Switch a skill on or off: off, it stays out of the Claude panel's session.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn set_on(id: &str, on: bool) -> Result<()> {
    anyhow::ensure!(known(id).is_some(), "there is no skill {id}");
    let mut offs = off();
    offs.retain(|o| o != id);
    if !on {
        offs.push(id.to_string());
    }
    fs::create_dir_all(paths::state_dir())?;
    paths::write_atomic(&off_file(), &serde_json::to_vec_pretty(&offs)?)?;
    Ok(())
}

/// A skill's folder (the user's), for VS Code or Finder.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn folder(id: &str) -> Option<PathBuf> {
    known(id).and_then(|s| s["folder"].as_str().map(PathBuf::from))
}

/// The `my-skills` plugin, written afresh: the skills that are on and valid, every file of each.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn plugin() -> Result<PathBuf> {
    let root = paths::config_dir().join(PLUGIN);
    let skills = root.join("skills");
    if skills.exists() {
        fs::remove_dir_all(&skills).with_context(|| format!("clearing {}", skills.display()))?;
    }
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
    Ok(root)
}

#[cfg(test)]
mod tests {
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
        let root = plugin().unwrap();
        assert_eq!(fs::read_to_string(root.join("skills/status-report/scripts/collect.sh")).unwrap(), "echo hi");
        assert!(root.join("skills/status-report/references/FORMAT.md").is_file());
        // Taken away: theirs to the trash, Echo's back to built in.
        remove("status-report").unwrap();
        remove("follow-up-mail").unwrap();
        assert_eq!(list()[0]["kind"], "builtin");
        assert!(remove("follow-up-mail").is_err());
        assert!(create("Follow up mail").is_err(), "Echo's name");
    }
}

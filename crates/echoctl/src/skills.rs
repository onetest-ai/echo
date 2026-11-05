//! Settings → Templates → Skills: Claude Code skills for the work a task asks for (a follow-up mail…). Echo's own are
//! built in; the user's — and Echo's as the user changed them — are `<home>/.echo/skills/<id>/SKILL.md`. Echo puts them
//! together as a small Claude Code plugin, `my-skills` (`<config>/my-skills/`), loaded by the Claude panel's session
//! (`terminal::start_folder`): only their descriptions sit in its context until one is used. Send to Claude on a task
//! types the task, and the skill chosen, into that session (`/my-skills:<id>`).

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::paths;

/// The plugin's name: a skill is `/my-skills:<id>` in the session.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub const PLUGIN: &str = "my-skills";

const BUILTIN: &[(&str, &str)] = &[("follow-up-mail", include_str!("../skills/follow-up-mail.md"))];

fn dir() -> PathBuf {
    paths::state_dir().join("skills")
}

/// A skill file's description (frontmatter `description:`) and body.
fn parse(text: &str) -> (String, String) {
    let Some((front, body)) = text.strip_prefix("---\n").and_then(|t| t.split_once("\n---\n")) else {
        return (String::new(), text.trim().to_string());
    };
    let desc = front.lines().find_map(|l| l.strip_prefix("description:")).map(|d| d.trim().trim_matches('"').to_string()).unwrap_or_default();
    (desc, body.trim().to_string())
}

fn file_text(id: &str, description: &str, body: &str) -> String {
    let one_line = description.split_whitespace().collect::<Vec<_>>().join(" ").replace('"', "'");
    format!("---\nname: {id}\ndescription: \"{one_line}\"\n---\n\n{}\n", body.trim())
}

/// Every skill — `{id, description, body, kind}`, kind builtin / edited / yours — Echo's first.
pub fn list() -> Vec<Value> {
    let mut out = vec![];
    let own = |id: &str| fs::read_to_string(dir().join(id).join("SKILL.md")).ok();
    for (id, text) in BUILTIN {
        let (kind, t) = match own(id) {
            Some(t) => ("edited", t),
            None => ("builtin", text.to_string()),
        };
        let (description, body) = parse(&t);
        out.push(json!({"id": id, "description": description, "body": body, "kind": kind}));
    }
    let mut mine: Vec<String> = fs::read_dir(dir()).into_iter().flatten().flatten().filter(|e| e.path().join("SKILL.md").is_file()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    mine.sort();
    for id in mine.into_iter().filter(|i| !BUILTIN.iter().any(|(b, _)| b == i)) {
        let (description, body) = parse(&own(&id).unwrap_or_default());
        out.push(json!({"id": id, "description": description, "body": body, "kind": "yours"}));
    }
    out
}

/// Save a skill the user changed (a built-in keeps its id) or made (its id from its name).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn save(id: Option<&str>, name: &str, description: &str, body: &str) -> Result<String> {
    let all = list();
    let id = match id.filter(|i| !i.is_empty()) {
        Some(i) => {
            anyhow::ensure!(all.iter().any(|s| s["id"] == i), "there is no skill {i}");
            i.to_string()
        }
        None => {
            let i = crate::recorder::slugify(name).trim_end_matches('-').to_string();
            anyhow::ensure!(!i.is_empty(), "give the skill a name");
            anyhow::ensure!(!all.iter().any(|s| s["id"] == i.as_str()) && !dir().join(&i).exists(), "a skill named {i} exists already");
            i
        }
    };
    anyhow::ensure!(!description.trim().is_empty(), "say when to use it: Claude picks a skill by that");
    let d = dir().join(&id);
    fs::create_dir_all(&d)?;
    paths::write_atomic(&d.join("SKILL.md"), file_text(&id, description, body).as_bytes())?;
    Ok(id)
}

/// Take the user's file away: an edited built-in is Echo's own again, one of theirs is gone (kept in the trash).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn remove(id: &str) -> Result<()> {
    anyhow::ensure!(list().iter().any(|s| s["id"] == id && s["kind"] != "builtin"), "{id} is Echo's own: nothing to take away");
    let bin = paths::state_dir().join("trash").join("skills");
    fs::create_dir_all(&bin)?;
    fs::rename(dir().join(id), bin.join(format!("{id}-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"))))?;
    Ok(())
}

/// The `my-skills` plugin, written afresh from the skills there are now (the Claude panel's session loads it).
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
    for s in list() {
        let id = s["id"].as_str().unwrap_or_default();
        fs::create_dir_all(skills.join(id))?;
        fs::write(skills.join(id).join("SKILL.md"), file_text(id, s["description"].as_str().unwrap_or(""), s["body"].as_str().unwrap_or("")))?;
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skills_are_echos_then_yours_and_make_a_plugin() {
        let (_env, _) = paths::testenv::folders("skills");
        assert_eq!(list()[0]["id"], "follow-up-mail");
        assert_eq!(list()[0]["kind"], "builtin");
        assert!(list()[0]["description"].as_str().unwrap().starts_with("Use when"));
        save(Some("follow-up-mail"), "", "Use when a follow-up mail is due.", "Short, and in the client's language.").unwrap();
        assert_eq!(list()[0]["kind"], "edited");
        assert_eq!(save(None, "Status report", "Use when a weekly status is due.", "# Status\n- done\n- next").unwrap(), "status-report");
        assert!(save(None, "Status report", "x", "y").is_err(), "taken");
        assert!(save(None, "No description", "  ", "y").is_err());
        let p = plugin().unwrap();
        let manifest: Value = serde_json::from_slice(&fs::read(p.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
        assert_eq!(manifest["name"], PLUGIN);
        let s = fs::read_to_string(p.join("skills/status-report/SKILL.md")).unwrap();
        assert!(s.starts_with("---\nname: status-report\ndescription: \"Use when a weekly status is due.\"\n---"), "{s}");
        assert!(fs::read_to_string(p.join("skills/follow-up-mail/SKILL.md")).unwrap().contains("client's language"), "the edit, not Echo's");
        remove("status-report").unwrap();
        remove("follow-up-mail").unwrap();
        assert!(remove("follow-up-mail").is_err());
        let p = plugin().unwrap();
        assert!(!p.join("skills/status-report").exists(), "a skill taken away leaves the plugin");
        assert_eq!(list()[0]["kind"], "builtin");
    }
}

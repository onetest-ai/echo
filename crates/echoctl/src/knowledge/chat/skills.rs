//! The skills Ask actually loads: the selected profile's user skills/commands and the vault's skills. This same
//! catalog drives the slash picker, the process prompt and the guard. Nothing is installed or copied into a vault.

use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ai;
use crate::knowledge::{detect, fm, Vault};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub source: String,
    pub argument_hint: String,
    pub user_invocable: bool,
    #[serde(skip)]
    pub read: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog(pub Vec<Skill>);

fn user_dir(p: &ai::Profile, _cwd: &Path) -> Result<PathBuf> {
    // Tests never discover the developer's own skills or start their login shell.
    #[cfg(test)]
    return Ok(p.config_folder());
    #[cfg(not(test))]
    if p.kind.managed() {
        Ok(p.config_folder())
    } else {
        ai::system_config_dirs(_cwd).map_err(anyhow::Error::msg)?.into_iter().next().ok_or_else(|| anyhow::anyhow!("Claude Code names no user settings folder"))
    }
}

fn metadata(file: &Path, fallback: String, source: &str, command: bool) -> Option<Skill> {
    let text = std::fs::read_to_string(file).ok()?;
    let (head, body) = fm::split(&text);
    let props = head.map(fm::parse).unwrap_or_default();
    let get = |key| fm::get(&props, key).map(|p| p.text()).unwrap_or_default();
    let name = if command || get("name").is_empty() { fallback } else { get("name") };
    if name.is_empty() || name.len() > 256 || name.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '<' | '>')) {
        return None;
    }
    let description = get("description");
    let description = if description.is_empty() { body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim_start_matches('#').trim().to_string() } else { description };
    let read = dunce::canonicalize(if command { file } else { file.parent()? }).ok()?;
    Some(Skill { name, description: description.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(400).collect(), source: source.into(), argument_hint: get("argument-hint"), user_invocable: get("user-invocable") != "false", read })
}

/// Skills first, then legacy commands, including namespaced command subdirectories. No supporting files are read.
fn scan(dir: &Path, source: &str) -> Vec<Skill> {
    let mut out = vec![];
    let base = dir.join("skills");
    let mut dirs: Vec<_> = std::fs::read_dir(&base).into_iter().flatten().flatten().filter(|e| !e.file_name().to_string_lossy().starts_with('.')).collect();
    dirs.sort_by_key(|e| e.file_name());
    for e in dirs {
        let file = e.path().join("SKILL.md");
        if let Some(s) = metadata(&file, e.file_name().to_string_lossy().into_owned(), source, false) {
            out.push(s);
        }
    }
    let base = dir.join("commands");
    for rel in detect::walk(&base, &[], |r| r.ends_with(".md")) {
        if let Some(s) = metadata(&base.join(&rel), rel.trim_end_matches(".md").replace('/', ":"), source, true) {
            out.push(s);
        }
    }
    out
}

impl Catalog {
    pub fn load(v: &Vault, p: &ai::Profile) -> Result<Self> {
        let user = user_dir(p, &v.root)?;
        let mut by_name = BTreeMap::new();
        // Claude Code gives personal skills precedence over project skills. A skill beats a command in its scope.
        for s in scan(&user, "user") {
            by_name.entry(s.name.clone()).or_insert(s);
        }
        for mut s in scan(&v.root.join(".claude"), "vault") {
            if p.kind.managed() {
                s.name = format!("{}:{}", v.id, s.name);
            }
            by_name.entry(s.name.clone()).or_insert(s);
        }
        // Honour visibility choices from the settings sources this profile loads; Claude still enforces its own
        // managed policies. Never return settings, credentials or skill bodies to the page.
        let mut settings = vec![user.join("settings.json")];
        if !p.kind.managed() {
            settings.extend([v.root.join(".claude/settings.json"), v.root.join(".claude/settings.local.json")]);
        }
        let mut visibility = BTreeMap::new();
        for file in settings {
            let value: serde_json::Value = std::fs::read(file).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            if let Some(overrides) = value["skillOverrides"].as_object() {
                for (name, state) in overrides {
                    visibility.insert(name.clone(), state.clone());
                }
            }
        }
        for (name, state) in visibility {
            if state == "off" { by_name.remove(&name); }
            else if state == "name-only" { if let Some(s) = by_name.get_mut(&name) { s.description.clear(); } }
        }
        Ok(Catalog(by_name.into_values().collect()))
    }

    pub fn names(&self) -> Vec<String> { self.0.iter().map(|s| s.name.clone()).collect() }

    pub fn read_paths(&self) -> Vec<String> { self.0.iter().filter(|s| s.source == "user").map(|s| s.read.display().to_string()).collect() }

    pub fn validate(&self, text: &str) -> Result<()> {
        if text == "/" { bail!("Choose a skill from the / menu first."); }
        if let Some(name) = text.strip_prefix('/').and_then(|t| t.split_whitespace().next()) {
            match self.0.iter().find(|s| s.name == name) {
                Some(s) if s.user_invocable => {}
                Some(_) => bail!("/{name} is a background skill; choose a skill from the / menu"),
                None => bail!("/{name} isn't available on this AI profile. Type / to choose a skill."),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{self, fixture, AddOpts};

    #[test]
    fn user_skills_and_commands_load_without_a_copy_and_keep_their_metadata() {
        let (_env, root) = crate::paths::testenv::folders("chat-user-skills");
        let t = fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        let p = ai::Settings::load().unwrap().get("system").unwrap();
        let user = p.config_folder();
        fixture::write(&user, "commands/personal-twin.md", "Answer from my knowledge.\n\nUse $ARGUMENTS.");
        fixture::write(&user, "commands/tools/review.md", "---\nname: ignored\ndescription: Review a document\nargument-hint: \"[question]\"\n---\nRead it.");
        fixture::write(&user, "skills/s/SKILL.md", "---\nname: s\ndescription: My personal skill wins\n---\nRead references.md.");
        fixture::write(&user, "commands/s.md", "A command loses to the skill.");
        fixture::write(&user, "skills/background/SKILL.md", "---\nuser-invocable: false\n---\nBackground guidance.");
        let before = fixture::snapshot(&v.root);
        let c = Catalog::load(&v, &p).unwrap();
        let own = c.0.iter().find(|s| s.name == "personal-twin").unwrap();
        assert_eq!(own.source, "user");
        assert_eq!(own.description, "Answer from my knowledge.");
        assert_eq!(own.read, dunce::canonicalize(user.join("commands/personal-twin.md")).unwrap());
        assert_eq!(c.0.iter().find(|s| s.name == "tools:review").unwrap().argument_hint, "[question]");
        assert_eq!(c.0.iter().find(|s| s.name == "s").unwrap().description, "My personal skill wins");
        assert!(c.validate("/personal-twin explain the flywheel").is_ok());
        assert!(c.validate("/background").is_err());
        assert!(c.validate("/unknown").is_err());
        assert!(c.validate("/").is_err());
        assert_eq!(fixture::snapshot(&v.root), before, "discovery writes nothing to the vault");
        let page = serde_json::to_string(&c.0).unwrap();
        assert!(!page.contains(&user.display().to_string()), "no host paths sent to the page");
    }

    #[test]
    fn skills_belong_to_the_selected_profile_and_visibility_is_respected() {
        let (_env, root) = crate::paths::testenv::folders("chat-skill-profiles");
        let t = fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        let p: ai::Profile = serde_json::from_value(serde_json::json!({"id":"work", "name":"Work", "kind":"claude"})).unwrap();
        fixture::write(&p.config_folder(), "commands/personal-twin.md", "My profile's twin.");
        fixture::write(&p.config_folder(), "settings.json", r#"{"skillOverrides":{"personal-twin":"off"}}"#);
        fixture::write(&root.join("config/claude/system"), "commands/other.md", "Another profile's command.");
        let c = Catalog::load(&v, &p).unwrap();
        assert_eq!(c.names(), ["twin:c", "twin:s"]);
        std::fs::remove_file(p.config_folder().join("settings.json")).unwrap();
        assert_eq!(Catalog::load(&v, &p).unwrap().names(), ["personal-twin", "twin:c", "twin:s"]);
    }
}

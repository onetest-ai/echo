//! Skills a plugin adds to the user's own Claude Code (`ctx.claude.installSkill` / `removeSkill`, manifest
//! `uses.claude.skills: true`): each on the user's click in its window half, only `<Claude Code folder>/skills/<name>/SKILL.md`
//! (Markdown, nothing else). Which plugin added which skill Echo keeps in `<config dir>/plugins/claude-skills.json`
//! (`{name: plugin id}`), never in the skill: a plugin changes or removes only the skills it added, never one that
//! was there already, and its skills go when it is removed.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, ensure, Result};
use serde_json::{Map, Value};

use crate::paths;

/// The longest skill a plugin adds.
pub const MAX_SKILL: usize = 256 * 1024;

fn record_file() -> PathBuf {
    paths::config_dir().join("plugins").join("claude-skills.json")
}

/// One writer of the record and the skills it names at a time, across processes (the app, `echoctl plugin remove`).
fn lock() -> Result<crate::app::FileLock> {
    let f = record_file();
    fs::create_dir_all(f.parent().expect("a parent"))?;
    super::wait_lock(&f.with_extension("lock"))
}

fn record() -> Map<String, Value> {
    fs::read(record_file()).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}

fn save(r: &Map<String, Value>) -> Result<()> {
    paths::write_atomic(&record_file(), &serde_json::to_vec_pretty(r)?)?;
    Ok(())
}

/// The user's own Claude Code skills folder (`CLAUDE_CONFIG_DIR` when Echo has it, else `~/.claude`). On test data
/// (`ECHOCTL_TEST_OPEN_LOG_ONLY`) only the test's own Claude Code folder (`CLAUDE_CONFIG_DIR`), never this computer's.
fn skills_dir() -> Result<PathBuf> {
    let test = std::env::var_os("ECHOCTL_TEST_OPEN_LOG_ONLY").is_some_and(|v| !v.is_empty());
    if test && std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).is_none_or(|d| !d.is_absolute()) {
        bail!("on test data a plugin's skill goes only to the test's own Claude Code folder (CLAUDE_CONFIG_DIR), and none is set");
    }
    Ok(paths::own_claude_folder().join("skills"))
}

/// May plugin `id` add skills? Installed and on, its sheet naming `uses.claude.skills`. Its name.
fn may(id: &str) -> Result<String> {
    let Some(p) = super::enabled().into_iter().find(|p| p.id == id) else { bail!("{id} isn't installed and on") };
    let m = super::read_manifest(&super::plugin_dir(&p))?;
    ensure!(m.uses.get("claude").is_some_and(|c| c["skills"] == true), "{} didn't ask to add skills to your Claude Code when it was installed", m.name);
    Ok(m.name)
}

fn check_name(name: &str) -> Result<()> {
    ensure!(crate::grants::skill_name(name), "a skill's name is 1–64 of a-z, 0-9 and -");
    Ok(())
}

/// `ctx.claude.installSkill(name, md)`: `md` as `<skills>/<name>/SKILL.md` (written whole, replacing the file, never
/// following one), its folder made new — or one this plugin added before. Its path.
pub fn install(id: &str, name: &str, md: &str) -> Result<PathBuf> {
    let plugin = may(id)?;
    check_name(name)?;
    ensure!(md.len() <= MAX_SKILL, "a skill is 256 KB at most");
    ensure!(!md.trim().is_empty(), "the skill is empty");
    let skills = skills_dir()?;
    let _l = lock()?;
    let mut r = record();
    let dir = skills.join(name);
    match fs::symlink_metadata(&dir) {
        Ok(_) if r.get(name).and_then(Value::as_str) != Some(id) => bail!("your Claude Code has a skill “{name}” already: {plugin} doesn't change it"),
        Ok(m) if !m.is_dir() => bail!("“{name}” in your Claude Code's skills isn't a folder: {plugin} doesn't change it"),
        Ok(_) => {}
        Err(_) => {
            fs::create_dir_all(&skills)?;
            // Made new: a folder that came meanwhile is the user's.
            fs::create_dir(&dir).map_err(|e| anyhow!("“{name}”: {e}"))?;
            r.insert(name.into(), id.into());
            save(&r)?;
        }
    }
    let f = dir.join("SKILL.md");
    ensure!(!fs::symlink_metadata(&f).is_ok_and(|m| !m.is_file()), "“{name}/SKILL.md” isn't a file: {plugin} doesn't change it");
    paths::write_atomic(&f, md.as_bytes())?;
    Ok(f)
}

/// Take skill `name` out: its `SKILL.md`, and its folder when nothing else is in it (what the user added stays). A
/// link in its place is left as it is.
fn take_out(skills: &Path, name: &str) {
    let dir = skills.join(name);
    if fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
        let f = dir.join("SKILL.md");
        if fs::symlink_metadata(&f).is_ok_and(|m| m.is_file()) {
            let _ = fs::remove_file(&f);
        }
        let _ = fs::remove_dir(&dir);
    }
}

/// `ctx.claude.removeSkill(name)`: a skill this plugin added, taken out.
pub fn remove(id: &str, name: &str) -> Result<()> {
    let plugin = may(id)?;
    check_name(name)?;
    let skills = skills_dir()?;
    let _l = lock()?;
    let mut r = record();
    ensure!(r.get(name).and_then(Value::as_str) == Some(id), "“{name}” isn't a skill {plugin} added");
    take_out(&skills, name);
    r.remove(name);
    save(&r)
}

/// The skills plugin `id` added: all taken out (its removal, data kept or not).
pub fn remove_plugin(id: &str) -> Result<()> {
    let _l = lock()?;
    let mut r = record();
    let mine: Vec<String> = r.iter().filter(|(_, v)| v.as_str() == Some(id)).map(|(k, _)| k.clone()).collect();
    if mine.is_empty() {
        return Ok(());
    }
    let skills = skills_dir()?;
    for name in &mine {
        take_out(&skills, name);
        r.remove(name);
    }
    save(&r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn install_test_plugins(root: &Path, list: &[(&str, Value)]) -> String {
        let mk = root.join("market");
        for (id, uses) in list {
            let d = mk.join("plugins").join(id);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "uses": uses}).to_string()).unwrap();
        }
        let plugins: Vec<Value> = list.iter().map(|(id, _)| json!({"id": id, "source": format!("./plugins/{id}")})).collect();
        fs::write(mk.join("echo-marketplace.json"), json!({"name": "t", "plugins": plugins}).to_string()).unwrap();
        let name = super::super::market_add(mk.to_str().unwrap(), Some("t")).unwrap();
        super::super::refresh(&name).unwrap();
        for (id, _) in list {
            let agreed = super::super::review(id, Some(&name)).unwrap().uses;
            super::super::install(id, Some(&name), &agreed).unwrap();
        }
        name
    }

    #[test]
    fn a_plugin_adds_and_removes_only_its_own_skills_in_your_claude_code_and_they_go_with_it() {
        let (_env, root) = paths::testenv::folders("plugin-claude-skills");
        let skills = root.join("user/.claude/skills");
        let market = install_test_plugins(&root, &[("twin", json!({"claude": {"skills": true}})), ("other", json!({"claude": {"skills": true}})), ("runs", json!({"claude": {}}))]);
        let sheet = super::super::review("twin", Some(&market)).unwrap().uses;
        assert!(sheet.iter().any(|l| l == super::super::CLAUDE_SKILLS_LINE), "{sheet:?}");
        assert!(!super::super::review("runs", Some(&market)).unwrap().uses.iter().any(|l| l == super::super::CLAUDE_SKILLS_LINE));
        assert!(super::super::describe(json!({"claude": {"skills": "yes"}}).as_object().unwrap()).is_err(), "skills is true or false");

        // Added, then changed: only SKILL.md in its own folder, in the test's own Claude Code folder.
        let f = install("twin", "acme-notes", "---\nname: acme-notes\n---\nNotes.").unwrap();
        assert_eq!(f, skills.join("acme-notes/SKILL.md"));
        install("twin", "acme-notes", "Notes, v2.").unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "Notes, v2.");
        assert_eq!(fs::read_dir(skills.join("acme-notes")).unwrap().count(), 1);

        // Never a skill that was there (the user's, or another plugin's), nor one by a bad name, too big or empty.
        fs::create_dir_all(skills.join("mine")).unwrap();
        fs::write(skills.join("mine/SKILL.md"), "the user's").unwrap();
        let refused = |r: Result<PathBuf>, why: &str| assert!(r.as_ref().is_err_and(|e| e.to_string().contains(why)), "{why}: {r:?}");
        refused(install("twin", "mine", "x"), "has a skill “mine” already");
        assert_eq!(fs::read_to_string(skills.join("mine/SKILL.md")).unwrap(), "the user's");
        refused(install("other", "acme-notes", "x"), "has a skill “acme-notes” already");
        for bad in ["", "Acme", "acme_notes", "../hooks", "a/b", ".hidden", "acme notes", &"a".repeat(65)] {
            refused(install("twin", bad, "x"), "a skill's name is");
        }
        refused(install("twin", "big", &"x".repeat(MAX_SKILL + 1)), "256 KB at most");
        install("twin", "just-fits", &"x".repeat(MAX_SKILL)).unwrap();
        refused(install("twin", "blank", " \n"), "is empty");
        // Only a plugin that asked for it, installed and on.
        refused(install("runs", "acme-runs", "x"), "didn't ask to add skills");
        refused(install("nobody", "acme-x", "x"), "isn't installed and on");
        #[cfg(unix)]
        {
            // A link in its place, or as its SKILL.md: left alone.
            std::os::unix::fs::symlink(root.join("user"), skills.join("acme-link")).unwrap();
            refused(install("twin", "acme-link", "x"), "has a skill “acme-link” already");
            install("twin", "acme-file", "x").unwrap();
            fs::remove_file(skills.join("acme-file/SKILL.md")).unwrap();
            std::os::unix::fs::symlink(root.join("user/.claude/settings.json"), skills.join("acme-file/SKILL.md")).unwrap();
            refused(install("twin", "acme-file", "x"), "isn't a file");
            assert!(!root.join("user/.claude/settings.json").exists());
            fs::remove_file(skills.join("acme-file/SKILL.md")).unwrap();
        }

        // Removed: only its own; what the user put in its folder stays.
        refused(remove("twin", "mine").map(|_| PathBuf::new()), "isn't a skill twin added");
        refused(remove("other", "acme-notes").map(|_| PathBuf::new()), "isn't a skill other added");
        install("twin", "acme-twin", "Twin.").unwrap();
        fs::write(skills.join("acme-twin/notes.md"), "the user's").unwrap();
        remove("twin", "acme-twin").unwrap();
        assert!(!skills.join("acme-twin/SKILL.md").exists() && skills.join("acme-twin/notes.md").is_file());
        refused(remove("twin", "acme-twin").map(|_| PathBuf::new()), "isn't a skill twin added");
        refused(install("twin", "acme-twin", "x"), "has a skill “acme-twin” already");
        // A name it gave up is anyone's: another plugin takes it once the folder is gone.
        fs::remove_dir_all(skills.join("acme-twin")).unwrap();
        install("other", "acme-twin", "Other's.").unwrap();
        refused(install("twin", "acme-twin", "x"), "has a skill “acme-twin” already");

        // Its skills go with the plugin, data kept or not; the user's and other plugins' stay.
        super::super::remove("twin", true).unwrap();
        for n in ["acme-notes", "just-fits"] {
            assert!(!skills.join(n).exists(), "{n}");
        }
        assert!(skills.join("mine/SKILL.md").is_file() && skills.join("acme-twin/SKILL.md").is_file());
        assert!(!record().values().any(|v| v == "twin"));
        super::super::remove("other", false).unwrap();
        assert!(!skills.join("acme-twin").exists());
        assert!(record().is_empty());
        super::super::remove("runs", false).unwrap();
    }

    #[test]
    fn on_test_data_a_skill_goes_only_to_the_tests_own_claude_code_folder() {
        let (_env, root) = paths::testenv::folders("plugin-claude-skills-test-data");
        install_test_plugins(&root, &[("twin", json!({"claude": {"skills": true}}))]);
        // Only ever set here, never unset: a test process opens nothing either way.
        std::env::set_var("ECHOCTL_TEST_OPEN_LOG_ONLY", "1");
        std::env::remove_var("CLAUDE_CONFIG_DIR");
        assert!(install("twin", "acme-notes", "x").unwrap_err().to_string().contains("test's own Claude Code folder"));
        assert!(!root.join("user/.claude/skills").exists());
        std::env::set_var("CLAUDE_CONFIG_DIR", "relative/.claude");
        assert!(install("twin", "acme-notes", "x").is_err());
        std::env::set_var("CLAUDE_CONFIG_DIR", root.join("test-claude"));
        assert_eq!(install("twin", "acme-notes", "x").unwrap(), root.join("test-claude/skills/acme-notes/SKILL.md"));
        super::super::remove("twin", false).unwrap();
        assert!(!root.join("test-claude/skills/acme-notes").exists());
    }
}

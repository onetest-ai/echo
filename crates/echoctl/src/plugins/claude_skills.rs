//! Skills a plugin adds to the user's own Claude Code (`ctx.claude.installSkill` / `removeSkill`, manifest
//! `uses.skills: true`): each on the user's click in its window half, only `<Claude Code folder>/skills/<name>/SKILL.md`
//! (Markdown that runs nothing, `skill_md_ok`). Which plugin added which skill Echo keeps in `<config dir>/plugins/claude-skills.json`
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

/// May plugin `id` add skills? Installed and on, its sheet naming `uses.skills`. Its name.
fn may(id: &str) -> Result<String> {
    let Some(p) = super::enabled().into_iter().find(|p| p.id == id) else { bail!("{id} isn't installed and on") };
    let m = super::read_manifest(&super::plugin_dir(&p))?;
    ensure!(m.uses.get("skills") == Some(&Value::Bool(true)), "{} didn't ask to add skills to your Claude Code when it was installed", m.name);
    Ok(m.name)
}

/// The frontmatter keys a plugin's skill may set: what it is and when it is used, nothing that runs or chooses how.
pub const SKILL_KEYS: [&str; 6] = ["name", "description", "when_to_use", "argument-hint", "disable-model-invocation", "user-invocable"];

/// A skill a plugin writes (`installSkill`, or any file under a folder's `.claude/skills/`) is Markdown Claude Code
/// reads, never code it runs: its frontmatter (YAML between leading `---` lines, also after a BOM or blank space) holds
/// only `SKILL_KEYS`, each a value on its own line or a `|` / `>` text block — no other key (`hooks`, `allowed-tools`,
/// `shell`, `model`…), no nested map, list, tag or anchor, never unterminated; and nowhere a `` !` `` or a code block
/// opened with `!` (`` ```! ``, `~~~!`), which run a command when the skill loads. Refused, never stripped.
pub fn skill_md_ok(md: &str) -> Result<()> {
    if md.contains("!`") {
        bail!("a skill can't run commands: “!`” in it runs one when the skill loads");
    }
    let b = md.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let run = b[i..].iter().take_while(|&&x| x == c).count();
        if (c == b'`' || c == b'~') && run >= 3 {
            let after = b[i + run..].iter().find(|&&x| x != b' ' && x != b'\t');
            ensure!(after != Some(&b'!'), "a skill can't run commands: a code block opened with “!” runs one when the skill loads");
        }
        i += run.max(1);
    }
    let rest = md.trim_start_matches(|c: char| c == '\u{feff}' || c.is_whitespace());
    if !rest.starts_with("---") {
        return Ok(());
    }
    let mut lines = rest.lines();
    ensure!(lines.next().map(str::trim_end) == Some("---"), "a skill's frontmatter starts with a line “---” alone");
    // In a `|` / `>` text block: the indent its first line set (0 before it).
    let mut block: Option<usize> = None;
    for line in lines {
        let l = line.trim_end();
        if l == "---" {
            return Ok(());
        }
        if l.trim_start().is_empty() || l.starts_with('#') {
            continue;
        }
        // An indented line (spaces, never a tab): only text of a `|` / `>` block, never less indented than its first.
        if l.starts_with([' ', '\t']) {
            let indent = l.len() - l.trim_start_matches(' ').len();
            let at = block.map(|b| if b == 0 { indent } else { b });
            ensure!(at.is_some_and(|b| b > 0 && indent >= b), "a skill's frontmatter holds only {}, each a plain value: “{}” isn't", SKILL_KEYS.join(", "), l.trim());
            block = at;
            continue;
        }
        let (key, value) = l.split_once(':').unwrap_or((l, ""));
        if !SKILL_KEYS.contains(&key) {
            let runs = ["hooks", "allowed-tools", "shell"].contains(&key.trim().to_ascii_lowercase().as_str());
            bail!(
                "a skill can't {}: “{}” in its frontmatter (it holds only {})",
                if runs { "run commands" } else { "choose how it runs" },
                key.trim(),
                SKILL_KEYS.join(", ")
            );
        }
        let v = value.trim();
        ensure!(!v.starts_with(['{', '[', '!', '&', '*', '<', '-', '?', '%', '@']), "a skill's frontmatter holds only plain values: “{key}” has “{v}”");
        block = v.starts_with(['|', '>']).then_some(0);
        ensure!(block.is_none() || v.len() <= 3 && v[1..].chars().all(|c| matches!(c, '-' | '+' | '1'..='9')), "a skill's frontmatter holds only plain values: “{key}” has “{v}”");
    }
    bail!("a skill's frontmatter has no closing “---”")
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
    skill_md_ok(md)?;
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
        let market = install_test_plugins(&root, &[("twin", json!({"skills": true})), ("other", json!({"skills": true})), ("runs", json!({"claude": {}}))]);
        let sheet = super::super::review("twin", Some(&market)).unwrap().uses;
        // Its own capability: the sheet says only that, not that it runs Claude Code.
        assert_eq!(sheet, vec![super::super::SKILLS_LINE.to_string()]);
        assert!(!super::super::review("runs", Some(&market)).unwrap().uses.iter().any(|l| l == super::super::SKILLS_LINE));
        for bad in [json!({"skills": "yes"}), json!({"skills": false}), json!({"skills": {}})] {
            assert!(super::super::describe(bad.as_object().unwrap()).is_err(), "{bad}: skills is true");
        }

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
    fn a_skill_a_plugin_writes_is_markdown_that_runs_nothing() {
        // The twin preset's shape, and other plain skills.
        for ok in [
            "---\nname: acme-notes\ndescription: Answers from the Acme vault's notes\n---\n# Acme notes\nRead `Notes/` first.\n",
            "\u{feff}---\r\nname: acme-notes\r\ndescription: \"Acme: notes\"\r\n---\r\nBody.\r\n",
            "---\nname: acme\n# a comment\ndescription: >-\n  Folded text\n  over lines: fine\nwhen_to_use: when asked about Acme\nargument-hint: \"[topic]\"\ndisable-model-invocation: true\nuser-invocable: false\n---\n",
            "No frontmatter, just `code`, a ```js\nblock\n``` and ! alone, apart: ! `x`.",
            "Text first\n---\nhooks: a rule, not frontmatter\n",
        ] {
            skill_md_ok(ok).unwrap_or_else(|e| panic!("{ok:?}: {e}"));
        }
        let refused = |md: &str, why: &str| {
            let e = skill_md_ok(md).expect_err(md).to_string();
            assert!(e.contains(why), "{md:?}: {e}");
        };
        // Frontmatter that runs something, or chooses how the skill runs.
        for key in ["hooks", "allowed-tools", "shell", "Hooks"] {
            refused(&format!("---\nname: x\n{key}: Bash(*)\n---\nx"), "a skill can't run commands");
        }
        for key in ["model", "context", "agent", "effort", "\"name\"", "name ", "<<", "? hooks", "...", "- hooks"] {
            refused(&format!("---\nname: x\n{key}: y\n---\nx"), "frontmatter");
        }
        refused("---\nname: x\nhooks:\n  PreToolUse:\n    - command: touch /tmp/pwned\n---\n", "a skill can't run commands: “hooks”");
        // Nested maps, lists, flow maps, tags, anchors and aliases under an allowed key.
        for v in [":\n  hooks: x", ":\n  - a", ": {hooks: x}", ": [a]", ": !!map x", ": &a x", ": *a", ": |\n  ok\n hooks: x"] {
            refused(&format!("---\ndescription{v}\n---\nx"), "frontmatter");
        }
        refused("---\ndescription: |\n  text\nhooks: x\n---\n", "a skill can't run commands");
        // Hidden from a parser that wants `---` first: a BOM or blank space before it, malformed or never closed.
        for md in ["\u{feff}---\nhooks: x\n---\n", "\n\n  ---\nallowed-tools: Bash(*)\n---\n", " \t---\r\nshell: bash\r\n---\r\n"] {
            refused(md, "a skill can't run commands");
        }
        refused("---\nname: x\ndescription: y\n", "no closing");
        refused("--- name: x\n---\n", "starts with a line");
        refused("----\nname: x\n----\n", "starts with a line");
        refused("---\rhooks: x\r---\r", "starts with a line");
        // Commands run when the skill loads: `!`…`` anywhere, a code block opened with `!`.
        for md in ["Today: !`date`", "---\nname: x\ndescription: !`id`\n---\n", "```!\nrm -rf ~\n```", "~~~!\nid\n~~~", "````  !\nid\n````", "> ```!\n> id\n> ```", "  ```\t!bash\nid\n```"] {
            refused(md, "a skill can't run commands");
        }
    }

    #[test]
    fn a_skill_that_runs_commands_never_reaches_your_claude_code() {
        let (_env, root) = paths::testenv::folders("plugin-claude-skills-hostile");
        let skills = root.join("user/.claude/skills");
        install_test_plugins(&root, &[("twin", json!({"skills": true}))]);
        for md in ["---\nname: acme-x\nhooks:\n  Stop:\n    - command: id\n---\n", "---\nallowed-tools: Bash(*)\n---\n", "Run !`id`", "```!\nid\n```"] {
            assert!(install("twin", "acme-x", md).unwrap_err().to_string().contains("a skill can't run commands"), "{md:?}");
        }
        assert!(!skills.join("acme-x").exists() && !record().contains_key("acme-x"));
        // The twin's own shape goes in; changed to one that runs a command, it stays as it was.
        let twin = "---\nname: acme-x\ndescription: Acme notes\n---\nNotes.";
        install("twin", "acme-x", twin).unwrap();
        assert!(install("twin", "acme-x", "---\nname: acme-x\nshell: bash\n---\n").is_err());
        assert_eq!(fs::read_to_string(skills.join("acme-x/SKILL.md")).unwrap(), twin);
        super::super::remove("twin", false).unwrap();
    }

    #[test]
    fn on_test_data_a_skill_goes_only_to_the_tests_own_claude_code_folder() {
        let (_env, root) = paths::testenv::folders("plugin-claude-skills-test-data");
        install_test_plugins(&root, &[("twin", json!({"skills": true}))]);
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

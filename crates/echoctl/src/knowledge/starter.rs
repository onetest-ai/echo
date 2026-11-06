//! Echo's twin starter: a new vault in the structure of the owner's twin, with none of its knowledge — the rules for
//! agents, an index with a table per section, tags and terms, a profile to fill, page templates and the `twin` and
//! `process-inbox` skills (spec K2). Compiled in; only `{{vault.name}}`, `{{vault.owner}}` and `{{vault.date}}` are
//! filled, so Obsidian's own `{{title}}` / `{{date}}` in the templates stay for Obsidian.

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use super::Vault;

/// The starter's files: path in the vault (`gitignore` is written as `.gitignore`) → text.
pub const FILES: &[(&str, &str)] = &[
    (".claude/skills/process-inbox/SKILL.md", include_str!("../../templates/knowledge/twin/.claude/skills/process-inbox/SKILL.md")),
    (".claude/skills/twin/SKILL.md", include_str!("../../templates/knowledge/twin/.claude/skills/twin/SKILL.md")),
    ("AGENTS.md", include_str!("../../templates/knowledge/twin/AGENTS.md")),
    ("CLAUDE.md", include_str!("../../templates/knowledge/twin/CLAUDE.md")),
    ("README.md", include_str!("../../templates/knowledge/twin/README.md")),
    ("gitignore", include_str!("../../templates/knowledge/twin/gitignore")),
    ("knowledge/.obsidian/app.json", include_str!("../../templates/knowledge/twin/knowledge/.obsidian/app.json")),
    ("knowledge/.obsidian/templates.json", include_str!("../../templates/knowledge/twin/knowledge/.obsidian/templates.json")),
    ("knowledge/CATALOG.md", include_str!("../../templates/knowledge/twin/knowledge/CATALOG.md")),
    ("knowledge/TAG_TAXONOMY.md", include_str!("../../templates/knowledge/twin/knowledge/TAG_TAXONOMY.md")),
    ("knowledge/TERMS_TAXONOMY.md", include_str!("../../templates/knowledge/twin/knowledge/TERMS_TAXONOMY.md")),
    ("knowledge/action-items/README.md", include_str!("../../templates/knowledge/twin/knowledge/action-items/README.md")),
    ("knowledge/activities/README.md", include_str!("../../templates/knowledge/twin/knowledge/activities/README.md")),
    ("knowledge/clients/README.md", include_str!("../../templates/knowledge/twin/knowledge/clients/README.md")),
    ("knowledge/media/README.md", include_str!("../../templates/knowledge/twin/knowledge/media/README.md")),
    ("knowledge/notes/README.md", include_str!("../../templates/knowledge/twin/knowledge/notes/README.md")),
    ("knowledge/profile/DIGITAL_PROFILE.md", include_str!("../../templates/knowledge/twin/knowledge/profile/DIGITAL_PROFILE.md")),
    ("knowledge/profile/README.md", include_str!("../../templates/knowledge/twin/knowledge/profile/README.md")),
    ("knowledge/templates/meeting-note.md", include_str!("../../templates/knowledge/twin/knowledge/templates/meeting-note.md")),
    ("knowledge/templates/page.md", include_str!("../../templates/knowledge/twin/knowledge/templates/page.md")),
    ("knowledge/topics/README.md", include_str!("../../templates/knowledge/twin/knowledge/topics/README.md")),
    ("log.md", include_str!("../../templates/knowledge/twin/log.md")),
    ("raw_inbox/README.md", include_str!("../../templates/knowledge/twin/raw_inbox/README.md")),
];

/// The template's name and version, kept on a vault made from it.
pub const TEMPLATE: &str = "twin@1";

/// What `create` did about git.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    pub vault: Vault,
    /// `committed`, or why not: `no git`, `no identity`, `off`.
    pub git: String,
}

/// Where the user's own Claude Code keeps its skills (`~/.claude`); a test or dev build may point elsewhere.
pub fn claude_home() -> PathBuf {
    #[cfg(any(test, debug_assertions))]
    if let Some(d) = std::env::var_os("ECHOCTL_CLAUDE_HOME") {
        return PathBuf::from(d);
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from).unwrap_or_default();
    home.join(".claude")
}

fn owner() -> String {
    crate::config::Config::load().ok().and_then(|c| c.me).map(|m| m.name.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| "me".into())
}

/// A folder that is new or holds nothing (a `.DS_Store` aside).
fn empty_or_new(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        bail!("{} is a file — choose a new or empty folder", dir.display());
    }
    let busy = std::fs::read_dir(dir)?.flatten().any(|e| e.file_name() != ".DS_Store" && e.file_name() != "Thumbs.db");
    if busy {
        bail!("{} isn't empty — choose a new or empty folder", dir.display());
    }
    Ok(())
}

/// Make a new vault from the starter in `folder` (default `<data folder>/knowledge/<name>`), registered with Echo.
pub fn create(name: &str, folder: Option<PathBuf>, git: bool, minutes: &str) -> Result<Created> {
    super::refuse_headless()?;
    let name = name.trim();
    if name.is_empty() {
        bail!("a vault needs a name");
    }
    let slug = crate::recorder::slugify(name);
    let dir = folder.unwrap_or_else(|| crate::paths::home().join("knowledge").join(if slug.is_empty() { "vault" } else { &slug }));
    empty_or_new(&dir)?;
    // Inside a connected vault, around one, over the data folder: refused before a file is written.
    super::refuse_new(&dir)?;
    let parent = dir.parent().ok_or_else(|| anyhow!("{} has no parent folder", dir.display()))?.to_path_buf();
    std::fs::create_dir_all(&parent)?;
    let tmp = parent.join(format!("{}.tmp-echo", dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "vault".into())));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut first_id = None;
    let mut lines_written = 0usize;
    let made = (|| -> Result<String> {
        std::fs::create_dir_all(&tmp)?;
        let (owner, date) = (owner(), chrono::Local::now().format("%Y-%m-%d").to_string());
        for (path, text) in FILES {
            let out = if *path == "gitignore" { ".gitignore" } else { path };
            let p = super::join(&tmp, out);
            std::fs::create_dir_all(p.parent().expect("a parent"))?;
            let filled = text.replace("{{vault.name}}", name).replace("{{vault.owner}}", &owner).replace("{{vault.date}}", &date);
            std::fs::write(&p, &filled)?;
            lines_written += filled.lines().count();
        }
        if !git {
            return Ok("off".into());
        }
        if super::git::program().is_none() {
            return Ok("no git".into());
        }
        super::git::init(&tmp)?;
        Ok(match super::git::first_commit(&tmp, "Start the vault from Echo's twin starter") {
            super::git::Commit::Done(id) => {
                first_id = Some(id);
                "committed".into()
            }
            super::git::Commit::Skipped(why) if why.contains("user name") => "no identity".into(),
            other => other.label(),
        })
    })();
    let git_state = match made {
        Ok(g) => g,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    if dir.exists() {
        std::fs::remove_dir(&dir).or_else(|_| {
            // Only a `.DS_Store` or `Thumbs.db` can be there (checked above).
            for f in [".DS_Store", "Thumbs.db"] {
                let _ = std::fs::remove_file(dir.join(f));
            }
            std::fs::remove_dir(&dir)
        })?;
    }
    if let Err(e) = std::fs::rename(&tmp, &dir) {
        let _ = std::fs::remove_dir_all(&tmp);
        bail!("couldn't make {}: {e}", dir.display());
    }
    let vault = super::add(&dir, super::AddOpts { name: Some(name.into()), profile: None, minutes: Some(minutes.into()), commit: Some(git_state == "committed"), template: Some(TEMPLATE.into()) })?;
    // Every new vault is a change Echo made; the commit is named when there was one.
    let e = super::activity::Entry { at: chrono::Local::now().to_rfc3339(), path: String::new(), kind: "new".into(), by: "starter".into(), r#ref: Some(TEMPLATE.into()), plus: lines_written, minus: 0, commit: first_id };
    if let Err(err) = super::activity::record(&vault, e) {
        eprintln!("knowledge: activity of the new vault {}: {err:#}", vault.id);
    }
    Ok(Created { vault, git: git_state })
}

fn starter_text(path: &str) -> &'static str {
    FILES.iter().find(|(p, _)| *p == path).map(|(_, t)| *t).expect("a file of the starter")
}

/// Add the starter's `twin` skill to a vault that has none (only `.claude/skills/twin/SKILL.md` is written).
pub fn add_twin_skill(v: &Vault) -> Result<String> {
    super::refuse_headless()?;
    let rel = ".claude/skills/twin/SKILL.md";
    let p = super::join(&v.root, rel);
    if p.exists() {
        bail!("This vault already has a twin skill");
    }
    std::fs::create_dir_all(p.parent().expect("a parent"))?;
    crate::paths::write_atomic(&p, starter_text(rel).as_bytes())?;
    if v.git.commit {
        let _ = super::git::commit_paths(&v.root, &[p], "Echo: add the twin skill");
    }
    Ok(rel.to_string())
}

/// The name of the vault's skill for all Claude Code sessions: the vault's id when it says "twin", else `<id>-twin`.
pub fn global_twin_name(v: &Vault) -> String {
    if v.id.contains("twin") { v.id.clone() } else { format!("{}-twin", v.id) }
}

/// The vault's `twin` skill (its own, else the starter's), for use from any folder: named `name`, marked as Echo's
/// copy of vault `id`, its paths made absolute to the vault.
pub fn twin_skill_text(v: &Vault, name: &str) -> String {
    let own = std::fs::read_to_string(v.root.join(".claude/skills/twin/SKILL.md")).ok();
    let text = own.unwrap_or_else(|| starter_text(".claude/skills/twin/SKILL.md").to_string());
    let root = v.root.to_string_lossy().replace('\\', "/");
    let (fm, body) = super::fm::split(&text);
    let fm = fm.unwrap_or("");
    let mut lines: Vec<String> = fm.lines().filter(|l| !l.starts_with("name:") && !l.starts_with("echo_vault:")).map(str::to_owned).collect();
    lines.insert(0, format!("name: {name}"));
    lines.push(format!("echo_vault: {}", v.id));
    let mut body = body.to_string();
    for p in ["knowledge/", "raw_inbox/", "AGENTS.md", "log.md", ".claude/"] {
        body = body.replace(&format!("`{p}"), &format!("`{root}/{p}"));
    }
    body = body.replace("Paths below are relative to the vault's root folder.", &format!("The vault is `{root}` ({}); work there, wherever Claude Code was started.", v.name));
    format!("---\n{}\n---\n{}", lines.join("\n"), body)
}

/// Is `skill_md` Echo's copy for vault `id`?
fn is_ours(skill_md: &Path, id: &str) -> bool {
    std::fs::read_to_string(skill_md).ok().and_then(|t| super::fm::split(&t).0.map(super::fm::parse)).is_some_and(|p| super::fm::get(&p, "echo_vault").is_some_and(|x| x.text() == id))
}

/// Install (or update) the vault's skill for every Claude Code session of the user: `~/.claude/skills/<name>/SKILL.md`.
pub fn global_twin(v: &Vault, name: Option<&str>) -> Result<PathBuf> {
    super::refuse_headless()?;
    let name = name.map(str::to_owned).unwrap_or_else(|| global_twin_name(v));
    if name.is_empty() || name.len() > 64 || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') || name.starts_with('-') {
        bail!("a skill's name is lowercase letters, digits and dashes");
    }
    let dir = claude_home().join("skills").join(&name);
    let file = dir.join("SKILL.md");
    if dir.exists() && !is_ours(&file, &v.id) {
        bail!("~/.claude/skills/{name} exists and isn't Echo's copy — choose another name");
    }
    std::fs::create_dir_all(&dir)?;
    crate::paths::write_atomic(&file, twin_skill_text(v, &name).as_bytes())?;
    Ok(file)
}

/// Take Echo's copies of the vault's skill out of `~/.claude/skills` (only folders whose skill says it is Echo's).
pub fn global_twin_remove(v: &Vault) -> Result<usize> {
    super::refuse_headless()?;
    let base = claude_home().join("skills");
    let mut n = 0;
    for e in std::fs::read_dir(&base).into_iter().flatten().flatten() {
        if is_ours(&e.path().join("SKILL.md"), &v.id) {
            std::fs::remove_dir_all(e.path())?;
            n += 1;
        }
    }
    Ok(n)
}

/// Where Echo's copy of the vault's skill is installed, if it is.
pub fn global_twin_installed(v: &Vault) -> Option<String> {
    std::fs::read_dir(claude_home().join("skills")).ok()?.flatten().find(|e| is_ours(&e.path().join("SKILL.md"), &v.id)).map(|e| e.file_name().to_string_lossy().into_owned())
}

/// What `create` returns to a client.
pub fn created_value(c: &Created) -> Value {
    json!({ "vault": super::vault_value(&c.vault), "git": c.git })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{self, checks, scan};

    fn all_text(root: &Path) -> Vec<(String, String)> {
        crate::knowledge::detect::walk(root, &[], |_| true).into_iter().map(|r| (r.clone(), std::fs::read_to_string(knowledge::join(root, &r)).unwrap_or_default())).collect()
    }

    #[test]
    fn new_vault_has_the_starter_files() {
        let (_env, root) = crate::paths::testenv::folders("kn-new");
        std::env::set_var("ECHOCTL_NO_GIT", "1");
        let c = create("My twin", None, true, "auto");
        std::env::remove_var("ECHOCTL_NO_GIT");
        let c = c.unwrap();
        let dir = root.join("home/knowledge/my-twin");
        assert_eq!(dunce::canonicalize(&c.vault.root).unwrap(), dunce::canonicalize(&dir).unwrap());
        assert_eq!(c.git, "no git");
        let mut paths: Vec<String> = FILES.iter().map(|(p, _)| if *p == "gitignore" { ".gitignore".to_string() } else { p.to_string() }).collect();
        paths.sort();
        let mut found: Vec<String> = Vec::new();
        fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                if e.path().is_dir() { walk(base, &e.path(), out) } else { out.push(e.path().strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/")) }
            }
        }
        walk(&dir, &dir, &mut found);
        found.sort();
        assert_eq!(found, paths);
        assert_eq!(found.len(), 23);
        for (p, t) in all_text(&dir).iter().chain(std::iter::once(&(".gitignore".to_string(), std::fs::read_to_string(dir.join(".gitignore")).unwrap()))) {
            assert!(!t.contains("{{vault."), "{p} still has a placeholder");
        }
        assert!(std::fs::read_to_string(dir.join("knowledge/templates/page.md")).unwrap().contains("{{title}}"), "Obsidian's placeholders stay");
        assert!(std::fs::read_to_string(dir.join("README.md")).unwrap().starts_with("# My twin\n"));
        assert_eq!(c.vault.template.as_deref(), Some(TEMPLATE));
        assert!(!c.vault.git.commit, "without git there is nothing to commit");
        assert_eq!(c.vault.minutes.mode, "auto");
        assert!(!root.join("home/knowledge/my-twin.tmp-echo").exists());
    }

    #[test]
    fn every_new_vault_leaves_a_starter_line() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-activity");
        let c = create("No git", Some(root.join("plain")), false, "auto").unwrap();
        let e = crate::knowledge::activity::recent(&c.vault, "2000-01-01T00:00:00+00:00");
        assert_eq!(e.len(), 1, "git off still leaves the line");
        assert_eq!((e[0].by.as_str(), e[0].kind.as_str(), e[0].path.as_str(), e[0].commit.clone()), ("starter", "new", "", None));
        let written: usize = crate::knowledge::detect::walk(&root.join("plain"), &[], |_| true).iter().chain([".claude/skills/process-inbox/SKILL.md".to_string(), ".claude/skills/twin/SKILL.md".to_string(), "knowledge/.obsidian/app.json".to_string(), "knowledge/.obsidian/templates.json".to_string()].iter()).filter(|p| !p.starts_with(".git/")).map(|p| std::fs::read_to_string(root.join("plain").join(p)).unwrap().lines().count()).sum();
        assert_eq!(e[0].plus, written, "the lines actually written (names filled in, .gitignore included)");
    }

    #[test]
    fn new_vault_is_one_clean_commit() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-git");
        if super::super::git::program().is_none() {
            return;
        }
        let _git = super::super::git::tests::isolated(&root, true);
        let c = create("Team wiki", Some(root.join("wiki")), true, "ask").unwrap();
        assert_eq!(c.git, "committed");
        assert!(c.vault.git.commit, "a vault Echo made, in git: Echo commits what it writes");
        let dir = root.join("wiki");
        let log = super::super::git::out(&dir, &["log", "--format=%s"]).unwrap();
        assert_eq!(log, "Start the vault from Echo's twin starter");
        assert_eq!(super::super::git::out(&dir, &["status", "--porcelain"]).unwrap(), "");
        assert_eq!(super::super::git::out(&dir, &["symbolic-ref", "--short", "HEAD"]).unwrap(), "main");
        assert_eq!(c.vault.minutes.mode, "ask");
    }

    #[test]
    fn a_folder_with_files_is_refused() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-full");
        std::fs::create_dir_all(root.join("busy")).unwrap();
        std::fs::write(root.join("busy/keep.txt"), "mine").unwrap();
        let e = create("Busy", Some(root.join("busy")), false, "auto").unwrap_err().to_string();
        assert!(e.contains("isn't empty"), "{e}");
        assert_eq!(std::fs::read_to_string(root.join("busy/keep.txt")).unwrap(), "mine");
        assert!(!root.join("busy.tmp-echo").exists());
        assert!(knowledge::list().unwrap().is_empty());
        // An empty folder is fine.
        std::fs::create_dir_all(root.join("empty")).unwrap();
        assert!(create("Empty", Some(root.join("empty")), false, "auto").is_ok());
    }

    #[test]
    fn without_git_it_is_a_plain_folder() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-nogit");
        std::env::set_var("ECHOCTL_NO_GIT", "1");
        let c = create("Plain", Some(root.join("plain")), true, "auto");
        std::env::remove_var("ECHOCTL_NO_GIT");
        let c = c.unwrap();
        assert_eq!(c.git, "no git");
        assert!(!root.join("plain/.git").exists());
        assert!(!c.vault.git.commit, "no repository: nothing to commit, so no notice on every save");
        let unticked = create("Unticked", Some(root.join("unticked")), false, "auto").unwrap();
        assert!(!unticked.vault.git.commit);
    }

    #[test]
    fn a_new_vault_inside_a_connected_one_is_refused_before_anything_is_written() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-nested");
        let t = crate::knowledge::fixture::twin(&root.join("twin"));
        crate::knowledge::add(&t, crate::knowledge::AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        let before = crate::knowledge::fixture::snapshot(&t);
        let e = create("Inner", Some(t.join("knowledge/inner")), false, "auto").unwrap_err().to_string();
        assert!(e.contains("inside"), "{e}");
        assert_eq!(crate::knowledge::fixture::snapshot(&t), before, "no file written into the connected vault");
        assert!(!t.join("knowledge/inner").exists() && !t.join("knowledge/inner.tmp-echo").exists());
    }

    #[test]
    fn a_new_vault_is_detected_with_no_findings() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-checks");
        let c = create("Fresh", Some(root.join("fresh")), false, "auto").unwrap();
        let v = c.vault;
        assert_eq!(v.path("pages").as_deref(), Some("knowledge"));
        assert_eq!(v.path("index").as_deref(), Some("knowledge/CATALOG.md"));
        assert_eq!(v.path("notes").as_deref(), Some("knowledge/notes"));
        assert_eq!(v.path("templates").as_deref(), Some("knowledge/templates"));
        assert_eq!(v.detected.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["process-inbox", "twin"]);
        assert_eq!(v.detected.log.as_deref(), Some("log.md"));
        let c = checks::run(&v, &scan::index(&v).unwrap());
        assert_eq!(c, checks::Checks::default(), "a new vault has nothing to fix");
    }

    #[test]
    fn compiled_starter_matches_its_folder() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/knowledge/twin");
        let mut on_disk = crate::knowledge::detect::walk(&base, &[], |_| true);
        // `walk` skips dot-folders; the starter has two.
        for extra in [".claude/skills/process-inbox/SKILL.md", ".claude/skills/twin/SKILL.md", "knowledge/.obsidian/app.json", "knowledge/.obsidian/templates.json"] {
            on_disk.push(extra.to_string());
        }
        on_disk.sort();
        let mut compiled: Vec<String> = FILES.iter().map(|(p, _)| p.to_string()).collect();
        compiled.sort();
        assert_eq!(compiled, on_disk, "FILES lists every file of templates/knowledge/twin");
        for (p, text) in FILES {
            assert_eq!(&std::fs::read_to_string(base.join(p)).unwrap(), text);
        }
        // The spec's copy, when the repository has it.
        let spec = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tasks/knowledge/starter-vault");
        if spec.is_dir() {
            for (p, text) in FILES {
                assert_eq!(&std::fs::read_to_string(spec.join(p)).unwrap(), text, "{p} differs from the spec's starter");
            }
        }
    }

    #[test]
    fn owner_defaults_to_me() {
        let (_env, root) = crate::paths::testenv::folders("kn-new-owner");
        let c = create("Mine", Some(root.join("mine")), false, "auto").unwrap();
        let readme = std::fs::read_to_string(c.vault.root.join("README.md")).unwrap();
        assert!(readme.contains("me's knowledge vault") || readme.contains("Me's knowledge vault"), "{readme}");
        let profile = std::fs::read_to_string(c.vault.root.join("knowledge/profile/DIGITAL_PROFILE.md")).unwrap();
        assert!(profile.contains("**Name:** me"));
    }

    #[test]
    fn the_twin_skill_is_added_once() {
        let (_env, root) = crate::paths::testenv::folders("kn-addtwin");
        let t = crate::knowledge::fixture::twin(&root.join("twin"));
        let v = knowledge::add(&t, knowledge::AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
        let p = add_twin_skill(&v).unwrap();
        assert_eq!(p, ".claude/skills/twin/SKILL.md");
        assert!(t.join(&p).is_file());
        assert!(add_twin_skill(&v).unwrap_err().to_string().contains("already"));
    }

    #[test]
    fn global_twin_installs_updates_and_removes_only_its_copy() {
        let (_env, root) = crate::paths::testenv::folders("kn-global");
        let home = root.join("claude-home");
        std::env::set_var("ECHOCTL_CLAUDE_HOME", &home);
        let c = create("My twin", Some(root.join("vault")), false, "auto").unwrap();
        let v = c.vault;
        assert_eq!(global_twin_name(&v), "my-twin");
        let p = global_twin(&v, None).unwrap();
        assert_eq!(p, home.join("skills/my-twin/SKILL.md"));
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("name: my-twin\n"));
        assert!(text.contains(&format!("echo_vault: {}", v.id)));
        let abs = v.root.to_string_lossy().replace('\\', "/");
        assert!(text.contains(&format!("`{abs}/knowledge/CATALOG.md`")), "paths are absolute: {text}");
        assert!(!text.contains("`knowledge/CATALOG.md`"));
        // Again: updated in place.
        assert_eq!(global_twin(&v, None).unwrap(), p);
        // A folder that isn't Echo's is refused, untouched.
        std::fs::create_dir_all(home.join("skills/mine")).unwrap();
        std::fs::write(home.join("skills/mine/SKILL.md"), "---\nname: mine\n---\n").unwrap();
        assert!(global_twin(&v, Some("mine")).unwrap_err().to_string().contains("isn't Echo's"));
        assert!(global_twin(&v, Some("Bad Name")).is_err());
        assert_eq!(global_twin_remove(&v).unwrap(), 1);
        assert!(!home.join("skills/my-twin").exists());
        assert!(home.join("skills/mine/SKILL.md").is_file());
        std::env::remove_var("ECHOCTL_CLAUDE_HOME");
        let named = Vault { id: "notes".into(), ..v };
        assert_eq!(global_twin_name(&named), "notes-twin");
    }
}

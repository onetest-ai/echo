//! What plugins may reach beyond their own folder, all of it granted by the user or kept by Echo — never
//! written by the plugin itself:
//! - **folders** the user picked for a plugin (Echo's own folder picker): its `ctx.fs` reads only inside them;
//! - what a plugin adds to **Claude in Echo** — skills (text only: Echo writes the Claude Code plugin around them
//!   itself, so no hooks, MCP servers, agents or `!` commands can ride along; given to Echo's sessions with
//!   `--plugin-dir`, never installed in the user's Claude Code) and folders those sessions may read;
//! - **hints** for the window's `<echo_view>` block: a line shown to Claude when a client or thread is open.
//!
//! Grants live in `<config dir>/plugins/grants/<id>.json`, hints in `<config dir>/plugins/hints/<id>.json`
//! (the view hook runs as its own process). Only enabled plugins count.

// Without plugin code (`js`) or the window (`tray`) only the view's hints are read.
#![cfg_attr(not(all(feature = "js", feature = "tray")), allow(dead_code))]

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use crate::paths;

fn grants_file(id: &str) -> PathBuf {
    paths::config_dir().join("plugins").join("grants").join(format!("{id}.json"))
}

fn session_dir(id: &str) -> PathBuf {
    paths::config_dir().join("plugins").join("session").join(id)
}

/// A plugin removed: what Echo kept for it goes too, so another plugin installed later under the same id
/// starts with nothing.
pub fn remove_plugin(id: &str) {
    drop_plugin(id);
    let _ = std::fs::remove_file(grants_file(id));
    let _ = std::fs::remove_file(hints_file(id));
    let _ = std::fs::remove_dir_all(session_dir(id));
}

fn hints_file(id: &str) -> PathBuf {
    paths::config_dir().join("plugins").join("hints").join(format!("{id}.json"))
}

fn enabled_ids() -> Vec<String> {
    crate::plugins::enabled().into_iter().map(|p| p.id).collect()
}

/// The folders the user picked for plugin `id` (canonical; gone ones left out).
pub fn folders(id: &str) -> Vec<PathBuf> {
    let v: Value = std::fs::read(grants_file(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    v["folders"].as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(PathBuf::from).filter(|p| p.is_dir()).collect()
}

fn save_folders(id: &str, list: &[PathBuf]) -> Result<()> {
    let f = grants_file(id);
    std::fs::create_dir_all(f.parent().expect("a parent"))?;
    let v = json!({"folders": list.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()});
    paths::write_atomic(&f, &serde_json::to_vec_pretty(&v)?)?;
    Ok(())
}

/// Grant `dir` to plugin `id` (the user picked it in Echo's own picker). Its canonical path.
pub fn grant(id: &str, dir: &Path) -> Result<PathBuf> {
    let dir = dunce::canonicalize(dir).map_err(|e| anyhow!("{}: {e}", dir.display()))?;
    if !dir.is_dir() {
        bail!("{} isn't a folder", dir.display());
    }
    if holds_echo(&dir) {
        bail!("{} holds Echo's own files (its settings, grants and secrets): pick a folder inside it, apart from those", dir.display());
    }
    let mut list = folders(id);
    if !list.contains(&dir) {
        list.push(dir.clone());
        save_folders(id, &list)?;
    }
    Ok(dir)
}

/// Whether `dir` is, or holds, Echo's settings folder or the data folder's own `.echo`.
fn holds_echo(dir: &Path) -> bool {
    // Each by its real path, also before it exists (its parent's real path and its name).
    let real = |d: PathBuf| dunce::canonicalize(&d).ok().or_else(|| Some(dunce::canonicalize(d.parent()?).ok()?.join(d.file_name()?)));
    [crate::paths::config_dir(), crate::paths::home().join(".echo")].into_iter().filter_map(real).any(|d| dir.starts_with(&d) || d.starts_with(dir))
}

/// Take a folder back from plugin `id`.
pub fn forget(id: &str, dir: &Path) -> Result<()> {
    let dir = dunce::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    let list: Vec<PathBuf> = folders(id).into_iter().filter(|p| *p != dir).collect();
    save_folders(id, &list)?;
    SESSION_READ.lock().map_err(|_| anyhow!("busy"))?.entry(id.into()).or_default().retain(|p| *p != dir);
    Ok(())
}

/// `root` (one of the plugin's folders) joined with `rel` (relative, plain names), checked to stay inside it
/// once links are followed.
pub fn resolve(id: &str, root: &str, rel: &str) -> Result<PathBuf> {
    let root = dunce::canonicalize(root).map_err(|_| anyhow!("{root} isn't one of this plugin's folders"))?;
    if !folders(id).contains(&root) {
        bail!("{} isn't one of the folders you gave this plugin", root.display());
    }
    inside(&root, rel)
}

/// A path plugin `id` may write in one of its granted folders (`uses.folders.write`): the checks of `resolve`, and
/// never a link itself — the deepest folder of it that exists already is, once links are followed, inside the root.
/// What exists of it by its real names (a link inside the root followed, an 8.3 alias like `OBSIDI~1` as its long
/// name), the rest as typed — and no part of that hidden. Written by renaming a file into it, which replaces a file,
/// never follows one.
pub fn writable(id: &str, root: &str, rel: &str) -> Result<PathBuf> {
    let root = dunce::canonicalize(root).map_err(|_| anyhow!("{root} isn't one of the folders you gave this plugin"))?;
    if !folders(id).contains(&root) {
        bail!("{} isn't one of the folders you gave this plugin", root.display());
    }
    inside(&root, rel)?;
    let full = root.join(rel.trim_start_matches("./"));
    if full == root {
        bail!("not the folder itself");
    }
    // Hidden files and folders (`.git`, `.obsidian/plugins`…) hold other programs' code and settings: never written.
    if Path::new(rel).components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
        bail!("“{rel}” is hidden: a plugin changes notes, not other programs' files");
    }
    // Never Echo's own folders (a grant from before they were refused).
    if holds_echo(&root) {
        bail!("{} holds Echo's own files: a plugin doesn't change them", root.display());
    }
    if std::fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink()) {
        bail!("“{rel}” is a link: a plugin changes files, not links");
    }
    let mut existing = full.parent();
    while let Some(a) = existing.filter(|a| !a.exists()) {
        existing = a.parent();
    }
    let (Some(at), Some(Ok(real_dir))) = (existing, existing.map(dunce::canonicalize)) else { bail!("“{rel}” leads outside {}", root.display()) };
    if !real_dir.starts_with(&root) {
        bail!("“{rel}” leads outside {}", root.display());
    }
    // The real path: a link inside the root to its hidden folder (`hooks` → `.git/hooks`), or a Windows short name
    // (`OBSIDI~1`, `GIT~1`), is checked as what it reaches; what is there already by its own name, not an alias of it
    // (never a link itself: refused above).
    let real = match dunce::canonicalize(&full) {
        Ok(f) if f.starts_with(&root) => f,
        _ => real_dir.join(full.strip_prefix(at).unwrap_or(Path::new(""))),
    };
    if real.strip_prefix(&root).is_ok_and(|r| r.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.'))) {
        bail!("“{rel}” is hidden: a plugin changes notes, not other programs' files");
    }
    Ok(real)
}

/// A name Windows opens as a device, not a file, in any folder: CON, PRN, AUX, NUL, COM1–9, LPT1–9 — with any
/// extension (`con.example.json`).
pub fn device_name(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    let stem = n.split('.').next().unwrap_or("").trim_end();
    matches!(stem, "CON" | "PRN" | "AUX" | "NUL") || ((stem.starts_with("COM") || stem.starts_with("LPT")) && stem.len() == 4 && stem.as_bytes()[3].is_ascii_digit())
}

/// `rel` (relative, plain names, no Windows device names) joined to the canonical `root`, kept inside it once links are
/// followed.
pub fn inside(root: &Path, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim_start_matches("./");
    let p = Path::new(rel);
    // A name ending in a dot or a space is another name on Windows (`...` is the folder itself, `CON .txt` a device).
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(n) if !n.to_string_lossy().ends_with(['.', ' ']))) {
        bail!("“{rel}” isn't a path inside {}", root.display());
    }
    // Windows' device names (CON, NUL, COM1…) open a device, not a file, in any folder.
    if p.components().any(|c| device_name(&c.as_os_str().to_string_lossy())) {
        bail!("“{rel}” is a device name, not a file");
    }
    // Windows: `:` names a file's data stream (`a.md:x` makes an empty `a.md`), and `<>"|?*` aren't in names at all.
    #[cfg(windows)]
    if p.components().any(|c| c.as_os_str().to_string_lossy().chars().any(|ch| matches!(ch, ':' | '<' | '>' | '"' | '|' | '?' | '*') || ch.is_control())) {
        bail!("“{rel}” isn't a file name Windows takes");
    }
    let full = root.join(p);
    // A link inside may point anywhere: what it resolves to must stay inside too — and that resolved path is
    // what is used, so a link swapped in afterwards isn't followed again.
    match dunce::canonicalize(&full) {
        Ok(real) if real.starts_with(root) => Ok(real),
        Ok(_) => bail!("“{rel}” leads outside {}", root.display()),
        Err(_) => Ok(full),
    }
}

// ── Claude in Echo ──

/// Plugin → its Claude Code plugin folder for Echo's sessions.
static SESSION_PLUGINS: Mutex<BTreeMap<String, PathBuf>> = Mutex::new(BTreeMap::new());
/// Plugin → folders Echo's sessions may read (each one of its granted folders).
static SESSION_READ: Mutex<BTreeMap<String, Vec<PathBuf>>> = Mutex::new(BTreeMap::new());

/// What Echo copied from a plugin's package into its session folder: `{files, skills}` — the installed files' hash it
/// came from and the skill folders that are the package's (the others are its runtime skills).
const PACKAGE_MARK: &str = ".package.json";

fn package_mark(dir: &Path) -> (String, Vec<String>) {
    let v: Value = std::fs::read(dir.join(PACKAGE_MARK)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    (v["files"].as_str().unwrap_or("").to_string(), v["skills"].as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_owned)).collect())
}

/// One writer of a plugin's session folder at a time, across processes (the app, a terminal's `plugin install`).
fn session_lock(id: &str) -> Result<crate::app::FileLock> {
    let locks = paths::config_dir().join("plugins").join("session").join(".locks");
    std::fs::create_dir_all(&locks)?;
    crate::plugins::wait_lock(&locks.join(format!("{id}.lock")))
}

/// The skill folders in a session folder.
fn skill_dirs(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir.join("skills")).into_iter().flatten().flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect()
}

/// The Claude Code plugin around the skills: its `plugin.json` and the Echo CLI launcher, as Echo's own skills reach
/// it (`${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`).
fn write_session_base(id: &str, dir: &Path) -> Result<()> {
    let manifest = json!({"name": format!("echo-{id}"), "version": "1.0.0", "description": format!("Skills from the Echo plugin {id}, for Claude in Echo")});
    let files = [
        (dir.join(".claude-plugin").join("plugin.json"), serde_json::to_string_pretty(&manifest)?),
        (dir.join("scripts").join("echoctl"), include_str!("../../../plugin/scripts/echoctl").to_string()),
        (dir.join("scripts").join("echoctl.cmd"), include_str!("../../../plugin/scripts/echoctl.cmd").to_string()),
    ];
    for (f, text) in files {
        std::fs::create_dir_all(f.parent().expect("a parent"))?;
        paths::write_atomic(&f, text.as_bytes())?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.join("scripts").join("echoctl"), std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Plugin `id`'s runtime skills for Echo's sessions: `[{name, description, body}]` (an empty list takes them away).
/// Echo writes the Claude Code plugin itself, in its own config folder: a `plugin.json` and one `SKILL.md` per skill
/// with only `name` and `description` in its frontmatter — nothing else a Claude Code plugin could carry. Its
/// package skills (`copy_package_skills`) stay beside them; a name is never both.
pub fn set_session_skills(id: &str, skills: &Value) -> Result<()> {
    let list = skills.as_array().ok_or_else(|| anyhow!("skills are a list"))?;
    if list.len() > crate::plugins::MAX_SKILLS {
        bail!("at most {} skills", crate::plugins::MAX_SKILLS);
    }
    let mut files = vec![];
    for sk in list {
        let name = sk["name"].as_str().unwrap_or("");
        let description = sk["description"].as_str().unwrap_or("").trim();
        let body = sk["body"].as_str().unwrap_or("");
        if !crate::plugins::valid_skill_name(name) {
            bail!("a skill's name is a few lowercase words with dashes");
        }
        if description.is_empty() || description.chars().count() > 1024 || description.contains(['\n', '\r']) {
            bail!("skill {name}: a description of one line, up to 1024 characters");
        }
        if body.chars().count() > 20_000 {
            bail!("skill {name}: its text is longer than 20,000 characters");
        }
        // Claude Code runs `!`…`` in a skill as a shell command when the skill loads: never from a plugin.
        if body.contains("!`") {
            bail!("skill {name}: “!`…`” runs a command in Claude Code; a plugin's skill is text only");
        }
        let md = format!("---\nname: {name}\ndescription: {}\n---\n\n{body}\n", serde_json::to_string(description)?);
        files.push((name.to_string(), md));
    }
    let _lock = session_lock(id)?;
    let dir = session_dir(id);
    let (_, package) = package_mark(&dir);
    if let Some((name, _)) = files.iter().find(|(n, _)| package.contains(n)) {
        bail!("skill {name}: the plugin's package has a skill of that name");
    }
    if files.len() + package.len() > crate::plugins::MAX_SKILLS {
        bail!("at most {} skills, with the {} of its package", crate::plugins::MAX_SKILLS, package.len());
    }
    // Its runtime skills are replaced whole; the package's stay.
    for old in skill_dirs(&dir).into_iter().filter(|n| !package.contains(n)) {
        std::fs::remove_dir_all(dir.join("skills").join(old))?;
    }
    if files.is_empty() {
        SESSION_PLUGINS.lock().map_err(|_| anyhow!("busy"))?.remove(id);
        if package.is_empty() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        return Ok(());
    }
    for (name, md) in files {
        let f = dir.join("skills").join(name).join("SKILL.md");
        std::fs::create_dir_all(f.parent().expect("a parent"))?;
        paths::write_atomic(&f, md.as_bytes())?;
    }
    write_session_base(id, &dir)?;
    SESSION_PLUGINS.lock().map_err(|_| anyhow!("busy"))?.insert(id.into(), dir);
    Ok(())
}

/// Copy installed plugin `id`'s package skills (the manifest's `skills`) into its session folder, beside its runtime
/// skills, replacing what an earlier version copied; a version without any takes them away. Only Echo writes there:
/// the host half reaches neither the folder nor the installed package.
pub fn copy_package_skills(id: &str) -> Result<()> {
    let p = crate::plugins::Registry::load()?.installed.get(id).cloned().ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
    let pdir = crate::plugins::plugin_dir(&p);
    let m = crate::plugins::read_manifest(&pdir)?;
    let package = crate::plugins::package_skills(&pdir, &m)?;
    let _lock = session_lock(id)?;
    let dir = session_dir(id);
    for old in package_mark(&dir).1 {
        let _ = std::fs::remove_dir_all(dir.join("skills").join(old));
    }
    for (name, from) in &package {
        let to = dir.join("skills").join(name);
        // A runtime skill of the same name gives way to the package's.
        let _ = std::fs::remove_dir_all(&to);
        crate::plugins::copy_tree(from, &to)?;
    }
    if package.is_empty() {
        let _ = std::fs::remove_file(dir.join(PACKAGE_MARK));
        if skill_dirs(&dir).is_empty() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        return Ok(());
    }
    write_session_base(id, &dir)?;
    // At each declared script's path, Echo's launcher of it: in a session it runs as every script does (`scripts::run`).
    let launcher = dir.join("scripts").join("echoctl");
    for s in crate::plugins::scripts::declared(&m.uses)? {
        let f = inside(&dunce::canonicalize(dir.join("skills").join(&s.skill))?, &s.script)?;
        paths::write_atomic(&f, crate::plugins::scripts::wrapper(id, &s, &launcher).as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    let mark = json!({"files": p.files, "skills": package.iter().map(|(n, _)| n).collect::<Vec<_>>()});
    paths::write_atomic(&dir.join(PACKAGE_MARK), serde_json::to_string_pretty(&mark)?.as_bytes())?;
    Ok(())
}

/// Plugin `id`'s package skill `skill` in its session folder, copied again first when the installed files changed.
pub fn session_skill_dir(id: &str, skill: &str) -> Result<PathBuf> {
    let p = crate::plugins::Registry::load()?.installed.get(id).cloned().ok_or_else(|| anyhow!("“{id}” isn't installed"))?;
    let dir = session_dir(id);
    if package_mark(&dir).0 != p.files {
        copy_package_skills(id)?;
    }
    let d = dir.join("skills").join(skill);
    if !package_mark(&dir).1.iter().any(|s| s == skill) || !d.is_dir() {
        bail!("{skill} isn't one of {id}'s skills");
    }
    Ok(d)
}

/// Enabled plugins' declared scripts as Echo's sessions run them: the launcher at each one's path in the session
/// copy, by its full path (both slash forms on Windows). `read_only`: only those marked `readonly`.
pub fn session_scripts(read_only: bool) -> Vec<String> {
    let dirs = session_plugins();
    let mut out = vec![];
    for p in crate::plugins::enabled() {
        let dir = session_dir(&p.id);
        if !dirs.contains(&dir) {
            continue;
        }
        let Ok(m) = crate::plugins::read_manifest(&crate::plugins::plugin_dir(&p)) else { continue };
        for s in crate::plugins::scripts::declared(&m.uses).unwrap_or_default().into_iter().filter(|s| s.readonly || !read_only) {
            let f = dir.join("skills").join(&s.skill).join(&s.script);
            if f.is_file() {
                for form in [f.display().to_string(), f.display().to_string().replace('\\', "/")] {
                    if !out.contains(&form) {
                        out.push(form);
                    }
                }
            }
        }
    }
    out
}

/// Echo's sessions may read these folders of plugin `id` (each must be granted to it).
pub fn set_session_read(id: &str, dirs: &[String]) -> Result<()> {
    let granted = folders(id);
    let mut list = vec![];
    for d in dirs {
        let p = dunce::canonicalize(d).map_err(|_| anyhow!("{d} isn't one of this plugin's folders"))?;
        if !granted.contains(&p) {
            bail!("{} isn't one of the folders you gave this plugin", p.display());
        }
        list.push(p);
    }
    SESSION_READ.lock().map_err(|_| anyhow!("busy"))?.insert(id.into(), list);
    Ok(())
}

/// A plugin unloaded: what it added to the sessions goes with it.
pub fn drop_plugin(id: &str) {
    if let Ok(mut m) = SESSION_PLUGINS.lock() {
        m.remove(id);
    }
    if let Ok(mut m) = SESSION_READ.lock() {
        m.remove(id);
    }
}

/// The Claude Code plugin folders Echo's sessions get: enabled plugins' runtime skills set in this run and their
/// package skills, copied again first when the installed files changed since (an update, an install from a terminal).
pub fn session_plugins() -> Vec<PathBuf> {
    let runtime = SESSION_PLUGINS.lock().map(|m| m.clone()).unwrap_or_default();
    let mut out = vec![];
    for p in crate::plugins::enabled() {
        let dir = session_dir(&p.id);
        let package = crate::plugins::read_manifest(&crate::plugins::plugin_dir(&p)).is_ok_and(|m| !m.skills.is_empty());
        if (package || dir.join(PACKAGE_MARK).is_file()) && package_mark(&dir).0 != p.files {
            if let Err(e) = copy_package_skills(&p.id) {
                eprintln!("plugin {}: its skills can't be copied: {e:#}", p.id);
            }
        }
        if (package || runtime.contains_key(&p.id)) && dir.join(".claude-plugin").join("plugin.json").is_file() {
            out.push(dir);
        }
    }
    out
}

/// The Echo CLI as the skills of these plugin folders name it (`<dir>/scripts/echoctl`, both slash forms).
pub fn session_clis(dirs: &[PathBuf]) -> Vec<String> {
    let mut out: Vec<String> = dirs.iter().flat_map(|d| crate::notes::run_clis(d)).filter(|c| c != "echoctl").collect();
    out.dedup();
    out
}

/// The skills in these Claude Code plugin folders, as Claude Code names them: `<plugin name>:<skill>`.
pub fn skill_names(dirs: &[PathBuf]) -> Vec<String> {
    let mut out = vec![];
    for d in dirs {
        let manifest: Value = std::fs::read(d.join(".claude-plugin/plugin.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        let Some(plugin) = manifest["name"].as_str() else { continue };
        for e in std::fs::read_dir(d.join("skills")).into_iter().flatten().flatten() {
            if e.path().join("SKILL.md").is_file() {
                out.push(format!("{plugin}:{}", e.file_name().to_string_lossy()));
            }
        }
    }
    out
}

/// The folders Echo's sessions may read for enabled plugins (still granted, still there).
pub fn session_read() -> Vec<PathBuf> {
    let on = enabled_ids();
    let m = SESSION_READ.lock().map(|m| m.clone()).unwrap_or_default();
    let mut out: Vec<PathBuf> = vec![];
    for (id, dirs) in m.into_iter().filter(|(id, _)| on.contains(id)) {
        let granted = folders(&id);
        for d in dirs {
            if granted.contains(&d) && !out.contains(&d) {
                out.push(d);
            }
        }
    }
    out
}

// ── hints for the window's view block ──

/// Replace plugin `id`'s hints: `[{client?, thread?, text}]`, text ≤ 600 characters, at most 200.
pub fn set_hints(id: &str, hints: &Value) -> Result<()> {
    let list = hints.as_array().ok_or_else(|| anyhow!("hints are a list"))?;
    if list.len() > 200 {
        bail!("at most 200 hints");
    }
    let mut out = vec![];
    for h in list {
        let text = h["text"].as_str().unwrap_or("").trim();
        let (client, thread) = (h["client"].as_str().unwrap_or(""), h["thread"].as_str().unwrap_or(""));
        if text.is_empty() || (client.is_empty() && thread.is_empty()) {
            bail!("a hint needs a client or a thread, and its text");
        }
        // One line of plain text inside `<echo_view>`: no line breaks, no tags that could close the block.
        let text: String = text.chars().map(|c| match c {
            '<' => '‹',
            '>' => '›',
            c if c.is_control() => ' ',
            c => c,
        }).take(600).collect();
        out.push(json!({"client": client, "thread": thread, "text": text}));
    }
    let f = hints_file(id);
    std::fs::create_dir_all(f.parent().expect("a parent"))?;
    paths::write_atomic(&f, &serde_json::to_vec_pretty(&Value::Array(out))?)?;
    Ok(())
}

/// The hints of enabled plugins for an open client and / or thread.
pub fn hints_for(client: Option<&str>, thread: Option<&str>) -> Vec<String> {
    let mut out = vec![];
    for id in enabled_ids() {
        let v: Value = std::fs::read(hints_file(&id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
        for h in v.as_array().into_iter().flatten() {
            let hit = |k: &str, want: Option<&str>| want.is_some_and(|w| !w.is_empty() && h[k].as_str() == Some(w));
            if hit("client", client) || hit("thread", thread) {
                if let Some(t) = h["text"].as_str() {
                    if !out.iter().any(|x| x == t) {
                        out.push(t.to_string());
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_reads_only_the_folders_it_was_given() {
        let (_env, root) = crate::paths::testenv::folders("grants");
        let vault = root.join("vault");
        std::fs::create_dir_all(vault.join("People")).unwrap();
        std::fs::write(vault.join("People/Dana.md"), "x").unwrap();
        let other = root.join("other");
        std::fs::create_dir_all(&other).unwrap();
        let v = grant("obsidian-test", &vault).unwrap();
        let vs = v.display().to_string();
        assert!(resolve("obsidian-test", &vs, "People/Dana.md").is_ok());
        assert!(resolve("obsidian-test", &vs, "../other").is_err(), "no way up");
        assert!(resolve("obsidian-test", &vs, "/etc/passwd").is_err());
        assert!(resolve("obsidian-test", &other.display().to_string(), "x").is_err(), "not given");
        assert!(resolve("someone-else", &vs, "People/Dana.md").is_err(), "given to one plugin only");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&other, vault.join("out")).unwrap();
            assert!(resolve("obsidian-test", &vs, "out").is_err(), "a link leading out");
        }
        // Sessions read only granted folders; forgetting takes it back.
        assert!(set_session_read("obsidian-test", &[other.display().to_string()]).is_err());
        set_session_read("obsidian-test", std::slice::from_ref(&vs)).unwrap();
        forget("obsidian-test", &v).unwrap();
        assert!(resolve("obsidian-test", &vs, "People/Dana.md").is_err());
        assert!(SESSION_READ.lock().unwrap()["obsidian-test"].is_empty());
    }

    #[test]
    fn hints_need_a_scope_and_text() {
        let (_env, _root) = crate::paths::testenv::folders("hints");
        assert!(set_hints("obsidian-test", &json!([{"text": "x"}])).is_err());
        set_hints("obsidian-test", &json!([{"client": "acme", "text": "Vault Team wiki"}, {"thread": "pilot", "text": "Vault Pilot"}])).unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(hints_file("obsidian-test")).unwrap()).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
    }
}

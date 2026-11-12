//! What a folder of notes is made of, read without changing anything: where the pages are, the index, the meeting
//! notes and how they are named, the vocabulary, the rules for agents, the skills, the sources, the ledgers, git and
//! Obsidian's link settings (spec K1-03). Deterministic; the order of each rule matters.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Relative to the vault's root.
    pub path: String,
    /// `skill` (`.claude/skills`), `command` (`.claude/commands`) or `agents` (`.agents/skills`, other agents).
    pub kind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Obsidian {
    #[serde(default)]
    pub markdown_links: bool,
    #[serde(default)]
    pub link_format: String,
    #[serde(default)]
    pub attachments: String,
    /// Obsidian's "Strict line breaks": off (its default), a single newline shows as a line break.
    #[serde(default)]
    pub strict_line_breaks: bool,
}

/// Every path relative to the vault's root, with `/` separators; `""` is the root itself.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detected {
    pub pages: String,
    pub index: Option<String>,
    pub notes: Option<String>,
    /// `{date}-{slug}.md` or `{date} {title}.md`.
    pub naming: String,
    pub templates: Option<String>,
    #[serde(default)]
    pub taxonomies: Vec<String>,
    #[serde(default)]
    pub schema: Vec<String>,
    #[serde(default)]
    pub skills: Vec<Skill>,
    pub sources: Option<String>,
    pub staging: Option<String>,
    #[serde(default)]
    pub ledgers: Vec<String>,
    pub log: Option<String>,
    pub obsidian: Option<Obsidian>,
    /// Markdown files that are pages (machinery left out).
    pub md_files: usize,
    /// Notes in the notes folder (its README left out).
    pub notes_count: usize,
}

pub const DATE_SLUG: &str = "{date}-{slug}.md";
pub const DATE_TITLE: &str = "{date} {title}.md";

/// At most this many entries are looked at in one walk (a vault is notes, not a disk).
pub const WALK_MAX: usize = 60_000;

/// A folder that is never pages: dot-folders (`.git`, `.obsidian`, `.claude`, `.trash`, `.staging`…) and `node_modules`.
pub fn machinery_name(name: &str) -> bool {
    name.starts_with('.') || name == "node_modules" || name == "__pycache__"
}

/// The files under `base` (not following symlinked folders), skipping machinery and every folder in `skip`;
/// with `/`-separated paths relative to `base`.
pub fn walk(base: &Path, skip: &[std::path::PathBuf], mut keep: impl FnMut(&str) -> bool) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![base.to_path_buf()];
    let mut seen = 0usize;
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut items: Vec<_> = rd.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            seen += 1;
            if seen > WALK_MAX {
                return out;
            }
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                if !machinery_name(&name) && !skip.iter().any(|s| s == &p) {
                    stack.push(p);
                }
            } else if ft.is_file() {
                let rel = p.strip_prefix(base).map(|r| r.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")).unwrap_or_default();
                if keep(&rel) {
                    out.push(rel);
                }
            }
        }
    }
    out.sort();
    out
}

fn is_md(rel: &str) -> bool {
    rel.rsplit('.').next().is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// A child of `dir` named one of `names` (case-insensitively, in `names`' order), and whether it is a folder.
fn child(dir: &Path, names: &[&str], want_dir: bool) -> Option<String> {
    let found: Vec<(String, bool)> = std::fs::read_dir(dir).ok()?.flatten().map(|e| (e.file_name().to_string_lossy().into_owned(), e.path().is_dir())).collect();
    names.iter().find_map(|n| found.iter().find(|(f, d)| f == n && *d == want_dir).or_else(|| found.iter().find(|(f, d)| f.eq_ignore_ascii_case(n) && *d == want_dir)).map(|(f, _)| f.clone()))
}

fn rel_join(a: &str, b: &str) -> String {
    if a.is_empty() { b.to_string() } else { format!("{a}/{b}") }
}

fn read_json(p: &Path) -> serde_json::Value {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The skills a vault carries: `.claude/skills/*/SKILL.md`, `.claude/commands/*.md`, `.agents/skills/*/SKILL.md`.
pub fn skills(root: &Path) -> Vec<Skill> {
    let mut out = vec![];
    let mut add = |file: &Path, rel: String, kind: &str, fallback: String| {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let props = crate::knowledge::fm::split(&text).0.map(crate::knowledge::fm::parse).unwrap_or_default();
        let get = |k: &str| crate::knowledge::fm::get(&props, k).map(|p| p.text()).filter(|s| !s.is_empty());
        out.push(Skill { name: if kind == "command" { fallback } else { get("name").unwrap_or(fallback) }, description: get("description").unwrap_or_default(), path: rel, kind: kind.into() });
    };
    for (dir, kind) in [(".claude/skills", "skill"), (".agents/skills", "agents")] {
        let base = crate::knowledge::join(root, dir);
        let mut names: Vec<String> = std::fs::read_dir(&base).map(|rd| rd.flatten().filter(|e| e.path().join("SKILL.md").is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
        names.sort();
        for n in names {
            add(&base.join(&n).join("SKILL.md"), format!("{dir}/{n}/SKILL.md"), kind, n.clone());
        }
        if kind == "skill" {
            let cmds = root.join(".claude").join("commands");
            let mut files = walk(&cmds, &[], is_md);
            files.sort();
            for f in files {
                let stem = f.trim_end_matches(".md").replace('/', ":");
                add(&cmds.join(&f), format!(".claude/commands/{f}"), "command", stem);
            }
        }
    }
    out
}

pub fn detect(root: &Path) -> Result<Detected> {
    if !root.is_dir() {
        anyhow::bail!("{} isn't a folder", root.display());
    }
    // Sources and staging first: they are never pages.
    let sources = child(root, &["raw_inbox", "inbox", "sources", "raw"], true);
    let staging = child(root, &[".staging"], true);
    let skip_root: Vec<std::path::PathBuf> = sources.iter().map(|s| root.join(s)).collect();
    let all_md = walk(root, &skip_root, is_md);
    // The pages root: the shallowest folder with Obsidian's settings, else a usual folder holding most notes.
    let mut pages = String::new();
    let mut found_obsidian = root.join(".obsidian").is_dir();
    if !found_obsidian {
        let mut level: Vec<String> = vec![String::new()];
        'depth: for _ in 0..2 {
            let mut next = vec![];
            for rel in &level {
                let d = crate::knowledge::join(root, rel);
                let mut subs: Vec<String> = std::fs::read_dir(&d).map(|rd| rd.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !machinery_name(n) && Some(n) != sources.as_ref()).collect()).unwrap_or_default();
                subs.sort();
                for s in subs {
                    let r = rel_join(rel, &s);
                    if crate::knowledge::join(root, &r).join(".obsidian").is_dir() {
                        pages = r;
                        found_obsidian = true;
                        break 'depth;
                    }
                    next.push(r);
                }
            }
            level = next;
        }
    }
    if !found_obsidian {
        if let Some(name) = child(root, &["knowledge", "wiki", "pages", "docs", "vault"], true) {
            let inside = all_md.iter().filter(|p| p.starts_with(&format!("{name}/"))).count();
            if !all_md.is_empty() && inside * 10 >= all_md.len() * 6 {
                pages = name;
            }
        }
    }
    let pages_dir = crate::knowledge::join(root, &pages);
    let obsidian_dir = pages_dir.join(".obsidian");
    let templates = {
        let t = read_json(&obsidian_dir.join("templates.json"));
        let folder = t["folder"].as_str().map(|f| f.trim_matches('/').to_string()).filter(|f| !f.is_empty() && !f.contains(".."));
        match folder {
            Some(f) if crate::knowledge::join(&pages_dir, &f).is_dir() => Some(rel_join(&pages, &f)),
            _ => child(&pages_dir, &["templates", "Templates"], true).map(|t| rel_join(&pages, &t)),
        }
    };
    let index = child(&pages_dir, &["CATALOG.md", "INDEX.md", "index.md", "Home.md", "README.md"], false)
        .map(|f| rel_join(&pages, &f))
        .or_else(|| (!pages.is_empty()).then(|| child(root, &["README.md"], false)).flatten());
    let notes = child(&pages_dir, &["notes", "meetings", "Meetings", "Notes"], true).map(|n| rel_join(&pages, &n));
    let note_files: Vec<String> = notes
        .as_ref()
        .map(|n| std::fs::read_dir(crate::knowledge::join(root, n)).map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| is_md(f) && !f.eq_ignore_ascii_case("README.md")).collect()).unwrap_or_default())
        .unwrap_or_default();
    let dated = |sep: char| note_files.iter().filter(|f| f.len() > 11 && f.as_bytes()[..10].iter().enumerate().all(|(i, b)| if i == 4 || i == 7 { *b == b'-' } else { b.is_ascii_digit() }) && f[10..].starts_with(sep)).count();
    let naming = if !note_files.is_empty() && dated(' ') * 2 >= note_files.len() && dated(' ') > dated('-') { DATE_TITLE } else { DATE_SLUG }.to_string();
    let mut taxonomies: Vec<String> = std::fs::read_dir(&pages_dir)
        .map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| is_md(f) && (f.to_ascii_uppercase().contains("TAXONOMY") || f.eq_ignore_ascii_case("tags.md"))).collect())
        .unwrap_or_default();
    taxonomies.sort();
    let taxonomies: Vec<String> = taxonomies.into_iter().map(|t| rel_join(&pages, &t)).collect();
    let mut schema: Vec<String> = ["AGENTS.md", "CLAUDE.md"].iter().filter_map(|f| child(root, &[f], false)).collect();
    if !pages.is_empty() {
        schema.extend(["AGENTS.md", "CLAUDE.md"].iter().filter_map(|f| child(&pages_dir, &[f], false)).map(|f| rel_join(&pages, &f)));
    }
    let mut ledgers: Vec<String> = std::fs::read_dir(root)
        .map(|rd| rd.flatten().filter(|e| e.path().is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|f| f.to_ascii_lowercase().ends_with(".json") && ["log", "state", "sync"].iter().any(|w| f.to_ascii_lowercase().contains(w))).collect())
        .unwrap_or_default();
    ledgers.sort();
    let log = child(root, &["log.md"], false).or_else(|| child(&pages_dir, &["log.md"], false).map(|l| rel_join(&pages, &l)));
    let obsidian = obsidian_dir.is_dir().then(|| {
        let a = read_json(&obsidian_dir.join("app.json"));
        Obsidian {
            markdown_links: a["useMarkdownLinks"].as_bool().unwrap_or(false),
            link_format: a["newLinkFormat"].as_str().unwrap_or("").to_string(),
            attachments: a["attachmentFolderPath"].as_str().unwrap_or("").to_string(),
            strict_line_breaks: a["strictLineBreaks"].as_bool().unwrap_or(false),
        }
    });
    let mut skip: Vec<std::path::PathBuf> = skip_root;
    if let Some(t) = &templates {
        skip.push(crate::knowledge::join(root, t));
    }
    let md_files = walk(&pages_dir, &skip, is_md).len();
    Ok(Detected { pages, index, notes, naming, templates, taxonomies, schema, skills: skills(root), sources, staging, ledgers, log, obsidian, md_files, notes_count: note_files.len() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::fixture;

    #[test]
    fn detects_a_twin_shaped_vault() {
        let (_env, root) = crate::paths::testenv::folders("kn-detect");
        let v = fixture::twin(&root.join("twin"));
        let d = detect(&v).unwrap();
        assert_eq!(d.pages, "knowledge");
        assert_eq!(d.index.as_deref(), Some("knowledge/CATALOG.md"));
        assert_eq!(d.notes.as_deref(), Some("knowledge/notes"));
        assert_eq!(d.naming, DATE_SLUG);
        assert_eq!(d.notes_count, 3);
        assert_eq!(d.taxonomies, ["knowledge/TAG_TAXONOMY.md"]);
        assert_eq!(d.schema, ["AGENTS.md"]);
        let skills: Vec<(&str, &str, &str)> = d.skills.iter().map(|s| (s.name.as_str(), s.kind.as_str(), s.description.as_str())).collect();
        assert_eq!(skills, [("s", "skill", "Does s"), ("c", "command", "Does c")]);
        assert_eq!(d.sources.as_deref(), Some("raw_inbox"));
        assert_eq!(d.ledgers, ["processing_log.json"]);
        assert_eq!(d.obsidian, Some(Obsidian { markdown_links: true, link_format: "relative".into(), attachments: String::new(), strict_line_breaks: false }));
        // CATALOG, TAG_TAXONOMY, the client, three notes and the notes README.
        assert_eq!(d.md_files, 7);
        assert_eq!(d.templates, None);
        assert_eq!(d.log, None);
    }

    #[test]
    fn docs_folder_is_the_pages_root() {
        let (_env, root) = crate::paths::testenv::folders("kn-docs");
        let v = root.join("v");
        for i in 0..9 {
            fixture::write(&v, &format!("docs/page-{i}.md"), "# p\n");
        }
        fixture::write(&v, "README.md", "# readme\n");
        let d = detect(&v).unwrap();
        assert_eq!(d.pages, "docs");
        assert_eq!(d.index.as_deref(), Some("README.md"), "the root's README when the pages root has no index");
    }

    #[test]
    fn a_plain_folder_is_its_own_pages_root() {
        let (_env, root) = crate::paths::testenv::folders("kn-plain");
        let v = root.join("v");
        fixture::write(&v, "a.md", "# a\n");
        fixture::write(&v, "Meetings/2026-03-01 Weekly.md", "# w\n");
        fixture::write(&v, "Meetings/2026-03-08 Weekly.md", "# w\n");
        fixture::write(&v, "node_modules/x/readme.md", "# no\n");
        fixture::write(&v, ".trash/old.md", "# no\n");
        let d = detect(&v).unwrap();
        assert_eq!(d.pages, "");
        assert_eq!(d.notes.as_deref(), Some("Meetings"));
        assert_eq!(d.naming, DATE_TITLE);
        assert_eq!(d.md_files, 3);
        assert_eq!(d.index, None);
    }

    #[test]
    fn templates_and_log_are_found() {
        let (_env, root) = crate::paths::testenv::folders("kn-tpl");
        let v = root.join("v");
        fixture::write(&v, "knowledge/.obsidian/templates.json", "{\"folder\": \"tpl\"}");
        fixture::write(&v, "knowledge/tpl/page.md", "---\ntitle: {{title}}\n---\n");
        fixture::write(&v, "knowledge/a.md", "# a\n");
        fixture::write(&v, "log.md", "# Log\n");
        fixture::write(&v, ".staging/x.md", "# staged\n");
        let d = detect(&v).unwrap();
        assert_eq!(d.templates.as_deref(), Some("knowledge/tpl"));
        assert_eq!(d.log.as_deref(), Some("log.md"));
        assert_eq!(d.staging.as_deref(), Some(".staging"));
        assert_eq!(d.md_files, 1, "templates and staging are not pages");
    }
}

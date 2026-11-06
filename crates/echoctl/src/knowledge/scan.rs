//! Echo's index of a vault: every page's title, properties, tags, headings and links, read the way Obsidian reads
//! them. Kept in Echo's data folder (`<home>/.echo/knowledge/<id>/index.json`), rebuilt incrementally by size and
//! modification time, and kept in memory while the app runs. Page paths are relative to the vault's pages root.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{fm, Vault};

/// At most this much of a file is read.
pub const READ_MAX: u64 = 2 << 20;

/// A link as written in a page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawLink {
    pub target: String,
    /// The line it is on (1-based, frontmatter lines counted).
    pub line: usize,
    pub wiki: bool,
    pub embed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub path: String,
    pub title: String,
    pub props: fm::Props,
    pub has_props: bool,
    pub tags: Vec<String>,
    pub headings: Vec<(u8, String)>,
    pub links: Vec<RawLink>,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Index {
    pub pages: BTreeMap<String, Page>,
    pub rev: String,
    /// The pages root, relative to the vault's root.
    pub root: String,
}

/// The folders under the pages root that hold no pages: the sources and the templates (vault-relative → absolute).
fn skipped(v: &Vault) -> Vec<PathBuf> {
    ["sources", "templates"].iter().filter_map(|k| v.path(k)).map(|p| super::join(&v.root, &p)).collect()
}

fn is_md(rel: &str) -> bool {
    rel.rsplit('.').next().is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

/// The pages' paths with size and time (no reading).
fn stat(v: &Vault) -> Vec<(String, u64, i64)> {
    let base = v.pages_dir();
    super::detect::walk(&base, &skipped(v), is_md)
        .into_iter()
        .filter_map(|rel| {
            let m = std::fs::metadata(super::join(&base, &rel)).ok()?;
            let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
            Some((rel, m.len(), t))
        })
        .collect()
}

fn rev_of(list: &[(String, u64, i64)]) -> String {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    for (p, s, t) in list {
        h.update(format!("{p}\0{s}\0{t}\n"));
    }
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// A cheap revision of the vault's pages: their paths, sizes and times.
pub fn rev(v: &Vault) -> String {
    rev_of(&stat(v))
}

static CACHE: Mutex<Option<HashMap<String, Arc<Index>>>> = Mutex::new(None);
static SCANNING: Mutex<()> = Mutex::new(());

fn cache_file(v: &Vault) -> PathBuf {
    super::cache_dir(&v.id).join("index.json")
}

/// Echo's index of the vault, scanned again where files changed.
pub fn index(v: &Vault) -> Result<Arc<Index>> {
    let _one = SCANNING.lock().unwrap_or_else(|e| e.into_inner());
    let list = stat(v);
    let now = rev_of(&list);
    let root = v.path("pages").unwrap_or_default();
    let held = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|c| c.get(&v.id).cloned());
    if let Some(ix) = held.as_ref().filter(|ix| ix.rev == now && ix.root == root) {
        return Ok(ix.clone());
    }
    // What was read before: from memory, else from Echo's cache file.
    let before: Index = held.map(|a| (*a).clone()).or_else(|| std::fs::read(cache_file(v)).ok().and_then(|b| serde_json::from_slice(&b).ok())).filter(|ix: &Index| ix.root == root).unwrap_or_default();
    let base = v.pages_dir();
    let mut pages = BTreeMap::new();
    for (rel, size, mtime) in &list {
        if let Some(p) = before.pages.get(rel).filter(|p| p.size == *size && p.mtime == *mtime) {
            pages.insert(rel.clone(), p.clone());
            continue;
        }
        let Some(text) = read_capped(&super::join(&base, rel)) else { continue };
        pages.insert(rel.clone(), page(rel, &text, *size, *mtime));
    }
    let ix = Arc::new(Index { pages, rev: now, root });
    if let Some(dir) = cache_file(v).parent() {
        let _ = std::fs::create_dir_all(dir);
        if let Ok(b) = serde_json::to_vec(&*ix) {
            let _ = crate::paths::write_atomic(&cache_file(v), &b);
        }
    }
    CACHE.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(v.id.clone(), ix.clone());
    Ok(ix)
}

/// Forget what is held in memory for a vault (it was removed, or its pages root changed).
pub fn forget(id: &str) {
    if let Some(c) = CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        c.remove(id);
    }
}

/// A file's text, at most `READ_MAX` bytes of it; `None` for a file that can't be read or isn't text.
pub fn read_capped(p: &Path) -> Option<String> {
    use std::io::Read as _;
    let mut buf = vec![];
    std::fs::File::open(p).ok()?.take(READ_MAX).read_to_end(&mut buf).ok()?;
    if buf.contains(&0) {
        return None;
    }
    Some(match String::from_utf8(buf) {
        Ok(s) => s,
        // Cut mid-character at the cap: keep what decodes.
        Err(e) => {
            let valid = e.utf8_error().valid_up_to();
            let mut b = e.into_bytes();
            b.truncate(valid);
            String::from_utf8(b).ok()?
        }
    })
}

/// The line's text outside inline code spans (`` `…` ``), the spans blanked out (positions kept).
fn outside_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// `[[…]]` and `![[…]]` in a line.
fn wikilinks(line: &str, n: usize, out: &mut Vec<RawLink>) {
    let mut rest = line;
    while let Some(i) = rest.find("[[") {
        let embed = rest[..i].ends_with('!');
        let after = &rest[i + 2..];
        let Some(j) = after.find("]]") else { break };
        let target = after[..j].trim();
        if !target.is_empty() && !target.contains('\n') {
            out.push(RawLink { target: target.to_string(), line: n, wiki: true, embed });
        }
        rest = &after[j + 2..];
    }
}

/// `[text](dest)` and `![alt](dest)` in a line, skipping web, mail and other schemes, and `#anchor`-only links.
fn md_links(line: &str, n: usize, out: &mut Vec<RawLink>) {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b']' && i + 1 < b.len() && b[i + 1] == b'(' {
            // The `[` that opens this text (not one of a wikilink).
            let Some(open) = line[..i].rfind('[') else { i += 1; continue };
            if open > 0 && b[open - 1] == b'[' {
                i += 1;
                continue;
            }
            let embed = open > 0 && b[open - 1] == b'!';
            let rest = &line[i + 2..];
            let (dest, used) = if let Some(r) = rest.strip_prefix('<') {
                match r.find('>') {
                    Some(k) if r[k + 1..].starts_with(')') => (format!("<{}>", &r[..k]), k + 3),
                    _ => { i += 1; continue }
                }
            } else {
                // Balanced parentheses inside the destination; a title after a space ends it.
                let mut depth = 0;
                let mut end = None;
                for (k, c) in rest.char_indices() {
                    match c {
                        '(' => depth += 1,
                        ')' if depth == 0 => { end = Some(k); break }
                        ')' => depth -= 1,
                        _ => {}
                    }
                }
                let Some(k) = end else { i += 1; continue };
                let d = rest[..k].split(" \"").next().unwrap_or("").trim().to_string();
                (d, k + 1)
            };
            let bare = dest.trim_start_matches('<');
            let scheme = bare.split_once(':').is_some_and(|(s, _)| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') && s.len() > 1);
            if !dest.is_empty() && !dest.starts_with('#') && !scheme {
                out.push(RawLink { target: dest, line: n, wiki: false, embed });
            }
            i += 2 + used;
            continue;
        }
        i += 1;
    }
}

/// Inline `#tags` in a line (not headings, not inside words, not all digits).
fn inline_tags(line: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' && (i == 0 || chars[i - 1].is_whitespace() || chars[i - 1] == '(') {
            let start = i + 1;
            let mut j = start;
            while j < chars.len() && (chars[j].is_alphanumeric() || matches!(chars[j], '_' | '-' | '/')) {
                j += 1;
            }
            let tag: String = chars[start..j].iter().collect();
            if !tag.is_empty() && !tag.chars().all(|c| c.is_ascii_digit()) {
                out.push(tag);
            }
            i = j;
            continue;
        }
        i += 1;
    }
}

/// Read one page from its text.
pub fn page(path: &str, text: &str, size: u64, mtime: i64) -> Page {
    let (raw, _) = fm::split(text);
    let props = raw.map(fm::parse).unwrap_or_default();
    let fm_lines = raw.map(|r| r.lines().count() + 2).unwrap_or(0);
    let mut tags: Vec<String> = fm::get(&props, "tags").map(|p| p.items()).unwrap_or_default().into_iter().map(|t| t.trim_start_matches('#').trim().to_string()).filter(|t| !t.is_empty()).collect();
    let mut headings = vec![];
    let mut links = vec![];
    let mut fence: Option<&str> = None;
    for (k, line) in text.lines().enumerate() {
        let n = k + 1;
        if n <= fm_lines {
            // Properties may link: `client: "[[Acme Corp]]"`.
            wikilinks(line, n, &mut links);
            continue;
        }
        let t = line.trim_start();
        if let Some(f) = fence {
            if t.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = Some(&t[..3]);
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
            let h = t[hashes..].trim().trim_end_matches('#').trim().to_string();
            if !h.is_empty() {
                headings.push((hashes as u8, h));
            }
            continue;
        }
        let plain = outside_code(line);
        wikilinks(&plain, n, &mut links);
        md_links(&plain, n, &mut links);
        inline_tags(&plain, &mut tags);
    }
    let mut seen = std::collections::HashSet::new();
    tags.retain(|t| seen.insert(t.to_lowercase()));
    let stem = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".md").trim_end_matches(".MD").to_string();
    let title = fm::get(&props, "title").map(|p| p.text()).filter(|s| !s.trim().is_empty()).or_else(|| headings.iter().find(|(l, _)| *l == 1).map(|(_, h)| h.clone())).unwrap_or(stem);
    Page { path: path.into(), title, has_props: raw.is_some(), props, tags, headings, links, size, mtime }
}

/// Is a vault-relative path one of the vault's pages (not machinery, sources or templates)?
pub fn is_page_path(v: &Vault, vault_rel: &str) -> bool {
    let pages = v.path("pages").unwrap_or_default();
    let inside = if pages.is_empty() { Some(vault_rel) } else { vault_rel.strip_prefix(&format!("{pages}/")) };
    let Some(rel) = inside else { return false };
    if !is_md(rel) || rel.split('/').any(|c| c.is_empty() || c == ".." || super::detect::machinery_name(c)) {
        return false;
    }
    let abs = super::join(&v.root, vault_rel);
    !skipped(v).iter().any(|s| abs.starts_with(s))
}

/// A vault-relative path as a page path (relative to the pages root), if it is under it.
pub fn page_rel(v: &Vault, vault_rel: &str) -> Option<String> {
    let pages = v.path("pages").unwrap_or_default();
    if pages.is_empty() { Some(vault_rel.to_string()) } else { vault_rel.strip_prefix(&format!("{pages}/")).map(str::to_owned) }
}

/// A page path as vault-relative.
pub fn vault_rel(v: &Vault, page: &str) -> String {
    let pages = v.path("pages").unwrap_or_default();
    if pages.is_empty() { page.to_string() } else { format!("{pages}/{page}") }
}

/// (vault id, page) → (size, mtime, the page's text in lower case), for search.
type TextCache = HashMap<(String, String), (u64, i64, String)>;
static TEXT: Mutex<Option<TextCache>> = Mutex::new(None);

/// Find pages: `#tag`, or words in the title, the path and the text (titles first). At most `limit`.
pub fn search(v: &Vault, ix: &Index, q: &str, limit: usize) -> Vec<serde_json::Value> {
    let q = q.trim();
    if q.is_empty() {
        return vec![];
    }
    let hit = |p: &Page, how: &str, line: Option<String>| serde_json::json!({ "path": p.path, "title": p.title, "hit": how, "line": line });
    if let Some(tag) = q.strip_prefix('#') {
        let tag = tag.to_lowercase();
        return ix.pages.values().filter(|p| p.tags.iter().any(|t| t.to_lowercase() == tag || t.to_lowercase().starts_with(&format!("{tag}/")))).take(limit).map(|p| hit(p, "tag", None)).collect();
    }
    let words: Vec<String> = q.to_lowercase().split_whitespace().map(str::to_owned).collect();
    let all = |s: &str| { let s = s.to_lowercase(); words.iter().all(|w| s.contains(w.as_str())) };
    let mut out = vec![];
    let mut taken = std::collections::HashSet::new();
    let mut by_title: Vec<&Page> = ix.pages.values().filter(|p| all(&p.title)).collect();
    by_title.sort_by_key(|p| (!p.title.to_lowercase().starts_with(&words[0]), p.title.len()));
    for p in by_title {
        if out.len() >= limit { return out; }
        taken.insert(p.path.clone());
        out.push(hit(p, "title", None));
    }
    let by_path: Vec<&Page> = ix.pages.values().filter(|p| !taken.contains(&p.path) && all(&p.path)).collect();
    for p in by_path {
        if out.len() >= limit { return out; }
        taken.insert(p.path.clone());
        out.push(hit(p, "path", None));
    }
    let base = v.pages_dir();
    let mut cache = TEXT.lock().unwrap_or_else(|e| e.into_inner());
    let cache = cache.get_or_insert_with(HashMap::new);
    for p in ix.pages.values().filter(|p| !taken.contains(&p.path)) {
        if out.len() >= limit { break; }
        let key = (v.id.clone(), p.path.clone());
        let fresh = cache.get(&key).is_some_and(|(s, t, _)| *s == p.size && *t == p.mtime);
        if !fresh {
            let text = read_capped(&super::join(&base, &p.path)).unwrap_or_default().to_lowercase();
            cache.insert(key.clone(), (p.size, p.mtime, text));
        }
        let text = &cache[&key].2;
        if words.iter().all(|w| text.contains(w.as_str())) {
            // The match cache is lower case: the line as written comes from the file.
            let line = text.lines().position(|l| l.contains(words[0].as_str())).and_then(|n| {
                let raw = read_capped(&super::join(&base, &p.path))?;
                let plain = super::page::plain_line(raw.lines().nth(n)?);
                let at = plain.to_lowercase().find(words[0].as_str()).map_or(0, |b| plain.to_lowercase()[..b].chars().count());
                let from = at.saturating_sub(40);
                let cut: String = plain.chars().skip(from).take(140).collect();
                Some(if from > 0 { format!("…{}", cut.trim_start()) } else { cut })
            });
            out.push(hit(p, "text", line));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{self, fixture, AddOpts};

    fn vault(root: &Path) -> Vault {
        knowledge::add(root, AddOpts { name: Some("T".into()), ..Default::default() }).unwrap()
    }

    #[test]
    fn a_page_is_read_like_obsidian() {
        let text = "---\ntitle: Pricing model\ntags: [concept, '#pricing']\nclient: \"[[Acme Corp]]\"\n---\n# Heading one\n\nSee [[2026-01-01-a|the first sync]] and [Acme](../clients/acme.md#people).\n![[diagram.png]] ![x](img/a.png) [web](https://example.com) [mail](mailto:a@b.c)\n\n```\n[[not-a-link]] #notatag\n```\nInline `[[code]]` #real-tag and #2026 but not a#b.\n## Two ##\n";
        let p = page("topics/pricing.md", text, text.len() as u64, 1);
        assert_eq!(p.title, "Pricing model");
        assert!(p.has_props);
        assert_eq!(p.tags, ["concept", "pricing", "real-tag"]);
        assert_eq!(p.headings, [(1, "Heading one".to_string()), (2, "Two".to_string())]);
        let links: Vec<(&str, usize, bool, bool)> = p.links.iter().map(|l| (l.target.as_str(), l.line, l.wiki, l.embed)).collect();
        assert_eq!(links, [("Acme Corp", 4, true, false), ("2026-01-01-a|the first sync", 8, true, false), ("../clients/acme.md#people", 8, false, false), ("diagram.png", 9, true, true), ("img/a.png", 9, false, true)]);
    }

    #[test]
    fn a_title_comes_from_the_first_heading_or_the_name() {
        assert_eq!(page("a/b.md", "intro\n# The real title\n# Second\n", 1, 1).title, "The real title");
        assert_eq!(page("a/plain-name.md", "no heading", 1, 1).title, "plain-name");
    }

    #[test]
    fn machinery_is_not_pages() {
        let (_env, root) = crate::paths::testenv::folders("kn-machinery");
        let t = fixture::twin(&root.join("twin"));
        fixture::write(&t, ".claude/worktrees/x/knowledge/a.md", "# copy\n");
        fixture::write(&t, "raw_inbox/a.md", "# source\n");
        fixture::write(&t, "knowledge/.obsidian/templates.json", "{\"folder\":\"templates\"}");
        fixture::write(&t, "knowledge/templates/t.md", "# {{title}}\n");
        fixture::write(&t, "knowledge/node_modules/a.md", "# no\n");
        let v = vault(&t);
        let ix = index(&v).unwrap();
        let paths: Vec<&str> = ix.pages.keys().map(String::as_str).collect();
        assert_eq!(paths, ["CATALOG.md", "TAG_TAXONOMY.md", "clients/acme.md", "notes/2026-01-01-a.md", "notes/2026-01-02-b.md", "notes/2026-01-03-c.md", "notes/README.md"]);
        assert!(is_page_path(&v, "knowledge/clients/acme.md"));
        assert!(!is_page_path(&v, "knowledge/templates/t.md"));
        assert!(!is_page_path(&v, "raw_inbox/a.md"));
        assert!(!is_page_path(&v, "AGENTS.md"), "outside the pages root");
    }

    #[test]
    fn external_edits_change_the_revision() {
        let (_env, root) = crate::paths::testenv::folders("kn-rev");
        let t = fixture::twin(&root.join("twin"));
        let v = vault(&t);
        let ix = index(&v).unwrap();
        let r1 = rev(&v);
        assert_eq!(ix.rev, r1);
        std::thread::sleep(std::time::Duration::from_millis(20));
        fixture::write(&t, "knowledge/clients/acme.md", "---\ntitle: Acme Corporation\n---\n# x\n");
        let r2 = rev(&v);
        assert_ne!(r1, r2);
        let ix2 = index(&v).unwrap();
        assert_eq!(ix2.pages["clients/acme.md"].title, "Acme Corporation");
        assert!(std::path::Path::new(&crate::knowledge::cache_dir(&v.id)).join("index.json").is_file());
    }

    #[test]
    #[cfg(unix)]
    fn symlinks_out_of_the_vault_are_not_pages() {
        let (_env, root) = crate::paths::testenv::folders("kn-symlink");
        let t = fixture::twin(&root.join("twin"));
        fixture::write(&root, "outside/secret.md", "# secret\n");
        std::os::unix::fs::symlink(root.join("outside"), t.join("knowledge/linked")).unwrap();
        std::os::unix::fs::symlink(root.join("outside/secret.md"), t.join("knowledge/secret.md")).unwrap();
        let ix = index(&vault(&t)).unwrap();
        assert!(!ix.pages.keys().any(|p| p.contains("secret") || p.starts_with("linked")));
    }

    #[test]
    fn huge_files_are_read_up_to_the_cap() {
        let (_env, root) = crate::paths::testenv::folders("kn-huge");
        let t = fixture::twin(&root.join("twin"));
        let mut big = String::from("# Big\n");
        while (big.len() as u64) < READ_MAX + 1000 {
            big.push_str("filler line with [[2026-01-01-a]]\n");
        }
        fixture::write(&t, "knowledge/big.md", &big);
        let ix = index(&vault(&t)).unwrap();
        let p = &ix.pages["big.md"];
        assert_eq!(p.title, "Big");
        assert!(p.size > READ_MAX);
        assert!(p.links.len() < big.lines().count(), "only the first {READ_MAX} bytes were read");
    }

    #[test]
    fn search_finds_titles_paths_tags_and_words() {
        let (_env, root) = crate::paths::testenv::folders("kn-search");
        let t = fixture::twin(&root.join("twin"));
        fixture::write(&t, "knowledge/topics/pricing.md", "---\ntitle: Pricing model\ntags: [concept/pricing]\n---\nThe Acme discount is 12%.\n");
        let v = vault(&t);
        let ix = index(&v).unwrap();
        let paths = |q: &str| search(&v, &ix, q, 10).iter().map(|h| (h["path"].as_str().unwrap().to_string(), h["hit"].as_str().unwrap().to_string())).collect::<Vec<_>>();
        assert_eq!(paths("acme")[0], ("clients/acme.md".into(), "title".into()));
        assert!(paths("acme").contains(&("topics/pricing.md".into(), "text".into())));
        assert_eq!(paths("#concept"), [("topics/pricing.md".to_string(), "tag".to_string())]);
        assert_eq!(paths("2026-01-02")[0], ("notes/2026-01-02-b.md".to_string(), "path".to_string()), "paths before text");
        assert!(paths("   ").is_empty());
        assert!(paths("nothing-like-this").is_empty());
    }

    #[test]
    fn a_text_hit_shows_its_line_as_written() {
        let (_env, root) = crate::paths::testenv::folders("kn-search-line");
        let t = fixture::twin(&root.join("twin"));
        let long = "word ".repeat(30);
        fixture::write(&t, "knowledge/topics/plane.md", &format!("---\ntitle: Notes\n---\n| [Cell](a.md) | The **Control Plane** ships |\n{long}and the Rollout Plan here.\n"));
        let v = vault(&t);
        let ix = index(&v).unwrap();
        let line = |q: &str| search(&v, &ix, q, 10).into_iter().find(|h| h["path"] == "topics/plane.md").map(|h| h["line"].as_str().unwrap().to_string()).unwrap();
        assert_eq!(line("control plane"), "Cell · The Control Plane ships", "the case as written, no Markdown");
        let far = line("rollout");
        assert!(far.starts_with('…') && far.contains("the Rollout Plan here."), "a match far into the line is shown: {far}");
    }

    #[test]
    #[ignore]
    fn five_thousand_pages_scan_in_time() {
        let (_env, root) = crate::paths::testenv::folders("kn-perf");
        let t = root.join("big");
        for i in 0..5000 {
            fixture::write(&t, &format!("notes/{:02}/page-{i}.md", i % 50), &format!("---\ntitle: Page {i}\ntags: [meeting]\n---\n# Page {i}\n\nSee [[page-{}]] and [[page-{}]].\n{}", (i + 1) % 5000, (i * 7) % 5000, "Some words of the page. ".repeat(40)));
        }
        let v = vault(&t);
        let t0 = std::time::Instant::now();
        let ix = index(&v).unwrap();
        let cold = t0.elapsed();
        forget(&v.id);
        let t1 = std::time::Instant::now();
        let _ = index(&v).unwrap();
        let warm = t1.elapsed();
        let t2 = std::time::Instant::now();
        let e = crate::knowledge::links::edges(&ix);
        let edges = t2.elapsed();
        eprintln!("5000 pages: cold {cold:?}, from cache {warm:?}, {} edges in {edges:?}", e.len());
        assert_eq!(ix.pages.len(), 5000);
        assert!(cold < std::time::Duration::from_secs(3), "cold {cold:?}");
        assert!(warm < std::time::Duration::from_millis(300), "incremental {warm:?}");
    }
}

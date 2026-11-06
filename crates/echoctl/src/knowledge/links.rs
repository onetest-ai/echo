//! Where a page's links lead, resolved the way Obsidian resolves them: a relative Markdown link from the page's own
//! folder (or from the vault), a `[[wikilink]]` by its path or, failing that, by the note's name — the shortest path
//! when several notes share it. Links never leave the vault.

use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::scan::{Index, RawLink};

/// Lookups over an index, built once per index.
pub struct Resolver<'a> {
    ix: &'a Index,
    by_lower: HashMap<String, String>,
    by_stem: HashMap<String, Vec<String>>,
}

/// `%xx` decoded (a broken escape is kept as written).
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// `a/b/../c` → `a/c`; `None` when it climbs above the start.
pub fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = vec![];
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

fn with_md(p: &str) -> String {
    let last = p.rsplit('/').next().unwrap_or(p);
    let other_file = last.contains('.') && last.rsplit('.').next().is_some_and(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()));
    if other_file || p.to_ascii_lowercase().ends_with(".md") {
        p.to_string()
    } else {
        format!("{p}.md")
    }
}

fn stem(p: &str) -> String {
    let last = p.rsplit('/').next().unwrap_or(p);
    last.strip_suffix(".md").or_else(|| last.strip_suffix(".MD")).unwrap_or(last).to_lowercase()
}

fn folder(p: &str) -> &str {
    p.rsplit_once('/').map(|(f, _)| f).unwrap_or("")
}

impl<'a> Resolver<'a> {
    pub fn new(ix: &'a Index) -> Self {
        let mut by_lower = HashMap::new();
        let mut by_stem: HashMap<String, Vec<String>> = HashMap::new();
        for p in ix.pages.keys() {
            by_lower.insert(p.to_lowercase(), p.clone());
            by_stem.entry(stem(p)).or_default().push(p.clone());
        }
        for v in by_stem.values_mut() {
            v.sort_by(|a, b| a.matches('/').count().cmp(&b.matches('/').count()).then(a.len().cmp(&b.len())).then(a.cmp(b)));
        }
        Resolver { ix, by_lower, by_stem }
    }

    fn page(&self, rel: &str) -> Option<String> {
        self.ix.pages.get(rel).map(|_| rel.to_string()).or_else(|| self.by_lower.get(&rel.to_lowercase()).cloned())
    }

    /// The page a link from page `from` leads to (pages-root-relative), if it is a page of the vault.
    pub fn resolve(&self, from: &str, link: &RawLink) -> Option<String> {
        let bare = Self::bare(link);
        if bare.is_empty() {
            return None;
        }
        if link.wiki {
            let path = bare.trim_start_matches('/');
            if path.contains('/') {
                if let Some(p) = normalize(path).and_then(|n| self.page(&with_md(&n))) {
                    return Some(p);
                }
            }
            let candidates = self.by_stem.get(&stem(path))?;
            let here = folder(from);
            return candidates.iter().find(|c| folder(c) == here).or_else(|| candidates.first()).cloned();
        }
        // A Markdown link: from the page's folder, then from the pages root.
        let target = bare.replace('\\', "/");
        let rel = if let Some(abs) = target.strip_prefix('/') { normalize(abs) } else { normalize(&format!("{}/{}", folder(from), target)) };
        rel.and_then(|r| self.page(&with_md(&r)))
            .or_else(|| normalize(target.trim_start_matches('/')).and_then(|r| self.page(&with_md(&r))))
            // Then by its name, as Obsidian resolves a Markdown link it can't find by path.
            .or_else(|| {
                if target.contains('/') || !with_md(&target).to_ascii_lowercase().ends_with(".md") {
                    return None;
                }
                let candidates = self.by_stem.get(&stem(&target))?;
                let here = folder(from);
                candidates.iter().find(|c| folder(c) == here).or_else(|| candidates.first()).cloned()
            })
    }

    /// The link's target without its `#heading`, `|alias` or `^block`, decoded — what it names.
    pub fn bare(link: &RawLink) -> String {
        let t = link.target.trim();
        if link.wiki {
            let t = t.split('|').next().unwrap_or("");
            return t.split('#').next().unwrap_or("").trim().to_string();
        }
        let t = t.strip_prefix('<').and_then(|x| x.strip_suffix('>')).unwrap_or(t);
        let t = t.split('#').next().unwrap_or("");
        percent_decode(t).trim().to_string()
    }
}

/// Every resolved link: from → (to, line).
pub fn edges(ix: &Index) -> Vec<(String, String, usize)> {
    let r = Resolver::new(ix);
    let mut out = vec![];
    for (from, p) in &ix.pages {
        for l in p.links.iter().filter(|l| !l.embed) {
            if let Some(to) = r.resolve(from, l) {
                if &to != from {
                    out.push((from.clone(), to, l.line));
                }
            }
        }
    }
    out
}

/// The pages linking to `path`, with the line of each one's first link to it.
pub fn backlinks(ix: &Index, path: &str) -> Vec<(String, usize)> {
    let mut seen = BTreeMap::new();
    for (from, to, line) in edges(ix) {
        if to == path {
            seen.entry(from).or_insert(line);
        }
    }
    seen.into_iter().collect()
}

/// The graph of the vault: pages as nodes (with their top folder and tags), links as edges, and the hubs — the index
/// page and pages linking to more than 40 % of the others. `mode`: `links`, `tags` (tags as nodes) or `both`.
pub fn graph(ix: &Index, index_page: Option<&str>, mode: &str) -> Value {
    let e = edges(ix);
    let mut deg: HashMap<&str, usize> = HashMap::new();
    let mut out_n: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for (a, b, _) in &e {
        *deg.entry(a).or_default() += 1;
        *deg.entry(b).or_default() += 1;
        out_n.entry(a).or_default().insert(b);
    }
    let n = ix.pages.len().max(1);
    let hubs: Vec<&str> = ix.pages.keys().map(String::as_str).filter(|p| Some(*p) == index_page || out_n.get(p).is_some_and(|o| o.len() * 10 > n * 4 && n >= 10)).collect();
    let mut nodes: Vec<Value> = ix
        .pages
        .values()
        .map(|p| json!({ "id": p.path, "title": p.title, "folder": p.path.split_once('/').map(|(f, _)| f).unwrap_or(""), "tags": p.tags, "deg": deg.get(p.path.as_str()).copied().unwrap_or(0) }))
        .collect();
    let mut edges: Vec<Value> = if mode == "tags" { vec![] } else {
        let mut uniq = BTreeSet::new();
        e.iter().filter(|(a, b, _)| uniq.insert((a.clone(), b.clone()))).map(|(a, b, _)| json!([a, b])).collect()
    };
    if mode == "tags" || mode == "both" {
        let tags: BTreeSet<&str> = ix.pages.values().flat_map(|p| p.tags.iter().map(String::as_str)).collect();
        nodes.extend(tags.iter().map(|t| json!({ "id": format!("#{t}"), "title": format!("#{t}"), "tag": true })));
        for p in ix.pages.values() {
            edges.extend(p.tags.iter().map(|t| json!([p.path, format!("#{t}")])));
        }
    }
    json!({ "nodes": nodes, "edges": edges, "hubs": hubs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::scan::{page, Index};

    fn ix(files: &[(&str, &str)]) -> Index {
        let mut ix = Index::default();
        for (p, t) in files {
            ix.pages.insert(p.to_string(), page(p, t, t.len() as u64, 1));
        }
        ix
    }

    fn link(target: &str, wiki: bool) -> RawLink {
        RawLink { target: target.into(), line: 1, wiki, embed: false }
    }

    #[test]
    fn links_resolve_like_obsidian() {
        let ix = ix(&[
            ("clients/acme.md", "# Acme"),
            ("clients/b c.md", "# B C"),
            ("notes/2026-01-01-a.md", "# a"),
            ("Клиент.md", "# Клиент"),
            ("topics/same.md", "# same"),
            ("topics/deep/same.md", "# same, deeper"),
            ("archive/old/same.md", "# same, old"),
        ]);
        let r = Resolver::new(&ix);
        let from = "notes/2026-01-01-a.md";
        assert_eq!(r.resolve(from, &link("../clients/acme.md", false)).as_deref(), Some("clients/acme.md"));
        assert_eq!(r.resolve(from, &link("../clients/acme.md#people", false)).as_deref(), Some("clients/acme.md"));
        assert_eq!(r.resolve(from, &link("../clients/b%20c.md", false)).as_deref(), Some("clients/b c.md"));
        assert_eq!(r.resolve(from, &link("<../clients/b c.md>", false)).as_deref(), Some("clients/b c.md"));
        assert_eq!(r.resolve(from, &link("clients/acme.md", false)).as_deref(), Some("clients/acme.md"), "vault-relative");
        assert_eq!(r.resolve(from, &link("../clients/ACME.md", false)).as_deref(), Some("clients/acme.md"), "case-insensitive");
        assert_eq!(r.resolve(from, &link("acme", true)).as_deref(), Some("clients/acme.md"));
        assert_eq!(r.resolve(from, &link("acme|Acme Corp", true)).as_deref(), Some("clients/acme.md"));
        assert_eq!(r.resolve(from, &link("acme#People", true)).as_deref(), Some("clients/acme.md"));
        assert_eq!(r.resolve(from, &link("clients/acme", true)).as_deref(), Some("clients/acme.md"));
        assert_eq!(r.resolve(from, &link("Клиент", true)).as_deref(), Some("Клиент.md"));
        assert_eq!(r.resolve(from, &link("same", true)).as_deref(), Some("topics/same.md"), "the shortest path wins");
        assert_eq!(r.resolve("topics/deep/x.md", &link("same", true)).as_deref(), Some("topics/deep/same.md"), "the same folder first");
        assert_eq!(r.resolve(from, &link("acme.md", false)).as_deref(), Some("clients/acme.md"), "a Markdown link falls back to the name, as Obsidian does");
        assert_eq!(r.resolve(from, &link("b%20c.md", false)).as_deref(), Some("clients/b c.md"));
        assert_eq!(r.resolve(from, &link("missing", true)), None);
        assert_eq!(r.resolve(from, &link("../../../etc/passwd", false)), None, "never out of the vault");
        assert_eq!(r.resolve(from, &link("#heading-only", false)), None);
        assert_eq!(Resolver::bare(&link("acme|Acme Corp", true)), "acme");
        assert_eq!(Resolver::bare(&link("../b%20c.md#x", false)), "../b c.md");
    }

    #[test]
    fn backlinks_skip_code_blocks() {
        let ix = ix(&[
            ("a.md", "# A"),
            ("b.md", "line\nsee [[a]]\nagain [[a]]"),
            ("c.md", "[A](a.md)"),
            ("d.md", "```\n[[a]]\n```\n`[[a]]`"),
        ]);
        assert_eq!(backlinks(&ix, "a.md"), [("b.md".to_string(), 2), ("c.md".to_string(), 1)]);
    }

    #[test]
    fn graph_marks_the_index_as_a_hub() {
        let mut files = vec![("CATALOG.md".to_string(), String::new())];
        for i in 0..12 {
            files[0].1.push_str(&format!("[[p{i}]]\n"));
            files.push((format!("p{i}.md"), if i == 0 { "[[p1]]".into() } else { String::new() }));
        }
        let refs: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let g = graph(&ix(&refs), Some("CATALOG.md"), "links");
        assert_eq!(g["hubs"], json!(["CATALOG.md"]));
        assert_eq!(g["nodes"].as_array().unwrap().len(), 13);
        assert_eq!(g["edges"].as_array().unwrap().len(), 13);
    }
}

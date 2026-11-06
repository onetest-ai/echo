//! A vault's health, read-only and deterministic (spec K8-05, K16-02): pages not in the index, broken links, isolated
//! pages, pages without properties, tags the taxonomy doesn't allow; index rows to missing pages, properties a folder
//! usually has, duplicate titles, an oversized index. Only pages count — never the index, the taxonomies, a folder's
//! README, templates or machinery.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use super::links::Resolver;
use super::scan::{Index, RawLink};
use super::Vault;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Checks {
    pub not_in_index: Vec<String>,
    /// (page, the link as written)
    pub broken: Vec<(String, String)>,
    pub isolated: Vec<String>,
    pub no_props: Vec<String>,
    /// (page, tag)
    pub unknown_tags: Vec<(String, String)>,
    /// The tag meeting notes get, when the taxonomy doesn't list it.
    pub meeting_tag_missing: Option<String>,
    /// No index file to check against.
    pub no_index: bool,
    /// The index's links (its rows) to pages that aren't there, as written.
    pub dead_rows: Vec<RawLink>,
    /// (page, key): a property at least 80 % of the page's folder has (folders of 5 pages or more) and it lacks.
    pub usual_props: Vec<(String, String)>,
    /// Pages sharing one title (case and spacing ignored), each group sorted.
    pub duplicate_titles: Vec<Vec<String>>,
    /// The index, when it is over [`INDEX_MAX_BYTES`] or [`INDEX_MAX_ROWS`].
    pub oversized_index: Option<Oversized>,
    /// `broken`, each link as written (Health's fixes need it).
    #[serde(skip)]
    pub broken_links: Vec<(String, RawLink)>,
}

/// An index over this is oversized (spec K16-02).
pub const INDEX_MAX_BYTES: u64 = 150 * 1024;
pub const INDEX_MAX_ROWS: usize = 1500;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Oversized {
    pub bytes: u64,
    pub rows: usize,
}

fn tag_like(t: &str) -> bool {
    let mut cs = t.chars();
    cs.next().is_some_and(|c| c.is_alphanumeric()) && cs.all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '/'))
}

/// The tags a taxonomy file names, in order: every `` `tag` `` and every word of a fenced list (`meeting, concept`).
pub fn tags_in_order(text: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut push = |t: &str| {
        let t = t.trim();
        if tag_like(t) && !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    };
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            line.split([',', ' ', '\t']).for_each(&mut push);
            continue;
        }
        let parts: Vec<&str> = line.split('`').collect();
        for (i, p) in parts.iter().enumerate() {
            if i % 2 == 1 && i + 1 < parts.len() {
                push(p);
            }
        }
    }
    out
}

/// The tags a taxonomy file allows.
pub fn allowed_tags(text: &str) -> BTreeSet<String> {
    tags_in_order(text).into_iter().collect()
}

/// The vault's tag taxonomy file (vault-relative), if any.
pub fn tag_taxonomy(v: &Vault) -> Option<String> {
    v.detected.taxonomies.iter().find(|t| t.to_ascii_uppercase().contains("TAG") || t.to_ascii_lowercase().ends_with("tags.md")).cloned()
}

/// The tag meeting notes get: the user's choice, else the taxonomy's first `meeting…` tag, else `meeting`.
pub fn meeting_tag(v: &Vault) -> String {
    if let Some(t) = &v.minutes.tag {
        return t.clone();
    }
    tag_taxonomy(v)
        .and_then(|t| std::fs::read_to_string(super::join(&v.root, &t)).ok())
        .and_then(|text| tags_in_order(&text).into_iter().find(|t| t.to_lowercase().starts_with("meeting")))
        .unwrap_or_else(|| "meeting".into())
}

/// Each of the index's links with the page it leads to (pages-root-relative), if any. `None`: the vault has no index.
fn index_targets(v: &Vault, ix: &Index) -> Option<Vec<(RawLink, Option<String>)>> {
    let index = v.path("index")?;
    let text = super::scan::read_capped(&super::join(&v.root, &index))?;
    let as_page = super::scan::page(&index, &text, 0, 0);
    let r = Resolver::new(ix);
    let index_dir = index.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    Some(as_page.links.into_iter().map(|l| {
        // Resolved from where the index sits (it may be outside the pages root).
        let by_page = super::scan::page_rel(v, &index).and_then(|p| r.resolve(&p, &l));
        let to = by_page.or_else(|| {
            if l.wiki { return None; }
            let vault_rel = super::links::normalize(&format!("{index_dir}/{}", Resolver::bare(&l)))?;
            super::scan::page_rel(v, &vault_rel).filter(|p| ix.pages.contains_key(p))
        });
        (l, to)
    }).collect())
}

/// The pages the index links to (pages-root-relative). `None`: the vault has no index.
pub fn index_links(v: &Vault, ix: &Index) -> Option<BTreeSet<String>> {
    Some(index_targets(v, ix)?.into_iter().filter_map(|(_, to)| to).collect())
}

/// A Markdown link that isn't to a page: `Some(true)` when what it names is there in the vault (a source, a folder, a
/// file outside the pages root) or lies outside the vault (Echo doesn't look there), `Some(false)` when it's missing
/// from the vault. Wikilinks to pages aren't files: `None`. `dir`: the linking file's folder, vault-relative.
fn file_exists(v: &Vault, dir: &str, l: &RawLink) -> Option<bool> {
    let bare = Resolver::bare(l);
    if l.wiki {
        let last = bare.rsplit('/').next().unwrap_or(&bare);
        let ext = last.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
        return ext.filter(|e| e != "md").map(|_| true);
    }
    let Some(rel) = super::links::normalize(&format!("{dir}/{bare}")) else { return Some(true) };
    Some(super::join(&v.root, &rel).exists())
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

/// An embed of something that isn't a page (an image, a PDF): not a link to check.
fn not_a_page(l: &RawLink) -> bool {
    l.embed && !Resolver::bare(l).to_ascii_lowercase().ends_with(".md")
}

/// The rows of a Markdown text's tables: its `|` lines but each table's header and the `|---|` line under it.
fn table_rows(text: &str) -> usize {
    text.lines().map(str::trim).filter(|l| l.starts_with('|')).fold(0, |n, l| {
        if l.contains('-') && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) { n.saturating_sub(1) } else { n + 1 }
    })
}

/// The pages the checks look at, and the index should list (paths under the pages root): not the index itself, a
/// taxonomy or a folder's README.
pub fn in_scope(v: &Vault) -> impl Fn(&str) -> bool {
    let index_page = v.path("index").and_then(|i| super::scan::page_rel(v, &i));
    let taxonomies: BTreeSet<String> = v.detected.taxonomies.iter().filter_map(|t| super::scan::page_rel(v, t)).collect();
    move |p: &str| Some(p) != index_page.as_deref() && !taxonomies.contains(p) && !p.rsplit('/').next().unwrap_or(p).eq_ignore_ascii_case("README.md")
}

pub fn run(v: &Vault, ix: &Index) -> Checks {
    let in_scope = in_scope(v);
    let r = Resolver::new(ix);
    let mut c = Checks::default();
    let index = v.path("index");
    let index_page = index.as_deref().and_then(|i| super::scan::page_rel(v, i));
    let (mut incoming, mut outgoing) = (BTreeSet::new(), BTreeSet::new());
    for (from, p) in &ix.pages {
        for l in &p.links {
            match r.resolve(from, l) {
                Some(to) if &to != from => {
                    incoming.insert(to);
                    outgoing.insert(from.clone());
                }
                Some(_) => {}
                None if not_a_page(l) => {}
                // The index's own links are its rows: `dead_rows`.
                None if index_page.as_ref() == Some(from) => {}
                None => {
                    if file_exists(v, dir_of(&super::scan::vault_rel(v, from)), l) != Some(true) {
                        c.broken.push((from.clone(), Resolver::bare(l)));
                        c.broken_links.push((from.clone(), l.clone()));
                    }
                }
            }
        }
    }
    match index_targets(v, ix) {
        Some(targets) => {
            let dir = dir_of(index.as_deref().unwrap_or(""));
            let listed: BTreeSet<&String> = targets.iter().filter_map(|(_, to)| to.as_ref()).collect();
            c.not_in_index = ix.pages.keys().filter(|p| in_scope(p) && !listed.contains(*p)).cloned().collect();
            c.dead_rows = targets.iter().filter(|(l, to)| to.is_none() && !not_a_page(l) && file_exists(v, dir, l) != Some(true)).map(|(l, _)| l.clone()).collect();
        }
        None => c.no_index = true,
    }
    if let Some(i) = &index {
        let full = super::join(&v.root, i);
        let bytes = std::fs::metadata(&full).map(|m| m.len()).unwrap_or(0);
        let rows = super::scan::read_capped(&full).map(|t| table_rows(&t)).unwrap_or(0);
        if bytes > INDEX_MAX_BYTES || rows > INDEX_MAX_ROWS {
            c.oversized_index = Some(Oversized { bytes, rows });
        }
    }
    let scoped: Vec<&super::scan::Page> = ix.pages.values().filter(|p| in_scope(&p.path)).collect();
    // A key at least 80 % of a folder's pages have (folders of 5 or more), missing from a page that has properties.
    let mut folders: BTreeMap<&str, Vec<&super::scan::Page>> = BTreeMap::new();
    for p in &scoped {
        folders.entry(dir_of(&p.path)).or_default().push(p);
    }
    for pages in folders.values().filter(|ps| ps.len() >= 5) {
        let mut have: BTreeMap<&str, usize> = BTreeMap::new();
        for p in pages {
            for k in p.props.iter().map(|(k, _)| k.as_str()).filter(|k| !k.is_empty()).collect::<BTreeSet<_>>() {
                *have.entry(k).or_default() += 1;
            }
        }
        for (k, _) in have.iter().filter(|(_, n)| **n * 5 >= pages.len() * 4) {
            c.usual_props.extend(pages.iter().filter(|p| p.has_props && !p.props.iter().any(|(pk, _)| pk == k)).map(|p| (p.path.clone(), k.to_string())));
        }
    }
    c.usual_props.sort();
    let mut titles: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in &scoped {
        let t = p.title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        if !t.is_empty() {
            titles.entry(t).or_default().push(p.path.clone());
        }
    }
    c.duplicate_titles = titles.into_values().filter(|ps| ps.len() > 1).collect();
    c.duplicate_titles.sort();
    c.isolated = ix.pages.keys().filter(|p| in_scope(p) && !incoming.contains(*p) && !outgoing.contains(*p)).cloned().collect();
    c.no_props = ix.pages.values().filter(|p| in_scope(&p.path) && !p.has_props).map(|p| p.path.clone()).collect();
    if let Some(text) = tag_taxonomy(v).and_then(|t| std::fs::read_to_string(super::join(&v.root, &t)).ok()) {
        let allowed: BTreeSet<String> = allowed_tags(&text).into_iter().map(|t| t.to_lowercase()).collect();
        for p in ix.pages.values().filter(|p| in_scope(&p.path)) {
            c.unknown_tags.extend(p.tags.iter().filter(|t| !allowed.contains(&t.to_lowercase())).map(|t| (p.path.clone(), t.clone())));
        }
        let mt = meeting_tag(v);
        if !allowed.contains(&mt.to_lowercase()) {
            c.meeting_tag_missing = Some(mt);
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge::{self, fixture, scan, AddOpts};

    #[test]
    fn allowed_tags_are_the_backticked_words() {
        let t = "| `meeting` | x |\n| `ai-strategy` | y |\n```\nmeeting, concept\n```\nNot `Two Words` nor `#x`.\n";
        assert_eq!(allowed_tags(t).into_iter().collect::<Vec<_>>(), ["ai-strategy", "concept", "meeting"], "backticked words and the words of a fenced list");
    }

    #[test]
    fn checks_count_only_pages() {
        let (_env, root) = crate::paths::testenv::folders("kn-checks");
        let t = fixture::twin(&root.join("twin"));
        // Two pages the index doesn't list (one isolated, one without properties), a broken link, an unknown tag.
        fixture::write(&t, "knowledge/topics/lonely.md", "---\ntitle: Lonely\ntags: [concept]\n---\n# Lonely\n");
        fixture::write(&t, "knowledge/topics/bare.md", "# Bare\nLinks to [[missing-page]] and [Acme](../clients/acme.md).\n");
        fixture::write(&t, "knowledge/clients/acme.md", "---\ntitle: Acme Corp\ntags: [client, invented]\n---\n# Acme Corp\n\nSee [[2026-01-01-a]].\n");
        fixture::write(&t, "knowledge/topics/README.md", "# Topics\n");
        let v = knowledge::add(&t, AddOpts { name: Some("T".into()), ..Default::default() }).unwrap();
        let c = run(&v, &scan::index(&v).unwrap());
        assert_eq!(c.not_in_index, ["notes/2026-01-03-c.md", "topics/bare.md", "topics/lonely.md"]);
        assert_eq!(c.broken, [("topics/bare.md".to_string(), "missing-page".to_string())]);
        assert_eq!(c.isolated, ["notes/2026-01-03-c.md", "topics/lonely.md"]);
        assert_eq!(c.no_props, ["topics/bare.md"]);
        assert_eq!(c.unknown_tags, [("clients/acme.md".to_string(), "invented".to_string())]);
        assert_eq!(c.meeting_tag_missing, None);
        assert!(!c.no_index);
    }

    #[test]
    fn links_to_the_vaults_other_files_or_outside_it_are_not_broken() {
        let (_env, root) = crate::paths::testenv::folders("kn-checks-out");
        let t = fixture::twin(&root.join("twin"));
        fixture::write(&t, "teams/CATALOG.md", "# Teams\n");
        fixture::write(&t, "knowledge/topics/links.md", "---\ntitle: L\n---\n[teams](../../teams/CATALOG.md) [folder](../../teams/) [elsewhere](../../../other-repo/spec.md) [gone](../../teams/missing.md)\n");
        let v = knowledge::add(&t, AddOpts { name: Some("T".into()), ..Default::default() }).unwrap();
        let c = run(&v, &scan::index(&v).unwrap());
        let broken: Vec<&str> = c.broken.iter().filter(|(p, _)| p == "topics/links.md").map(|(_, l)| l.as_str()).collect();
        assert_eq!(broken, ["../../teams/missing.md"], "only the missing file in the vault");
    }

    #[test]
    fn a_missing_meeting_tag_is_named() {
        let (_env, root) = crate::paths::testenv::folders("kn-mtag");
        let t = fixture::twin(&root.join("twin"));
        fixture::write(&t, "knowledge/TAG_TAXONOMY.md", "`call`, `concept`\n");
        let v = knowledge::add(&t, AddOpts { name: Some("T".into()), ..Default::default() }).unwrap();
        assert_eq!(meeting_tag(&v), "meeting");
        assert_eq!(run(&v, &scan::index(&v).unwrap()).meeting_tag_missing.as_deref(), Some("meeting"));
        fixture::write(&t, "knowledge/TAG_TAXONOMY.md", "`meetings`, `concept`\n");
        assert_eq!(meeting_tag(&v), "meetings", "the taxonomy's own meeting tag");
    }
}

//! A vault's pages as the window and the CLI use them (spec K5, K6, K8-05): read one as safe HTML with its links,
//! backlinks and local graph; save an edit only over the text it was opened with; make a new page from the vault's
//! template; its git history; images from inside the vault; rows for pages missing from the index. Paths are
//! vault-relative with `/`; every one is checked before it is joined (untrusted-text R4).

use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::links::Resolver;
use super::scan::{self, RawLink};
use super::{catalog, checks, fm, git, Vault};

/// The most a page may be for Echo to edit it (a bigger one is read only).
const EDIT_MAX: u64 = scan::READ_MAX;
/// Images are served up to this size.
const ASSET_MAX: u64 = 10 << 20;

fn sha(b: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// `rel` checked and joined to the vault's root: relative, plain components only (no `..`, no drive, no Windows device
/// name), and — after links are followed — still inside the vault.
pub fn inside(v: &Vault, rel: &str) -> Result<PathBuf> {
    let rel = rel.trim();
    if rel.is_empty() || rel.contains('\\') || rel.contains(':') || rel.contains('\0') {
        bail!("not a path in this vault");
    }
    let p = Path::new(rel);
    if p.is_absolute() || !p.components().all(|c| matches!(c, Component::Normal(_))) {
        bail!("not a path in this vault");
    }
    let device = |c: &str| {
        let stem = c.split('.').next().unwrap_or("").to_ascii_uppercase();
        matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem.as_bytes()[3].is_ascii_digit())
    };
    if rel.split('/').any(device) {
        bail!("not a path in this vault");
    }
    let root = dunce::canonicalize(&v.root)?;
    let full = super::join(&root, rel);
    if full.exists() {
        let real = dunce::canonicalize(&full)?;
        if !real.starts_with(&root) {
            bail!("that leads out of the vault");
        }
    } else if let Some(there) = full.ancestors().skip(1).find(|p| p.exists()) {
        // The nearest folder that is there: a link above a folder still to be made can't lead out either.
        if !dunce::canonicalize(there)?.starts_with(&root) {
            bail!("that leads out of the vault");
        }
    }
    Ok(full)
}

/// Files Echo reads and edits as pages: the vault's pages, its rules and vocabulary, its log and its skills.
fn editable(v: &Vault, rel: &str) -> bool {
    if scan::is_page_path(v, rel) {
        return true;
    }
    let d = &v.detected;
    d.schema.iter().chain(&d.taxonomies).chain(d.log.iter()).chain(v.path("index").iter()).any(|p| p == rel) || d.skills.iter().any(|s| s.path == rel)
}

fn checked(v: &Vault, rel: &str) -> Result<PathBuf> {
    if !super::available(v) {
        bail!("the vault's folder isn't there: {}", v.root.display());
    }
    let full = inside(v, rel)?;
    if !editable(v, rel) {
        bail!("{rel} isn't a page of this vault");
    }
    Ok(full)
}

/// A vault path in a `#kn:` link: its own `%`, spaces and the like encoded, its slashes kept.
fn link_part(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn url_part(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// `obsidian://open?path=…` for a file.
pub fn obsidian_uri(full: &Path) -> String {
    format!("obsidian://open?path={}", url_part(&full.to_string_lossy()))
}

const IMAGES: [(&str, &str); 5] = [("png", "image/png"), ("jpg", "image/jpeg"), ("jpeg", "image/jpeg"), ("gif", "image/gif"), ("webp", "image/webp")];

fn image_mime(rel: &str) -> Option<&'static str> {
    let ext = rel.rsplit_once('.')?.1.to_ascii_lowercase();
    IMAGES.iter().find(|(e, _)| *e == ext).map(|(_, m)| *m)
}

/// An attachment named by an embed (`![[chart.png]]`): beside the page, in Obsidian's attachments folder, else the
/// shortest path holding that name. Vault-relative.
fn find_attachment(v: &Vault, from_vault: &str, name: &str) -> Option<String> {
    let name = super::links::percent_decode(name.trim());
    let dir = from_vault.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut candidates = vec![super::links::normalize(&format!("{dir}/{name}"))?];
    if let Some(att) = v.detected.obsidian.as_ref().map(|o| o.attachments.trim_matches('/').to_string()).filter(|a| !a.is_empty() && !a.starts_with('.')) {
        candidates.push(scan::vault_rel(v, &format!("{att}/{name}")));
    }
    if let Some(c) = candidates.iter().find(|c| inside(v, c).is_ok_and(|p| p.is_file())) {
        return Some(c.clone());
    }
    if name.contains('/') {
        return None;
    }
    let base = v.pages_dir();
    let mut found = super::detect::walk(&base, &[], |r| r.rsplit('/').next() == Some(name.as_str()));
    found.sort_by_key(|f| (f.matches('/').count(), f.len()));
    found.first().map(|f| scan::vault_rel(v, f))
}

/// The page's link or image, for the safe writer: a page of the vault → `#kn:<path>`; a missing page → `#kn-new:<name>`;
/// web and Echo's own links as they are; an image of the vault → its asset route. Anything else: text.
fn decide(v: &Vault, r: &Resolver, from_page: &str, dest: &str, image: bool) -> Option<String> {
    let from_vault = scan::vault_rel(v, from_page);
    let (wiki, target) = match dest.strip_prefix("wiki:") {
        Some(t) => (true, super::links::percent_decode(t)),
        None => (false, dest.to_string()),
    };
    if !wiki && (target.starts_with("https://") || target.starts_with("http://")) {
        return (!image).then_some(target);
    }
    if !wiki && crate::links::Link::parse(&target).is_some() {
        return (!image).then_some(target);
    }
    if !wiki && target.split_once(':').is_some_and(|(s, _)| s.len() > 1 && s.chars().all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))) {
        return None;
    }
    let raw = RawLink { target: target.clone(), line: 0, wiki, embed: image };
    if image {
        let bare = Resolver::bare(&raw);
        let rel = if wiki { find_attachment(v, &from_vault, &bare)? } else {
            let dir = from_vault.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            super::links::normalize(&format!("{dir}/{bare}"))?
        };
        image_mime(&rel)?;
        inside(v, &rel).ok().filter(|p| p.is_file())?;
        return Some(format!("api/kn/{}/asset?path={}", v.id, url_part(&rel)));
    }
    match r.resolve(from_page, &raw) {
        Some(p) => Some(format!("#kn:{}", link_part(&scan::vault_rel(v, &p)))),
        None => {
            let bare = Resolver::bare(&raw);
            let is_page_like = wiki || bare.to_ascii_lowercase().ends_with(".md") || !bare.rsplit('/').next().unwrap_or("").contains('.');
            (is_page_like && !bare.is_empty()).then(|| format!("#kn-new:{}", link_part(bare.trim_end_matches(".md"))))
        }
    }
}

/// Text being edited, as the reader would show it (the editor's preview): nothing is written.
pub fn render(v: &Vault, rel: &str, text: &str) -> Result<String> {
    checked(v, rel)?;
    let ix = scan::index(v)?;
    let page_rel = scan::page_rel(v, rel).unwrap_or_else(|| rel.to_string());
    let r = Resolver::new(&ix);
    let (_, body) = fm::split(text);
    Ok(crate::minutes::page_html(body, line_breaks(v), &|dest, image| decide(v, &r, &page_rel, dest, image)))
}

/// Model text (an Ask answer) as the vault-page writer shows it: raw HTML as text, links decided as in a page. A `code`
/// span whose whole text is an existing page of the vault (vault-relative, or relative to the pages root) links like
/// a wikilink to it; any other `code` stays code. No frontmatter handling. Model text reaches HTML only through here
/// (untrusted-text R3).
#[allow(dead_code)] // the Ask chat view (plan 2, Task 5) is its first caller
pub fn render_fragment(v: &Vault, text: &str) -> String {
    let linked = link_code_paths(v, text);
    let breaks = line_breaks(v);
    match scan::index(v) {
        Ok(ix) => {
            let r = Resolver::new(&ix);
            crate::minutes::page_html(&linked, breaks, &|dest, image| decide(v, &r, "", dest, image))
        }
        Err(_) => crate::minutes::page_html(&linked, breaks, &|dest, image| (!image && dest.starts_with("https://")).then(|| dest.to_string())),
    }
}

/// `` `knowledge/x.md` `` → `[[x.md|knowledge/x.md]]` when that names an existing page; fenced code and every other
/// span are left alone.
fn link_code_paths(v: &Vault, text: &str) -> String {
    let page_of = |s: &str| -> Option<String> {
        let s = s.trim();
        if s.is_empty() || s.len() > 300 || s.contains(['[', ']', '|', '\\', '\n']) || s.starts_with('/') || s.split('/').any(|c| c.is_empty() || c == ".." || c == ".") {
            return None;
        }
        [s.to_string(), scan::vault_rel(v, s)].into_iter().find(|c| scan::is_page_path(v, c) && inside(v, c).is_ok_and(|p| p.is_file())).and_then(|c| scan::page_rel(v, &c))
    };
    let mut out = String::with_capacity(text.len());
    let mut fence = false;
    for l in text.split_inclusive('\n') {
        let t = l.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
        }
        if fence || t.starts_with("```") || t.starts_with("~~~") {
            out.push_str(l);
            continue;
        }
        let mut rest = l;
        while let Some(i) = rest.find('`') {
            let run = rest[i..].chars().take_while(|&c| c == '`').count();
            let (before, after) = rest.split_at(i + run);
            out.push_str(before);
            rest = after;
            // Only a single-backtick span can be a path; longer runs are copied.
            let Some(j) = rest.find('`').filter(|_| run == 1) else { continue };
            if rest[j..].starts_with("``") {
                continue;
            }
            match page_of(&rest[..j]) {
                Some(page) => {
                    out.truncate(out.len() - 1);
                    out.push_str(&format!("[[{page}|{}]]", &rest[..j]));
                }
                None => out.push_str(&rest[..j + 1]),
            }
            rest = &rest[j + 1..];
        }
        out.push_str(rest);
    }
    out
}

/// A page's first paragraph of prose, as plain text (not a heading, table, code, rule or a lone image), at most 200
/// characters.
fn first_prose(body: &str) -> String {
    let mut fence = false;
    let mut para: Vec<String> = vec![];
    for l in body.lines() {
        let t = l.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        let skip = fence || t.starts_with('#') || t.starts_with('|') || t.starts_with("<!--") || t.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '));
        if t.is_empty() || skip {
            if !para.is_empty() {
                break;
            }
            continue;
        }
        let plain = plain_line(t);
        if !plain.is_empty() {
            para.push(plain);
        }
    }
    let text = para.join(" ");
    if text.chars().count() > 200 {
        let cut: String = text.chars().take(199).collect();
        format!("{}…", cut.rsplit_once(' ').map_or(cut.as_str(), |(a, _)| a).trim_end())
    } else {
        text
    }
}

/// Does a newline show as a line break here: in an Obsidian vault, unless its "Strict line breaks" is on.
fn line_breaks(v: &Vault) -> bool {
    v.detected.obsidian.as_ref().is_some_and(|o| !o.strict_line_breaks)
}

/// One line of Markdown as plain text, for a backlink's context: links become their text, images go, emphasis and
/// code marks go, a table row's cells are joined by " · ", list, quote and heading marks go.
pub(super) fn plain_line(line: &str) -> String {
    let t = line.trim();
    if t.contains('|') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) {
        return String::new();
    }
    let mut t = t;
    loop {
        let s = t.trim_start_matches('#').trim_start();
        let s = ["> ", "- ", "* ", "+ ", ">"].iter().find_map(|m| s.strip_prefix(m)).unwrap_or(s);
        let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
        let s = if digits > 0 { s[digits..].strip_prefix(". ").or_else(|| s[digits..].strip_prefix(") ")).unwrap_or(s) } else { s };
        let s = s.trim_start();
        if s.len() == t.len() {
            break;
        }
        t = s;
    }
    let c: Vec<char> = t.chars().collect();
    let close = |from: usize, pat: &str| -> Option<usize> {
        let p: Vec<char> = pat.chars().collect();
        (from..c.len()).find(|&i| c[i..].starts_with(&p))
    };
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        let embed = c[i] == '!' && c.get(i + 1).is_some_and(|&n| n == '[');
        let at = if embed { i + 1 } else { i };
        if c[at] == '[' && c.get(at + 1) == Some(&'[') {
            if let Some(end) = close(at + 2, "]]") {
                if !embed {
                    let inner: String = c[at + 2..end].iter().collect::<String>().replace("\\|", "|");
                    let (target, alias) = inner.split_once('|').map_or((inner.as_str(), None), |(a, b)| (a, Some(b)));
                    out.push_str(alias.unwrap_or_else(|| {
                        let name = target.split('#').next().unwrap_or(target);
                        let name = name.rsplit('/').next().unwrap_or(name);
                        name.strip_suffix(".md").unwrap_or(name)
                    }));
                }
                i = end + 2;
                continue;
            }
        }
        if c[at] == '[' {
            if let Some(mid) = close(at + 1, "](") {
                let end = if c.get(mid + 2) == Some(&'<') { close(mid + 3, ">)").map(|e| e + 1) } else { close(mid + 2, ")") };
                if let Some(end) = end {
                    if !embed {
                        out.extend(&c[at + 1..mid]);
                    }
                    i = end + 1;
                    continue;
                }
            }
        }
        if c[i] == '`' || (matches!(c[i], '*' | '_') && c.get(i + 1) == Some(&c[i])) {
            i += if c[i] == '`' { 1 } else { 2 };
            continue;
        }
        out.push(c[i]);
        i += 1;
    }
    let out = if out.trim_start().starts_with('|') {
        out.split('|').map(str::trim).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")
    } else {
        out
    };
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A page as the window shows it.
pub fn read(v: &Vault, rel: &str) -> Result<Value> {
    let full = checked(v, rel)?;
    let bytes = std::fs::read(&full).map_err(|e| anyhow!("{rel}: {e}"))?;
    let size = bytes.len() as u64;
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(scan::READ_MAX as usize)]).into_owned();
    let ix = scan::index(v)?;
    let page_rel = scan::page_rel(v, rel).unwrap_or_else(|| rel.to_string());
    let p = scan::page(&page_rel, &text, size, 0);
    let (_, body) = fm::split(&text);
    let r = Resolver::new(&ix);
    let html = crate::minutes::page_html(body, line_breaks(v), &|dest, image| decide(v, &r, &page_rel, dest, image));
    // Backlinks with the line each one is on.
    let back: Vec<Value> = super::links::backlinks(&ix, &page_rel)
        .into_iter()
        .take(200)
        .map(|(from, line)| {
            let from_v = scan::vault_rel(v, &from);
            let text = scan::read_capped(&super::join(&v.root, &from_v)).unwrap_or_default();
            let l = plain_line(text.lines().nth(line.saturating_sub(1)).unwrap_or("")).chars().take(240).collect::<String>();
            json!({ "path": from_v, "title": ix.pages.get(&from).map(|p| p.title.clone()).unwrap_or_default(), "line": l })
        })
        .collect();
    let mut seen = BTreeSet::new();
    let out: Vec<Value> = p.links.iter().filter(|l| !l.embed).filter_map(|l| {
        let to = r.resolve(&page_rel, l).map(|t| scan::vault_rel(v, &t));
        let bare = Resolver::bare(l);
        seen.insert(to.clone().unwrap_or_else(|| bare.clone())).then(|| json!({ "target": bare, "path": to, "title": to.as_ref().and_then(|t| scan::page_rel(v, t)).and_then(|t| ix.pages.get(&t).map(|p| p.title.clone())) }))
    }).collect();
    // The local graph: this page, what it links and what links it.
    let mut nodes = vec![json!({ "id": rel, "title": p.title, "me": true })];
    let mut edges = vec![];
    for o in out.iter().filter_map(|o| o["path"].as_str()) {
        if o != rel && !nodes.iter().any(|n| n["id"] == o) {
            nodes.push(json!({ "id": o, "title": scan::page_rel(v, o).and_then(|t| ix.pages.get(&t).map(|p| p.title.clone())).unwrap_or_default() }));
        }
        edges.push(json!([rel, o]));
    }
    for b in &back {
        let b = b["path"].as_str().unwrap_or("");
        if !nodes.iter().any(|n| n["id"] == b) {
            nodes.push(json!({ "id": b, "title": scan::page_rel(v, b).and_then(|t| ix.pages.get(&t).map(|p| p.title.clone())).unwrap_or_default() }));
        }
        edges.push(json!([b, rel]));
    }
    let in_index = checks::index_links(v, &ix).filter(|_| checks::in_scope(v)(&page_rel)).map(|l| l.contains(&page_rel));
    let mtime = std::fs::metadata(&full).ok().and_then(|m| m.modified().ok()).map(|t| chrono::DateTime::<chrono::Local>::from(t).to_rfc3339());
    Ok(json!({
        "path": rel,
        "title": p.title,
        "props": p.props.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        "has_props": p.has_props,
        "tags": p.tags,
        "html": html,
        "outline": p.headings.iter().map(|(l, h)| json!([l, h])).collect::<Vec<_>>(),
        "backlinks": back,
        "links": out,
        "local": { "nodes": nodes, "edges": edges },
        "base": sha(&bytes),
        "text": if size <= EDIT_MAX { Value::String(String::from_utf8_lossy(&bytes).into_owned()) } else { Value::Null },
        "editable": size <= EDIT_MAX && std::str::from_utf8(&bytes).is_ok(),
        // The index lists pages; it isn't one of them.
        "in_index": in_index,
        "is_index": v.path("index").as_deref() == Some(rel),
        "git": git::info(&v.root).is_some(),
        "mtime": mtime,
        "obsidian": obsidian_uri(&full),
        "abs": full,
    }))
}

/// The file's line ending, applied to text a textarea handed back with `\n`.
fn with_line_endings(old: &[u8], text: &str) -> String {
    if old.windows(2).any(|w| w == b"\r\n") {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_string()
    }
}

fn commit_one(v: &Vault, paths: &[PathBuf], dirty: &[PathBuf], message: &str) -> String {
    if !v.git.commit {
        return git::Commit::Off.label();
    }
    let clean: Vec<PathBuf> = paths.iter().filter(|p| !dirty.contains(p)).cloned().collect();
    if clean.is_empty() {
        return git::Commit::Skipped("the file had changes of yours that weren't committed".into()).label();
    }
    git::commit_paths(&v.root, &clean, message).label()
}

/// Save an edit, only over the text it was opened with (`base`, its sha256). Never a silent overwrite.
pub fn save(v: &Vault, rel: &str, text: &str, base: &str, message: Option<&str>) -> Result<Value> {
    super::refuse_headless()?;
    let full = checked(v, rel)?;
    // git is asked first: between the check that the file is as it was opened and the write, nothing slow runs.
    let dirty = if v.git.commit { git::dirty(&v.root, std::slice::from_ref(&full)) } else { vec![] };
    let old = std::fs::read(&full).map_err(|e| anyhow!("{rel}: {e}"))?;
    if sha(&old) != base {
        bail!("changed_on_disk: {rel} changed since it was opened");
    }
    if old.len() as u64 > EDIT_MAX {
        bail!("{rel} is too large to edit in Echo");
    }
    if std::str::from_utf8(&old).is_err() {
        bail!("{rel} isn't UTF-8 text, so Echo doesn't edit it");
    }
    let text = with_line_endings(&old, text);
    crate::paths::write_atomic(&full, text.as_bytes())?;
    let message = message.map(str::trim).filter(|m| !m.is_empty()).map(str::to_owned).unwrap_or_else(|| format!("Edit {rel}"));
    let commit = commit_one(v, std::slice::from_ref(&full), &dirty, &message);
    super::activity::changed(v, rel, "editor", None, std::str::from_utf8(&old).ok(), &text, super::activity::commit_id(&commit));
    Ok(json!({ "path": rel, "base": sha(text.as_bytes()), "commit": commit }))
}

/// Obsidian's template fields: `{{title}}`, `{{date}}`, `{{date:FORMAT}}`, `{{time}}`.
fn fill_template(t: &str, title: &str) -> String {
    let now = chrono::Local::now();
    let mut out = t.replace("{{title}}", title).replace("{{date}}", &now.format("%Y-%m-%d").to_string()).replace("{{time}}", &now.format("%H:%M").to_string());
    while let Some(i) = out.find("{{date:") {
        let Some(j) = out[i..].find("}}") else { break };
        let fmt = &out[i + 7..i + j];
        let chrono_fmt = fmt.replace('%', "%%").replace("YYYY", "%Y").replace("MM", "%m").replace("DD", "%d").replace("HH", "%H").replace("mm", "%M");
        let mut date = String::new();
        if std::fmt::Write::write_fmt(&mut date, format_args!("{}", now.format(&chrono_fmt))).is_err() {
            date = fmt.to_string();
        }
        out.replace_range(i..i + j + 2, &date);
    }
    out
}

/// The index row for a page (vault-relative), and the section it goes in.
fn row_for(v: &Vault, rel: &str) -> Result<(String, String, String, String)> {
    let index = v.path("index").ok_or_else(|| anyhow!("the vault has no index"))?;
    let full = inside(v, rel)?;
    let text = scan::read_capped(&full).unwrap_or_default();
    let p = scan::page(rel, &text, 0, 0);
    let summary = ["summary", "description"].iter().find_map(|k| fm::get(&p.props, k).map(|s| s.text()).filter(|s| !s.trim().is_empty())).unwrap_or_else(|| first_prose(fm::split(&text).1));
    let desc = if summary.is_empty() || summary == p.title { p.title.clone() } else { format!("{} — {summary}", p.title) };
    let date = fm::get(&p.props, "date").map(|d| d.text()).filter(|d| d.len() >= 10 && d.as_bytes()[4] == b'-').map(|d| d[..10].to_string())
        .or_else(|| std::fs::metadata(&full).ok().and_then(|m| m.modified().ok()).map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d").to_string()))
        .unwrap_or_else(today);
    let page_rel = scan::page_rel(v, rel).unwrap_or_else(|| rel.to_string());
    let folder = page_rel.split_once('/').map(|(f, _)| f.to_string()).unwrap_or_default();
    let index_dir = index.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    let rel_to_index = |x: &str| if index_dir.is_empty() { x.to_string() } else { x.strip_prefix(&format!("{index_dir}/")).map(str::to_owned).unwrap_or_else(|| x.to_string()) };
    let folder_vault = scan::vault_rel(v, &folder);
    Ok((rel_to_index(&folder_vault), rel_to_index(rel), catalog::cell(&desc), date))
}

/// Make a new page in `folder` (vault-relative) named `name`, from a template of the vault or its page format; with a
/// row in the index when asked (and the index has a section for that folder).
pub fn create(v: &Vault, folder: &str, name: &str, template: Option<&str>, add_row: bool) -> Result<String> {
    super::refuse_headless()?;
    let title = name.trim();
    if title.is_empty() {
        bail!("a page needs a name");
    }
    let folder = folder.trim_matches('/');
    if !folder.is_empty() {
        inside(v, folder)?;
    }
    // The folder's own naming: kebab-case unless its pages keep spaces or capitals.
    let dir = super::join(&v.root, folder);
    let names: Vec<String> = std::fs::read_dir(&dir).map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".md") && !n.eq_ignore_ascii_case("README.md")).collect()).unwrap_or_default();
    let as_typed = !names.is_empty() && names.iter().filter(|n| n.contains(' ') || n.chars().any(|c| c.is_uppercase())).count() * 2 > names.len();
    let stem = if as_typed { title.chars().map(|c| if "/\\:*?\"<>|#".contains(c) { '-' } else { c }).collect::<String>() } else { crate::recorder::slugify(title) };
    let stem = if stem.is_empty() { "page".to_string() } else { stem };
    let rel = if folder.is_empty() { format!("{stem}.md") } else { format!("{folder}/{stem}.md") };
    let full = inside(v, &rel)?;
    if full.exists() {
        bail!("{rel} already exists");
    }
    if !scan::is_page_path(v, &rel) {
        bail!("{folder} isn't a folder of pages");
    }
    let text = match template {
        Some(t) => {
            let tp = inside(v, t)?;
            fill_template(&std::fs::read_to_string(&tp).map_err(|e| anyhow!("{t}: {e}"))?, title)
        }
        None => format!("---\ntitle: {}\ndate: {}\ntags: []\nsource: \nsummary: \"\"\n---\n\n# {title}\n", super::export::yaml(title), today()),
    };
    let index = v.path("index").map(|i| super::join(&v.root, &i));
    let mut targets = vec![full.clone()];
    targets.extend(index.iter().cloned());
    let dirty = if v.git.commit { git::dirty(&v.root, &targets) } else { vec![] };
    std::fs::create_dir_all(full.parent().expect("a parent"))?;
    crate::paths::write_atomic(&full, text.as_bytes())?;
    let mut written = vec![full];
    if add_row {
        if let (Some(ipath), Ok((folder_i, note_i, desc, date))) = (index, row_for(v, &rel)) {
            if let Ok(old) = std::fs::read_to_string(&ipath) {
                if let Ok(Some(new)) = catalog::insert(&old, &folder_i, &note_i, &desc, &date, &today()) {
                    crate::paths::write_atomic(&ipath, new.as_bytes())?;
                    written.push(ipath);
                }
            }
        }
    }
    let commit = commit_one(v, &written, &dirty, &format!("Add {rel}"));
    super::activity::changed(v, &rel, "new-page", None, None, &text, super::activity::commit_id(&commit));
    Ok(rel)
}

/// The page's versions: its git log (following renames), newest first.
pub fn history(v: &Vault, rel: &str) -> Value {
    if checked(v, rel).is_err() || git::info(&v.root).is_none() {
        return json!({ "no_history": true, "versions": [] });
    }
    let log = git::out(&v.root, &["log", "--follow", "-n", "50", "--format=%x1e%h%x1f%an%x1f%aI%x1f%s", "--numstat", "--", rel]).unwrap_or_default();
    let versions: Vec<Value> = log.split('\u{1e}').filter(|c| !c.trim().is_empty()).filter_map(|c| {
        let mut lines = c.lines();
        let head: Vec<&str> = lines.next()?.split('\u{1f}').collect();
        let (mut added, mut removed) = (0u64, 0u64);
        for l in lines.filter(|l| !l.trim().is_empty()) {
            let mut f = l.split('\t');
            added += f.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            removed += f.next().and_then(|x| x.parse().ok()).unwrap_or(0);
        }
        Some(json!({ "rev": head.first()?, "author": head.get(1)?, "at": head.get(2)?, "message": head.get(3).unwrap_or(&""), "added": added, "removed": removed }))
    }).collect();
    json!({ "no_history": false, "versions": versions })
}

fn valid_rev(rev: &str) -> Result<()> {
    if rev.is_empty() || rev.len() > 64 || !rev.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("not a version");
    }
    Ok(())
}

/// One version of a page: its text and what that commit changed in it.
pub fn version(v: &Vault, rel: &str, rev: &str) -> Result<Value> {
    checked(v, rel)?;
    valid_rev(rev)?;
    // The path at that commit (a rename since): the one git's log names.
    let at = git::out(&v.root, &["log", "--follow", "--name-only", "--format=%h", "--", rel]).unwrap_or_default();
    let mut path_then = rel.to_string();
    let mut lines = at.lines().filter(|l| !l.trim().is_empty());
    while let Some(h) = lines.next() {
        let p = lines.next().unwrap_or(rel);
        if rev.starts_with(h) || h.starts_with(rev) {
            path_then = p.to_string();
            break;
        }
    }
    let top = git::info(&v.root).ok_or_else(|| anyhow!("the vault isn't in a git repository"))?.top;
    let root = dunce::canonicalize(&v.root)?;
    let prefix = root.strip_prefix(dunce::canonicalize(&top)?).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
    let in_repo = if prefix.is_empty() { path_then.clone() } else { format!("{prefix}/{path_then}") };
    let text = git::out(&v.root, &["show", &format!("{rev}:{in_repo}")]).ok_or_else(|| anyhow!("that version can't be read"))?;
    let diff = git::out(&v.root, &["show", "--format=", "--no-color", rev, "--", &path_then]).unwrap_or_default();
    Ok(json!({ "path": rel, "rev": rev, "text": text, "diff": diff }))
}

/// Write an old version's text back (a new commit when the vault commits; history is never rewritten).
pub fn restore(v: &Vault, rel: &str, rev: &str) -> Result<Value> {
    super::refuse_headless()?;
    let old = version(v, rel, rev)?;
    let full = checked(v, rel)?;
    // The current text must be in the history before it is replaced.
    let dirty = git::dirty(&v.root, std::slice::from_ref(&full));
    if !dirty.is_empty() {
        bail!("{rel} has changes that are not committed — commit them first, or copy what you need from this version");
    }
    let current = std::fs::read(&full).unwrap_or_default();
    let text = with_line_endings(&current, old["text"].as_str().unwrap_or(""));
    let mut text = text;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    crate::paths::write_atomic(&full, text.as_bytes())?;
    let commit = commit_one(v, std::slice::from_ref(&full), &dirty, &format!("Restore {rel} to {rev}"));
    super::activity::changed(v, rel, "editor", Some(rev), std::str::from_utf8(&current).ok(), &text, super::activity::commit_id(&commit));
    Ok(json!({ "path": rel, "base": sha(text.as_bytes()), "commit": commit }))
}

/// The rows *Add to the index* would write, one per page, with its section (nothing written).
pub fn index_rows(v: &Vault, rels: &[String]) -> Result<Vec<Value>> {
    let index = v.path("index").ok_or_else(|| anyhow!("the vault has no index"))?;
    let text = std::fs::read_to_string(super::join(&v.root, &index))?;
    rels.iter().map(|rel| {
        checked(v, rel)?;
        let (folder_i, note_i, desc, date) = row_for(v, rel)?;
        // Which heading the row would land under: try it on the index as it is.
        let section = match catalog::insert(&text, &folder_i, &note_i, &desc, &date, &today()) {
            Ok(Some(new)) => {
                let at = new.lines().position(|l| l.contains(&format!("]({})", note_i.replace(' ', "%20")))).unwrap_or(0);
                new.lines().take(at).filter(|l| l.trim_start().starts_with('#')).last().map(|h| h.trim_start_matches('#').trim().to_string()).unwrap_or_default()
            }
            Ok(None) => "already listed".into(),
            Err(why) => format!("skipped: {why}"),
        };
        Ok(json!({ "path": rel, "section": section, "row": catalog::row(3, &catalog::link(&note_i), &desc, &date), "desc": desc, "date": date }))
    }).collect()
}

/// Add index rows for `rels`: one write of the index (and a log line), one commit.
pub fn index_add(v: &Vault, rels: &[String]) -> Result<Value> {
    super::refuse_headless()?;
    let index = v.path("index").ok_or_else(|| anyhow!("the vault has no index"))?;
    let ipath = super::join(&v.root, &index);
    let log = v.detected.log.as_ref().map(|l| super::join(&v.root, l));
    let targets: Vec<PathBuf> = std::iter::once(ipath.clone()).chain(log.iter().cloned()).collect();
    let dirty = if v.git.commit { git::dirty(&v.root, &targets) } else { vec![] };
    let mut text = std::fs::read_to_string(&ipath)?;
    let before = text.clone();
    let (mut added, mut skipped) = (0, vec![]);
    for rel in rels {
        checked(v, rel)?;
        let (folder_i, note_i, desc, date) = row_for(v, rel)?;
        match catalog::insert(&text, &folder_i, &note_i, &desc, &date, &today()) {
            Ok(Some(new)) => {
                text = new;
                added += 1;
            }
            Ok(None) => {}
            Err(why) => skipped.push(json!({ "path": rel, "why": why })),
        }
    }
    let mut written = vec![];
    if added > 0 {
        crate::paths::write_atomic(&ipath, text.as_bytes())?;
        written.push(ipath);
        if let Some(l) = log.filter(|l| l.is_file()) {
            let mut t = std::fs::read_to_string(&l)?;
            if !t.ends_with('\n') {
                t.push('\n');
            }
            t.push_str(&format!("- {} · changed · {index} · {added} page{} added to the index by Echo\n", today(), if added == 1 { "" } else { "s" }));
            crate::paths::write_atomic(&l, t.as_bytes())?;
            written.push(l);
        }
    }
    let commit = if written.is_empty() { "off".into() } else { commit_one(v, &written, &dirty, &format!("Echo: {added} page{} added to the index", if added == 1 { "" } else { "s" })) };
    if added > 0 {
        super::activity::changed(v, &index, "index-rows", None, Some(&before), &text, super::activity::commit_id(&commit));
    }
    Ok(json!({ "added": added, "skipped": skipped, "commit": commit }))
}

/// An image of the vault (png, jpg, gif, webp; up to 10 MB).
pub fn asset(v: &Vault, rel: &str) -> Result<(Vec<u8>, &'static str)> {
    let mime = image_mime(rel).ok_or_else(|| anyhow!("not an image Echo shows"))?;
    let full = inside(v, rel)?;
    let m = std::fs::metadata(&full).map_err(|_| anyhow!("no such image"))?;
    if !m.is_file() || m.len() > ASSET_MAX {
        bail!("no such image");
    }
    Ok((std::fs::read(&full)?, mime))
}

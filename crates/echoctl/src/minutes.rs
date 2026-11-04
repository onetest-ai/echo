//! The minutes page: a call's `notes.md` as one self-contained HTML file next to it,
//! `<title>-minutes.html` — header block, tables, timestamps, the call's screenshots embedded
//! (click to enlarge), diagrams, light and dark, print — to share, print or archive.
//! Rendered from the call's files, no model involved. Opt-in (`config.minutes_page`, menu →
//! Claude Code → Write a minutes page): a notes run from the app writes it when it is done
//! (`notes.rs`), a session's notes skill runs `echoctl html <call> --auto`; `echoctl html <call>`
//! writes it any time.
//!
//! Notes hold other people's words, so nothing in them becomes markup: raw HTML is shown as text,
//! links are https only, images are only this call's own frames, and diagrams are drawn by
//! mermaid with `securityLevel: 'strict'`.

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::calls;

/// Diagrams (```mermaid): mermaid 11.17.2, MIT. A page embeds it only when its minutes draw a
/// diagram (the window's Minutes tab shows the page itself).
const MERMAID_JS: &str = include_str!("../ui/vendor/mermaid.min.js");

/// Marks the pages Echo wrote, so a renamed call's old page is replaced, never someone's own file.
const GENERATOR: &str = r#"<meta name="generator" content="Echo minutes">"#;
const SUFFIX: &str = "-minutes.html";
/// A frame bigger than this isn't embedded (Echo's own are a few hundred KB).
const MAX_FRAME: u64 = 20 << 20;
/// All the images of one page together (each frame is embedded once).
const MAX_IMAGES: usize = 64 << 20;

/// A frame the call kept (`frames.jsonl`): its time captions a screenshot the minutes cite.
#[derive(Debug, Clone, PartialEq)]
struct Frame {
    name: String,
    t: f64,
    shared_by: Option<String>,
}

/// `echoctl html <call> [--open] [--auto]`: `--auto` writes only when the setting is on.
pub fn command(call: &str, open: bool, auto: bool) -> Result<()> {
    let dir = calls::resolve_call(call)?;
    if auto && !crate::config::Config::load()?.minutes_page {
        println!("minutes page is off (menu → Claude Code → Write a minutes page)");
        return Ok(());
    }
    let page = write(&dir)?;
    println!("{}", page.display());
    if open {
        crate::app::reveal(&page); // `open` / `explorer`: the default browser shows the file
    }
    Ok(())
}

/// The call's title as Echo shows it (a retitled call's, `echoctl title`), else the meeting's.
fn title_of(dir: &Path) -> String {
    let meta: Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    meta["title"]
        .as_str()
        .filter(|t| !t.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| calls::subject_of(dir))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// The page of the call in `dir` (it needs `notes.md`), and its CSP; `embed`: the copy the
/// window's Minutes tab shows (one more script: timestamps and links talk to the window).
fn page(dir: &Path, embed: bool) -> Result<(String, String)> {
    let md = fs::read_to_string(dir.join("notes.md")).with_context(|| format!("{} has no minutes yet", dir.display()))?;
    let load = |name: &str| frame_bytes(dir, name);
    // The model is named only while the minutes are as the run that wrote them left them.
    Ok(render_page(&md, &title_of(dir), crate::notes::minutes_model(dir).as_deref(), &load, &kept_frames(dir), embed))
}

/// The page for the window's Minutes tab, rendered now (no file written), and its CSP.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn render_for_window(dir: &Path) -> Result<(String, String)> {
    page(dir, true)
}

/// The page in a theme the user chose in Echo's window (`light` / `dark`), whatever the system's: its dark rules are
/// written for the system's appearance (`prefers-color-scheme`), so they are switched on or off as a whole, and the
/// diagrams follow. Anything else: as the system says.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn with_theme(html: String, theme: Option<&str>) -> String {
    let (media, js) = match theme {
        Some("dark") => ("@media all", "true"),
        Some("light") => ("@media not all", "false"),
        _ => return html,
    };
    html.replace("@media (prefers-color-scheme: dark)", media).replace("matchMedia('(prefers-color-scheme: dark)').matches", js)
}

/// Bumped when the page's rendering changes, so an open Minutes tab shows the new one.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
const RENDER_VERSION: u32 = 8;

/// A short hash of exactly what the page is rendered from: the minutes and the kept frames (size
/// and time), the title, the model it names, the renderer. The Minutes tab reloads its frame only
/// when this changes — never for a note, an action ticked or a run's state.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn revision(dir: &Path) -> String {
    let stamp = |p: PathBuf| fs::metadata(p).ok().map(|m| (m.len(), m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos())));
    let input = format!(
        "{RENDER_VERSION}|{:?}|{:?}|{:?}|{}|{:?}",
        stamp(dir.join("notes.md")),
        stamp(dir.join("frames.jsonl")),
        stamp(dir.join("frames")),
        title_of(dir),
        crate::notes::minutes_model(dir),
    );
    Sha256::digest(input.as_bytes()).iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Write the minutes page of the call in `dir` (it needs `notes.md`); returns its path.
pub fn write(dir: &Path) -> Result<PathBuf> {
    let (html, _) = page(dir, false)?;
    let title = title_of(dir);

    // Under the title, unless a file there isn't one of ours: then "<title> (2)-minutes.html", ….
    let stem = file_stem(&title);
    let ours = |p: &Path| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(SUFFIX)) && head_of(p).contains(GENERATOR);
    let page = (1..100)
        .map(|n| dir.join(if n == 1 { format!("{stem}{SUFFIX}") } else { format!("{stem} ({n}){SUFFIX}") }))
        .find(|p| !p.exists() || ours(p))
        .context("no free name for the minutes page")?;
    // A temp file of its own (a run and `echoctl html` may write at once), then the page.
    let (tmp, mut file) = (0..100)
        .find_map(|n| {
            let tmp = dir.join(format!(".minutes-{}-{n}.tmp", std::process::id()));
            fs::OpenOptions::new().write(true).create_new(true).open(&tmp).ok().map(|f| (tmp, f))
        })
        .context("no temp file for the minutes page")?;
    let written = file.write_all(html.as_bytes()).and_then(|_| file.sync_all());
    drop(file);
    if let Err(e) = written.and_then(|_| fs::rename(&tmp, &page)) {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {}", page.display()));
    }
    // Only then the page of an earlier title (the call was renamed since).
    for e in fs::read_dir(dir)?.flatten() {
        let p = e.path();
        if p != page && ours(&p) {
            let _ = fs::remove_file(&p);
        }
    }
    Ok(page)
}

/// The first bytes of a file (where a page's generator tag is).
fn head_of(p: &Path) -> String {
    let mut buf = vec![0; 2048];
    let n = fs::File::open(p).and_then(|mut f| f.read(&mut buf)).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// A title as a file name: no path or reserved characters, one line, ≤ 80 characters, and not a
/// Windows device name before its first dot (`CON.review` would be the console).
fn file_stem(title: &str) -> String {
    let s: String = title.chars().map(|c| if c.is_control() || r#"/\:*?"<>|"#.contains(c) { '-' } else { c }).collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s: String = s.chars().take(80).collect();
    let s = s.trim_matches(['.', ' ', '-']).to_string();
    let base: Vec<char> = s.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase().chars().collect();
    let first3: String = base.iter().take(3).collect();
    let device = (base.len() == 3 && ["CON", "PRN", "AUX", "NUL"].contains(&first3.as_str()))
        || (base.len() == 4 && ["COM", "LPT"].contains(&first3.as_str()) && "0123456789¹²³".contains(base[3]));
    if s.is_empty() { "call".into() } else if device { format!("_{s}") } else { s }
}

/// The frames this call kept (still on disk), in order.
fn kept_frames(dir: &Path) -> Vec<Frame> {
    fs::read_to_string(dir.join("frames.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|f| {
            let name = Path::new(f["file"].as_str()?).file_name()?.to_str()?.to_string();
            dir.join("frames").join(&name).is_file().then(|| Frame {
                name,
                t: f["t"].as_f64().unwrap_or(0.0),
                shared_by: f["shared_by"].as_str().map(str::to_owned),
            })
        })
        .collect()
}

/// A frame's bytes and media type, by file name: one plain name (no separators, no drive or
/// stream colon), a file really inside this call's own `frames/` (a symlink pointing out, or a
/// `frames` that is itself a link elsewhere, is refused),
/// ≤ `MAX_FRAME` bytes, and the image its extension says.
fn frame_bytes(dir: &Path, name: &str) -> Option<(&'static str, Vec<u8>)> {
    let mut parts = Path::new(name).components();
    let plain = matches!((parts.next(), parts.next()), (Some(Component::Normal(_)), None));
    if !plain || name.starts_with('.') || name.contains(['/', '\\', ':']) {
        return None;
    }
    let mime = match Path::new(name).extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        _ => return None,
    };
    let frames = dir.join("frames").canonicalize().ok()?;
    if frames != dir.canonicalize().ok()?.join("frames") {
        return None; // `frames` itself links elsewhere
    }
    let file = frames.join(name).canonicalize().ok()?;
    if file.parent() != Some(frames.as_path()) || !file.metadata().is_ok_and(|m| m.is_file() && m.len() <= MAX_FRAME) {
        return None;
    }
    let bytes = fs::read(&file).ok()?;
    let ok = if mime == "image/png" { bytes.starts_with(b"\x89PNG\r\n\x1a\n") } else { bytes.starts_with(&[0xFF, 0xD8, 0xFF]) };
    ok.then_some((mime, bytes))
}

type Loader<'a> = dyn Fn(&str) -> Option<(&'static str, Vec<u8>)> + 'a;

/// The whole page.
#[cfg(test)]
fn render(md: &str, title: &str, model: Option<&str>, load: &Loader, frames: &[Frame]) -> String {
    render_page(md, title, model, load, frames, false).0
}

/// The page and its CSP (also sent as a header when the window serves it).
fn render_page(md: &str, title: &str, model: Option<&str>, load: &Loader, frames: &[Frame], embed: bool) -> (String, String) {
    let (front, rest) = calls::split_frontmatter(md);
    let (mut meta, rest) = header_block(rest);
    // Then the frontmatter's fields, unless a row already says the same thing (`time` beside the
    // header block's "Time", `minute_taker` beside its "Minutes taken by").
    for key in front.map(yaml_keys).unwrap_or_default() {
        let Some(v) = front.and_then(|f| yaml_value(f, &key)).filter(|v| !v.is_empty()) else { continue };
        let label = if key == "type" { "Meeting type".to_string() } else { label_of(&key) };
        let said = meta.iter().any(|(l, _)| {
            let l = l.to_lowercase();
            l == label.to_lowercase() || same_as(&key).contains(&l.as_str())
        });
        if !said {
            meta.push((label, v));
        }
    }
    let mut images = Images { load, frames, embedded: vec![], left: MAX_IMAGES };
    let rest = attach_screens(&rest, frames);
    let body = body(&rest, &mut images);

    let mut top = String::new();
    if !meta.is_empty() {
        top.push_str("<div class=\"meta\">");
        for (label, value) in &meta {
            top.push_str(&format!("<div><b>{}</b>{}</div>", esc(label), times(&esc(value))));
        }
        top.push_str("</div>");
    }
    let by = model.map(|m| format!("Echo, {}", esc(m))).unwrap_or_else(|| "Echo".into());
    top.push_str(&format!(
        "<p class=\"note\">Written by AI ({by}) from an automatic transcript. Check names, figures and commitments before you share it.</p>"
    ));
    // Under the notes' own title, else under one made from the call's.
    let main = match body.html.find("</h1>") {
        Some(i) => format!("{}{top}{}", &body.html[..i + 5], &body.html[i + 5..]),
        None => format!("<h1>{}</h1>{top}{}", esc(title), body.html),
    };

    // Screenshots only where the minutes cite them (their "On screen" section): one that proves no
    // point isn't added at the end (the window's Screens tab still has every kept frame).
    // Only these exact scripts may run (by hash); images only from the page itself — so a diagram
    // can't fetch a URL (mermaid's `img:` shapes), nor anything else in the page.
    let mut sources = vec![PAGE_JS]; // first: it polices links before mermaid binds any
    if embed {
        sources.push(EMBED_JS);
    }
    if body.mermaid {
        sources.extend([MERMAID_JS, MERMAID_INIT]);
    }
    let scripts = hashed(&sources);
    let hashes: Vec<&str> = scripts.iter().map(|(_, h)| h.as_str()).collect();
    let csp = format!(
        "default-src 'none'; img-src data:; style-src 'unsafe-inline'; form-action 'none'; base-uri 'none'; script-src {}",
        hashes.join(" ")
    );
    let tags: String = scripts.iter().map(|(s, _)| format!("<script>{s}</script>\n")).collect();
    let html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n{GENERATOR}\n\
         <title>{} – minutes</title>\n<style>{CSS}</style>\n{}</head>\n<body>\n<main>\n{main}\n</main>\n\
         <div id=\"lightbox\"><img alt=\"\"></div>\n{tags}</body>\n</html>\n",
        esc(title),
        if embed { format!("<style>{ECHO_CSS}</style>\n<style>{EMBED_CSS}</style>\n") } else { format!("<style>{ECHO_CSS}</style>\n") }
    );
    (html, csp)
}

/// Each script as the page carries it, with its CSP source (`'sha256-…'`). LF only: a browser
/// hashes a script's text with its line endings normalised, and a Windows checkout may give
/// `include_str!` CRLF.
fn hashed(sources: &[&str]) -> Vec<(String, String)> {
    sources
        .iter()
        .map(|s| {
            let s = s.replace("\r\n", "\n").replace('\r', "\n");
            let hash = format!("'sha256-{}'", B64.encode(Sha256::digest(s.as_bytes())));
            (s, hash)
        })
        .collect()
}

/// Seconds into the call as `mm:ss` (`h:mm:ss` past an hour).
fn clock(t: f64) -> String {
    let s = t.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{:02}:{:02}", s / 60, s % 60) }
}

/// The page's images: each frame embedded once, all of them together ≤ `MAX_IMAGES` bytes.
struct Images<'a> {
    load: &'a Loader<'a>,
    frames: &'a [Frame],
    embedded: Vec<String>,
    left: usize,
}

impl Images<'_> {
    /// An `<img>` of a frame not shown yet that still fits.
    fn img(&mut self, name: &str, alt: &str) -> Option<String> {
        if self.embedded.iter().any(|n| n == name) {
            return None;
        }
        let (mime, bytes) = (self.load)(name).filter(|(_, b)| b.len() <= self.left)?;
        self.left -= bytes.len();
        self.embedded.push(name.to_string());
        Some(format!("<img src=\"data:{mime};base64,{}\" alt=\"{}\">", B64.encode(bytes), esc(alt)))
    }

    /// When Echo took the frame, if it logged it.
    fn when(&self, name: &str) -> Option<String> {
        self.frames.iter().find(|f| f.name == name).map(|f| clock(f.t))
    }
}

/// The header block under the title — `> **Project:** Atlas` lines — as (label, value) rows, and
/// the notes without it. Notes without one come back unchanged.
fn header_block(md: &str) -> (Vec<(String, String)>, String) {
    let lines: Vec<&str> = md.lines().collect();
    let Some(h1) = lines.iter().position(|l| l.starts_with("# ")) else { return (vec![], md.to_string()) };
    let start = (h1 + 1..lines.len()).find(|&i| !lines[i].trim().is_empty()).unwrap_or(lines.len());
    let end = (start..lines.len()).find(|&i| !lines[i].trim_start().starts_with('>')).unwrap_or(lines.len());
    let rows: Option<Vec<(String, String)>> = lines[start..end]
        .iter()
        .map(|l| {
            let l = l.trim_start().trim_start_matches('>').trim();
            let (label, value) = l.strip_prefix("**")?.split_once(":**")?;
            Some((label.trim().to_string(), value.trim().to_string()))
        })
        .collect();
    match rows.filter(|r| !r.is_empty()) {
        Some(rows) => {
            let kept: Vec<&str> = lines[..start].iter().chain(&lines[end..]).copied().collect();
            (rows, kept.join("\n"))
        }
        None => (vec![], md.to_string()),
    }
}

/// The frontmatter's top-level keys, in order.
fn yaml_keys(front: &str) -> Vec<String> {
    front
        .lines()
        .filter(|l| !l.starts_with([' ', '\t']))
        .filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_string()))
        .filter(|k| !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-'))
        .collect()
}

/// Header-block labels (lower case) that say the same as a frontmatter key.
fn same_as(key: &str) -> &'static [&'static str] {
    match key {
        "minute_taker" | "minutes_taken_by" => &["minutes taken by", "minute taker"],
        "type" => &["meeting type", "type"],
        _ => &[],
    }
}

/// A frontmatter key as a label: `minute_taker` → "Minute taker".
fn label_of(key: &str) -> String {
    let words = key.replace(['_', '-'], " ");
    let mut chars = words.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// A top-level frontmatter value (`key: value`, or `key:` + `  - item` lines joined), unquoted.
fn yaml_value(front: &str, key: &str) -> Option<String> {
    let unquote = |v: &str| {
        let v = v.trim();
        let q = v.len() >= 2 && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')));
        if q { v[1..v.len() - 1].to_string() } else { v.to_string() }
    };
    let mut lines = front.lines().skip_while(|l| l.split_once(':').is_none_or(|(k, _)| k != key));
    let (_, v) = lines.next()?.split_once(':')?;
    if !v.trim().is_empty() {
        return Some(unquote(v));
    }
    let items: Vec<String> = lines.map_while(|l| l.trim_start().strip_prefix("- ").filter(|_| l.starts_with([' ', '\t']))).map(unquote).collect();
    Some(items.join(", "))
}

struct Body {
    html: String,
    mermaid: bool,
}

/// Markdown as safe HTML for the Echo window (Settings → About me): the minutes' own writer — raw
/// HTML shown as text, https links only, no images.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn markdown_html(md: &str) -> String {
    let load = |_: &str| None;
    let mut images = Images { load: &load, frames: &[], embedded: vec![], left: 0 };
    // A profile is written line by line ("Role: …"): each line stays one (a hard break).
    let lines: Vec<String> = md.lines().map(|l| if l.trim().is_empty() { String::new() } else { format!("{}  ", l.trim_end()) }).collect();
    body(&lines.join("\n"), &mut images).html
}

/// The notes' Markdown as HTML. Raw HTML in it is shown as text.
fn body(md: &str, images: &mut Images) -> Body {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let mut b = Body { html: String::new(), mermaid: false };
    let out = &mut b.html;
    let mut text = String::new(); // a run of text, written once complete (so `[mm:ss]` is whole)
    let mut aligns: Vec<Alignment> = vec![];
    let (mut cell, mut in_head) = (0usize, false);
    let mut links: Vec<bool> = vec![]; // per open link: did it become an <a>?
    let mut para: Option<usize> = None; // where the open <p> starts
    let mut image: Option<(String, String)> = None; // (destination, alt text so far)
    let mut code: Option<(bool, String)> = None; // (is mermaid, source so far)
    for ev in Parser::new_ext(md, opts) {
        if let Some((_, alt)) = image.as_mut() {
            match &ev {
                Event::Text(t) | Event::Code(t) => alt.push_str(t),
                Event::SoftBreak | Event::HardBreak => alt.push(' '),
                _ => {}
            }
            if !matches!(ev, Event::End(TagEnd::Image)) {
                continue;
            }
        }
        if let Some((_, src)) = code.as_mut() {
            if let Event::Text(t) = &ev {
                src.push_str(t);
            }
            if !matches!(ev, Event::End(TagEnd::CodeBlock)) {
                continue;
            }
        }
        match &ev {
            Event::Text(t) => {
                text.push_str(t);
                continue;
            }
            Event::SoftBreak => {
                text.push('\n');
                continue;
            }
            _ => {}
        }
        if !text.is_empty() {
            out.push_str(&times(&esc(&text)));
            text.clear();
        }
        match ev {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    para = Some(out.len());
                    out.push_str("<p>");
                }
                Tag::Heading { level, .. } => out.push_str(&format!("<{level}>")),
                Tag::BlockQuote(_) => out.push_str("<blockquote>"),
                Tag::CodeBlock(kind) => {
                    let lang = match kind {
                        CodeBlockKind::Fenced(l) => l.split_whitespace().next().unwrap_or("").to_ascii_lowercase(),
                        CodeBlockKind::Indented => String::new(),
                    };
                    code = Some((lang == "mermaid", String::new()));
                }
                Tag::List(Some(1)) => out.push_str("<ol>"),
                Tag::List(Some(n)) => out.push_str(&format!("<ol start=\"{n}\">")),
                Tag::List(None) => out.push_str("<ul>"),
                Tag::Item => out.push_str("<li>"),
                Tag::Table(a) => {
                    aligns = a;
                    out.push_str("<div class=\"tablewrap\"><table>");
                }
                Tag::TableHead => {
                    (in_head, cell) = (true, 0);
                    out.push_str("<thead><tr>");
                }
                Tag::TableRow => {
                    cell = 0;
                    out.push_str("<tr>");
                }
                Tag::TableCell => {
                    let align = match aligns.get(cell) {
                        Some(Alignment::Center) => " style=\"text-align:center\"",
                        Some(Alignment::Right) => " style=\"text-align:right\"",
                        _ => "",
                    };
                    out.push_str(&format!("<{}{align}>", if in_head { "th" } else { "td" }));
                }
                Tag::Emphasis => out.push_str("<em>"),
                Tag::Strong => out.push_str("<strong>"),
                Tag::Strikethrough => out.push_str("<del>"),
                Tag::Link { dest_url, .. } => {
                    let ok = dest_url.starts_with("https://");
                    if ok {
                        out.push_str(&format!("<a href=\"{}\" rel=\"noopener noreferrer\">", esc(&dest_url)));
                    }
                    links.push(ok);
                }
                Tag::Image { dest_url, .. } => image = Some((dest_url.to_string(), String::new())),
                _ => {} // footnotes, HTML blocks, metadata: their text still comes through as text
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => match para.take() {
                    // A paragraph that is only a screenshot: the figure alone (no <figure> in a <p>).
                    Some(at) if out[at + 3..].starts_with("<figure") && out.ends_with("</figure>") => out.replace_range(at..at + 3, ""),
                    _ => out.push_str("</p>"),
                },
                TagEnd::Heading(level) => out.push_str(&format!("</{level}>")),
                TagEnd::BlockQuote(_) => out.push_str("</blockquote>"),
                TagEnd::CodeBlock => {
                    let (diagram, src) = code.take().unwrap_or_default();
                    if diagram {
                        b.mermaid = true;
                        out.push_str(&format!("<figure class=\"diagram\"><pre class=\"mermaid\">{}</pre></figure>", esc(&diagram_source(&src))));
                    } else {
                        out.push_str(&format!("<pre><code>{}</code></pre>", esc(&src)));
                    }
                }
                TagEnd::List(true) => out.push_str("</ol>"),
                TagEnd::List(false) => out.push_str("</ul>"),
                TagEnd::Item => out.push_str("</li>"),
                TagEnd::Table => out.push_str("</tbody></table></div>"),
                TagEnd::TableHead => {
                    in_head = false;
                    out.push_str("</tr></thead><tbody>");
                }
                TagEnd::TableRow => out.push_str("</tr>"),
                TagEnd::TableCell => {
                    out.push_str(if in_head { "</th>" } else { "</td>" });
                    cell += 1;
                }
                TagEnd::Emphasis => out.push_str("</em>"),
                TagEnd::Strong => out.push_str("</strong>"),
                TagEnd::Strikethrough => out.push_str("</del>"),
                TagEnd::Link => out.push_str(if links.pop() == Some(true) { "</a>" } else { "" }),
                TagEnd::Image => {
                    let (dest, alt) = image.take().unwrap_or_default();
                    out.push_str(&figure(&dest, &alt, images));
                }
                _ => {}
            },
            Event::Code(t) => out.push_str(&format!("<code>{}</code>", esc(&t))),
            Event::Html(t) | Event::InlineHtml(t) => out.push_str(&esc(&t)), // shown, never markup
            Event::HardBreak => out.push_str("<br>"),
            Event::Rule => out.push_str("<hr>"),
            Event::TaskListMarker(done) => out.push_str(if done { "<input type=\"checkbox\" checked disabled> " } else { "<input type=\"checkbox\" disabled> " }),
            _ => {}
        }
    }
    if !text.is_empty() {
        out.push_str(&times(&esc(&text)));
    }
    b
}

/// `![what it shows](frames/<file>)`: the frame, embedded once, captioned with what it shows and
/// when Echo took it. Anything else (another path, a URL, a missing file, a frame already shown,
/// no room left) stays a text reference.
fn figure(dest: &str, alt: &str, images: &mut Images) -> String {
    if alt == ATTACHED {
        let name = dest.strip_prefix("frames/");
        if let Some(img) = name.and_then(|n| images.img(n, "Screenshot")) {
            return format!("<figure>{img}</figure>");
        }
    }
    let alt = alt.trim();
    let name = dest.strip_prefix("frames/");
    let mut caption = times(&esc(alt));
    if let Some(when) = name.and_then(|n| images.when(n)).filter(|_| !caption.contains("class=\"ts\"")) {
        caption.push_str(&format!("<span class=\"ts\">{when}</span>"));
    }
    match name.and_then(|n| images.img(n, alt)) {
        Some(img) if caption.is_empty() => format!("<figure>{img}</figure>"),
        Some(img) => format!("<figure>{img}<figcaption>{caption}</figcaption></figure>"),
        None => format!("<span class=\"figref\">[{}]</span>", if caption.is_empty() { "image".to_string() } else { caption }),
    }
}

/// The alt text `attach_screens` gives a screenshot it places: its line above says what and when, so
/// the figure gets no caption (an invisible separator: never typed in minutes).
const ATTACHED: &str = "\u{2063}";

/// The minutes' "On screen" section names what was shown by its time ("- [08:41] Slide 4/6 …");
/// the screenshot of that moment goes right under its line, so text and picture read together.
/// The nearest kept frame within 30 s, each once; a bullet that cites a frame itself is left alone.
fn attach_screens(md: &str, frames: &[Frame]) -> String {
    let heading = |l: &str| {
        let t = l.trim_start();
        let level = t.chars().take_while(|c| *c == '#').count();
        (level > 0 && t[level..].starts_with(' ')).then(|| (level, t[level..].trim().to_lowercase()))
    };
    let stamp = |l: &str| -> Option<f64> {
        let rest = l.trim_start().strip_prefix(['-', '*'])?.trim_start().strip_prefix('[')?;
        let (t, _) = rest.split_once(']')?;
        let parts: Vec<f64> = t.split(':').map(|p| p.parse::<f64>().ok()).collect::<Option<_>>()?;
        match parts.as_slice() {
            [m, s] => Some(m * 60.0 + s),
            [h, m, s] => Some(h * 3600.0 + m * 60.0 + s),
            _ => None,
        }
    };
    let lines: Vec<&str> = md.lines().collect();
    let mut used: Vec<&str> = vec![];
    let mut out: Vec<String> = vec![];
    let mut section: Option<usize> = None; // the On screen heading's level while inside it
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i];
        if let Some((level, text)) = heading(l) {
            section = if text.starts_with("on screen") { Some(level) } else if section.is_some_and(|s| level <= s) { None } else { section };
        }
        out.push(l.to_string());
        i += 1;
        let Some(at) = section.and(stamp(l)) else { continue };
        // The bullet's own continuation lines stay with it.
        let mut item = l.to_string();
        while i < lines.len() && lines[i].starts_with(char::is_whitespace) && !lines[i].trim().is_empty() {
            item.push_str(lines[i]);
            out.push(lines[i].to_string());
            i += 1;
        }
        if item.contains("](frames/") {
            continue;
        }
        let best = frames
            .iter()
            .filter(|f| !used.contains(&f.name.as_str()) && (f.t - at).abs() <= 30.0)
            .min_by(|a, b| (a.t - at).abs().total_cmp(&(b.t - at).abs()));
        if let Some(f) = best {
            used.push(&f.name);
            out.push(format!("  ![{ATTACHED}](frames/{})", f.name));
        }
    }
    let mut s = out.join("\n");
    if md.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// A diagram without its own configuration — `%%{init: …}%%` directives and a leading `---` YAML
/// block — so notes can't turn on what the page keeps off (HTML labels: forms, links).
fn diagram_source(src: &str) -> String {
    let mut lines = src.lines().peekable();
    let mut out = vec![];
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }
    if lines.peek().is_some_and(|l| l.trim() == "---") {
        lines.next();
        for l in lines.by_ref() {
            if l.trim() == "---" {
                break;
            }
        }
    }
    let mut directive = false;
    for l in lines {
        if directive || l.trim_start().starts_with("%%{") {
            directive = !l.contains("}%%");
            continue;
        }
        out.push(l);
    }
    out.join("\n")
}

/// `[mm:ss]` / `[h:mm:ss]` anchors in (already escaped) text as timestamps.
fn times(s: &str) -> String {
    let is_time = |t: &str| {
        let parts: Vec<&str> = t.split(':').collect();
        (2..=3).contains(&parts.len())
            && parts.iter().enumerate().all(|(k, p)| p.chars().all(|c| c.is_ascii_digit()) && (if k == 0 { (1..=2).contains(&p.len()) } else { p.len() == 2 }))
    };
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find(']').map(|j| &after[..j]).filter(|t| is_time(t)) {
            Some(t) => {
                out.push_str(&format!("<span class=\"ts\">{t}</span>"));
                rest = &after[t.len() + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Draws the page's ```mermaid blocks. `strict`, no HTML labels (and a diagram may not change that:
/// `secure`, plus `diagram_source`) — but a diagram's `click … href` still links, so PAGE_JS lets
/// links and windows go to https: only, and the CSP lets no form submit.
const MERMAID_INIT: &str = "mermaid.initialize({startOnLoad: true, securityLevel: 'strict', secure: ['secure', 'securityLevel', 'startOnLoad', 'maxTextSize', 'suppressErrorRendering', 'maxEdges', 'htmlLabels', 'flowchart'], theme: matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'default', htmlLabels: false, flowchart: {htmlLabels: false}})";

/// The page's own behaviour: links and windows only to https: (the notes' links already are; a
/// diagram's could be file: or an app's scheme) — clicks and window.open refused, and any other
/// destination removed from a link as it appears — and the screenshots' click-to-enlarge.
const PAGE_JS: &str = "(function () {
  var https = function (u) { return /^https:\\/\\//i.test(String(u || '')); };
  var open = window.open;
  window.open = function (u, t, f) { return https(u) ? open.call(window, u, t, f) : null; };
  var XLINK = 'http://www.w3.org/1999/xlink';
  var target = function (a) { return a.getAttribute('href') || a.getAttributeNS(XLINK, 'href'); };
  ['click', 'auxclick'].forEach(function (type) {
    document.addEventListener(type, function (e) {
      var a = e.target && e.target.closest && e.target.closest('a, area');
      if (a && !https(target(a))) e.preventDefault();
    }, true);
  });
  // And no other destination stays in the page (a context menu's Open link opens what's there):
  // a link a diagram draws loses it as soon as it appears.
  var scrub = function (a) {
    if (!https(target(a))) { a.removeAttribute('href'); a.removeAttributeNS(XLINK, 'href'); }
  };
  new MutationObserver(function (changes) {
    changes.forEach(function (c) {
      if (c.type === 'attributes') { if (c.target.closest) { var a = c.target.closest('a, area'); if (a) scrub(a); } return; }
      c.addedNodes.forEach(function (n) {
        if (n.nodeType !== 1) return;
        if (n.matches('a, area')) scrub(n);
        n.querySelectorAll('a, area').forEach(scrub);
      });
    });
  }).observe(document.documentElement, { childList: true, subtree: true, attributes: true, attributeFilter: ['href'] });
  var lb = document.getElementById('lightbox'), big = lb.querySelector('img');
  document.querySelectorAll('figure img').forEach(function (img) {
    img.addEventListener('click', function () { big.src = img.src; big.alt = img.alt; lb.style.display = 'flex'; });
  });
  lb.addEventListener('click', function () { lb.style.display = 'none'; big.removeAttribute('src'); });
  document.addEventListener('keydown', function (e) { if (e.key === 'Escape') lb.click(); });
})();";

/// Only in the window's copy (`render_for_window`), in a sandboxed frame: a click on a time posts
/// `{echo: 'at', t}` to the window (it opens the transcript there), a click on an https link
/// posts `{echo: 'open', url}` (the window opens it in the browser) instead of navigating the frame.
const EMBED_JS: &str = "(function () {
  document.documentElement.className += ' embedded';
  var post = function (m) { try { window.parent.postMessage(m, '*'); } catch (e) {} };
  var XLINK = 'http://www.w3.org/1999/xlink';
  document.addEventListener('click', function (e) {
    var el = e.target && e.target.closest ? e.target : null;
    if (!el) return;
    var ts = el.closest('.ts');
    if (ts) {
      var t = ts.textContent.trim().split(':').reduce(function (a, p) { return a * 60 + Number(p); }, 0);
      if (isFinite(t) && t >= 0) post({ echo: 'at', t: t });
      return;
    }
    var a = el.closest('a, area');
    if (!a) return;
    e.preventDefault();
    var u = a.getAttribute('href') || a.getAttributeNS(XLINK, 'href') || '';
    if (/^https:\\/\\//i.test(u)) post({ echo: 'open', url: u });
  }, true);
  // Words selected here: the window tells Echo's Claude terminals what \"this\" is.
  var selT = null;
  document.addEventListener('selectionchange', function () {
    clearTimeout(selT);
    selT = setTimeout(function () { post({ echo: 'sel', text: String(window.getSelection ? window.getSelection() : '').replace(/\\s+/g, ' ').trim().slice(0, 400) }); }, 300);
  });
  // Right-click a line: the window offers Add note, at the line's time, about the words selected
  // in it (else the line), under its section.
  var secs = function (ts) { var t = ts.textContent.trim().split(':').reduce(function (a, p) { return a * 60 + Number(p); }, 0); return isFinite(t) && t >= 0 ? t : null; };
  document.addEventListener('contextmenu', function (e) {
    var el = e.target && e.target.closest ? e.target : null;
    var block = el && el.closest('li, p, td, figure, blockquote, h1, h2, h3, h4');
    if (!block) return;
    e.preventDefault();
    var ts = block.querySelector('.ts');
    var sel = String(window.getSelection ? window.getSelection() : '').replace(/\\s+/g, ' ').trim();
    var top = block;
    while (top.parentElement && top.parentElement.tagName !== 'MAIN') top = top.parentElement;
    var h = top;
    while (h && !/^H[1-3]$/.test(h.tagName)) h = h.previousElementSibling;
    post({ echo: 'note', t: ts ? secs(ts) : null, quote: (sel || block.textContent.replace(/\\s+/g, ' ').trim()).slice(0, 300),
      section: h ? h.textContent.trim().slice(0, 80) : '', x: e.clientX, y: e.clientY });
  }, true);
  // The user's own notes (from the window, never in the page itself): each under the line with its
  // time — the same one, else the nearest within 3 min. Text only (textContent), and only from the
  // window this page is shown in.
  var LABEL = { note: 'Your note', question: 'Your question', decision: 'Your decision', todo: 'Your to-do', flag: 'You flagged' };
  var MARK = { note: '•', question: '?', decision: '!', todo: '☐', flag: '⚑' };
  var clock = function (t) { var m = Math.floor(t / 60), s = Math.floor(t % 60); var h = Math.floor(m / 60); m = m % 60;
    return (h ? h + ':' + (m < 10 ? '0' : '') + m : (m < 10 ? '0' : '') + m) + ':' + (s < 10 ? '0' : '') + s; };
  window.addEventListener('message', function (e) {
    var d = e.data;
    if (e.source !== window.parent || !d || d.echo !== 'notes' || !Array.isArray(d.notes)) return;
    var old = document.querySelectorAll('.mynote');
    for (var i = 0; i < old.length; i++) old[i].remove();
    var stamps = Array.prototype.slice.call(document.querySelectorAll('main .ts')).filter(function (ts) { return !ts.closest('.mynote, .meta, figcaption'); })
      .map(function (ts) { return { el: ts, t: secs(ts) }; }).filter(function (x) { return x.t != null; });
    d.notes.slice(0, 200).forEach(function (n) {
      if (typeof n.t !== 'number' || !isFinite(n.t)) return;
      var best = null;
      stamps.forEach(function (x) { var dt = Math.abs(x.t - n.t); if (dt <= 180 && (!best || dt < best.dt)) best = { el: x.el, dt: dt }; });
      if (!best) return;
      var block = best.el.closest('li, p, td, blockquote') || best.el.parentElement;
      var box = document.createElement('div');
      box.className = 'mynote k-' + (LABEL[n.kind] ? n.kind : 'note');
      var head = document.createElement('b');
      head.textContent = (MARK[n.kind] || '•') + ' ' + (LABEL[n.kind] || LABEL.note);
      var ts = document.createElement('span');
      ts.className = 'ts';
      ts.textContent = clock(n.t);
      box.appendChild(head); box.appendChild(document.createTextNode(' ')); box.appendChild(ts);
      box.appendChild(document.createTextNode(' ' + String(n.text || '').slice(0, 1000)));
      var sub = block.querySelector(':scope > ul, :scope > ol');
      if (sub) block.insertBefore(box, sub); else block.appendChild(box);
    });
  });
  post({ echo: 'ready' });
})();";

const CSS: &str = "
:root {
  --bg: #ffffff; --fg: #1d2733; --muted: #5b6875; --line: #dfe5ea;
  --soft: #f4f7f9; --accent: #0b5c7a; --accent-soft: #e6f1f5; --warn: #8a5a00; --warn-soft: #fff6e0;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #12171c; --fg: #e3e9ee; --muted: #9aa7b3; --line: #2c3640;
    --soft: #1a2128; --accent: #6cc0dd; --accent-soft: #16303b; --warn: #f0c060; --warn-soft: #2e2612;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg);
  font: 15px/1.6 -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif; }
main { max-width: 980px; margin: 0 auto; padding: 40px 20px 80px; }
h1 { font-size: 28px; line-height: 1.25; margin: 0 0 18px; }
h2 { font-size: 20px; margin: 44px 0 14px; padding-bottom: 6px; border-bottom: 2px solid var(--accent); }
h3 { font-size: 17px; margin: 32px 0 8px; color: var(--accent); }
h4, h5, h6 { font-size: 15px; margin: 22px 0 6px; }
a { color: var(--accent); }
hr { display: none; }
.meta { background: var(--soft); border-left: 4px solid var(--accent); padding: 12px 16px; border-radius: 4px; }
.meta div { margin: 2px 0; }
.meta b { display: inline-block; min-width: 140px; }
.note { background: var(--warn-soft); border-left: 4px solid var(--warn); padding: 10px 14px; border-radius: 4px; font-size: 14px; margin-top: 16px; }
table { width: 100%; border-collapse: collapse; margin: 10px 0 6px; font-size: 14px; }
th, td { border: 1px solid var(--line); padding: 7px 9px; text-align: left; vertical-align: top; }
th { background: var(--soft); font-weight: 600; }
.tablewrap { overflow-x: auto; }
.ts { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; margin-left: 4px; }
.embedded .ts { cursor: pointer; text-decoration: underline dotted; }
ul, ol { padding-left: 22px; }
li { margin: 3px 0; }
li:has(> input[type=checkbox]) { list-style: none; }
li > input[type=checkbox] { margin: 0 6px 0 -20px; }
blockquote { margin: 12px 0; padding: 4px 14px; border-left: 3px solid var(--line); color: var(--muted); }
code { font: 13px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: var(--soft); padding: 1px 4px; border-radius: 3px; }
pre { background: var(--soft); padding: 12px 14px; border-radius: 6px; overflow-x: auto; }
pre code { background: none; padding: 0; }
figure { margin: 18px 0 22px; border: 1px solid var(--line); border-radius: 6px; overflow: hidden; background: #fff; }
figure img { display: block; width: 100%; height: auto; cursor: zoom-in; }
figcaption { background: var(--soft); color: var(--fg); font-size: 13px; padding: 8px 12px; border-top: 1px solid var(--line); }
figure.diagram { background: var(--bg); padding: 14px; text-align: center; }
figure.diagram pre.mermaid { background: none; margin: 0; }
.figref { font-size: 12px; color: var(--muted); }
#lightbox { position: fixed; inset: 0; background: rgba(0,0,0,.88); display: none; align-items: center;
  justify-content: center; z-index: 10; cursor: zoom-out; padding: 16px; }
#lightbox img { max-width: 100%; max-height: 100%; }
@media print { figure { break-inside: avoid; } #lightbox { display: none !important; } main { padding: 0; } }
@media (max-width: 600px) { main { padding: 24px 16px 60px; } .meta b { min-width: 0; display: block; } }
";

/// Every minutes page — the file opened in a browser and the window's Minutes tab — in Echo's own
/// design: its colour tokens (`ui/app.css`, light and dark) and type (`.doc`), times as the window's
/// chips. Laid over `CSS`, which keeps the page's structure (tables, figures, print).
const ECHO_CSS: &str = "
:root {
  --bg: #ffffff; --page: #f6f6f8; --fg: #18181d; --muted: #55555f; --line: rgb(17 17 34 / 0.08); --soft: #f6f6f8;
  --accent: #5b34d6; --accent-soft: rgb(139 98 255 / 0.12); --warn: #b25b00; --warn-soft: rgb(178 91 0 / 0.08);
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1b1b21; --page: #141418; --fg: #ececf1; --muted: #a9a9b4; --line: rgb(255 255 255 / 0.07); --soft: #141418;
    --accent: #c4b0ff; --accent-soft: rgb(155 120 255 / 0.16); --warn: #f0a24a; --warn-soft: rgb(240 162 74 / 0.09);
  }
}
body { background: var(--page); font: 14px/1.65 -apple-system, BlinkMacSystemFont, 'Segoe UI Variable Text', 'Segoe UI', system-ui, sans-serif;
  -webkit-font-smoothing: antialiased; }
main { max-width: 820px; margin: 32px auto 64px; padding: 36px 44px 48px; background: var(--bg);
  border: 1px solid var(--line); border-radius: 10px; box-shadow: 0 1px 2px rgb(30 20 70 / 0.06), 0 6px 24px rgb(30 20 70 / 0.06); }
h1 { font-size: 22px; font-weight: 650; letter-spacing: -0.02em; line-height: 1.25; margin: 0 0 14px; }
h2 { font-size: 15px; font-weight: 650; margin: 28px 0 8px; padding: 0; border: 0; }
h3 { font-size: 14px; font-weight: 650; margin: 20px 0 6px; color: var(--fg); }
h4, h5, h6 { font-size: 14px; font-weight: 600; margin: 16px 0 4px; }
p { margin: 0 0 10px; }
ul, ol { padding-left: 20px; margin: 0 0 12px; }
li::marker { color: var(--muted); }
.meta { background: none; border: 0; padding: 0; border-radius: 0; color: var(--fg); }
.meta b { min-width: 120px; font-weight: 600; color: var(--muted); }
.note { border-left: 0; border-radius: 8px; font-size: 12.5px; padding: 8px 12px; color: var(--fg); }
blockquote { border-left: 2px solid var(--line); padding: 2px 14px; }
body .ts, .embedded .ts { font: 11px ui-monospace, 'SF Mono', 'Cascadia Mono', Menlo, Consolas, monospace;
  color: var(--accent); background: var(--accent-soft); padding: 0 5px; border-radius: 4px; margin-left: 2px; text-decoration: none; }
table { font-size: 13px; }
figure { border-radius: 10px; }
li > figure { margin: 8px 0 16px; max-width: 640px; }
code { font-size: 12px; }
@media (max-width: 860px) { main { margin: 0; border: 0; border-radius: 0; box-shadow: none; padding: 24px 18px 48px; } }
@media print { body { background: #fff; } main { margin: 0; border: 0; box-shadow: none; padding: 0; } }
";

/// In the window's Minutes tab the page is part of the window: no card around it, and no title of
/// its own (the call's header shows it).
const EMBED_CSS: &str = "
body { background: var(--bg); }
main { max-width: 1280px; margin: 0; padding: 22px 32px 48px; border: 0; border-radius: 0; box-shadow: none; }
main > h1:first-child { display: none; }
.mynote { margin: 6px 0 10px; padding: 6px 10px; border-radius: 8px; font-size: 13px; line-height: 1.5;
  background: var(--accent-soft); border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent); color: var(--fg); }
.mynote b { font-weight: 600; margin-right: 2px; }
.mynote.k-question b { color: #0b72a0; } .mynote.k-decision b { color: #0f8a5f; } .mynote.k-flag b { color: var(--warn); }
@media (prefers-color-scheme: dark) { .mynote.k-question b { color: #6cc4f0; } .mynote.k-decision b { color: #3ccf91; } }
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_renders_line_by_line_and_safely() {
        let html = markdown_html("# About me\n\nName: Ann\nRole: PM\n\n- a\n- b\n\n<script>x</script>\n\nSee [l](http://x) and [ok](https://example.com)");
        assert!(html.contains("Name: Ann<br>"), "{html}");
        assert!(html.contains("<li>a</li>") || html.contains("<li>a"), "{html}");
        assert!(!html.contains("<script>") && html.contains("&lt;script&gt;"));
        assert!(html.contains("href=\"https://example.com\"") && !html.contains("href=\"http://x\""));
    }

    const NOTES: &str = "---\ndate: 2026-10-22\ntype: \"architecture-review\"\nfacilitator: \"Lena Varga\"\nattendees:\n  - \"Lena Varga\"\n---\n\n\
        # Meeting Minutes: Atlas\n\n> **Project:** Atlas\n> **Date:** 2026-10-22\n\n---\n\n## Decisions\n\n\
        | # | Decision |\n|---|---|\n| 1 | Phased migration [04:12] |\n\n\
        ## Discussion Summary\n\n- Said <script>alert(1)</script> and [a link](http://evil.example) and [docs](https://example.com/x)\n\
        - Range [02:27–02:35] stays text\n\n![Migration options [01:08]](frames/0001.jpg)\n\n![x](../secret.png)\n\n- [ ] `risk log` — add the freeze\n";

    fn load(name: &str) -> Option<(&'static str, Vec<u8>)> {
        ["0001.jpg", "0002.jpg"].contains(&name).then(|| ("image/jpeg", vec![1, 2, 3]))
    }

    #[test]
    fn the_page_is_built_from_the_minutes() {
        let frames = [
            Frame { name: "0001.jpg".into(), t: 68.0, shared_by: None },
            Frame { name: "0002.jpg".into(), t: 3725.0, shared_by: Some("Tom <b>".into()) },
        ];
        let html = render(NOTES, "Atlas", Some("qwen3.6-echo:35b-a3b"), &load, &frames);
        // Header block (with the frontmatter's type and facilitator) right under the title.
        assert!(html.contains("<h1>Meeting Minutes: Atlas</h1><div class=\"meta\"><div><b>Project</b>Atlas</div>"));
        assert!(html.contains("<div><b>Meeting type</b>architecture-review</div><div><b>Facilitator</b>Lena Varga</div>"));
        assert!(!html.contains("attendees:") && !html.contains("&gt; **Project"));
        // The frontmatter's other fields too, unless the header block already has them (date).
        assert!(html.contains("<div><b>Attendees</b>Lena Varga</div>") && html.matches("<b>Date</b>").count() == 1);
        assert!(html.contains("Written by AI (Echo, qwen3.6-echo:35b-a3b)"));
        // Tables, timestamps, task lists.
        assert!(html.contains("<div class=\"tablewrap\"><table><thead><tr><th>#</th><th>Decision</th></tr></thead><tbody><tr><td>1</td><td>Phased migration <span class=\"ts\">04:12</span></td></tr></tbody></table></div>"));
        assert!(html.contains("[02:27–02:35]"));
        assert!(html.contains("<input type=\"checkbox\" disabled> <code>risk log</code>"));
        // Other people's words never become markup; only https links do.
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;") && !html.contains("<script>alert"));
        assert!(!html.contains("evil.example\"") && html.contains("<a href=\"https://example.com/x\" rel=\"noopener noreferrer\">docs</a>"));
        // Only this call's frames are embedded, and only where the notes cite them.
        assert!(html.contains("<figure><img src=\"data:image/jpeg;base64,AQID\" alt=\"Migration options [01:08]\"><figcaption>Migration options <span class=\"ts\">01:08</span></figcaption></figure>"));
        assert!(!html.contains("<p><figure"));
        assert!(html.contains("<span class=\"figref\">[x]</span>"));
        assert!(!html.contains("<h2>Screens</h2>") && !html.contains("Screen at 1:02:05"), "an uncited frame isn't added");
        assert_eq!(html.matches("<figure>").count(), 1);
        assert!(!html.contains("mermaid.initialize"));
    }

    #[test]
    fn diagrams_bring_mermaid_and_old_notes_still_render() {
        let html = render("# T\n\nA line.\n\n```mermaid\nflowchart TD\n  A[a<br>b] --> B\n```\n", "T", None, &load, &[]);
        assert!(html.contains("<pre class=\"mermaid\">flowchart TD\n  A[a&lt;br&gt;b] --&gt; B</pre>"));
        assert!(html.contains("mermaid.initialize"));
        assert!(!html.contains("<div class=\"meta\">") && html.contains("<h1>T</h1><p class=\"note\">Written by AI (Echo)"));
        // No title in the notes: the call's, escaped.
        assert!(render("Just text", "A <b> call", None, &load, &[]).contains("<h1>A &lt;b&gt; call</h1>"));
    }

    #[test]
    fn frontmatter_fields_beside_the_header_block() {
        // The same field once ("Time" with its timezone wins; "Minutes taken by" = `minute_taker`),
        // a different field kept even when its value appears elsewhere (Ann facilitated and attended).
        let md = "---\ntime: \"10:00 - 10:04\"\nminute_taker: \"Echo\"\nfacilitator: \"Ann\"\n---\n\n# Sync\n\n> **Time:** 10:00 - 10:04 (UTC+02:00)\n> **Minutes taken by:** Echo\n> **Attendees:** Ann, Bo\n\nText\n";
        let html = render(md, "Sync", None, &load, &[]);
        assert!(html.contains("<div><b>Attendees</b>Ann, Bo</div><div><b>Facilitator</b>Ann</div></div>"));
        assert_eq!(html.matches("<b>Time</b>").count() + html.matches("<b>Minute taker</b>").count(), 1);
    }

    #[test]
    fn notes_with_only_frontmatter_keep_it() {
        let html = render("---\ndate: 2026-10-22\nminute_taker: \"Ann\"\nattendees:\n  - \"Ann\"\n  - \"Bo\"\n---\n\n# Sync\n\nText\n", "Sync", None, &load, &[]);
        assert!(html.contains("<div class=\"meta\"><div><b>Date</b>2026-10-22</div><div><b>Minute taker</b>Ann</div><div><b>Attendees</b>Ann, Bo</div></div>"));
    }

    #[test]
    fn a_diagram_cant_change_the_pages_settings() {
        let src = "---\nconfig:\n  flowchart:\n    htmlLabels: true\n---\n%%{init: {\"flowchart\": {\"htmlLabels\": true}}}%%\nflowchart TD\n  %%{init:\n    {\"x\": 1}}%%\n  %% a comment stays\n  A[\"<form action='http://x.example/'></form>\"]\n";
        assert_eq!(diagram_source(src), "flowchart TD\n  %% a comment stays\n  A[\"<form action='http://x.example/'></form>\"]");
        let html = render(&format!("# T\n\n```mermaid\n{src}```\n"), "T", None, &load, &[]);
        assert!(!html.contains("htmlLabels: true") && !html.contains("&quot;htmlLabels&quot;"));
        assert!(html.contains("A[&quot;&lt;form action=&#39;http://x.example/&#39;&gt;&lt;/form&gt;&quot;]"));
    }

    #[test]
    fn mermaid_can_sit_inside_a_script_tag() {
        let lower = MERMAID_JS.to_ascii_lowercase();
        assert!(!lower.contains("<script") && !lower.contains("</script"), "embed mermaid differently (e.g. as a data: URL)");
    }

    #[test]
    fn names_values_and_header_blocks() {
        assert_eq!(file_stem("Atlas: review / Q4?"), "Atlas- review - Q4");
        assert_eq!(file_stem("  ..  "), "call");
        assert_eq!(file_stem("CON.review"), "_CON.review");
        assert_eq!(file_stem("com1"), "_com1");
        assert_eq!(file_stem("COM¹.review"), "_COM¹.review");
        assert_eq!(file_stem("Lpt³"), "_Lpt³");
        assert_eq!(file_stem("Conference"), "Conference");
        assert_eq!(file_stem("nul .x"), "_nul .x");
        assert_eq!(file_stem("Console review"), "Console review");
        assert_eq!(yaml_value("a: \"x: y\"\nlist:\n  - 'p'\n  - q\nb: 1\n", "a").as_deref(), Some("x: y"));
        assert_eq!(yaml_value("list:\n  - 'p'\n  - q\nb: 1\n", "list").as_deref(), Some("p, q"));
        assert_eq!(yaml_value("a: 1\n", "missing"), None);
        let (rows, rest) = header_block("# T\n\n> just a quote\n\nBody");
        assert!(rows.is_empty() && rest == "# T\n\n> just a quote\n\nBody");
        assert_eq!(times("at [5:07] and [1:02:03], not [12] or [1:2]"), "at <span class=\"ts\">5:07</span> and <span class=\"ts\">1:02:03</span>, not [12] or [1:2]");
    }

    #[test]
    fn on_screen_lines_get_the_screenshot_of_their_moment() {
        let frames = [
            Frame { name: "01m02s.jpg".into(), t: 62.0, shared_by: None },
            Frame { name: "08m41s.jpg".into(), t: 521.0, shared_by: None },
            Frame { name: "30m00s.jpg".into(), t: 1800.0, shared_by: None },
        ];
        let md = "# T\n\n- [01:02] Not on screen: no picture here\n\n## On screen\n- [01:02] Slide 2/6, the plan\n- [08:45] Slide 4/6\n  that goes on\n- [20:00] Nothing kept near this\n- [08:41] Cites its own ![x](frames/08m41s.jpg)\n\n## Decisions\n- [30:00] Not on screen\n";
        let got = attach_screens(md, &frames);
        assert_eq!(got.matches("](frames/01m02s.jpg)").count(), 1, "{got}");
        assert!(got.contains(&format!("- [08:45] Slide 4/6\n  that goes on\n  ![{ATTACHED}](frames/08m41s.jpg)\n")), "{got}");
        assert!(got.contains("- [20:00] Nothing kept near this\n- [08:41]"), "{got}");
        assert!(!got.contains("30m00s"), "only the On screen section: {got}");
        assert!(got.starts_with("# T\n\n- [01:02] Not on screen: no picture here\n\n## On screen"));
        let html = render(md, "T", None, &load_frames, &frames);
        assert_eq!(html.matches("<figure>").count(), 2, "{html}");
        assert!(!html.contains("<figcaption>"), "the line above is the caption: {html}");
    }

    fn load_frames(name: &str) -> Option<(&'static str, Vec<u8>)> {
        ["01m02s.jpg", "08m41s.jpg", "30m00s.jpg"].contains(&name).then(|| ("image/jpeg", vec![1, 2, 3]))
    }

    #[test]
    fn each_frame_once_with_its_time() {
        let frames = [Frame { name: "0001.jpg".into(), t: 68.0, shared_by: None }];
        let html = render("# T\n\n![Options](frames/0001.jpg)\n\nAgain: ![Options](frames/0001.jpg)\n", "T", None, &load, &frames);
        assert_eq!(html.matches("<img src=").count(), 1);
        assert!(html.contains("<figcaption>Options<span class=\"ts\">01:08</span></figcaption>"));
        assert!(html.contains("Again: <span class=\"figref\">[Options<span class=\"ts\">01:08</span>]</span>"));
        assert!(!html.contains("<h2>Screens</h2>"));
    }

    #[test]
    fn only_the_pages_own_scripts_and_images() {
        // mermaid's `img:` shapes would fetch a URL: the policy allows data: images only.
        let html = render("# T\n\n```mermaid\nflowchart TD\n  A@{ img: \"https://x.example/p\" }\n```\n", "T", None, &load, &[]);
        let csp = html.split("Content-Security-Policy\" content=\"").nth(1).unwrap().split('"').next().unwrap();
        assert!(csp.starts_with("default-src 'none'; img-src data:; style-src 'unsafe-inline'; form-action 'none'; base-uri 'none'; script-src 'sha256-"));
        assert!(!csp.split("script-src").nth(1).unwrap().contains("unsafe"));
        let scripts: Vec<&str> = html.split("<script>").skip(1).map(|s| s.split("</script>").next().unwrap()).collect();
        assert_eq!(scripts.len(), 3);
        assert!(csp.contains("form-action 'none'; base-uri 'none';"));
        for body in scripts {
            assert!(csp.contains(&format!("'sha256-{}'", B64.encode(Sha256::digest(body.as_bytes())))));
        }
        assert_eq!(render("# T\n", "T", None, &load, &[]).matches("'sha256-").count(), 1);
        // A CRLF checkout's script is carried and hashed as the browser will see it.
        assert_eq!(hashed(&["a\r\nb\rc"]), hashed(&["a\nb\nc"]));
    }

    #[test]
    fn frames_only_from_the_calls_own_folder() {
        let dir = std::env::temp_dir().join(format!("echo-minutes-frames-{}", std::process::id()));
        let frames = dir.join("frames");
        fs::create_dir_all(&frames).unwrap();
        fs::write(frames.join("ok.jpg"), [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        fs::write(frames.join("fake.jpg"), b"not an image").unwrap();
        fs::write(dir.join("secret.jpg"), [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
        assert!(frame_bytes(&dir, "ok.jpg").is_some());
        for bad in ["fake.jpg", "../secret.jpg", "C:secret.jpg", ".hidden.jpg", "sub/ok.jpg", "ok.gif", ""] {
            assert!(frame_bytes(&dir, bad).is_none(), "{bad}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("secret.jpg"), frames.join("link.jpg")).unwrap();
            assert!(frame_bytes(&dir, "link.jpg").is_none());
            // A call whose `frames` folder is itself a link to another folder.
            let other = dir.join("other-call");
            fs::create_dir_all(other.join("elsewhere")).unwrap();
            fs::write(other.join("elsewhere").join("ok.jpg"), [0xFF, 0xD8, 0xFF, 0xE0]).unwrap();
            std::os::unix::fs::symlink(other.join("elsewhere"), other.join("frames")).unwrap();
            assert!(frame_bytes(&other, "ok.jpg").is_none());
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_page_never_replaces_someone_elses_file() {
        let dir = std::env::temp_dir().join(format!("echo-minutes-write-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.md"), "# Atlas\n\nText\n").unwrap();
        fs::write(dir.join("meta.json"), r#"{"title":"Atlas"}"#).unwrap();
        fs::write(dir.join("Atlas-minutes.html"), "mine").unwrap();
        fs::write(dir.join("Old title-minutes.html"), format!("{GENERATOR} an earlier page")).unwrap();
        let page = write(&dir).unwrap();
        assert_eq!(page.file_name().unwrap(), "Atlas (2)-minutes.html");
        assert_eq!(fs::read_to_string(dir.join("Atlas-minutes.html")).unwrap(), "mine");
        assert!(!dir.join("Old title-minutes.html").exists());
        assert_eq!(write(&dir).unwrap(), page); // its own page again, not a third
        // Retitled (`echoctl title`) while the invite keeps its subject: the call's title wins, and
        // the page under the old name goes.
        fs::write(dir.join("meeting.json"), r#"{"subject":"Weekly sync"}"#).unwrap();
        fs::write(dir.join("meta.json"), r#"{"title":"Incident review"}"#).unwrap();
        let renamed = write(&dir).unwrap();
        assert_eq!(renamed.file_name().unwrap(), "Incident review-minutes.html");
        assert!(!page.exists() && fs::read_to_string(&renamed).unwrap().contains("<title>Incident review – minutes</title>"));
        // A blank title falls back to the invite's subject, not the folder name.
        for blank in ["", "  "] {
            fs::write(dir.join("meta.json"), format!(r#"{{"title":"{blank}"}}"#)).unwrap();
            assert_eq!(write(&dir).unwrap().file_name().unwrap(), "Weekly sync-minutes.html");
        }
        assert!(fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_windows_copy_talks_to_the_window_under_its_own_policy() {
        let dir = std::env::temp_dir().join(format!("echo-minutes-embed-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.md"), "# T\n\n- Agreed [12:30]\n\n```mermaid\nflowchart TD\n  A --> B\n```\n").unwrap();
        fs::write(dir.join("meta.json"), r#"{"title":"T"}"#).unwrap();
        let (html, csp) = render_for_window(&dir).unwrap();
        assert!(html.contains("<span class=\"ts\">12:30</span>") && html.contains("window.parent.postMessage"));
        // The header the window sends is the page's own policy, and covers every script it carries.
        assert_eq!(html.split("Content-Security-Policy\" content=\"").nth(1).unwrap().split('"').next().unwrap(), csp);
        let scripts: Vec<&str> = html.split("<script>").skip(1).map(|s| s.split("</script>").next().unwrap()).collect();
        assert_eq!(scripts.len(), 4, "the page's, the window's, mermaid and its settings");
        for body in scripts {
            assert!(csp.contains(&format!("'sha256-{}'", B64.encode(Sha256::digest(body.as_bytes())))));
        }
        // The file never carries it.
        let page = write(&dir).unwrap();
        assert!(!fs::read_to_string(page).unwrap().contains("window.parent.postMessage"));
        // The frame reloads for the minutes, never for the user's notes, a tick or a run's state.
        let rev = revision(&dir);
        fs::write(dir.join(crate::calls::USER_NOTES), "mine").unwrap();
        fs::write(dir.join(crate::calls::ACTIONS), "[]").unwrap();
        fs::write(dir.join(crate::notes::RUN), r#"{"state":"running"}"#).unwrap();
        assert_eq!(revision(&dir), rev);
        fs::write(dir.join("notes.md"), "# T\n\nChanged\n").unwrap();
        assert_ne!(revision(&dir), rev);
        let _ = fs::remove_dir_all(&dir);
    }
}

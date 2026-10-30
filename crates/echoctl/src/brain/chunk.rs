//! Brain's chunker (`knowledge-index/chunking.py`, CHUNKER_VERSION 2), ported as it is: heading-aware
//! sections with their breadcrumb, a long section split at paragraphs into "(part n)", no overlap. A
//! chunk's id is Brain's: the first 8 bytes of sha256(source \0 ord), shifted to a positive int64, so
//! a re-sync keeps ids (and the tags and edges on them).

use sha2::{Digest, Sha256};

pub const CHUNKER_VERSION: &str = "2";
/// The longest section before it is split (the indexer's default).
pub const MAX_CHARS: usize = 1200;

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub ord: usize,
    pub title: String,
    pub body: String,
    pub parent_heading: String,
    pub breadcrumb: String,
}

/// Brain's chunk id: a positive int64 from sha256(source \0 ord).
pub fn chunk_id(source: &str, ord: usize) -> i64 {
    let d = Sha256::digest(format!("{source}\0{ord}").as_bytes());
    (u64::from_be_bytes(d[..8].try_into().unwrap()) >> 1) as i64
}

fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(' ') && !rest.is_empty() {
        return None;
    }
    let t = rest.trim();
    Some((level, t.to_string()))
}

/// The first ten words of the first substantive line, at most 70 characters.
fn derive_title(body: &str) -> String {
    let line = body.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("<!--")).unwrap_or("");
    let words: Vec<&str> = line.split_whitespace().take(10).collect();
    let t = words.join(" ");
    if t.chars().count() > 70 {
        t.chars().take(70).collect::<String>().trim_end().to_string()
    } else {
        t
    }
}

/// A body is empty when it holds nothing but blank lines and markers (`<!-- … -->`).
fn substantive(body: &str) -> bool {
    body.lines().map(str::trim).any(|l| !(l.is_empty() || l.starts_with("<!--") && l.ends_with("-->")))
}

/// Split a long body at blank lines into parts no longer than `max` (a paragraph longer than that
/// stays whole).
fn split_long(body: &str, max: usize) -> Vec<String> {
    if body.chars().count() <= max {
        return vec![body.to_string()];
    }
    let mut parts: Vec<String> = vec![];
    let mut cur = String::new();
    for para in body.split("\n\n") {
        let p = para.trim_matches('\n');
        if p.trim().is_empty() {
            continue;
        }
        if !cur.is_empty() && cur.chars().count() + 2 + p.chars().count() > max {
            parts.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push_str("\n\n");
        }
        cur.push_str(p);
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

/// Sections of a Markdown document, in order.
pub fn chunk(md: &str, max: usize) -> Vec<Chunk> {
    // (level, heading, body) blocks; text before the first heading is a block of its own.
    let mut blocks: Vec<(usize, String, String)> = vec![];
    let mut cur: Option<(usize, String, String)> = None;
    let mut pre = String::new();
    for line in md.lines() {
        if let Some((level, title)) = heading(line) {
            if let Some(b) = cur.take() {
                blocks.push(b);
            }
            cur = Some((level, title, String::new()));
        } else if let Some(b) = cur.as_mut() {
            b.2.push_str(line);
            b.2.push('\n');
        } else {
            pre.push_str(line);
            pre.push('\n');
        }
    }
    if let Some(b) = cur.take() {
        blocks.push(b);
    }
    let mut out = vec![];
    let mut ord = 0;
    let push = |title: &str, body: &str, parent: &str, crumb: &str, out: &mut Vec<Chunk>, ord: &mut usize| {
        let parts = split_long(body.trim(), max);
        let n = parts.len();
        for (i, p) in parts.into_iter().enumerate() {
            let t = if title.is_empty() { derive_title(&p) } else { title.to_string() };
            let t = if n > 1 { format!("{t} (part {})", i + 1) } else { t };
            out.push(Chunk { ord: *ord, title: t, body: p, parent_heading: parent.to_string(), breadcrumb: crumb.to_string() });
            *ord += 1;
        }
    };
    if blocks.is_empty() {
        if substantive(&pre) {
            push("", &pre, "", "", &mut out, &mut ord);
        }
        return out;
    }
    if substantive(&pre) {
        push("", &pre, "", "", &mut out, &mut ord);
    }
    let mut stack: Vec<(usize, String)> = vec![];
    for (level, title, body) in blocks {
        while stack.last().is_some_and(|(l, _)| *l >= level) {
            stack.pop();
        }
        let parent = stack.last().map(|(_, t)| t.clone()).unwrap_or_default();
        stack.push((level, title.clone()));
        let crumb = stack.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join(" > ");
        if substantive(&body) {
            push(&title, &body, &parent, &crumb, &mut out, &mut ord);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_with_breadcrumbs_and_parts() {
        let md = "# Aurora weekly\n\n## Minutes\n\n### Summary\nSSO lands first.\n\n### Decisions\n- Pilot moves to 24 Nov.\n\n## Empty\n\n## Transcript\n";
        let c = chunk(md, 1200);
        assert_eq!(c.len(), 2, "headings without a body are dropped: {c:?}");
        assert_eq!(c[0].title, "Summary");
        assert_eq!(c[0].breadcrumb, "Aurora weekly > Minutes > Summary");
        assert_eq!(c[0].parent_heading, "Minutes");
        assert_eq!((c[1].ord, c[1].title.as_str()), (1, "Decisions"));

        let long = format!("# T\n\n{}\n\n{}\n", "a ".repeat(400), "b ".repeat(400));
        let c = chunk(&long, 1200);
        assert_eq!(c.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(), ["T (part 1)", "T (part 2)"]);
        assert!(chunk("no headings at all, just a line about Москва", 1200)[0].title.contains("Москва"));
        assert!(chunk("", 1200).is_empty());
    }

    #[test]
    fn ids_are_brains() {
        let a = chunk_id("call/2026-10-28_110000", 0);
        assert!(a > 0);
        assert_eq!(a, chunk_id("call/2026-10-28_110000", 0));
        assert_ne!(a, chunk_id("call/2026-10-28_110000", 1));
    }
}

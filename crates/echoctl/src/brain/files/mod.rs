//! Thread folders in the Brain (phase 5): Brain's document lane (`parse_corpus`, `visual-parse`,
//! `source_registry`, `scribe_marker`) ported. Each file is read deterministically; each page or slide
//! is routed — indexed from its text, or (when its text misses the meaning: a diagram, a timeline, a
//! title-only slide) read from its image by the page reader. Only the index is kept; every section
//! links back to its file and page.

pub mod folder;
pub mod office;
pub mod pdf;
pub mod render;
pub mod sheet;
pub mod text;
pub mod vision;
pub mod xml;

use anyhow::Result;
use std::path::Path;

/// A page (PDF) or a visible slide (PowerPoint), as read from the file.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Page {
    /// 1-based, in order (slides: visible ones only — the same numbers as LibreOffice's PDF).
    pub n: usize,
    pub title: Option<String>,
    /// The text layer.
    pub text: String,
    /// Real table cells as Markdown grids (the factual layer: numbers are cited from these).
    pub tables: Vec<String>,
    /// Drawn objects: PDF vector paths; a slide's non-placeholder shapes.
    pub drawings: usize,
    pub pictures: usize,
    /// A slide's free text boxes and their characters (many short ones: a diagram's labels).
    pub labels: usize,
    pub label_chars: usize,
    pub connectors: usize,
    pub charts: usize,
    pub smartart: usize,
}

/// What a reader made of a file.
#[derive(Debug, Default)]
pub struct Parsed {
    /// `pptx-xml`, `docx-xml`, `pdfium`, `spreadsheet`, `transcript`, `text`, `html`.
    pub method: &'static str,
    /// `full`, `partial` (rows or pages left out), `degraded` (text without layout or images).
    pub fidelity: &'static str,
    /// Paged formats (PDF, PowerPoint).
    pub pages: Vec<Page>,
    /// Flowing formats (Word, spreadsheets, text): the Markdown.
    pub md: Option<String>,
    /// Pictures and charts in a Word file (their meaning isn't in its text).
    pub figures: usize,
    /// Read, but nothing worth indexing (and why).
    pub skip: Option<String>,
}

/// The formats read, by extension; anything else in a folder is listed as unsupported.
pub const FORMATS: &[&str] = &["pptx", "docx", "pdf", "xlsx", "xlsm", "xls", "md", "markdown", "txt", "vtt", "srt", "html", "htm"];

/// Which of a set of twins (one stem, several formats) is read: the editable one first.
pub const TWIN_ORDER: &[&str] = &["pptx", "docx", "xlsx", "xlsm", "xls", "md", "markdown", "txt", "pdf", "html", "htm", "vtt", "srt"];

pub fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase()
}

/// Read a file with the reader for its format.
pub fn read(path: &Path) -> Result<Parsed> {
    let mut p = match ext(path).as_str() {
        "pptx" => office::pptx(path)?,
        "docx" => office::docx(path)?,
        "pdf" => pdf::read(path)?,
        "xlsx" | "xlsm" | "xls" => sheet::read(path)?,
        "vtt" | "srt" => text::transcript(path)?,
        "md" | "markdown" | "txt" => text::plain(path)?,
        "html" | "htm" => text::html(path)?,
        other => anyhow::bail!("not a format the Brain reads: .{other}"),
    };
    if p.fidelity.is_empty() {
        p.fidelity = "full";
    }
    Ok(p)
}

/// Brain's `render_pages` rule: should this page go to the page reader, and why — thin text with no
/// table (`thin-text`: a title, a picture, a pure diagram), ≥ 60 vector paths (`dense-draw`: a timeline
/// or diagram, whatever its labels), or ≥ 28 with modest text (< 1000: `diagram`). Measured on PDF
/// pages: slides and Word pages with figures are measured on LibreOffice's PDF of them (on a sample of
/// real-world decks the slides' own XML misjudged half of them: free text boxes and divider lines
/// everywhere). A page that is a table we have the cells of is never sent for being thin.
pub fn route(p: &Page) -> Option<&'static str> {
    let tl = p.text.trim().chars().count();
    if tl < 220 && p.tables.is_empty() {
        return Some("thin-text");
    }
    if p.drawings >= 60 {
        return Some("dense-draw");
    }
    if p.drawings >= 28 && tl < 1000 {
        return Some("diagram");
    }
    None
}

/// Why a file in a folder isn't read, before reading it (Brain's `scribe_marker.skip_reason`):
/// hidden (any dot component, AppleDouble, an Office lock file), excluded by the folder's globs.
pub fn skip_reason(rel: &str, excludes: &[String]) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.iter().any(|p| p.starts_with('.')) || parts.last().is_some_and(|f| f.starts_with("~$")) {
        return Some("hidden".into());
    }
    excludes.iter().find(|g| glob_match(rel, g)).map(|g| format!("excluded by {g}"))
}

/// A folder-relative glob: `*` within a name, `**` across folders, `?` one character; a glob with no
/// `/` matches a name anywhere.
pub fn glob_match(rel: &str, glob: &str) -> bool {
    fn m(p: &[char], s: &[char]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some('*'), _) if p.get(1) == Some(&'*') => {
                let rest = if p.get(2) == Some(&'/') { &p[3..] } else { &p[2..] };
                (0..=s.len()).any(|i| m(rest, &s[i..]))
            }
            (Some('*'), _) => (0..=s.len()).take_while(|&i| i == 0 || s[i - 1] != '/').any(|i| m(&p[1..], &s[i..])),
            (Some('?'), Some(c)) if *c != '/' => m(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => m(&p[1..], &s[1..]),
            _ => false,
        }
    }
    let g = glob.trim().trim_start_matches("./");
    if g.is_empty() {
        return false;
    }
    let g = if g.contains('/') { g.to_string() } else { format!("**/{g}") };
    let g = if g.ends_with('/') { format!("{g}**") } else { g };
    m(&g.chars().collect::<Vec<_>>(), &rel.chars().collect::<Vec<_>>())
}

// ── dates (Brain's `exact_date`) ─────────────────────────────────────────────────────────────────

fn valid(y: i32, m: u32, d: u32) -> Option<String> {
    ((2000..=2035).contains(&y)).then_some(())?;
    chrono::NaiveDate::from_ymd_opt(y, m, d).map(|x| x.format("%Y-%m-%d").to_string())
}

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

/// The first full date in a document's first 2000 characters (ISO, US 10/24/2023, European
/// 28.05.2024 — not a version like v2.1.22 —, or "October 24, 2023"); one on or after today is an
/// auto-updated field, so the text counts as undated.
fn content_date(text: &str, today: &str) -> Option<String> {
    let sample: String = text.chars().take(2000).collect();
    let b = sample.as_bytes();
    let mut hits: Vec<(usize, String)> = vec![];
    let num = |i: usize, n: usize| -> Option<u32> { (i + n <= b.len() && b[i..i + n].iter().all(u8::is_ascii_digit)).then(|| sample[i..i + n].parse().ok()).flatten() };
    let boundary = |i: usize| i == 0 || !b[i - 1].is_ascii_alphanumeric();
    for i in 0..b.len() {
        if !sample.is_char_boundary(i) || !boundary(i) {
            continue;
        }
        // 2024-05-28
        if let (Some(y), Some(m), Some(d)) = (num(i, 4), num(i + 5, 2), num(i + 8, 2)) {
            if b.get(i + 4) == Some(&b'-') && b.get(i + 7) == Some(&b'-') && b.get(i + 10).is_none_or(|c| !c.is_ascii_digit()) {
                if let Some(x) = valid(y as i32, m, d) {
                    hits.push((i, x));
                    continue;
                }
            }
        }
        // 10/24/2023 and 28.05.2024
        let mut j = i;
        let read = |j: &mut usize| -> Option<(u32, usize)> {
            let st = *j;
            while *j < b.len() && b[*j].is_ascii_digit() && *j - st < 4 {
                *j += 1;
            }
            (*j > st).then(|| (sample[st..*j].parse().ok().unwrap_or(0), *j - st))
        };
        if let Some((a, la)) = read(&mut j) {
            if la <= 2 && j < b.len() && (b[j] == b'/' || b[j] == b'.') {
                let sep = b[j];
                j += 1;
                if let Some((c, lc)) = read(&mut j) {
                    if lc <= 2 && j < b.len() && b[j] == sep {
                        j += 1;
                        if let Some((y, ly)) = read(&mut j) {
                            let chained = j < b.len() && (b[j].is_ascii_digit() || (b[j] == b'.' && b.get(j + 1).is_some_and(u8::is_ascii_digit)));
                            let version = i > 0 && b[i - 1].is_ascii_alphabetic();
                            // A dotted 1.2.2024 is ambiguous with a version: left undated.
                            let ambiguous = sep == b'.' && la == 1 && lc == 1;
                            if ly == 4 && !chained && !version && !ambiguous {
                                let x = if sep == b'/' { valid(y as i32, a, c) } else { valid(y as i32, c, a) };
                                if let Some(x) = x {
                                    hits.push((i, x));
                                }
                            }
                        }
                    }
                }
            }
        }
        // October 24, 2023
        let lower = sample[i..].chars().take(12).collect::<String>().to_lowercase();
        if let Some(mi) = MONTHS.iter().position(|m| lower.starts_with(m)) {
            let rest = &sample[i..];
            let word_end = rest.find(|c: char| !c.is_alphabetic()).unwrap_or(rest.len());
            let after = rest[word_end..].trim_start_matches(['.', ' ']);
            let day: String = after.chars().take_while(char::is_ascii_digit).collect();
            let after = after[day.len()..].trim_start_matches(|c: char| c.is_alphabetic() && !day.is_empty() && "stndrh".contains(c)).trim_start_matches([',', ' ']);
            let year: String = after.chars().take_while(char::is_ascii_digit).collect();
            if let (Ok(d), Ok(y)) = (day.parse::<u32>(), year.parse::<i32>()) {
                if year.len() == 4 {
                    if let Some(x) = valid(y, mi as u32 + 1, d) {
                        hits.push((i, x));
                    }
                }
            }
        }
    }
    let first = hits.into_iter().min_by_key(|(i, _)| *i)?.1;
    (first.as_str() < today).then_some(first)
}

/// A full date in a file name: 20240528 / 2024-05-28, a leading 240528, 05.28.2024, 5.28.24.
fn filename_date(stem: &str) -> Option<String> {
    let b = stem.as_bytes();
    let digits_at = |i: usize, n: usize| i + n <= b.len() && b[i..i + n].iter().all(u8::is_ascii_digit) && (i == 0 || !b[i - 1].is_ascii_digit()) && b.get(i + n).is_none_or(|c| !c.is_ascii_digit());
    for i in 0..b.len() {
        if !stem.is_char_boundary(i) {
            continue;
        }
        if i + 8 <= b.len() && b[i..i + 4].iter().all(u8::is_ascii_digit) && (i == 0 || !b[i - 1].is_ascii_digit()) && &stem[i..i + 2] == "20" {
            let rest: String = stem[i + 4..].chars().take(6).filter(|c| c.is_ascii_digit()).take(4).collect();
            let raw: String = stem[i + 4..].chars().take(6).collect();
            let sep_ok = raw.starts_with(['-', '_']) || raw.chars().take(4).all(|c| c.is_ascii_digit());
            if rest.len() == 4 && sep_ok {
                if let Some(x) = valid(stem[i..i + 4].parse().ok()?, rest[..2].parse().ok()?, rest[2..].parse().ok()?) {
                    return Some(x);
                }
            }
        }
    }
    if digits_at(0, 6) {
        if let Some(x) = valid(2000 + stem[0..2].parse::<i32>().ok()?, stem[2..4].parse().ok()?, stem[4..6].parse().ok()?) {
            return Some(x);
        }
    }
    for (i, _) in stem.match_indices('.') {
        let before: String = stem[..i].chars().rev().take_while(char::is_ascii_digit).collect::<String>().chars().rev().collect();
        let after = &stem[i + 1..];
        let mid: String = after.chars().take_while(char::is_ascii_digit).collect();
        if before.is_empty() || before.len() > 2 || mid.is_empty() || mid.len() > 2 {
            continue;
        }
        let tail = &after[mid.len()..];
        let Some(y) = tail.strip_prefix('.') else { continue };
        let year: String = y.chars().take_while(char::is_ascii_digit).collect();
        let start = i - before.len();
        if start > 0 && stem.as_bytes()[start - 1].is_ascii_alphabetic() {
            continue; // a version: v2.1.22
        }
        let y = match year.len() {
            4 => year.parse::<i32>().ok(),
            2 => year.parse::<i32>().ok().map(|y| 2000 + y),
            _ => None,
        };
        if let (Some(y), Ok(m), Ok(d)) = (y, before.parse::<u32>(), mid.parse::<u32>()) {
            if let Some(x) = valid(y, m, d) {
                return Some(x);
            }
        }
    }
    None
}

/// A document's own date and where it came from (`text` | `filename` | `none`). Transcripts and
/// spreadsheets are dated by their file name only (their dates are what was said or entered).
pub fn exact_date(text: &str, path: &Path, today: &str) -> (Option<String>, &'static str) {
    let e = ext(path);
    if ["pdf", "pptx", "docx", "html", "htm", "md", "markdown", "txt"].contains(&e.as_str()) {
        if let Some(d) = content_date(text, today) {
            return (Some(d), "text");
        }
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    match filename_date(stem) {
        Some(d) => (Some(d), "filename"),
        None => (None, "none"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_follows_brain() {
        let page = |text: usize, drawings: usize| Page { text: "x".repeat(text), drawings, ..Default::default() };
        assert_eq!(route(&page(100, 0)), Some("thin-text"));
        assert_eq!(route(&Page { text: "x".into(), tables: vec!["| a |".into()], ..Default::default() }), None);
        assert_eq!(route(&page(3000, 70)), Some("dense-draw"));
        assert_eq!(route(&page(800, 30)), Some("diagram"));
        assert_eq!(route(&page(1500, 30)), None);
        assert_eq!(route(&page(1200, 4)), None);
    }

    #[test]
    fn skips_and_globs() {
        assert_eq!(skip_reason(".git/x.md", &[]).as_deref(), Some("hidden"));
        assert_eq!(skip_reason("a/._deck.pptx", &[]).as_deref(), Some("hidden"));
        assert_eq!(skip_reason("~$deck.pptx", &[]).as_deref(), Some("hidden"));
        assert_eq!(skip_reason("Archive/old.pdf", &["Archive/".into()]).as_deref(), Some("excluded by Archive/"));
        assert!(glob_match("a/b/Draft v1.docx", "*draft*"));
        assert!(glob_match("x/y/z.xlsx", "x/**/*.xlsx"));
        assert!(glob_match("x/z.xlsx", "x/**/*.xlsx"));
        assert!(!glob_match("x/y/z.xlsx", "x/*.xlsx"));
    }

    #[test]
    fn dates_as_brain_reads_them() {
        let today = "2026-10-30";
        assert_eq!(content_date("Readout — October 24, 2023\nas of 2026-12-31", today).as_deref(), Some("2023-10-24"));
        assert_eq!(content_date("Printed 2026-11-02 · then 2025-01-01", today), None);
        assert_eq!(content_date("Release v2.1.22 notes", today), None);
        assert_eq!(content_date("Updated 28.05.2024", today).as_deref(), Some("2024-05-28"));
        assert_eq!(content_date("Due 10/24/2023", today).as_deref(), Some("2023-10-24"));
        assert_eq!(filename_date("Forecast Tool 08.07.26").as_deref(), Some("2026-08-07"));
        assert_eq!(filename_date("260601 - Service Handbook").as_deref(), Some("2026-06-01"));
        assert_eq!(filename_date("notes_2026-05-28_final").as_deref(), Some("2026-05-28"));
        assert_eq!(filename_date("Proposal v2.1.22"), None);
        assert_eq!(filename_date("2026 Team Holiday Calendar"), None);
        let p = Path::new("Call.vtt");
        assert_eq!(exact_date("Today is 2024-01-01", p, today), (None, "none"));
    }
}

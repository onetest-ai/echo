//! PowerPoint and Word from their XML: exact text in order, headings, real table cells as Markdown
//! grids, and per slide what is drawn (shapes, connectors, pictures, charts, SmartArt) — the
//! routing signals. Hidden slides are left out (LibreOffice leaves them out of its PDF too, so slide
//! N here is page N there).

use super::xml::{self, El};
use super::{Page, Parsed};
use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

type Zip = zip::ZipArchive<std::fs::File>;

fn open(path: &Path) -> Result<Zip> {
    let f = std::fs::File::open(path)?;
    zip::ZipArchive::new(f).map_err(|e| anyhow!("not a readable Office file (truncated download?): {e}"))
}

/// The largest part read (the document's XML of a 300-slide deck is a few MB; a zip bomb unpacks to GB).
const MAX_PART: u64 = 64 << 20;

fn part(z: &mut Zip, name: &str) -> Result<Option<El>> {
    let f = match z.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(anyhow!("{name}: {e}")),
    };
    if f.size() > MAX_PART {
        return Err(anyhow!("{name} unpacks to {} MB: not read", f.size() >> 20));
    }
    let mut s = String::new();
    // The declared size can lie: read no more than the cap either way.
    f.take(MAX_PART + 1).read_to_string(&mut s).with_context(|| name.to_string())?;
    if s.len() as u64 > MAX_PART {
        return Err(anyhow!("{name} unpacks to more than {} MB: not read", MAX_PART >> 20));
    }
    Ok(Some(xml::parse(&s).with_context(|| name.to_string())?))
}

/// A part's relationships: id → (type's last word, target resolved against the part's folder).
fn rels(z: &mut Zip, part_name: &str) -> Result<BTreeMap<String, (String, String)>> {
    let (dir, file) = part_name.rsplit_once('/').unwrap_or(("", part_name));
    let name = format!("{dir}/_rels/{file}.rels");
    let mut out = BTreeMap::new();
    if let Some(r) = part(z, &name)? {
        for rel in r.all("Relationship") {
            let (Some(id), Some(target)) = (rel.attr("Id"), rel.attr("Target")) else { continue };
            let kind = rel.attr("Type").unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
            out.insert(id.to_string(), (kind, resolve(dir, target)));
        }
    }
    Ok(out)
}

fn resolve(dir: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A DrawingML / WordprocessingML table as a Markdown grid (the first row as its header).
fn grid(rows: Vec<Vec<String>>) -> Option<String> {
    let rows: Vec<Vec<String>> = rows.into_iter().filter(|r| r.iter().any(|c| !c.is_empty())).collect();
    let w = rows.iter().map(|r| r.len()).max()?;
    if w == 0 {
        return None;
    }
    let line = |r: &Vec<String>| {
        let mut cells: Vec<String> = r.iter().map(|c| c.replace('|', "\\|")).collect();
        cells.resize(w, String::new());
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0]), format!("|{}|", vec!["---"; w].join("|"))];
    out.extend(rows[1..].iter().map(line));
    Some(out.join("\n"))
}

/// A DrawingML paragraph's text (runs, fields, line breaks).
fn a_para(p: &El) -> String {
    let mut s = String::new();
    for k in p.children() {
        match k.name.as_str() {
            "r" | "fld" => s.push_str(&k.child("t").map(|t| t.text()).unwrap_or_default()),
            "br" => s.push('\n'),
            _ => {}
        }
    }
    s
}

fn a_text(body: &El) -> Vec<String> {
    body.all("p").into_iter().map(a_para).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
}

fn a_table(tbl: &El) -> Option<String> {
    grid(tbl.all("tr").into_iter().map(|tr| tr.all("tc").into_iter().map(|tc| clean(&a_text(tc).join(" "))).collect()).collect())
}

#[derive(Default)]
struct Slide {
    /// The slide's area (EMU²), for telling a picture from a logo.
    area: f64,
    title: Option<String>,
    lines: Vec<String>,
    tables: Vec<String>,
    /// Shapes with no text: boxes, bars, arrows — what a drawing is made of.
    drawings: usize,
    /// Free text boxes (not placeholders) and their characters: many short ones are a diagram's labels.
    labels: usize,
    label_chars: usize,
    connectors: usize,
    pictures: usize,
    charts: usize,
    smartart: usize,
    smartart_rels: Vec<String>,
}

fn walk_tree(t: &El, s: &mut Slide) {
    for k in t.children() {
        match k.name.as_str() {
            "sp" => {
                let ph = k.child("nvSpPr").and_then(|n| n.child("nvPr")).and_then(|n| n.child("ph"));
                let lines = k.child("txBody").map(a_text).unwrap_or_default();
                let chars: usize = lines.iter().map(|l| l.chars().count()).sum();
                let is_title = ph.is_some_and(|p| matches!(p.attr("type"), Some("title") | Some("ctrTitle")));
                if is_title && s.title.is_none() && !lines.is_empty() {
                    s.title = Some(clean(&lines.join(" ")));
                } else if !lines.is_empty() {
                    s.lines.extend(lines);
                }
                if ph.is_none() {
                    if chars == 0 {
                        s.drawings += 1;
                    } else {
                        s.labels += 1;
                        s.label_chars += chars;
                    }
                }
            }
            "cxnSp" => s.connectors += 1,
            "pic" => {
                // A picture bigger than a logo (≥ 4% of the slide) carries content.
                let ext = k.all("ext").into_iter().find(|e| e.attr("cx").is_some());
                let a = ext.map(|e| e.attr("cx").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) * e.attr("cy").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)).unwrap_or(0.0);
                if s.area <= 0.0 || a / s.area >= 0.04 {
                    s.pictures += 1;
                }
            }
            "grpSp" => walk_tree(k, s),
            "graphicFrame" => {
                let data = k.all("graphicData");
                let uri = data.first().and_then(|d| d.attr("uri")).unwrap_or("");
                if uri.ends_with("/table") {
                    if let Some(g) = k.all("tbl").first().and_then(|t| a_table(t)) {
                        s.tables.push(g);
                    }
                } else if uri.ends_with("/chart") {
                    s.charts += 1;
                } else if uri.ends_with("/diagram") {
                    s.smartart += 1;
                    if let Some(id) = k.all("relIds").first().and_then(|r| r.attr("r:dm")) {
                        s.smartart_rels.push(id.to_string());
                    }
                } else {
                    s.drawings += 1;
                }
            }
            "AlternateContent" => {
                if let Some(c) = k.child("Choice").or_else(|| k.child("Fallback")) {
                    walk_tree(c, s);
                }
            }
            _ => {}
        }
    }
}

pub fn pptx(path: &Path) -> Result<Parsed> {
    let mut z = open(path)?;
    let pres = part(&mut z, "ppt/presentation.xml")?.ok_or_else(|| anyhow!("no ppt/presentation.xml: not a PowerPoint file"))?;
    let prels = rels(&mut z, "ppt/presentation.xml")?;
    let area = pres.child("sldSz").map(|z| z.attr("cx").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) * z.attr("cy").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0)).unwrap_or(0.0);
    let order: Vec<String> = pres.all("sldId").into_iter().filter_map(|s| s.attr("r:id")).filter_map(|id| prels.get(id).map(|(_, t)| t.clone())).collect();
    let mut pages = vec![];
    for name in order {
        let Some(sld) = part(&mut z, &name)? else { continue };
        if sld.attr("show") == Some("0") {
            continue; // hidden
        }
        let srels = rels(&mut z, &name)?;
        let mut s = Slide { area, ..Default::default() };
        if let Some(tree) = sld.child("cSld").and_then(|c| c.child("spTree")) {
            walk_tree(tree, &mut s);
        }
        // SmartArt's words live in its data part.
        for id in &s.smartart_rels.clone() {
            if let Some((_, target)) = srels.get(id) {
                if let Some(d) = part(&mut z, target)? {
                    s.lines.extend(d.all("t").into_iter().map(|t| clean(&t.text())).filter(|t| !t.is_empty()));
                }
            }
        }
        // The speaker notes: text the presenter meant, often more than the slide says.
        let mut notes = vec![];
        if let Some((_, target)) = srels.values().find(|(k, _)| k == "notesSlide") {
            if let Some(n) = part(&mut z, target)? {
                for sp in n.all("sp") {
                    let ph = sp.child("nvSpPr").and_then(|x| x.child("nvPr")).and_then(|x| x.child("ph")).and_then(|p| p.attr("type"));
                    if ph == Some("body") {
                        notes.extend(sp.child("txBody").map(a_text).unwrap_or_default());
                    }
                }
            }
        }
        let mut text = s.lines.join("\n");
        if !notes.is_empty() {
            text.push_str(&format!("\n\nNotes: {}", notes.join(" ")));
        }
        pages.push(Page {
            n: pages.len() + 1,
            title: s.title,
            text,
            tables: s.tables,
            drawings: s.drawings,
            pictures: s.pictures,
            labels: s.labels,
            label_chars: s.label_chars,
            connectors: s.connectors,
            charts: s.charts,
            smartart: s.smartart,
        });
    }
    Ok(Parsed { method: "pptx-xml", pages, ..Default::default() })
}

/// "M:SS" / "H:MM:SS" → seconds.
fn clock_secs(t: &str) -> Option<f64> {
    let t = t.trim();
    let parts: Vec<&str> = t.split(':').collect();
    if !(2..=3).contains(&parts.len()) || parts.iter().any(|p| p.is_empty() || p.len() > 2 || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some(parts.iter().fold(0.0, |a, p| a * 60.0 + p.parse::<f64>().unwrap_or(0.0)))
}

type Turn = (f64, Option<String>, String);

/// A turn whose words are all in: kept when it has some.
fn close_turn(open: &mut Option<(f64, String, Vec<String>)>, turns: &mut Vec<Turn>) {
    if let Some((at, who, words)) = open.take() {
        let w = words.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
        if !w.is_empty() {
            turns.push((at, Some(who), w));
        }
    }
}

/// A meeting transcript saved as Word: Teams' (Brain's `read_teams_docx`: one paragraph per turn —
/// the speaker's run(s), a run that is exactly "M:SS" / "H:MM:SS", then the words, or all of it in one
/// run: "Name  0:05 words"; "started / stopped transcription" lines dropped) and Otter's ("Name  00:03"
/// on a line of its own, the words in the paragraphs after it). What comes before the first turn —
/// title, date, duration, Otter's keywords and speakers — is its preface. Only runs' own text is read
/// (an avatar's drawing never enters). None: not a transcript.
fn teams_turns(body: &El) -> Option<(Vec<String>, Vec<Turn>)> {
    let mut turns: Vec<Turn> = vec![];
    let mut preface: Vec<String> = vec![];
    // A turn whose words come in the next paragraphs (Otter).
    let mut open: Option<(f64, String, Vec<String>)> = None;
    for p in body.children().filter(|p| p.name == "p") {
        let runs: Vec<String> = p
            .children()
            .filter(|r| r.name == "r" && r.child("t").is_some())
            .map(|r| r.children().map(|k| match k.name.as_str() { "t" => k.text(), "br" | "tab" => " ".into(), _ => String::new() }).collect())
            .collect();
        let joined = runs.concat();
        let line = joined.trim();
        if line.is_empty() {
            continue;
        }
        let i = (1..runs.len()).find(|&k| clock_secs(&runs[k]).is_some());
        let head = match i {
            Some(i) => Some((runs[..i].concat().trim().to_string(), clock_secs(&runs[i]).unwrap_or(0.0), runs[i + 1..].iter().map(|r| r.trim()).filter(|r| !r.is_empty()).collect::<Vec<_>>().join(" "))),
            None => line.find("  ").and_then(|gap| {
                // "Name  0:05 words": the name, two spaces or more, a clock.
                let rest = line[gap..].trim_start();
                let clock: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':').collect();
                let at = clock_secs(&clock)?;
                let name = line[..gap].trim();
                (!name.is_empty() && name.split_whitespace().count() <= 6).then(|| (name.to_string(), at, rest[clock.len()..].trim().to_string()))
            }),
        };
        match head {
            Some((who, at, words)) if !who.is_empty() => {
                close_turn(&mut open, &mut turns);
                open = Some((at, who, if words.is_empty() { vec![] } else { vec![words] }));
            }
            _ => {
                if line.to_lowercase().ends_with("started transcription") || line.to_lowercase().ends_with("stopped transcription") {
                    continue;
                }
                match open.as_mut() {
                    Some((_, _, words)) => words.push(line.to_string()),
                    None if turns.is_empty() => preface.push(line.to_string()),
                    None => {}
                }
            }
        }
    }
    close_turn(&mut open, &mut turns);
    // Two turns at least, else it's a document that happens to hold a clock.
    (turns.len() >= 2).then_some((preface, turns))
}

/// Word: headings (by the style's name, so localized style ids work), lists, tables as grids, in
/// order; drawings and charts counted (their meaning isn't in the text). A Teams transcript saved as
/// Word is read as one: packed turns, like a .vtt.
pub fn docx(path: &Path) -> Result<Parsed> {
    let mut z = open(path)?;
    let doc = part(&mut z, "word/document.xml")?.ok_or_else(|| anyhow!("no word/document.xml: not a Word file"))?;
    if let Some((preface, turns)) = doc.child("body").and_then(teams_turns) {
        let head = if preface.is_empty() { String::new() } else { format!("{}\n\n", preface.join("\n\n")) };
        return Ok(Parsed { method: "meeting-transcript", md: Some(format!("{head}{}", super::text::pack(turns))), ..Default::default() });
    }
    let mut style_names: BTreeMap<String, String> = BTreeMap::new();
    if let Some(st) = part(&mut z, "word/styles.xml")? {
        for s in st.all("style") {
            if let (Some(id), Some(name)) = (s.attr("styleId"), s.child("name").and_then(|n| n.attr("val"))) {
                style_names.insert(id.to_string(), name.to_lowercase());
            }
        }
    }
    let heading = |p: &El| -> Option<usize> {
        let id = p.child("pPr").and_then(|x| x.child("pStyle")).and_then(|s| s.attr("val"))?;
        let name = style_names.get(id).cloned().unwrap_or_else(|| id.to_lowercase());
        if name == "title" {
            return Some(1);
        }
        name.strip_prefix("heading").and_then(|n| n.trim().parse::<usize>().ok()).map(|n| (n + 1).min(6))
    };
    let w_para = |p: &El| -> String {
        let mut s = String::new();
        for r in p.all("r") {
            for k in r.children() {
                match k.name.as_str() {
                    "t" => s.push_str(&k.text()),
                    "tab" => s.push(' '),
                    "br" | "cr" => s.push('\n'),
                    _ => {}
                }
            }
        }
        s.trim().to_string()
    };
    let body = doc.child("body").ok_or_else(|| anyhow!("word/document.xml has no body"))?;
    let mut md = String::new();
    let mut figures = 0;
    for b in body.children() {
        match b.name.as_str() {
            "p" => {
                figures += b.count("drawing") + b.count("pict");
                let t = w_para(b);
                if t.is_empty() {
                    continue;
                }
                match heading(b) {
                    Some(level) => md.push_str(&format!("\n{} {}\n\n", "#".repeat(level), clean(&t))),
                    None if b.child("pPr").and_then(|x| x.child("numPr")).is_some() => md.push_str(&format!("- {t}\n")),
                    None => md.push_str(&format!("{t}\n\n")),
                }
            }
            "tbl" => {
                figures += b.count("drawing");
                let rows: Vec<Vec<String>> = b
                    .children()
                    .filter(|r| r.name == "tr")
                    .map(|tr| tr.children().filter(|c| c.name == "tc").map(|tc| clean(&tc.all("p").into_iter().map(w_para).collect::<Vec<_>>().join(" "))).collect())
                    .collect();
                if let Some(g) = grid(rows) {
                    md.push_str(&format!("\n{g}\n\n"));
                }
            }
            _ => {}
        }
    }
    Ok(Parsed { method: "docx-xml", md: Some(md.trim().to_string()), figures, ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_targets_resolve() {
        assert_eq!(resolve("ppt", "slides/slide1.xml"), "ppt/slides/slide1.xml");
        assert_eq!(resolve("ppt/slides", "../notesSlides/notesSlide1.xml"), "ppt/notesSlides/notesSlide1.xml");
        assert_eq!(resolve("ppt/slides", "/ppt/diagrams/data1.xml"), "ppt/diagrams/data1.xml");
    }

    #[test]
    fn teams_transcripts_in_word() {
        let x = |s: &str| xml::parse(s).unwrap();
        let body = x(r#"<w:body>
            <w:p><w:r><w:t>Customer Service Strategy</w:t></w:r></w:p>
            <w:p><w:r><w:t>October 2, 2026</w:t></w:r></w:p>
            <w:p><w:r><w:t>42m 10s</w:t></w:r></w:p>
            <w:p><w:r><w:t>Ana Lopez started transcription</w:t></w:r></w:p>
            <w:p><w:r><w:drawing><wp:posOffset>12345</wp:posOffset></w:drawing></w:r><w:r><w:t>Ana Lopez</w:t></w:r><w:r><w:t>0:05</w:t></w:r><w:r><w:t>We start with the IVR.</w:t></w:r></w:p>
            <w:p><w:r><w:t>Bo Chen  1:02:07 The budget is approved.</w:t></w:r></w:p>
        </w:body>"#);
        let (pre, t) = teams_turns(&body).unwrap();
        assert_eq!(pre, vec!["Customer Service Strategy", "October 2, 2026", "42m 10s"]);
        assert_eq!(t, vec![(5.0, Some("Ana Lopez".into()), "We start with the IVR.".into()), (3727.0, Some("Bo Chen".into()), "The budget is approved.".into())]);
        // Otter: the name and clock on a line of their own, the words after.
        let otter = x(r#"<w:body><w:p><w:r><w:t>SPEAKERS</w:t></w:r></w:p><w:p><w:r><w:t>Speaker 1  00:03</w:t></w:r></w:p><w:p><w:r><w:t>You're on.</w:t></w:r></w:p><w:p><w:r><w:t>Can you see it?</w:t></w:r></w:p><w:p><w:r><w:t>Speaker 2  00:09</w:t></w:r></w:p><w:p><w:r><w:t>Yes.</w:t></w:r></w:p></w:body>"#);
        let (pre, t) = teams_turns(&otter).unwrap();
        assert_eq!(pre, vec!["SPEAKERS"]);
        assert_eq!(t[0], (3.0, Some("Speaker 1".into()), "You're on. Can you see it?".into()));
        assert_eq!(t[1].2, "Yes.");
        assert!(teams_turns(&x("<w:body><w:p><w:r><w:t>Plain text, 10:30 meeting</w:t></w:r></w:p></w:body>")).is_none());
    }

    #[test]
    fn tables_become_grids() {
        let g = grid(vec![vec!["Phase".into(), "Weeks".into()], vec!["Build | test".into(), "7".into()], vec![String::new(), String::new()]]).unwrap();
        assert_eq!(g, "| Phase | Weeks |\n|---|---|\n| Build \\| test | 7 |");
    }
}

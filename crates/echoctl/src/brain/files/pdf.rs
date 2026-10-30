//! PDF pages through PDFium (BSD-3; loaded at runtime, like the NeMo runtime): each page's text layer
//! and what is drawn on it (vector paths, images), and a page rendered to PNG for the page reader.
//! Where it is: `ECHOCTL_PDFIUM_DIR`, else the app's own copy (macOS `Echo.app/Contents/Frameworks/pdfium`,
//! Windows `<app folder>\pdfium`, from `scripts/fetch-pdfium.sh`), else `<config dir>/pdfium`.

use super::{Page, Parsed};
use crate::paths;
use anyhow::{anyhow, Result};
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn candidates() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(d) = std::env::var_os("ECHOCTL_PDFIUM_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        if cfg!(target_os = "macos") {
            dirs.push(exe_dir.join("../Frameworks/pdfium"));
        }
        dirs.push(exe_dir.join("pdfium"));
    }
    dirs.push(paths::config_dir().join("pdfium"));
    dirs
}

/// The library, if this computer has it.
pub fn library() -> Option<PathBuf> {
    candidates().into_iter().map(|d| Pdfium::pdfium_platform_library_name_at_path(&d)).find(|p| p.exists())
}

static PDFIUM: OnceLock<std::result::Result<Pdfium, String>> = OnceLock::new();

fn pdfium() -> Result<&'static Pdfium> {
    PDFIUM
        .get_or_init(|| {
            let lib = library().ok_or_else(|| "PDFium isn't installed (scripts/fetch-pdfium.sh)".to_string())?;
            Pdfium::bind_to_library(&lib).map(Pdfium::new).map_err(|e| format!("PDFium: {e}"))
        })
        .as_ref()
        .map_err(|e| anyhow!("{e}"))
}

/// Vector paths on a page (inside form objects too): Brain's "drawings".
fn count_objects<'a>(objs: impl Iterator<Item = PdfPageObject<'a>>, paths: &mut usize, images: &mut usize, depth: usize) {
    if depth > 16 {
        return; // forms within forms this deep: a crafted file, not a page
    }
    for o in objs {
        match o.object_type() {
            PdfPageObjectType::Path => *paths += 1,
            PdfPageObjectType::Image => *images += 1,
            PdfPageObjectType::XObjectForm => {
                if let Some(f) = o.as_x_object_form_object() {
                    count_objects(f.iter(), paths, images, depth + 1);
                }
            }
            _ => {}
        }
    }
}

pub fn read(path: &Path) -> Result<Parsed> {
    let doc = pdfium()?.load_pdf_from_file(path, None).map_err(|e| anyhow!("not a readable PDF: {e}"))?;
    let mut pages = vec![];
    for (i, page) in doc.pages().iter().enumerate() {
        let text = page.text().map(|t| t.all()).unwrap_or_default();
        let (mut paths, mut images) = (0, 0);
        count_objects(page.objects().iter(), &mut paths, &mut images, 0);
        pages.push(Page { n: i + 1, text: text.trim().to_string(), drawings: paths, pictures: images, ..Default::default() });
    }
    running_titles(&mut pages);
    Ok(Parsed { method: "pdfium", pages, ..Default::default() })
}

/// A page's title is its first line with words that isn't a running header or footer (a line — page
/// numbers aside — on more than a third of the pages: "PRIVILEGED AND CONFIDENTIAL 28").
fn running_titles(pages: &mut [Page]) {
    // A line without the page number at its start or end ("Confidential 28", "3 | Acme").
    let key = |l: &str| l.trim().trim_matches(|c: char| c.is_ascii_digit() || c.is_whitespace() || "|·-–/".contains(c)).to_lowercase();
    let mut seen: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for p in pages.iter() {
        let mut once = std::collections::BTreeSet::new();
        for l in p.text.lines() {
            let k = key(l);
            if k.chars().filter(|c| c.is_alphabetic()).count() >= 3 && once.insert(k.clone()) {
                *seen.entry(k).or_default() += 1;
            }
        }
    }
    let n = pages.len();
    let running = |k: &str| n >= 3 && seen.get(k).is_some_and(|c| *c * 3 > n);
    for p in pages.iter_mut() {
        p.title = p
            .text
            .lines()
            .map(str::trim)
            .find(|l| {
                let k = key(l);
                k.chars().filter(|c| c.is_alphabetic()).count() >= 3 && !running(&k)
            })
            .map(|l| l.chars().take(90).collect());
    }
}

/// Page `n` (1-based) as a PNG, at most `width` pixels wide (and 2× that tall).
pub fn render(path: &Path, n: usize, width: i32) -> Result<Vec<u8>> {
    let doc = pdfium()?.load_pdf_from_file(path, None).map_err(|e| anyhow!("not a readable PDF: {e}"))?;
    let page = doc.pages().get((n - 1) as i32).map_err(|e| anyhow!("page {n}: {e}"))?;
    let img = page
        .render_with_config(&PdfRenderConfig::new().set_target_width(width).set_maximum_height(width * 2))
        .and_then(|b| b.as_image())
        .map_err(|e| anyhow!("rendering page {n}: {e}"))?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.to_rgb8().write_to(&mut out, image::ImageFormat::Png)?;
    Ok(out.into_inner())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_headers_are_not_titles() {
        let mut pages: Vec<Page> = (1..=4).map(|i| Page { n: i, text: format!("PRIVILEGED AND CONFIDENTIAL {i}\nSection {i} plan"), ..Default::default() }).collect();
        running_titles(&mut pages);
        assert_eq!(pages[1].title.as_deref(), Some("Section 2 plan"));
    }
}

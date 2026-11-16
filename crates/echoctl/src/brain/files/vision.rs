//! The page reader (Brain's `vision_prep` + its Sonnet readers): a page flagged visual is read from its
//! image into faithful structured Markdown — a flow as an ordered list, a timeline as a table, a
//! diagram's relations spelled out — by Claude on the default AI profile (its minutes model; images as
//! content blocks, no tools) or a vision endpoint (Settings → Brain). Readings are cached by the
//! page image's hash and the prompt's version: an unchanged page is never read twice.

use crate::brain::embed::{self, quote};
use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};

/// Bump when the instructions change in a way worth reading every page again (each re-read is a
/// model request): pages read under an older version are read again.
pub const PROMPT_VERSION: i64 = 1;
/// Pages in one request.
pub const PER_REQUEST: usize = 4;

const SYSTEM: &str = "You transcribe slide and document page images into faithful structured Markdown for a knowledge base. \
Preserve structure; do not summarize detail away and do not invent:\n\
- a process or flow → an ORDERED list of its stages with their sub-items, and the sequence (X → Y)\n\
- a timeline or Gantt chart → a TABLE (rows = activities, columns = the periods or dates shown)\n\
- a diagram → spell out its relationships (A contains B; C precedes D; E reports to F)\n\
- a chart → its title, axes, series and the values shown\n\
- when extracted table cells are given for a page, TRUST them for numbers; your job is the surrounding meaning and labels, not re-reading the grid\n\
- leave out what every page repeats or what only decorates: logos, page or slide numbers, confidentiality marks, footers, decorative icons and their descriptions\n\
- copy names, identifiers, codes and numbers character for character; write [illegible] for text too small to read — never guess\n\
- start each page with a `# <slide or page title>` line; use only ### and deeper headings inside a page\n\
Each page is read on its own: never refer to other pages.";

pub fn ensure_schema(c: &Connection) -> Result<()> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS page_render(img_sha TEXT NOT NULL, prompt_v INTEGER NOT NULL, md TEXT NOT NULL, reader TEXT, at TEXT, PRIMARY KEY(img_sha, prompt_v));")?;
    Ok(())
}

pub fn cached(c: &Connection, img_sha: &str) -> Result<Option<String>> {
    Ok(c.query_row("SELECT md FROM page_render WHERE img_sha = ?1 AND prompt_v = ?2", params![img_sha, PROMPT_VERSION], |r| r.get(0)).optional()?)
}

pub fn save(c: &Connection, img_sha: &str, md: &str, reader: &str) -> Result<()> {
    c.execute(
        "INSERT OR REPLACE INTO page_render(img_sha, prompt_v, md, reader, at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![img_sha, PROMPT_VERSION, md, reader, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

/// A page to read.
pub struct Item {
    pub png: Vec<u8>,
    /// "<file> p<NN>".
    pub hint: String,
    /// Extracted table cells (Markdown), trusted for numbers.
    pub tables: String,
}

/// Who reads pages now, or why nobody does.
pub enum Reader {
    Claude,
    Endpoint(Box<embed::Settings>),
}

impl Reader {
    pub fn name(&self) -> String {
        match self {
            Reader::Claude => {
                let p = crate::ai::default_id();
                format!("claude:{p}")
            }
            Reader::Endpoint(s) => format!("endpoint:{}", s.vision_model),
        }
    }
}

pub fn reader() -> std::result::Result<Reader, String> {
    let s = crate::brain::settings();
    match s.pages() {
        "off" => Err("the page reader is off (Settings → Knowledge): those pages are indexed from their text".into()),
        "endpoint" if s.vision_endpoint.trim().is_empty() || s.vision_model.trim().is_empty() => Err("no vision endpoint set (Settings → Knowledge)".into()),
        "endpoint" => Ok(Reader::Endpoint(Box::new(s))),
        _ => Ok(Reader::Claude),
    }
}

fn prompt(items: &[Item]) -> String {
    let mut t = format!("Transcribe these {} page images, in order.\n\n", items.len());
    for (i, it) in items.iter().enumerate() {
        t.push_str(&format!("Image {}: {}\n", i + 1, it.hint));
        if !it.tables.trim().is_empty() {
            t.push_str(&format!("Extracted table cells for image {} (trust these for numbers):\n{}\n", i + 1, it.tables.trim()));
        }
        t.push('\n');
    }
    t.push_str("Answer with JSON only: {\"pages\": [{\"image\": 1, \"md\": \"# Title\\n…\"}, …]} — one entry per image, in order.");
    t
}

/// The answer's Markdown per image, checked: one non-empty entry per image.
fn parse_answer(text: &str, n: usize) -> Result<Vec<String>> {
    let s = text.find('{').ok_or_else(|| anyhow!("the page reader's answer has no JSON"))?;
    let e = text.rfind('}').ok_or_else(|| anyhow!("the page reader's answer has no JSON"))?;
    let v: Value = serde_json::from_str(&text[s..=e]).map_err(|err| anyhow!("the page reader's JSON doesn't parse: {err}"))?;
    let mut out = vec![String::new(); n];
    for p in v["pages"].as_array().into_iter().flatten() {
        let i = p["image"].as_u64().unwrap_or(0) as usize;
        if (1..=n).contains(&i) {
            out[i - 1] = p["md"].as_str().unwrap_or("").trim().to_string();
        }
    }
    if let Some(i) = out.iter().position(String::is_empty) {
        bail!("the page reader left image {} of {n} without an answer", i + 1);
    }
    Ok(out)
}

/// Read up to `PER_REQUEST` pages in one request.
pub fn read(r: &Reader, items: &[Item]) -> Result<Vec<String>> {
    let text = prompt(items);
    let answer = match r {
        Reader::Claude => {
            let ready = crate::ai::ready(&crate::ai::default_id())?;
            let pngs: Vec<Vec<u8>> = items.iter().map(|i| i.png.clone()).collect();
            crate::livenotes::complete_vision(ready.profile(), SYSTEM, &pngs, &text, 8000)?
        }
        Reader::Endpoint(s) => endpoint(s, items, &text)?,
    };
    parse_answer(&answer, items.len())
}

fn endpoint(s: &embed::Settings, items: &[Item], text: &str) -> Result<String> {
    use base64::Engine as _;
    let b64: Vec<String> = items.iter().map(|i| base64::engine::general_purpose::STANDARD.encode(&i.png)).collect();
    let base = s.vision_endpoint.trim().trim_end_matches('/');
    let openai = s.vision_api == "openai";
    let (url, body) = if openai {
        let mut content = vec![json!({"type": "text", "text": text})];
        content.extend(b64.iter().map(|b| json!({"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b}")}})));
        (
            if base.ends_with("/v1") { format!("{base}/chat/completions") } else { format!("{base}/v1/chat/completions") },
            json!({"model": s.vision_model, "max_tokens": 8000, "temperature": 0, "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": content}]}),
        )
    } else {
        (
            format!("{base}/api/chat"),
            json!({"model": s.vision_model, "stream": false, "options": {"temperature": 0}, "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": text, "images": b64}]}),
        )
    };
    let mut cfg = format!(
        "url = {}\nrequest = \"POST\"\nheader = \"Content-Type: application/json\"\ndata-binary = {}\nmax-time = 600\nconnect-timeout = 5\nwrite-out = \"\\n%{{http_code}}\"\n",
        quote(&url),
        quote(&body.to_string())
    );
    if let Some(k) = s.vision_key.as_deref().filter(|k| !k.is_empty()) {
        cfg.push_str(&format!("header = {}\n", quote(&format!("Authorization: Bearer {k}"))));
    }
    if crate::ai::loopback(base) {
        cfg.push_str("noproxy = \"*\"\n");
    }
    let mut c = Command::new("curl");
    #[cfg(windows)]
    {
        use crate::app::NoWindow;
        c = c.no_window();
    }
    let mut child = c.args(["-q", "-sS", "-K", "-"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("running curl")?;
    child.stdin.take().expect("stdin").write_all(cfg.as_bytes())?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        bail!("the vision endpoint at {base} didn't answer: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let raw = String::from_utf8_lossy(&out.stdout).to_string();
    let (body, status) = raw.rsplit_once('\n').unwrap_or((raw.as_str(), ""));
    let v: Value = serde_json::from_str(body).map_err(|_| anyhow!("the vision endpoint answered HTTP {}: {}", status.trim(), body.chars().take(200).collect::<String>()))?;
    if let Some(e) = v.get("error") {
        bail!("the vision endpoint: {}", e["message"].as_str().or(e.as_str()).unwrap_or("error"));
    }
    let t = if openai { v["choices"][0]["message"]["content"].as_str() } else { v["message"]["content"].as_str() };
    t.map(str::to_owned).ok_or_else(|| anyhow!("the vision endpoint gave no text"))
}

/// A reading inside a document: its own `#`/`##` headings demoted, so one page stays one section.
pub fn demote(md: &str) -> String {
    md.lines()
        .map(|l| {
            let t = l.trim_start();
            if let Some(rest) = t.strip_prefix("## ") {
                format!("**{}**", rest.trim())
            } else if let Some(rest) = t.strip_prefix("# ") {
                format!("**{}**", rest.trim())
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_are_checked() {
        let a = "Here: {\"pages\": [{\"image\": 2, \"md\": \"# B\"}, {\"image\": 1, \"md\": \"# A\\n1. x → y\"}]}";
        assert_eq!(parse_answer(a, 2).unwrap(), vec!["# A\n1. x → y".to_string(), "# B".to_string()]);
        assert!(parse_answer("{\"pages\": [{\"image\": 1, \"md\": \"# A\"}]}", 2).is_err());
        assert!(parse_answer("no json", 1).is_err());
        assert_eq!(demote("# Title\n## Part\n### Keep"), "**Title**\n**Part**\n### Keep");
    }
}

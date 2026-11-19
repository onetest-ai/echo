//! Text as it is (Markdown, plain text), transcripts (WebVTT, SRT) packed into turns like Echo's own
//! calls, and HTML as its text (degraded: no JavaScript, no images; a page with almost no text is a
//! JavaScript-rendered one and is skipped rather than stored near-empty).

use super::Parsed;
use anyhow::Result;
use std::path::Path;

fn read_lossy(path: &Path) -> Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).replace("\r\n", "\n"))
}

pub fn plain(path: &Path) -> Result<Parsed> {
    Ok(Parsed { method: "text", md: Some(read_lossy(path)?.trim().to_string()), ..Default::default() })
}

/// Seconds from `00:01:02.500` / `01:02,500` / `1:02`.
fn secs(t: &str) -> Option<f64> {
    let t = t.trim().replace(',', ".");
    let parts: Vec<&str> = t.split(':').collect();
    let nums: Vec<f64> = parts.iter().map(|p| p.parse::<f64>()).collect::<Result<_, _>>().ok()?;
    Some(nums.iter().fold(0.0, |acc, n| acc * 60.0 + n))
}

fn clock(s: f64) -> String {
    let s = s.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// A cue's speaker and words: `<v Name>text`, or `Name: text`.
fn speaker(text: &str) -> (Option<String>, String) {
    if let Some(rest) = text.strip_prefix("<v ") {
        if let Some((name, words)) = rest.split_once('>') {
            return (Some(name.trim().to_string()), words.replace("</v>", "").trim().to_string());
        }
    }
    if let Some((name, words)) = text.split_once(": ") {
        if name.len() <= 40 && name.split_whitespace().count() <= 4 && !name.contains(['.', '?', '!']) {
            return (Some(name.trim().to_string()), words.trim().to_string());
        }
    }
    (None, text.trim().to_string())
}

/// Turns (same speaker in a row joined; a turn under 20 characters folded into the previous one as
/// `[Name: text]`), packed into sections of about 1000 characters, `### [hh:mm:ss] Names`.
pub fn transcript(path: &Path) -> Result<Parsed> {
    let raw = read_lossy(path)?;
    let mut turns: Vec<(f64, Option<String>, String)> = vec![];
    for block in raw.split("\n\n") {
        let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let Some(i) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let Some(start) = lines[i].split("-->").next().and_then(secs) else { continue };
        let text = lines[i + 1..].join(" ");
        if text.is_empty() {
            continue;
        }
        let (who, words) = speaker(&text);
        turns.push((start, who, words));
    }
    Ok(Parsed { method: "transcript", md: Some(pack(turns)), ..Default::default() })
}

/// Turns as sections (Brain's `--fold-interjections 20 --pack-turns 1000`): the same speaker in a row
/// joined; a turn under 20 characters folded into the previous one as `[Name: text]`; packed into
/// sections of about 1000 characters, `### [hh:mm:ss] Names`, lines `[hh:mm:ss] Name: text`.
pub fn pack(raw: Vec<(f64, Option<String>, String)>) -> String {
    let mut turns: Vec<(f64, Option<String>, String)> = vec![];
    for (start, who, words) in raw {
        match turns.last_mut() {
            Some(last) if last.1 == who => {
                last.2.push(' ');
                last.2.push_str(&words);
            }
            Some(last) if words.chars().count() < 20 => last.2.push_str(&format!(" [{}: {words}]", who.as_deref().unwrap_or("?"))),
            _ => turns.push((start, who, words)),
        }
    }
    let mut md = String::new();
    let mut pack: Vec<&(f64, Option<String>, String)> = vec![];
    let flush = |pack: &mut Vec<&(f64, Option<String>, String)>, md: &mut String| {
        if let Some(first) = pack.first() {
            let mut names: Vec<&str> = vec![];
            for n in pack.iter().filter_map(|t| t.1.as_deref()) {
                if !names.contains(&n) {
                    names.push(n);
                }
            }
            md.push_str(&format!("\n### [{}] {}\n\n", clock(first.0), names.join(", ")));
            for t in pack.iter() {
                md.push_str(&format!("[{}] {}{}\n", clock(t.0), t.1.as_deref().map(|n| format!("{n}: ")).unwrap_or_default(), t.2));
            }
        }
        pack.clear();
    };
    let mut size = 0;
    for t in &turns {
        if size + t.2.len() > 1000 && !pack.is_empty() {
            flush(&mut pack, &mut md);
            size = 0;
        }
        size += t.2.len();
        pack.push(t);
    }
    flush(&mut pack, &mut md);
    md.trim().to_string()
}

/// The text of an HTML page; under 220 characters (a JavaScript-rendered page) → skipped.
pub fn html(path: &Path) -> Result<Parsed> {
    let raw = read_lossy(path)?;
    let mut s = raw.clone();
    // ASCII lowercasing keeps every byte where it was (full lowercasing changes lengths: "İ"), so its
    // offsets cut the original.
    for tag in ["script", "style", "noscript"] {
        while let Some(a) = s.to_ascii_lowercase().find(&format!("<{tag}")) {
            let end = s.to_ascii_lowercase()[a..].find(&format!("</{tag}>")).map(|e| a + e + tag.len() + 3).unwrap_or(s.len());
            s.replace_range(a..end, " ");
        }
    }
    let mut text = String::new();
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                text.push(' ');
            }
            _ if !inside => text.push(c),
            _ => {}
        }
    }
    let text = text.replace("&nbsp;", " ").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    let text: String = text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    if text.chars().count() < 220 {
        return Ok(Parsed { method: "html", skip: Some("a JavaScript-rendered page (almost no text without a browser)".into()), ..Default::default() });
    }
    Ok(Parsed { method: "html", md: Some(text), fidelity: "degraded", ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_with_case_changing_letters() {
        let dir = std::env::temp_dir().join(format!("echo-html-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.html");
        std::fs::write(&p, format!("<p>{}</p><script>x='İ K'</script><p>İstanbul office</p>", "Quarterly review of the contact centre. ".repeat(8))).unwrap();
        let md = html(&p).unwrap().md.unwrap();
        assert!(md.contains("İstanbul office") && !md.contains("x='"), "{md}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn transcripts_pack_turns() {
        let dir = std::env::temp_dir().join(format!("echo-vtt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.vtt");
        std::fs::write(&p, "WEBVTT\n\n1\n00:00:01.000 --> 00:00:04.000\n<v Ana Lopez>We need the budget by Friday.</v>\n\n2\n00:00:05.000 --> 00:00:06.000\n<v Bo Chen>Sure.</v>\n\n3\n00:00:07.000 --> 00:00:09.000\n<v Ana Lopez>And the RACI.</v>\n").unwrap();
        let md = transcript(&p).unwrap().md.unwrap();
        assert!(md.starts_with("### [00:00:01] Ana Lopez"), "{md}");
        assert!(md.contains("Ana Lopez: We need the budget by Friday. [Bo Chen: Sure.] And the RACI."), "{md}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

//! A call as a Brain document: its minutes (their sections), the transcript as packed turns with
//! real speaker names, their organisation and the time, and the meeting chat. Nothing is copied:
//! the document is built from the call's files each sync and hashed; each chunk links back to the
//! moment in the call (`echo://call/<name>?t=<s>`) and keeps the call's threads and client.

use serde_json::{json, Value};
use std::path::Path;

use crate::calls;

/// About this many characters of transcript per section (Brain's `--pack-turns 1000`).
const PACK: usize = 1000;

pub struct Doc {
    pub source: String,
    pub title: String,
    pub event_date: Option<String>,
    pub md: String,
    /// What every chunk of it refers to: the call, its threads, its client.
    pub refs: Value,
    /// The call's folder name (for links).
    pub call: String,
}

fn clock(t: f64) -> String {
    let s = t.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// `[12:05]` or `[1:02:05]` at the start of a title → seconds.
pub fn title_time(title: &str) -> Option<f64> {
    let t = title.trim().strip_prefix('[')?;
    let (clock, _) = t.split_once(']')?;
    let p: Vec<f64> = clock.split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    Some(p.iter().fold(0.0, |a, x| a * 60.0 + x))
}

/// The call in `dir` as a document, or None when it has nothing to read yet (no transcript).
pub fn document(dir: &Path) -> Option<Doc> {
    let meta = calls::read_json(&dir.join("meta.json"));
    let segs = calls::read_json(&dir.join("segments.json"));
    let segs = segs.as_array()?;
    if segs.is_empty() {
        return None;
    }
    let name = dir.file_name()?.to_string_lossy().into_owned();
    let stamp = calls::stamp_of(dir);
    let title = meta["title"].as_str().unwrap_or(&name).trim().to_string();
    let started = meta["started_at"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
    let date = started.map(|t| t.with_timezone(&chrono::Local).date_naive().to_string());
    let me = crate::config::Config::load().ok().and_then(|c| c.me).map(|m| m.name).unwrap_or_else(|| "Me".into());
    // A speaker's organisation, from the invite's addresses and the clients' domains.
    let clients = crate::projects::clients().unwrap_or_default();
    let meeting = crate::meeting::MeetingContext::load(dir).ok().flatten();
    let org_of = |who: &str| -> Option<String> {
        let m = meeting.as_ref()?;
        let p = m.organizer.iter().chain(&m.attendees).find(|p| crate::people::display(&p.name).eq_ignore_ascii_case(who))?;
        let dom = p.email.as_deref()?.rsplit_once('@')?.1.to_lowercase();
        clients.iter().find(|c| c.domains.iter().any(|d| dom == *d || dom.ends_with(&format!(".{d}")))).map(|c| c.name.clone())
    };
    let who = |s: &Value| -> String {
        match s["speaker"].as_str() {
            Some("me") | Some("mic") => me.clone(),
            _ => s["name"].as_str().map(crate::people::display).unwrap_or_else(|| "Remote".into()),
        }
    };

    let mut md = format!("# {title}\n");
    if let Some(d) = &date {
        md.push_str(&format!("<!-- event_date: {d} -->\n"));
    }
    // The minutes, their own headings one level down.
    if let Ok(notes) = std::fs::read_to_string(dir.join("notes.md")) {
        let body: Vec<String> = notes
            .lines()
            .skip_while(|l| l.starts_with("# ") || l.trim().is_empty())
            .map(|l| if l.starts_with('#') { format!("##{l}") } else { l.to_string() })
            .collect();
        if body.iter().any(|l| !l.trim().is_empty()) {
            md.push_str("\n## Minutes\n\n");
            md.push_str(&body.join("\n"));
            md.push('\n');
        }
    }
    // The transcript: consecutive lines of one speaker as one turn, turns packed into sections.
    let mut turns: Vec<(f64, String, String)> = vec![];
    for s in segs {
        let text = s["text"].as_str().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let w = who(s);
        let t = s["start"].as_f64().unwrap_or(0.0);
        match turns.last_mut() {
            Some(last) if last.1 == w => {
                last.2.push(' ');
                last.2.push_str(text);
            }
            _ => turns.push((t, w, text.to_string())),
        }
    }
    md.push_str("\n## Transcript\n");
    let mut pack: Vec<&(f64, String, String)> = vec![];
    let mut size = 0;
    let flush = |pack: &mut Vec<&(f64, String, String)>, md: &mut String| {
        if pack.is_empty() {
            return;
        }
        let mut names: Vec<String> = vec![];
        for (_, w, _) in pack.iter() {
            if !names.contains(w) {
                names.push(w.clone());
            }
        }
        md.push_str(&format!("\n### [{}] {}\n\n", clock(pack[0].0), names.join(", ")));
        for (t, w, text) in pack.iter() {
            let org = org_of(w).map(|o| format!(" ({o})")).unwrap_or_default();
            md.push_str(&format!("[{}] {w}{org}: {text}\n\n", clock(*t)));
        }
        pack.clear();
    };
    for t in &turns {
        if size > 0 && size + t.2.len() > PACK {
            flush(&mut pack, &mut md);
            size = 0;
        }
        size += t.2.len() + t.1.len() + 10;
        pack.push(t);
    }
    flush(&mut pack, &mut md);
    // The meeting chat.
    if let Some(chat) = calls::read_json(&dir.join("chat.json")).as_array().filter(|c| !c.is_empty()) {
        md.push_str("\n## Chat\n\n");
        for m in chat {
            let t = m["t"].as_f64().map(|t| format!("[{}] ", clock(t))).unwrap_or_default();
            md.push_str(&format!("{t}{}: {}\n\n", m["from"].as_str().unwrap_or("?"), m["text"].as_str().unwrap_or("").trim()));
        }
    }
    let threads: Vec<Value> = crate::projects::of_call(dir).unwrap_or_default().into_iter().map(|(p, _)| json!({"id": p.id, "name": p.name, "client": p.client})).collect();
    let client = threads.iter().find_map(|t| t["client"].as_str().map(str::to_owned));
    Some(Doc {
        source: format!("call/{stamp}"),
        title,
        event_date: date,
        md,
        refs: json!({"kind": "call", "call": name, "threads": threads, "client": client}),
        call: name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_reads_as_minutes_turns_and_chat() {
        let (_env, _root) = crate::paths::testenv::folders("brain-call");
        let d = crate::paths::calls_dir().join("2026-10-28_110000-aurora");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), json!({"title": "Aurora weekly", "started_at": "2026-10-28T11:00:00+02:00"}).to_string()).unwrap();
        std::fs::write(d.join("notes.md"), "# Aurora weekly\n\n## Summary\nSSO lands first.\n\n## Decisions\n- Pilot moves to 24 Nov [12:05].\n").unwrap();
        std::fs::write(d.join("segments.json"), json!([
            {"speaker": "remote", "name": "Ortiz, Karen (Partner)", "start": 725.0, "text": "Then SSO has to land first."},
            {"speaker": "remote", "name": "Ortiz, Karen (Partner)", "start": 728.0, "text": "Let's say the 24th."},
            {"speaker": "me", "start": 741.0, "text": "I can have the SAML config by Friday."},
        ]).to_string()).unwrap();
        std::fs::write(d.join("chat.json"), json!([{"t": 730.0, "from": "Ravi", "text": "link to the diagram"}]).to_string()).unwrap();
        let doc = document(&d).unwrap();
        assert_eq!(doc.source, format!("call/{}", calls::stamp_of(&d)));
        let c = super::super::chunk::chunk(&doc.md, 1200);
        let titles: Vec<&str> = c.iter().map(|x| x.title.as_str()).collect();
        assert_eq!(titles, ["Summary", "Decisions", "[12:05] Karen Ortiz, Me", "Chat"], "{}", doc.md);
        assert_eq!(c[2].breadcrumb, "Aurora weekly > Transcript > [12:05] Karen Ortiz, Me");
        assert!(c[2].body.contains("[12:05] Karen Ortiz: Then SSO has to land first. Let's say the 24th."), "one turn: {}", c[2].body);
        assert_eq!(title_time(&c[2].title), Some(725.0));
        assert!(doc.md.contains("<!-- event_date: 2026-10-28 -->"));
        assert!(document(&crate::paths::calls_dir().join("nothing")).is_none());
    }
}

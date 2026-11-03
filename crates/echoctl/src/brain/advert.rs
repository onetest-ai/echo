//! The Brain telling Claude it is there (#34): what this reader's Brain holds now — generated from the store and
//! the reader's access, no model — and when to go there first. Delivered where the model reads it: the MCP
//! server's instructions, `<echo_brain>` at the start of Echo's sessions, routines' and the call session's
//! prompts. Nothing at all when the Brain is off. Also: how often Claude asks it (`usage.json`, counts only).

use anyhow::Result;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use super::access::Access;
use super::store;

/// When to go to the Brain first, and how (fixed; the model sees it with what the Brain holds).
pub const WHEN: &str = "When to use it — before Microsoft 365, files or guessing:\n\
- What was said, decided, promised or asked — by whom, when — across calls, mail, Teams channels and documents: search_knowledge first; cite the source and its time. Go to Microsoft 365 or files only for what the Brain doesn't hold (not indexed yet, outside its sources, live state like today's calendar).\n\
- Who someone is, their role, what was said about them: find_people / get_person.\n\
- Figures from reporting workbooks: list_metrics / get_metric — never a number read from text.\n\
- Before a brief, a meeting prep, a follow-up or a summary of a client or thread: search it, scoped to that client or thread.\n\
- Scope every search by client or thread when the question is about one (ids from list_scopes); say \"not in the Brain\" when nothing supports an answer.\n\
Its tools are the echo-brain MCP server's (mcp__echo-brain__…); load them by name if they are deferred.";

/// A document's source, kind, date and refs.
type DocRow = (String, Option<String>, Option<String>, Option<String>);

fn kind_word(kind: &str, n: usize) -> String {
    let (one, many) = match kind {
        "call" => ("call", "calls"),
        "mail" => ("mail conversation", "mail conversations"),
        "channel" => ("Teams channel post", "Teams channel posts"),
        "file" | "document" => ("document", "documents"),
        other => (other, other),
    };
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What this reader's Brain holds, in a few lines (≤ ~900 characters): kinds with counts and dates, the
/// clients and threads it covers, whether people, numbers and insights are there, the last sync.
pub fn summary(c: &Connection, access: &Access) -> Result<String> {
    let ok = access.sources(c)?;
    let docs: Vec<DocRow> = c
        .prepare("SELECT source, kind, event_date, refs FROM documents")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;
    let mut kinds: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    let (mut clients, mut threads): (BTreeSet<String>, BTreeSet<String>) = (BTreeSet::new(), BTreeSet::new());
    for (src, kind, date, refs) in docs.into_iter().filter(|d| ok.contains(&d.0)) {
        let kind = kind.unwrap_or_else(|| src.split('/').next().unwrap_or("other").to_string());
        let e = kinds.entry(kind).or_insert((0, String::new(), String::new()));
        e.0 += 1;
        if let Some(d) = date.map(|d| d.chars().take(10).collect::<String>()).filter(|d| d.len() == 10) {
            if e.1.is_empty() || d < e.1 {
                e.1 = d.clone();
            }
            if d > e.2 {
                e.2 = d;
            }
        }
        let refs: Value = refs.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
        clients.extend(refs["client"].as_str().map(str::to_owned));
        for t in refs["threads"].as_array().into_iter().flatten() {
            threads.extend(t["id"].as_str().map(str::to_owned));
            clients.extend(t["client"].as_str().map(str::to_owned));
        }
    }
    if kinds.is_empty() {
        return Ok("It holds nothing you may read yet (not synced, or nothing on this AI profile).".into());
    }
    let mut out = vec![];
    let held: Vec<String> = kinds.iter().map(|(k, (n, from, to))| {
        let span = if from.is_empty() { String::new() } else if from == to { format!(" ({from})") } else { format!(" ({from} – {to})") };
        format!("{}{span}", kind_word(k, *n))
    }).collect();
    let sections: i64 = c.query_row("SELECT count(*) FROM chunks", [], |r| r.get(0)).unwrap_or(0);
    out.push(format!("Holds: {} — {sections} sections, searchable by words and meaning.", held.join(", ")));
    let names = |ids: &BTreeSet<String>, all: &[(String, String)]| -> String {
        let shown: Vec<String> = ids.iter().filter_map(|id| all.iter().find(|(i, _)| i == id).map(|(_, n)| format!("{n} ({id})"))).collect();
        let more = shown.len().saturating_sub(8);
        format!("{}{}", shown.into_iter().take(8).collect::<Vec<_>>().join(", "), if more > 0 { format!(" and {more} more") } else { String::new() })
    };
    let all_clients: Vec<(String, String)> = crate::projects::clients().unwrap_or_default().into_iter().map(|c| (c.id, c.name)).collect();
    let all_threads: Vec<(String, String)> = crate::projects::list().into_iter().map(|p| (p.id, p.name)).collect();
    if !clients.is_empty() {
        out.push(format!("Clients (id in brackets, for `client`): {}.", names(&clients, &all_clients)));
    }
    if !threads.is_empty() {
        out.push(format!("Threads (for `thread`): {}.", names(&threads, &all_threads)));
    }
    let mut also = vec![];
    if !access.people_threads().is_empty() {
        also.push("people (roles, facts, who works with whom)".to_string());
    }
    let numbers = access.numbers_clients();
    if !numbers.is_empty() {
        also.push(format!("reported figures for {}", numbers.join(", ")));
    }
    let insights = super::insights::count_for(&clients, &threads);
    if insights > 0 {
        also.push(format!("{insights} insights (what moved, slipped, recurs, contradicts)"));
    }
    if !also.is_empty() {
        out.push(format!("Also: {}.", also.join("; ")));
    }
    if let Some(at) = store::meta(c, "built_at") {
        out.push(format!("Last updated {}.", at.chars().take(16).collect::<String>().replace('T', " ")));
    }
    Ok(out.join("\n"))
}

/// The block for a reader, or None when the Brain is off (or can't be read).
pub fn block(access: &Access) -> Option<String> {
    if !super::enabled() {
        return None;
    }
    let c = store::open(&super::store_path()).ok()?;
    let s = summary(&c, access).ok()?;
    Some(format!("The user's Brain in Echo — their own work, indexed: calls, mail, Teams channels and folder documents as cited facts, read-only.\n{s}\n\n{WHEN}"))
}

/// The same as a `<echo_brain>` block for a session's context.
pub fn session_block(access: &Access) -> Option<String> {
    block(access).map(|b| format!("<echo_brain>\n{b}\n</echo_brain>"))
}

// ── how often Claude asks it (no content: counts by day, session kind and tool) ──

fn usage_file() -> std::path::PathBuf {
    super::store_path().with_file_name("usage.json")
}

/// Days kept.
const USAGE_DAYS: i64 = 30;

/// One more `what` (a tool, or a window view) from `who` (folder | call | routine | window) today.
pub fn count(who: &str, what: &str) {
    let mut v: Value = std::fs::read(usage_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let n = v[&today][who][what].as_u64().unwrap_or(0) + 1;
    v[&today][who][what] = json!(n);
    let cutoff = (chrono::Local::now() - chrono::Duration::days(USAGE_DAYS)).format("%Y-%m-%d").to_string();
    if let Some(m) = v.as_object_mut() {
        m.retain(|day, _| *day >= cutoff);
    }
    let _ = crate::paths::write_atomic(&usage_file(), &serde_json::to_vec(&v).unwrap_or_default());
}

/// The last `days` days added up: `{who: {what: n}}` and the total of Claude's calls.
pub fn usage(days: i64) -> Value {
    let v: Value = std::fs::read(usage_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    let cutoff = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut sum: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    for (day, by) in v.as_object().into_iter().flatten().filter(|(d, _)| **d > cutoff) {
        let _ = day;
        for (who, tools) in by.as_object().into_iter().flatten() {
            for (what, n) in tools.as_object().into_iter().flatten() {
                *sum.entry(who.clone()).or_default().entry(what.clone()).or_default() += n.as_u64().unwrap_or(0);
            }
        }
    }
    let claude: u64 = sum.iter().filter(|(w, _)| *w != "window").flat_map(|(_, t)| t.values()).sum();
    json!({"days": days, "by": sum, "claude": claude})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_when_the_brain_is_off_and_what_it_holds_when_on() {
        let (_env, _root) = crate::paths::testenv::folders("brain-advert");
        // A new data folder starts with the Brain off.
        assert!(block(&Access::default_profile()).is_none(), "off: not a word");
        let c = store::open(&crate::brain::store_path()).unwrap();
        c.execute("INSERT INTO documents(source, kind, title, event_date, refs) VALUES ('call/a', 'call', 'Weekly', '2026-10-01', '{\"client\": \"acme\"}'), ('mail/b', 'mail', 'RFP', '2026-10-20', '{}')", []).unwrap();
        let s = summary(&c, &Access::default_profile()).unwrap();
        // A call whose folder doesn't say its AI profile is nobody's (strict routing): not advertised either.
        assert!(s.contains("1 mail conversation (2026-10-20)") && !s.contains("call"), "{s}");
        count("folder", "search_knowledge");
        count("folder", "search_knowledge");
        count("window", "also");
        let u = usage(7);
        assert_eq!((u["by"]["folder"]["search_knowledge"].as_u64(), u["claude"].as_u64()), (Some(2), Some(2)), "{u}");
    }
}

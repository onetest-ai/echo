//! Recorded calls: minutes, templates, meeting types.

use anyhow::Result;

use crate::{calls, notes, paths, taskmodes, watch};

pub fn notes(call: String, wait: bool, auto: bool) -> Result<()> {
    if auto {
        notes::request_auto(&call)
    } else {
        notes::request(&call, wait)
    }
}

pub fn templates() -> Result<()> {
    let mut v = calls::templates_view();
    v["folder"] = serde_json::json!(paths::state_dir().join("templates"));
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

pub fn classify() -> Result<()> {
    let agenda = watch::agenda();
    let n = calls::classify(&agenda)?;
    let book = calls::type_book();
    let out: Vec<_> = agenda.iter().map(|m| serde_json::json!({"subject": m.subject, "start": m.start, "type": calls::event_type_in(m, &book)})).collect();
    // And who does each of the user's open tasks (Claude or by hand), as the app does in the background.
    let tasks = taskmodes::classify()?;
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"classified": n, "meetings": out, "task_modes": tasks}))?);
    Ok(())
}

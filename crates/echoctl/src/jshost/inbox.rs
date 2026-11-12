//! The plugins' Inbox items (`ctx.inbox.set` / `clear`): each plugin's `inbox.json` in its store (moved from
//! `jshost.rs` as it is).

use std::path::PathBuf;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::{paths, plugins};

fn inbox_file(id: &str) -> PathBuf {
    plugins::data_dir(id).join("inbox.json")
}

pub(super) fn inbox_set(id: &str, key: &str, item: Option<&Value>) -> Result<()> {
    if key.is_empty() || key.len() > 80 {
        bail!("an Inbox item needs a short key");
    }
    let f = inbox_file(id);
    let mut all: serde_json::Map<String, Value> = std::fs::read(&f).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    match item {
        Some(v) => {
            all.insert(key.into(), v.clone());
        }
        None => {
            all.remove(key);
        }
    }
    std::fs::create_dir_all(plugins::data_dir(id))?;
    paths::write_atomic(&f, serde_json::to_string_pretty(&all)?.as_bytes())?;
    Ok(())
}

/// Every enabled plugin's open Inbox items: `(plugin, key, item)`.
pub fn inbox_items() -> Vec<(String, String, Value)> {
    plugins::enabled()
        .into_iter()
        .flat_map(|p| {
            let all: serde_json::Map<String, Value> = std::fs::read(inbox_file(&p.id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
            all.into_iter().map(move |(k, v)| (p.id.clone(), k, v)).collect::<Vec<_>>()
        })
        .collect()
}

//! Profiling (Brain's `profile_workbooks.py`, no model) and the families' proposal (Claude, from the
//! profiles; the user accepts it). Per sheet: its size, the header row (the densest of the first
//! rows), its columns, two sample rows, month-like header cells; per file its month from the name.
//! Files of one report series (same folder, same name but for dates) are profiled once, the others
//! listed, so a proposal sees the series and its drift without reading every month.

use super::load::{self, Book, Cell};
use super::Config;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// A file name without its dates and numbers: what a monthly series shares.
fn series_key(rel: &str) -> String {
    let (dir, name) = rel.rsplit_once('/').unwrap_or(("", rel));
    const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];
    let is_month = |w: &str| MONTHS.iter().any(|m| *m == w || (w.len() == 3 && m.starts_with(w)) || (w == "sept"));
    let stem: String = name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && !w.chars().all(|c| c.is_ascii_digit()) && !is_month(w)).collect::<Vec<_>>().join(" ");
    format!("{dir}/{stem}")
}

fn sheet_profile(book: &mut Book, name: &str) -> Value {
    let rows = book.head(name, 60);
    let nonempty: Vec<(usize, &Vec<Cell>)> = rows.iter().enumerate().filter(|(_, r)| r.iter().any(|c| !c.is_empty())).collect();
    let header = nonempty.iter().take(12).max_by_key(|(i, r)| (r.iter().filter(|c| !c.is_empty()).count(), r.iter().filter(|c| matches!(c, Cell::Text(_))).count(), std::cmp::Reverse(*i))).map(|(i, _)| *i);
    let cells = |r: &Vec<Cell>| -> Vec<String> {
        let mut v: Vec<String> = r.iter().take(24).map(|c| c.text().chars().take(32).collect()).collect();
        while v.last().is_some_and(|s| s.is_empty()) {
            v.pop();
        }
        v
    };
    let first_rows: Vec<Value> = nonempty.iter().take(2).filter(|(i, _)| Some(*i) != header).map(|(i, r)| json!({"row": i, "cells": cells(r)})).collect();
    let samples: Vec<Value> = header.map(|h| nonempty.iter().filter(|(i, _)| *i > h).take(1).map(|(i, r)| json!({"row": i, "cells": cells(r)})).collect()).unwrap_or_default();
    let months: Vec<String> = header.and_then(|h| rows.get(h)).map(|r| r.iter().filter_map(|c| match c { Cell::Date(y, m, _) => Some(format!("{y:04}-{m:02}")), Cell::Text(t) => load::month_from_text(t, 2026).filter(|_| t.len() < 20), _ => None }).collect()).unwrap_or_default();
    // A matrix sheet's metric names run down a column: the first column's texts.
    let labels: Vec<String> = rows.iter().filter_map(|r| r.first()).filter_map(|c| if let Cell::Text(t) = c { Some(t.trim().chars().take(40).collect::<String>()) } else { None }).filter(|t| !t.is_empty()).take(40).collect();
    json!({"sheet": name, "rows": nonempty.len(), "header_row": header, "header": header.and_then(|h| rows.get(h)).map(cells), "first_rows": first_rows, "samples": samples, "month_headers": months, "first_column": labels})
}

/// Another file of a series, briefly: per sheet its header row and header (template drift shows).
fn brief(book: &mut Book) -> Value {
    let names = book.names.clone();
    json!(names.iter().take(12).map(|n| {
        let rows = book.head(n, 40);
        let h = rows.iter().enumerate().take(12).max_by_key(|(i, r)| (r.iter().filter(|c| !c.is_empty()).count(), std::cmp::Reverse(*i))).map(|(i, _)| i);
        json!({"sheet": n, "header_row": h, "header": h.and_then(|h| rows.get(h)).map(|r| r.iter().take(24).map(|c| c.text().chars().take(32).collect::<String>()).collect::<Vec<_>>())})
    }).collect::<Vec<_>>())
}

/// Profile a client's workbooks (its threads' folders, or `roots`): `<numbers>/<client>/profile.json`.
pub fn run(client: &str, roots: Option<Vec<(std::path::PathBuf, Vec<String>)>>) -> Result<Value> {
    let roots = roots.unwrap_or_else(|| super::roots(client));
    anyhow::ensure!(!roots.is_empty(), "no folders: add one to a thread of this client (Profile → Sources → Folder)");
    let books = super::workbooks(&roots);
    let mut series: BTreeMap<String, Vec<(std::path::PathBuf, String)>> = BTreeMap::new();
    for (root, rel) in &books {
        series.entry(series_key(rel)).or_default().push((root.clone(), rel.clone()));
    }
    let mut out = vec![];
    for (_, files) in series {
        let (root, rel) = &files[0];
        let profile = match Book::open(&root.join(rel)) {
            Ok(mut b) => {
                let names = b.names.clone();
                let mut v: Vec<Value> = names.iter().take(12).map(|n| sheet_profile(&mut b, n)).collect();
                if names.len() > 12 {
                    v.push(json!({"more_sheets": names[12..].iter().take(40).collect::<Vec<_>>()}));
                }
                json!(v)
            }
            Err(e) => json!({"error": format!("{e:#}")}),
        };
        // The series' other files, briefly: where the template drifts.
        let others: Vec<Value> = files[1..].iter().take(8).map(|(root, rel)| match Book::open(&root.join(rel)) {
            Ok(mut b) => json!({"file": rel.rsplit('/').next().unwrap_or(rel), "sheets": brief(&mut b)}),
            Err(e) => json!({"file": rel, "error": format!("{e:#}")}),
        }).collect();
        out.push(json!({"file": rel, "month_from_name": load::month_from_text(rel.rsplit('/').next().unwrap_or(rel), 2026), "series": files.iter().map(|(_, r)| r.rsplit('/').next().unwrap_or(r).to_string()).collect::<Vec<_>>(), "sheets": profile, "other_files": others}));
    }
    let v = json!({"summary": {"workbooks": books.len(), "series": out.len(), "at": chrono::Utc::now().to_rfc3339()}, "series": out});
    std::fs::create_dir_all(super::dir(client))?;
    crate::paths::write_atomic(&super::dir(client).join("profile.json"), serde_json::to_string_pretty(&v)?.as_bytes())?;
    Ok(v)
}

const SYSTEM: &str = "You design the numbers lane of a knowledge base: a config that loads a client's reporting workbooks into governed facts \
(family, metric, grain, entity, month, value), so numeric questions are answered from real cells, never from a model. You see profiles of the \
workbooks (sheets, header rows, columns, sample rows, month-like headers), one per report series with the series' other files listed. \
Write ONLY JSON: {\"families\": [...], \"conform_titlecase\": [...], \"dimension_map\": {...}, \"derived\": [...], \"rollups\": [...], \"metrics\": {...}, \"skipped\": [{\"series\": file, \"why\": ...}]}.\n\
A family: {\"name\": snake_case, \"glob\": folder-relative glob matching the whole series (use * for dates; a list when files moved folders), \
\"month_from\": \"filename\" | \"wide_banner\" | \"matrix_header\", \"layout\": one of \
\"long\" (a sheet per grain: dimension rows, measures across; grains: {grain: {\"sheet\": name or [alternatives], \"dim_header\": header text}}), \
\"tolerant_long\" (template-less exports: \"dim_candidates\": {grain: [header texts]}; the header is found by content on any sheet), \
\"matrix_month_cols\" (metric rows × month columns: \"sheet\", \"month_header_row\" (0-based), \"metric_col\" (0-based), grains: {\"overall\": {\"entity\": \"OVERALL\"}}; measures match the row label exactly), \
\"wide_month\" (a banner row of months over repeated measure blocks: \"banner_row\", \"header_row\" (0-based), grains as long); \
\"measures\": {canonical_snake_name: header text, or [alternatives] when headers drift across the series}, optional \"header_row\" (0-based hint), \
\"collision\": {\"policy\": last|first|max|min|sum|mean|weighted_mean, \"weight\": a count measure of the family, \"sum\": [count measures]} when one file lists an entity on several rows.\n\
Model only real reporting tables (KPIs by branch/region/division/month); skip calendars, tables of contents, workshop notes, one-off lists — name them in skipped. \
Header texts must be copied from the profiles exactly (they are matched case-insensitively). Rows are 0-based as in the profiles.\n\
\"metrics\": {metric name: {\"family\", \"metric\", \"unit\": count|pct|ratio|seconds|currency|score, \"desc\": a few words, \"definition\": what it measures and whether it is REPORTED (a vendor cell) or COMPUTED}} — one per measure, plus derived ones. \
\"derived\": [{\"family\", \"metric\", \"grain\", \"numerator\", \"denominator\"}] only for a ratio people ask for that the files don't carry. Never reconstruct a reported composite (AHT, occupancy, service level).";

/// A model's answer as a config: each family on its own (one that doesn't fit the format or can't
/// be built is dropped with the reason), the rest of Brain's config kept when well formed.
fn parse_config(v: &Value) -> (Config, Vec<Value>) {
    let mut dropped: Vec<Value> = vec![];
    let mut cfg = Config::default();
    for f in v["families"].as_array().into_iter().flatten() {
        let name = f["name"].as_str().unwrap_or("?").to_string();
        match serde_json::from_value::<super::Family>(f.clone()) {
            Ok(fam) => match (Config { families: vec![fam.clone()], ..Default::default() }).check() {
                Ok(()) if !fam.measures.is_empty() => cfg.families.push(fam),
                Ok(()) => dropped.push(json!({"family": name, "why": "no measures"})),
                Err(e) => dropped.push(json!({"family": name, "why": format!("{e:#}")})),
            },
            Err(e) => dropped.push(json!({"family": name, "why": format!("not in the families format: {e}")})),
        }
    }
    cfg.conform_titlecase = v["conform_titlecase"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect();
    cfg.dimension_map = v["dimension_map"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(dim, m)| m.as_object().map(|m| (dim.clone(), m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.to_lowercase(), v.to_string()))).collect())))
        .collect();
    cfg.derived = v["derived"].as_array().cloned().unwrap_or_default();
    cfg.rollups = v["rollups"].as_array().cloned().unwrap_or_default();
    (cfg, dropped)
}

fn ask(user: &str) -> Result<Value> {
    let ready = crate::ai::ready(&crate::ai::default_id())?;
    let answer = crate::livenotes::complete_text(ready.profile(), SYSTEM, user, 16000)?;
    let s = answer.find('{').ok_or_else(|| anyhow!("no JSON in the answer"))?;
    let e = answer.rfind('}').ok_or_else(|| anyhow!("no JSON in the answer"))?;
    serde_json::from_str(&answer[s..=e]).map_err(|err| anyhow!("the answer's JSON doesn't parse: {err}"))
}

/// How well a config loads: fully loaded units, then facts.
fn score(audit: &Value) -> (usize, usize) {
    (audit["summary"]["ok"].as_u64().unwrap_or(0) as usize, audit["summary"]["facts"].as_u64().unwrap_or(0) as usize)
}

/// The units that didn't load, each with its reason and that file's actual sheets and headers.
fn failures(audit: &Value, roots: &[(std::path::PathBuf, Vec<String>)]) -> Vec<Value> {
    let mut out = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for x in audit["audit"].as_array().into_iter().flatten().filter(|x| x["status"] != "ok") {
        let rel = x["file"].as_str().unwrap_or("");
        let sheets = if seen.insert(rel.to_string()) {
            roots.iter().map(|(r, _)| r.join(rel)).find(|p| p.is_file()).and_then(|p| Book::open(&p).ok()).map(|mut b| brief(&mut b)).unwrap_or(Value::Null)
        } else {
            json!("(as above)")
        };
        out.push(json!({"family": x["family"], "file": rel.rsplit('/').next().unwrap_or(rel), "unit": x["unit"], "status": x["status"], "reason": x["reason"], "sheets": sheets}));
        if out.len() >= 40 {
            break;
        }
    }
    out
}

/// Claude proposes the families and the catalog from the profiles; saved as `proposed` (nothing is
/// built until the user accepts).
pub fn propose(client: &str, roots: Option<Vec<(std::path::PathBuf, Vec<String>)>>) -> Result<Value> {
    // Strict routing: the workbooks' cells go to the default profile's model only when every thread of
    // this client runs on it.
    let default = crate::ai::default_id();
    anyhow::ensure!(
        crate::brain::access::client_profile_ok(client, &default),
        "{client}'s threads run on another AI profile than the default ({default}): its workbooks aren't sent to the default profile's model"
    );
    let roots = roots.unwrap_or_else(|| super::roots(client));
    let prof = run(client, Some(roots.clone()))?;
    let mut text = serde_json::to_string(&prof["series"])?;
    if text.len() > 200_000 {
        text = text.chars().take(200_000).collect();
    }
    let first = ask(&format!("Client: {client}. Workbook profiles (one per series, with the series' other files' sheets and headers — templates drift):\n{text}\n\nPropose the config."))?;
    std::fs::create_dir_all(super::dir(client))?;
    crate::paths::write_atomic(&super::dir(client).join("proposal-raw.json"), serde_json::to_string_pretty(&first)?.as_bytes())?;
    let (mut cfg, mut dropped) = parse_config(&first);
    let mut answer = first;
    let mut audit = super::build::dry(client, &cfg, Some(roots.clone()))?;
    let mut rounds = vec![audit["summary"].clone()];
    // Repair rounds (Brain's audit as the feedback): what didn't load, with those files' real headers.
    for _ in 0..2 {
        let fails = failures(&audit, &roots);
        if fails.is_empty() {
            break;
        }
        let user = format!(
            "Client: {client}. This config was built against the workbooks; these units didn't load fully (status, reason, and that file's actual sheets and header rows):\n{}\n\nThe config:\n{}\n\n\
             Revise it so these load: add the sheet names and header texts each file really uses as alternatives (lists), fix header rows, \
             or switch a family whose files have no stable template to tolerant_long. Keep what loads. Answer the whole config (with metrics and skipped) as before.",
            serde_json::to_string(&fails)?,
            serde_json::to_string(&answer)?
        );
        let Ok(next) = ask(&user) else { break };
        let (c2, d2) = parse_config(&next);
        let a2 = super::build::dry(client, &c2, Some(roots.clone()))?;
        rounds.push(a2["summary"].clone());
        if score(&a2) <= score(&audit) {
            break;
        }
        (cfg, dropped, audit, answer) = (c2, d2, a2, next);
    }
    cfg._echo = Some(json!({"status": "proposed", "by": "claude", "at": chrono::Utc::now().to_rfc3339(), "skipped": answer["skipped"], "dropped": dropped, "rounds": rounds, "kept": audit["summary"]}));
    super::save_config(client, &cfg)?;
    let metrics: BTreeMap<String, Value> = serde_json::from_value(answer["metrics"].clone()).unwrap_or_default();
    super::save_catalog(client, &metrics)?;
    Ok(json!({"families": cfg.families.iter().map(|f| json!({"name": f.name, "layout": f.layout, "measures": f.measures.len(), "grains": f.grains.keys().collect::<Vec<_>>()})).collect::<Vec<_>>(),
              "metrics": metrics.len(), "rounds": rounds, "skipped": answer["skipped"], "dropped": dropped}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monthly_series_share_a_key() {
        let a = series_key("Ops/Volume/01 - January 2026 - Volume by Region 02.07.26.xlsx");
        let b = series_key("Ops/Volume/06 - June Volume by Region 7.11.26.xlsx");
        assert_eq!(a, b);
        assert_ne!(series_key("x/Decision log.xlsx"), series_key("x/log.xlsx"));
    }
}

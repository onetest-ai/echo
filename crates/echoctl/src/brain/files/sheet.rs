//! Spreadsheets (Brain's `parse_xlsx_structure`): per sheet, the densest of the first ten non-empty
//! rows is its header; then one section per data row, `Column: value`. A narrative map of the book —
//! numbers people ask about belong to a numbers lane (later), not to these sections. A reporting book
//! (a sheet with more than `REPORTING_ROWS` data rows: call volumes, daily figures) keeps its schema
//! and first rows only, so it can be found but doesn't flood the Brain.

use super::Parsed;
use anyhow::{anyhow, Result};
use calamine::{open_workbook_auto, Reader};
use std::path::Path;

/// A book with a sheet this long is a reporting book: every sheet sampled.
pub const REPORTING_ROWS: usize = 200;
/// A workbook bigger than this is only sampled (its first rows per sheet).
pub const MAX_FULL_BYTES: u64 = 20_000_000;
const SAMPLE_ROWS: usize = 8;

/// An xlsx/xlsm is a zip: refuse one that unpacks to more than 1 GB in all or 512 MB in a part (a zip
/// bomb, or a reporting dump far past what a section map could use) before calamine inflates it.
fn unpacked_size_ok(path: &Path) -> Result<()> {
    if !matches!(super::ext(path).as_str(), "xlsx" | "xlsm") {
        return Ok(());
    }
    let Ok(mut z) = zip::ZipArchive::new(std::fs::File::open(path)?) else { return Ok(()) };
    let (mut total, mut biggest) = (0u64, 0u64);
    for i in 0..z.len() {
        if let Ok(f) = z.by_index_raw(i) {
            total = total.saturating_add(f.size());
            biggest = biggest.max(f.size());
        }
    }
    anyhow::ensure!(total <= 1 << 30 && biggest <= 512 << 20, "the workbook unpacks to {} MB: not read", total >> 20);
    Ok(())
}

pub fn read(path: &Path) -> Result<Parsed> {
    let size = std::fs::metadata(path)?.len();
    unpacked_size_ok(path)?;
    let mut wb = open_workbook_auto(path).map_err(|e| anyhow!("not a readable workbook: {e}"))?;
    // Each sheet read once.
    let sheets: Vec<(String, calamine::Range<calamine::Data>)> = wb.sheet_names().to_vec().into_iter().filter_map(|n| wb.worksheet_range(&n).ok().map(|r| (n, r))).collect();
    let longest = sheets.iter().map(|(_, r)| r.height()).max().unwrap_or(0);
    let reporting = size > MAX_FULL_BYTES || longest > REPORTING_ROWS;
    let limit = if reporting { SAMPLE_ROWS } else { REPORTING_ROWS };
    let mut md = String::new();
    let mut cut = false;
    for (name, range) in sheets {
        let dims = format!("{}×{}", range.height(), range.width());
        let rows: Vec<(usize, Vec<String>)> = range
            .rows()
            .take(limit + 20)
            .enumerate()
            .map(|(i, r)| (i + 1, r.iter().map(|c| c.to_string().split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<String>>()))
            .filter(|(_, r)| r.iter().any(|c| !c.is_empty()))
            .collect();
        if rows.is_empty() {
            continue;
        }
        let numeric = |c: &str| c.replace(['.', ',', '-', '%'], "").chars().all(|ch| ch.is_ascii_digit());
        let (header_no, header) = rows
            .iter()
            .take(10)
            .max_by_key(|(n, r)| (r.iter().filter(|c| !c.is_empty()).count(), r.iter().filter(|c| !c.is_empty() && !numeric(c)).count(), std::cmp::Reverse(*n)))
            .cloned()
            .unwrap_or_default();
        let labels: Vec<String> = header.iter().enumerate().map(|(i, h)| if h.is_empty() { format!("Column {}", i + 1) } else { h.clone() }).collect();
        md.push_str(&format!("\n## sheet: {name} · schema ({dims})\n\nColumns: {}\n", labels.iter().filter(|l| !l.starts_with("Column ")).cloned().collect::<Vec<_>>().join("; ")));
        let data: Vec<&(usize, Vec<String>)> = rows.iter().filter(|(n, _)| *n > header_no).collect();
        for (n, cells) in data.iter().take(limit) {
            let fields: Vec<String> = cells
                .iter()
                .enumerate()
                .filter(|(_, v)| !v.is_empty())
                .map(|(i, v)| format!("- {}: {v}", labels.get(i).cloned().unwrap_or_else(|| format!("Column {}", i + 1))))
                .collect();
            if !fields.is_empty() {
                md.push_str(&format!("\n## sheet: {name} · row {n}\n\n{}\n", fields.join("\n")));
            }
        }
        // Rows were read only up to the limit (plus room for a header); the sheet's height says how many are left.
        let shown = data.len().min(limit);
        let left = range.height().saturating_sub(header_no).saturating_sub(shown);
        if left > 0 {
            cut = true;
            md.push_str(&format!("\n_{left} more rows in {name} — a reporting sheet: its figures are for the numbers lane, not read here._\n"));
        }
    }
    Ok(Parsed { method: "spreadsheet", md: Some(md.trim().to_string()), fidelity: if cut { "partial" } else { "full" }, ..Default::default() })
}

//! Brain's `build_marts.py` loaders, ported: cells read as openpyxl gives them (positions from A1,
//! dates as dates, time cells as seconds), then the four layouts — `long` (a grain sheet, dimension
//! rows, measures across), `tolerant_long` (template-less exports: header found by content),
//! `matrix_month_cols` (metric rows × month columns), `wide_month` (month-banner column blocks).

use super::{Family, Grain, Spec};
use anyhow::{anyhow, Result};
use calamine::{open_workbook_auto, Data, Reader};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Empty,
    Num(f64),
    Text(String),
    /// A date (year, month, day).
    Date(i32, u32, u32),
    /// A time of day or a duration, in seconds (ASA, AHT).
    Time(f64),
}

impl Cell {
    pub fn is_empty(&self) -> bool {
        matches!(self, Cell::Empty) || matches!(self, Cell::Text(s) if s.is_empty())
    }

    /// The cell as text (openpyxl's `str(v)`).
    pub fn text(&self) -> String {
        match self {
            Cell::Empty => String::new(),
            Cell::Num(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            Cell::Num(n) => format!("{n}"),
            Cell::Text(s) => s.clone(),
            Cell::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02} 00:00:00"),
            Cell::Time(s) => format!("{:02}:{:02}:{:02}", (*s as u64) / 3600, (*s as u64) / 60 % 60, (*s as u64) % 60),
        }
    }

    /// Brain's `to_float`: numbers, times as seconds, text "1,234" / "$5" / "12%".
    pub fn number(&self) -> Option<f64> {
        match self {
            Cell::Num(n) => Some(*n),
            Cell::Time(s) => Some(*s),
            Cell::Text(s) => s.replace([',', '$', '%'], "").trim().parse::<f64>().ok(),
            _ => None,
        }
    }
}

fn excel_date(serial: f64) -> Option<(i32, u32, u32)> {
    use chrono::Datelike;
    let base = chrono::NaiveDate::from_ymd_opt(1899, 12, 30)?;
    let d = base.checked_add_signed(chrono::Duration::days(serial.floor() as i64))?;
    Some((d.year(), d.month(), d.day()))
}

fn cell(d: &Data) -> Cell {
    match d {
        Data::Empty => Cell::Empty,
        Data::Int(i) => Cell::Num(*i as f64),
        Data::Float(f) => Cell::Num(*f),
        Data::String(s) => Cell::Text(s.clone()),
        Data::Bool(b) => Cell::Text(if *b { "True".into() } else { "False".into() }),
        Data::DateTime(t) => {
            let v = t.as_f64();
            if t.is_duration() || (0.0..1.0).contains(&v) {
                Cell::Time((v * 86400.0 * 1000.0).round() / 1000.0)
            } else {
                excel_date(v).map(|(y, m, dd)| Cell::Date(y, m, dd)).unwrap_or(Cell::Num(v))
            }
        }
        Data::DateTimeIso(s) | Data::DurationIso(s) => Cell::Text(s.clone()),
        Data::Error(_) => Cell::Text("#ERR".into()),
    }
}

pub type Rows = Vec<Vec<Cell>>;

/// A workbook, its sheets read once each, on demand.
pub struct Book {
    pub names: Vec<String>,
    sheets: BTreeMap<String, Rows>,
    wb: calamine::Sheets<std::io::BufReader<std::fs::File>>,
}

impl Book {
    pub fn open(p: &Path) -> Result<Book> {
        let wb = open_workbook_auto(p).map_err(|e| anyhow!("open failed: {e}"))?;
        Ok(Book { names: wb.sheet_names().to_vec(), sheets: BTreeMap::new(), wb })
    }

    /// A sheet's rows, from A1 (cells before the used range are empty, as openpyxl reads them).
    pub fn rows(&mut self, name: &str) -> Rows {
        if let Some(r) = self.sheets.get(name) {
            return r.clone();
        }
        let rows: Rows = match self.wb.worksheet_range(name) {
            Ok(range) => {
                let (r0, c0) = range.start().unwrap_or((0, 0));
                let mut out: Rows = (0..r0).map(|_| vec![]).collect();
                for r in range.rows().take(100_000) {
                    let mut v = vec![Cell::Empty; c0 as usize];
                    v.extend(r.iter().map(cell));
                    out.push(v);
                }
                out
            }
            Err(_) => vec![],
        };
        self.sheets.insert(name.to_string(), rows.clone());
        rows
    }

    /// A sheet's first `n` rows (profiling).
    pub fn head(&mut self, name: &str, n: usize) -> Rows {
        if let Some(r) = self.sheets.get(name) {
            return r.iter().take(n).cloned().collect();
        }
        match self.wb.worksheet_range(name) {
            Ok(range) => {
                let (r0, c0) = range.start().unwrap_or((0, 0));
                let mut out: Rows = (0..r0.min(n as u32)).map(|_| vec![]).collect();
                for r in range.rows().take(n.saturating_sub(out.len())) {
                    let mut v = vec![Cell::Empty; c0 as usize];
                    v.extend(r.iter().take(40).map(cell));
                    out.push(v);
                }
                out
            }
            Err(_) => vec![],
        }
    }

    /// Brain's `get_sheet`: an exact name wins over any alternative's contains-match.
    pub fn sheet(&self, spec: &Spec) -> Option<String> {
        let names = spec.alts();
        for n in &names {
            if self.names.iter().any(|s| s == n) {
                return Some(n.clone());
            }
        }
        for n in &names {
            if let Some(s) = self.names.iter().find(|s| norm(s).contains(&norm(n))) {
                return Some(s.clone());
            }
        }
        None
    }
}

pub fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn ncell(c: &Cell) -> String {
    norm(&c.text())
}

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

/// Brain's `month_from_filename`: the first month name by position (`June … May` → June), else a
/// three-letter abbreviation at a word start; the year from "202x", else `default_year`.
pub fn month_from_text(s: &str, default_year: i32) -> Option<String> {
    let low = s.to_lowercase();
    let mut best: Option<(usize, usize)> = None;
    for (i, m) in MONTHS.iter().enumerate() {
        if let Some(p) = low.find(m) {
            if best.is_none_or(|b| p < b.0) {
                best = Some((p, i + 1));
            }
        }
    }
    if best.is_none() {
        for (i, m) in MONTHS.iter().enumerate() {
            let ab = &m[..3];
            let mut from = 0;
            while let Some(off) = low[from..].find(ab) {
                let p = from + off;
                let word_start = p == 0 || !low[..p].chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_');
                if word_start && best.is_none_or(|b| p < b.0) {
                    best = Some((p, i + 1));
                    break;
                }
                from = p + ab.len();
            }
        }
    }
    let (_, m) = best?;
    let year = low.find("202").and_then(|p| low.get(p..p + 4)).and_then(|y| y.parse::<i32>().ok()).filter(|y| (2020..=2029).contains(y)).unwrap_or(default_year);
    Some(format!("{year}-{m:02}"))
}

/// Brain's `is_dim_value`: not empty, not a total or an error.
pub fn is_dim_value(c: &Cell) -> bool {
    if c.is_empty() {
        return false;
    }
    let s = c.text().trim().to_lowercase();
    !["total", "grand total", "all", "sum", "#n/a", "n/a", "#ref!", "#value!", "#div/0!", "(blank)", "blank", "#err"].contains(&s.as_str())
}

fn header_matches(cell_norm: &str, spec: &Spec) -> bool {
    spec.alts().iter().any(|a| cell_norm.contains(&norm(a)))
}

fn header_exact(cell_norm: &str, spec: &Spec) -> bool {
    spec.alts().iter().any(|a| cell_norm == norm(a))
}

/// Brain's `col_index`: an exact match per alternative first, then a substring.
pub fn col_index(header: &[Cell], spec: &Spec) -> Option<usize> {
    for a in spec.alts() {
        let na = norm(&a);
        if let Some(j) = header.iter().position(|c| ncell(c) == na) {
            return Some(j);
        }
    }
    for a in spec.alts() {
        let na = norm(&a);
        if let Some(j) = header.iter().position(|c| ncell(c).contains(&na)) {
            return Some(j);
        }
    }
    None
}

/// The row with the dimension header and at least one measure header (the hint first).
fn find_header(rows: &Rows, dim: &str, measures: &BTreeMap<String, Spec>, hint: Option<usize>) -> Option<usize> {
    let mut cand: Vec<usize> = hint.into_iter().collect();
    cand.extend(0..rows.len());
    for i in cand {
        let Some(r) = rows.get(i) else { continue };
        let cells: Vec<String> = r.iter().map(ncell).collect();
        if cells.contains(&norm(dim)) && measures.values().any(|ms| cells.iter().any(|c| header_matches(c, ms))) {
            return Some(i);
        }
    }
    hint
}

/// Conform a dimension value: an explicit alias, else title case for the dims listed, else as is.
pub struct Conform<'a> {
    pub map: &'a BTreeMap<String, BTreeMap<String, String>>,
    pub titlecase: &'a BTreeSet<String>,
}

impl Conform<'_> {
    pub fn apply(&self, dim: &str, v: &str) -> String {
        let v = v.split_whitespace().collect::<Vec<_>>().join(" ");
        if let Some(m) = self.map.get(dim).and_then(|m| m.get(&norm(&v))) {
            return m.clone();
        }
        if self.titlecase.contains(dim) {
            return smart_title(&v);
        }
        v
    }
}

/// Title-case each alphanumeric run; short all-caps acronyms stay.
fn smart_title(s: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut String| {
        if !w.is_empty() {
            let keep = w.chars().all(|c| !c.is_lowercase()) && w.chars().any(char::is_alphabetic) && w.chars().count() <= 3;
            if keep {
                out.push_str(w);
            } else {
                let mut cs = w.chars();
                if let Some(f) = cs.next() {
                    out.extend(f.to_uppercase());
                    out.push_str(&cs.as_str().to_lowercase());
                }
            }
            w.clear();
        }
    };
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// A fact as loaded: metric, grain, entity, month, value, and the source row (pairs a value with
/// its weight for collisions).
#[derive(Debug, Clone)]
pub struct Raw {
    pub metric: String,
    pub grain: String,
    pub entity: String,
    pub month: Option<String>,
    pub value: f64,
    pub row: u64,
}

pub struct Diag {
    pub reason: Option<String>,
    /// The table's header row (where notes beside it are looked for).
    pub header: Option<usize>,
}

impl Diag {
    fn because(reason: impl Into<String>) -> Diag {
        Diag { reason: Some(reason.into()), header: None }
    }
}

fn is_sentence(s: &str) -> bool {
    let s = s.trim();
    s.chars().count() >= 40 && s.char_indices().any(|(i, c)| ".;!?".contains(c) && s[i + c.len_utf8()..].chars().next().is_none_or(char::is_whitespace))
}

fn cell_ref(i: usize, j: usize) -> String {
    let (mut col, mut n) = (String::new(), j + 1);
    while n > 0 {
        let r = (n - 1) % 26;
        col.insert(0, (b'A' + r as u8) as char);
        n = (n - 1) / 26;
    }
    format!("{col}{}", i + 1)
}

/// Brain's `scan_notes`: text written BESIDE a loaded table — right of its header run, past a blank
/// column, from the header row down — that reads as a sentence (≥ 40 characters, sentence
/// punctuation) and stands alone (nothing next to it, nothing below). Titles above a table, side
/// tables' labels and block headings aren't notes. Returns (cell, text).
pub fn scan_notes(rows: &Rows, header: Option<usize>) -> Vec<(String, String)> {
    let Some(hi) = header.filter(|h| *h < rows.len()) else { return vec![] };
    let h = &rows[hi];
    let first = h.iter().position(|c| !c.is_empty()).unwrap_or(0);
    let mut end = first;
    while end + 1 < h.len() && !h[end + 1].is_empty() {
        end += 1;
    }
    let mut out = vec![];
    for (i, r) in rows.iter().enumerate().skip(hi) {
        for j in (end + 2)..r.len() {
            let Cell::Text(t) = &r[j] else { continue };
            if !is_sentence(t) {
                continue;
            }
            let beside = r[end + 1..j].iter().any(|x| !x.is_empty()) || r.iter().skip(j + 1).take(3).any(|x| !x.is_empty());
            let below = rows.get(i + 1).and_then(|n| n.get(j)).is_some_and(|x| !x.is_empty());
            if !beside && !below {
                out.push((cell_ref(i, j), t.split_whitespace().collect::<Vec<_>>().join(" ")));
            }
        }
    }
    out
}

/// Rows counter: unique per source row across a build.
pub struct RowIds(pub u64);
impl RowIds {
    fn next(&mut self) -> u64 {
        self.0 += 1;
        self.0
    }
}

/// Brain's `entity_resolver`: an `entity_map` (looked up only for names that collide in the
/// sheet, unless `when: always`) and `entity_with` (other columns appended).
pub struct Entities {
    table: BTreeMap<String, String>,
    always: bool,
    ambiguous: BTreeSet<String>,
    with: Vec<String>,
    on: bool,
    pub mapped: Vec<(String, String)>,
}

impl Entities {
    pub fn new(book: &mut Book, g: &Grain) -> Entities {
        let mut e = Entities { table: BTreeMap::new(), always: false, ambiguous: BTreeSet::new(), with: g.entity_with.clone(), on: g.entity_map.is_some() || !g.entity_with.is_empty(), mapped: vec![] };
        if let Some(em) = &g.entity_map {
            e.always = em.when.as_deref() == Some("always");
            if let Some(name) = book.sheet(&Spec::One(em.sheet.clone())) {
                let rows = book.rows(&name);
                let (from, to) = (Spec::One(em.from.clone()), Spec::One(em.to.clone()));
                if let Some(hi) = rows.iter().take(20).position(|r| col_index(r, &from).is_some() && col_index(r, &to).is_some()) {
                    let (fc, tc) = (col_index(&rows[hi], &from).unwrap_or(0), col_index(&rows[hi], &to).unwrap_or(0));
                    for r in &rows[hi + 1..] {
                        if let (Some(a), Some(b)) = (r.get(fc), r.get(tc)) {
                            if is_dim_value(a) && is_dim_value(b) {
                                e.table.entry(a.text()).or_insert_with(|| b.text().trim().to_string());
                                e.table.entry(a.text().trim().to_string()).or_insert_with(|| b.text().trim().to_string());
                            }
                        }
                    }
                }
            }
        }
        e
    }

    fn scan(&mut self, values: &[&Cell]) {
        let mut folded: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for v in values.iter().filter(|v| is_dim_value(v)) {
            folded.entry(ncell(v)).or_default().insert(v.text());
        }
        self.ambiguous = folded.into_iter().filter(|(_, r)| r.len() > 1).map(|(k, _)| k).collect();
    }

    fn resolve(&mut self, raw: &Cell, row: &[Cell], header: &[Cell]) -> String {
        let mut out = raw.text();
        if !self.on {
            return out;
        }
        if !self.table.is_empty() && (self.always || self.ambiguous.contains(&ncell(raw))) {
            if let Some(t) = self.table.get(&out).or_else(|| self.table.get(out.trim())) {
                self.mapped.push((out.trim().to_string(), t.clone()));
                out = t.clone();
            }
        }
        if !self.with.is_empty() {
            let extra: Vec<String> = self.with.iter().filter_map(|h| col_index(header, &Spec::One(h.clone()))).filter_map(|j| row.get(j)).filter(|c| is_dim_value(c)).map(|c| c.text().trim().to_string()).collect();
            out = std::iter::once(out.trim().to_string()).chain(extra).collect::<Vec<_>>().join(" ");
        }
        out
    }
}

/// `long`: a grain sheet, dimension rows, measures across; the table ends at its first blank row.
#[allow(clippy::too_many_arguments)]
pub fn long(rows: &Rows, fam: &Family, grain: &str, dim_header: &str, month: Option<&str>, conform: &Conform, ents: &mut Entities, ids: &mut RowIds) -> (Vec<Raw>, Diag) {
    let Some(hi) = find_header(rows, dim_header, &fam.measures, fam.header_row) else {
        return (vec![], Diag::because("no header row containing dim + a measure"));
    };
    let Some(header) = rows.get(hi) else { return (vec![], Diag::because(format!("no row {hi}"))) };
    let Some(dcol) = col_index(header, &Spec::One(dim_header.into())) else {
        return (vec![], Diag::because(format!("dim column '{dim_header}' not found in header")));
    };
    let mcols: BTreeMap<&String, Option<usize>> = fam.measures.iter().map(|(m, s)| (m, col_index(header, s))).collect();
    let missing: Vec<&String> = mcols.iter().filter(|(_, j)| j.is_none()).map(|(m, _)| *m).collect();
    if missing.len() == mcols.len() {
        return (vec![], Diag::because(format!("no measure columns matched: {missing:?}")));
    }
    let body = &rows[hi + 1..];
    ents.scan(&body.iter().filter_map(|r| r.get(dcol)).collect::<Vec<_>>());
    let mut facts = vec![];
    let mut started = false;
    for r in body {
        if r.iter().all(Cell::is_empty) {
            if started {
                break;
            }
            continue;
        }
        let Some(d) = r.get(dcol).filter(|d| is_dim_value(d)) else { continue };
        started = true;
        let ent = conform.apply(grain, &ents.resolve(d, r, header));
        let rid = ids.next();
        for (m, j) in &mcols {
            if let Some(v) = j.and_then(|j| r.get(j)).and_then(Cell::number) {
                facts.push(Raw { metric: (*m).clone(), grain: grain.into(), entity: ent.clone(), month: month.map(str::to_owned), value: v, row: rid });
            }
        }
    }
    let reason = (!missing.is_empty()).then(|| format!("partial: measures not matched: {missing:?}"));
    (facts, Diag { reason, header: Some(hi) })
}

/// `tolerant_long`: the header found by content (a dimension candidate + a measure) within 40 rows;
/// of several dimension columns, the one with the most distinct values keys the rows.
pub fn tolerant(rows: &Rows, fam: &Family, month: Option<&str>, conform: &Conform, ids: &mut RowIds) -> (Vec<Raw>, Diag) {
    let re = fam.entity_regex.as_deref().and_then(|r| regex::Regex::new(r).ok());
    let mut found: Option<(usize, String, usize)> = None;
    for (i, row) in rows.iter().take(40).enumerate() {
        let cells: Vec<String> = row.iter().map(ncell).collect();
        if !fam.measures.values().any(|ms| cells.iter().any(|c| header_matches(c, ms))) {
            continue;
        }
        let present: Vec<(String, usize)> = fam
            .dim_candidates
            .iter()
            .filter_map(|(g, cands)| cands.iter().find_map(|c| cells.iter().position(|x| *x == norm(c))).map(|col| (g.clone(), col)))
            .collect();
        if present.is_empty() {
            continue;
        }
        let distinct = |col: usize| rows[i + 1..].iter().filter_map(|r| r.get(col)).filter(|c| is_dim_value(c)).map(|c| c.text().trim().to_string()).collect::<BTreeSet<_>>().len();
        let best = (0..present.len()).max_by_key(|&k| (distinct(present[k].1), std::cmp::Reverse(k))).unwrap_or(0);
        found = Some((i, present[best].0.clone(), present[best].1));
        break;
    }
    let Some((hi, grain, dcol)) = found else { return (vec![], Diag::because("no header row with a dim candidate + a measure")) };
    let header = &rows[hi];
    let mcols: BTreeMap<&String, Option<usize>> = fam.measures.iter().map(|(m, s)| (m, col_index(header, s))).collect();
    let missing: Vec<&String> = mcols.iter().filter(|(_, j)| j.is_none()).map(|(m, _)| *m).collect();
    let mut facts = vec![];
    for r in &rows[hi + 1..] {
        let Some(d) = r.get(dcol).filter(|d| is_dim_value(d)) else { continue };
        // An entity's encoding cleaned (`entity_regex`'s first group): "SE1: Pat Ode" → "SE1".
        let raw = d.text().trim().to_string();
        let raw = re.as_ref().and_then(|re| re.captures(&raw)).and_then(|c| c.get(1)).map(|m| m.as_str().to_string()).unwrap_or(raw);
        let ent = conform.apply(&grain, &raw);
        let rid = ids.next();
        for (m, j) in &mcols {
            if let Some(v) = j.and_then(|j| r.get(j)).and_then(Cell::number) {
                facts.push(Raw { metric: (*m).clone(), grain: grain.clone(), entity: ent.clone(), month: month.map(str::to_owned), value: v, row: rid });
            }
        }
    }
    let reason = if !missing.is_empty() { Some(format!("partial: {missing:?}")) } else if facts.is_empty() { Some("no data rows".into()) } else { None };
    (facts, Diag { reason, header: Some(hi) })
}

/// `matrix_month_cols`: metric rows × month columns; the first (leftmost) column of each month (the
/// monthly totals block); a metric's first matching row only.
pub fn matrix(rows: &Rows, fam: &Family, year: i32, ids: &mut RowIds) -> (Vec<Raw>, Diag) {
    let hr = fam.month_header_row.unwrap_or(1);
    let mcol = fam.metric_col.unwrap_or(0);
    let Some(head) = rows.get(hr) else { return (vec![], Diag::because(format!("fewer rows than month_header_row={hr}"))) };
    let mut month_col: BTreeMap<String, usize> = BTreeMap::new();
    for (j, c) in head.iter().enumerate() {
        if j == mcol || c.is_empty() {
            continue;
        }
        let m = match c {
            Cell::Date(y, m, _) => Some(format!("{y:04}-{m:02}")),
            _ => month_from_text(&c.text(), year),
        };
        if let Some(m) = m {
            month_col.entry(m).or_insert(j);
        }
    }
    let entity = fam.grains.values().next().and_then(|g| g.entity.clone()).unwrap_or_else(|| "overall".into());
    let (mut facts, mut missing) = (vec![], vec![]);
    for (m, label) in &fam.measures {
        let row = rows[hr + 1..].iter().find(|r| r.get(mcol).is_some_and(|c| !c.is_empty() && header_exact(&ncell(c), label)));
        match row {
            Some(r) => {
                let rid = ids.next();
                for (month, j) in &month_col {
                    if let Some(v) = r.get(*j).and_then(Cell::number) {
                        facts.push(Raw { metric: m.clone(), grain: "overall".into(), entity: entity.clone(), month: Some(month.clone()), value: v, row: rid });
                    }
                }
            }
            None => missing.push(m.clone()),
        }
    }
    let reason = if month_col.is_empty() { Some(format!("no month headers found in row {hr}")) } else if !missing.is_empty() { Some(format!("partial: labels not matched: {missing:?}")) } else { None };
    (facts, Diag { reason, header: Some(hr) })
}

/// `wide_month`: a banner row with a month per column block (forward-filled; a non-month label ends
/// the block), the header row repeating the measures.
#[allow(clippy::too_many_arguments)]
pub fn wide(rows: &Rows, fam: &Family, grain: &str, dim_header: &str, conform: &Conform, ents: &mut Entities, ids: &mut RowIds) -> (Vec<Raw>, Diag) {
    let br = fam.banner_row.unwrap_or(0);
    let hr = fam.header_row.unwrap_or(1);
    let (Some(banner), Some(header)) = (rows.get(br), rows.get(hr)) else { return (vec![], Diag::because(format!("fewer rows than header_row={hr}"))) };
    let mut colmonth: Vec<Option<String>> = vec![];
    let mut cur: Option<String> = None;
    for j in 0..header.len().max(banner.len()) {
        match banner.get(j) {
            Some(Cell::Date(y, m, _)) => cur = Some(format!("{y:04}-{m:02}")),
            Some(Cell::Text(s)) if !s.trim().is_empty() => cur = month_from_text(s, 2026),
            _ => {}
        }
        colmonth.push(cur.clone());
    }
    let dim = Spec::One(dim_header.into());
    let Some(dcol) = col_index(header, &dim).or_else(|| col_index(banner, &dim)) else {
        return (vec![], Diag::because(format!("dim column '{dim_header}' not found in rows {br}/{hr}")));
    };
    let body = &rows[hr + 1..];
    ents.scan(&body.iter().filter_map(|r| r.get(dcol)).collect::<Vec<_>>());
    let mut facts = vec![];
    for r in body {
        let Some(d) = r.get(dcol).filter(|d| is_dim_value(d)) else { continue };
        let ent = conform.apply(grain, &ents.resolve(d, r, header));
        let rid = ids.next();
        for (j, h) in header.iter().enumerate() {
            let hn = ncell(h);
            for (m, spec) in &fam.measures {
                if header_exact(&hn, spec) {
                    if let (Some(Some(month)), Some(v)) = (colmonth.get(j), r.get(j).and_then(Cell::number)) {
                        facts.push(Raw { metric: m.clone(), grain: grain.into(), entity: ent.clone(), month: Some(month.clone()), value: v, row: rid });
                    }
                }
            }
        }
    }
    let reason = facts.is_empty().then(|| "no month-banner columns matched measures".to_string());
    (facts, Diag { reason, header: Some(hr) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_and_titles_as_brain_reads_them() {
        assert_eq!(month_from_text("June 7-Day and May 30-Day Service Summary.xlsx", 2026).as_deref(), Some("2026-06"));
        assert_eq!(month_from_text("JAN Survey Ranker by Region 02.09.26.xlsx", 2026).as_deref(), Some("2026-01"));
        assert_eq!(month_from_text("01 - January 2026 - Volume by Region 02.07.26.xlsx", 2026).as_deref(), Some("2026-01"));
        assert_eq!(month_from_text("Region Scorecard - April.xlsx", 2025).as_deref(), Some("2025-04"));
        assert_eq!(month_from_text("Totals", 2026), None);
        // As Brain's smart_title: an all-caps word of three letters or fewer stays.
        assert_eq!(smart_title("NEW TOWN CITY"), "NEW Town City");
        assert_eq!(smart_title("RG3 - north region"), "RG3 - North Region");
        assert_eq!(Cell::Text("$1,234.5".into()).number(), Some(1234.5));
        assert_eq!(Cell::Text("12%".into()).number(), Some(12.0));
        assert!(!is_dim_value(&Cell::Text(" Grand Total ".into())));
        // A note beside the table: right of it past a blank column, alone; a side table's label isn't.
        let t = |s: &str| Cell::Text(s.into());
        let rows: Rows = vec![
            vec![t("Report title — July")],
            vec![t("Branch"), t("Calls"), Cell::Empty, t("Export failed; 37 rows missing from 3/1.")],
            vec![t("Akron"), Cell::Num(10.0), Cell::Empty, Cell::Empty],
            vec![t("Boise"), Cell::Num(12.0), Cell::Empty, t("Side table"), Cell::Num(3.0)],
        ];
        assert_eq!(scan_notes(&rows, Some(1)), vec![("D2".to_string(), "Export failed; 37 rows missing from 3/1.".to_string())]);
        assert_eq!(cell_ref(0, 27), "AB1");
    }
}

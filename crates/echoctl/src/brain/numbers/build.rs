//! A client's build (Brain's `build_marts.py` main, ported): every family's files in vintage order
//! (oldest report first, so the newest wins), loaded by layout; collisions merged by the family's
//! policy; restatements (a later report changed a figure) and conflicts (reports of one month
//! disagree) kept in `fact_versions`, every report in `fact_reports`; rollups and derived metrics;
//! the audit (a file that yielded nothing is loud) and coverage (an entity or month that vanished).

use super::load::{self, Book, Conform, Entities, Raw, RowIds};
use super::{Config, Family, Spec};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// A loaded value with where it came from.
#[derive(Debug, Clone)]
struct Fact {
    family: String,
    metric: String,
    grain: String,
    entity: String,
    month: Option<String>,
    value: f64,
    source: String,
    row: u64,
}

type Key = (String, String, String, String, Option<String>);
/// A report of a fact: the month its file reports, whether it is the current value.
type Report = (Fact, Option<String>, bool);

impl Fact {
    fn key(&self) -> Key {
        (self.family.clone(), self.metric.clone(), self.grain.clone(), self.entity.clone(), self.month.clone())
    }
}

fn file_year(p: &Path) -> i32 {
    use chrono::Datelike;
    std::fs::metadata(p).and_then(|m| m.modified()).map(|t| chrono::DateTime::<chrono::Utc>::from(t).year()).unwrap_or_else(|_| chrono::Utc::now().year())
}

fn basename(rel: &str) -> String {
    rel.rsplit('/').next().unwrap_or(rel).to_string()
}

/// Build a client's facts from its config; `roots`: the folders to read (default: its threads').
pub fn run(client: &str, cfg: &Config, roots: Option<Vec<(PathBuf, Vec<String>)>>) -> Result<Value> {
    build(client, cfg, roots, true)
}

/// The audit of a config without keeping anything (the proposal's repair rounds).
pub fn dry(client: &str, cfg: &Config, roots: Option<Vec<(PathBuf, Vec<String>)>>) -> Result<Value> {
    build(client, cfg, roots, false)
}

fn build(client: &str, cfg: &Config, roots: Option<Vec<(PathBuf, Vec<String>)>>, keep: bool) -> Result<Value> {
    cfg.check()?;
    let roots = roots.unwrap_or_else(|| super::roots(client));
    let books = super::workbooks(&roots);
    let titlecase: BTreeSet<String> = cfg.conform_titlecase.iter().cloned().collect();
    let conform = Conform { map: &cfg.dimension_map, titlecase: &titlecase };
    let mut ids = RowIds(0);
    let mut facts: Vec<Fact> = vec![];
    let mut audit: Vec<Value> = vec![];
    let mut vintage: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut mapped: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    // Notes beside loaded tables: (family, file, sheet, cell, month, text).
    let mut notes: BTreeSet<(String, String, String, String, Option<String>, String)> = BTreeSet::new();
    for fam in &cfg.families {
        let globs = fam.glob.alts();
        let mut files: Vec<(PathBuf, String)> = books.iter().filter(|(_, rel)| globs.iter().any(|g| crate::brain::files::glob_match(rel, g))).cloned().collect();
        // Oldest report first: on a duplicate metric-month the newest report wins.
        files.sort_by_key(|(root, rel)| (load::month_from_text(&basename(rel), file_year(&root.join(rel))).unwrap_or_default(), basename(rel)));
        if files.is_empty() {
            audit.push(json!({"family": fam.name, "file": null, "unit": "-", "facts": 0, "status": "zero", "reason": format!("glob matched no files: {:?}", globs)}));
        }
        let benign: Vec<String> = fam.allow_zero.iter().map(|s| s.to_lowercase()).collect();
        for (root, rel) in files {
            let path = root.join(&rel);
            // A file is its folder-relative path (monthly folders hold same-named reports), its month
            // still read from its name.
            let fname = rel.clone();
            let year = file_year(&path);
            vintage.insert(fname.clone(), load::month_from_text(&basename(&rel), year));
            let month = (fam.month_from == "filename").then(|| load::month_from_text(&basename(&rel), year)).flatten();
            let mut record = |unit: &str, got: Vec<Raw>, reason: Option<String>| {
                let n = got.len();
                facts.extend(got.into_iter().map(|r| Fact { family: fam.name.clone(), metric: r.metric, grain: r.grain, entity: r.entity, month: r.month, value: r.value, source: fname.clone(), row: r.row }));
                let status = if n == 0 {
                    if benign.iter().any(|b| basename(&fname).to_lowercase().contains(b)) { "benign" } else { "zero" }
                } else if reason.is_some() {
                    "partial"
                } else {
                    "ok"
                };
                audit.push(json!({"family": fam.name, "file": rel, "unit": unit, "facts": n, "status": status, "reason": reason}));
            };
            let mut book = match Book::open(&path) {
                Ok(b) => b,
                Err(e) => {
                    audit.push(json!({"family": fam.name, "file": rel, "unit": "-", "facts": 0, "status": "error", "reason": format!("{e:#}")}));
                    continue;
                }
            };
            match fam.layout.as_str() {
                "matrix_month_cols" => {
                    let spec = fam.sheet.clone().unwrap_or(Spec::One("MOM".into()));
                    match book.sheet(&spec) {
                        None => record(&spec.alts().join("|"), vec![], Some(format!("no sheet {:?}", spec.alts()))),
                        Some(sh) => {
                            let rows = book.rows(&sh);
                            let (got, d) = load::matrix(&rows, fam, year, &mut ids);
                            if fam.notes != Some(false) {
                                notes.extend(load::scan_notes(&rows, d.header).into_iter().map(|(cell, t)| (fam.name.clone(), fname.clone(), sh.clone(), cell, None, t)));
                            }
                            record(&sh, got, d.reason);
                        }
                    }
                }
                "tolerant_long" => {
                    let mut total = vec![];
                    for sh in book.names.clone() {
                        let rows = book.rows(&sh);
                        let (got, d) = load::tolerant(&rows, fam, month.as_deref(), &conform, &mut ids);
                        // Only sheets the family loaded (detail dumps can hold personal data).
                        if !got.is_empty() && fam.notes != Some(false) {
                            notes.extend(load::scan_notes(&rows, d.header).into_iter().map(|(cell, t)| (fam.name.clone(), fname.clone(), sh.clone(), cell, month.clone(), t)));
                        }
                        total.extend(got);
                    }
                    let reason = total.is_empty().then(|| "no sheet yielded rows (dim+measure not found)".to_string());
                    record("auto", total, reason);
                }
                _ => {
                    for (grain, g) in &fam.grains {
                        let spec = g.sheet.clone().unwrap_or(Spec::One(grain.clone()));
                        let Some(sh) = book.sheet(&spec) else {
                            record(grain, vec![], Some(format!("no sheet {:?}", spec.alts())));
                            continue;
                        };
                        let rows = book.rows(&sh);
                        let dim = g.dim_header.clone().unwrap_or_else(|| grain.clone());
                        let mut ents = Entities::new(&mut book, g);
                        let (got, d) = if fam.layout == "wide_month" {
                            load::wide(&rows, fam, grain, &dim, &conform, &mut ents, &mut ids)
                        } else {
                            load::long(&rows, fam, grain, &dim, month.as_deref(), &conform, &mut ents, &mut ids)
                        };
                        if fam.notes != Some(false) {
                            notes.extend(load::scan_notes(&rows, d.header).into_iter().map(|(cell, t)| (fam.name.clone(), fname.clone(), sh.clone(), cell, month.clone(), t)));
                        }
                        for (raw, to) in ents.mapped.drain(..) {
                            mapped.entry((fam.name.clone(), raw)).or_default().insert(to);
                        }
                        record(grain, got, d.reason);
                    }
                }
            }
        }
    }
    let fams: BTreeMap<String, &Family> = cfg.families.iter().map(|f| (f.name.clone(), f)).collect();
    let (per_file, collisions, merges) = resolve_collisions(facts, &fams);
    let (versions, restated) = restatements(&per_file, &vintage);
    // Every report (one per key and file, file order); the last per key is current.
    let mut last_of: BTreeMap<Key, usize> = BTreeMap::new();
    for (i, f) in per_file.iter().enumerate() {
        last_of.insert(f.key(), i);
    }
    let reports: Vec<Report> = per_file.iter().enumerate().map(|(i, f)| (f.clone(), vintage.get(&f.source).cloned().flatten(), last_of.get(&f.key()) == Some(&i))).collect();
    let mut current: BTreeMap<Key, Fact> = BTreeMap::new();
    for f in &per_file {
        current.insert(f.key(), f.clone());
    }
    rollups(&mut current, cfg, &super::dir(client));
    derived(&mut current, cfg);
    let coverage = coverage(&current, cfg);
    let n = |s: &str| audit.iter().filter(|x| x["status"] == s).count();
    if !keep {
        return Ok(json!({"summary": {"units": audit.len(), "ok": n("ok"), "partial": n("partial"), "zero_or_error": n("zero") + n("error"), "facts": current.len()}, "audit": audit}));
    }
    // Write: this client's rows replaced in one transaction.
    let path = crate::brain::store_path();
    let c = crate::brain::store::open(&path)?;
    super::ensure_schema(&c)?;
    let tx = c.unchecked_transaction()?;
    for t in ["facts", "fact_reports", "fact_versions", "fact_merges", "fact_notes"] {
        tx.execute(&format!("DELETE FROM {t} WHERE client = ?1"), [client])?;
    }
    {
        let mut st = tx.prepare("INSERT INTO facts VALUES (?1,?2,?3,?4,?5,?6,?7,?8)")?;
        for f in current.values() {
            st.execute(rusqlite::params![client, f.family, f.metric, f.grain, f.entity, f.month, f.value, f.source])?;
        }
        let mut st = tx.prepare("INSERT INTO fact_reports VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)")?;
        for (f, rin, cur) in &reports {
            st.execute(rusqlite::params![client, f.family, f.metric, f.grain, f.entity, f.month, f.value, f.source, rin, *cur as i64])?;
        }
        let mut st = tx.prepare("INSERT INTO fact_versions VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)")?;
        for (f, rin, cur) in &versions {
            st.execute(rusqlite::params![client, f.family, f.metric, f.grain, f.entity, f.month, f.value, f.source, rin, *cur as i64])?;
        }
        let mut st = tx.prepare("INSERT INTO fact_notes VALUES (?1,?2,?3,?4,?5,?6,?7)")?;
        for (fam, file, sheet, cell, month, text) in &notes {
            st.execute(rusqlite::params![client, fam, file, sheet, cell, month, text])?;
        }
        let mut st = tx.prepare("INSERT INTO fact_merges VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)")?;
        for m in &merges {
            st.execute(rusqlite::params![client, m["family"].as_str(), m["metric"].as_str(), m["grain"].as_str(), m["entity"].as_str(), m["month"].as_str(), m["source_file"].as_str(), m["policy"].as_str(), m["n_rows"].as_i64(), m["value"].as_f64(), m["inputs"].to_string()])?;
        }
    }
    tx.commit()?;
    let inconsistent: Vec<Value> = mapped.iter().filter(|(_, t)| t.len() > 1).map(|((f, r), t)| json!({"family": f, "raw": r, "targets": t})).collect();
    let summary = json!({"units": audit.len(), "ok": n("ok"), "partial": n("partial"), "benign": n("benign"), "zero_or_error": n("zero") + n("error"),
                         "facts": current.len(), "restated": restated.iter().filter(|r| r["kind"] == "restated").count(),
                         "conflicts": restated.iter().filter(|r| r["kind"] == "conflict").count(), "collisions": collisions.len(),
                         "collisions_merged": collisions.iter().filter(|c| c.get("resolved").is_some()).count(), "entity_map_inconsistent": inconsistent.len(),
                         "notes": notes.len(), "holes": coverage["intra_family_holes"].as_array().map(|a| a.len()).unwrap_or(0),
                         "expected_violations": coverage["expected_violations"].as_array().map(|a| a.len()).unwrap_or(0)});
    let out = json!({"summary": summary, "built_at": chrono::Utc::now().to_rfc3339(), "inputs": super::inputs(client), "audit": audit, "restatements": restated, "collisions": collisions, "entity_map_inconsistent": inconsistent,
                     "notes": notes.iter().map(|(f, file, sheet, cell, month, text)| json!({"family": f, "source_file": file, "sheet": sheet, "cell": cell, "month": month, "text": text})).collect::<Vec<_>>()});
    std::fs::create_dir_all(super::dir(client))?;
    crate::paths::write_atomic(&super::dir(client).join("build_audit.json"), serde_json::to_string_pretty(&out)?.as_bytes())?;
    crate::paths::write_atomic(&super::dir(client).join("coverage.json"), serde_json::to_string_pretty(&coverage)?.as_bytes())?;
    Ok(json!({"summary": summary, "coverage": coverage["family_grains"]}))
}

fn policy_for<'a>(c: &'a super::Collision, metric: &str) -> &'a str {
    if c.sum.iter().any(|m| m == metric) {
        return "sum";
    }
    c.metrics.get(metric).map(String::as_str).unwrap_or(&c.policy)
}

/// One file listing a key on several rows. No policy: the last row wins (reported when values
/// differ). A policy merges them; `weighted_mean` weighs each value by the weight measure of the
/// same source row. Returns one fact per key and file (file order kept), the audit, the merges.
fn resolve_collisions(facts: Vec<Fact>, fams: &BTreeMap<String, &Family>) -> (Vec<Fact>, Vec<Value>, Vec<Value>) {
    let mut weights: BTreeMap<(String, String, u64), f64> = BTreeMap::new();
    for f in &facts {
        if let Some(w) = fams.get(&f.family).and_then(|x| x.collision.as_ref()).and_then(|c| c.weight.as_deref()) {
            if f.metric == w {
                weights.insert((f.family.clone(), f.source.clone(), f.row), f.value);
            }
        }
    }
    let mut groups: BTreeMap<(Key, String), Vec<usize>> = BTreeMap::new();
    for (i, f) in facts.iter().enumerate() {
        groups.entry((f.key(), f.source.clone())).or_default().push(i);
    }
    let (mut audit, mut merges) = (vec![], vec![]);
    let mut resolved: BTreeMap<(Key, String), f64> = BTreeMap::new();
    for ((key, src), idx) in &groups {
        let rows: BTreeSet<u64> = idx.iter().map(|&i| facts[i].row).collect();
        if rows.len() < 2 {
            continue;
        }
        let vals: Vec<f64> = idx.iter().map(|&i| facts[i].value).collect();
        let entry = json!({"family": key.0, "metric": key.1, "grain": key.2, "entity": key.3, "month": key.4, "source_file": src});
        let distinct: BTreeSet<i64> = vals.iter().map(|v| (v * 1e9).round() as i64).collect();
        let Some(spec) = fams.get(&key.0).and_then(|x| x.collision.as_ref()) else {
            if distinct.len() > 1 {
                let mut e = entry;
                e["policy"] = json!("last");
                e["values"] = json!(vals);
                audit.push(e);
            }
            continue;
        };
        let pol = policy_for(spec, &key.1);
        let (value, inputs): (Option<f64>, Value) = if pol == "weighted_mean" {
            let pairs: Vec<(f64, Option<f64>)> = idx.iter().map(|&i| (facts[i].value, weights.get(&(key.0.clone(), src.clone(), facts[i].row)).copied())).collect();
            let ok: Vec<(f64, f64)> = pairs.iter().filter_map(|(v, w)| w.filter(|w| *w > 0.0).map(|w| (*v, w))).collect();
            let v = (!ok.is_empty()).then(|| ok.iter().map(|(v, w)| v * w).sum::<f64>() / ok.iter().map(|(_, w)| w).sum::<f64>());
            (v, json!(pairs.iter().map(|(v, w)| json!({"value": v, "weight": w})).collect::<Vec<_>>()))
        } else {
            let v = match pol {
                "first" => vals[0],
                "max" => vals.iter().cloned().fold(f64::MIN, f64::max),
                "min" => vals.iter().cloned().fold(f64::MAX, f64::min),
                "sum" => vals.iter().sum(),
                "mean" => vals.iter().sum::<f64>() / vals.len() as f64,
                _ => *vals.last().expect("a value"),
            };
            (Some(v), json!(vals.iter().map(|v| json!({"value": v})).collect::<Vec<_>>()))
        };
        let mut e = entry.clone();
        e["policy"] = json!(pol);
        e["values"] = json!(vals);
        match (pol, value) {
            ("error", _) => e["error"] = json!(true),
            (_, None) => e["unresolved"] = json!("no row carries a positive weight; kept the last row"),
            (_, Some(v)) => {
                e["resolved"] = json!(v);
                resolved.insert((key.clone(), src.clone()), v);
                let mut m = entry;
                m["policy"] = json!(pol);
                m["n_rows"] = json!(vals.len());
                m["value"] = json!(v);
                m["inputs"] = inputs;
                merges.push(m);
            }
        }
        audit.push(e);
    }
    // One per key and file: the last row (file order), with a merged value where there is one.
    let mut seen: BTreeSet<(Key, String)> = BTreeSet::new();
    let mut out: Vec<Fact> = vec![];
    for f in facts.into_iter().rev() {
        let k = (f.key(), f.source.clone());
        if seen.insert(k.clone()) {
            let mut f = f;
            if let Some(v) = resolved.get(&k) {
                f.value = *v;
            }
            out.push(f);
        }
    }
    out.reverse();
    (out, audit, merges)
}

/// Keys whose reported value changed between files: every other value once (cited to the first file
/// that reported it) plus the current one; `restated` when a later report changed it, else `conflict`.
fn restatements(per_file: &[Fact], vintage: &BTreeMap<String, Option<String>>) -> (Vec<Report>, Vec<Value>) {
    let mut by: BTreeMap<Key, Vec<&Fact>> = BTreeMap::new();
    for f in per_file {
        by.entry(f.key()).or_default().push(f);
    }
    let r9 = |v: f64| (v * 1e9).round() as i64;
    let (mut versions, mut audit) = (vec![], vec![]);
    for (k, fs) in by {
        let distinct: BTreeSet<i64> = fs.iter().map(|f| r9(f.value)).collect();
        if distinct.len() < 2 {
            continue;
        }
        let cur = *fs.last().expect("a report");
        let mut earlier: Vec<&Fact> = vec![];
        let mut had: BTreeSet<i64> = BTreeSet::new();
        for f in &fs {
            if r9(f.value) != r9(cur.value) && had.insert(r9(f.value)) {
                earlier.push(f);
            }
        }
        let vin = |f: &Fact| vintage.get(&f.source).cloned().flatten();
        for f in &earlier {
            versions.push(((*f).clone(), vin(f), false));
        }
        versions.push((cur.clone(), vin(cur), true));
        let cur_v = vin(cur);
        let restated = cur_v.as_ref().is_some_and(|cv| earlier.iter().all(|f| vin(f).is_some_and(|v| &v < cv)));
        audit.push(json!({"family": k.0, "metric": k.1, "grain": k.2, "entity": k.3, "month": k.4, "kind": if restated { "restated" } else { "conflict" },
                          "current": {"value": cur.value, "source_file": cur.source}, "earlier": earlier.iter().map(|f| json!({"value": f.value, "source_file": f.source})).collect::<Vec<_>>()}));
    }
    (versions, audit)
}

/// Weighted rollups to a coarser grain (rates weighted by a count; the count summed), parents by a
/// crosswalk or a code prefix; rows cited `<rollup>`.
fn rollups(cur: &mut BTreeMap<Key, Fact>, cfg: &Config, dir: &Path) {
    for spec in &cfg.rollups {
        let (Some(fam), Some(from), Some(to), Some(wm)) = (spec["family"].as_str(), spec["from"].as_str(), spec["to"].as_str(), spec["weight"].as_str()) else { continue };
        let xwalk: BTreeMap<String, String> = match &spec["crosswalk"] {
            Value::Object(m) => m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.trim().to_lowercase(), v.to_string()))).collect(),
            // A path: relative to the client's numbers folder.
            Value::String(p) => std::fs::read(dir.join(p)).ok().and_then(|b| serde_json::from_slice::<BTreeMap<String, String>>(&b).ok()).map(|m| m.into_iter().map(|(k, v)| (k.trim().to_lowercase(), v)).collect()).unwrap_or_default(),
            _ => BTreeMap::new(),
        };
        let prefixes: Vec<(String, String)> = spec["map_prefix"].as_object().map(|m| m.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.to_uppercase(), v.to_string()))).collect()).unwrap_or_default();
        let parent = |e: &str| -> Option<String> { xwalk.get(&e.trim().to_lowercase()).cloned().or_else(|| prefixes.iter().find(|(p, _)| e.to_uppercase().starts_with(p)).map(|(_, v)| v.clone())) };
        let sub: Vec<Fact> = cur.values().filter(|f| f.family == fam && f.grain == from).cloned().collect();
        let weight: BTreeMap<(String, Option<String>), f64> = sub.iter().filter(|f| f.metric == wm).map(|f| ((f.entity.clone(), f.month.clone()), f.value)).collect();
        let mut acc: BTreeMap<(String, String, Option<String>), (f64, f64)> = BTreeMap::new();
        for f in &sub {
            let Some(p) = parent(&f.entity) else { continue };
            if f.metric == wm {
                acc.entry((p, wm.to_string(), f.month.clone())).or_default().0 += f.value;
                continue;
            }
            let w = weight.get(&(f.entity.clone(), f.month.clone())).copied().unwrap_or(0.0);
            let e = acc.entry((p, f.metric.clone(), f.month.clone())).or_default();
            e.0 += f.value * w;
            e.1 += w;
        }
        for ((p, metric, month), (s, w)) in acc {
            let value = if metric == wm { s } else if w > 0.0 { s / w } else { continue };
            let f = Fact { family: fam.into(), metric, grain: to.into(), entity: p, month, value, source: "<rollup>".into(), row: 0 };
            cur.insert(f.key(), f);
        }
    }
}

/// Ratio metrics at a grain (numerator / denominator, joined on entity and month; a missing operand
/// or a zero denominator gives no row), cited `<derived>`.
fn derived(cur: &mut BTreeMap<Key, Fact>, cfg: &Config) {
    for spec in &cfg.derived {
        let (Some(fam), Some(metric), Some(grain), Some(num), Some(den)) = (spec["family"].as_str(), spec["metric"].as_str(), spec["grain"].as_str(), spec["numerator"].as_str(), spec["denominator"].as_str()) else { continue };
        let scale = spec["scale"].as_f64().unwrap_or(1.0);
        let pick = |m: &str| -> BTreeMap<(String, Option<String>), f64> { cur.values().filter(|f| f.family == fam && f.grain == grain && f.metric == m).map(|f| ((f.entity.clone(), f.month.clone()), f.value)).collect() };
        let (n, d) = (pick(num), pick(den));
        for ((e, m), nv) in n {
            if let Some(dv) = d.get(&(e.clone(), m.clone())).filter(|v| **v != 0.0) {
                let f = Fact { family: fam.into(), metric: metric.into(), grain: grain.into(), entity: e, month: m, value: nv / dv * scale, source: "<derived>".into(), row: 0 };
                cur.insert(f.key(), f);
            }
        }
    }
}

fn month_range(a: &str, b: &str) -> Vec<String> {
    let p = |s: &str| -> Option<(i32, u32)> { Some((s.get(..4)?.parse().ok()?, s.get(5..7)?.parse().ok()?)) };
    let (Some((mut y, mut m)), Some((ye, me))) = (p(a), p(b)) else { return vec![] };
    let mut out = vec![];
    while (y, m) <= (ye, me) && out.len() < 240 {
        out.push(format!("{y:04}-{m:02}"));
        m += 1;
        if m > 12 {
            m = 1;
            y += 1;
        }
    }
    out
}

/// Completeness the audit can't see: per family and grain its months and entities, an entity there
/// some months and missing others (holes), and the expected roster's missing months or entities.
fn coverage(cur: &BTreeMap<Key, Fact>, cfg: &Config) -> Value {
    let mut by: BTreeMap<(String, String), Vec<&Fact>> = BTreeMap::new();
    for f in cur.values() {
        by.entry((f.family.clone(), f.grain.clone())).or_default().push(f);
    }
    let (mut grains, mut holes, mut violations) = (vec![], vec![], vec![]);
    let mut fam_grains: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ((fam, grain), fs) in by {
        let months: BTreeSet<String> = fs.iter().filter_map(|f| f.month.clone()).collect();
        let ents: BTreeSet<String> = fs.iter().map(|f| f.entity.clone()).collect();
        let present: BTreeSet<(String, String)> = fs.iter().filter_map(|f| f.month.clone().map(|m| (f.entity.clone(), m))).collect();
        let ent_holes: BTreeMap<String, Vec<String>> = ents
            .iter()
            .filter_map(|e| {
                let miss: Vec<String> = months.iter().filter(|m| !present.contains(&(e.clone(), (*m).clone()))).cloned().collect();
                (!miss.is_empty()).then(|| (e.clone(), miss))
            })
            .collect();
        if let Some(exp) = cfg.coverage.get(&fam).or(cfg.coverage.get("*")) {
            let want: Vec<String> = match exp["month_range"].as_array() {
                Some(r) if r.len() == 2 => month_range(r[0].as_str().unwrap_or(""), r[1].as_str().unwrap_or("")),
                _ => exp["months"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
            };
            let miss: Vec<&String> = want.iter().filter(|m| !months.contains(*m)).collect();
            if !miss.is_empty() {
                violations.push(json!({"family": fam, "grain": grain, "missing_months": miss}));
            }
            let miss_e: Vec<&str> = exp["entities"].as_array().into_iter().flatten().filter_map(|e| e.as_str()).filter(|e| !ents.contains(*e)).collect();
            if !miss_e.is_empty() {
                violations.push(json!({"family": fam, "grain": grain, "missing_entities": miss_e}));
            }
        }
        grains.push(json!({"family": fam, "grain": grain, "n_months": months.len(), "month_span": [months.first(), months.last()], "n_entities": ents.len(), "holes": ent_holes.len()}));
        fam_grains.entry(fam.clone()).or_default().push(grain.clone());
        if !ent_holes.is_empty() {
            holes.push(json!({"family": fam, "grain": grain, "holes": ent_holes}));
        }
    }
    json!({"grains": grains, "family_grains": fam_grains, "intra_family_holes": holes, "expected_violations": violations})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(metric: &str, entity: &str, month: &str, value: f64, source: &str, row: u64) -> Fact {
        Fact { family: "nps".into(), metric: metric.into(), grain: "branch".into(), entity: entity.into(), month: Some(month.into()), value, source: source.into(), row }
    }

    #[test]
    fn collisions_restatements_and_derived() {
        // One file lists Riverton twice: weighted by its records.
        let fam = Family { name: "nps".into(), layout: "long".into(), glob: Spec::One("*".into()), collision: Some(super::super::Collision { policy: "weighted_mean".into(), weight: Some("n".into()), sum: vec!["n".into()], metrics: BTreeMap::new() }), ..Default::default() };
        let fams: BTreeMap<String, &Family> = [("nps".to_string(), &fam)].into_iter().collect();
        let facts = vec![fact("score", "Riverton", "2026-05", 80.0, "may.xlsx", 1), fact("n", "Riverton", "2026-05", 30.0, "may.xlsx", 1), fact("score", "Riverton", "2026-05", 40.0, "may.xlsx", 2), fact("n", "Riverton", "2026-05", 10.0, "may.xlsx", 2)];
        let (out, audit, merges) = resolve_collisions(facts, &fams);
        let score = out.iter().find(|f| f.metric == "score").unwrap();
        assert!((score.value - 70.0).abs() < 1e-9, "{}", score.value);
        assert_eq!(out.iter().find(|f| f.metric == "n").unwrap().value, 40.0);
        assert_eq!((audit.len(), merges.len()), (2, 2));
        // A later report changes May: restated; the June file is newer than the May one.
        let v: BTreeMap<String, Option<String>> = [("may.xlsx".to_string(), Some("2026-05".to_string())), ("june.xlsx".to_string(), Some("2026-06".to_string()))].into_iter().collect();
        let per_file = vec![fact("score", "Riverton", "2026-05", 70.0, "may.xlsx", 1), fact("score", "Riverton", "2026-05", 72.0, "june.xlsx", 3)];
        let (versions, restated) = restatements(&per_file, &v);
        assert_eq!(restated[0]["kind"], "restated");
        assert_eq!(versions.len(), 2);
        // Derived: a missing operand gives no row.
        let mut cur: BTreeMap<Key, Fact> = BTreeMap::new();
        for f in [fact("calls", "A", "2026-05", 50.0, "x", 0), fact("base", "A", "2026-05", 200.0, "x", 0), fact("calls", "B", "2026-05", 9.0, "x", 0)] {
            cur.insert(f.key(), f);
        }
        let cfg = Config { derived: vec![json!({"family": "nps", "metric": "per", "grain": "branch", "numerator": "calls", "denominator": "base"})], ..Default::default() };
        derived(&mut cur, &cfg);
        let per: Vec<&Fact> = cur.values().filter(|f| f.metric == "per").collect();
        assert_eq!(per.len(), 1);
        assert_eq!(per[0].value, 0.25);
    }
}


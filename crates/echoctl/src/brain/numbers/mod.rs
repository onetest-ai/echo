//! The numbers lane (Brain's `tabular-semantic-layer`, ported): exact figures from a client's
//! reporting workbooks. The model picks a governed metric and filters; SQLite gives the value from
//! the real cells, citing its file — a number is never asserted by a model. Per client, from its
//! threads' folders: `families.json` (Brain's format: which files, sheets, grains, measures, layout;
//! proposed by Claude from profiles, accepted by the user) and `metrics.json` (the catalog: unit,
//! description, definition — reported vs computed) in `<home>/brain/numbers/<client>/`; the facts in
//! the Brain's store (`facts`, `fact_reports`, `fact_versions`, `fact_merges`), the build's audit and
//! coverage beside the config.

pub mod build;
pub mod load;
pub mod profile;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A header or sheet name, or a list of alternatives (templates drift).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Spec {
    One(String),
    Many(Vec<String>),
}

impl Default for Spec {
    fn default() -> Self {
        Spec::One(String::new())
    }
}

impl Spec {
    pub fn alts(&self) -> Vec<String> {
        match self {
            Spec::One(s) => vec![s.clone()],
            Spec::Many(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityMap {
    pub sheet: String,
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Grain {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<Spec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim_header: Option<String>,
    /// `matrix_month_cols`: the one entity the sheet is about ("OVERALL").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_map: Option<EntityMap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_with: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Collision {
    #[serde(default = "last")]
    pub policy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sum: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metrics: BTreeMap<String, String>,
}

fn last() -> String {
    "last".into()
}

pub const POLICIES: &[&str] = &["last", "first", "max", "min", "sum", "mean", "weighted_mean", "error"];

/// One family of reports (Brain's `families.<corpus>.json` block).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Family {
    pub name: String,
    /// Folder-relative globs (`**` spans folders), one or several.
    pub glob: Spec,
    /// `filename` | `wide_banner` | `matrix_header`.
    #[serde(default)]
    pub month_from: String,
    /// `long` | `tolerant_long` | `matrix_month_cols` | `wide_month`.
    pub layout: String,
    #[serde(default)]
    pub grains: BTreeMap<String, Grain>,
    #[serde(default)]
    pub measures: BTreeMap<String, Spec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_row: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_row: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub month_header_row: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric_col: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<Spec>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dim_candidates: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision: Option<Collision>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_zero: Vec<String>,
    /// `tolerant_long`: a regex whose first group is the entity (`^(\w+\d*):` → "SE1").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_regex: Option<String>,
    /// Notes written beside a loaded table become caveats on its figures (none: yes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<bool>,
    /// Anything else Brain's format carries (`_comment`, `notes`…), kept as written.
    #[serde(flatten)]
    pub other: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub families: Vec<Family>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conform_titlecase: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dimension_map: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rollups: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived: Vec<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub coverage: BTreeMap<String, Value>,
    /// Echo's own: `proposed` (by Claude, waiting for the user) | `accepted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub _echo: Option<Value>,
}

impl Config {
    pub fn accepted(&self) -> bool {
        self._echo.as_ref().is_some_and(|e| e["status"] == "accepted")
    }

    /// Refuse what can't be built (Brain's `check_collision_config`), before anything is read.
    pub fn check(&self) -> Result<()> {
        for f in &self.families {
            anyhow::ensure!(["long", "tolerant_long", "matrix_month_cols", "wide_month"].contains(&f.layout.as_str()), "family {}: unknown layout {:?}", f.name, f.layout);
            if let Some(r) = &f.entity_regex {
                let re = regex::Regex::new(r).map_err(|e| anyhow!("family {}: entity_regex {r:?}: {e}", f.name))?;
                anyhow::ensure!(re.captures_len() > 1, "family {}: entity_regex {r:?} needs a group (the entity)", f.name);
            }
            if let Some(c) = &f.collision {
                for p in std::iter::once(&c.policy).chain(c.metrics.values()) {
                    anyhow::ensure!(POLICIES.contains(&p.as_str()), "family {}: unknown collision policy {p:?}", f.name);
                }
                let weighted = c.policy == "weighted_mean" || c.metrics.values().any(|p| p == "weighted_mean");
                if weighted {
                    let w = c.weight.as_deref().unwrap_or("");
                    anyhow::ensure!(f.measures.contains_key(w), "family {}: collision weight {w:?} must be one of its measures", f.name);
                }
            }
        }
        Ok(())
    }
}

pub fn dir(client: &str) -> PathBuf {
    crate::paths::home().join("brain").join("numbers").join(client)
}

pub fn load_config(client: &str) -> Option<Config> {
    std::fs::read(dir(client).join("families.json")).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

pub fn save_config(client: &str, c: &Config) -> Result<()> {
    std::fs::create_dir_all(dir(client))?;
    crate::paths::write_atomic(&dir(client).join("families.json"), serde_json::to_string_pretty(c)?.as_bytes())?;
    Ok(())
}

/// The metric catalog (Brain's `metrics.<corpus>.json`): name → {family, metric, unit, desc, definition}.
pub fn load_catalog(client: &str) -> BTreeMap<String, Value> {
    std::fs::read(dir(client).join("metrics.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| serde_json::from_value(v.get("metrics").cloned().unwrap_or(v)).ok())
        .unwrap_or_default()
}

pub fn save_catalog(client: &str, m: &BTreeMap<String, Value>) -> Result<()> {
    std::fs::create_dir_all(dir(client))?;
    crate::paths::write_atomic(&dir(client).join("metrics.json"), serde_json::to_string_pretty(&json!({"metrics": m}))?.as_bytes())?;
    Ok(())
}

/// The folders a client's threads read (root, excludes).
pub fn roots(client: &str) -> Vec<(PathBuf, Vec<String>)> {
    let mut out: Vec<(PathBuf, Vec<String>)> = vec![];
    for p in crate::projects::list().into_iter().filter(|p| p.client.as_deref() == Some(client) && !p.stage.closed()) {
        for s in p.sources.iter().filter(|s| s.kind == "folder") {
            if !out.iter().any(|(r, _)| r == Path::new(&s.id)) {
                out.push((PathBuf::from(&s.id), s.exclude.clone()));
            }
        }
    }
    out
}

/// Every spreadsheet under the roots, as (root, folder-relative path), hidden and excluded ones left
/// out (the same rules as the folder lane).
pub fn workbooks(roots: &[(PathBuf, Vec<String>)]) -> Vec<(PathBuf, String)> {
    let mut out = vec![];
    for (root, ex) in roots {
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                let Ok(rel) = p.strip_prefix(root) else { continue };
                let rel = rel.to_string_lossy().replace('\\', "/");
                if super::files::skip_reason(&rel, ex).is_some() {
                    continue;
                }
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    stack.push(p);
                } else if ["xlsx", "xlsm", "xls"].contains(&super::files::ext(&p).as_str()) {
                    out.push((root.clone(), rel));
                }
            }
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

pub fn ensure_schema(c: &rusqlite::Connection) -> Result<()> {
    c.execute_batch(
        "CREATE TABLE IF NOT EXISTS facts(client TEXT NOT NULL, family TEXT, metric TEXT, grain TEXT, entity TEXT, month TEXT, value REAL, source_file TEXT);
         CREATE INDEX IF NOT EXISTS idx_facts ON facts(client, family, metric, grain, entity, month);
         CREATE TABLE IF NOT EXISTS fact_reports(client TEXT NOT NULL, family TEXT, metric TEXT, grain TEXT, entity TEXT, month TEXT, value REAL, source_file TEXT, reported_in TEXT, is_current INTEGER);
         CREATE INDEX IF NOT EXISTS idx_fact_reports ON fact_reports(client, family, metric, grain, entity, month);
         CREATE TABLE IF NOT EXISTS fact_versions(client TEXT NOT NULL, family TEXT, metric TEXT, grain TEXT, entity TEXT, month TEXT, value REAL, source_file TEXT, reported_in TEXT, is_current INTEGER);
         CREATE INDEX IF NOT EXISTS idx_fact_versions ON fact_versions(client, family, metric, grain, entity, month);
         CREATE TABLE IF NOT EXISTS fact_notes(client TEXT NOT NULL, family TEXT, source_file TEXT, sheet TEXT, cell TEXT, month TEXT, text TEXT);
         CREATE TABLE IF NOT EXISTS fact_merges(client TEXT NOT NULL, family TEXT, metric TEXT, grain TEXT, entity TEXT, month TEXT, source_file TEXT, policy TEXT, n_rows INTEGER, value REAL, inputs TEXT);",
    )?;
    Ok(())
}

/// Clients whose families the user accepted: the only ones whose numbers are served.
pub fn served() -> Vec<String> {
    clients().into_iter().filter(|c| load_config(c).is_some_and(|c| c.accepted())).collect()
}

/// Clients with facts or a config.
pub fn clients() -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(crate::paths::home().join("brain").join("numbers"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("families.json").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    out.sort();
    out
}

/// `list_metrics`: the catalog with what the facts hold for each (grains, months, entities).
pub fn list_metrics(client: Option<&str>) -> Result<Value> {
    let c = super::store::open(&super::store_path())?;
    ensure_schema(&c)?;
    let mut out = vec![];
    for cl in client.map(|c| vec![c.to_string()]).unwrap_or_else(served).into_iter().filter(|c| load_config(c).is_some_and(|c| c.accepted())) {
        for (name, m) in load_catalog(&cl) {
            let (fam, metric) = (m["family"].as_str().unwrap_or(""), m["metric"].as_str().unwrap_or(&name));
            let grains: Vec<Value> = c
                .prepare("SELECT grain, count(DISTINCT entity), min(month), max(month), count(*) FROM facts WHERE client = ?1 AND family = ?2 AND metric = ?3 GROUP BY grain")?
                .query_map(rusqlite::params![cl, fam, metric], |r| {
                    Ok(json!({"grain": r.get::<_, String>(0)?, "entities": r.get::<_, i64>(1)?, "months": [r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?], "facts": r.get::<_, i64>(4)?}))
                })?
                .collect::<Result<_, _>>()?;
            out.push(json!({"name": name, "client": cl, "family": fam, "metric": metric, "unit": m["unit"], "desc": m["desc"], "definition": m["definition"], "grains": grains}));
        }
    }
    Ok(json!({"metrics": out, "count": out.len()}))
}

/// `get_metric`: a governed metric's values (filters: grain, entity exact or like, months), each
/// with its source file; restated or conflicting figures say so with the other reported values,
/// merged rows with what they merged.
pub fn get_metric(a: &Value) -> Result<Value> {
    let name = a["name"].as_str().or(a["metric"].as_str()).ok_or_else(|| anyhow!("name: which metric (list_metrics names them)"))?;
    // Only accepted families answer: a proposal is never served as numbers.
    let clients: Vec<String> = a["client"].as_str().map(|c| vec![c.to_string()]).unwrap_or_else(served).into_iter().filter(|c| load_config(c).is_some_and(|c| c.accepted())).collect();
    let Some((client, m)) = clients.iter().find_map(|c| load_catalog(c).get(name).cloned().map(|m| (c.clone(), m))) else {
        return Ok(json!({"status": "not_modeled", "reason": format!("no governed metric {name:?}"), "guidance": "Call list_metrics; answer from search_knowledge and say the figure isn't modeled."}));
    };
    let (fam, metric) = (m["family"].as_str().unwrap_or("").to_string(), m["metric"].as_str().unwrap_or(name).to_string());
    let c = super::store::open(&super::store_path())?;
    ensure_schema(&c)?;
    let months: Vec<String> = a["months"].as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).or_else(|| a["month"].as_str().map(|m| vec![m.to_string()])).unwrap_or_default();
    let like = a["entity_like"].as_str().map(|e| format!("%{}%", e.to_lowercase()));
    // Every filter in the query (the month too), so the row cap never cuts the rows asked for.
    let month_list: Option<String> = (!months.is_empty()).then(|| format!(",{},", months.join(",")));
    let mut st = c.prepare(
        "SELECT grain, entity, month, value, source_file FROM facts WHERE client = ?1 AND family = ?2 AND metric = ?3
           AND (?4 IS NULL OR grain = ?4) AND (?5 IS NULL OR lower(entity) = lower(?5)) AND (?6 IS NULL OR lower(entity) LIKE ?6)
           AND (?7 IS NULL OR instr(?7, ',' || month || ',') > 0)
         ORDER BY grain, entity, month LIMIT 500",
    )?;
    let rows: Vec<(String, String, Option<String>, f64, String)> = st
        .query_map(rusqlite::params![client, fam, metric, a["grain"].as_str(), a["entity"].as_str(), like, month_list], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<_, _>>()?;
    let capped = rows.len() == 500;
    let mut out = vec![];
    for (grain, entity, month, value, src) in rows {
        let mut row = json!({"grain": grain, "entity": entity, "month": month, "value": value, "source_file": src});
        let others: Vec<Value> = c
            .prepare("SELECT value, source_file, reported_in FROM fact_versions WHERE client = ?1 AND family = ?2 AND metric = ?3 AND grain = ?4 AND entity = ?5 AND month IS ?6 AND is_current = 0")?
            .query_map(rusqlite::params![client, fam, metric, grain, entity, month], |r| Ok(json!({"value": r.get::<_, f64>(0)?, "source_file": r.get::<_, String>(1)?, "reported_in": r.get::<_, Option<String>>(2)?})))?
            .collect::<Result<_, _>>()?;
        if !others.is_empty() {
            row["other_reported_values"] = json!(others);
        }
        let merged: Option<(String, i64, String)> = c
            .query_row(
                "SELECT policy, n_rows, inputs FROM fact_merges WHERE client = ?1 AND family = ?2 AND metric = ?3 AND grain = ?4 AND entity = ?5 AND month IS ?6 AND source_file = ?7",
                rusqlite::params![client, fam, metric, grain, entity, month, src],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        if let Some((policy, n, inputs)) = merged {
            row["merged_from"] = json!({"policy": policy, "rows": n, "inputs": serde_json::from_str::<Value>(&inputs).unwrap_or(Value::Null)});
        }
        // Notes the report's author wrote beside its table ("tool outage: N contacts missing…").
        let caveats: Vec<Value> = c
            .prepare("SELECT sheet, cell, text FROM fact_notes WHERE client = ?1 AND family = ?2 AND source_file = ?3")?
            .query_map(rusqlite::params![client, fam, src], |r| Ok(json!({"sheet": r.get::<_, String>(0)?, "cell": r.get::<_, String>(1)?, "text": r.get::<_, String>(2)?})))?
            .collect::<Result<_, _>>()?;
        if !caveats.is_empty() {
            row["caveats"] = json!(caveats);
        }
        out.push(row);
    }
    // Restated (a later report changed it) or conflicting (reports of the same month disagree): from the build's audit.
    let audit: Value = std::fs::read(dir(&client).join("build_audit.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
    for r in out.iter_mut() {
        if let Some(x) = audit["restatements"].as_array().into_iter().flatten().find(|x| x["family"] == fam && x["metric"] == metric && x["grain"] == r["grain"] && x["entity"] == r["entity"] && x["month"] == r["month"]) {
            r[if x["kind"] == "restated" { "restated" } else { "conflicting" }] = json!(true);
        }
    }
    Ok(json!({"client": client, "metric": name, "unit": m["unit"], "desc": m["desc"], "definition": m["definition"], "rows": out, "count": out.len(), "truncated": capped,
              "guidance": "Quote values as given, with their source_file; say when a figure was restated or the sources conflict. No rows = not modeled for that filter."}))
}

/// `get_metric_history`: every report of one metric-month (each file that reported it).
pub fn get_metric_history(a: &Value) -> Result<Value> {
    let name = a["name"].as_str().or(a["metric"].as_str()).ok_or_else(|| anyhow!("name: which metric"))?;
    // Only accepted families answer: a proposal is never served as numbers.
    let clients: Vec<String> = a["client"].as_str().map(|c| vec![c.to_string()]).unwrap_or_else(served).into_iter().filter(|c| load_config(c).is_some_and(|c| c.accepted())).collect();
    let Some((client, m)) = clients.iter().find_map(|c| load_catalog(c).get(name).cloned().map(|m| (c.clone(), m))) else {
        return Ok(json!({"status": "not_modeled", "reason": format!("no governed metric {name:?}")}));
    };
    let c = super::store::open(&super::store_path())?;
    ensure_schema(&c)?;
    let rows: Vec<Value> = c
        .prepare(
            "SELECT grain, entity, month, value, source_file, reported_in, is_current FROM fact_reports WHERE client = ?1 AND family = ?2 AND metric = ?3
               AND (?4 IS NULL OR lower(entity) = lower(?4)) AND (?5 IS NULL OR month = ?5) AND (?6 IS NULL OR grain = ?6) ORDER BY grain, entity, month, reported_in LIMIT 300",
        )?
        .query_map(rusqlite::params![client, m["family"].as_str(), m["metric"].as_str(), a["entity"].as_str(), a["month"].as_str(), a["grain"].as_str()], |r| {
            Ok(json!({"grain": r.get::<_, String>(0)?, "entity": r.get::<_, String>(1)?, "month": r.get::<_, Option<String>>(2)?, "value": r.get::<_, f64>(3)?,
                      "source_file": r.get::<_, String>(4)?, "reported_in": r.get::<_, Option<String>>(5)?, "is_current": r.get::<_, i64>(6)? == 1}))
        })?
        .collect::<Result<_, _>>()?;
    Ok(json!({"client": client, "metric": name, "reports": rows, "count": rows.len()}))
}

/// What the window shows: per client, the config's state, families, the catalog and the last build.
pub fn overview() -> Result<Value> {
    let mut out = vec![];
    let mut all: Vec<String> = clients();
    for p in crate::projects::list() {
        if let Some(c) = p.client {
            if !roots(&c).is_empty() && !all.contains(&c) {
                all.push(c);
            }
        }
    }
    for cl in all {
        let cfg = load_config(&cl);
        let read = |f: &str| std::fs::read(dir(&cl).join(f)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or(Value::Null);
        out.push(json!({"client": cl, "config": cfg.as_ref().map(|c| serde_json::to_value(c).unwrap_or(Value::Null)), "status": cfg.as_ref().and_then(|c| c._echo.as_ref().map(|e| e["status"].clone())),
                        "catalog": load_catalog(&cl), "audit": read("build_audit.json")["summary"], "built_at": read("build_audit.json")["built_at"],
                        "units": read("build_audit.json")["audit"], "restatements": read("build_audit.json")["restatements"].as_array().map(|a| a.len()),
                        "coverage": read("coverage.json"), "profile": read("profile.json")["summary"], "workbooks": workbooks(&roots(&cl)).len()}));
    }
    Ok(json!({"clients": out}))
}

/// Read a JSON config from a file (a Brain `families.<corpus>.json`): checked before it is saved.
pub fn import_config(client: &str, path: &Path, accept: bool) -> Result<Config> {
    let mut c: Config = serde_json::from_slice(&std::fs::read(path).with_context(|| path.display().to_string())?).with_context(|| format!("{}: not a families config", path.display()))?;
    c.check()?;
    c._echo = Some(json!({"status": if accept { "accepted" } else { "proposed" }, "from": path.display().to_string(), "at": chrono::Utc::now().to_rfc3339()}));
    save_config(client, &c)?;
    Ok(c)
}

/// What a client's build reads: its workbooks (path, size, time), its config and catalog.
pub fn inputs(client: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for (root, rel) in workbooks(&roots(client)) {
        let md = std::fs::metadata(root.join(&rel)).ok();
        h.update(format!("{}\0{rel}\0{:?}\0{:?}\n", root.display(), md.as_ref().map(|m| m.len()), md.and_then(|m| m.modified().ok())).as_bytes());
    }
    for f in ["families.json", "metrics.json"] {
        h.update(std::fs::read(dir(client).join(f)).unwrap_or_default());
    }
    h.finalize().iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// After a Brain sync: every client with accepted families whose inputs changed is built again.
pub fn refresh() -> Vec<(String, Result<Value>)> {
    let mut out = vec![];
    for client in clients() {
        let Some(cfg) = load_config(&client).filter(Config::accepted) else { continue };
        let now = inputs(&client);
        let last = std::fs::read(dir(&client).join("build_audit.json")).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).and_then(|v| v["inputs"].as_str().map(str::to_owned));
        if last.as_deref() == Some(now.as_str()) {
            continue;
        }
        out.push((client.clone(), build::run(&client, &cfg, None)));
    }
    out
}

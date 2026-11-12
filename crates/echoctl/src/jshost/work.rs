//! Plugins' work rows (`uses.work`, `ctx.work.provide` / `changed`): items tracked outside Echo — a review asked of the
//! user, an issue assigned to them — on Today → My work and in a thread's Work tab, never in the ledger. Echo asks a
//! provider in the background (5 s), per scope (`me`, `thread:<id>`), keeps its last answer in the plugin's
//! `work.json` and draws from that, never waiting on the plugin; one that fails or is slow has its rows left out, and
//! says why (Settings → Plugins, `echoctl work show`).

use std::collections::BTreeSet;

use chrono::NaiveDate;

use super::*;

/// The file in a plugin's store (`ctx.store.write` refuses it).
pub const FILE: &str = "work.json";
/// Rows in one answer, at most.
const MAX_ROWS: usize = 200;
/// A scope's answer is asked for again once it is this old (seconds), or when the plugin says it changed.
const EVERY: i64 = 60;

static LOCK: Mutex<()> = Mutex::new(());
/// The asks on their way (`plugin|scope`): one at a time each.
static ASKING: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn file(id: &str) -> PathBuf {
    plugins::data_dir(id).join(FILE)
}

fn key(thread: Option<&str>) -> String {
    thread.map_or_else(|| "me".to_string(), |t| format!("thread:{t}"))
}

fn read(id: &str) -> serde_json::Map<String, Value> {
    std::fs::read(file(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// The enabled plugins that give work rows (`uses.work`).
fn givers() -> Vec<(String, plugins::Manifest)> {
    plugins::enabled_manifests().into_iter().filter(|(_, m)| m.uses.contains_key("work")).collect()
}

/// An answer checked and cut to what Echo draws: a list of at most 200 rows `{id, title, kind, where?, url?, due?, late?,
/// thread?}` — text only, `url` https, `due` a date (`late` also when it is past). A row Echo can't draw refuses the
/// answer, saying which and why.
pub fn check_rows(v: &Value, today: NaiveDate) -> Result<Vec<Value>> {
    let Some(all) = v.as_array() else {
        if v.is_null() {
            return Ok(vec![]);
        }
        bail!("its answer isn't a list of rows");
    };
    if all.len() > MAX_ROWS {
        bail!("its answer has {} rows: {MAX_ROWS} at most", all.len());
    }
    let mut ids = BTreeSet::new();
    all.iter()
        .enumerate()
        .map(|(i, r)| {
            let n = i + 1;
            let text = |k: &str, max: usize, needed: bool| -> Result<Option<String>> {
                match &r[k] {
                    Value::Null if !needed => Ok(None),
                    Value::String(s) if !s.trim().is_empty() && s.chars().count() <= max => Ok(Some(s.trim().to_string())),
                    _ => bail!("row {n}: “{k}” must be text of 1–{max} characters"),
                }
            };
            if !r.is_object() {
                bail!("row {n} isn't an object");
            }
            let id = text("id", 200, true)?.unwrap_or_default();
            if !ids.insert(id.clone()) {
                bail!("row {n}: id “{id}” twice");
            }
            let mut row = json!({"id": id, "title": text("title", 300, true)?, "kind": text("kind", 30, true)?});
            if let Some(w) = text("where", 120, false)? {
                row["where"] = json!(w);
            }
            if let Some(u) = text("url", 2000, false)? {
                if !u.starts_with("https://") || u.contains(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '\\')) {
                    bail!("row {n}: “url” must be an https:// address");
                }
                row["url"] = json!(u);
            }
            let due = match text("due", 10, false)? {
                Some(d) => Some(NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|_| anyhow!("row {n}: “due” must be a date, YYYY-MM-DD"))?),
                None => None,
            };
            if let Some(d) = due {
                row["due"] = json!(d.to_string());
            }
            let late = match &r["late"] {
                Value::Null => false,
                Value::Bool(b) => *b,
                _ => bail!("row {n}: “late” must be true or false"),
            };
            row["late"] = json!(late || due.is_some_and(|d| d < today));
            if let Some(t) = text("thread", 60, false)? {
                row["thread"] = json!(t);
            }
            Ok(row)
        })
        .collect()
}

/// A scope's answer kept: its rows, or why there are none (`rows` left empty: a plugin that fails shows nothing).
fn store(id: &str, scope: &str, rows: Result<Vec<Value>, String>) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read(id);
    let t = now();
    all.insert(
        scope.to_string(),
        match rows {
            Ok(rows) => json!({"rows": rows, "at": t, "asked": t}),
            Err(e) => json!({"rows": [], "error": e, "asked": t}),
        },
    );
    let write = || -> Result<()> {
        std::fs::create_dir_all(plugins::data_dir(id))?;
        paths::write_atomic(&file(id), serde_json::to_string_pretty(&all)?.as_bytes())?;
        Ok(())
    };
    if let Err(e) = write() {
        eprintln!("plugin {id}: its work rows: {e:#}");
    }
}

/// Ask a plugin's provider for a scope's rows and wait (5 s at most), keeping the answer — `echoctl work show`, where
/// its author checks it.
pub fn ask_now(id: &str, thread: Option<&str>) {
    let arg = json!({"scope": if thread.is_some() { "thread" } else { "me" }, "thread": thread});
    let rows = match call_within(id, "__work", &arg, Duration::from_secs(5)) {
        Ok(v) => check_rows(&v, chrono::Local::now().date_naive()).map_err(|e| format!("{e:#}")),
        // It gives none yet (no `provide`): nothing to show, nothing wrong.
        Err(e) if e.to_string().contains("no command “__work”") => Ok(vec![]),
        Err(e) => Err(format!("{e:#}")),
    };
    store(id, &key(thread), rows);
}

/// The same in the background, once at a time for a plugin's scope.
fn ask(id: &str, thread: Option<&str>) {
    let k = format!("{id}|{}", key(thread));
    if !ASKING.lock().unwrap_or_else(|e| e.into_inner()).insert(k.clone()) {
        return;
    }
    let (id, thread) = (id.to_string(), thread.map(str::to_owned));
    std::thread::spawn(move || {
        ask_now(&id, thread.as_deref());
        ASKING.lock().unwrap_or_else(|e| e.into_inner()).remove(&k);
    });
}

/// The plugin said its rows changed (`ctx.work.changed()`): each scope it answered is asked again, My work now.
pub fn changed(id: &str) {
    {
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = read(id);
        for v in all.values_mut() {
            if let Some(o) = v.as_object_mut() {
                o.remove("asked");
            }
        }
        if let Err(e) = paths::write_atomic(&file(id), serde_json::to_string_pretty(&all).unwrap_or_default().as_bytes()) {
            eprintln!("plugin {id}: its work rows: {e:#}");
        }
    }
    ask(id, None);
}

/// Every giving plugin's last rows for a scope, as drawn: each with its `source` (the plugin), `source_name` and `icon`
/// (its manifest's, served from its folder); late first, then by date. A scope's answer older than a minute (or one the
/// plugin said changed) is asked for again in the background — never waited for.
pub fn rows(thread: Option<&str>) -> Vec<Value> {
    let scope = key(thread);
    let mut out = vec![];
    for (id, m) in givers() {
        let last = read(&id).remove(&scope);
        let asked = last.as_ref().and_then(|l| l["asked"].as_str()).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
        if asked.is_none_or(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds() >= EVERY) {
            ask(&id, thread);
        }
        let icon = m.icon.as_deref().map(|i| format!("plugin/{id}/{}", i.trim_start_matches("./")));
        for mut r in last.and_then(|l| l["rows"].as_array().cloned()).unwrap_or_default() {
            r["source"] = json!(id);
            r["source_name"] = json!(m.name);
            r["icon"] = json!(icon);
            out.push(r);
        }
    }
    sort(&mut out);
    out
}

fn sort(rows: &mut [Value]) {
    rows.sort_by_key(|r| (r["late"] != true, r["due"].as_str().unwrap_or("9999").to_string()));
}

/// A thread's rows: what the plugins give for it (`scope: "thread"`) and the user's own that name it, each once.
pub fn of_thread(thread: &str) -> Vec<Value> {
    let mut out = rows(Some(thread));
    for r in rows(None).into_iter().filter(|r| r["thread"] == thread) {
        if !out.iter().any(|o| o["source"] == r["source"] && o["id"] == r["id"]) {
            out.push(r);
        }
    }
    sort(&mut out);
    out
}

/// Why a plugin's rows were left out the last time it was asked (any scope; none: it answered).
pub fn error(id: &str) -> Option<String> {
    read(id).into_values().find_map(|v| v["error"].as_str().map(str::to_owned))
}

/// `echoctl work show`: every giving plugin asked now for a scope (waited for, 5 s each), then its rows and errors.
pub fn show(thread: Option<&str>) -> Value {
    let scope = key(thread);
    let mut errors = serde_json::Map::new();
    for (id, _) in givers() {
        ask_now(&id, thread);
        if let Some(e) = read(&id).get(&scope).and_then(|l| l["error"].as_str()) {
            errors.insert(id, json!(e));
        }
    }
    json!({"scope": if thread.is_some() { "thread" } else { "me" }, "thread": thread, "rows": thread.map_or_else(|| rows(None), of_thread), "errors": errors})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugins_rows_are_checked_and_cut_to_what_echo_draws() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
        let rows = check_rows(
            &json!([
                {"id": "pr-1", "title": "Review: fix login", "kind": "review", "where": "acme-test/api", "url": "https://git.acme.example/pr/1", "due": "2026-11-10", "extra": "<b>dropped</b>"},
                {"id": "is-2", "title": "<img src=x>", "kind": "assigned", "late": true, "thread": "alpha"},
            ]),
            today,
        )
        .unwrap();
        assert_eq!(rows[0], json!({"id": "pr-1", "title": "Review: fix login", "kind": "review", "where": "acme-test/api", "url": "https://git.acme.example/pr/1", "due": "2026-11-10", "late": true}), "past due: late; unknown fields dropped");
        assert_eq!(rows[1], json!({"id": "is-2", "title": "<img src=x>", "kind": "assigned", "late": true, "thread": "alpha"}), "text kept as text (the page escapes it)");
        assert_eq!(check_rows(&Value::Null, today).unwrap(), Vec::<Value>::new());
        let refused = |v: Value, why: &str| assert!(check_rows(&v, today).unwrap_err().to_string().contains(why), "{v}: {why}");
        refused(json!({"id": 1}), "isn't a list");
        refused(json!([{"id": "a", "title": "t"}]), "row 1: “kind”");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "url": "http://acme.example"}]), "https");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "url": "javascript:alert(1)"}]), "https");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "due": "12/11/2026"}]), "YYYY-MM-DD");
        refused(json!([{"id": "a", "title": "t", "kind": "PR"}, {"id": "a", "title": "u", "kind": "PR"}]), "twice");
        refused(json!((0..201).map(|i| json!({"id": i.to_string(), "title": "t", "kind": "PR"})).collect::<Vec<_>>()), "200 at most");
    }
}

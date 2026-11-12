//! Plugins' work rows (`uses.work`, `ctx.work.provide` / `changed`): items tracked outside Echo — a review asked of the
//! user, an issue assigned to them — on Today → My work and in a thread's Work tab, never in the ledger. Echo asks a
//! provider in the background (5 s), per scope (`me`, `thread:<id>`), keeps its last answer in the plugin's
//! `work.json` and draws from that, never waiting on the plugin; one that fails or is slow keeps its last rows, marked
//! not updated, and says why (Settings → Plugins, `echoctl work show`). A scope not read for 30 days is dropped, a
//! thread's when it is deleted or merged.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use chrono::NaiveDate;

use super::*;

/// The file in a plugin's store (`ctx.store.write` refuses it).
pub const FILE: &str = "work.json";
/// Rows in one answer, at most.
const MAX_ROWS: usize = 200;
/// A scope's answer is asked for again once it is this old (seconds), or when the plugin says it changed.
const EVERY: i64 = 60;
/// A thread's scope not asked for in this many days is dropped from `work.json` (a thread nobody opens any more).
const KEEP_DAYS: i64 = 30;

static LOCK: Mutex<()> = Mutex::new(());
/// The asks on their way (`plugin|scope`), one at a time each: `true` when the plugin said its rows changed while it
/// was on its way (asked again as soon as it ends: what the first answer gives may predate the change).
static ASKING: Mutex<BTreeMap<String, bool>> = Mutex::new(BTreeMap::new());

fn file(id: &str) -> PathBuf {
    plugins::data_dir(id).join(FILE)
}

fn key(thread: Option<&str>) -> String {
    thread.map_or_else(|| "me".to_string(), |t| format!("thread:{t}"))
}

fn read(id: &str) -> serde_json::Map<String, Value> {
    std::fs::read(file(id)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn write(id: &str, all: &serde_json::Map<String, Value>) {
    let w = || -> Result<()> {
        std::fs::create_dir_all(plugins::data_dir(id))?;
        paths::write_atomic(&file(id), serde_json::to_string_pretty(all)?.as_bytes())?;
        Ok(())
    };
    if let Err(e) = w() {
        eprintln!("plugin {id}: its work rows: {e:#}");
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// How long ago (seconds) a scope was last asked; none: never.
fn age(last: &Value) -> Option<i64> {
    last["asked"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds())
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

/// A scope's answer kept: its rows, or why there are none — the last good rows then kept, marked by the `error` (a
/// plugin that fails for a while still shows what it gave). `changed`: the plugin said its rows changed while this ask
/// was on its way (still waited for). Scopes of threads not asked for in 30 days go.
fn store(id: &str, scope: &str, rows: Result<Vec<Value>, String>, changed: bool) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = read(id);
    let t = now();
    let prev = all.remove(scope);
    let mut entry = match rows {
        Ok(rows) => json!({"rows": rows, "at": t, "asked": t}),
        Err(e) => {
            let mut v = json!({"rows": prev.as_ref().map_or(json!([]), |p| p["rows"].clone()), "error": e, "asked": t});
            if let Some(at) = prev.as_ref().and_then(|p| p["at"].as_str()) {
                v["at"] = json!(at);
            }
            v
        }
    };
    if changed {
        entry["changed"] = json!(true);
    }
    all.insert(scope.to_string(), entry);
    all.retain(|k, v| k == "me" || age(v).is_some_and(|a| a < KEEP_DAYS * 86_400));
    write(id, &all);
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
    let again = ASKING.lock().unwrap_or_else(|e| e.into_inner()).get(&format!("{id}|{}", key(thread))).copied().unwrap_or(false);
    store(id, &key(thread), rows, again);
}

/// The same in the background, once at a time for a plugin's scope; `again` (the plugin said its rows changed): one
/// on its way is asked again when it ends.
fn ask(id: &str, thread: Option<&str>, again: bool) {
    let k = format!("{id}|{}", key(thread));
    {
        let mut asking = ASKING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(d) = asking.get_mut(&k) {
            *d |= again;
            return;
        }
        asking.insert(k.clone(), false);
    }
    let (id, thread) = (id.to_string(), thread.map(str::to_owned));
    std::thread::spawn(move || loop {
        ask_now(&id, thread.as_deref());
        let mut asking = ASKING.lock().unwrap_or_else(|e| e.into_inner());
        if asking.get(&k) == Some(&true) {
            asking.insert(k.clone(), false);
            continue;
        }
        asking.remove(&k);
        break;
    });
}

/// The plugin said its rows changed (`ctx.work.changed()`): each scope it answered is asked again (shown as waited
/// for until then), My work now.
pub fn changed(id: &str) {
    {
        let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = read(id);
        for v in all.values_mut() {
            if let Some(o) = v.as_object_mut() {
                o.insert("changed".into(), json!(true));
            }
        }
        write(id, &all);
    }
    ask(id, None, true);
}

/// A thread was deleted or merged away: every plugin's rows for it go.
pub fn forget_thread(thread: &str) {
    let _l = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let scope = key(Some(thread));
    for id in std::fs::read_dir(paths::state_dir().join("plugins")).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()) {
        let mut all = read(&id);
        if all.remove(&scope).is_some() {
            write(&id, &all);
        }
    }
}

/// Every giving plugin's last rows for a scope, as drawn: each with its `source` (the plugin), `source_name` and `icon`
/// (its manifest's, served from its folder), and `stale` (why it wasn't updated) when its last ask failed; late first,
/// then by date. A scope's answer older than a minute (or one the plugin said changed) is asked for again in the
/// background — never waited for. `pending`: a plugin hasn't answered it yet, or said it changed and is being asked
/// (the page reads it again soon).
pub fn view(thread: Option<&str>) -> (Vec<Value>, bool) {
    let scope = key(thread);
    let (mut out, mut pending) = (vec![], false);
    for (id, m) in givers() {
        let last = read(&id).remove(&scope);
        let changed = last.as_ref().is_some_and(|l| l["changed"] == true);
        pending |= last.is_none() || changed;
        if changed || last.as_ref().and_then(age).is_none_or(|a| a >= EVERY) {
            ask(&id, thread, false);
        }
        let icon = m.icon.as_deref().map(|i| format!("plugin/{id}/{}", i.trim_start_matches("./")));
        let stale = last.as_ref().and_then(|l| l["error"].as_str().map(str::to_owned));
        for mut r in last.and_then(|l| l["rows"].as_array().cloned()).unwrap_or_default() {
            r["source"] = json!(id);
            r["source_name"] = json!(m.name);
            r["icon"] = json!(icon);
            if let Some(e) = &stale {
                r["stale"] = json!(e);
            }
            out.push(r);
        }
    }
    sort(&mut out);
    (out, pending)
}

/// The rows of `view`, without whether one is waited for.
pub fn rows(thread: Option<&str>) -> Vec<Value> {
    view(thread).0
}

fn sort(rows: &mut [Value]) {
    rows.sort_by_key(|r| (r["late"] != true, r["due"].as_str().unwrap_or("9999").to_string()));
}

/// A thread's rows: what the plugins give for it (`scope: "thread"`) and the user's own that name it, each once; and
/// whether one is waited for.
pub fn thread_view(thread: &str) -> (Vec<Value>, bool) {
    let (mut out, a) = view(Some(thread));
    let (mine, b) = view(None);
    for r in mine.into_iter().filter(|r| r["thread"] == thread) {
        if !out.iter().any(|o| o["source"] == r["source"] && o["id"] == r["id"]) {
            out.push(r);
        }
    }
    sort(&mut out);
    (out, a || b)
}

/// The rows of `thread_view`.
pub fn of_thread(thread: &str) -> Vec<Value> {
    thread_view(thread).0
}

/// Why a plugin's rows weren't updated the last time it was asked, and where: My work's first, else a thread's
/// (named); none: it answered.
pub fn error(id: &str) -> Option<(String, String)> {
    let all = read(id);
    if let Some(e) = all.get("me").and_then(|v| v["error"].as_str()) {
        return Some(("My work".into(), e.to_owned()));
    }
    all.iter().find_map(|(k, v)| {
        let t = k.strip_prefix("thread:")?;
        let e = v["error"].as_str()?;
        let name = crate::projects::load(t).map(|p| p.name).unwrap_or_else(|_| t.to_string());
        Some((format!("thread {name}'s Work"), e.to_owned()))
    })
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

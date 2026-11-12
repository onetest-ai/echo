//! Plugins' work rows (`uses.work`, `ctx.work.provide` / `changed`): items tracked outside Echo — a review asked of the
//! user, an issue assigned to them — on Today → My work and in a thread's Work tab, never in the ledger. Echo asks a
//! provider in the background (5 s), per scope (`me`, `thread:<id>`), keeps its last answer in the plugin's
//! `work.json` and draws from that, never waiting on the plugin; one that fails or is slow keeps its last rows, marked
//! not updated, and says why (Settings → Plugins, `echoctl work show`) — for a day after its last good answer, then
//! none. A scope is asked at most every 10 s (`changed()` said more often waits for the next turn; said from inside
//! the provider itself, it is not heard). A scope not read for 30 days is dropped, a thread's when it is deleted or
//! merged. `late` is never kept: it is the plugin's own flag, or its `due` past, on the day it is drawn.
//!
//! Every change of `work.json` is read-change-write under the plugin's lock across processes
//! (`.locks/<id>.work.lock` beside the stores): the app's asks and `echoctl project delete` never undo each other.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::time::Instant;

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
/// A scope is asked at most this often: a plugin that says `changed()` all the time (a timer) is asked once a turn.
#[cfg(not(test))]
const GAP: Duration = Duration::from_secs(10);
#[cfg(test)]
const GAP: Duration = Duration::from_secs(1);
/// The last good rows are kept after a failed ask for this long (hours) after that answer, then dropped.
const KEEP_STALE_HOURS: i64 = 24;

/// A scope's asks (`plugin|scope`): one on its way at a time (`on_way`, a wait for its turn included), when the last
/// started, and `again`: the plugin said its rows changed while it was on its way (asked again in its next turn: what
/// the answer gives may predate the change). Read and cleared by `store` and set by `changed`, each under the file's
/// lock: a change is never lost between an answer and its keeping.
#[derive(Default)]
struct Asking {
    on_way: bool,
    again: bool,
    started: Option<Instant>,
}

static ASKING: Mutex<BTreeMap<String, Asking>> = Mutex::new(BTreeMap::new());

fn asking() -> std::sync::MutexGuard<'static, BTreeMap<String, Asking>> {
    ASKING.lock().unwrap_or_else(|e| e.into_inner())
}

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

/// The plugin's `work.json` changed in one go, under its lock across processes, read again inside it; `f` says
/// whether there is anything to write.
fn update(id: &str, f: impl FnOnce(&mut serde_json::Map<String, Value>) -> bool) {
    let locked = (|| -> Result<crate::app::FileLock> {
        let dir = paths::state_dir().join("plugins").join(".locks");
        std::fs::create_dir_all(&dir)?;
        plugins::wait_lock(&dir.join(format!("{id}.work.lock")))
    })();
    let _l = match locked {
        Ok(l) => l,
        Err(e) => return eprintln!("plugin {id}: its work rows: {e:#}"),
    };
    let mut all = read(id);
    if f(&mut all) {
        write(id, &all);
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// How long ago (seconds) a time a scope keeps (`asked`, `at`) was; none: there is none.
fn since(t: &Value) -> Option<i64> {
    t.as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|t| (chrono::Utc::now() - t.with_timezone(&chrono::Utc)).num_seconds())
}

/// How long ago (seconds) a scope was last asked; none: never.
fn age(last: &Value) -> Option<i64> {
    since(&last["asked"])
}

/// Its last good answer (`at`) is recent enough for its rows to be kept after a failed ask.
fn keepable(last: &Value) -> bool {
    since(&last["at"]).is_some_and(|s| s < KEEP_STALE_HOURS * 3600)
}

/// The enabled plugins that give work rows (`uses.work`).
fn givers() -> Vec<(String, plugins::Manifest)> {
    plugins::enabled_manifests().into_iter().filter(|(_, m)| m.uses.contains_key("work")).collect()
}

/// An answer checked and cut to what Echo draws: a list of at most 200 rows `{id, title, kind, where?, url?, due?, late?,
/// thread?}` — text only, `url` https, `due` a date, `late` the plugin's own flag (past due is worked out when drawn:
/// `late`). A row Echo can't draw refuses the answer, saying which and why.
pub fn check_rows(v: &Value) -> Result<Vec<Value>> {
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
            if late {
                row["late"] = json!(true);
            }
            if let Some(t) = text("thread", 60, false)? {
                row["thread"] = json!(t);
            }
            Ok(row)
        })
        .collect()
}

/// A scope's answer kept: its rows, or why there are none — the last good rows then kept, marked by the `error` (a
/// plugin that fails for a while still shows what it gave), for a day after they came. Under the file's lock, whether
/// the plugin said its rows changed while this ask was on its way is read and cleared: `true`, the scope is marked
/// still waited for, and asked again. Scopes of threads not asked for in 30 days go.
fn store(id: &str, scope: &str, rows: Result<Vec<Value>, String>) -> bool {
    let k = format!("{id}|{scope}");
    let mut again = false;
    update(id, |all| {
        again = asking().get_mut(&k).is_some_and(|a| std::mem::take(&mut a.again));
        let t = now();
        let prev = all.remove(scope).filter(keepable);
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
        if again {
            entry["changed"] = json!(true);
        }
        all.insert(scope.to_string(), entry);
        all.retain(|k, v| k == "me" || age(v).is_some_and(|a| a < KEEP_DAYS * 86_400));
        true
    });
    again
}

/// Ask a plugin's provider for a scope's rows and wait (5 s at most), keeping the answer — `echoctl work show`, where
/// its author checks it; `true`: it said its rows changed meanwhile (to be asked again).
pub fn ask_now(id: &str, thread: Option<&str>) -> bool {
    let arg = json!({"scope": if thread.is_some() { "thread" } else { "me" }, "thread": thread});
    let rows = match call_within(id, "__work", &arg, Duration::from_secs(5)) {
        Ok(v) => check_rows(&v).map_err(|e| format!("{e:#}")),
        // It gives none yet (no `provide`): nothing to show, nothing wrong.
        Err(e) if e.to_string().contains("no command “__work”") => Ok(vec![]),
        Err(e) => Err(format!("{e:#}")),
    };
    store(id, &key(thread), rows)
}

/// The same in the background, once at a time for a plugin's scope and never sooner than 10 s after its last ask (one
/// asked for sooner waits for its turn); `again` (the plugin said its rows changed): one on its way is asked again in
/// its next turn.
fn ask(id: &str, thread: Option<&str>, again: bool) {
    let k = format!("{id}|{}", key(thread));
    {
        let mut all = asking();
        all.retain(|_, a| a.on_way || a.started.is_some_and(|t| t.elapsed() < GAP));
        let a = all.entry(k.clone()).or_default();
        if a.on_way {
            a.again |= again;
            return;
        }
        a.on_way = true;
    }
    let (id, thread) = (id.to_string(), thread.map(str::to_owned));
    std::thread::spawn(move || loop {
        let wait = asking().get(&k).and_then(|a| a.started).map_or(Duration::ZERO, |t| GAP.saturating_sub(t.elapsed()));
        std::thread::sleep(wait);
        {
            // What it said changed until now, this ask answers.
            let mut all = asking();
            let a = all.entry(k.clone()).or_default();
            a.started = Some(Instant::now());
            a.again = false;
        }
        let again = ask_now(&id, thread.as_deref());
        let mut all = asking();
        let a = all.entry(k.clone()).or_default();
        if again || std::mem::take(&mut a.again) {
            continue;
        }
        a.on_way = false;
        break;
    });
}

/// The plugin said its rows changed (`ctx.work.changed()`; not heard from inside its provider, `jshost::host_op`):
/// each scope it answered is marked waited for and asked again — one on its way in its next turn, My work now (or
/// when its turn comes), a thread's when it is read.
pub fn changed(id: &str) {
    mark_changed(id);
    ask(id, None, true);
}

/// `changed`'s marks: the scopes on their way asked again, each kept scope waited for (written only when one wasn't).
fn mark_changed(id: &str) {
    update(id, |all| {
        let mine = format!("{id}|");
        for (_, a) in asking().iter_mut().filter(|(k, a)| k.starts_with(&mine) && a.on_way) {
            a.again = true;
        }
        let mut marked = false;
        for v in all.values_mut() {
            if let Some(o) = v.as_object_mut().filter(|o| o.get("changed") != Some(&json!(true))) {
                o.insert("changed".into(), json!(true));
                marked = true;
            }
        }
        marked
    });
}

/// A thread was deleted or merged away: every plugin's rows for it go (each plugin's folder, under its lock).
pub fn forget_thread(thread: &str) {
    let scope = key(Some(thread));
    let ids = std::fs::read_dir(paths::state_dir().join("plugins"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.starts_with('.'));
    for id in ids {
        if file(&id).exists() {
            update(&id, |all| all.remove(&scope).is_some());
        }
    }
}

/// Late on `today`: the plugin says so, or its `due` is past.
fn late(r: &Value, today: NaiveDate) -> bool {
    r["late"] == true || r["due"].as_str().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok()).is_some_and(|d| d < today)
}

/// A scope's kept answer as drawn on `today`: its rows, `late` worked out now, `stale` (why) when its last ask
/// failed — none once its last good answer is a day old.
fn drawn(last: &Value, today: NaiveDate) -> Vec<Value> {
    let stale = last["error"].as_str();
    if stale.is_some() && !keepable(last) {
        return vec![];
    }
    let mut rows = last["rows"].as_array().cloned().unwrap_or_default();
    for r in &mut rows {
        r["late"] = json!(late(r, today));
        if let Some(e) = stale {
            r["stale"] = json!(e);
        }
    }
    rows
}

/// Every giving plugin's last rows for a scope, as drawn: each with its `source` (the plugin), `source_name` and `icon`
/// (its manifest's, served from its folder), `late` as of today, and `stale` (why it wasn't updated) when its last ask
/// failed; late first, then by date, undated last. A scope's answer older than a minute (or one the plugin said
/// changed) is asked for again in the background — never waited for. `pending`: a plugin hasn't answered it yet, or
/// said it changed and is being asked (the page reads it again soon).
pub fn view(thread: Option<&str>) -> (Vec<Value>, bool) {
    let scope = key(thread);
    let today = chrono::Local::now().date_naive();
    let (mut out, mut pending) = (vec![], false);
    for (id, m) in givers() {
        let last = read(&id).remove(&scope);
        let changed = last.as_ref().is_some_and(|l| l["changed"] == true);
        pending |= last.is_none() || changed;
        if changed || last.as_ref().and_then(age).is_none_or(|a| a >= EVERY) {
            ask(&id, thread, false);
        }
        let icon = m.icon.as_deref().map(|i| format!("plugin/{id}/{}", i.trim_start_matches("./")));
        for mut r in last.as_ref().map(|l| drawn(l, today)).unwrap_or_default() {
            r["source"] = json!(id);
            r["source_name"] = json!(m.name);
            r["icon"] = json!(icon);
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
        let rows = check_rows(&json!([
            {"id": "pr-1", "title": "Review: fix login", "kind": "review", "where": "acme-test/api", "url": "https://git.acme.example/pr/1", "due": "2026-11-10", "extra": "<b>dropped</b>"},
            {"id": "is-2", "title": "<img src=x>", "kind": "assigned", "late": true, "thread": "alpha"},
        ]))
        .unwrap();
        assert_eq!(rows[0], json!({"id": "pr-1", "title": "Review: fix login", "kind": "review", "where": "acme-test/api", "url": "https://git.acme.example/pr/1", "due": "2026-11-10"}), "unknown fields dropped; late not kept (worked out when drawn)");
        assert_eq!(rows[1], json!({"id": "is-2", "title": "<img src=x>", "kind": "assigned", "late": true, "thread": "alpha"}), "text kept as text (the page escapes it); its own late kept");
        assert_eq!(check_rows(&Value::Null).unwrap(), Vec::<Value>::new());
        let refused = |v: Value, why: &str| assert!(check_rows(&v).unwrap_err().to_string().contains(why), "{v}: {why}");
        refused(json!({"id": 1}), "isn't a list");
        refused(json!([{"id": "a", "title": "t"}]), "row 1: “kind”");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "url": "http://acme.example"}]), "https");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "url": "javascript:alert(1)"}]), "https");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "due": "12/11/2026"}]), "YYYY-MM-DD");
        refused(json!([{"id": "a", "title": "t", "kind": "PR", "late": "yes"}]), "true or false");
        refused(json!([{"id": "a", "title": "t", "kind": "PR"}, {"id": "a", "title": "u", "kind": "PR"}]), "twice");
        refused(json!((0..201).map(|i| json!({"id": i.to_string(), "title": "t", "kind": "PR"})).collect::<Vec<_>>()), "200 at most");
    }

    fn ago(hours: i64) -> String {
        (chrono::Utc::now() - chrono::Duration::hours(hours)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    }

    #[test]
    fn late_is_worked_out_on_the_day_drawn_and_kept_rows_go_a_day_after_their_last_good_answer() {
        let rows = json!([{"id": "a", "due": "2026-11-12"}, {"id": "b", "due": "2026-11-13"}, {"id": "c", "late": true}, {"id": "d"}]);
        let late = |l: &Value, day: u32| drawn(l, NaiveDate::from_ymd_opt(2026, 11, day).unwrap()).iter().filter(|r| r["late"] == true).map(|r| r["id"].as_str().unwrap().to_owned()).collect::<Vec<_>>();
        let good = json!({"rows": rows, "at": ago(48), "asked": ago(48)});
        assert_eq!(late(&good, 12), ["c"], "due today: not late yet");
        assert_eq!(late(&good, 13), ["a", "c"], "a day on, the same kept answer: late");
        assert_eq!(late(&good, 20), ["a", "b", "c"]);
        assert_eq!(drawn(&good, NaiveDate::from_ymd_opt(2026, 11, 12).unwrap()).len(), 4, "a good answer stays however old (it is asked again)");
        let failing = json!({"rows": rows, "at": ago(23), "asked": ago(0), "error": "GitHub is down"});
        let kept = drawn(&failing, NaiveDate::from_ymd_opt(2026, 11, 13).unwrap());
        assert_eq!(kept.len(), 4, "kept within a day of its last good answer");
        assert!(kept.iter().all(|r| r["stale"] == "GitHub is down") && kept[0]["late"] == true, "{kept:?}");
        assert!(drawn(&json!({"rows": rows, "at": ago(25), "asked": ago(0), "error": "down"}), NaiveDate::from_ymd_opt(2026, 11, 13).unwrap()).is_empty(), "a day on: none");
        assert!(drawn(&json!({"rows": [], "asked": ago(0), "error": "down"}), NaiveDate::from_ymd_opt(2026, 11, 13).unwrap()).is_empty());
    }

    #[test]
    fn a_change_said_while_an_ask_is_on_its_way_is_never_lost_and_stale_rows_are_not_kept_past_a_day() {
        let (_env, _) = paths::testenv::folders("work-store");
        let id = "work-store-demo";
        let row = || json!([{"id": "a", "title": "A", "kind": "PR"}]);
        // Said changed between the answer and its keeping: kept as waited for, and asked again.
        asking().insert(format!("{id}|me"), Asking { on_way: true, again: false, started: Some(Instant::now()) });
        mark_changed(id);
        assert!(store(id, "me", Ok(vec![row()[0].clone()])), "asked again");
        assert_eq!(read(id)["me"]["changed"], true, "still waited for");
        assert!(!store(id, "me", Ok(vec![])), "read and cleared: the next answer is the change's");
        assert_eq!(read(id)["me"]["changed"], Value::Null);
        // A scope not on its way: only marked (asked when read), its rows kept.
        update(id, |all| {
            all.insert("thread:alpha".into(), json!({"rows": row(), "at": ago(25), "asked": ago(25)}));
            true
        });
        mark_changed(id);
        assert_eq!(read(id)["thread:alpha"]["changed"], true);
        // It fails a day after its last good answer: nothing kept.
        assert!(!store(id, "thread:alpha", Err("down".into())));
        assert_eq!(read(id)["thread:alpha"]["rows"], json!([]));
        assert_eq!(read(id)["thread:alpha"]["at"], Value::Null);
        asking().remove(&format!("{id}|me"));
    }

    #[test]
    fn a_deleted_threads_rows_go_from_each_plugins_folder_and_nothing_else() {
        let (_env, _) = paths::testenv::folders("work-forget");
        for id in ["gh", "jira"] {
            update(id, |all| {
                all.insert("me".into(), json!({"rows": []}));
                all.insert("thread:alpha".into(), json!({"rows": []}));
                true
            });
        }
        let plugins = paths::state_dir().join("plugins");
        std::fs::create_dir_all(plugins.join(".locks")).unwrap();
        std::fs::write(plugins.join("stray.json"), "{}").unwrap();
        forget_thread("alpha");
        for id in ["gh", "jira"] {
            assert_eq!(read(id).keys().collect::<Vec<_>>(), ["me"], "{id}");
        }
        assert!(!plugins.join(".locks").join(FILE).exists() && !plugins.join("stray.json").join(FILE).exists(), "only plugin folders");
        assert!(plugins.join(".locks/gh.work.lock").exists(), "changed under the plugin's lock");
    }
}

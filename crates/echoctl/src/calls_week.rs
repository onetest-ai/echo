//! Recorded this week (Today's Calls widget `core:calls.week`, #315): the calls recorded since Monday — memos counted,
//! as the Calls list shows them — the time recorded, how many have minutes, and the calls of each day. Read from the
//! calls' folders; no model call.

use std::path::Path;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate};
use serde_json::{json, Value};

use crate::calls;

/// The local day a call was recorded: its folder's stamp (the local wall clock it started at, as Your day reads it);
/// a folder without one, its `started_at` in local time (its meta read only then, into `meta`: an old call is never read).
fn day_of(dir: &Path, meta: &mut Option<Value>) -> Option<NaiveDate> {
    let name = dir.file_name()?.to_string_lossy();
    if calls::has_stamp(&name) {
        return NaiveDate::parse_from_str(&name[..10], "%Y-%m-%d").ok();
    }
    let meta = meta.get_or_insert_with(|| calls::read_json(&dir.join("meta.json")));
    meta["started_at"].as_str().and_then(|s| DateTime::parse_from_rfc3339(s).ok()).map(|t| t.with_timezone(&Local).date_naive())
}

/// How long a call recorded, in seconds: `ended_at − started_at` (none while it records, or without its times).
fn seconds(meta: &Value) -> i64 {
    let t = |k: &str| meta[k].as_str().and_then(|v| DateTime::parse_from_rfc3339(v).ok());
    t("started_at").zip(t("ended_at")).map_or(0, |(s, e)| (e - s).num_seconds().max(0))
}

/// The week of `today` since its Monday: `calls` (memos among them, also counted in `memos`), `recorded_min` (their
/// time, rounded to the minute), `with_minutes` (those with minutes, `notes.md`), and `days`: Monday to Sunday, each
/// day's calls (the days after today 0). `scope`: only the calls of these threads (a widget set to a thread or a client).
pub fn view(today: NaiveDate, scope: Option<&[String]>) -> Value {
    let from = today - Duration::days(i64::from(today.weekday().num_days_from_monday()));
    let mut days = [0u64; 7];
    let (mut n, mut memos, mut with_minutes, mut secs) = (0u64, 0u64, 0u64, 0i64);
    for dir in calls::all_calls() {
        let mut meta = None;
        let Some(day) = day_of(&dir, &mut meta).filter(|d| *d >= from && *d <= today) else { continue };
        if let Some(s) = scope {
            if !crate::projects::of_call(&dir).unwrap_or_default().iter().any(|(p, _)| s.contains(&p.id)) {
                continue;
            }
        }
        let meta = meta.unwrap_or_else(|| calls::read_json(&dir.join("meta.json")));
        days[(day - from).num_days() as usize] += 1;
        n += 1;
        memos += u64::from(meta["kind"] == "memo");
        with_minutes += u64::from(dir.join("notes.md").exists());
        secs += seconds(&meta);
    }
    let days: Vec<Value> = days.iter().enumerate().map(|(i, c)| json!({"date": (from + Duration::days(i as i64)).to_string(), "calls": c})).collect();
    json!({"since": from.to_string(), "today": today.to_string(), "calls": n, "memos": memos, "recorded_min": (secs + 30) / 60, "with_minutes": with_minutes, "days": days})
}

/// Today's view (`view` for the local date), read again at most every 30 s: Today asks for it on each refresh.
pub fn today(scope: Option<&[String]>) -> Value {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};
    static LAST: Mutex<Vec<(String, Instant, Value)>> = Mutex::new(Vec::new());
    let key = scope.map(|s| s.join(",")).unwrap_or_else(|| "*".into());
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, _, v)) = last.iter().find(|(k, at, _)| *k == key && at.elapsed() < Duration::from_secs(30)) {
        return v.clone();
    }
    let v = view(Local::now().date_naive(), scope);
    last.retain(|(k, at, _)| *k != key && at.elapsed() < Duration::from_secs(30));
    last.push((key, Instant::now(), v.clone()));
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// A call folder: its name, its times (local), whether it is a memo and has minutes.
    fn call(name: &str, start: DateTime<Local>, minutes: i64, memo: bool, notes: bool) {
        let dir = crate::paths::calls_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let mut meta = json!({"app": "teams", "title": "Acme sync", "started_at": start.to_rfc3339(), "ended_at": (start + Duration::minutes(minutes)).to_rfc3339()});
        if memo {
            meta["kind"] = json!("memo");
        }
        std::fs::write(dir.join("meta.json"), meta.to_string()).unwrap();
        if notes {
            std::fs::write(dir.join("notes.md"), "# Acme sync\n").unwrap();
        }
    }

    fn local(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).single().unwrap()
    }

    #[test]
    fn the_week_counts_calls_memos_time_and_minutes_since_monday_in_local_time() {
        let (_env, _) = crate::paths::testenv::folders("calls-week");
        // Sunday 8 Nov 2026 late at night: last week. Monday 9 Nov early: this week.
        call("2026-11-08_233000-acme-late", local(2026, 11, 8, 23, 30), 20, false, true);
        call("2026-11-09_073000-acme-early", local(2026, 11, 9, 7, 30), 45, false, true);
        call("2026-11-10_100000-globex-review", local(2026, 11, 10, 10, 0), 50, false, false);
        call("2026-11-10_120000-memo", local(2026, 11, 10, 12, 0), 4, true, true);
        // A folder without a stamp: its day is its start in local time — Sunday night here stays Sunday, last week,
        // whatever date it is in UTC.
        let late = local(2026, 11, 8, 23, 50);
        call("old-dictation", late, 10, false, false);
        let utc = json!({"started_at": late.with_timezone(&chrono::Utc).to_rfc3339()});
        assert_eq!(day_of(Path::new("old-dictation"), &mut Some(utc.clone())), Some(NaiveDate::from_ymd_opt(2026, 11, 8).unwrap()), "{utc}");
        // One still recording (no end): counted, no time.
        let rec = crate::paths::calls_dir().join("2026-11-10_150000-initech-planning");
        std::fs::create_dir_all(&rec).unwrap();
        std::fs::write(rec.join("meta.json"), json!({"started_at": local(2026, 11, 10, 15, 0).to_rfc3339()}).to_string()).unwrap();

        let v = view(NaiveDate::from_ymd_opt(2026, 11, 10).unwrap(), None);
        assert_eq!(v["since"], "2026-11-09", "{v}");
        assert_eq!((v["calls"].as_u64(), v["memos"].as_u64(), v["with_minutes"].as_u64(), v["recorded_min"].as_i64()), (Some(4), Some(1), Some(2), Some(45 + 50 + 4)), "{v}");
        let days: Vec<u64> = v["days"].as_array().unwrap().iter().map(|d| d["calls"].as_u64().unwrap()).collect();
        assert_eq!(days, [1, 3, 0, 0, 0, 0, 0], "{v}");
        assert_eq!((v["days"][0]["date"].as_str(), v["days"][6]["date"].as_str()), (Some("2026-11-09"), Some("2026-11-15")));
        // On Sunday, last week's Sunday-night calls are its own: the week then starts on 2 Nov.
        let v = view(NaiveDate::from_ymd_opt(2026, 11, 8).unwrap(), None);
        assert_eq!((v["since"].as_str(), v["calls"].as_u64(), v["recorded_min"].as_i64(), v["days"][6]["calls"].as_u64()), (Some("2026-11-02"), Some(2), Some(30), Some(2)), "{v}");
    }

    #[test]
    fn a_scope_counts_only_its_threads_calls() {
        let (_env, _) = crate::paths::testenv::folders("calls-week-scope");
        let today = Local::now().date_naive();
        let name = Local::now().format("%Y-%m-%d_%H-%M").to_string() + "_Acme sync";
        call(&name, Local::now(), 30, false, false);
        let t = crate::projects::create("Acme rollout", None, None).unwrap().id;
        assert_eq!(view(today, None)["calls"], 1);
        assert_eq!(view(today, Some(std::slice::from_ref(&t)))["calls"], 0, "no rule puts it in");
        crate::projects::set_membership(&t, &crate::paths::calls_dir().join(&name), "include").unwrap();
        assert_eq!(view(today, Some(&[t]))["calls"], 1);
        assert_eq!(view(today, Some(&[]))["calls"], 0, "a client with no open thread: none");
    }

    #[test]
    fn a_week_with_no_calls_is_all_zeros() {
        let (_env, _) = crate::paths::testenv::folders("calls-week-empty");
        call("2026-11-01_100000-acme-sync", local(2026, 11, 1, 10, 0), 30, false, true);
        let v = view(NaiveDate::from_ymd_opt(2026, 11, 12).unwrap(), None);
        assert_eq!((v["calls"].as_u64(), v["memos"].as_u64(), v["with_minutes"].as_u64(), v["recorded_min"].as_i64()), (Some(0), Some(0), Some(0), Some(0)), "{v}");
        assert!(v["days"].as_array().unwrap().iter().all(|d| d["calls"] == 0) && v["days"].as_array().unwrap().len() == 7, "{v}");
    }
}

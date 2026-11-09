//! What Echo does with every meeting of a series (set on one of its meetings in the calendar, `echoctl series`):
//! record it always, ask or never, and in which language. `<home>/projects/series.json`, keyed by the calendar series (`id:<seriesMasterId>`), else by the
//! subject as the threads' series rule reads it (`subject:<normalised>`); written under the threads' lock.
//! A meeting set by hand (the Record button, a calendar event's own choice) still wins: a rule only fills what
//! nobody chose.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::meeting::MeetingContext;
use crate::{paths, projects, vocab};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Record {
    Always,
    Ask,
    Never,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// The series' name as the calendar gives it (shown; the key is what matches).
    #[serde(default)]
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn file() -> PathBuf {
    projects::dir().join("series.json")
}

pub fn load() -> BTreeMap<String, Rule> {
    fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// A meeting's keys, the calendar series' first: a rule set from either applies.
pub fn keys_of(m: &MeetingContext) -> Vec<String> {
    let mut out = vec![];
    if let Some(id) = m.series_id.as_deref().filter(|s| !s.is_empty()) {
        out.push(format!("id:{id}"));
    }
    if let Some(s) = m.subject.as_deref().map(vocab::norm_subject).filter(|s| !s.is_empty()) {
        out.push(format!("subject:{s}"));
    }
    out
}

/// The key a new rule for this meeting is saved under.
pub fn key_of(m: &MeetingContext) -> Option<String> {
    keys_of(m).into_iter().next()
}

pub fn rule_for(m: &MeetingContext) -> Option<Rule> {
    let all = load();
    keys_of(m).iter().find_map(|k| all.get(k).cloned())
}

/// Change a rule: `patch` holds the fields to set (`null` = back to Echo's default). A rule left with nothing set
/// is removed.
pub fn set(key: &str, subject: Option<&str>, patch: &Value) -> Result<Option<Rule>> {
    if !(key.starts_with("id:") || key.starts_with("subject:")) || key.len() > 600 {
        bail!("not a meeting series: {key:?}");
    }
    let Some(obj) = patch.as_object() else { bail!("the change is not an object") };
    projects::with_lock(|| {
        let mut all = load();
        let mut r = all.get(key).cloned().unwrap_or_default();
        if let Some(s) = subject.map(str::trim).filter(|s| !s.is_empty()) {
            r.subject = s.chars().take(200).collect();
        }
        for (k, v) in obj {
            match k.as_str() {
                "record" => r.record = if v.is_null() { None } else { Some(serde_json::from_value(v.clone()).map_err(|_| anyhow::anyhow!("record: always, ask or never"))?) },
                "languages" => {
                    r.languages = v.as_array().into_iter().flatten().filter_map(Value::as_str).map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty() && s.len() <= 8).take(8).collect();
                }
                other => bail!("unknown field {other:?}"),
            }
        }
        r.updated_at = Some(chrono::Local::now().to_rfc3339());
        let empty = Rule { subject: r.subject.clone(), updated_at: r.updated_at.clone(), ..Default::default() };
        let out = if r == empty {
            all.remove(key);
            None
        } else {
            all.insert(key.to_string(), r.clone());
            Some(r)
        };
        fs::create_dir_all(projects::dir())?;
        paths::write_atomic(&file(), &serde_json::to_vec_pretty(&all)?)?;
        Ok(out)
    })
}

/// A meeting about to be recorded takes its series' language when nobody chose one.
pub fn fill(m: &mut MeetingContext) {
    if m.languages.is_empty() {
        if let Some(r) = rule_for(m) {
            m.languages = r.languages;
        }
    }
}

/// Whether the agenda may ask "Record?" for this meeting. A meeting of a calendar series isn't recorded unless its
/// series says so (the user, 2026-11-05: "by default every series: don't record") — Record puts it in "Record when it
/// starts" instead, Ask asks; a one-off meeting is asked about as before, unless its subject's rule says never.
pub fn may_offer(m: &MeetingContext) -> bool {
    match rule_for(m).and_then(|r| r.record) {
        Some(Record::Never) => false,
        Some(Record::Ask) => true,
        Some(Record::Always) => false,
        None => m.series_id.is_none(),
    }
}

fn armed_file() -> PathBuf {
    paths::state_dir().join("series-armed.json")
}

/// After each agenda sync: today's meetings of a series set to record always join "Record when it starts" — each
/// occurrence once (`<home>/.echo/series-armed.json`), so cancelling that arm is never undone by the next sync.
pub fn schedule_always(agenda: &[MeetingContext]) -> Result<usize> {
    let all = load();
    if !all.values().any(|r| r.record == Some(Record::Always)) {
        return Ok(0);
    }
    let mut done: BTreeSet<String> = fs::read(armed_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut n = 0;
    for m in agenda {
        let (Some(id), Some(start)) = (m.id.as_deref(), m.start.as_deref()) else { continue };
        let always = keys_of(m).iter().find_map(|k| all.get(k)).and_then(|r| r.record) == Some(Record::Always);
        let ended = m.end.as_deref().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok()).is_some_and(|e| e < chrono::Utc::now());
        let occ = format!("{id}@{start}");
        // A Zoom or Google Meet meeting: Echo can't record it yet.
        if !always || ended || done.contains(&occ) || m.recordable().is_err() {
            continue;
        }
        let mut m = m.clone();
        fill(&mut m);
        crate::schedule::add(m)?;
        done.insert(occ);
        n += 1;
    }
    // Only this week's are kept (the key carries its start).
    done.retain(|k| k.rsplit_once('@').is_some_and(|(_, s)| s.get(..10).is_some_and(|d| d >= today.as_str()) || s.get(..10).is_some_and(|d| days_between(d, &today) <= 7)));
    fs::create_dir_all(paths::state_dir())?;
    paths::write_atomic(&armed_file(), &serde_json::to_vec(&done)?)?;
    Ok(n)
}

fn days_between(a: &str, b: &str) -> i64 {
    let p = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    match (p(a), p(b)) {
        (Some(a), Some(b)) => (b - a).num_days().abs(),
        _ => i64::MAX,
    }
}

/// The series of a thread: from its calls' meetings and its upcoming calendar events, each with its rule.
pub fn of_thread(id: &str) -> Result<Value> {
    let p = projects::load(id)?;
    let all = load();
    let mut seen: BTreeMap<String, Value> = BTreeMap::new();
    let mut add = |m: &MeetingContext, when: Option<&str>, upcoming: bool| {
        let Some(key) = keys_of(m).into_iter().find(|k| all.contains_key(k)).or_else(|| key_of(m)) else { return };
        let e = seen.entry(key.clone()).or_insert_with(|| {
            json!({"key": key, "subject": m.subject.clone().unwrap_or_default(), "calls": 0, "last": null, "next": null, "rule": all.get(&key)})
        });
        if upcoming {
            if e["next"].is_null() || e["next"].as_str() > when {
                e["next"] = json!(when);
            }
        } else {
            e["calls"] = json!(e["calls"].as_u64().unwrap_or(0) + 1);
            if e["last"].is_null() || e["last"].as_str() < when {
                e["last"] = json!(when);
            }
        }
    };
    for (dir, _) in projects::calls_of(&p)? {
        if let Ok(Some(m)) = MeetingContext::load(&dir) {
            // A meeting that recurs, or a rule already set: one-off meetings aren't series.
            if m.series_id.is_some() || keys_of(&m).iter().any(|k| all.contains_key(k)) || calls_share_subject(&dir) {
                add(&m, m.start.as_deref(), false);
            }
        }
    }
    for m in crate::watch::agenda() {
        if projects::of_meeting(&m).iter().any(|x| x.id == p.id) && (m.series_id.is_some() || keys_of(&m).iter().any(|k| all.contains_key(k))) {
            add(&m, m.start.as_deref(), true);
        }
    }
    let mut list: Vec<Value> = seen.into_values().collect();
    list.sort_by(|a, b| b["calls"].as_u64().cmp(&a["calls"].as_u64()).then(a["subject"].as_str().cmp(&b["subject"].as_str())));
    Ok(json!({ "series": list }))
}

/// Another call of the same subject exists (a series without a calendar id).
fn calls_share_subject(dir: &Path) -> bool {
    crate::calls::previous_subject(dir).is_some_and(|s| crate::calls::previous_with_notes(&s, Some(dir)).is_some())
}


#[cfg(test)]
mod tests {
    use super::*;

    fn meeting(id: &str, series: Option<&str>, subject: &str, start: chrono::DateTime<chrono::Utc>) -> MeetingContext {
        MeetingContext {
            id: Some(id.into()),
            series_id: series.map(Into::into),
            subject: Some(subject.into()),
            start: Some(start.to_rfc3339()),
            end: Some((start + chrono::Duration::minutes(30)).to_rfc3339()),
            ..Default::default()
        }
    }

    #[test]
    fn a_rule_is_found_by_series_then_subject_and_fills_what_nobody_chose() {
        let (_env, _) = paths::testenv::folders("series-rule");
        let now = chrono::Utc::now();
        let m = meeting("e1", Some("S1"), "Acme · Weekly sync", now);
        assert_eq!(key_of(&m).as_deref(), Some("id:S1"));
        set("id:S1", Some("Acme · Weekly sync"), &json!({"record": "always", "languages": ["UK"]})).unwrap();
        let r = rule_for(&m).unwrap();
        assert_eq!((r.record, r.languages.clone()), (Some(Record::Always), vec!["uk".to_string()]));
        // The same subject without the series id still finds it only by a subject rule.
        let loose = meeting("e2", None, "acme weekly sync", now);
        assert!(rule_for(&loose).is_none());
        assert!(may_offer(&loose), "a one-off meeting is asked about");
        assert!(!may_offer(&meeting("e3", Some("S9"), "Globex review", now)), "a series isn't, unless it says so");
        set(&key_of(&loose).unwrap(), None, &json!({"record": "never"})).unwrap();
        assert!(!may_offer(&loose));
        let mut chosen = m.clone();
        chosen.languages = vec!["en".into()];
        fill(&mut chosen);
        assert_eq!(chosen.languages, vec!["en".to_string()], "a choice made by hand wins");
        let mut open = m.clone();
        fill(&mut open);
        assert_eq!(open.languages, vec!["uk".to_string()]);
        // Back to defaults: the rule goes.
        assert!(set("id:S1", None, &json!({"record": null, "languages": []})).unwrap().is_none());
        assert!(!load().contains_key("id:S1"));
        assert_eq!(rule_for(&m).and_then(|r| r.record), Some(Record::Never), "its subject's rule still applies");
        assert!(set("nope", None, &json!({})).is_err());
        assert!(set("id:S1", None, &json!({"record": "sometimes"})).is_err());
    }

    /// #228: a series set to record always skips its Zoom or Google Meet meetings (Echo can't record them yet).
    #[test]
    fn always_skips_a_zoom_or_google_meet_meeting() {
        let (_env, _) = paths::testenv::folders("series-always-other-app");
        let now = chrono::Utc::now();
        let mut zoom = meeting("e1", Some("S1"), "Umbrella weekly", now + chrono::Duration::hours(1));
        zoom.join_url = Some("https://umbrella.zoom.us/j/81234567890".into());
        let teams = meeting("e2", Some("S1"), "Umbrella weekly", now + chrono::Duration::hours(2));
        set("id:S1", None, &json!({"record": "always"})).unwrap();
        assert_eq!(schedule_always(&[zoom, teams]).unwrap(), 1, "the Teams one only, and no error");
        assert_eq!(crate::schedule::load().iter().filter_map(|m| m.id.as_deref()).collect::<Vec<_>>(), vec!["e2"]);
    }

    #[test]
    fn always_schedules_each_occurrence_once() {
        let (_env, _) = paths::testenv::folders("series-always");
        let now = chrono::Utc::now();
        let soon = meeting("e1", Some("S1"), "Acme · Weekly sync", now + chrono::Duration::hours(1));
        let other = meeting("e2", Some("S2"), "Globex review", now + chrono::Duration::hours(2));
        set("id:S1", None, &json!({"record": "always", "languages": ["uk"]})).unwrap();
        assert_eq!(schedule_always(&[soon.clone(), other.clone()]).unwrap(), 1);
        let s = crate::schedule::load();
        assert_eq!((s.len(), s[0].languages.clone()), (1, vec!["uk".to_string()]));
        // The user cancels it: the next sync doesn't bring it back.
        crate::schedule::remove_id("e1").unwrap();
        assert_eq!(schedule_always(&[soon]).unwrap(), 0);
        assert!(crate::schedule::load().is_empty());
    }
}

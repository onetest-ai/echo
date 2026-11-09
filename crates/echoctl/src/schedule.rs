//! Recordings chosen ahead of time ("Record when it starts" in the calendar): a list of
//! meetings in `<home>/.echo/scheduled.json`. The watcher arms a meeting ~1 min before it
//! starts and keeps it armed until it ends; the entry goes away once a recording starts for it,
//! when the user cancels that arm, when its recording fails to start, or after the meeting ends.
//! Still a per-meeting opt-in — just made earlier.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

use crate::meeting::MeetingContext;
use crate::paths;

/// Arm this long before the start.
const LEAD: i64 = 60;

fn file() -> PathBuf {
    paths::state_dir().join("scheduled.json")
}

pub fn load() -> Vec<MeetingContext> {
    fs::read(file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(list: &[MeetingContext]) -> Result<()> {
    fs::create_dir_all(paths::state_dir())?;
    let tmp = file().with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(list)?)?;
    fs::rename(tmp, file())?;
    Ok(())
}

fn time(s: &Option<String>) -> Option<DateTime<Utc>> {
    s.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Utc))
}

/// Same occurrence: event id and start (a recurring series reuses ids across occurrences
/// in some calendars).
fn same(a: &MeetingContext, b: &MeetingContext) -> bool {
    a.id.is_some() && a.id == b.id && a.start == b.start
}

/// Refused for a meeting Echo can't record yet (Zoom, Google Meet).
pub fn add(m: MeetingContext) -> Result<()> {
    m.recordable()?;
    let mut list = load();
    if !list.iter().any(|x| same(x, &m)) {
        list.push(m);
    }
    save(&list)
}

pub fn remove_id(id: &str) -> Result<()> {
    let mut list = load();
    list.retain(|m| m.id.as_deref() != Some(id));
    save(&list)
}

#[cfg(test)]
pub fn is_scheduled(id: &str) -> bool {
    load().iter().any(|m| m.id.as_deref() == Some(id))
}

/// Drop meetings that are over; return the one to arm now (started or about to), if any.
pub fn due(now: DateTime<Utc>) -> Option<MeetingContext> {
    let list = load();
    let live: Vec<MeetingContext> = list.iter().filter(|m| time(&m.end).is_none_or(|e| e > now)).cloned().collect();
    if live.len() != list.len() {
        let _ = save(&live);
    }
    live.into_iter().find(|m| m.recordable().is_ok() && time(&m.start).is_some_and(|s| now >= s - Duration::seconds(LEAD)))
}

/// A recording that couldn't start: a meeting set to "Record when it starts" leaves the schedule
/// too, or the watcher would arm it again the next second and fail (and notify) again until it
/// ends. Returns the notification's text.
pub(crate) fn start_failed(meeting: Option<&MeetingContext>, e: &anyhow::Error) -> String {
    let scheduled = meeting
        .and_then(|m| m.id.as_deref())
        .filter(|id| load().iter().any(|s| s.id.as_deref() == Some(*id)))
        .is_some_and(|id| remove_id(id).is_ok());
    if scheduled {
        let name = meeting.and_then(|m| m.subject.as_deref()).unwrap_or("this meeting");
        format!("{e:#} — “{name}” is no longer set to record when it starts; click Record to try again")
    } else {
        format!("{e:#} — click Record to try again")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meeting(id: &str, start: DateTime<Utc>, mins: i64) -> MeetingContext {
        MeetingContext {
            id: Some(id.into()),
            subject: Some(id.into()),
            start: Some(start.to_rfc3339()),
            end: Some((start + Duration::minutes(mins)).to_rfc3339()),
            ..Default::default()
        }
    }

    /// #228: a Zoom or Google Meet meeting can't be set to record when it starts, and one in the list (written by
    /// hand, or before #228) is never armed.
    #[test]
    fn a_zoom_or_google_meet_meeting_is_never_scheduled() {
        let (_env, home) = crate::paths::testenv::folders("sched-other-app");
        let now = Utc::now();
        let zoom = MeetingContext { join_url: Some("https://zoom.us/j/81234567890".into()), ..meeting("zoom", now + Duration::seconds(30), 30) };
        assert!(add(zoom.clone()).unwrap_err().to_string().contains("recording isn't available for Zoom meetings yet"));
        assert!(!is_scheduled("zoom"));
        save(&[zoom]).unwrap();
        assert!(due(now).is_none(), "skipped by the scheduler");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn due_only_from_a_minute_before_until_the_end() {
        let (_env, home) = crate::paths::testenv::folders("sched");
        let now = Utc::now();
        add(meeting("later", now + Duration::minutes(30), 30)).unwrap();
        add(meeting("soon", now + Duration::seconds(30), 30)).unwrap();
        add(meeting("over", now - Duration::minutes(90), 30)).unwrap();
        assert_eq!(due(now).and_then(|m| m.id).as_deref(), Some("soon"));
        assert!(!is_scheduled("over"), "finished meetings are dropped");
        remove_id("soon").unwrap();
        assert!(due(now).is_none());
        assert!(is_scheduled("later"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn a_failed_start_unschedules_its_meeting_and_says_so() {
        let (_env, _root) = crate::paths::testenv::folders("watch-start-failed");
        let start = Utc::now() - chrono::Duration::seconds(30);
        let m = MeetingContext {
            id: Some("ev1".into()),
            subject: Some("Weekly".into()),
            start: Some(start.to_rfc3339()),
            end: Some((start + chrono::Duration::minutes(30)).to_rfc3339()),
            ..Default::default()
        };
        add(m.clone()).unwrap();
        assert!(due(Utc::now()).is_some());
        let e = anyhow::anyhow!("creating calls: read-only");
        let text = start_failed(Some(&m), &e);
        assert!(text.contains("“Weekly” is no longer set to record"), "{text}");
        assert!(due(Utc::now()).is_none(), "not armed again the next second");
        // A meeting that wasn't scheduled (or none) just says to try again.
        assert_eq!(start_failed(Some(&m), &e), "creating calls: read-only — click Record to try again");
        assert_eq!(start_failed(None, &e), "creating calls: read-only — click Record to try again");
    }
}

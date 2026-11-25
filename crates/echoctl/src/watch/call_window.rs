//! The call window, read off the watcher's thread: which call Teams is in (its timer and title), whether it is an armed
//! meeting's call, and whether the call going on is still the one being recorded.

use super::*;

/// Which call Teams is in, as its call window shows it: when the call began (its timer, counted from the call's start
/// — a rejoin of the same meeting reads the same), and its name (the window's title).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct CallSeen {
    pub(super) began: Option<DateTime<Utc>>,
    pub(super) name: Option<String>,
    /// The window names a channel ("Meeting in “General”"), not the meeting: its name says nothing about which meeting.
    pub(super) channel: bool,
}

pub(super) fn call_seen() -> CallSeen {
    // Development builds and tests: the call window as a file says (`{"began_s_ago": …, "title": …}`), for calls fed
    // from files — the real Teams is never read for them.
    // A test's stand-in set: only it is read, never the real Teams (missing or unreadable: nothing read).
    #[cfg(debug_assertions)]
    if std::env::var_os("ECHOCTL_TEST_CALL_WINDOW").is_some() {
        let v = test_call_window().unwrap_or_default();
        return seen_of(v["began_s_ago"].as_i64().map(|s| s.max(0) as u64), v["title"].as_str());
    }
    let (elapsed, title) = crate::ax::call_glance();
    seen_of(elapsed, title.as_deref())
}

pub(super) fn seen_of(elapsed: Option<u64>, title: Option<&str>) -> CallSeen {
    let channel = title.is_some_and(|t| t.split(" | ").map(str::trim).any(|p| p.starts_with("Meeting in ")));
    CallSeen { began: elapsed.map(|e| Utc::now() - chrono::Duration::seconds(e as i64)), name: title.and_then(crate::ax::call_name), channel }
}

/// A look at the call window, taken off the watcher's thread (an accessibility query can wait seconds on a busy
/// Teams): one at a time; its result is picked up on a later tick.
#[derive(Default)]
pub(super) struct CallLook {
    busy: Arc<AtomicBool>,
    out: Arc<std::sync::Mutex<Option<CallSeen>>>,
}

impl CallLook {
    pub(super) fn start(&self) {
        if self.busy.swap(true, Ordering::SeqCst) {
            return;
        }
        let (busy, out) = (self.busy.clone(), self.out.clone());
        std::thread::spawn(move || {
            let seen = call_seen();
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(seen);
            busy.store(false, Ordering::SeqCst);
        });
    }

    pub(super) fn take(&self) -> Option<CallSeen> {
        self.out.lock().unwrap_or_else(|e| e.into_inner()).take()
    }
}

/// Development builds and tests: `ECHOCTL_TEST_CALL_WINDOW` names a JSON file standing for the call window.
#[cfg(debug_assertions)]
pub(super) fn test_call_window() -> Option<Value> {
    let f = std::env::var_os("ECHOCTL_TEST_CALL_WINDOW")?;
    serde_json::from_slice(&fs::read(f).ok()?).ok()
}

/// Whether the call window can be read here (the setting, or a test's stand-in).
pub(super) fn call_window_readable() -> bool {
    #[cfg(debug_assertions)]
    if std::env::var_os("ECHOCTL_TEST_CALL_WINDOW").is_some() {
        return true;
    }
    crate::config::reads_call_window()
}

/// An arm waiting for its meeting's call while the user is on another one (`waits`: the waiting arm's id): kept until
/// the meeting ends (#483).
pub(super) fn waiting_for(waits: Option<&str>, a: &Armed, now: DateTime<Utc>) -> bool {
    waits == Some(a.id.as_str()) && a.meeting.as_ref().and_then(|m| parse_time(&m.end)).is_some_and(|end| now < end)
}

/// Whether the call going on is `meeting`'s, from one look at the call window (#483). Its name naming the meeting
/// says yes, however early the call began (people gather early, a bridge stays open). Otherwise the call's timer
/// decides: a call that began before the meeting could (`MATCH_EARLY_MIN` before its start) is another call only when
/// its window names something else too — never on a name alone (a renamed occurrence, a translated or channel title
/// read differently). `None` when the look can't tell.
pub(super) fn meetings_call(meeting: &MeetingContext, seen: &CallSeen) -> Option<bool> {
    if same_name(meeting, seen) == Some(true) {
        return Some(true);
    }
    let start = parse_time(&meeting.start)?;
    let began = seen.began?;
    if began >= start - chrono::Duration::minutes(crate::calendar::MATCH_EARLY_MIN) {
        return Some(true);
    }
    // Began before the meeting could, and its window doesn't name the meeting: another call (#483) — named otherwise,
    // or with no name to tell (a channel's, none).
    Some(false)
}

/// Whether the call window names the meeting: `None` when it names nothing to tell (no name, a channel's) or the
/// meeting has no subject.
pub(super) fn same_name(meeting: &MeetingContext, seen: &CallSeen) -> Option<bool> {
    let norm = |s: &str| s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let named = seen.name.as_deref().filter(|_| !seen.channel).map(norm).filter(|n| !n.is_empty())?;
    let subject = meeting.subject.as_deref().map(norm).filter(|s| !s.is_empty())?;
    // The meeting's name in the window's, as whole words (a chat "Acme" isn't "Acme design review"); or the window's a
    // cut form of it (three words or more, the start of the meeting's name).
    Some(format!(" {named} ").contains(&format!(" {subject} ")) || (named.split(' ').count() >= 3 && subject.starts_with(named.as_str())))
}

/// Whether the agenda meeting a call overlaps — the user asked to record the call, not that meeting — is that call's,
/// from one look at its window (#896): not when the window names something else (an ad-hoc call, another meeting
/// joined meanwhile) or the call began before the meeting could. A window that tells nothing, or a name within the
/// meeting's subject ("Acme design review" of "FW: Acme design review"): it is.
pub(super) fn guess_holds(meeting: &MeetingContext, seen: &CallSeen) -> bool {
    let within = |m: &MeetingContext| {
        let swapped = MeetingContext { subject: seen.name.clone(), ..Default::default() };
        let as_seen = CallSeen { name: m.subject.clone(), ..seen.clone() };
        same_name(&swapped, &as_seen) == Some(true)
    };
    (same_name(meeting, seen) != Some(false) || within(meeting)) && meetings_call(meeting, seen) != Some(false)
}

/// A reading that says "another call", waiting for a later one to confirm it.
#[derive(Clone, Debug)]
pub(super) struct Candidate {
    pub(super) seen: CallSeen,
    pub(super) at: Instant,
}

#[derive(Debug, PartialEq)]
pub(super) enum Judged {
    Same,
    Other,
}

/// One look at the call window against the recording's call (`rec`, filled as it is first read). `started`: when the
/// recording started — a call that began well after it isn't the recording's own, however late it is first read.
/// "Another call" only when a later look, `CONFIRM_AFTER` on, reads the same other call (`candidate`).
pub(super) fn judge(rec: &mut Option<CallSeen>, candidate: &mut Option<Candidate>, seen: CallSeen, rejoined: bool, started: Option<DateTime<Utc>>, now: Instant) -> Judged {
    let known = rec.get_or_insert_with(CallSeen::default);
    // The recording's own start, first read: never one well after the recording started (that call is a later one).
    let late = |b: DateTime<Utc>| started.is_some_and(|s| (b - s).num_seconds() > OTHER_CALL_BEGAN_S);
    let other = match (known.began, seen.began) {
        (None, Some(b)) if late(b) => true,
        _ => another_call(known, &seen, rejoined),
    };
    if !other {
        *candidate = None;
        if known.began.is_none() {
            known.began = seen.began;
        }
        if known.name.is_none() {
            known.name = seen.name;
        }
        return Judged::Same;
    }
    let same_reading = |c: &CallSeen| match (c.began, seen.began) {
        (Some(a), Some(b)) => (a - b).num_seconds().abs() <= SAME_BEGAN_S,
        (None, None) => c.name.is_some() && c.name == seen.name,
        _ => false,
    };
    match candidate {
        Some(c) if same_reading(&c.seen) && now.duration_since(c.at) >= CONFIRM_AFTER => {
            *candidate = None;
            Judged::Other
        }
        Some(c) if same_reading(&c.seen) => Judged::Same, // not long enough yet
        _ => {
            *candidate = Some(Candidate { seen, at: now });
            Judged::Same
        }
    }
}

/// `echoctl call-seen` (dev): what `call_seen` reads now.
pub fn call_seen_json() -> Value {
    let s = call_seen();
    json!({ "began": s.began, "name": s.name })
}

/// Is `now` another call than `rec`, the one being recorded? By the timer when both were read (a new call starts its
/// own from zero; the same meeting rejoined keeps its start); else, right after the mic came back (`rejoined`), by
/// name. Unreadable, or a name change within an unbroken call → the same: a wrong split would cut a call short.
pub(super) fn another_call(rec: &CallSeen, now: &CallSeen, rejoined: bool) -> bool {
    match (rec.began, now.began) {
        (Some(a), Some(b)) => (a - b).num_seconds().abs() > OTHER_CALL_BEGAN_S,
        _ => rejoined && matches!((&rec.name, &now.name), (Some(a), Some(b)) if !a.eq_ignore_ascii_case(b)),
    }
}

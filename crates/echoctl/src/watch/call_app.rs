//! Which call app is in a call: Teams, or Google Meet — in a Chrome tab or the Meet app installed from it (#230).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::apps::TargetApp;

/// How often Chrome's windows and the calendar are looked at while Chrome holds the mic and no Meet call is known.
const MEET_LOOK_EVERY: Duration = Duration::from_secs(3);

/// Whether a call is going on, and in which app. `on`: the app of the call going on, or of the recording still open
/// (it stays that app's call while that app holds the mic; another app's is no call until then). Otherwise Teams holding
/// the mic is a call; Chrome holding it is one only with a Meet sign (`meet_sign`, asked only then: which Meet, a tab or
/// the app): Chrome on the mic for anything else is no call. None: this platform can't tell.
pub(super) fn which_call(on: Option<TargetApp>, teams: Option<bool>, chrome: Option<bool>, meet_sign: impl FnOnce() -> Option<TargetApp>) -> (Option<bool>, Option<TargetApp>) {
    let holds = |app: TargetApp| if app.is_chrome() { chrome } else { teams } == Some(true);
    if let Some(app) = on.filter(|&a| holds(a)) {
        return (Some(true), Some(app));
    }
    if on.is_none() {
        if holds(TargetApp::Teams) {
            return (Some(true), Some(TargetApp::Teams));
        }
        if holds(TargetApp::Meet) {
            if let Some(meet) = meet_sign() {
                return (Some(true), Some(meet));
            }
        }
    }
    (teams.or(chrome).map(|_| false), None)
}

/// A sign that the user is on a Google Meet call, and in which: a Chrome window titled as one (a tab), else the Meet app
/// open (the app), else a calendar meeting with a Meet link from 10 min before it to its end (a tab; an arm for a Meet
/// meeting is the watcher's own sign). Looked at off the watcher's thread, once every few seconds at most: Chrome answers
/// Accessibility itself and may be slow or hung, and the loop must never wait for it; until a look has answered, there
/// is no sign.
/// The last look: when, and which Meet it saw.
type Look = Option<(Instant, Option<TargetApp>)>;

#[derive(Default)]
pub(super) struct MeetSign {
    last: Arc<Mutex<Look>>,
    busy: Arc<AtomicBool>,
}

/// Which Meet the signs point to: a Meet tab's title wins (the app may be open beside it, idle), then the app, then a
/// calendar meeting with a Meet link (in a tab: the app shows none).
fn meet_of(tab: bool, app: bool, calendar: bool) -> Option<TargetApp> {
    if tab {
        Some(TargetApp::Meet)
    } else if app {
        Some(TargetApp::MeetApp)
    } else {
        calendar.then_some(TargetApp::Meet)
    }
}

impl MeetSign {
    pub(super) fn get(&self) -> Option<TargetApp> {
        let last = *self.last.lock().unwrap_or_else(|e| e.into_inner());
        if last.is_none_or(|(at, _)| at.elapsed() >= MEET_LOOK_EVERY) && !self.busy.swap(true, Ordering::SeqCst) {
            let (out, busy) = (self.last.clone(), self.busy.clone());
            std::thread::spawn(move || {
                let seen = meet_of(
                    crate::audio::meet_tab_open() == Some(true),
                    crate::audio::meet_app_open() == Some(true),
                    crate::calendar::meet_meeting_now(Utc::now()),
                );
                *out.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), seen));
                busy.store(false, Ordering::SeqCst);
            });
        }
        // Only a recent look counts: one from before (an earlier call, Chrome then idle) may point to a Meet gone since.
        last.filter(|(at, _)| at.elapsed() < MEET_LOOK_EVERY * 2).and_then(|(_, seen)| seen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TargetApp::{Meet, MeetApp, Teams};

    #[test]
    fn teams_on_the_mic_is_a_call_and_wins_over_the_browser() {
        assert_eq!(which_call(None, Some(true), Some(false), || panic!("not asked")), (Some(true), Some(Teams)));
        assert_eq!(which_call(None, Some(true), Some(true), || Some(Meet)), (Some(true), Some(Teams)));
        assert_eq!(which_call(None, Some(false), Some(false), || panic!("not asked")), (Some(false), None));
    }

    #[test]
    fn chrome_on_the_mic_is_a_call_only_with_a_meet_sign_and_of_the_meet_it_points_to() {
        assert_eq!(which_call(None, Some(false), Some(true), || Some(Meet)), (Some(true), Some(Meet)));
        assert_eq!(which_call(None, Some(false), Some(true), || Some(MeetApp)), (Some(true), Some(MeetApp)));
        assert_eq!(which_call(None, Some(false), Some(true), || None), (Some(false), None), "a voice note in another tab");
    }

    #[test]
    fn a_meet_tab_wins_over_the_app_open_beside_it_and_the_app_over_the_calendar() {
        assert_eq!(meet_of(true, true, true), Some(Meet));
        assert_eq!(meet_of(false, true, true), Some(MeetApp));
        assert_eq!(meet_of(false, false, true), Some(Meet));
        assert_eq!(meet_of(false, false, false), None);
    }

    #[test]
    fn a_call_stays_its_apps_while_that_app_holds_the_mic() {
        // On Meet: the window title may change (another tab), the call goes on while Chrome holds the mic.
        assert_eq!(which_call(Some(Meet), Some(false), Some(true), || panic!("not asked")), (Some(true), Some(Meet)));
        assert_eq!(which_call(Some(MeetApp), Some(false), Some(true), || panic!("not asked")), (Some(true), Some(MeetApp)));
        // Teams takes the mic during a Meet call: still the Meet call.
        assert_eq!(which_call(Some(Meet), Some(true), Some(true), || Some(Meet)), (Some(true), Some(Meet)));
        // The Meet call's app let go: no call (the watcher's release debounce decides), even with Teams on the mic.
        assert_eq!(which_call(Some(Meet), Some(true), Some(false), || Some(Meet)), (Some(false), None));
        assert_eq!(which_call(Some(Teams), Some(false), Some(true), || Some(Meet)), (Some(false), None));
    }

    #[test]
    fn a_platform_that_cant_tell_says_so() {
        assert_eq!(which_call(None, None, None, || panic!("not asked")), (None, None));
    }
}

//! Which call app is in a call: Teams, or Google Meet in Chrome (#230).

use std::time::{Duration, Instant};

use chrono::Utc;

use crate::apps::{MeetingApp, TargetApp};

/// How often Chrome's windows and the calendar are looked at while Chrome holds the mic and no Meet call is known.
const MEET_LOOK_EVERY: Duration = Duration::from_secs(3);

/// Whether a call is going on, and in which app. `on`: the app of the call going on (it stays that app while it holds
/// the mic). Otherwise Teams holding the mic is a call; Chrome holding it is one only with a Meet sign (`meet_sign`,
/// asked only then): Chrome on the mic for anything else is no call. None: this platform can't tell.
pub(super) fn which_call(on: Option<TargetApp>, teams: Option<bool>, chrome: Option<bool>, meet_sign: impl FnOnce() -> bool) -> (Option<bool>, Option<TargetApp>) {
    let holds = |app| match app {
        TargetApp::Teams => teams,
        TargetApp::Meet => chrome,
    } == Some(true);
    if let Some(app) = on.filter(|&a| holds(a)) {
        return (Some(true), Some(app));
    }
    if on.is_none() {
        if holds(TargetApp::Teams) {
            return (Some(true), Some(TargetApp::Teams));
        }
        if holds(TargetApp::Meet) && meet_sign() {
            return (Some(true), Some(TargetApp::Meet));
        }
    }
    (teams.or(chrome).map(|_| false), None)
}

/// A sign that the user is on a Google Meet call: a Chrome window titled as one, or a calendar meeting with a Meet link
/// from 10 min before it to its end (an arm for a Meet meeting is the watcher's own sign). Looked at once every few
/// seconds at most.
#[derive(Default)]
pub(super) struct MeetSign {
    last: Option<(Instant, bool)>,
}

impl MeetSign {
    pub(super) fn get(&mut self) -> bool {
        if let Some((_, seen)) = self.last.filter(|(at, _)| at.elapsed() < MEET_LOOK_EVERY) {
            return seen;
        }
        let seen = crate::audio::meet_window_open() == Some(true)
            || crate::calendar::meeting_now(Utc::now(), TargetApp::Meet).is_some_and(|m| m.app() == Some(MeetingApp::Meet));
        self.last = Some((Instant::now(), seen));
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TargetApp::{Meet, Teams};

    #[test]
    fn teams_on_the_mic_is_a_call_and_wins_over_the_browser() {
        assert_eq!(which_call(None, Some(true), Some(false), || panic!("not asked")), (Some(true), Some(Teams)));
        assert_eq!(which_call(None, Some(true), Some(true), || true), (Some(true), Some(Teams)));
        assert_eq!(which_call(None, Some(false), Some(false), || panic!("not asked")), (Some(false), None));
    }

    #[test]
    fn chrome_on_the_mic_is_a_call_only_with_a_meet_sign() {
        assert_eq!(which_call(None, Some(false), Some(true), || true), (Some(true), Some(Meet)));
        assert_eq!(which_call(None, Some(false), Some(true), || false), (Some(false), None), "a voice note in another tab");
    }

    #[test]
    fn a_call_stays_its_apps_while_that_app_holds_the_mic() {
        // On Meet: the window title may change (another tab), the call goes on while Chrome holds the mic.
        assert_eq!(which_call(Some(Meet), Some(false), Some(true), || panic!("not asked")), (Some(true), Some(Meet)));
        // Teams takes the mic during a Meet call: still the Meet call.
        assert_eq!(which_call(Some(Meet), Some(true), Some(true), || true), (Some(true), Some(Meet)));
        // The Meet call's app let go: no call (the watcher's release debounce decides), even with Teams on the mic.
        assert_eq!(which_call(Some(Meet), Some(true), Some(false), || true), (Some(false), None));
        assert_eq!(which_call(Some(Teams), Some(false), Some(true), || true), (Some(false), None));
    }

    #[test]
    fn a_platform_that_cant_tell_says_so() {
        assert_eq!(which_call(None, None, None, || panic!("not asked")), (None, None));
    }
}

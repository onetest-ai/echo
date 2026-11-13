//! Call apps Echo knows how to capture: Teams desktop, and Google Meet in Chrome or installed as an app from it (macOS).

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetApp {
    Teams,
    /// Google Meet in Chrome: its audio is the browser's, every tab's.
    Meet,
    /// Google Meet installed as an app from Chrome: its own window, but its audio is still the browser's, every tab's.
    #[serde(rename = "meet-app")]
    MeetApp,
}

impl TargetApp {
    /// Every call app, in the order the watcher looks: a Teams call wins over the browser.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub const ALL: [TargetApp; 3] = [TargetApp::Teams, TargetApp::Meet, TargetApp::MeetApp];

    /// What the user sees it called.
    pub fn name(self) -> &'static str {
        match self {
            TargetApp::Teams => "Teams",
            TargetApp::Meet => "Google Meet",
            TargetApp::MeetApp => "Google Meet app",
        }
    }

    /// Whether its sound is Chrome's (a Meet tab or the Meet app): every tab's is in it.
    pub fn is_chrome(self) -> bool {
        matches!(self, TargetApp::Meet | TargetApp::MeetApp)
    }

    /// macOS bundle-id prefixes. New Teams (`com.microsoft.teams2`) plays audio from helper
    /// processes whose bundle ids share the prefix, so we match on prefix; Chrome's audio and mic run in its helpers
    /// (`com.google.Chrome.helper`). A test call app (debug builds) stands for Teams, and the real Chrome is then
    /// never looked at.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub fn mac_bundle_prefixes(self) -> &'static [&'static str] {
        match (self, test_call_app()) {
            (TargetApp::Teams, Some(test)) => test,
            (TargetApp::Meet | TargetApp::MeetApp, Some(_)) => &[],
            (TargetApp::Teams, None) => &["com.microsoft.teams2", "com.microsoft.teams"],
            (TargetApp::Meet | TargetApp::MeetApp, None) => &["com.google.Chrome"],
        }
    }

    /// Windows executable names (lowercase). New Teams is `ms-teams.exe`; classic is `teams.exe`. Meet isn't
    /// recorded on Windows yet: the browser's own loopback needs #227.
    #[allow(dead_code)]
    pub fn windows_exe_names(self) -> &'static [&'static str] {
        match self {
            TargetApp::Teams => &["ms-teams.exe", "teams.exe"],
            TargetApp::Meet | TargetApp::MeetApp => &[],
        }
    }

    /// The meeting app of a calendar meeting this call app records.
    pub fn meeting_app(self) -> MeetingApp {
        match self {
            TargetApp::Teams => MeetingApp::Teams,
            TargetApp::Meet | TargetApp::MeetApp => MeetingApp::Meet,
        }
    }
}

/// A browser window's title that says it shows a Google Meet call: Meet's own tab title ("Meet – abc-defg-hij",
/// "Meet - Weekly sync"), with or without the browser's suffix. A Meet-code-shaped word elsewhere ("fix-mute-bug") is
/// not one.
pub fn is_meet_title(title: &str) -> bool {
    title.trim().strip_prefix("Meet ").is_some_and(|r| ["-", "–", "—"].iter().any(|d| r.trim_start().starts_with(d)))
}

/// Functional tests only, in a debug build: `ECHOCTL_CALL_APP_BUNDLE=<bundle id>` makes that app the call app
/// instead of Teams (`tests/tools/testcall`, a call between two Macs on the local network), so the whole
/// recording path runs without a Teams account. A release build never reads it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn test_call_app() -> Option<&'static [&'static str]> {
    #[cfg(debug_assertions)]
    {
        static APP: std::sync::OnceLock<Option<Vec<&'static str>>> = std::sync::OnceLock::new();
        APP.get_or_init(|| {
            let id = std::env::var("ECHOCTL_CALL_APP_BUNDLE").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())?;
            Some(vec![&*Box::leak(id.into_boxed_str())])
        })
        .as_deref()
    }
    #[cfg(not(debug_assertions))]
    None
}

/// Teams' own hosts: its join links, and the ones the desktop app opens.
const TEAMS_HOSTS: &[&str] = &["teams.microsoft.com", "teams.live.com", "teams.cloud.microsoft"];
/// A join link longer than this isn't taken from an invite's text (Teams' own run to about 600 characters).
const LINK_MAX: usize = 2048;

/// The app a calendar meeting is in, from its join link (#228). Echo records Teams calls only: a Zoom or Google Meet
/// meeting shows in Today and the calendar with its app, and recording it isn't available yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeetingApp {
    Teams,
    Zoom,
    Meet,
}

impl MeetingApp {
    /// What the window and the calendar view's JSON call it (`"app"`).
    pub fn key(self) -> &'static str {
        match self {
            MeetingApp::Teams => "teams",
            MeetingApp::Zoom => "zoom",
            MeetingApp::Meet => "meet",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            MeetingApp::Teams => "Teams",
            MeetingApp::Zoom => "Zoom",
            MeetingApp::Meet => "Google Meet",
        }
    }

    /// The app of an https join link: Teams (`teams.microsoft.com/l/meetup-join/…`, `teams.live.com/meet/…`), Zoom
    /// (`zoom.us/j/<id>`, `<sub>.zoom.us/j/<id>`, `<sub>.zoom.us/my/<name>`) or Google Meet (`meet.google.com/<code>`),
    /// its host matched exactly or as a real subdomain; none for any other link.
    pub fn from_url(url: &str) -> Option<Self> {
        let rest = url.get(..8).filter(|s| s.eq_ignore_ascii_case("https://")).and(url.get(8..))?;
        if url.len() > LINK_MAX || !url.bytes().all(|b| b.is_ascii_graphic() && !b"\"'<>\\^`{|}".contains(&b)) {
            return None;
        }
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let (host, path) = (rest[..end].to_ascii_lowercase(), &rest[end..]);
        // Only a plain host: no user (`zoom.us@evil.example`), no port.
        if !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            return None;
        }
        let sub_of = |base: &str| host.strip_suffix(base).and_then(|h| h.strip_suffix('.')).is_some_and(|s| !s.is_empty() && !s.split('.').any(str::is_empty));
        // The path's first segment after `prefix`, when it starts so.
        let first = |prefix: &str| path.strip_prefix(prefix).map(|r| r.split(['/', '?', '#']).next().unwrap_or(""));
        // Teams turns the link into an `msteams:` deep link: only a meeting's own (`meetup-join/19:meeting_…`,
        // `/meet/<digits>`), never one with a dot segment that would walk to another of its pages (a pre-filled chat).
        let dots = path.split(['/', '?', '#']).any(|seg| matches!(seg.to_ascii_lowercase().replace("%2e", ".").as_str(), "." | ".."))
            // …nor an encoded slash, backslash or percent in the path (`%2f..%2f`, `%252e`), whatever Teams decodes.
            || ["%2f", "%5c", "%25"].iter().any(|e| path.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase().contains(e));
        let teams = TEAMS_HOSTS.contains(&host.as_str())
            && !dots
            && (first("/l/meetup-join/").is_some_and(|s| ["19%3ameeting_", "19:meeting_"].iter().any(|p| s.len() > p.len() && s.to_ascii_lowercase().starts_with(p)))
                || first("/meet/").is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())));
        let zoom_id = first("/j/").is_some_and(|id| (9..=12).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_digit()));
        let zoom_room = first("/my/").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_".contains(&b)));
        let zoom = (host == "zoom.us" && zoom_id) || (sub_of("zoom.us") && (zoom_id || zoom_room));
        let meet = host == "meet.google.com" && first("/").is_some_and(is_meet_code);
        [(teams, MeetingApp::Teams), (zoom, MeetingApp::Zoom), (meet, MeetingApp::Meet)].into_iter().find_map(|(hit, app)| hit.then_some(app))
    }
}

/// Google Meet's code as it writes it: `abc-defg-hij`.
fn is_meet_code(code: &str) -> bool {
    let parts: Vec<&str> = code.split('-').collect();
    parts.iter().map(|p| p.len()).eq([3, 4, 3]) && parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_lowercase()))
}

/// The first join link of a known meeting app in an invite's text (its location, preview or body; plain text or
/// HTML): others' words, so a link is taken only whole and only of a known app's host (`MeetingApp::from_url`). Cut at
/// whitespace, quotes and angle brackets, without trailing punctuation; a bare `meet.google.com/<code>` (as Google
/// writes it) is taken as its https link.
pub fn find_join_link(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`'))
        .filter_map(|word| {
            let word = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '*']);
            let link = match word.to_ascii_lowercase().find("https://") {
                Some(i) => word[i..].to_string(),
                None => format!("https://{}", word.trim_start_matches(['(', '[', '{', '*']).strip_prefix("meet.google.com/").map(|c| format!("meet.google.com/{c}"))?),
            };
            let link = link.replace("&amp;", "&");
            MeetingApp::from_url(&link).map(|_| link)
        })
        .next()
}

/// Turn a Teams https join link into the `msteams:` scheme so it opens the desktop app
/// directly instead of a browser tab.
pub fn desktop_join_url(url: &str) -> String {
    for h in TEAMS_HOSTS {
        if let Some(rest) = url.strip_prefix(&format!("https://{h}")) {
            if rest.starts_with('/') {
                return format!("msteams:{rest}");
            }
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_links_open_in_desktop_app() {
        assert_eq!(
            desktop_join_url("https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%40thread.v2/0?context=%7b%7d"),
            "msteams:/l/meetup-join/19%3ameeting_x%40thread.v2/0?context=%7b%7d"
        );
        assert_eq!(desktop_join_url("https://teams.live.com/meet/123"), "msteams:/meet/123");
        assert_eq!(desktop_join_url("https://zoom.us/j/1"), "https://zoom.us/j/1");
    }

    #[test]
    fn a_join_link_of_each_app_is_found_in_an_invites_text() {
        let teams = "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%40thread.v2/0?context=%7b%22Tid%22%3a%22t%22%7d";
        assert_eq!(find_join_link(&format!("Join on your computer\n{teams}\nMeeting ID: 1")).as_deref(), Some(teams));
        assert_eq!(find_join_link("Join: https://teams.live.com/meet/9312345678?p=abc").as_deref(), Some("https://teams.live.com/meet/9312345678?p=abc"));
        assert_eq!(find_join_link("Join Zoom Meeting https://zoom.us/j/81234567890?pwd=abc123 Meeting ID: 812 3456 7890").as_deref(), Some("https://zoom.us/j/81234567890?pwd=abc123"));
        assert_eq!(find_join_link("https://umbrella.zoom.us/j/81234567890").as_deref(), Some("https://umbrella.zoom.us/j/81234567890"));
        assert_eq!(find_join_link("My room: https://umbrella.zoom.us/my/kim.park").as_deref(), Some("https://umbrella.zoom.us/my/kim.park"));
        assert_eq!(find_join_link("Google Meet joining info\nVideo call link: https://meet.google.com/abc-defg-hij\nOr dial").as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        assert_eq!(find_join_link("Join with Google Meet: meet.google.com/abc-defg-hij").as_deref(), Some("https://meet.google.com/abc-defg-hij"), "Google's bare link, made https");
        assert_eq!(find_join_link("No link here, just a room: Berlin 4.12"), None);
    }

    #[test]
    fn a_link_in_html_or_followed_by_punctuation_is_cut_clean() {
        let html = r#"<p>Join: <a href="https://zoom.us/j/81234567890?pwd=abc&amp;uname=x">https://zoom.us/j/81234567890?pwd=abc&amp;uname=x</a></p>"#;
        assert_eq!(find_join_link(html).as_deref(), Some("https://zoom.us/j/81234567890?pwd=abc&uname=x"));
        assert_eq!(find_join_link("(see https://meet.google.com/abc-defg-hij).").as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        assert_eq!(find_join_link("Link: https://zoom.us/j/81234567890, thanks!").as_deref(), Some("https://zoom.us/j/81234567890"));
        assert_eq!(find_join_link("<https://meet.google.com/abc-defg-hij>").as_deref(), Some("https://meet.google.com/abc-defg-hij"));
        assert_eq!(find_join_link("first https://meet.google.com/abc-defg-hij then https://zoom.us/j/81234567890").as_deref(), Some("https://meet.google.com/abc-defg-hij"));
    }

    #[test]
    fn look_alike_hosts_and_other_links_are_not_join_links() {
        for text in [
            "https://zoom.us.evil.example/j/81234567890",
            "https://evilzoom.us/j/81234567890",
            "https://zoom.us@evil.example/j/81234567890",
            "https://evil.example/?u=zoom.us/j/81234567890",
            "https://zoom.us:8443/j/81234567890",
            "http://zoom.us/j/81234567890",
            "https://zoom.us/my/kim",
            "https://zoom.us/j/abc",
            "https://.zoom.us/j/81234567890",
            "https://a..zoom.us/j/81234567890",
            "https://meet.google.com.evil.example/abc-defg-hij",
            "https://x.meet.google.com/abc-defg-hij",
            "https://meet.google.com/landing",
            "evilmeet.google.com/abc-defg-hij",
            "https://teams.microsoft.com.evil.example/l/meetup-join/19%3a1",
            "https://teams.microsoft.com/",
            // #253 review: a dot segment walks Teams' deep link to a pre-filled chat; only a meeting's own link.
            "https://teams.microsoft.com/l/meetup-join/x/../../chat/0/0?users=a@evil.example&message=hi",
            "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/%2e%2e/%2E%2E/chat/0/0?users=a@evil.example",
            "https://teams.microsoft.com/l/meetup-join/x/0",
            "https://teams.microsoft.com/l/chat/0/0?users=a@evil.example",
            "https://teams.live.com/meet/abc",
            "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%2f..%2f..%2fchat/0/0?users=a@evil.example",
            "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%5c..%5cchat/0",
            "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x/%252e%252e/chat/0",
            "javascript:alert(1)//https://zoom.us/j/",
        ] {
            assert_eq!(find_join_link(text), None, "{text}");
        }
        assert_eq!(find_join_link(&format!("https://zoom.us/j/81234567890?pwd={}", "a".repeat(LINK_MAX))), None, "too long");
    }

    #[test]
    fn a_browser_window_shows_a_meet_call_only_by_meets_own_tab_title() {
        for t in ["Meet – abc-defg-hij - Google Chrome", "Meet - Weekly sync", "Meet — abc-defg-hij", "  Meet - abc-defg-hij"] {
            assert!(is_meet_title(t), "{t}");
        }
        for t in ["Meeting notes - Google Docs", "Meetup – Berlin Rust", "Google Meet", "Inbox (3) - kim@acme.example - Gmail", "Meet", "Meet-up - x", "Meetings-room-abc - Google Chrome", "fix-mute-bug · Pull Request #12 · acme/app", "abc-defg-hij - Google Chrome"] {
            assert!(!is_meet_title(t), "{t}");
        }
    }

    #[test]
    fn meet_is_chromes_audio_on_a_mac_and_nothing_on_windows_yet() {
        assert_eq!(TargetApp::Meet.mac_bundle_prefixes(), &["com.google.Chrome"]);
        assert!(TargetApp::Meet.windows_exe_names().is_empty());
        assert_eq!(TargetApp::ALL.map(|a| (a.name(), a.meeting_app())), [("Teams", MeetingApp::Teams), ("Google Meet", MeetingApp::Meet), ("Google Meet app", MeetingApp::Meet)]);
        assert_eq!(serde_json::to_value(TargetApp::Meet).unwrap(), "meet", "meta.json's app");
        assert_eq!(serde_json::to_value(TargetApp::MeetApp).unwrap(), "meet-app", "meta.json's app");
    }

    #[test]
    fn the_app_follows_the_links_host() {
        assert_eq!(MeetingApp::from_url("https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0"), Some(MeetingApp::Teams));
        assert_eq!(MeetingApp::from_url("HTTPS://Umbrella.Zoom.US/j/81234567890"), Some(MeetingApp::Zoom));
        assert_eq!(MeetingApp::from_url("https://meet.google.com/abc-defg-hij?authuser=0"), Some(MeetingApp::Meet));
        assert_eq!(MeetingApp::from_url("https://teams.example/ev-m01"), None, "the test data's links: no app known");
    }
}

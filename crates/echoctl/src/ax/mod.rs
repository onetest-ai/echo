//! Reading the call app's window through the platform's accessibility API (macOS AX, Windows
//! UI Automation): `MuteWatch` follows the microphone button, `UiRecorder` logs the window's
//! text during a recording (raw material for naming remote speakers; text only, no
//! screenshots), and the dev tools `echoctl ax-dump` / `ax-mic`. Same API on both platforms.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

/// `ui.jsonl` stops growing past this.
const UI_LOG_MAX_BYTES: u64 = 40_000_000;
/// Full window trees saved at these offsets into the call (structure for building the reader).
const UI_TREES_AT: &[u64] = &[10, 120, 600];

/// Teams' microphone button (an HTML id: AutomationId on Windows, AXDOMIdentifier on macOS —
/// Teams is the same web app in WebView2 on both). The window holding it is the call window.
const MIC_BUTTON_ID: &str = "microphone-button";
/// On the stage while someone else shares (whatever Teams' language; absent while you present).
const POPOUT_ID: &str = "popout-content-button";
/// English only, and the only source of the presenter's name.
const SHARED_BY: &str = "Content shared by ";
const SHARE_BUTTON_ID: &str = "share-button";
/// The share button's label while you present (English only).
const STOP_SHARING: &str = "Stop sharing";

/// Screen sharing in the call.
#[derive(Debug, Clone, PartialEq)]
pub enum Sharing {
    Nobody,
    /// Someone else presents (their name when Teams is in English).
    Them(Option<String>),
    /// You present (detected in English Teams only).
    Me,
}

/// Who is presenting, folded from the call window's nodes (id + each of its labels).
#[derive(Default)]
struct ShareSeen {
    popout: bool,
    by: Option<String>,
    mine: bool,
}

impl ShareSeen {
    fn see(&mut self, id: &str, label: &str) {
        self.popout |= id == POPOUT_ID;
        self.mine |= id == SHARE_BUTTON_ID && label.starts_with(STOP_SHARING);
        if let Some(who) = label.strip_prefix(SHARED_BY) {
            self.by = Some(who.trim().to_string());
        }
    }

    fn sharing(self) -> Sharing {
        match self {
            ShareSeen { mine: true, .. } => Sharing::Me,
            ShareSeen { popout, by, .. } if popout || by.is_some() => Sharing::Them(by),
            _ => Sharing::Nobody,
        }
    }
}

/// Who the call is with, from the call window's title: Teams titles it after the chat it was
/// started from (a 1:1 chat: the other person; a group chat: its name) or the meeting, with
/// " | "-separated suffixes (tenant, account, "Microsoft Teams"). Returns the first part.
pub fn call_name(title: &str) -> Option<String> {
    let first = title.split(" | ").next()?.trim();
    let name = ["Call with ", "Meeting with ", "Meeting in "]
        .iter()
        .find_map(|p| first.strip_prefix(p))
        .unwrap_or(first)
        .trim_matches(|c: char| c == '"' || c == '“' || c == '”' || c.is_whitespace());
    let generic = ["", "Microsoft Teams", "Meeting", "Call", "Calls", "Chat", "Teams"];
    (!generic.iter().any(|g| name.eq_ignore_ascii_case(g)) && !name.contains('@')).then(|| name.to_string())
}

/// Looks like one person's name ("Jane Doe", "Mary-Ann O'Neil"): 2–4 capitalised words of
/// letters. Group chat names and meeting subjects usually aren't.
pub fn is_person_name(s: &str) -> bool {
    let words: Vec<&str> = s.split_whitespace().collect();
    (2..=4).contains(&words.len())
        && words.iter().all(|w| {
            w.chars().next().is_some_and(char::is_uppercase) && w.chars().all(|c| c.is_alphabetic() || c == '-' || c == '\'' || c == '.')
        })
}

/// The call window's title right now (`echoctl` dev aid and the call's names).
pub fn call_title() -> Option<String> {
    CallWindow::teams()?.title()
}

/// Muted? from a microphone button's label: "Unmute …" ⇒ muted, "Mute …" ⇒ live.
/// Labels about notifications, speakers or other people are ignored.
pub fn mute_state_from_labels(labels: &[String]) -> Option<bool> {
    let ignore = ["notification", "speaker", "everyone", "all ", "attendee", "participant", "chat", "camera", "video"];
    for t in labels {
        if ignore.iter().any(|w| t.contains(w)) {
            continue;
        }
        if t.contains("unmute") {
            return Some(true);
        }
        if t.contains("mute") {
            return Some(false);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mute_labels_read_both_ways() {
        let l = |s: &str| vec![s.to_lowercase()];
        assert_eq!(mute_state_from_labels(&l("Unmute (⌘+Shift+M)")), Some(true));
        assert_eq!(mute_state_from_labels(&l("Mute (⌘+Shift+M)")), Some(false));
        assert_eq!(mute_state_from_labels(&l("Unmute (Ctrl+Shift+M)")), Some(true));
        assert_eq!(mute_state_from_labels(&l("Mic: Mute")), Some(false));
        assert_eq!(mute_state_from_labels(&l("Mute notifications")), None);
        assert_eq!(mute_state_from_labels(&l("Mute all attendees")), None);
        assert_eq!(mute_state_from_labels(&l("Share")), None);
    }

    #[test]
    fn who_the_call_is_with_from_the_window_title() {
        assert_eq!(call_name("Jane Doe | Contoso | jane@contoso.com | Microsoft Teams").as_deref(), Some("Jane Doe"));
        assert_eq!(call_name("Call with Jane Doe | Microsoft Teams").as_deref(), Some("Jane Doe"));
        assert_eq!(call_name("Weekly sync | Microsoft Teams").as_deref(), Some("Weekly sync"));
        assert_eq!(call_name("Microsoft Teams"), None);
        assert_eq!(call_name("Meeting | Microsoft Teams"), None);
        assert!(is_person_name("Jane Doe") && is_person_name("Mary-Ann O'Neil"));
        assert!(!is_person_name("Weekly sync") && !is_person_name("Jane") && !is_person_name("Q3 Planning Review"));
    }

    #[test]
    fn sharing_from_the_call_window() {
        let read = |nodes: &[(&str, &str)]| {
            let mut s = ShareSeen::default();
            nodes.iter().for_each(|(id, label)| s.see(id, label));
            s.sharing()
        };
        assert_eq!(read(&[(MIC_BUTTON_ID, "Mute mic"), (SHARE_BUTTON_ID, "Share")]), Sharing::Nobody);
        assert_eq!(read(&[(POPOUT_ID, "Pop out")]), Sharing::Them(None)); // any language
        assert_eq!(read(&[("", "Content shared by Jane Doe "), (POPOUT_ID, "")]), Sharing::Them(Some("Jane Doe".into())));
        assert_eq!(read(&[(SHARE_BUTTON_ID, "Stop sharing (⌘+Shift+E)")]), Sharing::Me);
    }
}

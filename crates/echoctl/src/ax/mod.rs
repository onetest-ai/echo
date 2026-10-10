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
}

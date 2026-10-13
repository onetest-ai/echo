//! Settings: `<home>/.echo/config.json` (home = the data folder, see `paths`).

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::paths;

/// Call languages a new data folder starts with (the team's: English, Ukrainian, Russian,
/// Spanish). `echoctl config --languages any` clears them.
pub const DEFAULT_LANGUAGES: &[&str] = &["en", "uk", "ru", "es"];

fn default_languages() -> Vec<String> {
    DEFAULT_LANGUAGES.iter().map(|l| l.to_string()).collect()
}

/// The settings of a folder that has none yet: the serde defaults, so a missing file, a
/// missing field and `unwrap_or_default()` all agree (languages included).
impl Default for Config {
    fn default() -> Self {
        serde_json::from_str("{}").expect("every Config field has a default")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Active transcription model (a catalog name, e.g. "large-v3-turbo-q5_0")
    pub model: Option<String>,
    /// Languages spoken in this workspace's calls (ISO codes, e.g. ["en", "uk", "ru"]).
    /// Language detection runs per utterance and may only pick from this list, which stops
    /// Whisper confusing close languages (uk/ru) or drifting to unrelated ones. Empty = any;
    /// a new data folder (no setting yet) starts with `DEFAULT_LANGUAGES`.
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
    /// Terms for every call (names, products, acronyms the user wants always): `echoctl vocab
    /// add --always`. Learned terms live in `learned`, each tied to its meetings (`vocab.rs`).
    #[serde(default)]
    pub vocabulary: Vec<String>,
    /// Terms learned from calls, each with the subjects and people it came up with; used only
    /// in calls like those. None = not yet migrated from the old one-list `vocabulary`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned: Option<Vec<crate::vocab::Learned>>,
    /// Calls not on the agenda: "ask" (prompt when the call starts) or "manual" (never prompt;
    /// the user says "record" → `echoctl arm`). Default: ask.
    #[serde(default)]
    pub unscheduled: Option<String>,
    /// Keep mic.wav/remote.wav after a successful transcription. Default: false — only text
    /// (transcript, segments, notes) is kept.
    #[serde(default)]
    pub keep_audio: bool,
    /// Calendar meetings: "ask" (offer "record?" ~1 min before) or "manual" (never prompt; the
    /// user arms from the tray or `/echo:record`). Default: ask.
    #[serde(default)]
    pub scheduled: Option<String>,
    /// Command printing calendar events as JSON (msgraph shape) for `agenda sync`; default: the
    /// msgraph skill's CLI when installed. Empty = use the default.
    #[serde(default)]
    pub calendar_command: Option<Vec<String>>,
    /// Run whisper.cpp on the GPU: true / false; unset = the GPU unless it failed its check
    /// (Windows, `gpu.rs`).
    #[serde(default)]
    pub use_gpu: Option<bool>,
    /// Follow the call app's mute button: while muted, the mic track records silence. Default on.
    #[serde(default)]
    pub follow_app_mute: Option<bool>,
    /// The user (name/email) — splits attendees into colleagues vs external in notes.
    #[serde(default)]
    pub me: Option<crate::meeting::Person>,
    /// Record the call app's window text during calls (`<call>/ui.jsonl`, macOS Accessibility),
    /// used to name remote speakers. Default on (when Accessibility is granted).
    #[serde(default)]
    pub capture_ui: Option<bool>,
    /// Read the meeting's Teams chat during calls (`<call>/chat.json`, needs the Microsoft 365
    /// sign-in). Default on.
    #[serde(default)]
    pub capture_chat: Option<bool>,
    /// Microsoft 365 app registration (public client) the calendar sign-in uses; default
    /// CodeMie's (Contoso). Other orgs: register a public client with redirect http://localhost
    /// and delegated Calendars.Read, and put its id here.
    #[serde(default)]
    pub graph_client_id: Option<String>,
    /// Entra tenant for the sign-in: "common" (default), "organizations" or a tenant id.
    #[serde(default)]
    pub graph_tenant: Option<String>,
    /// Cancel the speaker echo in the mic track before transcribing (WebRTC AEC3, offline).
    /// Default on; skipped automatically when there is no echo (headphones).
    #[serde(default)]
    pub echo_cancel: Option<bool>,
    /// Save key frames of the meeting window (shared screens) during calls
    /// (`<call>/frames/`, macOS, needs Screen Recording). Default off.
    #[serde(default)]
    pub capture_frames: bool,
    /// Only Claude Code sessions opened in this folder (or below) write notes automatically.
    /// None = any session with the plugin.
    #[serde(default)]
    pub notes_in: Option<PathBuf>,
    /// Model Claude Code uses when the app writes notes itself (`echoctl notes`): an alias
    /// (`sonnet`, `opus`, `haiku`), a full model id, or `default` for Claude Code's own setting
    /// (local models, gateways, Bedrock / Vertex). None = sonnet.
    #[serde(default)]
    pub notes_model: Option<String>,
    /// Model for the live pane's rolling summary and quick questions during a call (a fast
    /// one). None = haiku.
    #[serde(default)]
    pub live_model: Option<String>,
}

pub const DEFAULT_NOTES_MODEL: &str = "sonnet";
pub const DEFAULT_LIVE_MODEL: &str = "haiku";

impl Config {
    /// The `--model` value for notes runs; None = leave the choice to Claude Code.
    pub fn notes_model_arg(&self) -> Option<String> {
        match self.notes_model.as_deref().map(str::trim) {
            None | Some("") => Some(DEFAULT_NOTES_MODEL.into()),
            Some("default") => None,
            Some(m) => Some(m.to_string()),
        }
    }

    /// The model as the user set it, for display.
    pub fn notes_model_label(&self) -> String {
        self.notes_model.clone().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| DEFAULT_NOTES_MODEL.into())
    }
}

fn path() -> PathBuf {
    paths::state_dir().join("config.json")
}

/// A vocabulary term: 2–50 chars, ≤ 6 words.
pub fn valid_term(t: &str) -> bool {
    let n = t.chars().count();
    (2..=50).contains(&n) && t.split_whitespace().count() <= 6
}

impl Config {
    /// Prompt the user when an unscheduled call starts?
    pub fn ask_unscheduled(&self) -> bool {
        self.unscheduled.as_deref() != Some("manual")
    }

    /// Offer "record?" before calendar meetings?
    pub fn ask_scheduled(&self) -> bool {
        self.scheduled.as_deref() != Some("manual")
    }

    /// May a Claude Code session working in `cwd` write notes automatically?
    pub fn notes_allowed(&self, cwd: &std::path::Path) -> bool {
        let Some(dir) = &self.notes_in else { return true };
        let norm = |p: &std::path::Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        norm(cwd).starts_with(norm(dir))
    }

    pub fn load() -> Result<Self> {
        match fs::read(path()) {
            Ok(b) => {
                let mut cfg: Config = serde_json::from_slice(&b)?;
                // Once: the old one-list vocabulary is scoped to the calls it came from.
                if crate::vocab::migrate(&mut cfg) {
                    cfg.save()?;
                }
                Ok(cfg)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic (temp file + rename): a concurrent reader never sees a half-written file — and
    /// callers must not save after a failed load, or they'd write defaults over the settings.
    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(paths::state_dir())?;
        let tmp = path().with_extension(format!("json.{}.tmp", std::process::id()));
        fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        fs::rename(&tmp, path())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_folders_get_the_default_languages_but_any_stays_any() {
        let new: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(new.languages, ["en", "uk", "ru", "es"]);
        assert_eq!(Config::default().languages, new.languages);
        let any: Config = serde_json::from_str(r#"{"languages": []}"#).unwrap();
        assert!(any.languages.is_empty());
    }

    #[test]
    fn notes_folder_limits_sessions() {
        let mut c = Config::default();
        assert!(c.notes_allowed(std::path::Path::new("/anywhere")));
        c.notes_in = Some("/work/calls".into());
        assert!(c.notes_allowed(std::path::Path::new("/work/calls")));
        assert!(c.notes_allowed(std::path::Path::new("/work/calls/sub")));
        assert!(!c.notes_allowed(std::path::Path::new("/work/callsx")));
        assert!(!c.notes_allowed(std::path::Path::new("/work/other")));
    }
}

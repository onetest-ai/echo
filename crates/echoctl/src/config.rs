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
    /// Languages this workspace's calls may be in (ISO codes, e.g. ["en", "uk", "ru"]). A call is
    /// in one of them, detected once from its first utterances (`transcribe::CallLanguage`); only
    /// this list is considered, which stops Whisper confusing close languages (uk/ru) or drifting
    /// to unrelated ones. One language: no detection at all. Empty = any;
    /// a new data folder (no setting yet) starts with `DEFAULT_LANGUAGES`.
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,
    /// Terms for every call (names, products, acronyms the user wants always): `echoctl vocab
    /// add --always`. Learned terms live in `learned`, each tied to its meetings (`vocab.rs`).
    #[serde(default)]
    pub vocabulary: Vec<String>,
    /// Terms learned from calls, each with the subjects and people it came up with; used only
    /// in calls like those. None = a config from before learned terms (no `learned` key), whose
    /// one-list `vocabulary` `vocab::migrate` scopes once. Always written, `[]` for None: a config
    /// this build saved is never taken for an old one, or its every-call terms would be moved (#106).
    #[serde(default, serialize_with = "learned_always")]
    pub learned: Option<Vec<crate::vocab::Learned>>,
    /// Calls not on the agenda: "ask" (prompt when the call starts) or "manual" (never prompt;
    /// the user says "record" → `echoctl arm`). Default: ask.
    #[serde(default)]
    pub unscheduled: Option<String>,
    /// Keep mic.wav/remote.wav after a successful transcription. Default: false — only text
    /// (transcript, segments, notes) is kept.
    #[serde(default)]
    pub keep_audio: bool,
    /// When notes are written, also write the minutes page: notes.md as one self-contained HTML
    /// file next to it, screenshots embedded (`minutes.rs`). Default: false.
    #[serde(default)]
    pub minutes_page: bool,
    /// The app writes a call's minutes by itself, 15 minutes after the call ends or after the
    /// user's last note (`notes::auto_due`). Default: on. Off: minutes are written only when asked.
    #[serde(default = "yes")]
    pub auto_minutes: bool,
    /// Projects' work ledgers kept current by themselves after their calls (`projectsync.rs`): the
    /// project's AI profile reads each new call into its tree. Default: on.
    #[serde(default = "yes")]
    pub project_sync: bool,
    /// The window's Echo folder session (the user's own Claude Code) starts with
    /// `--dangerously-skip-permissions`: no prompts, so in auto mode it can reach other sessions and
    /// other systems (closing the user's tasks there). Never the call's session, which reads others'
    /// words under the guard, nor any run Echo starts. Default: off.
    #[serde(default)]
    pub folder_skip_permissions: bool,
    /// macOS: no Dock icon while the Echo window is closed — Echo only in the menu bar, as before 0.7.8. Off by
    /// default: a full menu bar hides icons, and Echo then looked gone.
    #[serde(default)]
    pub hide_dock_icon: bool,
    /// Windows: no system title bar — Echo's strip is the top of the window, with its own minimise, maximise and close
    /// (#76). Default on (unset = on); off only when the user turned it off — and only then written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub own_title_bar: Option<bool>,
    /// Calls that ended before this are never written automatically (set when the switch is first
    /// on, and again on each off → on), so turning it on doesn't send a backlog of calls to a model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_minutes_since: Option<chrono::DateTime<chrono::Utc>>,
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
    /// CodeMie's. An organisation may register its own public client with redirect http://localhost
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
    /// The folder Echo runs Claude Code in for minutes (the data folder when None; with the built-in
    /// `system` AI profile its `.claude/settings.json` applies), and the only one (or below) whose
    /// sessions `echoctl claim --for` accepts.
    #[serde(default)]
    pub notes_in: Option<PathBuf>,
    /// Model Claude Code uses when the app writes notes itself (`echoctl notes`): an alias
    /// (`sonnet`, `opus`, `haiku`), a full model id, or `default` for Claude Code's own setting
    /// (local models, gateways, Bedrock / Vertex). None = sonnet.
    #[serde(default)]
    pub notes_model: Option<String>,
    /// Model for the live notes' rolling summary and quick questions during a call (a fast
    /// one). None = haiku.
    #[serde(default)]
    pub live_model: Option<String>,
}

fn yes() -> bool {
    true
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

fn learned_always<S: serde::Serializer>(l: &Option<Vec<crate::vocab::Learned>>, s: S) -> Result<S::Ok, S::Error> {
    l.as_deref().unwrap_or(&[]).serialize(s)
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

    /// Turn automatic minutes on or off. A real off → on starts counting from now; setting it on
    /// when it is on keeps the boundary (calls waiting for their minutes stay due).
    pub fn set_auto_minutes(&mut self, on: bool) {
        if on && (!self.auto_minutes || self.auto_minutes_since.is_none()) {
            self.auto_minutes_since = Some(chrono::Utc::now());
        }
        self.auto_minutes = on;
    }

    /// May a Claude Code session working in `cwd` claim a call (`echoctl claim --for`)?
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
    fn automatic_minutes_count_from_when_they_were_turned_on() {
        let mut cfg = Config::default();
        assert!(cfg.auto_minutes && cfg.auto_minutes_since.is_none(), "on by default, its start set by the app");
        cfg.set_auto_minutes(true);
        let since = cfg.auto_minutes_since.expect("set when first on");
        cfg.set_auto_minutes(true);
        assert_eq!(cfg.auto_minutes_since, Some(since), "on again: calls waiting stay due");
        cfg.set_auto_minutes(false);
        assert_eq!(cfg.auto_minutes_since, Some(since));
        std::thread::sleep(std::time::Duration::from_millis(5));
        cfg.set_auto_minutes(true);
        assert!(cfg.auto_minutes_since.unwrap() > since, "off → on: counted from now");
    }

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

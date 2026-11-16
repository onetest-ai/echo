//! A call's one language: chosen when its recording starts (the starter's choice, its meeting series', else the
//! default, `default_language`), kept in its `meta.json`, and changed only by hand (`set`). Nothing detects it.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::config::Config;

/// A language code Echo transcribes in ("en", "ru", "uk", …).
fn known(code: &str) -> bool {
    #[cfg(feature = "transcribe")]
    return whisper_rs::get_lang_id(code).is_some();
    #[cfg(not(feature = "transcribe"))]
    return (2..=3).contains(&code.len()) && code.chars().all(|c| c.is_ascii_lowercase());
}

/// What a call is transcribed in when nothing chose its language: the first of Settings' languages, else English.
pub(crate) fn default_language(cfg: &Config) -> String {
    cfg.languages.iter().find(|l| known(l)).cloned().unwrap_or_else(|| "en".into())
}

/// A new call's language: the first known one `chosen` (the Record button, `--languages`, the meeting's series),
/// else the default.
pub(crate) fn for_new_call(chosen: Option<&[String]>, cfg: &Config) -> String {
    chosen.into_iter().flatten().map(|l| l.trim().to_lowercase()).find(|l| known(l)).unwrap_or_else(|| default_language(cfg))
}

#[cfg(feature = "transcribe")]
/// A call's language: its `meta.json`'s, else the first it was started with (an older call), else the
/// default.
pub(crate) fn of_call(dir: Option<&Path>, cfg: &Config) -> i32 {
    let meta = dir.and_then(|d| fs::read(d.join("meta.json")).ok()).and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
    let own = meta.as_ref().and_then(|m| m["language"].as_str().map(str::to_owned).or_else(|| m["languages"][0].as_str().map(str::to_owned)));
    own.and_then(|l| whisper_rs::get_lang_id(&l))
        .or_else(|| whisper_rs::get_lang_id(&default_language(cfg)))
        .unwrap_or(0)
}

#[cfg(feature = "transcribe")]
/// The language a call's `meta.json` names, if any.
pub(crate) fn meta_language(dir: &Path) -> Option<i32> {
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).ok()?).ok()?;
    whisper_rs::get_lang_id(meta["language"].as_str()?)
}

/// A call's segments are all in its language: else (changed by hand after the live pass's last look) an error, and the
/// call is transcribed again from its audio (`watch::finish_transcript`).
#[cfg(feature = "transcribe")]
pub(crate) fn check_segments(dir: &Path, segs: &[crate::transcribe::Segment]) -> Result<()> {
    let lang = meta_language(dir).and_then(whisper_rs::get_lang_str);
    match lang.filter(|l| segs.iter().any(|s| s.lang != *l)) {
        Some(l) => Err(anyhow!("its language was changed to {l} as it ended")),
        None => Ok(()),
    }
}

/// What changing a call's language does (`change`).
#[derive(Debug, PartialEq)]
pub enum Changed {
    /// It records: what is said from now on is in the new language, the whole call again at its end (`live::run`).
    WhileRecording(String),
    /// It ended and its audio was kept: it is to be transcribed again, in the new language.
    TranscribeAgain(String),
}

/// Change a call's language by hand (`echoctl language`, the window). Refused after the call once its audio is gone:
/// nothing could be transcribed again.
pub fn change(dir: &Path, code: &str) -> Result<Changed> {
    let meta: serde_json::Value = fs::read(dir.join("meta.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let recording = meta["ended_at"].is_null();
    if !recording && !["mic.wav", "remote.wav"].iter().any(|f| dir.join(f).exists()) {
        return Err(anyhow!("its audio was deleted once the transcript was written, so it can't be transcribed again in another language"));
    }
    let code = set(dir, code)?;
    Ok(if recording { Changed::WhileRecording(code) } else { Changed::TranscribeAgain(code) })
}

/// Write a call's language into its `meta.json` (`language`, and `languages` for older readers).
fn set(dir: &Path, code: &str) -> Result<String> {
    let code = code.trim().to_lowercase();
    if !known(&code) {
        return Err(anyhow!("'{code}' is not a language Echo knows (e.g. en, ru, uk)"));
    }
    crate::calls::update_meta(dir, |m| {
        m["language"] = serde_json::json!(code);
        m["languages"] = serde_json::json!([code]);
        Ok(())
    })?;
    Ok(code)
}

#[cfg(all(test, feature = "transcribe"))]
mod tests {
    use super::*;

    fn id(c: &str) -> i32 {
        whisper_rs::get_lang_id(c).unwrap()
    }

    #[test]
    fn a_call_is_in_its_chosen_language_else_the_default() {
        let (_env, _) = crate::paths::testenv::folders("call-language-of-call");
        let cfg = Config { languages: vec!["ru".into(), "en".into()], ..Default::default() };
        assert_eq!(default_language(&cfg), "ru", "the first of Settings' languages");
        assert_eq!(default_language(&Config { languages: vec![], ..Default::default() }), "en");
        assert_eq!(for_new_call(Some(&["UK".into()]), &cfg), "uk", "the starter's choice wins");
        assert_eq!(for_new_call(Some(&["klingon".into()]), &cfg), "ru", "an unknown choice: the default");
        assert_eq!(for_new_call(None, &cfg), "ru");
        let dir = crate::paths::home().join("calls/2026-11-09_100000-x");
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(of_call(Some(&dir), &cfg), id("ru"), "no meta.json: the default");
        fs::write(dir.join("meta.json"), r#"{"title": "x", "languages": ["en", "uk"]}"#).unwrap();
        assert_eq!(of_call(Some(&dir), &cfg), id("en"), "an older call: the first it was started with");
        fs::write(dir.join("meta.json"), r#"{"title": "x", "language": "uk"}"#).unwrap();
        assert_eq!((of_call(Some(&dir), &cfg), meta_language(&dir)), (id("uk"), Some(id("uk"))));
    }

    #[test]
    fn a_language_changes_while_recording_or_with_the_audio_kept_never_after_it_is_gone() {
        let (_env, _) = crate::paths::testenv::folders("call-language-change");
        let dir = crate::paths::home().join("calls/2026-11-09_100000-x");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), r#"{"title": "x", "language": "en"}"#).unwrap();
        assert_eq!(change(&dir, "ru").unwrap(), Changed::WhileRecording("ru".into()), "no end yet: it records");
        fs::write(dir.join("meta.json"), r#"{"title": "x", "language": "ru", "ended_at": "2026-11-09T10:30:00+02:00"}"#).unwrap();
        fs::write(dir.join("remote.wav"), b"x").unwrap();
        assert_eq!(change(&dir, "en").unwrap(), Changed::TranscribeAgain("en".into()), "ended, the audio kept");
        fs::remove_file(dir.join("remote.wav")).unwrap();
        assert!(change(&dir, "uk").is_err(), "ended, the audio gone");
        assert_eq!(meta_language(&dir), Some(id("en")), "a refused change leaves the language");
    }

    #[test]
    fn a_transcript_in_another_language_than_the_calls_is_refused() {
        let (_env, _) = crate::paths::testenv::folders("call-language-check");
        let dir = crate::paths::home().join("calls/2026-11-09_100000-x");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), r#"{"language": "uk"}"#).unwrap();
        let seg = |l: &str| crate::transcribe::Segment::from_piece("me", 0.0, l, crate::transcribe::Piece::new(0.0, 1.0, "x".into(), vec![]), None);
        assert!(check_segments(&dir, &[seg("uk"), seg("uk")]).is_ok());
        assert!(check_segments(&dir, &[seg("uk"), seg("en")]).is_err(), "changed to uk as it ended: done again");
        assert!(check_segments(&dir, &[]).is_ok(), "nothing said");
    }

    #[test]
    fn a_call_s_language_is_changed_by_hand_only_to_a_known_one() {
        let (_env, _) = crate::paths::testenv::folders("call-language-set");
        let dir = crate::paths::home().join("calls/2026-11-09_100000-x");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("meta.json"), r#"{"title": "x", "language": "en", "languages": ["en"]}"#).unwrap();
        assert_eq!(set(&dir, " RU ").unwrap(), "ru");
        let m: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
        assert_eq!((m["language"].as_str(), m["languages"][0].as_str(), m["title"].as_str()), (Some("ru"), Some("ru"), Some("x")));
        assert!(set(&dir, "klingon").is_err());
        assert_eq!(meta_language(&dir), Some(id("ru")), "unchanged by a refused one");
    }
}

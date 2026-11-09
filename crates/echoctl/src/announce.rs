//! Announcements: a release's one-time banner in the window (a deprecation, a new feature), shown to the users it
//! applies to until they answer it. A new one is an entry in `LIST` (its buttons name tray actions) and its condition in
//! `applies`; an answer is kept in `Config::announcements_seen`, so it never comes back.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config::Config;

struct Announcement {
    id: &'static str,
    title: &'static str,
    text: &'static str,
    /// (label, the tray action it runs through `window::settings_action` with its argument, or "" for none): the
    /// first is the main one; every button answers it.
    buttons: &'static [(&'static str, &'static str, &'static str)],
}

const LIST: &[Announcement] = &[Announcement {
    id: "whisper-deprecated",
    title: "Whisper is deprecated.",
    text: "It weighs on the computer during calls. Parakeet v3 is lighter and more accurate on English calls; calls in \
           other languages keep using Whisper until it's replaced.",
    buttons: &[("Switch to Parakeet v3", "switch_model", crate::models::PARAKEET), ("Not now", "", "")],
}];

/// Does `a` concern this user now?
fn applies(a: &Announcement, cfg: &Config) -> bool {
    match a.id {
        // The active model is Whisper, and Parakeet runs in this build.
        "whisper-deprecated" => {
            let m = |n: &str| crate::models::find(n).ok();
            cfg.model.as_deref().and_then(m).is_some_and(|m| m.engine == crate::models::Engine::Whisper)
                && m(crate::models::PARAKEET).is_some_and(crate::models::runnable)
        }
        _ => false,
    }
}

/// The announcement the window shows now: the first not answered yet that applies (`{id, title, text, buttons}`).
pub fn current() -> Value {
    let Ok(cfg) = Config::load() else { return Value::Null };
    LIST.iter()
        .find(|a| !cfg.announcements_seen.iter().any(|s| s == a.id) && applies(a, &cfg))
        .map(|a| json!({ "id": a.id, "title": a.title, "text": a.text, "buttons": a.buttons.iter().map(|(label, act, arg)| json!({ "label": label, "action": act, "arg": arg })).collect::<Vec<_>>() }))
        .unwrap_or(Value::Null)
}

/// An announcement answered (any of its buttons): kept as seen, never shown again. The button's own action, if any,
/// the window runs as the tray's (`window::settings_action`).
pub fn answer(id: &str) -> Result<()> {
    if !LIST.iter().any(|a| a.id == id) {
        bail!("no announcement '{id}'");
    }
    let mut cfg = Config::load()?;
    if !cfg.announcements_seen.iter().any(|s| s == id) {
        cfg.announcements_seen.push(id.to_string());
        cfg.save()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_announcement_shows_to_whom_it_applies_until_answered_once() {
        let (_env, _) = crate::paths::testenv::folders("announce");
        Config { model: Some("nemotron-3.5-0.6b".into()), ..Default::default() }.save().unwrap();
        assert!(current().is_null(), "not on Whisper: nothing to say");
        Config { model: Some("large-v3-turbo-q5_0".into()), ..Default::default() }.save().unwrap();
        if !crate::models::find(crate::models::PARAKEET).is_ok_and(crate::models::runnable) {
            assert!(current().is_null(), "no Parakeet in this build: nothing to switch to");
            return;
        }
        let a = current();
        assert_eq!((a["id"].as_str(), a["buttons"][0]["action"].as_str()), (Some("whisper-deprecated"), Some("switch_model")));
        assert!(answer("nope").is_err());
        answer("whisper-deprecated").unwrap();
        assert!(current().is_null(), "answered: never again");
        assert_eq!(Config::load().unwrap().model.as_deref(), Some("large-v3-turbo-q5_0"), "answering changes nothing else");
    }
}

//! Announcements: a release's one-time banner in the window (a deprecation, a new feature), shown to the users it
//! applies to until they answer it. A new one is an entry in `LIST`, its condition in `applies` and its buttons'
//! effects in `act`; an answer is kept in `Config::announcements_seen`, so it never comes back.

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::config::Config;

struct Announcement {
    id: &'static str,
    title: &'static str,
    text: &'static str,
    /// (action, label): the first is the main one; every button answers it.
    buttons: &'static [(&'static str, &'static str)],
}

const LIST: &[Announcement] = &[Announcement {
    id: "whisper-deprecated",
    title: "Whisper is deprecated.",
    text: "It weighs on the computer during calls. Parakeet v3 is lighter and more accurate on English calls; calls in \
           other languages keep using Whisper until it's replaced.",
    buttons: &[("switch-parakeet", "Switch to Parakeet v3"), ("later", "Not now")],
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

/// What a button does besides answering.
fn act(action: &str) {
    if action == "switch-parakeet" {
        crate::models::switch_in_background(crate::models::PARAKEET);
    }
}

/// The announcement the window shows now: the first not answered yet that applies (`{id, title, text, buttons}`).
pub fn current() -> Value {
    let Ok(cfg) = Config::load() else { return Value::Null };
    LIST.iter()
        .find(|a| !cfg.announcements_seen.iter().any(|s| s == a.id) && applies(a, &cfg))
        .map(|a| json!({ "id": a.id, "title": a.title, "text": a.text, "buttons": a.buttons.iter().map(|(act, label)| json!({ "action": act, "label": label })).collect::<Vec<_>>() }))
        .unwrap_or(Value::Null)
}

/// An announcement answered with one of its buttons: kept as seen, then the button's effect.
pub fn answer(id: &str, action: &str) -> Result<()> {
    let Some(a) = LIST.iter().find(|a| a.id == id) else { bail!("no announcement '{id}'") };
    if !a.buttons.iter().any(|(b, _)| *b == action) {
        bail!("'{action}' isn't one of its buttons");
    }
    let mut cfg = Config::load()?;
    if !cfg.announcements_seen.iter().any(|s| s == id) {
        cfg.announcements_seen.push(id.to_string());
        cfg.save()?;
    }
    act(action);
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
        assert_eq!((a["id"].as_str(), a["buttons"][0]["action"].as_str()), (Some("whisper-deprecated"), Some("switch-parakeet")));
        assert!(answer("whisper-deprecated", "delete-everything").is_err(), "only its own buttons");
        assert!(answer("nope", "later").is_err());
        answer("whisper-deprecated", "later").unwrap();
        assert!(current().is_null(), "answered: never again");
        assert_eq!(Config::load().unwrap().model.as_deref(), Some("large-v3-turbo-q5_0"), "Not now changes nothing");
    }
}

//! An address the user typed for a plugin (`ctx.http.askAddress`, `uses.network.ask`): Echo's own box — a system
//! dialog, never the plugin's page — asks for it, and that address alone may then be fetched once by that plugin, soon
//! after, as a plain GET with no headers or body and no redirect followed (`jshost::fetch`). Kept in this process's memory only: nothing a plugin writes or sends can make or widen it.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde_json::{json, Value};

/// How long a typed address waits for its one fetch.
const FOR: Duration = Duration::from_secs(600);

/// The address each plugin was last given, and when.
static TYPED: Mutex<BTreeMap<String, (String, Instant)>> = Mutex::new(BTreeMap::new());

/// `ctx.http.askAddress()`: Echo's box asks the user, in Echo's words, for the address of a page this plugin fetches
/// once. The address typed (checked), or null when the user cancelled.
pub(super) fn ask(id: &str) -> Result<Value> {
    super::uses_flag(id, "network", "ask")?;
    if crate::notes::headless() {
        bail!("Echo asks you for an address only in your own window or terminal, not inside a run Echo started");
    }
    let name = crate::plugins::enabled().into_iter().find(|p| p.id == id).map(|p| p.name).unwrap_or_else(|| id.to_string());
    let prompt: String = format!("{name} asks for the address of a web page to fetch once. Type or paste it:").chars().take(200).collect();
    let Some(typed) = answer(&prompt) else { return Ok(Value::Null) };
    let url = checked(&typed)?;
    TYPED.lock().unwrap_or_else(|e| e.into_inner()).insert(id.to_string(), (url.clone(), Instant::now()));
    Ok(json!(url))
}

/// What the user typed in Echo's box. A development build's tests answer it with `ECHOCTL_ASK_ADDRESS` (no dialog).
fn answer(prompt: &str) -> Option<String> {
    #[cfg(debug_assertions)]
    if let Some(v) = std::env::var_os("ECHOCTL_ASK_ADDRESS") {
        return Some(v.to_string_lossy().into_owned()).filter(|v| !v.trim().is_empty());
    }
    crate::app::ask_text(prompt, "", "Fetch")
}

/// A typed address as Echo takes it: `http://` or `https://`, a host, no user part, no spaces, quotes or control
/// characters, 2,000 characters at most.
fn checked(typed: &str) -> Result<String> {
    let url = typed.trim();
    let scheme_ok = ["http://", "https://"].iter().any(|s| url.get(..s.len()).is_some_and(|p| p.eq_ignore_ascii_case(s)));
    let rest = url.split_once("://").map_or("", |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if !scheme_ok || url.chars().count() > 2000 || authority.is_empty() || authority.starts_with(':') || authority.contains('@') || url.chars().any(|c| c.is_whitespace() || c.is_control() || "\"'<>\\`".contains(c)) {
        bail!("“{}” isn't a web address Echo fetches (http:// or https://, a host, no spaces)", url.chars().take(200).collect::<String>());
    }
    Ok(url.to_string())
}

/// May plugin `id` fetch `url` now on what the user typed? Yes once: the address it was given, exactly, within 10
/// minutes — taken by this answer.
pub(super) fn take(id: &str, url: &str) -> bool {
    let mut t = TYPED.lock().unwrap_or_else(|e| e.into_inner());
    t.retain(|_, (_, at)| at.elapsed() < FOR);
    if t.get(id).is_some_and(|(u, _)| u == url) {
        t.remove(id);
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_address_is_a_plain_web_address() {
        assert_eq!(checked("  https://docs.acme.example/a?b=1#c ").unwrap(), "https://docs.acme.example/a?b=1#c");
        assert_eq!(checked("HTTP://127.0.0.1:8080/").unwrap(), "HTTP://127.0.0.1:8080/");
        for bad in ["ftp://acme.example", "acme.example", "https://", "https:///x", "https://:80/x", "https://user@acme.example/", "https://acme.example/a b", "https://acme.example/\"x", "javascript:alert(1)", "file:///etc/passwd"] {
            assert!(checked(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_typed_address_is_fetched_once_by_its_plugin_only() {
        TYPED.lock().unwrap().insert("typed-a".into(), ("https://acme.example/p".into(), Instant::now()));
        assert!(!take("typed-b", "https://acme.example/p"), "another plugin");
        assert!(!take("typed-a", "https://acme.example/p2"), "another address");
        assert!(take("typed-a", "https://acme.example/p"));
        assert!(!take("typed-a", "https://acme.example/p"), "once");
        TYPED.lock().unwrap().insert("typed-a".into(), ("https://acme.example/p".into(), Instant::now() - FOR));
        assert!(!take("typed-a", "https://acme.example/p"), "too late");
    }
}

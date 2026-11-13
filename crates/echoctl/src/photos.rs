//! Colleagues' photos from Microsoft 365, shown in place of initials. Cached by address in `<config dir>/photos`
//! (shared by every data folder, as the sign-in is): `<address>.jpg`, or `<address>.none` when they have none (asked
//! again after a week). Asked only when the sign-in may read them (`graph::photos_allowed`, its own sign-in) and only
//! for addresses in the user's own mail domain: outside people's photos aren't in the user's directory.

// Without the window (`tray` off) only the CLI fetches one: the rest serves the window.
#![cfg_attr(not(feature = "tray"), allow(dead_code))]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{Map, Value};

/// How long a "they have no photo" stays before it is asked again.
const NONE_FOR: Duration = Duration::from_secs(7 * 24 * 3600);
/// How long "the sign-in may (not) read photos" is believed before the token is looked at again.
const ALLOWED_FOR: Duration = Duration::from_secs(600);

/// Where a person's photo is kept: their address, lower case, as the file name (only [a-z0-9._@-] kept).
pub fn file(email: &str) -> PathBuf {
    let key: String = email.trim().to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric() || "._@-".contains(*c)).collect();
    crate::paths::config_dir().join("photos").join(format!("{key}.jpg"))
}

/// Whether photos are shown: the switch (Settings → Calendar & chat), off by default.
fn on() -> bool {
    crate::config::Config::load().ok().and_then(|c| c.people_photos) == Some(true)
}

/// The user's own mail domain: their address in the settings, else the signed-in account's.
fn my_domain() -> Option<String> {
    let me = crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| m.email).or_else(crate::graph::account)?;
    me.rsplit_once('@').map(|(_, d)| d.trim().to_lowercase()).filter(|d| !d.is_empty())
}

/// Whether an address is worth asking a photo for: in the user's own domain (or one under it).
pub fn same_domain(email: &str, mine: &str) -> bool {
    email.trim().to_lowercase().rsplit_once('@').is_some_and(|(_, d)| d == mine || d.ends_with(&format!(".{mine}")))
}

static ALLOWED: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// Whether the sign-in may read photos, looked at once in a while (it reads the token file).
pub fn allowed() -> bool {
    let mut g = ALLOWED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, yes)) = *g {
        if at.elapsed() < ALLOWED_FOR {
            return yes;
        }
    }
    let yes = crate::graph::photos_allowed();
    *g = Some((Instant::now(), yes));
    yes
}

/// Whether the sign-in may read photos, looked at now (after a sign-in).
pub fn allowed_again() -> bool {
    *ALLOWED.lock().unwrap_or_else(|e| e.into_inner()) = None;
    allowed()
}

/// A name's address in the people directory (its name or an alias), when it is in the user's domain.
fn email_of(name: &str, people: &[crate::people::Person], mine: &str) -> Option<String> {
    let n = crate::people::norm(name);
    people
        .iter()
        .filter(|p| crate::people::norm(&p.name) == n || p.aliases.iter().any(|a| crate::people::norm(a) == n))
        .flat_map(|p| p.emails.iter())
        .find(|e| same_domain(e, mine))
        .map(|e| e.trim().to_lowercase())
}

/// For each name: its photo's address when there is one (the window shows `api/photo?e=`), null while it is being
/// fetched (in the background: the window asks again), else nothing. Only `fetch` names (people of the user's calls and
/// meetings) are fetched, and only when not kept yet; any other is answered from what is kept. Nothing at all when the switch is off (Settings →
/// Calendar & chat) or the sign-in may not read photos.
pub fn of_names(names: &[String], fetch_missing: bool) -> Map<String, Value> {
    let mut out = Map::new();
    if !on() {
        return out;
    }
    let Some(mine) = my_domain() else { return out };
    if !allowed() {
        return out;
    }
    let people = crate::people::directory().unwrap_or_default();
    let mut wanted = vec![];
    for name in names {
        let Some(email) = email_of(name, &people, &mine) else { continue };
        let jpg = file(&email);
        if jpg.exists() {
            out.insert(name.clone(), Value::String(email));
        } else if fetch_missing && !cooling() && !jpg.with_extension("none").metadata().and_then(|m| m.modified()).is_ok_and(|t| SystemTime::now().duration_since(t).unwrap_or_default() < NONE_FOR) {
            out.insert(name.clone(), Value::Null);
            wanted.push(email);
        }
    }
    fetch(wanted);
    out
}

/// Between two photo requests: Microsoft 365 answers a burst with 502/503.
const PACE: Duration = Duration::from_millis(1500);
/// After a refusal (429, 5xx, the network): no photo is asked for this long, doubled each time in a row, up to 30 min.
const COOL_FIRST: Duration = Duration::from_secs(60);
const COOL_MAX: Duration = Duration::from_secs(30 * 60);

/// The photos to fetch, the one worker that fetches them, and the pause after a refusal.
struct Queue {
    todo: BTreeSet<String>,
    working: bool,
    cool_until: Option<Instant>,
    cool: Option<Duration>,
}
static QUEUE: Mutex<Queue> = Mutex::new(Queue { todo: BTreeSet::new(), working: false, cool_until: None, cool: None });

/// The pause after a refusal: the first, else twice the last, at most COOL_MAX.
fn next_cool(last: Option<Duration>) -> Duration {
    last.map_or(COOL_FIRST, |c| (c * 2).min(COOL_MAX))
}

/// Whether photos are paused after a refusal.
fn cooling() -> bool {
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).cool_until.is_some_and(|t| Instant::now() < t)
}

/// Queues these photos for the one worker, which asks them one at a time, a pause apart; a refusal stops it and pauses
/// every photo request (the queue is dropped: the window asks again later).
fn fetch(emails: Vec<String>) {
    {
        let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        q.todo.extend(emails);
        if q.working || q.todo.is_empty() {
            return;
        }
        q.working = true;
    }
    std::thread::spawn(|| loop {
        let next = {
            let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
            match q.todo.pop_first() {
                Some(e) => e,
                None => {
                    q.working = false;
                    return;
                }
            }
        };
        let jpg = file(&next);
        if let Some(dir) = jpg.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let got = crate::graph::photo(&next, &jpg);
        let mut q = QUEUE.lock().unwrap_or_else(|e| e.into_inner());
        match got {
            Ok(Some(true)) => {
                q.cool = None;
                drop(q);
                shrink(&jpg);
            }
            Ok(Some(false)) => {
                q.cool = None;
                drop(q);
                let _ = std::fs::write(jpg.with_extension("none"), b"");
            }
            // Not allowed (the permission was taken back): nothing more to ask until the token says otherwise.
            Ok(None) => {
                *ALLOWED.lock().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), false));
                q.todo.clear();
                q.working = false;
                return;
            }
            Err(_) => {
                let cool = next_cool(q.cool);
                q.cool = Some(cool);
                q.cool_until = Some(Instant::now() + cool);
                q.todo.clear();
                q.working = false;
                return;
            }
        }
        std::thread::sleep(PACE);
    });
}

/// A photo comes as big as it was uploaded (often 300–600 px): kept at 96 px, enough for a face of 20–32 px.
fn shrink(jpg: &std::path::Path) {
    #[cfg(target_os = "macos")]
    {
        use crate::app::NoWindow;
        let _ = std::process::Command::new("sips").no_window().args(["-Z", "96"]).arg(jpg).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = jpg;
}

/// A cached photo by its address (only one `of_names` handed out: in the cache folder, by its own name), while the
/// switch is on.
pub fn bytes(email: &str) -> Option<Vec<u8>> {
    on().then(|| std::fs::read(file(email)).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_pauses_photos_longer_each_time_in_a_row() {
        let mins = |d: Duration| d.as_secs() / 60;
        let mut c = None;
        let seen: Vec<u64> = (0..7).map(|_| { let n = next_cool(c); c = Some(n); mins(n) }).collect();
        assert_eq!(seen, vec![1, 2, 4, 8, 16, 30, 30]);
    }

    #[test]
    fn only_the_users_own_domain_is_asked() {
        assert!(same_domain("Dana.Lee@Acme.example", "acme.example"));
        assert!(same_domain("pat@eu.acme.example", "acme.example"));
        assert!(!same_domain("omar@globex.example", "acme.example"));
        assert!(!same_domain("pat@notacme.example", "acme.example"));
        assert!(!same_domain("no-address", "acme.example"));
    }

    #[test]
    fn nothing_is_shown_or_asked_with_the_switch_off_or_without_a_sign_in() {
        let _env = crate::paths::testenv::folders("photos-off");
        let mut cfg = crate::config::Config::load().unwrap();
        cfg.me = Some(crate::meeting::Person { name: "Sam Rivera".into(), email: Some("sam@acme.example".into()) });
        cfg.save().unwrap();
        std::fs::create_dir_all(crate::projects::dir()).unwrap();
        std::fs::write(crate::projects::dir().join("people.json"), r#"[{"id":"p1","name":"Dana Lee","emails":["dana@acme.example"]}]"#).unwrap();
        std::fs::create_dir_all(file("x@y").parent().unwrap()).unwrap();
        std::fs::write(file("dana@acme.example"), b"jpeg").unwrap();
        let names = vec!["Dana Lee".to_string()];
        assert!(of_names(&names, true).is_empty(), "the switch is off by default, even with a photo kept");
        assert!(bytes("dana@acme.example").is_none(), "nor is a kept photo served");
        cfg.people_photos = Some(true);
        cfg.save().unwrap();
        assert!(of_names(&names, true).is_empty(), "on, but not signed in: the sign-in may not read photos");
        assert!(!file("dana@acme.example").with_extension("none").exists(), "and nothing was asked");
    }

    #[test]
    fn a_photos_file_is_its_address_with_nothing_else_in_its_name() {
        let _env = crate::paths::testenv::folders("photos-file");
        let f = file(" Dana.Lee@Acme.example/../x ");
        assert_eq!(f.file_name().unwrap(), "dana.lee@acme.example..x.jpg");
        assert_eq!(f.parent().unwrap(), crate::paths::config_dir().join("photos"));
    }
}

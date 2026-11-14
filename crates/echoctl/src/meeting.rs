//! Optional meeting context for a call: `<call>/meeting.json`.
//!
//! Echo never talks to calendars itself. Anything may write this file — the plugin's
//! `meeting-context` skill fills it from Microsoft Graph when the user has msgraph set up.
//! Without it, recording and transcription work exactly the same, just with less context.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::apps::MeetingApp;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingContext {
    /// Calendar event id (with `start`, identifies one occurrence of a recurring meeting)
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    /// The calendar series an occurrence belongs to (Graph `seriesMasterId`): a project's series rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_id: Option<String>,
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub organizer: Option<Person>,
    #[serde(default)]
    pub attendees: Vec<Person>,
    #[serde(default)]
    pub join_url: Option<String>,
    /// Product names, acronyms, jargon from the invite/agenda — helps Whisper spell them.
    #[serde(default)]
    pub vocabulary: Vec<String>,
    /// Where this came from, e.g. "msgraph"
    #[serde(default)]
    pub source: Option<String>,
    /// Languages chosen for this recording (the window's Record ▾ menu); empty = the configured ones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// The AI profile chosen for this recording (the window's Record menu); none = the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_profile: Option<String>,
    /// The user's answer to the invite (Graph `responseStatus.response`: accepted, organizer, tentativelyAccepted,
    /// notResponded, none); none when the source doesn't say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// How each invited person answered (Graph `status.response`, by lowercased address): accepted,
    /// tentativelyAccepted, declined, notResponded, none.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub answers: std::collections::BTreeMap<String, String>,
    /// How each person is invited (Graph `type`, by lowercased address): required or optional (rooms left out).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub invited_as: std::collections::BTreeMap<String, String>,
    /// Those who declined: not among `attendees`, shown last in the call's header.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declined: Vec<Person>,
}

pub const FILE: &str = "meeting.json";

impl MeetingContext {
    /// One the user accepted (or organises), one that asked for no answer (Graph's `none`: a town hall, a list's
    /// invite), or one whose source doesn't say: what Today and Coming up today list. Not: tentative, not answered.
    pub fn accepted(&self) -> bool {
        self.response.as_deref().is_none_or(|r| matches!(r, "accepted" | "organizer" | "none"))
    }

    /// The app the meeting is in, from its join link; none without a link or with one of no app Echo knows.
    pub fn app(&self) -> Option<MeetingApp> {
        self.join_url.as_deref().and_then(MeetingApp::from_url)
    }

    /// Itself when Echo can record it: a Teams meeting, one of no known app (recorded as a Teams call, as before
    /// #228), or a Google Meet one on a Mac (in Chrome, #230). A Zoom meeting, or a Meet one elsewhere, is refused with
    /// why: recording isn't available for it yet.
    pub fn recordable(&self) -> Result<&Self> {
        match self.app() {
            Some(MeetingApp::Meet) if cfg!(target_os = "macos") => Ok(self),
            Some(app @ (MeetingApp::Zoom | MeetingApp::Meet)) => {
                let title = self.subject.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("This meeting");
                anyhow::bail!("“{title}” is a {} meeting: recording isn't available for {} meetings yet", app.name(), app.name())
            }
            _ => Ok(self),
        }
    }

    /// Whether a call in `app` can be this meeting's: a meeting of no known app is any app's.
    pub fn is_in(&self, app: crate::apps::TargetApp) -> bool {
        self.app().is_none_or(|a| a == app.meeting_app())
    }

    /// Its join link when Teams' own features may use it (its transcript, Join & record): not a Zoom or Google Meet one.
    pub fn teams_join_url(&self) -> Option<&str> {
        self.join_url.as_deref().filter(|_| self.is_in(crate::apps::TargetApp::Teams))
    }

    pub fn load(call_dir: &Path) -> Result<Option<Self>> {
        let p = call_dir.join(FILE);
        if !p.exists() {
            return Ok(None);
        }
        let ctx = serde_json::from_slice(&fs::read(&p)?).with_context(|| format!("parsing {}", p.display()))?;
        Ok(Some(ctx))
    }

    pub fn read_file(path: &Path) -> Result<Self> {
        let bytes = if path.as_os_str() == "-" {
            let mut b = vec![];
            std::io::Read::read_to_end(&mut std::io::stdin(), &mut b)?;
            b
        } else {
            fs::read(path).with_context(|| format!("reading {}", path.display()))?
        };
        serde_json::from_slice(&bytes).context("meeting context must be JSON (see `echoctl meeting --help`)")
    }

    pub fn save(&self, call_dir: &Path) -> Result<()> {
        fs::write(call_dir.join(FILE), serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    /// The one other person on a two-person invite (organizer + attendees, minus the user):
    /// on such a call the remote track is that person. None for group calls or unknown invites.
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
    pub fn sole_other(&self, me: Option<&Person>) -> Option<String> {
        let is_me = |p: &Person| {
            me.is_some_and(|m| match (&m.email, &p.email) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => !m.name.is_empty() && m.name.eq_ignore_ascii_case(&p.name),
            })
        };
        let mut others: Vec<&Person> = vec![];
        for p in self.organizer.iter().chain(self.attendees.iter()) {
            let dup = others.iter().any(|o| match (&o.email, &p.email) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => o.name.eq_ignore_ascii_case(&p.name),
            });
            if !is_me(p) && !dup {
                others.push(p);
            }
        }
        match others.as_slice() {
            [one] if !one.name.trim().is_empty() => Some(one.name.trim().to_string()),
            _ => None,
        }
    }

    /// The meeting as it went: the people who joined (`participants`) in place of the invite's, who declined among
    /// them if they came. Each is named by the invite when invited, else by the people directory when it knows one
    /// person by that name, else by their name as Teams showed it, read as a name ("Lee, Dana (Partner)" → "Dana
    /// Lee"); the organiser stays the organiser only if they came.
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    pub fn as_joined(&self, names: &[String], directory: &[crate::people::Person]) -> MeetingContext {
        use crate::people::{display, norm};
        let invited: Vec<&Person> = self.organizer.iter().chain(&self.attendees).chain(&self.declined).collect();
        let named = |raw: &String| {
            let n = norm(raw);
            if let Some(p) = invited.iter().find(|p| norm(&p.name) == n) {
                return (*p).clone();
            }
            let mut known = directory.iter().filter(|d| std::iter::once(&d.name).chain(&d.aliases).any(|a| norm(a) == n));
            match (known.next(), known.next()) {
                (Some(d), None) => Person { name: display(&d.name), email: d.emails.first().cloned().or_else(|| d.id.contains('@').then(|| d.id.clone())) },
                _ => Person { name: display(raw), email: None },
            }
        };
        let came: Vec<Person> = names.iter().map(named).collect();
        let organizer = self.organizer.clone().filter(|o| came.contains(o));
        MeetingContext { attendees: came.into_iter().filter(|p| Some(p) != organizer.as_ref()).collect(), organizer, declined: vec![], ..self.clone() }
    }

    /// Everyone on the invite, organizer first, de-duplicated.
    pub fn people(&self) -> Vec<&str> {
        let mut out: Vec<&str> = vec![];
        for p in self.organizer.iter().chain(self.attendees.iter()) {
            let n = p.name.trim();
            if !n.is_empty() && !out.contains(&n) {
                out.push(n);
            }
        }
        out
    }
}

/// What Echo read of the call window while recording (`ax::SpeakingLog`): per tile or participants-list row, the
/// tokens added and removed.
#[cfg_attr(not(any(feature = "tray", feature = "transcribe")), allow(dead_code))]
pub const UI_LOG: &str = "speaking.jsonl";

/// A participants-list row's tokens say the person is in the call (muted or unmuted); invitees who never joined are
/// listed too ("No response").
#[cfg_attr(not(any(feature = "tray", feature = "transcribe")), allow(dead_code))]
pub fn in_call(tokens: &std::collections::BTreeSet<String>) -> bool {
    tokens.contains("t:muted") || tokens.contains("t:unmuted")
}

/// Who was in the meeting (but the user), from the call window's log: everyone shown on a video tile or in the
/// participants list as in the call, as Teams named them, first seen first. None without a participants list that
/// showed someone in the call: the window read off, a Google Meet call, the list never opened.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn joined(dir: &Path, me: Option<&str>) -> Option<Vec<String>> {
    use std::collections::{BTreeMap, BTreeSet};
    let text = fs::read_to_string(dir.join(UI_LOG)).ok()?;
    // Per element, in the order first seen: its name, the user's own tile, in the call at some point.
    let mut seen: Vec<(String, String, bool, bool)> = vec![];
    let mut now: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        let Some(k) = v["k"].as_str() else { continue };
        if v["gone"] == true {
            now.remove(k);
            continue;
        }
        let set = now.entry(k.to_string()).or_default();
        let strs = |key: &str| v[key].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect::<Vec<_>>();
        for d in strs("del") {
            set.remove(&d);
        }
        set.extend(strs("add"));
        let i = seen.iter().position(|e| e.0 == k).unwrap_or_else(|| {
            seen.push((k.to_string(), String::new(), false, false));
            seen.len() - 1
        });
        let e = &mut seen[i];
        if let Some(n) = v["n"].as_str() {
            e.1 = n.to_string();
        }
        e.2 |= v["me"] == true;
        e.3 |= k.starts_with("tile:") || (k.starts_with("roster:") && in_call(set));
    }
    if !seen.iter().any(|e| e.0.starts_with("roster:") && e.3) {
        return None;
    }
    let mine: BTreeSet<String> = seen.iter().filter(|e| e.2).map(|e| crate::people::norm(&e.1)).chain(me.map(crate::people::norm)).collect();
    let mut names = BTreeSet::new();
    Some(seen.into_iter().filter(|e| e.3 && !e.2).map(|e| e.1).filter(|n| {
        let k = crate::people::norm(n);
        !k.is_empty() && !mine.contains(&k) && names.insert(k)
    }).collect())
}

/// Who was in a recorded call but the user, as Teams named them: `participants` in its `meta.json`, recorded from the
/// call window's log (`joined`) the first time it is asked after the call ended. Empty without a participants list:
/// the invite stands for them.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn participants(dir: &Path) -> Vec<String> {
    let names = |m: &serde_json::Value| m["participants"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect::<Vec<_>>();
    let meta = crate::calls::read_json(&dir.join("meta.json"));
    if meta.get("participants").is_some() || meta["ended_at"].is_null() || !dir.join(UI_LOG).exists() {
        return names(&meta);
    }
    let me = crate::config::Config::load().ok().and_then(|c| c.me).map(|m| m.name);
    let found = joined(dir, me.as_deref()).unwrap_or_default();
    if let Err(e) = crate::calls::update_meta(dir, |m| {
        m["participants"] = serde_json::json!(found);
        Ok(())
    }) {
        eprintln!("recording who joined {}: {e:#}", dir.display());
    }
    found
}

/// `echoctl meeting attach`: add or replace a call's meeting context.
pub fn attach(call: &str, file: &Path) -> Result<()> {
    let dir = crate::calls::resolve_call(call)?;
    let ctx = MeetingContext::read_file(file)?;
    ctx.save(&dir)?;
    println!("{}", dir.join(FILE).display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, email: &str) -> Person {
        Person { name: name.into(), email: Some(email.into()) }
    }

    #[test]
    fn sole_other_names_the_one_remote_person() {
        let me = p("Alex Morgan", "alex@x.com");
        let one = MeetingContext { organizer: Some(p("Daniel Osei", "d@x.com")), attendees: vec![me.clone(), p("Daniel Osei", "D@x.com")], ..Default::default() };
        assert_eq!(one.sole_other(Some(&me)).as_deref(), Some("Daniel Osei"));
        let group = MeetingContext { attendees: vec![me.clone(), p("A", "a@x.com"), p("B", "b@x.com")], ..Default::default() };
        assert_eq!(group.sole_other(Some(&me)), None);
        assert_eq!(MeetingContext::default().sole_other(Some(&me)), None);
    }

    #[test]
    fn a_zoom_meeting_is_refused_for_recording_and_a_google_meet_one_only_off_a_mac() {
        let with = |url: Option<&str>| MeetingContext { subject: Some("Umbrella check-in".into()), join_url: url.map(str::to_owned), ..Default::default() };
        let zoom = with(Some("https://umbrella.zoom.us/j/81234567890"));
        let e = zoom.recordable().unwrap_err().to_string();
        assert_eq!(e, "“Umbrella check-in” is a Zoom meeting: recording isn't available for Zoom meetings yet");
        let meet = with(Some("https://meet.google.com/abc-defg-hij"));
        if cfg!(target_os = "macos") {
            assert!(meet.recordable().is_ok(), "recorded in Chrome");
        } else {
            assert!(meet.recordable().unwrap_err().to_string().contains("recording isn't available for Google Meet meetings yet"));
        }
        assert_eq!(zoom.teams_join_url(), None, "no Teams features on it");
        assert_eq!(meet.teams_join_url(), None, "no Teams features on it");
        for ok in [Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0"), Some("https://teams.example/ev-1"), None] {
            assert!(with(ok).recordable().is_ok(), "{ok:?}: Teams, or no app known, as before");
            assert_eq!(with(ok).teams_join_url(), ok);
        }
    }

    #[test]
    fn a_meeting_is_a_call_of_its_own_app_or_any_without_one() {
        use crate::apps::TargetApp::{Meet, Teams};
        let with = |url: Option<&str>| MeetingContext { join_url: url.map(str::to_owned), ..Default::default() };
        let (meet, teams) = (with(Some("https://meet.google.com/abc-defg-hij")), with(Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0")));
        assert!(meet.is_in(Meet) && !meet.is_in(Teams));
        assert!(teams.is_in(Teams) && !teams.is_in(Meet));
        assert!(with(None).is_in(Teams) && with(None).is_in(Meet), "no link: either");
        assert!(with(Some("https://teams.example/ev-1")).is_in(Meet), "no app known: either");
    }

    /// A call window's log as `ax::SpeakingLog` writes it: the user's tile, a remote tile, and the participants list
    /// with one who came muted, one "Last, First (Org)" who came, one invited who never did, and the user's own row.
    const LOG: &[&str] = &[
        r#"{"t":0.5,"k":"tile:Sam Rivera","n":"Sam Rivera","me":true,"add":["a"],"del":[]}"#,
        r#"{"t":0.5,"k":"tile:Omar Haddad","n":"Omar Haddad","add":["a","t:muted"],"del":[]}"#,
        r#"{"t":4.0,"k":"roster:8:orgid:omar","n":"Omar Haddad","add":["t:unmuted"],"del":[]}"#,
        r#"{"t":4.0,"k":"roster:8:orgid:ivan","n":"Ivan Petrov","add":["t:in a meeting","t:no response"],"del":[]}"#,
        r#"{"t":4.0,"k":"roster:8:orgid:sam","n":"Sam Rivera (You)","add":["t:unmuted"],"del":[]}"#,
        r#"{"t":60.0,"k":"roster:8:orgid:rita","n":"Gomez, Rita (Acme)","add":["t:muted"],"del":[]}"#,
        r#"{"t":90.0,"k":"roster:8:orgid:rita","gone":true}"#,
    ];

    #[test]
    fn who_joined_is_who_the_participants_list_showed_in_the_call() {
        let dir = std::env::temp_dir().join(format!("echo-joined-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(joined(&dir, None), None, "no log: the window wasn't read");
        fs::write(dir.join(UI_LOG), LOG.join("\n")).unwrap();
        // Ivan never came; the user is left out by their tile and their row; Rita came and left.
        assert_eq!(joined(&dir, None).unwrap(), ["Omar Haddad", "Gomez, Rita (Acme)"]);
        assert_eq!(joined(&dir, Some("Rita Gomez")).unwrap(), ["Omar Haddad"], "the user by their configured name, in any form");
        // Tiles only (the participants list never opened): a tile shows only some of who came, so nothing is said.
        fs::write(dir.join(UI_LOG), LOG[..2].join("\n")).unwrap();
        assert_eq!(joined(&dir, None), None);
        // A list where nobody shows as in the call (another language's labels): nothing either.
        fs::write(dir.join(UI_LOG), [LOG[1], LOG[3]].join("\n")).unwrap();
        assert_eq!(joined(&dir, None), None);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_people_who_came_are_named_by_the_invite_then_the_directory() {
        let invite = MeetingContext {
            organizer: Some(p("Dana Lee", "dana@acme.example")),
            attendees: vec![p("Omar Haddad", "omar@acme.example"), p("Ivan Petrov", "ivan@acme.example")],
            declined: vec![p("Yuki Tanaka", "yuki@acme.example")],
            subject: Some("Acme security review".into()),
            ..Default::default()
        };
        let dir_person = |id: &str, name: &str| crate::people::Person { id: id.into(), name: name.into(), emails: vec![id.into()], ..Default::default() };
        let directory = [dir_person("rita@acme.example", "Rita Gomez"), dir_person("kim@globex.example", "Kim Park"), dir_person("kim.park@initech.example", "Park, Kim")];
        let came: Vec<String> = ["Haddad, Omar (Acme)", "Gomez, Rita (Acme)", "Tanaka, Yuki", "Kim Park", "Nina Brooks (Guest)"].map(String::from).to_vec();
        let m = invite.as_joined(&came, &directory);
        let who: Vec<(&str, Option<&str>)> = m.attendees.iter().map(|x| (x.name.as_str(), x.email.as_deref())).collect();
        assert_eq!(who, [
            ("Omar Haddad", Some("omar@acme.example")),  // the invite's, though Teams wrote it "Last, First (Org)"
            ("Rita Gomez", Some("rita@acme.example")),   // not invited: the directory knows her
            ("Yuki Tanaka", Some("yuki@acme.example")),  // declined, came all the same
            ("Kim Park", None),                          // two Kim Parks in the directory: not guessed
            ("Nina Brooks", None),                       // known nowhere: her name, read as a name
        ]);
        assert_eq!(m.organizer, None, "Dana organised it and never came");
        assert!(m.declined.is_empty() && m.subject == invite.subject);
        let m = invite.as_joined(&["Lee, Dana (Partner)".to_string()], &[]);
        assert_eq!((m.organizer, m.attendees.len()), (Some(p("Dana Lee", "dana@acme.example")), 0), "the organiser came: still the organiser, once");
    }

    #[test]
    fn who_joined_is_recorded_in_meta_once_the_call_ended() {
        let (_env, root) = crate::paths::testenv::folders("meeting-participants");
        let dir = root.join("home/calls/2026-10-20_100000-review");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(UI_LOG), LOG.join("\n")).unwrap();
        fs::write(dir.join("meta.json"), r#"{"title":"Review","started_at":"2026-10-20T10:00:00+02:00","ended_at":null}"#).unwrap();
        assert!(participants(&dir).is_empty(), "still recording: nothing yet");
        assert!(!fs::read_to_string(dir.join("meta.json")).unwrap().contains("participants"));
        fs::write(dir.join("meta.json"), r#"{"title":"Review","started_at":"2026-10-20T10:00:00+02:00","ended_at":"2026-10-20T10:30:00+02:00"}"#).unwrap();
        assert_eq!(participants(&dir), ["Omar Haddad", "Gomez, Rita (Acme)"]);
        let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
        assert_eq!((meta["participants"].clone(), meta["title"].clone()), (serde_json::json!(["Omar Haddad", "Gomez, Rita (Acme)"]), serde_json::json!("Review")));
        // Recorded once: the log isn't read again.
        fs::remove_file(dir.join(UI_LOG)).unwrap();
        assert_eq!(participants(&dir).len(), 2);
        // A call whose window wasn't read: its meta untouched, nobody.
        let other = root.join("home/calls/2026-10-21_100000-meet");
        fs::create_dir_all(&other).unwrap();
        fs::write(other.join("meta.json"), r#"{"ended_at":"2026-10-21T10:30:00+02:00"}"#).unwrap();
        assert!(participants(&other).is_empty());
        assert!(!fs::read_to_string(other.join("meta.json")).unwrap().contains("participants"));
    }
}

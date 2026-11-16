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
    /// Lee"); the organiser stays the organiser only if they came. Names only: Teams' list carries no addresses. With a
    /// `partial` list the invitees not seen are kept too.
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    pub fn as_joined(&self, joined: &Joined, directory: &[crate::people::Person]) -> MeetingContext {
        let names = &joined.names;
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
        let mut came: Vec<Person> = names.iter().map(named).collect();
        // A log cut short: whoever was invited and not seen may have come after it stopped.
        if joined.partial {
            let more: Vec<Person> = self.organizer.iter().chain(&self.attendees).filter(|p| !came.iter().any(|c| c == *p || norm(&c.name) == norm(&p.name))).cloned().collect();
            came.extend(more);
        }
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
/// The log's format, its first line (`{"v": 2}`): from 2 a participants-list row keeps a "Last, First" name whole
/// (`ax::roster_name`). Older logs named "Lee, Dana (Partner)" "Lee": who joined is never read from them.
#[cfg_attr(not(any(target_os = "macos", windows, feature = "tray")), allow(dead_code))]
pub const UI_LOG_VERSION: u64 = 2;
/// Who joined is recorded only this long after the call ended: the window reader's last read is in the log by then.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
const SETTLE_S: i64 = 60;

/// A participants-list row's tokens say the person is in the call (muted or unmuted); invitees who never joined are
/// listed too ("No response").
#[cfg_attr(not(any(feature = "tray", feature = "transcribe")), allow(dead_code))]
pub fn in_call(tokens: &std::collections::BTreeSet<String>) -> bool {
    tokens.contains("t:muted") || tokens.contains("t:unmuted")
}

/// Who joined a meeting but the user, as Teams named them (empty: nobody else came). `partial`: the log reached its
/// size limit before the call ended, so someone may have come after it stopped.
#[derive(Debug, Clone, PartialEq)]
pub struct Joined {
    pub names: Vec<String>,
    pub partial: bool,
}

/// Who was in the meeting (but the user), from the call window's log: everyone shown on a video tile or in the
/// participants list as in the call, first seen first. None without a participants list that showed anyone in the
/// call (the user included): the window read off, a Google Meet call, the list never opened; and for a log older than
/// `UI_LOG_VERSION`.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn joined(dir: &Path, me: Option<&str>) -> Option<Joined> {
    use std::collections::{BTreeMap, BTreeSet};
    let text = fs::read_to_string(dir.join(UI_LOG)).ok()?;
    let version = text.lines().next().and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok()).and_then(|v| v["v"].as_u64());
    if version.is_none_or(|n| n < UI_LOG_VERSION) {
        return None;
    }
    // Per element, in the order first seen: its name, the user's own tile, in the call at some point.
    let mut seen: Vec<(String, String, bool, bool)> = vec![];
    let mut now: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut partial = false;
    for line in text.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        partial |= v["capped"] == true;
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
    let mut keys = BTreeSet::new();
    let names = seen.into_iter().filter(|e| e.3 && !e.2).map(|e| e.1).filter(|n| {
        let k = crate::people::norm(n);
        !k.is_empty() && !mine.contains(&k) && keys.insert(k)
    }).collect();
    Some(Joined { names, partial })
}

/// Who joined a recorded call (`joined`), from its `meta.json` (`meta`, already read): `participants` — the names, or
/// null when the window was read without a participants list — and `participants_partial`. Recorded there the first
/// time it is asked `SETTLE_S` after the call ended; a write that fails is remembered for the app's run, not read
/// again. None: no participants list, the invite stands for them.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn participants(dir: &Path, meta: &serde_json::Value) -> Option<Joined> {
    type Unsaved = std::collections::HashMap<std::path::PathBuf, Option<Joined>>;
    static UNSAVED: std::sync::Mutex<Option<Unsaved>> = std::sync::Mutex::new(None);
    if let Some(p) = meta.get("participants") {
        let names: Vec<String> = p.as_array()?.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect();
        return Some(Joined { names, partial: meta["participants_partial"] == true });
    }
    let ended = meta["ended_at"].as_str().and_then(|e| chrono::DateTime::parse_from_rfc3339(e).ok())?;
    if (chrono::Utc::now() - ended.with_timezone(&chrono::Utc)).num_seconds() < SETTLE_S || !dir.join(UI_LOG).exists() {
        return None;
    }
    if let Some(j) = UNSAVED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).get(dir) {
        return j.clone();
    }
    let me = crate::config::Config::load().ok().and_then(|c| c.me).map(|m| m.name);
    let found = joined(dir, me.as_deref());
    let saved = crate::calls::update_meta(dir, |m| {
        m["participants"] = found.as_ref().map_or(serde_json::Value::Null, |j| serde_json::json!(j.names));
        if found.as_ref().is_some_and(|j| j.partial) {
            m["participants_partial"] = true.into();
        }
        Ok(())
    });
    if let Err(e) = saved {
        eprintln!("recording who joined {}: {e:#}", dir.display());
        UNSAVED.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Default::default).insert(dir.to_path_buf(), found.clone());
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

    /// A call window's log as `ax::SpeakingLog` writes it: its version, the user's tile, a remote tile, and the
    /// participants list with one who came muted, one "Last, First (Org)" who came, one invited who never did, and the
    /// user's own row.
    const LOG: &[&str] = &[
        r#"{"v":2}"#,
        r#"{"t":0.5,"k":"tile:Sam Rivera","n":"Sam Rivera","me":true,"add":["a"],"del":[]}"#,
        r#"{"t":0.5,"k":"tile:Omar Haddad","n":"Omar Haddad","add":["a","t:muted"],"del":[]}"#,
        r#"{"t":4.0,"k":"roster:8:orgid:omar","n":"Omar Haddad","add":["t:unmuted"],"del":[]}"#,
        r#"{"t":4.0,"k":"roster:8:orgid:ivan","n":"Ivan Petrov","add":["t:in a meeting","t:no response"],"del":[]}"#,
        r#"{"t":4.0,"k":"roster:8:orgid:sam","n":"Sam Rivera (You)","add":["t:unmuted"],"del":[]}"#,
        r#"{"t":60.0,"k":"roster:8:orgid:rita","n":"Gomez, Rita (Acme)","add":["t:muted"],"del":[]}"#,
        r#"{"t":90.0,"k":"roster:8:orgid:rita","gone":true}"#,
    ];

    fn names(j: Option<Joined>) -> Option<Vec<String>> {
        j.map(|j| j.names)
    }

    #[test]
    fn who_joined_is_who_the_participants_list_showed_in_the_call() {
        let dir = std::env::temp_dir().join(format!("echo-joined-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let log = |lines: &[&str]| fs::write(dir.join(UI_LOG), lines.join("\n")).unwrap();
        assert_eq!(joined(&dir, None), None, "no log: the window wasn't read");
        log(LOG);
        // Ivan never came; the user is left out by their tile and their row; Rita came and left.
        assert_eq!(joined(&dir, None), Some(Joined { names: vec!["Omar Haddad".into(), "Gomez, Rita (Acme)".into()], partial: false }));
        assert_eq!(names(joined(&dir, Some("Rita Gomez"))).unwrap(), ["Omar Haddad"], "the user by their configured name, in any form");
        // Tiles only (the participants list never opened): a tile shows only some of who came, so nothing is said.
        log(&LOG[..3]);
        assert_eq!(joined(&dir, None), None);
        // A list where nobody shows as in the call (another language's labels): nothing either.
        log(&[LOG[0], LOG[2], LOG[4]]);
        assert_eq!(joined(&dir, None), None);
        // The user alone in the call: nobody else joined, which is not "no list".
        log(&[LOG[0], LOG[1], LOG[4], LOG[5]]);
        assert_eq!(joined(&dir, None), Some(Joined { names: vec![], partial: false }));
        // The log hit its size limit: what it saw, marked incomplete.
        log(&[LOG, &[r#"{"capped":true}"#]].concat());
        assert_eq!(joined(&dir, None).map(|j| (j.names.len(), j.partial)), Some((2, true)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_log_from_before_the_whole_names_is_never_read_for_who_joined() {
        let dir = std::env::temp_dir().join(format!("echo-joined-old-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        // The old reader: no version line, "Lee, Dana (Partner), Muted" written as "Lee" with the first name a token,
        // and the user's "Rivera, Sam" row as "Rivera".
        fs::write(dir.join(UI_LOG), [
            r#"{"t":0.5,"k":"tile:Sam Rivera","n":"Sam Rivera","me":true,"add":["a"],"del":[]}"#,
            r#"{"t":4.0,"k":"roster:8:orgid:dana","n":"Lee","add":["t:dana (partner)","t:muted"],"del":[]}"#,
            r#"{"t":4.0,"k":"roster:8:orgid:sam","n":"Rivera","add":["t:sam","t:unmuted"],"del":[]}"#,
        ].join("\n")).unwrap();
        assert_eq!(joined(&dir, Some("Sam Rivera")), None, "no list: the invite stays");
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
        let came = |names: &[&str], partial: bool| Joined { names: names.iter().map(|n| n.to_string()).collect(), partial };
        let m = invite.as_joined(&came(&["Haddad, Omar (Acme)", "Gomez, Rita (Acme)", "Tanaka, Yuki", "Kim Park", "Nina Brooks (Guest)"], false), &directory);
        let who = |m: &MeetingContext| m.attendees.iter().map(|x| (x.name.clone(), x.email.clone())).collect::<Vec<_>>();
        let pair = |n: &str, e: Option<&str>| (n.to_string(), e.map(str::to_owned));
        assert_eq!(who(&m), [
            pair("Omar Haddad", Some("omar@acme.example")), // the invite's, though Teams wrote it "Last, First (Org)"
            pair("Rita Gomez", Some("rita@acme.example")),  // not invited: the directory knows her
            pair("Yuki Tanaka", Some("yuki@acme.example")), // declined, came all the same
            pair("Kim Park", None),                         // two Kim Parks in the directory: not guessed
            pair("Nina Brooks", None),                      // known nowhere: her name, read as a name
        ]);
        assert_eq!(m.organizer, None, "Dana organised it and never came");
        assert!(m.declined.is_empty() && m.subject == invite.subject);
        let m = invite.as_joined(&came(&["Lee, Dana (Partner)"], false), &[]);
        assert_eq!((m.organizer, m.attendees.len()), (Some(p("Dana Lee", "dana@acme.example")), 0), "the organiser came: still the organiser, once");
        let m = invite.as_joined(&came(&[], false), &[]);
        assert_eq!((m.organizer, m.attendees.len()), (None, 0), "nobody else came");
        // A list cut short: the invitees not seen stay (not who declined).
        let m = invite.as_joined(&came(&["Haddad, Omar (Acme)"], true), &[]);
        assert_eq!((m.organizer.as_ref().map(|o| o.name.clone()), who(&m)), (Some("Dana Lee".into()), vec![pair("Omar Haddad", Some("omar@acme.example")), pair("Ivan Petrov", Some("ivan@acme.example"))]));
    }

    #[test]
    fn who_joined_is_recorded_in_meta_once_the_call_ended() {
        let (_env, root) = crate::paths::testenv::folders("meeting-participants");
        let dir = root.join("home/calls/2026-10-20_100000-review");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(UI_LOG), LOG.join("\n")).unwrap();
        let meta = |m: serde_json::Value| {
            fs::write(dir.join("meta.json"), m.to_string()).unwrap();
            m
        };
        let read = || serde_json::from_slice::<serde_json::Value>(&fs::read(dir.join("meta.json")).unwrap()).unwrap();
        let m = meta(serde_json::json!({"title": "Review", "ended_at": null}));
        assert_eq!(participants(&dir, &m), None, "still recording: nothing yet");
        let just = (chrono::Local::now() - chrono::Duration::seconds(5)).to_rfc3339();
        let m = meta(serde_json::json!({"title": "Review", "ended_at": just}));
        assert_eq!(participants(&dir, &m), None, "just ended: the window reader may still write its last read");
        assert!(read().get("participants").is_none());
        let m = meta(serde_json::json!({"title": "Review", "ended_at": "2026-10-20T10:30:00+02:00"}));
        assert_eq!(names(participants(&dir, &m)).unwrap(), ["Omar Haddad", "Gomez, Rita (Acme)"]);
        assert_eq!((read()["participants"].clone(), read()["title"].clone()), (serde_json::json!(["Omar Haddad", "Gomez, Rita (Acme)"]), serde_json::json!("Review")));
        // Recorded once: the log isn't read again.
        fs::remove_file(dir.join(UI_LOG)).unwrap();
        assert_eq!(names(participants(&dir, &read())).unwrap().len(), 2);
        // The window read without a participants list: null, kept, and the invite stands.
        fs::write(dir.join(UI_LOG), LOG[..3].join("\n")).unwrap();
        let m = meta(serde_json::json!({"ended_at": "2026-10-20T10:30:00+02:00"}));
        assert_eq!(participants(&dir, &m), None);
        assert_eq!(read()["participants"], serde_json::Value::Null);
        assert!(read().get("participants").is_some());
        // A call whose window wasn't read: its meta untouched, nobody.
        fs::remove_file(dir.join(UI_LOG)).unwrap();
        let m = meta(serde_json::json!({"ended_at": "2026-10-21T10:30:00+02:00"}));
        assert_eq!(participants(&dir, &m), None);
        assert!(read().get("participants").is_none());
    }
}

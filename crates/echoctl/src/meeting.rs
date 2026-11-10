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

    /// Itself when Echo can record it: a Teams meeting, or one of no known app (recorded as a Teams call, as before
    /// #228). A Zoom or Google Meet meeting is refused with why: recording isn't available for it yet.
    pub fn recordable(&self) -> Result<&Self> {
        match self.app() {
            Some(app @ (MeetingApp::Zoom | MeetingApp::Meet)) => {
                let title = self.subject.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or("This meeting");
                anyhow::bail!("“{title}” is a {} meeting: recording isn't available for {} meetings yet", app.name(), app.name())
            }
            _ => Ok(self),
        }
    }

    /// Its join link when Teams' own features may use it (its transcript, Join & record): not a Zoom or Google Meet one.
    pub fn teams_join_url(&self) -> Option<&str> {
        self.join_url.as_deref().filter(|_| self.recordable().is_ok())
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
    fn only_a_zoom_or_google_meet_meeting_is_refused_for_recording() {
        let with = |url: Option<&str>| MeetingContext { subject: Some("Umbrella check-in".into()), join_url: url.map(str::to_owned), ..Default::default() };
        let zoom = with(Some("https://umbrella.zoom.us/j/81234567890"));
        let e = zoom.recordable().unwrap_err().to_string();
        assert_eq!(e, "“Umbrella check-in” is a Zoom meeting: recording isn't available for Zoom meetings yet");
        assert!(with(Some("https://meet.google.com/abc-defg-hij")).recordable().unwrap_err().to_string().contains("recording isn't available for Google Meet meetings yet"));
        assert_eq!(zoom.teams_join_url(), None, "no Teams features on it");
        for ok in [Some("https://teams.microsoft.com/l/meetup-join/19%3ameeting_1%40thread.v2/0"), Some("https://teams.example/ev-1"), None] {
            assert!(with(ok).recordable().is_ok(), "{ok:?}: Teams, or no app known, as before");
            assert_eq!(with(ok).teams_join_url(), ok);
        }
    }
}

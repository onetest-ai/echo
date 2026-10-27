//! Optional meeting context for a call: `<call>/meeting.json`.
//!
//! Echo never talks to calendars itself. Anything may write this file — the plugin's
//! `meeting-context` skill fills it from Microsoft Graph when the user has msgraph set up.
//! Without it, recording and transcription work exactly the same, just with less context.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

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
}

pub const FILE: &str = "meeting.json";

impl MeetingContext {
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
    #[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
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
}

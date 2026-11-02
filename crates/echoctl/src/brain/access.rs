//! Which AI profile a source's text may go to (strict routing, as for a call's own jobs: never
//! another profile). A call belongs to the profile pinned in its `meta.json` (one recorded before
//! profiles: the default; one that can't be read: none — it goes nowhere). Mail, channel posts and
//! folder documents belong to their threads' profiles (a thread's own, else the default). Everything
//! that sends the Brain's text to a model checks this: the MCP server a Claude session gets (its
//! profile, and for a call's session that call's client or threads), and the Brain's background jobs
//! (insights, the taxonomy, the page reader, the numbers proposal) on the default profile.

use super::store;
use anyhow::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The profiles a source may go to.
pub fn profiles_of(source: &str, refs: &Value, threads: &BTreeMap<String, String>, default: &str) -> BTreeSet<String> {
    if source.starts_with("call/") {
        let call = refs["call"].as_str().unwrap_or("");
        return crate::calls::resolve_call(call).ok().and_then(|d| crate::ai::call_profile_id(&d).ok()).into_iter().collect();
    }
    let ids: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
    if ids.is_empty() {
        return [default.to_string()].into_iter().collect();
    }
    ids.iter().map(|t| threads.get(*t).cloned().unwrap_or_else(|| default.to_string())).collect()
}

/// Each thread's profile (its own, else the default).
pub fn thread_profiles(default: &str) -> BTreeMap<String, String> {
    crate::projects::list().into_iter().map(|p| (p.id, p.ai_profile.unwrap_or_else(|| default.to_string()))).collect()
}

/// What a reader (a Claude session, a background job) may see of the Brain.
#[derive(Debug, Clone, Default)]
pub struct Access {
    /// The profile it runs on: only sources of this profile.
    pub profile: String,
    /// A call's session: its client, else its threads, else only the call itself.
    pub client: Option<String>,
    pub threads: Vec<String>,
    /// That call's source (`call/<stamp>`).
    pub call: Option<String>,
}

impl Access {
    /// A background job on the default profile, over everything of that profile.
    pub fn default_profile() -> Access {
        Access { profile: crate::ai::default_id(), ..Default::default() }
    }

    fn narrowed(&self) -> bool {
        self.client.is_some() || !self.threads.is_empty() || self.call.is_some()
    }

    /// The documents it may read.
    pub fn sources(&self, c: &Connection) -> Result<BTreeSet<String>> {
        let default = crate::ai::default_id();
        let threads = thread_profiles(&default);
        let docs: Vec<(String, Option<String>)> = c.prepare("SELECT source, refs FROM documents")?.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
        let mut out = BTreeSet::new();
        for (src, refs) in docs {
            let refs: Value = refs.as_deref().and_then(|r| serde_json::from_str(r).ok()).unwrap_or(Value::Null);
            if !profiles_of(&src, &refs, &threads, &default).contains(&self.profile) {
                continue;
            }
            if self.narrowed() {
                let doc_threads: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str()).collect();
                let doc_clients: Vec<&str> = refs["threads"].as_array().into_iter().flatten().filter_map(|t| t["client"].as_str()).chain(refs["client"].as_str()).collect();
                let ok = Some(&src) == self.call.as_ref()
                    || match &self.client {
                        Some(cl) => doc_clients.contains(&cl.as_str()),
                        None => doc_threads.iter().any(|t| self.threads.iter().any(|x| x == t)),
                    };
                if !ok {
                    continue;
                }
            }
            out.insert(src);
        }
        Ok(out)
    }

    /// The sections it may read (by id).
    pub fn chunks(&self, c: &Connection) -> Result<BTreeSet<i64>> {
        let sources = self.sources(c)?;
        let mut out = BTreeSet::new();
        let mut st = c.prepare("SELECT id, source FROM chunks")?;
        for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
            let (id, src) = row?;
            if sources.contains(&src) {
                out.insert(id);
            }
        }
        Ok(out)
    }

    /// The scope a search runs in when it is narrowed to one client or thread.
    pub fn scope(&self) -> store::Scope<'_> {
        match (&self.client, self.threads.as_slice()) {
            (Some(c), _) => Some(("client", c.as_str())),
            (None, [t]) => Some(("thread", t.as_str())),
            _ => None,
        }
    }

    /// The threads whose people it may see: those on its profile, narrowed as its documents are (a
    /// call's session with no client or thread sees nobody).
    pub fn people_threads(&self) -> Vec<String> {
        let default = crate::ai::default_id();
        let profiles = thread_profiles(&default);
        crate::projects::list()
            .into_iter()
            .filter(|p| profiles.get(&p.id) == Some(&self.profile))
            .filter(|p| match (&self.client, self.threads.is_empty(), &self.call) {
                (Some(c), _, _) => p.client.as_deref() == Some(c.as_str()),
                (None, false, _) => self.threads.contains(&p.id),
                (None, true, Some(_)) => false,
                (None, true, None) => true,
            })
            .map(|p| p.id)
            .collect()
    }

    /// The clients whose numbers it may read: its own client, or (not narrowed) every client whose
    /// threads all run on its profile.
    pub fn numbers_clients(&self) -> Vec<String> {
        if let Some(c) = &self.client {
            return if client_profile_ok(c, &self.profile) { vec![c.clone()] } else { vec![] };
        }
        if self.narrowed() {
            return vec![];
        }
        super::numbers::clients().into_iter().filter(|c| client_profile_ok(c, &self.profile)).collect()
    }
}

/// A client's documents and workbooks may go to `profile` when every one of its threads runs on it.
pub fn client_profile_ok(client: &str, profile: &str) -> bool {
    let default = crate::ai::default_id();
    crate::projects::list()
        .into_iter()
        .filter(|p| p.client.as_deref() == Some(client))
        .all(|p| p.ai_profile.unwrap_or_else(|| default.clone()) == profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn documents_belong_to_their_threads_profiles() {
        let threads: BTreeMap<String, String> = [("a".to_string(), "acme-account".to_string()), ("b".to_string(), "system".to_string())].into_iter().collect();
        let refs = json!({"threads": [{"id": "a", "client": "acme"}]});
        assert_eq!(profiles_of("mail/x", &refs, &threads, "system"), ["acme-account".to_string()].into_iter().collect());
        let both = json!({"threads": [{"id": "a"}, {"id": "b"}]});
        assert_eq!(profiles_of("file/y", &both, &threads, "system").len(), 2);
        assert_eq!(profiles_of("file/z", &json!({}), &threads, "system"), ["system".to_string()].into_iter().collect());
        // A call that can't be found goes nowhere.
        assert!(profiles_of("call/x", &json!({"call": "no-such-call-folder"}), &threads, "system").is_empty());
        assert_eq!(Access { client: Some("acme".into()), ..Default::default() }.scope(), Some(("client", "acme")));
    }
}

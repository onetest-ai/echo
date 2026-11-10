//! People: suggestions from the sync's people pass (split out of `people.rs`, moved as it was).

use super::*;

// ── suggestions from the sync's people pass ───────────────────────────────────────────────
// The agent proposes, with the words; the CLI checks the words and the person; the user decides.
// Nothing here changes a person, a placement or an organisation until the user says yes.

/// What a proposal may say about a person (in this thread, or in the directory) or an organisation.
pub const SUGGEST_FIELDS: [&str; 9] = ["role", "stance", "influence", "interest", "title", "group", "reports_to", "left", "relation"];

pub(super) fn suggestions_file(project: &str) -> PathBuf {
    projects::dir().join(project).join("people-suggestions.json")
}

pub fn suggestions(project: &str) -> Vec<Value> {
    std::fs::read(suggestions_file(project)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// What the people pass reads: the thread's people and organisations as they stand, the calls of
/// this run (their readable files), and what was already proposed or turned down.
pub fn context(project: &str, items: &[String]) -> Result<Value> {
    let v = of_thread(project, chrono::Local::now().date_naive())?;
    let l = crate::ledger::Ledger::of(project)?;
    let name_of = |k: &str| v["people"].as_array().into_iter().flatten().find(|x| x["key"] == k).and_then(|x| x["name"].as_str().map(str::to_owned));
    let people: Vec<Value> = v["people"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|x| x["touches"].as_u64().unwrap_or(0) > 0 || x["in_thread"] == true)
        .map(|x| {
            let pl = x["places"].get(0).cloned().unwrap_or(json!({}));
            json!({"name": x["name"], "email": x["email"], "organisation": x["org"]["name"].as_str().unwrap_or(match x["side"].as_str() { Some("yours") => "the user's company", _ => "" }),
                   "in_thread": x["in_thread"], "title": x["title"], "group": pl["group"].as_str().or(x["group"].as_str()), "reports_to": x["reports_to"].as_str().and_then(name_of),
                   "role": pl["role"], "role_by": pl["role_by"], "role_at": pl["role_at"], "stance": pl["stance"], "influence": pl["influence"], "interest": pl["interest"], "calls": x["calls"],
                   "about": if x["about_by"] == "you" || x["about_thread"] == project { x["about"].clone() } else { Value::Null }, "about_is_users": x["about_by"] == "you",
                   "facts": x["facts"].as_array().into_iter().flatten().take(12).map(|f| json!({"date": f["date"], "fact": f["text"]})).collect::<Vec<_>>()})
        })
        .collect();
    let orgs: Vec<Value> = v["orgs"].as_array().into_iter().flatten().map(|o| json!({"name": o["name"], "relations": o["relations"], "parent": o["parent"]})).collect();
    let known = projects::orgs()?.into_iter().map(|c| c.name).collect::<Vec<_>>();
    let said: Vec<Value> = suggestions(project).into_iter().map(|x| json!({"who": x["who"], "field": x["field"], "value": x["value"], "state": x["state"]})).collect();
    Ok(json!({
        "project": project, "people": people, "organisations": orgs, "known_organisations": known,
        "me_profile": crate::calls::me_profile(),
        "items": items.iter().map(|i| json!({"id": i, "file": l.evidence_md(i).display().to_string().replace('\\', "/")})).collect::<Vec<_>>(),
        "already_proposed": said,
        "fields": SUGGEST_FIELDS.iter().chain(&["person", "fact", "about"]).collect::<Vec<_>>(), "stances": projects::STANCES, "relations": projects::RELATIONS,
    }))
}

/// Find who a proposal is about among the thread's people: by address, by cleaned full name, else a
/// first name only one of them has.
pub(super) fn find_person<'a>(people: &'a [Value], who: &str) -> Option<&'a Value> {
    let w = who.trim().to_lowercase();
    if w.contains('@') {
        return people.iter().find(|x| x["email"].as_str().is_some_and(|e| e == w) || x["also"].as_array().into_iter().flatten().any(|e| e == w.as_str()));
    }
    let n = norm(who);
    if let Some(x) = people.iter().find(|x| norm(x["name"].as_str().unwrap_or("")) == n) {
        return Some(x);
    }
    let first: Vec<&Value> = people.iter().filter(|x| norm(x["name"].as_str().unwrap_or("")).split(' ').next() == Some(n.as_str())).collect();
    (first.len() == 1).then(|| first[0])
}

/// Check proposals for one evidence item and keep the good ones as suggestions: the words must be
/// one span of the item, the person (or organisation) known, the value valid and new. Returns
/// `{added, rejected: [{i, why}]}`.
pub fn apply_suggestions(project: &str, evid: &str, proposals: &[Value]) -> Result<Value> {
    let v = of_thread(project, chrono::Local::now().date_naive())?;
    let people = v["people"].as_array().cloned().unwrap_or_default();
    let clients = projects::orgs()?;
    let dir_people = directory()?;
    let me = MeIs::now();
    // Facts and who someone is go in at once (their words are checked; nothing to decide).
    let mut facts: Vec<(String, String, Option<String>, Value)> = vec![];
    let mut abouts: Vec<(String, String, Option<String>, String)> = vec![];
    let org_named = |n: &str| clients.iter().find(|c| norm(&c.name) == norm(n) || c.aliases.iter().any(|a| norm(a) == norm(n)) || c.id == n);
    let mut all = suggestions(project);
    let (mut added, mut rejected) = (0, vec![]);
    for (i, pr) in proposals.iter().enumerate() {
        let field = pr["field"].as_str().unwrap_or("");
        let value = match &pr["value"] {
            Value::String(x) => x.trim().to_string(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => String::new(),
        };
        let who = pr["who"].as_str().unwrap_or("");
        let why: Option<String> = (|| {
            if field == "fact" || field == "about" {
                let (key, name, email) = match find_person(&people, who) {
                    Some(x) => (x["key"].as_str().unwrap_or("").to_string(), x["name"].as_str().unwrap_or("").to_string(), x["email"].as_str().map(str::to_owned)),
                    // A mail's sender or recipient by address, not in a call yet — only one of this mail's.
                    None if field == "fact" && who.contains('@') && !who.contains(char::is_whitespace) => {
                        let e = who.trim().to_lowercase();
                        let item = crate::ledger::Ledger::of(project).ok().and_then(|l| l.evidence().get(evid).cloned()).unwrap_or_default();
                        let addrs: Vec<String> = std::iter::once(&item["from"]).chain(item["to"].as_array().into_iter().flatten()).filter_map(|p| p["email"].as_str().map(str::to_lowercase)).collect();
                        if !addrs.contains(&e) {
                            return Some(format!("{who:?} isn't among the thread's people nor this mail's sender or recipients"));
                        }
                        let name = pr["name"].as_str().map(display).filter(|n| !n.is_empty()).unwrap_or_else(|| e.clone());
                        (canon(&dir_people).get(&e).cloned().unwrap_or_else(|| e.clone()), name, Some(e))
                    }
                    None => return Some(format!("{who:?} isn't among the thread's people (use a name or an address from people-context)")),
                };
                if me.is(&name, email.as_deref()) {
                    return Some("nothing about the user".into());
                }
                let n = value.chars().count();
                if field == "about" {
                    if n == 0 || n > MAX_AGENT_ABOUT {
                        return Some(format!("who someone is: 1 to {MAX_AGENT_ABOUT} characters"));
                    }
                    if dir_people.iter().any(|p| p.id == key && p.sources.get("about").map(String::as_str) == Some("you")) {
                        return Some("the user wrote who they are: leave it".into());
                    }
                    abouts.push((key, name, email, value.clone()));
                    return None;
                }
                if n == 0 || n > MAX_FACT {
                    return Some(format!("a fact is one line of 1 to {MAX_FACT} characters"));
                }
                let quote = pr["quote"].as_str().unwrap_or("");
                let at = match crate::ledger::quote_in(project, evid, quote) {
                    Ok(Some(at)) => at,
                    Ok(None) => return Some("quote not verbatim in this evidence item (one contiguous span, 3 words or more)".into()),
                    Err(e) => return Some(format!("{e:#}")),
                };
                let id = format!("{:x}", { use sha2::Digest as _; let d = sha2::Sha256::digest(format!("{key}\0{evid}\0{}", norm(quote)).as_bytes()); u64::from_be_bytes(d[..8].try_into().unwrap()) });
                facts.push((key, name, email, json!({"id": id, "text": value, "quote": quote, "date": at["date"], "thread": project, "evidence": evid,
                    "kind": at["kind"], "call": at["call"], "title": at["title"], "url": at["url"], "ts": at["ts"], "added": chrono::Utc::now().to_rfc3339()})));
                return None;
            }
            // Someone the words name who was in no call (a client contact named in internal syncs): one
            // proposal per name, its mentions counted; on Yes a name-only person in the thread.
            if field == "person" {
                let name = display(who);
                if name.split_whitespace().next().is_none_or(|w| w.chars().count() < 2) || name.contains('@') || name.chars().count() > 80 {
                    return Some("a person is a name as the words give it (\"Dana Doe\", or \"Dana\" when that is all)".into());
                }
                if me.is(&name, None) || find_person(&people, &name).is_some() {
                    return Some(format!("{name:?} is already among the thread's people"));
                }
                let at = match crate::ledger::quote_in(project, evid, pr["quote"].as_str().unwrap_or("")) {
                    Ok(Some(at)) => at,
                    Ok(None) => return Some("quote not verbatim in this evidence item (one contiguous span, 3 words or more)".into()),
                    Err(e) => return Some(format!("{e:#}")),
                };
                let org = pr["org"].as_str().filter(|o| !o.trim().is_empty()).and_then(org_named);
                let role = pr["role"].as_str().map(str::trim).filter(|r| !r.is_empty()).map(|r| r.chars().take(80).collect::<String>());
                let key = key_of(&name, None);
                if let Some(x) = all.iter_mut().find(|x| x["field"] == "person" && x["target_key"] == key.as_str()) {
                    if x["state"] != "pending" {
                        return Some("already proposed (or turned down)".into());
                    }
                    if x["at"]["evidence"] != at["evidence"] {
                        x["mentions"] = json!(x["mentions"].as_u64().unwrap_or(1) + 1);
                    }
                    if x["target"]["org"].is_null() {
                        if let Some(o) = org {
                            x["target"]["org"] = json!(o.id);
                            x["target"]["org_name"] = json!(o.name);
                        }
                    }
                    if x["role"].is_null() && role.is_some() {
                        x["role"] = json!(role);
                    }
                    return None;
                }
                let sid = format!("{:x}", { use sha2::Digest as _; let d = sha2::Sha256::digest(format!("{key}\0person\0{evid}").as_bytes()); u64::from_be_bytes(d[..8].try_into().unwrap()) });
                all.push(json!({"sid": sid, "who": who, "target": {"key": key, "name": name, "email": null, "org": org.map(|o| &o.id), "org_name": org.map(|o| &o.name)},
                                "target_key": key, "field": "person", "value": name, "role": role, "mentions": 1,
                                "quote": pr["quote"], "note": pr["note"], "at": at, "state": "pending", "proposed": chrono::Utc::now().to_rfc3339()}));
                added += 1;
                return None;
            }
            if !SUGGEST_FIELDS.contains(&field) {
                return Some(format!("field is one of {SUGGEST_FIELDS:?}, person, fact or about"));
            }
            let at = match crate::ledger::quote_in(project, evid, pr["quote"].as_str().unwrap_or("")) {
                Ok(Some(at)) => at,
                Ok(None) => return Some("quote not verbatim in this evidence item (one contiguous span, 3 words or more)".into()),
                Err(e) => return Some(format!("{e:#}")),
            };
            let (target, current): (Value, Option<String>) = if field == "relation" {
                let (Some(a), Some(b)) = (org_named(who), org_named(pr["to"].as_str().unwrap_or(""))) else {
                    return Some("a relation names two known organisations: `who` and `to`".into());
                };
                if !projects::RELATIONS.contains(&value.as_str()) {
                    return Some(format!("a relation is one of {:?}", projects::RELATIONS));
                }
                let cur = a.relations.iter().find(|r| r.to == b.id).map(|r| r.kind.clone());
                (json!({"org": a.id, "name": a.name, "to": b.id, "to_name": b.name}), cur)
            } else {
                let Some(x) = find_person(&people, who) else { return Some(format!("{who:?} isn't among the thread's people (use a name or an address from people-context)")) };
                let pl = x["places"].get(0).cloned().unwrap_or(json!({}));
                if field == "role" && pl["role_by"] == "you" {
                    return Some("the user set this role: leave it".into());
                }
                let cur = match field {
                    "role" | "stance" => pl[field].as_str().map(str::to_owned),
                    "influence" | "interest" => pl[field].as_u64().map(|n| n.to_string()),
                    "left" => (x["in_thread"] != true).then(|| "true".to_string()),
                    "reports_to" => x["reports_to"].as_str().map(str::to_owned),
                    f => x[f].as_str().map(str::to_owned),
                };
                (json!({"key": x["key"], "name": x["name"], "email": x["email"]}), cur)
            };
            let value_ok = match field {
                "stance" => projects::STANCES.contains(&value.as_str()),
                "influence" | "interest" => matches!(value.as_str(), "1" | "2" | "3"),
                "left" => value == "true",
                "reports_to" => find_person(&people, &value).is_some(),
                _ => !value.is_empty() && value.chars().count() <= 80,
            };
            if !value_ok {
                return Some(format!("bad value {value:?} for {field}"));
            }
            let value = if field == "reports_to" { find_person(&people, &value).and_then(|x| x["key"].as_str()).unwrap_or("").to_string() } else { value.clone() };
            if current.as_deref() == Some(value.as_str()) {
                return Some("already so".into());
            }
            let tkey = target["key"].as_str().or(target["org"].as_str()).unwrap_or("").to_string();
            if all.iter().any(|x| x["target_key"] == tkey.as_str() && x["field"] == field && x["value"] == value.as_str() && x["state"] != "accepted") {
                return Some("already proposed (or turned down)".into());
            }
            let sid = format!("{:x}", { use sha2::Digest as _; let d = sha2::Sha256::digest(format!("{tkey}\0{field}\0{value}\0{evid}").as_bytes()); u64::from_be_bytes(d[..8].try_into().unwrap()) });
            all.push(json!({"sid": sid, "who": who, "target": target, "target_key": tkey, "field": field, "value": value,
                            "value_name": if field == "reports_to" { find_person(&people, pr["value"].as_str().unwrap_or("")).map(|x| x["name"].clone()) } else { None },
                            "was": current,
                            "quote": pr["quote"], "note": pr["note"], "at": at, "state": "pending", "proposed": chrono::Utc::now().to_rfc3339()}));
            added += 1;
            None
        })();
        if let Some(w) = why {
            rejected.push(json!({"i": i, "why": w}));
        }
    }
    projects::with_lock(|| projects::write_json(&suggestions_file(project), &all))?;
    let (n_facts, n_about) = (facts.len(), abouts.len());
    if n_facts + n_about > 0 {
        projects::with_lock(|| {
            let mut dir = directory()?;
            let now = chrono::Utc::now().to_rfc3339();
            for (key, name, email, f) in facts {
                let p = entry(&mut dir, &key, &name, email.as_deref());
                if !p.facts.iter().any(|x| x["id"] == f["id"]) {
                    p.facts.push(f);
                    p.facts.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
                    let over = p.facts.len().saturating_sub(MAX_FACTS);
                    p.facts.drain(..over);
                }
            }
            for (key, name, email, text) in abouts {
                let p = entry(&mut dir, &key, &name, email.as_deref());
                if p.sources.get("about").map(String::as_str) != Some("you") && p.fields.get("about") != Some(&text) {
                    p.history.push(json!({"field": "about", "from": null, "to": null, "source": "echo", "at": now}));
                    p.fields.insert("about".into(), text);
                    p.fields.insert("about_thread".into(), project.into());
                    p.sources.insert("about".into(), "echo".into());
                }
            }
            projects::write_json(&file(), &dir)
        })?;
    }
    Ok(json!({"added": added, "facts": n_facts, "about": n_about, "rejected": rejected}))
}

/// A person's directory entry (by key or one of their addresses), made when there is none.
pub(super) fn entry<'a>(all: &'a mut Vec<Person>, key: &str, name: &str, email: Option<&str>) -> &'a mut Person {
    let i = match all.iter().position(|p| p.id == key || email.is_some_and(|e| p.emails.iter().any(|x| x == e))) {
        Some(i) => i,
        None => {
            all.push(Person { id: key.into(), name: name.trim().into(), emails: email.map(|e| vec![e.trim().to_lowercase()]).unwrap_or_default(), ..Default::default() });
            all.len() - 1
        }
    };
    &mut all[i]
}

/// The user's answer to a suggestion: yes applies it as theirs (a placement, a directory field, a
/// relationship, or taking someone out of the thread); no keeps it turned down (not proposed again).
/// `value` = the user's own version instead of the proposed one ("Change…"): checked as a proposal
/// is, applied as theirs, and the proposed value stays turned down.
pub fn decide(project: &str, sid: &str, yes: bool, value: Option<&str>) -> Result<()> {
    let mut all = suggestions(project);
    let x = all.iter_mut().find(|x| x["sid"] == sid).ok_or_else(|| anyhow::anyhow!("no such suggestion"))?;
    if x["state"] != "pending" {
        bail!("already decided");
    }
    let field = x["field"].as_str().unwrap_or("").to_string();
    let own = match value.map(str::trim) {
        Some(v) if yes => Some(corrected(project, &field, v)?),
        _ => None,
    };
    if yes {
        let t = &x["target"];
        let field = field.as_str();
        let value = own.as_deref().unwrap_or(x["value"].as_str().unwrap_or(""));
        let (name, email) = (t["name"].as_str().unwrap_or(""), t["email"].as_str());
        let at = x["at"].clone();
        match field {
            // A role keeps where it came from: the proposal's words (yours when you changed it).
            "role" => place(project, name, email, &json!({"role": value, "role_by": if own.is_some() { "you" } else { "echo" }, "role_at": at["date"], "role_from": at["title"]}))?,
            // `group`: their workstream in this thread (the chart's column), not the directory's.
            "stance" | "group" => place(project, name, email, &json!({field: value}))?,
            // Named in the words, in no call: a name-only person, under the organisation the words gave.
            "person" => {
                let key = key_of(value, None);
                if let Some(o) = t["org"].as_str() {
                    set(&key, value, None, "org", o, "echo")?;
                }
                let mut v = json!({"role_by": "echo", "role_at": at["date"], "role_from": at["title"]});
                if let Some(r) = x["role"].as_str() {
                    v["role"] = json!(r);
                }
                place(project, value, None, &v)?;
            }
            "influence" | "interest" => place(project, name, email, &json!({field: value.parse::<u8>().unwrap_or(0)}))?,
            "left" => {
                let key = t["key"].as_str().unwrap_or("").to_string();
                projects::update(project, |p| {
                    p.people.retain(|m| key_of(&m.name, m.email.as_deref()) != key);
                    Ok(())
                })?;
            }
            "relation" => {
                let (org, to) = (t["org"].as_str().unwrap_or(""), t["to"].as_str().unwrap_or("").to_string());
                projects::update_client(org, |c| {
                    c.relations.retain(|r| r.to != to);
                    c.relations.push(projects::Relation { to: to.clone(), kind: value.into(), note: None });
                    Ok(())
                })?;
            }
            f => {
                set(t["key"].as_str().unwrap_or(""), name, email, f, value, "you")?;
            }
        }
    }
    x["state"] = json!(if own.is_some() { "corrected" } else if yes { "accepted" } else { "rejected" });
    if let Some(v) = &own {
        x["chosen"] = json!(v);
    }
    x["decided"] = json!(chrono::Utc::now().to_rfc3339());
    projects::with_lock(|| projects::write_json(&suggestions_file(project), &all))
}

/// The user's own value for a suggestion's field, checked as a proposal's is (`reports_to`: a
/// person of the thread, by key, name or address → their key). Whether someone left is yes or no.
pub(super) fn corrected(project: &str, field: &str, v: &str) -> Result<String> {
    let ok = match field {
        "stance" => projects::STANCES.contains(&v),
        "relation" => projects::RELATIONS.contains(&v),
        "influence" | "interest" => matches!(v, "1" | "2" | "3"),
        "left" => bail!("whether someone left is a yes or a no"),
        "reports_to" => {
            let people = of_thread(project, chrono::Local::now().date_naive())?["people"].as_array().cloned().unwrap_or_default();
            let x = people.iter().find(|x| x["key"] == v).or_else(|| find_person(&people, v));
            return x.and_then(|x| x["key"].as_str()).map(str::to_owned).ok_or_else(|| anyhow::anyhow!("{v:?} isn't among the thread's people"));
        }
        _ => !v.is_empty() && v.chars().count() <= 80,
    };
    if !ok {
        bail!("bad value {v:?} for {field}");
    }
    Ok(v.to_string())
}

//! Threads, clients, people and tasks: their pages and the thread operations (`project_op`).

use super::*;
use super::calendar::{CALENDAR_CACHE, RAW_EVENTS};
#[cfg(feature = "brain")]
use super::brain::brain_sync_now;

/// The Tasks activity: the piles and asks, with the next 7 days of the calendar for "next time you
/// meet" (the calendar's own 2-minute cache; without a sign-in, today's agenda).
/// A People tab's data; and in the background, Microsoft 365 is asked about its people not asked
/// this week (one refresh at a time; the page's next poll shows what came back).
pub(super) fn people_view(v: anyhow::Result<Value>) -> Value {
    static RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let v = match v {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("{e:#}") }),
    };
    let who: Vec<(String, String)> = v["people"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["name"].as_str()?.to_string(), p["email"].as_str()?.to_string())))
        .collect();
    if !who.is_empty() && !RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        std::thread::spawn(move || {
            if let Err(e) = crate::people::refresh_m365(&who) {
                eprintln!("echoctl: people from Microsoft 365: {e:#}");
            }
            RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }
    v
}

pub(super) fn client_view(id: &str) -> Value {
    let today = chrono::Local::now().date_naive();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    crate::tasks::client(id, &upcoming(today), today, &now).unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))
}

/// The next 7 days of the calendar (its own 2-minute cache; without a sign-in, today's agenda).
pub(super) fn upcoming(today: chrono::NaiveDate) -> Vec<crate::meeting::MeetingContext> {
    let mut meetings = watch::agenda();
    if crate::graph::account().is_some() {
        let _ = super::calendar::calendar(&format!("start={today}&days=7"));
        let key = format!("{today}/7");
        if let Some((_, _, raw)) = CALENDAR_CACHE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|(k, _, _)| *k == key) {
            meetings = raw.iter().filter_map(crate::calendar::to_meeting).collect();
        }
    }
    meetings
}

pub(super) fn tasks_view() -> Value {
    let today = chrono::Local::now().date_naive();
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    crate::tasks::across(&upcoming(today), today, &now).unwrap_or_else(|e| json!({ "error": format!("{e:#}"), "piles": {}, "asks": [], "projects": [] }))
}

/// Clients, projects (each with its call count), the stages, and each call's projects (the
/// calls list's chips) — or why the projects can't be read.
pub(super) fn projects_overview() -> Value {
    match crate::projects::overview() {
        Ok(mut v) => {
            v["by_call"] = json!(crate::projects::by_call());
            v["found_count"] = json!(crate::projects::found().map(|f| f.len()).unwrap_or(0));
            v
        }
        Err(e) => json!({ "error": format!("{e:#}"), "projects": [], "clients": [], "by_call": {} }),
    }
}

pub(super) fn project_op(v: &Value) -> anyhow::Result<Option<String>> {
    use crate::projects::{self as pj, Member, Rule, Stage, Term};
    let s = |k: &str| v[k].as_str().map(str::to_owned);
    let id = s("id").unwrap_or_default();
    let opt = |k: &str| s(k).map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    match v["op"].as_str().unwrap_or("") {
        "create" => {
            let p = pj::create(&s("name").unwrap_or_default(), opt("client").as_deref(), None)?;
            return Ok(Some(p.id));
        }
        "set" => {
            let value = v["value"].as_str().unwrap_or("").to_string();
            pj::update(&id, |p| {
                let some = || Some(value.trim().to_string()).filter(|x| !x.is_empty());
                match v["field"].as_str().unwrap_or("") {
                    "name" => p.name = value.trim().to_string(),
                    "client" => p.client = some(),
                    "since" => p.since = some(),
                    "history_before" => p.history_before = some(),
                    "color" => p.color = value.clone(),
                    "icon" => p.icon = some(),
                    "notes" => p.notes = value.clone(),
                    "ai_profile" => p.ai_profile = some(),
                    "model" => p.model = some(),
                    f => anyhow::bail!("no field {f:?}"),
                }
                Ok(())
            })?;
            // A history date changes what the Brain searches: apply it now.
            #[cfg(feature = "brain")]
            if v["field"] == "history_before" {
                brain_sync_now();
            }
        }
        "stage" => {
            pj::set_stage(&id, Stage::parse(v["stage"].as_str().unwrap_or(""))?, v["reason"].as_str())?;
        }
        // The Work board's columns, card fields and grouping (null: back to the defaults).
        "board" => {
            let b: Option<pj::Board> = if v["board"].is_null() { None } else { Some(serde_json::from_value(v["board"].clone()).map_err(|e| anyhow::anyhow!("not a board: {e}"))?) };
            pj::update(&id, |p| {
                p.board = b;
                Ok(())
            })?;
        }
        // A Teams channel or a mail rule the thread reads into the Brain (`brain::sources`).
        // A meeting series' rule, one field at a time (`field`, `value`; null = Echo's default).
        // Not this thread's: its meetings of that series stop joining it.
        // Prepare (a meeting with no prep) / Write it again: in the background.
        "prep_write" => {
            // Today's agenda, else the calendar the window shows (any day).
            let id = s("prep").unwrap_or_default();
            let m = crate::prep::meeting(&id)
                .or_else(|| RAW_EVENTS.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|r| r["id"].as_str().is_some_and(|e| crate::prep::id_of(e) == id)).and_then(crate::calendar::to_meeting))
                .ok_or_else(|| anyhow::anyhow!("that meeting isn't in the calendar Echo shows"))?;
            anyhow::ensure!(crate::prep::start(m), "a prep is being written: try again in a minute");
        }
        "series_unlink" => {
            let key = s("key").unwrap_or_default();
            pj::update(&id, |p| {
                if !p.not_series.contains(&key) {
                    p.not_series.push(key.clone());
                }
                Ok(())
            })?;
        }
        "series_set" => {
            let field = s("field").unwrap_or_default();
            crate::series::set(&s("key").unwrap_or_default(), s("subject").as_deref(), &json!({ field: v["value"].clone() }))?;
        }
        "source_add" => {
            let kind = s("kind").unwrap_or_default();
            let sid = s("source").unwrap_or_default().trim().to_string();
            let sid = match kind.as_str() {
                "teams_channel" if sid.split_once('|').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty()) => sid,
                // A meeting chat its work reads between calls (`ledger::chat_sync`).
                "teams_chat" if sid.starts_with("19:") && !sid.contains(char::is_whitespace) => sid,
                #[cfg(feature = "brain")]
                "mail" => crate::brain::sources::mail_source(&sid)?,
                _ => anyhow::bail!("not a source: {kind}"),
            };
            let name = opt("name");
            pj::update(&id, |p| {
                if !p.sources.iter().any(|x| x.kind == kind && x.id == sid) {
                    p.sources.push(pj::Source { kind: kind.clone(), id: sid.clone(), name: name.clone(), ..Default::default() });
                }
                Ok(())
            })?;
            #[cfg(feature = "brain")]
            brain_sync_now();
        }
        // A folder's globs of what not to read.
        "source_exclude" => {
            let sid = s("source").unwrap_or_default();
            let globs: Vec<String> = v["exclude"].as_array().into_iter().flatten().filter_map(|g| g.as_str()).map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect();
            pj::update(&id, |p| {
                if let Some(src) = p.sources.iter_mut().find(|x| x.kind == "folder" && x.id == sid) {
                    src.exclude = globs.clone();
                }
                Ok(())
            })?;
            #[cfg(feature = "brain")]
            brain_sync_now();
        }
        "source_remove" => {
            let kind = s("kind").unwrap_or_default();
            let sid = s("source").unwrap_or_default();
            pj::update(&id, |p| {
                p.sources.retain(|x| !(x.kind == kind && x.id == sid));
                Ok(())
            })?;
            #[cfg(feature = "brain")]
            brain_sync_now();
        }
        "rule_add" => {
            let r: Rule = serde_json::from_value(v["rule"].clone()).map_err(|e| anyhow::anyhow!("not a rule: {e}"))?;
            pj::update(&id, |p| {
                if !p.rules.contains(&r) {
                    p.rules.push(r);
                }
                Ok(())
            })?;
        }
        "rule_remove" => {
            let i = v["index"].as_u64().unwrap_or(u64::MAX) as usize;
            pj::update(&id, |p| {
                if i >= p.rules.len() {
                    anyhow::bail!("that rule is gone already");
                }
                p.rules.remove(i);
                Ok(())
            })?;
        }
        // A calendar event put in a project by hand (or taken out): its call will belong.
        "event_member" => {
            pj::set_event(&id, v["event"].as_str().unwrap_or(""), v["how"].as_str().unwrap_or(""))?;
        }
        "member" => {
            let dir = call_named(v).ok_or_else(|| anyhow::anyhow!("that call can't be found"))?;
            pj::set_membership(&id, &dir, v["how"].as_str().unwrap_or(""))?;
        }
        "term_add" | "term_remove" => {
            let term = s("term").unwrap_or_default().trim().to_string();
            let add = v["op"] == "term_add";
            if add && !crate::config::valid_term(&term) {
                anyhow::bail!("a term is 2–50 characters, at most 6 words");
            }
            pj::update(&id, |p| {
                p.terms.retain(|t| !t.term.eq_ignore_ascii_case(&term));
                if add {
                    p.terms.push(Term { term: term.clone(), kind: opt("kind"), note: None });
                }
                Ok(())
            })?;
        }
        "person_add" | "person_remove" => {
            let name = s("name").unwrap_or_default().trim().to_string();
            let email = opt("email");
            let key = email.clone().unwrap_or_else(|| name.clone()).to_lowercase();
            let add = v["op"] == "person_add";
            if add && name.is_empty() {
                anyhow::bail!("a person needs a name");
            }
            pj::update(&id, |p| {
                p.people.retain(|m| m.email.clone().unwrap_or_else(|| m.name.clone()).to_lowercase() != key);
                if add {
                    p.people.push(Member { name: name.clone(), email: email.clone(), role: opt("role"), ..Default::default() });
                }
                Ok(())
            })?;
        }
        "brief" => pj::set_brief(&id, v["text"].as_str().unwrap_or(""))?,
        "client_brief" => pj::set_client_brief(&id, v["text"].as_str().unwrap_or(""))?,
        // Found for you: make the project (as the user named it, with the client chosen), add its
        // rules to one the user has, or never suggest it again.
        "found_accept" => {
            let p = pj::found_accept(&s("key").unwrap_or_default(), s("name").as_deref(), v["client"].as_str())?;
            return Ok(Some(p.id));
        }
        "found_merge" => {
            pj::found_merge(&s("key").unwrap_or_default(), &s("into").unwrap_or_default())?;
        }
        "found_reject" => pj::found_reject(&s("key").unwrap_or_default())?,
        // A logo from the page (a file the user chose or dropped, base64): a project's or a client's.
        "logo" | "client_logo" => {
            use base64::Engine as _;
            let bytes = match v["data"].as_str() {
                Some(d) => Some(base64::engine::general_purpose::STANDARD.decode(d.trim()).map_err(|_| anyhow::anyhow!("that image couldn't be read"))?),
                None => None,
            };
            if v["op"] == "logo" {
                pj::set_logo_bytes(&id, bytes.as_deref())?
            } else {
                pj::set_client_logo(&id, bytes.as_deref())?
            }
        }
        "remove_logo" => pj::set_logo(&id, None)?,
        // The work ledger: sync now (in the background), or an edit of the user's (pinned, undoable).
        // Already syncing: the Work tab shows it running; nothing to say.
        "sync" => {
            crate::projectsync::start(&id);
        }
        "ledger" => {
            crate::ledger::human_event(&id, &v["body"])?;
        }
        "delete" => pj::delete(&id)?,
        "client_add" => {
            let domains: Vec<String> = v["domains"].as_str().unwrap_or("").split([',', ' ']).map(str::to_owned).collect();
            let c = pj::add_client(&s("name").unwrap_or_default(), &domains, v["internal"] == true)?;
            return Ok(Some(c.id));
        }
        "client_set" => {
            pj::update_client(&id, |c| {
                if let Some(n) = s("name") {
                    c.name = n;
                }
                if let Some(d) = v["domains"].as_array() {
                    c.domains = d.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect();
                } else if let Some(d) = v["domains"].as_str() {
                    c.domains = d.split([',', ' ']).map(str::to_owned).collect();
                }
                if let Some(a) = v["aliases"].as_array() {
                    c.aliases = a.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect();
                }
                if let Some(i) = v["internal"].as_bool() {
                    c.internal = i;
                }
                // The organisation it belongs to, its groups, a relationship added or removed.
                if let Some(p) = v.get("parent") {
                    c.parent = p.as_str().map(str::trim).filter(|p| !p.is_empty()).map(str::to_owned);
                    anyhow::ensure!(c.parent.as_deref() != Some(c.id.as_str()), "an organisation can't belong to itself");
                }
                if let Some(g) = v["groups"].as_array() {
                    c.groups = g.iter().filter_map(|x| x.as_str().map(|s| s.trim().to_string())).filter(|s| !s.is_empty()).collect();
                    c.groups.dedup();
                }
                if let (Some(to), Some(kind)) = (v["relation"]["to"].as_str(), v["relation"]["kind"].as_str()) {
                    c.relations.retain(|r| r.to != to);
                    if !kind.is_empty() {
                        anyhow::ensure!(pj::RELATIONS.contains(&kind), "a relationship is one of {:?}", pj::RELATIONS);
                        anyhow::ensure!(to != c.id, "an organisation can't relate to itself");
                        c.relations.push(pj::Relation { to: to.into(), kind: kind.into(), note: None });
                    }
                }
                Ok(())
            })?;
        }
        // People: placed in a thread (role, stance, influence, interest), the directory's fields,
        // organisations placed in a thread.
        "people_place" => crate::people::place(&id, &s("name").unwrap_or_default(), opt("email").as_deref(), &v["set"])?,
        "people_add" => {
            for x in v["people"].as_array().into_iter().flatten() {
                crate::people::place(&id, x["name"].as_str().unwrap_or(""), x["email"].as_str(), &json!({}))?;
            }
        }
        "person_merge" => {
            let key_emails: Vec<String> = v["key_emails"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect();
            crate::people::merge(&s("key").unwrap_or_default(), &s("key_name").unwrap_or_default(), &key_emails, &s("into").unwrap_or_default(), &s("into_name").unwrap_or_default(), opt("into_email").as_deref())?
        }
        "person_split" => crate::people::split(&s("key").unwrap_or_default(), &s("name").unwrap_or_default(), &s("email").unwrap_or_default())?,
        "people_decide" => crate::people::decide(&id, &s("sid").unwrap_or_default(), v["yes"] == true, opt("value").as_deref())?,
        "person_answer" => crate::people::answer(&s("key").unwrap_or_default(), &s("field").unwrap_or_default(), v["use"] == true)?,
        "org_place" => crate::people::place_org(&id, &s("client").unwrap_or_default(), &v["set"])?,
        // A thread's counterpart organisations: link one, unlink it, or make one (Profile's
        // + Organisation; People's "Name this organisation" for a domain no organisation has).
        "org_link" => crate::people::link_org(&id, &s("org").unwrap_or_default(), v["on"] != false)?,
        "org_new" => {
            let domains: Vec<String> = v["domains"].as_str().unwrap_or("").split([',', ' ']).map(str::to_owned).collect();
            let o = pj::add_org(&s("name").unwrap_or_default(), &domains, false, false)?;
            if !id.is_empty() {
                crate::people::link_org(&id, &o.id, true)?;
            }
        }
        "org_name" => {
            crate::people::name_org(&s("name").unwrap_or_default(), &s("domain").unwrap_or_default(), Some(id.as_str()).filter(|t| !t.is_empty()))?;
        }
        "person_set" => {
            let name = s("name").unwrap_or_default();
            let key = s("key").unwrap_or_else(|| crate::people::key_of(&name, opt("email").as_deref()));
            crate::people::set(&key, &name, opt("email").as_deref(), &s("field").unwrap_or_default(), &s("value").unwrap_or_default(), "you")?;
        }
        "client_remove" => pj::remove_client(&id)?,
        op => anyhow::bail!("no project change {op:?}"),
    }
    Ok(None)
}


/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    Some(match path {
        "/api/m365/teams" => json_reply(crate::graph::teams().map(Value::from).unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))),
        "/api/m365/channels" => {
            let team = query_param(req.uri().query().unwrap_or(""), "team").unwrap_or_default();
            json_reply(crate::graph::channels(&team).map(Value::from).unwrap_or_else(|e| json!({ "error": format!("{e:#}") })))
        }
        // Projects: the list (clients, stages, each call's projects), one project, its logo.
        "/api/projects" => json_reply(projects_overview()),
        "/api/tasks" => json_reply(tasks_view()),
        // Open work in other threads, by words in its title (#20: Link to an item in another thread…).
        "/api/work/search" => {
            let q = req.uri().query().unwrap_or("");
            json_reply(json!(crate::ledger::search_open(&query_param(q, "q").unwrap_or_default(), query_param(q, "not").as_deref())))
        }
        // Everyone across the threads (the Brain's People): who, where, what was said about them.
        "/api/brain/people" => json_reply(crate::people::everyone(chrono::Local::now().date_naive()).unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))),
        p if p.starts_with("/api/client/") && p.ends_with("/people") => {
            json_reply(people_view(crate::people::of_client(&p["/api/client/".len()..p.len() - "/people".len()], chrono::Local::now().date_naive())))
        }
        p if p.starts_with("/api/client/") && !p.ends_with("/logo") => json_reply(client_view(&p["/api/client/".len()..])),
        "/api/found" => json_reply(match crate::projects::found() {
            Ok(f) => json!({ "found": f }),
            Err(e) => json!({ "found": [], "error": format!("{e:#}") }),
        }),
        p if p.starts_with("/api/client/") && p.ends_with("/logo") => {
            match crate::projects::client_logo(&p["/api/client/".len()..p.len() - "/logo".len()]).and_then(|(f, mime)| Some((fs::read(f).ok()?, mime))) {
                Some((bytes, mime)) => reply(200, mime, bytes),
                None => reply(404, "text/plain", b"no logo".to_vec()),
            }
        }
        p if p.starts_with("/api/project/") => {
            let rest = &p["/api/project/".len()..];
            // Its people and organisations (the People tab).
            if let Some(id) = rest.strip_suffix("/people") {
                return Some(json_reply(people_view(crate::people::of_thread(id, chrono::Local::now().date_naive()))));
            }
            // Its meeting series and what Echo does with them (Profile → Meetings).
            if let Some(id) = rest.strip_suffix("/series") {
                return Some(json_reply(crate::series::of_thread(id).unwrap_or_else(|e| json!({ "error": format!("{e:#}") }))));
            }
            // Its work ledger: the tree, and the last sync.
            if let Some(id) = rest.strip_suffix("/work") {
                return Some(json_reply(match crate::ledger::tree(id) {
                    Ok(mut t) => {
                        t["sync"] = crate::projectsync::state(id);
                        t
                    }
                    Err(e) => json!({ "error": format!("{e:#}") }),
                }));
            }
            match rest.strip_suffix("/logo") {
                Some(id) => match crate::projects::logo(id).and_then(|(f, mime)| Some((fs::read(f).ok()?, mime))) {
                    Some((bytes, mime)) => reply(200, mime, bytes),
                    None => reply(404, "text/plain", b"no logo".to_vec()),
                },
                None => match crate::projects::load(rest).and_then(|p| crate::projects::view(&p, true)) {
                    Ok(v) => json_reply(v),
                    Err(e) => json_reply(json!({ "error": format!("{e:#}") })),
                },
            }
        }
        _ => return None,
    })
}

/// The page's commands of this area (`handle` asks each area in turn).
pub(super) fn handle(v: &Value) -> Option<Handled> {
    Some(match v["cmd"].as_str().unwrap_or("") {
        // Projects and clients: every change the page makes (`project_op`); a new one is opened.
        "project" => match project_op(v) {
            Ok(Some(id)) if v["op"] == "create" || v["op"] == "found_accept" => Handled { refresh: true, reply: Some(format!("window.echo && window.echo.projectCreated({})", json!(id))) },
            Ok(Some(id)) if v["op"] == "client_add" => Handled { refresh: true, reply: Some(format!("window.echo && window.echo.clientCreated({})", json!(id))) },
            Ok(_) => true.into(),
            Err(e) => notice(&format!("{e:#}")),
        },
        _ => return None,
    })
}

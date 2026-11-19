//! The window's search over what was said: the Calls list's box in search mode and ⌘K's "Search what was said…"
//! (`/api/search`), and a moment's context for the right panel (`/api/search/moment`). Only with the `brain` feature;
//! with Intake off it answers `enabled: false`, and the box keeps to titles and people.

use super::*;
use crate::brain::store;

/// The most sections a search reads (grouped by their sources), and the most calls matched by title or people.
const HITS: usize = 30;
const TITLES: usize = 20;
/// The longest snippet a moment gives (the page shows two lines of it).
const SNIPPET: usize = 400;
/// The longest query searched (characters; more is cut).
const QUERY_MAX: usize = 500;

/// The echo:// requests of this area (`respond` asks each area in turn).
pub(super) fn route(path: &str, req: &Request<Vec<u8>>) -> Option<Response<Cow<'static, [u8]>>> {
    let q = req.uri().query().unwrap_or("");
    Some(match path {
        // `?q=&kind=call,mail&section=decisions&client=&thread=&since=&until=&person=` → titles and sources.
        "/api/search" => json_reply(search(q)),
        // `?chunk=<id>` → the moment with its context: the turns around it, or its message's facts.
        "/api/search/moment" => json_reply(moment(q)),
        _ => return None,
    })
}

fn param(q: &str, key: &str) -> Option<String> {
    query_param(q, key).map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn list(q: &str, key: &str) -> Vec<String> {
    param(q, key).unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect()
}

/// A search from the window: the calls whose title or people hold the words (`titles`), and what was said — the index's
/// sections grouped by their source, best first (`sources`, each with its `moments`). The filters are the index's
/// (`store::Filters`); a client or a thread is the search's scope. The calls' cards are left out of what was said
/// unless a section filter asks for them: their titles and people are the titles' own matches.
pub(super) fn search(q: &str) -> Value {
    if !crate::brain::enabled() {
        return json!({ "enabled": false });
    }
    let query: String = param(q, "q").unwrap_or_default().chars().take(QUERY_MAX).collect();
    let (client, thread) = (param(q, "client"), param(q, "thread"));
    let sections = list(q, "section");
    let filters = store::Filters {
        kinds: list(q, "kind"),
        sections: if sections.is_empty() { store::SECTION_KINDS.iter().filter(|s| **s != "card").map(|s| s.to_string()).collect() } else { sections.clone() },
        since: param(q, "since"),
        until: param(q, "until"),
        person: param(q, "person"),
    };
    let scope = thread.as_deref().map(|t| ("thread", t)).or(client.as_deref().map(|c| ("client", c)));
    let r = match crate::brain::search(&query, HITS, None, scope, &filters) {
        Ok(r) => r,
        Err(e) => return json!({ "enabled": true, "error": format!("{e:#}") }),
    };
    // The words looked for: the query's, without a date it said (the page marks them in each snippet).
    let rest = match r["filters"]["dates_from_query"].as_str() {
        Some(_) => crate::brain::when::in_query(&query, chrono::Local::now().date_naive()).map_or(query.clone(), |x| x.1),
        None => query.clone(),
    };
    let words = store::query_words(&rest);
    let (since, until) = (r["filters"]["since"].as_str(), r["filters"]["until"].as_str());
    let with_calls = filters.kinds.is_empty() || filters.kinds.iter().any(|k| k == "call");
    let titles = if with_calls && (!words.is_empty() || since.is_some() || until.is_some()) {
        titles(&words, since, until, client.as_deref(), thread.as_deref(), filters.person.as_deref())
    } else {
        vec![]
    };
    let note = r["note"].as_str().unwrap_or("");
    json!({
        "enabled": true,
        "query": query,
        "words": words,
        "filters": {
            "kinds": filters.kinds, "sections": sections, "client": client, "thread": thread, "person": filters.person,
            "since": since, "until": until, "dates_from_query": r["filters"]["dates_from_query"],
        },
        "older": note.contains(crate::brain::retain::OLDER),
        "listed": note.contains("the sources of those days"),
        "titles": titles,
        "sources": sources(r["hits"].as_array().map(Vec::as_slice).unwrap_or(&[]), &words),
    })
}

/// The calls whose title, people or meeting subject hold every word, newest first, on the days asked for, in the
/// client or thread, with the person.
fn titles(words: &[String], since: Option<&str>, until: Option<&str>, client: Option<&str>, thread: Option<&str>, person: Option<&str>) -> Vec<Value> {
    let person = person.map(str::to_lowercase);
    let dirs: Vec<PathBuf> = call::matching(words)
        .into_iter()
        .filter(|d| {
            let day = d.file_name().map(|n| n.to_string_lossy().chars().take(10).collect::<String>()).unwrap_or_default();
            since.is_none_or(|s| day.as_str() >= s) && until.is_none_or(|u| day.as_str() <= u)
        })
        .filter(|d| {
            if client.is_none() && thread.is_none() {
                return true;
            }
            let of = crate::projects::of_call(d).unwrap_or_default();
            of.iter().any(|(p, _)| thread.is_some_and(|t| p.id == t) || (thread.is_none() && client.is_some() && p.client.as_deref() == client))
        })
        .filter(|d| person.as_deref().is_none_or(|p| call::people(d).iter().any(|n| n.to_lowercase().contains(p))))
        .take(TITLES)
        .collect();
    call::summaries(&dirs.iter().collect::<Vec<_>>())
}

/// A source's kind by its id (`store::SOURCE_KINDS`).
fn kind_of(source: &str) -> &'static str {
    store::SOURCE_KINDS.iter().find(|k| source.starts_with(k.1)).map_or("document", |k| k.0)
}

/// The search's hits as their sources, in the order of each source's best hit, each with its hits as moments.
fn sources(hits: &[Value], words: &[String]) -> Vec<Value> {
    let docs = store::open(&crate::brain::store_path()).ok();
    let doc = |source: &str| -> (Option<String>, Option<String>) {
        docs.as_ref()
            .and_then(|c| c.query_row("SELECT title, event_date FROM documents WHERE source = ?1", [source], |r| Ok((r.get(0)?, r.get(1)?))).ok())
            .unwrap_or((None, None))
    };
    let mut out: Vec<Value> = vec![];
    for h in hits {
        let source = h["source"].as_str().unwrap_or("");
        let at = match out.iter().position(|s| s["source"] == source) {
            Some(i) => i,
            None => {
                let refs = &h["refs"];
                let (title, date) = doc(source);
                let client = refs["client"].as_str().or_else(|| refs["threads"].as_array().into_iter().flatten().find_map(|t| t["client"].as_str()));
                out.push(json!({
                    "source": source, "kind": kind_of(source),
                    "title": title.unwrap_or_else(|| h["breadcrumb_path"].as_str().unwrap_or(source).split(" > ").next().unwrap_or(source).to_string()),
                    "date": date.or_else(|| h["event_date"].as_str().map(str::to_owned)).map(|d| d.chars().take(10).collect::<String>()),
                    "client": client, "threads": refs["threads"].as_array().cloned().unwrap_or_default(), "call": refs["call"],
                    "closed": h.get("closed").is_some(), "moments": [],
                }));
                out.len() - 1
            }
        };
        if let Some(m) = out[at]["moments"].as_array_mut() {
            m.push(moment_of(h, words));
        }
    }
    out
}

/// `[07:48]` or `[1:02:05]` at the start of `s`: the seconds, and what follows.
fn clock_at_start(s: &str) -> Option<(f64, &str)> {
    let rest = s.strip_prefix('[')?;
    let (clock, after) = rest.split_once(']')?;
    let t = crate::brain::calls::title_time(&format!("[{clock}]"))?;
    Some((t, after.trim_start()))
}

/// A time chip anywhere in a line (`… [04:12]`): its seconds.
fn clock_in(s: &str) -> Option<f64> {
    s.match_indices('[').find_map(|(i, _)| clock_at_start(&s[i..]).map(|x| x.0))
}

/// Whether a line holds one of the words, whole (as the index matches them).
fn has_word(line: &str, words: &[String]) -> bool {
    line.split(|c: char| !c.is_alphanumeric()).any(|w| !w.is_empty() && words.iter().any(|x| x == &w.to_lowercase()))
}

/// A line as it reads: no list mark, no bold, no time chips, a Markdown link by its label.
fn plain(line: &str) -> String {
    let mut s = line.trim().trim_start_matches(['-', '*', '•']).trim().replace("**", "");
    // [label](url) → label
    while let Some(i) = s.find("](") {
        let (Some(open), Some(close)) = (s[..i].rfind('['), s[i..].find(')').map(|j| i + j)) else { break };
        s = format!("{}{}{}", &s[..open], &s[open + 1..i], &s[close + 1..]);
    }
    // time chips
    while let Some(i) = s.find('[') {
        let Some(end) = clock_at_start(&s[i..]).map(|(_, after)| s.len() - after.len()) else { break };
        s = format!("{}{}", s[..i].trim_end(), if end < s.len() { format!(" {}", &s[end..]) } else { String::new() });
    }
    s.trim().chars().take(SNIPPET).collect()
}

/// A hit as a moment of its source: its section, who said it and when, the line that has the words (else its first).
fn moment_of(h: &Value, words: &[String]) -> Value {
    let kind = h["section_kind"].as_str().unwrap_or("");
    let text = h["text"].as_str().unwrap_or("");
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let line = lines.iter().find(|l| has_word(l, words)).or(lines.first()).copied().unwrap_or("");
    let (mut t, mut said, mut who) = (None, line, h["speaker"].as_str().map(str::to_owned));
    // A transcript's or a chat's line: "[07:48] Omar Haddad (Acme): …".
    if let Some((at, rest)) = clock_at_start(line).filter(|_| kind == "transcript" || kind == "chat") {
        t = Some(at);
        if let Some((name, words)) = rest.split_once(": ") {
            who = Some(name.rsplit_once(" (").filter(|x| x.1.ends_with(')')).map_or(name, |x| x.0).to_string());
            said = words;
        } else {
            said = rest;
        }
    }
    let t = t.or_else(|| clock_in(line));
    // The section's heading: a minutes section by its own ("Decisions"), else the part under the title.
    let crumbs: Vec<&str> = h["breadcrumb_path"].as_str().unwrap_or("").split(" > ").collect();
    let heading = match crumbs.get(1) {
        Some(&"Minutes") => crumbs.get(2).copied(),
        x => x.copied(),
    };
    let call = h["refs"]["call"].as_str().filter(|_| h["source"].as_str().is_some_and(|s| s.starts_with("call/")));
    let link = match (call, t) {
        (Some(c), Some(t)) => format!("echo://call/{c}?t={}", t as u64),
        _ => h["link"].as_str().unwrap_or("").to_string(),
    };
    json!({
        "chunk": h["chunk_id"], "section_kind": kind, "heading": heading, "who": who.filter(|w| !w.is_empty()), "t": t,
        "day": h["event_date"].as_str().map(|d| d.chars().take(10).collect::<String>()), "text": plain(said), "link": link,
    })
}

/// A moment with its context, for the right panel: its source; a call's row (title, when, how long, who) and the
/// turns around its time (the one before for a transcript's moment, the one said then, the one after); another
/// source's lines of that section.
pub(super) fn moment(q: &str) -> Value {
    let Some(id) = param(q, "chunk").and_then(|c| c.parse::<i64>().ok()) else { return json!({ "error": "no such moment" }) };
    let row = store::open(&crate::brain::store_path()).ok().and_then(|c| {
        c.query_row(
            "SELECT c.source, c.title, c.breadcrumb_path, c.text, c.event_date, c.speaker, c.link, c.refs, c.section, d.title FROM chunks c LEFT JOIN documents d ON d.source = c.source WHERE c.id = ?1",
            [id],
            |r| {
                Ok(json!({"chunk_id": id.to_string(), "source": r.get::<_, String>(0)?, "section": r.get::<_, Option<String>>(1)?, "breadcrumb_path": r.get::<_, Option<String>>(2)?,
                    "text": r.get::<_, Option<String>>(3)?, "event_date": r.get::<_, Option<String>>(4)?, "speaker": r.get::<_, Option<String>>(5)?, "link": r.get::<_, Option<String>>(6)?,
                    "refs": r.get::<_, Option<String>>(7)?.and_then(|s| serde_json::from_str::<Value>(&s).ok()), "section_kind": r.get::<_, Option<String>>(8)?, "title": r.get::<_, Option<String>>(9)?}))
            },
        )
        .ok()
    });
    let Some(h) = row else { return json!({ "error": "no such moment" }) };
    let words = store::query_words(&param(q, "q").unwrap_or_default());
    let m = moment_of(&h, &words);
    let source = h["source"].as_str().unwrap_or("");
    let dir = h["refs"]["call"].as_str().filter(|_| source.starts_with("call/")).and_then(|c| crate::calls::resolve_call(c).ok());
    let (call, turns) = match &dir {
        Some(d) => (
            call::summaries(&[d]).pop().unwrap_or(Value::Null),
            m["t"].as_f64().map(|t| turns_around(d, t, m["section_kind"] == "transcript", &words)).unwrap_or_default(),
        ),
        None => (Value::Null, vec![]),
    };
    let lines: Vec<String> = if dir.is_some() { vec![] } else { h["text"].as_str().unwrap_or("").lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).map(plain).filter(|l| !l.is_empty()).take(12).collect() };
    json!({
        "source": source, "kind": kind_of(source), "title": h["title"].as_str().unwrap_or(h["breadcrumb_path"].as_str().unwrap_or(source).split(" > ").next().unwrap_or(source)),
        "client": h["refs"]["client"], "threads": h["refs"]["threads"], "moment": m, "call": call, "turns": turns, "lines": lines,
    })
}

/// The call's turns (one speaker's lines that follow each other) around `t`: the one said then, the one after, and with
/// `before` the one before. Each: its time, who, whether it is the user, the remote speaker's number, its text, `hit`.
/// Of several turns started in that second, the one with the words said.
fn turns_around(dir: &Path, t: f64, before: bool, words: &[String]) -> Vec<Value> {
    let me = crate::config::Config::load().ok().and_then(|c| c.me).map(|m| m.name).unwrap_or_else(|| "Me".into());
    let segs = read_json(&dir.join("segments.json"));
    let mut turns: Vec<(f64, String, bool, Value, String)> = vec![];
    for s in segs.as_array().into_iter().flatten() {
        let text = s["text"].as_str().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let mine = matches!(s["speaker"].as_str(), Some("me") | Some("mic"));
        let who = if mine { me.clone() } else { s["name"].as_str().map(crate::people::display).unwrap_or_else(|| "Remote".into()) };
        match turns.last_mut() {
            Some(last) if last.1 == who => {
                last.4.push(' ');
                last.4.push_str(text);
            }
            _ => turns.push((s["start"].as_f64().unwrap_or(0.0), who, mine, s["speaker_id"].clone(), text.to_string())),
        }
    }
    // The index's clock drops the fraction of a second (`[00:35]` for 35.6 s): the turn that started within that second.
    let Some(last) = turns.iter().rposition(|x| x.0 < t + 1.0).or((!turns.is_empty()).then_some(0)) else { return vec![] };
    let at = (0..=last).rev().take_while(|&i| turns[i].0 >= t.floor()).find(|&i| has_word(&turns[i].4, words)).unwrap_or(last);
    let from = if before { at.saturating_sub(1) } else { at };
    turns[from..(at + 2).min(turns.len())]
        .iter()
        .enumerate()
        .map(|(i, x)| json!({ "t": x.0, "who": x.1, "me": x.2, "spk": x.3, "text": x.4, "hit": from + i == at }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(path: &str) -> Value {
        let r = respond(&Request::builder().uri(format!("echo://localhost{path}")).body(Vec::new()).expect("request"));
        assert_eq!(r.headers().get(CONTENT_TYPE).map(|v| v.to_str().unwrap_or("")), Some("application/json"));
        serde_json::from_slice(r.body()).expect("JSON")
    }

    fn input(source: &str, kind: &str, title: &str, day: &str, md: &str, refs: Value, speaker: Option<&str>) -> store::Input {
        let (s, who) = (source.to_string(), speaker.map(str::to_owned));
        store::Input {
            source: s.clone(),
            kind: kind.into(),
            title: title.into(),
            event_date: Some(day.into()),
            md: md.into(),
            refs,
            link: Box::new(move |_| format!("echo://{s}")),
            speaker: Box::new(move |c| who.clone().filter(|_| !c.title.is_empty())),
        }
    }

    /// Made-up sources: an Acme call (its folder too, with a transcript), an Acme mail and a Globex post, all saying
    /// "cutover"; Intake on.
    fn made_up(root: &Path) -> String {
        let today = chrono::Local::now().date_naive();
        let day = |n: i64| (today - chrono::Duration::days(n)).to_string();
        crate::brain::save_settings(&crate::brain::embed::Settings { enabled: Some(true), ..Default::default() }).unwrap();
        let name = format!("{}_123000-acme-go-live-check", day(1));
        let d = crate::paths::calls_dir().join(&name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("meta.json"), json!({"title": "Acme go-live check", "started_at": format!("{}T12:30:00+00:00", day(1)), "ended_at": format!("{}T12:40:00+00:00", day(1))}).to_string()).unwrap();
        std::fs::write(
            d.join("segments.json"),
            json!([
                {"speaker": "remote", "name": "Dana Lee", "start": 451.0, "end": 460.0, "text": "The risk is the apps that cache sessions."},
                {"speaker": "remote", "name": "Omar Haddad", "start": 468.2, "end": 468.6, "text": "If the cutover slips again we keep the old tenant <b>one</b> more week."},
                {"speaker": "remote", "name": "Kim Park", "start": 468.7, "end": 480.0, "text": "Right."},
                {"speaker": "me", "start": 485.0, "end": 490.0, "text": "Fine by us."},
                {"speaker": "remote", "name": "Dana Lee", "start": 500.0, "end": 505.0, "text": "Then Saturday stays."},
            ])
            .to_string(),
        )
        .unwrap();
        let threads = json!([{"id": "acme-rollout", "name": "Acme rollout", "client": "acme"}]);
        let call_md = "# Acme go-live check\n\n## Minutes\n\n### Decisions\n- Cutover moves to Saturday 6 AM [04:12]\n\n## Transcript\n\n### [07:31] Dana Lee, Omar Haddad\n\n[07:31] Dana Lee (Acme): The risk is the apps that cache sessions.\n\n[07:48] Omar Haddad (Acme): If the cutover slips again we keep the old tenant <b>one</b> more week.\n";
        let inputs = vec![
            input(&format!("call/{}", &name[..17]), "call", "Acme go-live check", &day(1), call_md, json!({"kind": "call", "call": name, "threads": threads, "client": "acme"}), None),
            input("mail/aa11", "mail", "Re: SSO cutover window", &day(2), "# Re: SSO cutover window\n\n### 2026-11-16 09:12 Dana Lee\n\n- The helpdesk staffs Saturday — “confirm the cutover starts at 6”\n", json!({"threads": threads, "client": "acme"}), Some("Dana Lee")),
            input("channel/bb22/1", "channel", "Globex pilot › General", &day(3), "# Globex pilot › General\n\n### 2026-11-15 10:00 Kim Park\n\n- The cutover rehearsal passed\n", json!({"threads": [{"id": "globex-pilot", "name": "Globex pilot", "client": "globex"}], "client": "globex"}), Some("Kim Park")),
        ];
        store::sync(&crate::brain::store_path(), inputs, None).unwrap();
        let _ = root;
        name
    }

    fn sources_of(v: &Value) -> Vec<String> {
        v["sources"].as_array().unwrap().iter().map(|s| s["source"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn with_intake_off_the_search_says_so_and_finds_nothing() {
        let (_env, _root) = crate::paths::testenv::folders("window-search-off");
        assert_eq!(get("/api/search?q=cutover"), json!({"enabled": false}));
    }

    #[test]
    fn what_was_said_is_grouped_by_source_with_its_moments_and_the_words_looked_for() {
        let (_env, root) = crate::paths::testenv::folders("window-search-grouped");
        let name = made_up(&root);
        let v = get("/api/search?q=the%20cutover");
        assert_eq!(v["enabled"], true);
        assert_eq!(v["words"], json!(["cutover"]), "common words aren't looked for");
        let srcs = sources_of(&v);
        assert_eq!(srcs.len(), 3, "{v}");
        let call = v["sources"].as_array().unwrap().iter().find(|s| s["kind"] == "call").unwrap();
        assert_eq!(call["title"], "Acme go-live check");
        assert_eq!(call["client"], "acme");
        assert_eq!(call["call"], name.as_str());
        let ms = call["moments"].as_array().unwrap();
        let said = ms.iter().find(|m| m["section_kind"] == "transcript").expect("the transcript's moment");
        assert_eq!(said["who"], "Omar Haddad", "the turn with the word, its organisation left out");
        assert_eq!(said["t"], 468.0);
        assert_eq!(said["link"], format!("echo://call/{name}?t=468"), "opens at the turn said, not the section's start");
        let decided = ms.iter().find(|m| m["section_kind"] == "decisions").expect("the decision's moment");
        assert_eq!((decided["heading"].as_str(), decided["t"].as_f64(), decided["text"].as_str()), (Some("Decisions"), Some(252.0), Some("Cutover moves to Saturday 6 AM")));
        let mail = v["sources"].as_array().unwrap().iter().find(|s| s["kind"] == "mail").unwrap();
        assert_eq!(mail["moments"][0]["who"], "Dana Lee");
        // The calls' cards aren't what was said: the call's title is the titles' own match.
        assert!(ms.iter().all(|m| m["section_kind"] != "card"));
        let t = get("/api/search?q=go-live%20check");
        assert_eq!(t["titles"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect::<Vec<_>>(), vec![name.as_str()]);
    }

    #[test]
    fn a_client_or_a_thread_scopes_the_search_and_the_filters_narrow_it() {
        let (_env, root) = crate::paths::testenv::folders("window-search-scoped");
        made_up(&root);
        let srcs = |q: &str| sources_of(&get(&format!("/api/search?q=cutover&{q}")));
        assert!(srcs("client=acme").iter().all(|s| !s.starts_with("channel/")), "Globex's post is out of Acme's scope");
        assert_eq!(srcs("client=globex"), vec!["channel/bb22/1".to_string()]);
        assert_eq!(srcs("thread=globex-pilot"), vec!["channel/bb22/1".to_string()]);
        assert_eq!(srcs("kind=mail"), vec!["mail/aa11".to_string()]);
        assert_eq!(srcs("kind=mail,channel").len(), 2);
        let decisions = get("/api/search?q=cutover&section=decisions");
        assert_eq!(decisions["filters"]["sections"], json!(["decisions"]));
        assert!(decisions["sources"].as_array().unwrap().iter().flat_map(|s| s["moments"].as_array().unwrap()).all(|m| m["section_kind"] == "decisions"));
        assert!(srcs("person=omar").iter().all(|s| s.starts_with("call/")));
        // Mail only: no titles (they are calls).
        assert_eq!(get("/api/search?q=go-live&kind=mail")["titles"], json!([]));
        // A query past its longest is cut, not refused.
        let long = format!("cutover{}", " acme".repeat(400));
        let cut = get(&format!("/api/search?q={}", long.replace(' ', "%20")));
        assert_eq!(cut["query"].as_str().unwrap().chars().count(), QUERY_MAX);
        // A filter that isn't one is said, not searched.
        assert!(get("/api/search?q=cutover&kind=fax")["error"].as_str().unwrap().contains("no source kind"));
    }

    #[test]
    fn a_date_in_the_query_is_a_filter_and_past_the_index_says_older() {
        let (_env, root) = crate::paths::testenv::folders("window-search-dates");
        made_up(&root);
        let v = get("/api/search?q=cutover%20yesterday");
        assert_eq!(v["filters"]["dates_from_query"], "yesterday");
        assert_eq!(v["words"], json!(["cutover"]), "the date's words aren't looked for");
        assert_eq!(v["filters"]["since"], v["filters"]["until"]);
        let long_ago = (chrono::Local::now().date_naive() - chrono::Duration::days(200)).to_string();
        let old = get(&format!("/api/search?q=cutover&until={long_ago}"));
        assert_eq!((old["older"].as_bool(), old["sources"].as_array().map(Vec::len)), (Some(true), Some(0)));
    }

    #[test]
    fn a_moment_comes_with_the_turns_around_it_as_data_never_markup() {
        let (_env, root) = crate::paths::testenv::folders("window-search-moment");
        let name = made_up(&root);
        let v = get("/api/search?q=cutover&section=transcript");
        let chunk = v["sources"][0]["moments"][0]["chunk"].as_str().unwrap().to_string();
        let m = get(&format!("/api/search/moment?chunk={chunk}&q=cutover"));
        assert_eq!(m["call"]["name"], name.as_str());
        assert_eq!(m["call"]["title"], "Acme go-live check");
        let turns = m["turns"].as_array().unwrap();
        // Two turns start at 07:48 (the index's clock has no fractions): the one with the words is the moment.
        assert_eq!(turns.iter().map(|t| t["who"].as_str().unwrap()).collect::<Vec<_>>(), vec!["Dana Lee", "Omar Haddad", "Kim Park"], "the turn before, the one said, the one after");
        assert_eq!(turns.iter().map(|t| t["hit"] == true).collect::<Vec<_>>(), vec![false, true, false]);
        // Others' words reach the page as they were said: the page escapes them (window guide R1).
        assert!(turns[1]["text"].as_str().unwrap().contains("<b>one</b>"));
        assert!(m["moment"]["text"].as_str().unwrap().contains("<b>one</b>"));
        // A mail's moment: its section's facts, no call.
        let mail = get("/api/search?q=cutover&kind=mail");
        let chunk = mail["sources"][0]["moments"][0]["chunk"].as_str().unwrap().to_string();
        let mm = get(&format!("/api/search/moment?chunk={chunk}"));
        assert_eq!((mm["kind"].as_str(), mm["call"].is_null()), (Some("mail"), true));
        assert_eq!(mm["lines"][0], "The helpdesk staffs Saturday — “confirm the cutover starts at 6”");
        assert_eq!(get("/api/search/moment?chunk=999999")["error"], "no such moment");
    }
}

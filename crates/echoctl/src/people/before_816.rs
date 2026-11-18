//! `people::view` as it was before #816, its scans over every pair of people kept as they were: the comparison
//! test proves the indexes that replaced them give the same view. When the view's output changes on purpose,
//! this copy goes (or changes with it).

use super::*;

/// Did the user say these two are different people?
pub(super) fn apart(dir_people: &[Person], a: &str, b: &str) -> bool {
    dir_people.iter().any(|p| (p.id == a || p.emails.iter().any(|e| e == a)) && p.not_same.iter().any(|x| x == b))
        || dir_people.iter().any(|p| (p.id == b || p.emails.iter().any(|e| e == b)) && p.not_same.iter().any(|x| x == a))
}

/// The People tab for some projects (one thread, or a client's threads): every person their calls
/// show or the user placed, with what Echo saw, the directory's fields and each project's placement;
/// and the organisations involved, with their relationships and placements.
pub fn view(threads: &[&Project], today: chrono::NaiveDate) -> Result<Value> {
    // Every organisation, accounts or not: whose people they are.
    let clients = projects::orgs()?;
    let dir_people = directory()?;
    let me = MeIs::now();
    let my_domain = crate::config::Config::load().ok().and_then(|c| c.me).and_then(|m| domain(m.email.as_deref()));
    let mut dirs: Vec<PathBuf> = vec![];
    // Each call's threads (by stamp), for the threads someone was seen in.
    let mut call_threads: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in threads {
        for (d, _) in projects::calls_of(p)? {
            call_threads.entry(calls::stamp_of(&d)).or_default().push(p.id.clone());
            if !dirs.contains(&d) {
                dirs.push(d);
            }
        }
    }
    let (mut seen, total) = gather(&dirs, &me, &dir_people);
    let ids: BTreeSet<&str> = threads.iter().map(|p| p.id.as_str()).collect();
    // Facts said in these threads put someone in them too (a mail's sender who was in no call).
    for p in &dir_people {
        if !seen.contains_key(&p.id) && p.facts.iter().any(|f| f["thread"].as_str().is_some_and(|t| ids.contains(t))) {
            seen.insert(p.id.clone(), Seen { name: p.name.clone(), email: p.emails.first().cloned(), ..Default::default() });
        }
    }
    // Who is in calls with whom: for each person, the three they shared most calls with (two or more).
    let mut with: BTreeMap<String, Vec<(usize, String)>> = BTreeMap::new();
    {
        let keys: Vec<&String> = seen.keys().collect();
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                let n = seen[*a].calls.intersection(&seen[*b].calls).count();
                if n >= 2 {
                    with.entry((*a).clone()).or_default().push((n, (*b).clone()));
                    with.entry((*b).clone()).or_default().push((n, (*a).clone()));
                }
            }
        }
        for v in with.values_mut() {
            v.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(&y.1)));
            v.truncate(3);
        }
    }
    let canon = canon(&dir_people);
    let member_key = |m: &projects::Member| m.email.as_ref().map(|e| e.to_lowercase()).and_then(|e| canon.get(&e).cloned()).unwrap_or_else(|| key_of(&m.name, m.email.as_deref()));
    // People placed by hand who no call shows (yet).
    for p in threads {
        for m in &p.people {
            seen.entry(member_key(m)).or_insert_with(|| Seen { name: display(&m.name), email: m.email.clone(), ..Default::default() });
        }
    }
    let work = work_of(threads, today);
    let call_days: Vec<String> = dirs.iter().filter_map(|d| calls::read_json(&d.join("meta.json"))["started_at"].as_str().map(|s| s.get(..10).unwrap_or("").to_string())).collect();
    let last_call = call_days.iter().max().cloned().unwrap_or_default();
    // "New" = joined an engagement already running: first seen more than a day after its first call.
    let first_call = call_days.iter().min().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());
    let joined_later = |first: &str| match (first_call, chrono::NaiveDate::parse_from_str(first, "%Y-%m-%d").ok()) {
        (Some(f), Some(x)) => x > f + chrono::Duration::days(1),
        _ => false,
    };
    let days = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok().map(|x| (today - x).num_days());
    let book = colleagues();
    let mut people: Vec<Value> = seen
        .into_iter()
        .map(|(key, mut s)| {
            let entry = dir_people.iter().find(|p| p.id == key || s.email.as_ref().is_some_and(|e| p.emails.contains(e)));
            let f = |k: &str| entry.and_then(|p| p.fields.get(k).cloned());
            // A colleague shown on their own address; a client address they were invited on stays in `also`.
            if let Some(i) = s.also.iter().position(|e| book.address(e)).filter(|_| !s.email.as_deref().is_some_and(|e| book.address(e))) {
                let own = s.also.remove(i);
                s.also.extend(s.email.replace(own));
            }
            // Addresses the user merged in that no call shows (a person added by hand).
            for e in entry.into_iter().flat_map(|p| &p.emails) {
                if s.email.as_ref() != Some(e) && !s.also.contains(e) {
                    s.also.push(e.clone());
                }
            }
            let colleague = s.email.as_deref().is_some_and(|e| book.address(e)) || entry.is_some_and(|p| p.emails.iter().any(|e| book.address(e)));
            let dom = domain(s.email.as_deref());
            let org = f("org").and_then(|o| clients.iter().find(|c| c.id == o)).or_else(|| org_of(&clients, dom.as_deref())).filter(|c| !colleague || c.internal || f("org").is_some());
            let side = match (org, &dom) {
                (Some(c), _) if c.internal => "yours",
                (None, _) if colleague => "yours",
                (Some(_), _) => "org",
                (None, Some(d)) if Some(d) == my_domain.as_ref() => "yours",
                (None, Some(_)) => "other",
                (None, None) => "unknown",
            };
            let places: Vec<Value> = threads
                .iter()
                .filter_map(|p| {
                    let m = p.people.iter().find(|m| member_key(m) == key || m.email.as_ref().is_some_and(|e| s.also.contains(&e.to_lowercase())) || (m.email.is_none() && s.email.is_some() && norm(&m.name) == norm(&s.name)))?;
                    Some(json!({"thread": p.id, "role": m.role, "stance": m.place.stance, "influence": m.place.influence, "interest": m.place.interest,
                                "group": m.group, "governance": m.governance, "works_for": m.works_for,
                                "role_by": m.role_by, "role_at": m.role_at, "role_from": m.role_from}))
                })
                .collect();
            let n = norm(entry.map(|p| p.name.as_str()).unwrap_or(&s.name));
            let first_name = n.split(' ').next().unwrap_or("").to_string();
            let (owes, late, waiting) = work.iter().filter(|(w, ..)| *w == n || *w == first_name).fold((0, 0, 0), |a, (_, x, y, z)| (a.0 + x, a.1 + y, a.2 + z));
            let in_thread = !places.is_empty();
            let new = in_thread && !s.first.is_empty() && days(&s.first).is_some_and(|d| d <= NEW_DAYS) && joined_later(&s.first);
            let quiet = in_thread && !s.last.is_empty() && days(&s.last).is_some_and(|d| d > QUIET_DAYS) && last_call > s.last;
            // What the threads' words say about them, newest first; and where they were seen.
            let mut facts: Vec<Value> = entry.map(|p| p.facts.iter().filter(|f| f["thread"].as_str().is_some_and(|t| ids.contains(t))).cloned().collect()).unwrap_or_default();
            facts.reverse();
            let mut seen_in: BTreeSet<String> = s.calls.iter().flat_map(|c| call_threads.get(c).cloned().unwrap_or_default()).collect();
            seen_in.extend(places.iter().filter_map(|p| p["thread"].as_str().map(str::to_owned)));
            seen_in.extend(facts.iter().filter_map(|f| f["thread"].as_str().map(str::to_owned)));
            let touches = s.calls.iter().cloned().chain(facts.iter().filter_map(|f| f["evidence"].as_str().map(str::to_owned))).collect::<BTreeSet<_>>().len();
            let with_them: Vec<Value> = with.get(&key).into_iter().flatten().map(|(n, k)| json!({"key": k, "calls": n})).collect();
            let addresses: Vec<Value> = s.email.iter().chain(&s.also).map(|e| {
                let d = domain(Some(e));
                json!({"email": e, "org": org_of(&clients, d.as_deref()).map(|c| c.name.clone()), "domain": d})
            }).collect();
            json!({
                "key": key, "name": entry.map(|p| p.name.clone()).unwrap_or(s.name), "email": s.email, "also": s.also, "addresses": addresses, "domain": dom,
                "org": org.map(|c| json!({"id": c.id, "name": c.name})), "side": side,
                "title": f("title"), "department": f("department"), "company": f("company"), "office": f("office"), "questions": entry.map(|p| json!(p.questions)).unwrap_or(json!([])), "group": f("group"), "reports_to": f("reports_to"), "matters": f("matters"), "notes": f("notes"), "phone": f("phone"),
                "sources": entry.map(|p| json!(p.sources)).unwrap_or(json!({})), "history": entry.map(|p| json!(p.history)).unwrap_or(json!([])),
                "calls": s.calls.len(), "of_calls": dirs.len(), "invited": s.invited, "chat": s.chat,
                "share": if total > 0.0 { (s.spoke / total * 100.0).round() } else { 0.0 },
                "first": s.first, "last": s.last, "new": new, "quiet": quiet,
                "owes": owes, "late": late, "waiting": waiting,
                "places": places, "in_thread": in_thread,
                "about": f("about"), "about_by": entry.and_then(|p| p.sources.get("about").cloned()), "about_thread": f("about_thread"),
                "facts": facts, "threads": seen_in, "touches": touches, "with": with_them,
            })
        })
        .collect();
    // Maybe the same person: names (or addresses) that share a first and a last name, unless the user
    // said they aren't. A suggestion in the panel, never merged by itself.
    let words: Vec<(String, BTreeSet<String>)> = people
        .iter()
        .map(|p| {
            let mut t = tokens(&display(p["name"].as_str().unwrap_or("")));
            if t.len() < 2 {
                t.extend(p["email"].as_str().and_then(|e| e.split('@').next()).map(tokens).unwrap_or_default());
            }
            (p["key"].as_str().unwrap_or("").to_string(), t)
        })
        .collect();
    let mut maybe: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for (i, (ka, ta)) in words.iter().enumerate() {
        for (kb, tb) in words.iter().skip(i + 1) {
            if ta.len() >= 2 && tb.len() >= 2 && ta.intersection(tb).count() >= 2 && !apart(&dir_people, ka, kb) {
                let a = &people[i];
                let b = people.iter().find(|p| p["key"] == kb.as_str()).unwrap();
                maybe.entry(ka.clone()).or_default().push(json!({"key": kb, "name": b["name"], "email": b["email"]}));
                maybe.entry(kb.clone()).or_default().push(json!({"key": ka, "name": a["name"], "email": a["email"]}));
            }
        }
    }
    for p in people.iter_mut() {
        p["maybe_same"] = json!(maybe.remove(p["key"].as_str().unwrap_or("")).unwrap_or_default());
    }
    // The chart's order and its suggestions: seniority by title, whom they work for here as the work
    // says (a name in the ledger → a person here: the full name, else a first name only one has).
    let hints = works_for_hints(threads);
    let by_name: Vec<(String, String)> = people.iter().map(|p| (norm(p["name"].as_str().unwrap_or("")), p["key"].as_str().unwrap_or("").to_string())).collect();
    let key_of_name = |n: &str| {
        by_name.iter().find(|(x, _)| x == n).map(|(_, k)| k.clone()).or_else(|| {
            let first: Vec<&(String, String)> = by_name.iter().filter(|(x, _)| x.split(' ').next() == Some(n)).collect();
            (first.len() == 1).then(|| first[0].1.clone())
        })
    };
    // Handing a task over isn't leading someone: governance, PMs and partners coordinate everyone. A
    // hint only within one workstream — the same one, or its lead's.
    let part: BTreeMap<String, (Option<String>, bool, Option<String>)> = people.iter().map(|p| {
        let pl = &p["places"][0];
        let role = pl["role"].as_str().unwrap_or("");
        let stream = pl["group"].as_str().or(p["group"].as_str()).map(str::to_owned).or_else(|| role_stream(role));
        let lead = role_stream(role).filter(|_| role.to_lowercase().contains("lead")).or_else(|| stream.clone().filter(|_| role.to_lowercase().contains("lead")));
        let coord = pl["governance"] == true || coordinates(role) || (pl["governance"].is_null() && pl["influence"] == 3 && pl["interest"] == 3 && stream.is_none());
        (p["key"].as_str().unwrap_or("").to_string(), (stream, coord, lead))
    }).collect();
    for p in people.iter_mut() {
        p["seniority"] = json!(seniority(p["title"].as_str().unwrap_or("")));
        let n = norm(p["name"].as_str().unwrap_or(""));
        let first = n.split(' ').next().unwrap_or("").to_string();
        let mine = part.get(p["key"].as_str().unwrap_or("")).and_then(|x| x.0.clone());
        let fits = |k: &String| part.get(k).is_some_and(|(stream, coord, lead)| {
            let same = |a: &Option<String>| a.as_deref().zip(mine.as_deref()).is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
            !coord && (same(stream) || same(lead))
        });
        if let Some(k) = hints.get(&n).or_else(|| hints.get(&first)).and_then(|l| key_of_name(l)).filter(|k| p["key"] != k.as_str()).filter(fits) {
            p["works_for_hint"] = json!(k);
        }
    }
    people.sort_by(|a, b| b["in_thread"].as_bool().cmp(&a["in_thread"].as_bool()).then(b["calls"].as_u64().cmp(&a["calls"].as_u64())).then(a["name"].as_str().cmp(&b["name"].as_str())));
    // Organisations: the threads' clients, the ones their people come from, and those related to them.
    let mut ids: BTreeSet<String> = threads.iter().filter_map(|p| p.client.clone()).collect();
    ids.extend(people.iter().filter(|p| p["in_thread"] == true).filter_map(|p| p["org"]["id"].as_str().map(str::to_owned)));
    ids.extend(threads.iter().flat_map(|p| p.orgs.iter().map(|o| o.client.clone())));
    let related: Vec<String> = clients.iter().filter(|c| ids.contains(&c.id)).flat_map(|c| c.parent.iter().cloned().chain(c.relations.iter().map(|r| r.to.clone()))).collect();
    ids.extend(related);
    let orgs: Vec<Value> = clients
        .iter()
        .filter(|c| ids.contains(&c.id))
        .map(|c| {
            let places: Vec<Value> = threads
                .iter()
                .filter_map(|p| p.orgs.iter().find(|o| o.client == c.id).map(|o| json!({"thread": p.id, "role": o.role, "stance": o.place.stance, "influence": o.place.influence, "interest": o.place.interest})))
                .collect();
            json!({"id": c.id, "name": c.name, "internal": c.internal, "account": c.account, "parent": c.parent, "relations": c.relations, "groups": c.groups,
                   "domains": c.domains, "logo_v": projects::client_logo_v(&c.id), "places": places,
                   "people": people.iter().filter(|p| p["org"]["id"] == c.id.as_str()).count()})
        })
        .collect();
    let pending: Vec<Value> = threads.iter().flat_map(|p| suggestions(&p.id).into_iter().filter(|x| x["state"] == "pending").map(move |mut x| { x["thread"] = json!(p.id); x })).collect();
    Ok(json!({"people": people, "orgs": orgs, "calls": dirs.len(), "suggestions": pending, "threads": threads.iter().map(|p| json!({"id": p.id, "name": p.name})).collect::<Vec<_>>()}))
}

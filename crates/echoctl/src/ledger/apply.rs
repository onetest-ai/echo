//! The ledger: apply: agent proposals; verify (split out of `ledger.rs`, moved as it was).

use super::*;

// ── apply: agent proposals ───────────────────────────────────────────────────────────────────────

/// Owners as the project knows them: a member's name for their name, email, or unique first name.
pub(super) fn canonical(p: &crate::projects::Project, who: Option<&str>) -> Option<String> {
    let w = who?.trim();
    if w.is_empty() {
        return None;
    }
    let lw = w.to_lowercase();
    if let Some(m) = p.people.iter().find(|m| m.name.to_lowercase() == lw || m.email.as_deref().is_some_and(|e| e.to_lowercase() == lw)) {
        return Some(m.name.clone());
    }
    let first: Vec<&crate::projects::Member> = p.people.iter().filter(|m| m.name.split_whitespace().next().is_some_and(|f| f.to_lowercase() == lw)).collect();
    if first.len() == 1 {
        return Some(first[0].name.clone());
    }
    Some(w.to_string())
}

pub(super) fn key_str(v: &Value) -> String {
    if v.is_null() { "None".into() } else { v.to_string() }
}

/// Apply one evidence item's proposals: each checked against the item's words and the tree's rules.
/// Returns `{events, by_op, rejected}`.
pub fn apply(project: &str, evid: &str, proposals: &[Value]) -> Result<Value> {
    let l = Ledger::of(project)?;
    let p = crate::projects::load(project)?;
    let _lock = l.lock()?;
    let ev = l.evidence();
    let item = ev.get(evid).ok_or_else(|| anyhow!("unknown evidence {evid:?}"))?.clone();
    let full = norm(&full_text(&item));
    let evs = l.events()?;
    let mut n = fold_events(&evs, ev.clone())?.nodes;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for e in &evs {
        if e["type"] == "create" {
            *counts.entry(s(&e["kind"]).unwrap_or_default()).or_default() += 1;
        }
    }
    let mut seen: BTreeSet<String> = evs.iter().filter_map(|e| s(&e["mid"])).collect();
    let mut made: BTreeMap<String, String> = evs.iter().filter(|e| e["type"] == "create").filter_map(|e| Some((s(&e["mid"])?, s(&e["id"])?))).collect();
    let mut handles: BTreeMap<String, String> = BTreeMap::new();
    let (mut out, mut rej): (Vec<Value>, Vec<Value>) = (vec![], vec![]);
    let date = s(&item["date"]).unwrap_or_default();
    // A due nobody said (#463): its words aren't in the item, and the date is the item's own (a mail's date taken as the
    // due). Kept, but guessed: it never makes the task late.
    let guessed = |text: Option<&str>, due: Option<&str>| due.is_some_and(|d| d == date.get(..10).unwrap_or("")) && text.is_none_or(|t| norm(t).is_empty() || !full.contains(&norm(t)));
    let turns = item["turns"].as_array().cloned().unwrap_or_default();
    let locate = |q: &str| -> (Value, Value) {
        let head: String = q.chars().take(40).collect();
        for t in &turns {
            if norm(t["text"].as_str().unwrap_or("")).contains(&head) {
                return (t["ts"].clone(), json!(canonical(&p, t["speaker"].as_str())));
            }
        }
        (Value::Null, Value::Null)
    };
    let base = |pr: &Value, q: &str, key: &[String]| -> Value {
        let (ts, sp) = locate(q);
        let mut parts: Vec<&str> = vec![evid, q];
        parts.extend(key.iter().map(String::as_str));
        json!({"mid": mid_of(&parts), "evidence": evid, "date": date, "source": item["source"], "kind": item["kind"], "ts": ts, "speaker": sp,
               "quote": pr["quote"], "quote_norm": q, "note": pr["note"], "by": "agent"})
    };
    for (i, pr) in proposals.iter().enumerate() {
        let r = |x: &Value, h: &BTreeMap<String, String>| x.as_str().map(|v| h.get(v).cloned().unwrap_or_else(|| v.to_string()));
        let q = norm(pr["quote"].as_str().unwrap_or(""));
        let op = pr["op"].as_str().unwrap_or("");
        let why = if q.split(' ').count() < 3 {
            Some("quote too short".to_string())
        } else if !full.contains(&q) {
            Some("quote not verbatim in this evidence item (one contiguous span)".to_string())
        } else if !AGENT_OPS.contains(&op) {
            Some(format!("bad op {op:?}"))
        } else {
            None
        };
        if let Some(w) = why {
            rej.push(json!({"i": i, "why": w, "quote": pr["quote"].as_str().unwrap_or("").chars().take(80).collect::<String>()}));
            continue;
        }
        if op == "create" {
            let k = pr["kind"].as_str().unwrap_or("");
            let par = r(&pr["parent"], &handles);
            let pk = par.as_deref().and_then(|x| n.get(x)).map(|x| x.kind.clone());
            let ty = pr["type"].as_str().unwrap_or("");
            let ac = pr["ac"].as_array().cloned().unwrap_or_default();
            let title = pr["title"].as_str().unwrap_or("").trim().to_string();
            let why = if !KINDS.contains(&k) {
                Some(format!("bad kind {k:?}"))
            } else if k == "epic" && pr["plan"] != "Off-plan" {
                Some("agents create workstreams (epics) only for off-plan work: plan must be 'Off-plan'".into())
            } else if title.is_empty() {
                Some("a create needs a title".into())
            } else if k != "epic" && !parent_ok(k, pk.as_deref()) {
                Some(format!("a {k} needs a parent of the right kind, got {par:?} ({pk:?})"))
            } else if k == "story" && ac.is_empty() {
                Some("a story needs acceptance criteria (ac: [...]): what must be true for it to be done".into())
            } else if k == "story" && ac.len() > MAX_AC {
                Some(format!("more than {MAX_AC} acceptance criteria: split it into two stories and link them"))
            } else if pr["description"].as_str().is_some_and(|t| t.chars().count() > MAX_AGENT_DESC) {
                Some(format!("description: at most {MAX_AGENT_DESC} characters"))
            } else if (k == "question" || k == "risk") && pr["description"].as_str().is_some_and(|t| !t.trim().is_empty()) {
                Some(format!(
                    "a {k}'s {what} (its `description`) is checked by the verifier: create the {k} without `description`, then op 'answer' on its ref with the {what} as `text`",
                    what = if k == "question" { "answer" } else { "response" }
                ))
            } else if let Some(f) = ["likelihood", "impact"].into_iter().find(|f| !pr[*f].is_null() && (k != "risk" || level_of(&pr[*f]).is_none())) {
                Some(format!("{f} is for a risk: one of {LEVELS:?}"))
            } else if k == "task" && !TASK_TYPES.contains(&ty) {
                Some(format!("task type must be one of {TASK_TYPES:?}"))
            } else if k == "task" && !(pr["internal_owner"].is_string() || pr["client_owner"].is_string() || pr["due_text"].is_string() || ty == "session" || ty == "dependency") {
                Some("admission bar: a task needs an owner, a date, or to be a session/dependency; else op 'note'".into())
            } else {
                None
            };
            if let Some(w) = why {
                rej.push(json!({"i": i, "why": w, "title": pr["title"]}));
                continue;
            }
            let cmid = mid_of(&[evid, &q, "create", k, par.as_deref().unwrap_or("None"), &title]);
            let handle = pr["ref"].as_str().map(str::to_owned).unwrap_or_else(|| format!("#{i}"));
            if let Some(id) = made.get(&cmid) {
                handles.insert(handle, id.clone());
                continue;
            }
            let c = counts.entry(k.to_string()).or_default();
            *c += 1;
            let nid = new_id(k, *c);
            handles.insert(handle, nid.clone());
            let mut e = base(pr, &q, &["create".into(), nid.clone()]);
            e["mid"] = json!(cmid);
            e["type"] = json!("create");
            e["id"] = json!(nid);
            e["kind"] = json!(k);
            e["fields"] = json!({
                "title": title, "parent": if k == "epic" { None } else { par.clone() }, "type": pr["type"],
                "plan": if k == "epic" { Some("Off-plan") } else { None },
                "internal_owner": canonical(&p, pr["internal_owner"].as_str()), "client_owner": canonical(&p, pr["client_owner"].as_str()),
                "delegated_by": canonical(&p, pr["delegated_by"].as_str()),
                "client_owed": pr["client_owed"] == true, "ac": ac,
                "due": resolve_due(pr["due_text"].as_str(), &date), "due_text": pr["due_text"],
                "due_guessed": guessed(pr["due_text"].as_str(), resolve_due(pr["due_text"].as_str(), &date).as_deref()),
                "description": pr["description"].as_str().map(str::trim).filter(|t| !t.is_empty()),
                "decided_by": if k == "decision" { canonical(&p, pr["decided_by"].as_str()) } else { None },
                "likelihood": pr["likelihood"], "impact": pr["impact"],
            });
            n.insert(nid.clone(), node_of(&e));
            made.insert(cmid, nid.clone());
            out.push(e);
            for d in pr["depends_on"].as_array().cloned().unwrap_or_default() {
                let d = r(&d, &handles).unwrap_or_default();
                if n.get(&d).is_some_and(|x| x.kind == "story" || x.kind == "task") && (k == "story" || k == "task") {
                    let mut le = base(pr, &q, &["link".into(), nid.clone(), d.clone()]);
                    le["type"] = json!("agent");
                    le["op"] = json!("link");
                    le["node"] = json!(nid);
                    le["depends_on"] = json!(d);
                    out.push(le);
                }
            }
            continue;
        }
        let node = r(&pr["node"], &handles).unwrap_or_default();
        if op == "note" {
            if !n.contains_key(&node) {
                rej.push(json!({"i": i, "why": format!("note needs a known node, got {node:?}")}));
                continue;
            }
            if pr["note"].as_str().is_none_or(|t| t.trim().is_empty()) {
                rej.push(json!({"i": i, "why": "note needs its text in `note`"}));
                continue;
            }
            let mut e = base(pr, &q, &["note".into(), node.clone()]);
            e["type"] = json!("note");
            e["node"] = json!(node);
            e["text"] = pr["note"].clone();
            out.push(e);
            continue;
        }
        let Some(x) = n.get(&node) else {
            rej.push(json!({"i": i, "why": format!("unknown node {node:?}")}));
            continue;
        };
        let later: Vec<String> = [Some(node.clone()), r(&pr["depends_on"], &handles), r(&pr["into"], &handles)]
            .into_iter()
            .flatten()
            .filter(|y| n.get(y).is_some_and(|z| z.created.as_deref().unwrap_or("") > date.as_str()))
            .collect();
        if !later.is_empty() {
            rej.push(json!({"i": i, "why": format!("{later:?} did not exist yet on {date}")}));
            continue;
        }
        let mut key: Vec<String> = ["effect", "depends_on", "into", "parent", "ac", "what"].iter().map(|k| key_str(&pr[*k])).collect();
        if op == "answer" {
            key.push(key_str(&pr["text"]));
        }
        if op == "revise" {
            key.push(key_str(&pr["title"]));
            key.push(key_str(&pr["ac_add"]));
            key.push(key_str(&pr["description"]));
        }
        let mut k2 = vec![op.to_string(), node.clone()];
        k2.extend(key);
        let mut e = base(pr, &q, &k2);
        e["type"] = json!("agent");
        e["op"] = json!(op);
        e["node"] = json!(node);
        let mid = s(&e["mid"]).unwrap_or_default();
        if seen.contains(&mid) {
            continue;
        }
        let bad = |w: String, rej: &mut Vec<Value>| rej.push(json!({"i": i, "why": w}));
        match op {
            "status" => {
                let eff = pr["effect"].as_str().unwrap_or("");
                if !STATUS.contains(&eff) && eff != "mention" {
                    bad(format!("bad effect {eff:?}"), &mut rej);
                    continue;
                }
                if eff == "due" && resolve_due(pr["due_text"].as_str(), &date).is_none() {
                    bad(format!("no date can be read from due_text {:?}: write it as a date (\"December 18\", \"next Friday\", \"2026-12-18\")", pr["due_text"].as_str().unwrap_or("")), &mut rej);
                    continue;
                }
                e["effect"] = json!(eff);
                e["due_text"] = pr["due_text"].clone();
                e["due"] = if eff == "delayed" { Value::Null } else { json!(resolve_due(pr["due_text"].as_str(), &date)) };
                if guessed(pr["due_text"].as_str(), e["due"].as_str()) {
                    e["due_guessed"] = json!(true);
                }
                e["internal_owner"] = json!(canonical(&p, pr["internal_owner"].as_str()));
                e["client_owner"] = json!(canonical(&p, pr["client_owner"].as_str()));
                e["delegated_by"] = json!(canonical(&p, pr["delegated_by"].as_str()));
            }
            "revise" => {
                let add = pr["ac_add"].as_array().map(|a| a.len()).unwrap_or(0);
                let desc = pr["description"].as_str().map(str::trim).filter(|t| !t.is_empty());
                if !pr["title"].is_string() && add == 0 && desc.is_none() {
                    bad("revise needs a new title, ac_add and/or description".into(), &mut rej);
                    continue;
                }
                if desc.is_some_and(|t| t.chars().count() > MAX_AGENT_DESC) {
                    bad(format!("description: at most {MAX_AGENT_DESC} characters"), &mut rej);
                    continue;
                }
                if desc.is_some() && (x.kind == "question" || x.kind == "risk") {
                    bad(format!("a {}'s answer or response is op 'answer' (the verifier checks it)", x.kind), &mut rej);
                    continue;
                }
                if x.ac.len() + add > MAX_AC {
                    bad(format!("{node} would have more than {MAX_AC} criteria: create a second story"), &mut rej);
                    continue;
                }
                e["title"] = pr["title"].clone();
                e["ac_add"] = pr["ac_add"].clone();
                e["description"] = json!(desc);
            }
            "ac_met" => {
                let a = pr["ac"].as_u64().unwrap_or(0) as usize;
                if x.kind != "story" || a == 0 || a > x.ac.len() {
                    bad("ac_met needs a story node and the 1-based number of one of its criteria".into(), &mut rej);
                    continue;
                }
                let live = x.ac[a - 1].text.clone();
                if let Some(saw) = pr["ac_text"].as_str() {
                    if norm(saw) != norm(&live) {
                        bad(format!("criterion {a} of {node} is now “{live}”, not “{saw}”: the criteria changed since you read context; re-read it"), &mut rej);
                        continue;
                    }
                }
                e["ac"] = json!(a);
                e["ac_text"] = json!(live);
            }
            "link" => {
                let d = r(&pr["depends_on"], &handles).unwrap_or_default();
                let ok = n.get(&d).is_some_and(|y| y.kind == "story" || y.kind == "task") && (x.kind == "story" || x.kind == "task");
                if !ok {
                    bad(format!("link joins stories/tasks; got {node} -> {d}"), &mut rej);
                    continue;
                }
                e["depends_on"] = json!(d);
            }
            "answer" => {
                let text = pr["text"].as_str().map(str::trim).filter(|t| !t.is_empty());
                if x.kind != "question" && x.kind != "risk" {
                    bad(format!("answer is for a question (its answer) or a risk (its response); {node} is a {}", x.kind), &mut rej);
                    continue;
                }
                let Some(text) = text else {
                    bad("answer needs its text: the answer, or what is done about the risk".into(), &mut rej);
                    continue;
                };
                if text.chars().count() > MAX_AGENT_DESC {
                    bad(format!("text: at most {MAX_AGENT_DESC} characters"), &mut rej);
                    continue;
                }
                if let Some(f) = ["likelihood", "impact"].into_iter().find(|f| !pr[*f].is_null() && (x.kind != "risk" || level_of(&pr[*f]).is_none())) {
                    bad(format!("{f} is for a risk: one of {LEVELS:?}"), &mut rej);
                    continue;
                }
                e["text"] = json!(text);
                if x.kind == "risk" {
                    e["internal_owner"] = json!(canonical(&p, pr["internal_owner"].as_str()));
                    e["likelihood"] = pr["likelihood"].clone();
                    e["impact"] = pr["impact"].clone();
                }
            }
            "waiting" => {
                if x.kind != "story" && x.kind != "task" {
                    bad("waiting applies to a story or task".into(), &mut rej);
                    continue;
                }
                e["who"] = json!(canonical(&p, pr["who"].as_str()));
                e["what"] = pr["what"].clone();
            }
            "suggest_move" => {
                let to = r(&pr["parent"], &handles).unwrap_or_default();
                if !n.get(&to).is_some_and(|y| parent_ok(&x.kind, Some(&y.kind))) {
                    bad(format!("cannot move {node} under {to}"), &mut rej);
                    continue;
                }
                e["parent"] = json!(to);
            }
            "suggest_merge" => {
                let to = r(&pr["into"], &handles).unwrap_or_default();
                if n.get(&to).is_none_or(|y| y.kind != x.kind) || to == node {
                    bad(format!("merge needs two nodes of one kind; got {node} -> {to}"), &mut rej);
                    continue;
                }
                e["into"] = json!(to);
            }
            _ => {}
        }
        seen.insert(mid);
        out.push(e);
    }
    // The same work twice (#39: a mail and the call after it asked for one thing): a new story or task whose
    // title shares most of its words with an open one of this thread is offered as "same as …?" — To review,
    // never merged by itself.
    let mut same: Vec<Value> = vec![];
    // Words that say what the work is (not "the", "with", "for"…), and pairs offered once.
    const STOP: [&str; 16] = ["the", "and", "for", "with", "from", "into", "onto", "this", "that", "their", "them", "our", "your", "about", "over", "via"];
    let key_words = |t: &str| -> BTreeSet<String> { words(t).into_iter().filter(|w| !STOP.contains(&w.as_str())).collect() };
    let mut pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for e in out.iter().filter(|e| e["type"] == "create" && (e["kind"] == "story" || e["kind"] == "task")) {
        let (id, kind) = (e["id"].as_str().unwrap_or(""), e["kind"].as_str().unwrap_or(""));
        let mine = key_words(e["fields"]["title"].as_str().unwrap_or(""));
        if mine.len() < 2 {
            continue;
        }
        // Most of both titles in common (shared over all their words), at least two words of it.
        let best = n.values().filter(|x| x.kind == kind && x.id != id && !closed(&x.state)).filter_map(|x| {
            let theirs = key_words(&x.title);
            let shared = mine.intersection(&theirs).count();
            let ratio = shared as f64 / mine.union(&theirs).count().max(1) as f64;
            (shared >= 2 && ratio >= 0.6).then_some((ratio, x))
        }).max_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, x)) = best {
            let pair = if id < x.id.as_str() { (id.to_string(), x.id.clone()) } else { (x.id.clone(), id.to_string()) };
            if !pairs.insert(pair) {
                continue;
            }
            let mid = mid_of(&["same", evid, id, &x.id]);
            if seen.insert(mid.clone()) {
                same.push(json!({"type": "agent", "op": "suggest_merge", "node": id, "into": x.id, "by": "agent", "evidence": evid, "date": date, "mid": mid,
                                 "note": format!("Same as {} “{}”? Their titles share most words (Echo's check, no model).", x.id, x.title)}));
            }
        }
    }
    out.extend(same);
    l.append(&out)?;
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for e in &out {
        let k = if e["type"] == "create" { format!("create_{}", e["kind"].as_str().unwrap_or("")) } else { e["op"].as_str().or(e["type"].as_str()).unwrap_or("").to_string() };
        *by.entry(k).or_default() += 1;
    }
    // What it made and touched, by id: the next item and the dependency pass refer to them (#39).
    let created: Vec<Value> = out.iter().filter(|e| e["type"] == "create").map(|e| json!({"id": e["id"], "kind": e["kind"], "title": e["fields"]["title"], "parent": e["fields"]["parent"]})).collect();
    let touched: BTreeSet<&str> = out.iter().filter(|e| e["type"] != "create").filter_map(|e| e["node"].as_str()).collect();
    Ok(json!({"events": out.len(), "by_op": by, "rejected": rej, "created": created, "touched": touched}))
}

// ── verify ──────────────────────────────────────────────────────────────────────────────────────

/// The claims a verifier must judge (each with its words in context). `weak`: also those judged weak.
/// How many claims wait for the verifier (`verify_prep`'s count, without building the packets).
pub fn verify_count(project: &str) -> Result<usize> {
    let evs = Ledger::of(project)?.events()?;
    let judged: BTreeSet<String> = evs.iter().filter(|e| e["type"] == "verdict").filter_map(|e| s(&e["mid"])).collect();
    Ok(evs.iter().filter(|e| needs_verdict(e) && s(&e["mid"]).is_none_or(|m| !judged.contains(&m))).count())
}

pub fn verify_prep(project: &str, weak: bool) -> Result<Vec<Value>> {
    let l = Ledger::of(project)?;
    let evs = l.events()?;
    let ev = l.evidence();
    let latest: BTreeMap<String, String> = evs.iter().filter(|e| e["type"] == "verdict").filter_map(|e| Some((s(&e["mid"])?, s(&e["verdict"])?))).collect();
    let n = fold_events(&evs, ev.clone())?.nodes;
    let mut out = vec![];
    let others = super::mail::Others::new(project);
    let name = crate::projects::load(project).map(|p| p.name).unwrap_or_default();
    for e in evs.iter().filter(|e| needs_verdict(e)) {
        let mid = s(&e["mid"]).unwrap_or_default();
        if let Some(v) = latest.get(&mid) {
            if !(weak && v == "weak") {
                continue;
            }
        }
        let item = e["evidence"].as_str().and_then(|i| ev.get(i)).cloned().unwrap_or(Value::Null);
        let turns = item["turns"].as_array().cloned().unwrap_or_default();
        let head: String = e["quote_norm"].as_str().unwrap_or("").chars().take(40).collect();
        let k = turns.iter().position(|t| norm(t["text"].as_str().unwrap_or("")).contains(&head));
        let ctx: Vec<String> = match k {
            Some(k) => turns[k.saturating_sub(2)..(k + 3).min(turns.len())].iter().map(|t| format!("[{}] {}: {}", t["ts"].as_str().unwrap_or(""), t["speaker"].as_str().unwrap_or(""), t["text"].as_str().unwrap_or(""))).collect(),
            None => vec![item["text"].as_str().unwrap_or("").chars().take(1200).collect()],
        };
        let node = e["node"].as_str().unwrap_or("");
        let x = n.get(node);
        let claim = match e["op"].as_str().unwrap_or("") {
            "ac_met" => format!("acceptance criterion {} met: {}", e["ac"], e["ac_text"].as_str().unwrap_or("?")),
            "link" => {
                let d = e["depends_on"].as_str().unwrap_or("");
                format!("cannot finish until “{}” ({d}) does", n.get(d).map(|y| y.title.as_str()).unwrap_or(d))
            }
            "waiting" => match e["what"].as_str() {
                Some(w) if !w.is_empty() => format!("is waiting on {}: {w}", e["who"].as_str().unwrap_or("someone")),
                _ => "is no longer waiting on anyone (what it waited for arrived or was dropped)".into(),
            },
            "answer" if x.is_some_and(|y| y.kind == "risk") => format!("what is done about this risk: {}", e["text"].as_str().unwrap_or("")),
            "answer" => format!("this question is answered: {}", e["text"].as_str().unwrap_or("")),
            _ => e["effect"].as_str().unwrap_or("").to_string(),
        };
        let mut packet = json!({"mid": mid, "node": node, "kind": x.map(|y| &y.kind), "path": path(&n, node), "claimed": claim, "due": e["due"],
                        "quote": e["quote"], "evidence": format!("{} {}", e["date"].as_str().unwrap_or(""), item["title"].as_str().unwrap_or("")), "context": ctx});
        // An item in several threads: the verifier checks the change is about this thread's work.
        if let Some(a) = e["evidence"].as_str().and_then(|i| others.as_ref()?.of(i, &item)) {
            packet["thread"] = json!(name);
            packet["also_in"] = a;
        }
        out.push(packet);
    }
    Ok(out)
}

pub fn verify_apply(project: &str, verdicts: &[Value]) -> Result<Value> {
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    let mids: BTreeSet<String> = l.events()?.iter().filter(|e| needs_verdict(e)).filter_map(|e| s(&e["mid"])).collect();
    let (mut out, mut rej) = (vec![], vec![]);
    for v in verdicts {
        let ok = v["mid"].as_str().is_some_and(|m| mids.contains(m)) && ["supports", "weak", "wrong"].contains(&v["verdict"].as_str().unwrap_or(""));
        if !ok {
            rej.push(v.clone());
            continue;
        }
        out.push(json!({"type": "verdict", "mid": v["mid"], "verdict": v["verdict"], "reason": v["reason"], "by": "verifier"}));
    }
    l.append(&out)?;
    let mut c: BTreeMap<String, usize> = BTreeMap::new();
    for v in &out {
        *c.entry(s(&v["verdict"]).unwrap_or_default()).or_default() += 1;
    }
    Ok(json!({"verdicts": c, "rejected": rej}))
}

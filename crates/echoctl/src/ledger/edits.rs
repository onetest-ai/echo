//! The ledger: the user's edits, linked threads and the tree (split out of `ledger.rs`, moved as it was).

use super::*;

// ── the user's edits ─────────────────────────────────────────────────────────────────────────────

pub(super) fn check_edit(sets: &Value, n: &BTreeMap<String, Node>) -> Result<()> {
    let Some(m) = sets.as_object() else { bail!("set must be an object of {HUMAN_FIELDS:?}") };
    for (k, v) in m {
        match k.as_str() {
            "due" if !v.is_null() => {
                let d = NaiveDate::parse_from_str(v.as_str().unwrap_or(""), "%Y-%m-%d").map_err(|_| anyhow!("due must be YYYY-MM-DD or null"))?;
                // A date no work item is due on: what a date box gives while its year is half typed (#303).
                if !(2000..=2100).contains(&d.year()) {
                    bail!("due must be a date between 2000 and 2100, not {d}");
                }
            }
            "due" => {}
            "state" if !HUMAN_STATES.contains(&v.as_str().unwrap_or("")) => bail!("state must be one of {HUMAN_STATES:?}"),
            "waiting_for" if !v.is_null() && !(v.is_object() && (v["what"].is_string() || v["who"].is_string())) => bail!("waiting_for is null or {{who, what}}"),
            "parent" if !v.as_str().is_some_and(|p| n.contains_key(p)) => bail!("unknown parent {v}"),
            "client_owed" if !v.is_boolean() => bail!("client_owed is true or false"),
            "column" if !(v.is_null() || v.as_str().is_some_and(|c| !c.is_empty() && c.len() <= 40)) => bail!("column is a column id"),
            "title" if v.as_str().is_none_or(|t| t.trim().is_empty()) => bail!("title cannot be empty"),
            "internal_owner" | "client_owner" | "delegated_by" | "decided_by" | "type" if !(v.is_null() || v.is_string()) => bail!("{k} must be text"),
            "likelihood" | "impact" if !(v.is_null() || level_of(v).is_some()) => bail!("{k} is null or one of {LEVELS:?}"),
            "links" => check_links(v)?,
            "mode" if !(v.is_null() || v.as_str().is_some_and(|m| crate::taskmodes::MODES.contains(&m))) => bail!("mode is null or one of {:?}", crate::taskmodes::MODES),
            "how" if !(v.is_null() || v.as_str().is_some_and(|t| t.chars().count() <= crate::taskmodes::MAX_HOW)) => bail!("how is text of at most {} characters", crate::taskmodes::MAX_HOW),
            "prompt" if !(v.is_null() || v.as_str().is_some_and(|t| t.chars().count() <= crate::taskmodes::MAX_PROMPT)) => bail!("prompt is text of at most {} characters", crate::taskmodes::MAX_PROMPT),
            "skill" if !(v.is_null() || v.as_str().is_some_and(crate::skills::valid_id)) => bail!("skill is null or a skill's id"),
            "description" if !(v.is_null() || v.as_str().is_some_and(|t| t.chars().count() <= MAX_DESC)) => bail!("description is text of at most {MAX_DESC} characters"),
            k if !HUMAN_FIELDS.contains(&k) => bail!("editable fields: {HUMAN_FIELDS:?}"),
            _ => {}
        }
    }
    Ok(())
}

/// An edit the user made (`{op, node, set|text|ac|met|drop|depends_on|into|target, …}` or a create). A link
/// to an item in another thread is written in both ledgers (`link_threads`), and so is its removal — an
/// undo of one included.
pub fn human_event(project: &str, body: &Value) -> Result<Value> {
    match body["op"].as_str().unwrap_or("") {
        "xref" => {
            let master_here = body["master"] == "here";
            return link_threads(project, body["node"].as_str().unwrap_or(""), body["thread"].as_str().unwrap_or(""), body["target"].as_str().unwrap_or(""), body["kind"].as_str().unwrap_or("same"), master_here);
        }
        "unxref" => return unlink_threads(project, body["node"].as_str().unwrap_or(""), body["thread"].as_str().unwrap_or(""), body["target"].as_str().unwrap_or("")),
        // Close on a mirror's Looks done (#463, #20) closes its master — the state is the master's; the accept here
        // remembers that edit, so its undo takes both back.
        "accept" => {
            let target = body["target"].as_str().unwrap_or("");
            let evs = Ledger::of(project)?.events()?;
            if let Some(m) = evs.iter().find(|x| x["mid"] == target && x["effect"] == "mention" && x["covers"] == true) {
                let raw = fold(&Ledger::of(project)?)?;
                let master = raw.nodes.get(m["node"].as_str().unwrap_or("")).and_then(|x| x.xrefs.iter().find(|r| r["kind"] == "same" && r["master"] == "there").cloned());
                if let Some(master) = master {
                    let th = master["thread"].as_str().unwrap_or("");
                    let fwd = human_event_here(th, &json!({"op": "edit", "node": master["node"], "set": {"state": "done"}}))?;
                    return human_event_here(project, &json!({"op": "accept", "target": target, "forwarded": {"thread": th, "mid": fwd["mid"]}}));
                }
            }
        }
        "undo" => {
            let evs = Ledger::of(project)?.events()?;
            // A Close that also closed the master: undone there too.
            if let Some(f) = evs.iter().find(|e| e["mid"] == body["target"] && e["op"] == "accept").map(|e| e["forwarded"].clone()).filter(|f| f.is_object()) {
                let _ = human_event_here(f["thread"].as_str().unwrap_or(""), &json!({"op": "undo", "target": f["mid"]}));
            }
            if let Some(x) = evs.iter().find(|e| e["mid"] == body["target"] && e["type"] == "human" && (e["op"] == "xref" || e["op"] == "unxref")) {
                let (node, thread, target) = (x["node"].as_str().unwrap_or(""), x["thread"].as_str().unwrap_or(""), x["target"].as_str().unwrap_or(""));
                // The pair's latest link event: undoing an older one would undo something else.
                let latest = evs.iter().rev().find(|e| e["type"] == "human" && (e["op"] == "xref" || e["op"] == "unxref") && e["node"] == node && e["thread"] == thread && e["target"] == target);
                if latest.is_some_and(|l| l["mid"] != x["mid"]) {
                    bail!("this link was changed since: undo the newer change, or link / unlink it again");
                }
                if x["op"] == "xref" {
                    return unlink_threads(project, node, thread, target);
                }
                // Undo an unlink: link them again as they were.
                let before = evs.iter().rev().find(|e| e["type"] == "human" && e["op"] == "xref" && e["node"] == node && e["thread"] == thread && e["target"] == target).ok_or_else(|| anyhow!("nothing to link again"))?;
                return link_threads(project, node, thread, target, before["kind"].as_str().unwrap_or("same"), before["master"] == "here");
            }
        }
        "edit" => {
            // A mirror's state, owners and due are its master's (#20): changed there, or they would never show.
            let node = body["node"].as_str().unwrap_or("");
            let raw = fold(&Ledger::of(project)?)?;
            let master = raw.nodes.get(node).and_then(|x| x.xrefs.iter().find(|r| r["kind"] == "same" && r["master"] == "there").cloned());
            if let (Some(m), Some(set)) = (master, body["set"].as_object()) {
                let (theirs, mine): (serde_json::Map<String, Value>, serde_json::Map<String, Value>) =
                    set.clone().into_iter().partition(|(k, _)| MIRRORED.contains(&k.as_str()));
                let mut out = Value::Null;
                if !theirs.is_empty() {
                    out = human_event_here(m["thread"].as_str().unwrap_or(""), &json!({"op": "edit", "node": m["node"], "set": theirs}))?;
                }
                if !mine.is_empty() {
                    out = human_event_here(project, &json!({"op": "edit", "node": node, "set": mine}))?;
                }
                return Ok(out);
            }
        }
        _ => {}
    }
    human_event_here(project, body)
}

/// A mirror's fields that are its master's (#20).
pub(super) const MIRRORED: [&str; 6] = ["state", "internal_owner", "client_owner", "due", "waiting_for", "delegated_by"];

/// Link `node` here to `target` in `thread` (#20), in both ledgers. `same`: one piece of work, its master
/// here or there; `depends_on`: this one waits on that one (the other records `blocks`).
pub fn link_threads(project: &str, node: &str, thread: &str, target: &str, kind: &str, master_here: bool) -> Result<Value> {
    if thread == project {
        bail!("a link to an item in the same thread is a dependency (link), not a cross-thread link");
    }
    if !["same", "depends_on"].contains(&kind) {
        bail!("kind is same or depends_on");
    }
    // Both items must be there before either side is written.
    for (p, id) in [(project, node), (thread, target)] {
        let t = fold(&Ledger::of(p)?)?;
        if t.nodes.get(id).is_none_or(|x| CLOSED.contains(&x.state.as_str())) {
            bail!("{p} has no open item {id}");
        }
    }
    let (here, there) = match kind {
        "same" => (if master_here { "here" } else { "there" }, if master_here { "there" } else { "here" }),
        _ => ("there", "there"),
    };
    let back = if kind == "depends_on" { "blocks" } else { "same" };
    let e = human_event_here(project, &json!({"op": "xref", "node": node, "thread": thread, "target": target, "kind": kind, "master": here}))?;
    if let Err(err) = human_event_here(thread, &json!({"op": "xref", "node": target, "thread": project, "target": node, "kind": back, "master": there})) {
        // Never a link on one side only.
        let _ = human_event_here(project, &json!({"op": "unxref", "node": node, "thread": thread, "target": target}));
        return Err(err.context(format!("{thread} couldn't take the link")));
    }
    Ok(e)
}

/// Take a cross-thread link away, in both ledgers: each item is on its own again, with its own last values.
pub fn unlink_threads(project: &str, node: &str, thread: &str, target: &str) -> Result<Value> {
    let e = human_event_here(project, &json!({"op": "unxref", "node": node, "thread": thread, "target": target}))?;
    if Ledger::of(thread).is_ok() {
        let _ = human_event_here(thread, &json!({"op": "unxref", "node": target, "thread": project, "target": node}));
    }
    Ok(e)
}

pub(super) fn human_event_here(project: &str, body: &Value) -> Result<Value> {
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    ensure_first_epic(&l)?;
    let evs = l.events()?;
    let t = fold_events(&evs, l.evidence())?;
    let n = &t.nodes;
    let op = body["op"].as_str().unwrap_or("");
    if !HUMAN_OPS.contains(&op) {
        bail!("bad op {op:?}");
    }
    let last = t.order.last().cloned();
    let now = Local::now().to_rfc3339();
    let date = last.as_ref().and_then(|c| s(&t.evidence[c]["date"])).unwrap_or_else(|| now[..10].to_string());
    let mut e = json!({"type": "human", "op": op, "by": "human", "evidence": last, "date": date, "at": now});
    // The user's own words with an edit (why they closed it), kept in its history.
    if let Some(note) = body["note"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
        e["note"] = json!(note.chars().take(500).collect::<String>());
    }
    if op == "create" {
        let k = body["kind"].as_str().unwrap_or("");
        let par = body["parent"].as_str();
        if !KINDS.contains(&k) || (k != "epic" && !parent_ok(k, par.and_then(|p| n.get(p)).map(|x| x.kind.as_str()))) {
            bail!("cannot create a {k} under {par:?}");
        }
        check_links(&body["links"])?;
        for f in ["likelihood", "impact"] {
            if !(body[f].is_null() || (k == "risk" && level_of(&body[f]).is_some())) {
                bail!("{f} is for a risk: one of {LEVELS:?}");
            }
        }
        if !(body["description"].is_null() || body["description"].as_str().is_some_and(|t| t.chars().count() <= MAX_DESC)) {
            bail!("description is text of at most {MAX_DESC} characters");
        }
        let c = evs.iter().filter(|x| x["type"] == "create" && x["kind"] == k).count() + 1;
        e = json!({"type": "create", "id": new_id(k, c), "kind": k, "by": "human", "evidence": last, "date": date, "at": now,
                   "fields": {"title": body["title"].as_str().filter(|t| !t.trim().is_empty()).unwrap_or("Untitled"),
                              "parent": if k == "epic" { None } else { par }, "type": body["type"].as_str().or(if k == "task" { Some("action") } else { None }),
                              "ac": body["ac"].as_array().cloned().unwrap_or_default(), "plan": if k == "epic" { Some("Off-plan") } else { None },
                              "internal_owner": body["internal_owner"], "delegated_by": body["delegated_by"], "due": body["due"], "links": body["links"], "description": body["description"],
                              "decided_by": body["decided_by"], "likelihood": body["likelihood"], "impact": body["impact"]}});
    } else if ["undo", "accept", "reject"].contains(&op) {
        let target = body["target"].as_str().ok_or_else(|| anyhow!("target mid required"))?;
        if !evs.iter().any(|x| x["mid"] == target) {
            bail!("unknown event {target}");
        }
        e["target"] = json!(target);
        if body["forwarded"].is_object() {
            e["forwarded"] = body["forwarded"].clone();
        }
    } else {
        let node = body["node"].as_str().filter(|x| n.contains_key(*x)).ok_or_else(|| anyhow!("unknown node {}", body["node"]))?;
        e["node"] = json!(node);
        for k in ["set", "text", "ac", "met", "drop", "depends_on", "into", "thread", "target", "kind", "master"] {
            if !body[k].is_null() {
                e[k] = body[k].clone();
            }
        }
        if (op == "xref" || op == "unxref") && !(body["thread"].as_str().is_some_and(|t| !t.is_empty() && t != project) && body["target"].is_string()) {
            bail!("{op} needs another thread and its item (thread, target)");
        }
        if op == "xref" && !(XREF_KINDS.contains(&body["kind"].as_str().unwrap_or("")) && ["here", "there"].contains(&body["master"].as_str().unwrap_or("there"))) {
            bail!("kind is one of {XREF_KINDS:?}, master here or there");
        }
        if op == "edit" {
            check_edit(&e["set"], n)?;
        }
        if op == "ac_set" && !body["ac"].is_u64() {
            bail!("ac must be a 1-based number");
        }
        if (op == "link" || op == "unlink") && !body["depends_on"].as_str().is_some_and(|d| n.contains_key(d)) {
            bail!("unknown node {}", body["depends_on"]);
        }
        if op == "merge" && !body["into"].as_str().is_some_and(|d| n.contains_key(d)) {
            bail!("unknown node {}", body["into"]);
        }
    }
    e["mid"] = json!(mid_of(&["human", &now, &body.to_string()]));
    l.append(std::slice::from_ref(&e))?;
    Ok(e)
}

/// Status changes an agent made from a mirror's own evidence (#20): applied, verified, in its history — the
/// ones that move work along (not mentions).
pub(super) fn status_moves(x: &Node) -> Vec<Value> {
    x.history.iter().filter(|h| h["type"] == "agent" && h["op"] == "status" && h["applied"] == true && h["verdict"] != "unsupported").cloned().collect()
}

/// When a history entry happened, comparable across entries: a call's day and time for an agent's, the
/// moment the user acted for theirs.
pub(super) fn when(h: &Value) -> String {
    match h["at"].as_str().filter(|_| h["by"] == "human") {
        Some(at) => at.chars().take(19).collect(),
        None => format!("{}T{}", h["date"].as_str().unwrap_or(""), h["ts"].as_str().unwrap_or("00:00:00")),
    }
}

/// A ledger folded with its master items taking the status changes their mirrors' threads heard (one
/// level: the mirrors' own ledgers as they are) — newer than the master's own last word on its state.
pub(super) fn fold_forwarded(project: &str) -> Result<Tree> {
    let mut t = fold(&Ledger::of(project)?)?;
    let mut mirrors: BTreeMap<String, Option<Tree>> = BTreeMap::new();
    let ids: Vec<String> = t.nodes.keys().cloned().collect();
    let mut moved = false;
    for id in ids {
        let refs: Vec<Value> = t.nodes[&id].xrefs.iter().filter(|r| r["kind"] == "same" && r["master"] == "here").cloned().collect();
        for r in refs {
            let (th, node) = (r["thread"].as_str().unwrap_or("").to_string(), r["node"].as_str().unwrap_or("").to_string());
            let other = mirrors.entry(th.clone()).or_insert_with(|| Ledger::of(&th).and_then(|l| fold(&l)).ok());
            let Some(m) = other.as_ref().and_then(|o| o.nodes.get(&node)).cloned() else { continue };
            let x = t.nodes.get_mut(&id).unwrap();
            x.also_in.push(json!({"thread": th, "node": node, "title": m.title}));
            // A merged or deleted master takes nothing.
            if x.state == "merged" || x.state == "deleted" {
                continue;
            }
            let last_here = x.history.iter().filter(|h| h["op"] == "status" || (h["op"] == "edit" && h["set"]["state"].is_string())).map(when).max().unwrap_or_default();
            for h in status_moves(&m).into_iter().filter(|h| when(h) > last_here) {
                if apply_status(x, &h) {
                    let mut h = h.clone();
                    h["from"] = json!({"thread": th, "node": node});
                    x.history.push(h);
                    moved = true;
                }
            }
            x.due = x.due_history.last().and_then(|h| s(&h["due"]));
            x.due_guessed = x.due.is_some() && x.due_history.last().is_some_and(|h| h["guessed"] == true);
        }
    }
    if moved {
        derive(&mut t.nodes);
    }
    Ok(t)
}

/// A ledger as the window, Tasks and the extractor read it (#20): masters with their mirrors' status
/// changes; each mirror showing its master's state, owners, due and last history entries; each item that
/// waits on an open item of another thread with it in `xwaits`.
pub fn fold_linked(project: &str) -> Result<Tree> {
    let mut t = fold_forwarded(project)?;
    let mut others: BTreeMap<String, Option<Tree>> = BTreeMap::new();
    let mut get = |th: &str, node: &str| -> Option<Node> { others.entry(th.to_string()).or_insert_with(|| fold_forwarded(th).ok()).as_ref().and_then(|t| t.nodes.get(node).cloned()) };
    let ids: Vec<String> = t.nodes.keys().cloned().collect();
    for id in &ids {
        let refs = t.nodes[id].xrefs.clone();
        for r in refs {
            let (th, node, kind) = (r["thread"].as_str().unwrap_or("").to_string(), r["node"].as_str().unwrap_or("").to_string(), r["kind"].as_str().unwrap_or(""));
            let Some(mut o) = get(&th, &node) else { continue };
            // An item that is itself a mirror stands for its master (one more step).
            if let Some(m) = o.xrefs.iter().find(|x| x["kind"] == "same" && x["master"] == "there").cloned() {
                if let Some(mm) = get(m["thread"].as_str().unwrap_or(""), m["node"].as_str().unwrap_or("")) {
                    o.state = mm.state;
                    o.internal_owner = mm.internal_owner;
                    o.client_owner = mm.client_owner;
                    o.due = mm.due;
                    o.due_guessed = mm.due_guessed;
                }
            }
            let x = t.nodes.get_mut(id).unwrap();
            match kind {
                "same" if r["master"] == "there" => {
                    x.state = o.state.clone();
                    x.internal_owner = o.internal_owner.clone();
                    x.client_owner = o.client_owner.clone();
                    x.delegated_by = o.delegated_by.clone();
                    x.due = o.due.clone();
                    // Guessed or not as the master's due is (#463).
                    x.due_guessed = o.due_guessed;
                    x.waiting_for = o.waiting_for.clone();
                    let last: Vec<Value> = o.history.iter().rev().take(3).rev().cloned().collect();
                    // A master in a closed thread isn't on Tasks: the mirror is shown there instead.
                    let closed_thread = crate::projects::load(&th).map(|p| p.stage.closed()).unwrap_or(true);
                    x.mirror_of = Some(json!({"thread": th, "node": node, "title": o.title, "history": last, "closed_thread": closed_thread}));
                }
                "depends_on" if !closed(&o.state) => {
                    let owner = o.internal_owner.clone().or(o.client_owner.clone());
                    x.xwaits.push(json!({"thread": th, "node": node, "title": o.title, "owner": owner, "state": o.state}));
                }
                _ => {}
            }
        }
    }
    // A local wait on a mirror is a wait on its master's work (Tasks shows the master, not the mirror).
    for id in &ids {
        let waits: Vec<String> = t.nodes[id].waiting_on.clone();
        for w in waits {
            let Some(m) = t.nodes.get(&w).and_then(|y| y.mirror_of.clone()).filter(|m| m["closed_thread"] != true) else { continue };
            let (state, owner) = (t.nodes[&w].state.clone(), t.nodes[&w].internal_owner.clone());
            let x = t.nodes.get_mut(id).unwrap();
            x.waiting_on.retain(|v| v != &w);
            if !closed(&state) {
                x.xwaits.push(json!({"thread": m["thread"], "node": m["node"], "title": m["title"], "owner": owner, "state": state}));
            }
        }
    }
    Ok(t)
}

/// Open stories, tasks and questions of every open thread (but `not`) whose title has every word of `q`
/// (no model; at most 20, the threads' newest first): what an item can be linked to (#20).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn search_open(q: &str, not: Option<&str>) -> Vec<Value> {
    let words: Vec<String> = norm(q).split_whitespace().map(str::to_owned).collect();
    if words.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for p in crate::projects::list().into_iter().filter(|p| !p.stage.closed() && Some(p.id.as_str()) != not) {
        let Ok(t) = Ledger::of(&p.id).and_then(|l| fold(&l)) else { continue };
        for x in t.nodes.values().filter(|x| ["story", "task", "question"].contains(&x.kind.as_str()) && !closed(&x.state)) {
            let title = norm(&x.title);
            if words.iter().all(|w| title.contains(w.as_str())) || x.id.eq_ignore_ascii_case(q.trim()) {
                out.push(json!({"thread": p.id, "thread_name": p.name, "id": x.id, "title": x.title, "kind": x.kind, "state": x.state, "owner": x.internal_owner.as_ref().or(x.client_owner.as_ref())}));
            }
        }
        if out.len() >= 20 {
            break;
        }
    }
    out.truncate(20);
    out
}

/// The tree for the window and `ledger tree`: every node, the workstreams first.
/// The tasks a call raised or moved in these threads that are still open — what is still to do from it (a call's
/// action items aren't ticked; the tasks they became are): `{task, owner, due, state, link}`, each once.
pub fn open_tasks_from(projects: &[String], stamp: &str) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for p in projects {
        let Ok(c) = call_changes(p, stamp) else { continue };
        for x in c["changes"].as_array().into_iter().flatten() {
            let n = &x["node"];
            let open = n["state"].as_str().is_some_and(|s| !closed(s));
            // A change the verifier called wrong didn't happen (What moved hides it too).
            if x["verdict"] == "wrong" || n["kind"] != "task" || !open || out.iter().any(|o| o["link"] == format!("echo://task/{p}/{}", n["id"].as_str().unwrap_or(""))) {
                continue;
            }
            out.push(json!({"task": n["title"], "owner": n["owner"], "due": n["due"], "state": n["state"],
                            "link": format!("echo://task/{p}/{}", n["id"].as_str().unwrap_or(""))}));
        }
    }
    out
}

/// `open_tasks_from` for a call folder: its threads, its stamp.
pub fn open_tasks_of_call(dir: &std::path::Path) -> Vec<Value> {
    let projects: Vec<String> = crate::projects::of_call(dir).map(|ps| ps.into_iter().map(|(p, _)| p.id).collect()).unwrap_or_default();
    open_tasks_from(&projects, &calls::stamp_of(dir))
}

/// What one call changed in a project's work (What moved, beside its minutes): every history entry whose
/// evidence is this call, by node, in the order said — new nodes, status changes, dates, links,
/// waits, criteria met — with the words, the verifier's mark, and the node as it is now. Mentions
/// are left out (they change nothing). `read`: the sync has read the call; `queued`: it will.
pub fn call_changes(project: &str, stamp: &str) -> Result<Value> {
    let l = Ledger::of(project)?;
    let t = fold(&l)?;
    let read = t.order.iter().any(|e| e == stamp);
    let queued = !read && pending(project)?.iter().any(|e| e["id"] == stamp);
    let mut out: Vec<Value> = vec![];
    let (me, modes) = (crate::tasks::Me::of(crate::config::Config::load().ok().and_then(|c| c.me).as_ref()), crate::taskmodes::guesses());
    for n in t.nodes.values().filter(|n| n.state != "merged" && n.state != "deleted" && n.kind != "epic") {
        for h in n.history.iter().filter(|h| h["evidence"] == stamp && h["by"] != "human" && h["effect"] != "mention" && !h["op"].as_str().is_some_and(|o| o.starts_with("suggest_"))) {
            let mut c = h.clone();
            c["node"] = json!({"id": n.id, "kind": n.kind, "type": n.task_type, "title": n.title, "state": n.state, "due": n.due,
                               "owner": n.internal_owner.as_ref().or(n.client_owner.as_ref()),
                               // The user's own open task: who does it (the window's What moved beside the minutes).
                               "mine": n.kind == "task" && crate::tasks::owner_of(n).is_some_and(|o| me.is(o)),
                               "mode": crate::taskmodes::of(&modes, project, n)});
            if let Some(d) = h["depends_on"].as_str().and_then(|d| t.nodes.get(d)) {
                c["depends_on_title"] = json!(d.title);
            }
            out.push(c);
        }
    }
    out.sort_by(|a, b| a["ts"].as_str().unwrap_or("").cmp(b["ts"].as_str().unwrap_or("")));
    Ok(json!({"read": read, "queued": queued, "changes": out}))
}

pub(super) fn words(s: &str) -> BTreeSet<String> {
    norm(s).split_whitespace().filter(|w| w.len() > 2).map(str::to_owned).collect()
}



pub fn tree(project: &str) -> Result<Value> {
    let t = fold_linked(project)?;
    let pend = pending(project)?.len();
    let mut nodes = serde_json::to_value(&t.nodes)?;
    let modes = crate::taskmodes::guesses();
    for (id, x) in nodes.as_object_mut().into_iter().flatten() {
        if let Some(d) = x["description"].as_str().map(crate::minutes::markdown_html) {
            x["description_html"] = json!(d);
        }
        // Who does it, the user's choice or Echo's guess (the task's details show and change it).
        if let Some(n) = t.nodes.get(id).filter(|n| n.kind == "task") {
            x["who_does"] = crate::taskmodes::of(&modes, project, n);
        }
    }
    Ok(json!({
        "nodes": nodes, "notes": t.notes,
        "processed": t.order.len(), "pending": pend,
        // Each evidence item's call: history entries name the call they came from.
        "evidence": t.evidence.iter().map(|(k, v)| (k.clone(), json!({"title": v["title"], "call": v["call"], "date": v["date"], "kind": v["kind"], "url": v["url"],
            "link": v["call"].as_str().map(|c| format!("echo://call/{c}")), "from": v["turns"][0]["speaker"].as_str().filter(|_| v["kind"] == "mail")}))).collect::<serde_json::Map<String, Value>>(),
        "roots": t.nodes.values().filter(|x| x.kind == "epic" && x.state != "merged" && x.state != "deleted").map(|x| &x.id).collect::<Vec<_>>(),
    }))
}

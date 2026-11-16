//! The ledger: an Echo plugin's tasks in a thread (`ctx.tasks`, uses.echo.write: tasks). A plugin writes only to
//! the tasks of its own keys, in a story of its own; a field the user set is never written again; with `propose`
//! a write waits for the user's Yes, and after their No the same write stays quiet.

use super::*;

/// The task states a plugin may set.
const PLUGIN_STATES: [&str; 5] = ["open", "in_progress", "blocked", "done", "cancelled"];

/// A plugin's event's plugin and key.
pub(super) fn plugin_key(e: &Value) -> Option<(String, String)> {
    Some((e["plugin"].as_str()?.to_string(), e["key"].as_str()?.to_string()))
}

/// What a plugin asks to set on its task (`{key, title, due?, owner?, state?, url?}`), as the ledger's fields; a
/// field it leaves out stays as it is (`due: null` clears it).
fn wanted(name: &str, t: &Value, needs_title: bool) -> Result<serde_json::Map<String, Value>> {
    let mut m = serde_json::Map::new();
    match t["title"].as_str().map(str::trim) {
        Some(x) if !x.is_empty() && x.chars().count() <= 200 => {
            m.insert("title".into(), json!(x));
        }
        None if !needs_title => {}
        _ => bail!("a task needs a title (≤ 200 characters)"),
    }
    if let Some(d) = t.get("due") {
        if !d.is_null() {
            let day = NaiveDate::parse_from_str(d.as_str().unwrap_or(""), "%Y-%m-%d").map_err(|_| anyhow!("due is YYYY-MM-DD or null"))?;
            if !(2000..=2100).contains(&day.year()) {
                bail!("due is a date between 2000 and 2100, not {day}");
            }
        }
        m.insert("due".into(), d.clone());
    }
    if let Some(o) = t.get("owner") {
        if !(o.is_null() || o.as_str().is_some_and(|o| !o.trim().is_empty() && o.chars().count() <= 80)) {
            bail!("owner is a name (≤ 80 characters) or null");
        }
        m.insert("internal_owner".into(), o.as_str().map(|o| json!(o.trim())).unwrap_or(Value::Null));
    }
    if let Some(st) = t.get("state") {
        if !PLUGIN_STATES.contains(&st.as_str().unwrap_or("")) {
            bail!("state is one of {PLUGIN_STATES:?}");
        }
        m.insert("state".into(), st.clone());
    }
    if let Some(u) = t.get("url") {
        let link = json!({"title": name, "url": u});
        if !(u.is_null() || (u.as_str().is_some_and(|u| u.starts_with("https://")) && link_ok(&link))) {
            bail!("url is an https:// address or null");
        }
        m.insert("links".into(), if u.is_null() { json!([]) } else { json!([link]) });
    }
    Ok(m)
}

fn key_of(key: &str) -> Result<String> {
    let k = key.trim();
    if k.is_empty() || k.chars().count() > 120 || k.chars().any(char::is_control) {
        bail!("a task's key is its own id in the plugin (≤ 120 characters)");
    }
    Ok(k.to_string())
}

fn reason_of(r: Option<&str>) -> Option<String> {
    r.map(str::trim).filter(|r| !r.is_empty()).map(|r| r.chars().take(300).collect())
}

/// A field's value on a task as a plugin's write compares it.
fn current(x: &Node, field: &str) -> Value {
    match field {
        "title" => json!(x.title),
        "due" => json!(x.due),
        "internal_owner" => json!(x.internal_owner),
        "state" => json!(x.state),
        "links" => json!(x.links),
        _ => Value::Null,
    }
}

/// The plugin's story in the thread (where the user merged it, if they did); made in the first workstream when it has
/// none or the user deleted it — and that workstream too when the user deleted them all.
fn plugin_story(l: &Ledger, evs: &[Value], t: &Tree, plugin: &str, name: &str) -> Result<String> {
    if let Some(id) = plugin_home(&t.nodes, plugin) {
        return Ok(id);
    }
    let now = Local::now().to_rfc3339();
    let count = |k: &str| evs.iter().filter(|x| x["type"] == "create" && x["kind"] == k).count() + 1;
    let mut recs = vec![];
    let epic = match t.nodes.values().filter(|x| x.kind == "epic" && !["merged", "deleted"].contains(&x.state.as_str())).map(|x| x.id.clone()).min() {
        Some(e) => e,
        None => {
            let e = new_id("epic", count("epic"));
            recs.push(json!({"type": "create", "id": e, "kind": "epic", "by": "plugin", "plugin": plugin, "plugin_name": name, "evidence": null, "date": &now[..10], "at": now,
                "mid": mid_of(&["plugin-epic", plugin, &now]), "fields": {"title": FIRST_EPIC, "plan": "Off-plan"}}));
            e
        }
    };
    let id = new_id("story", count("story"));
    recs.push(json!({"type": "create", "id": id, "kind": "story", "by": "plugin", "plugin": plugin, "plugin_name": name, "evidence": null, "date": &now[..10], "at": now,
        "mid": mid_of(&["plugin-story", plugin, &now]), "fields": {"title": name, "parent": epic}}));
    l.append(&recs)?;
    Ok(id)
}

/// Before the user's Yes or No to a plugin's proposal: only one still waiting (not replaced by a newer one, not decided);
/// a Yes to a new task makes the plugin's story it goes in.
pub(super) fn story_for_accept(project: &str, target: &str, yes: bool) -> Result<()> {
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    let evs = l.events()?;
    let Some(e) = evs.iter().find(|e| e["mid"] == target && e["plugin"].is_string() && e["propose"] == true) else { return Ok(()) };
    let t = fold_events(&evs, l.evidence())?;
    if !t.proposals.values().any(|p| p["mid"] == target) {
        bail!("that suggestion was replaced by a newer one or already decided");
    }
    if yes && e["type"] == "create" {
        plugin_story(&l, &evs, &t, e["plugin"].as_str().unwrap_or(""), e["plugin_name"].as_str().unwrap_or(""))?;
    }
    Ok(())
}

/// A plugin's own write isn't the user's to undo: they delete or change the task instead.
pub(super) fn not_a_plugins(project: &str, target: &str) -> Result<()> {
    if Ledger::of(project)?.events()?.iter().any(|e| e["mid"] == target && e["by"] == "plugin") {
        bail!("a plugin's change can't be undone: change or delete the task instead");
    }
    Ok(())
}

/// A plugin adds or updates its task `t.key` in a thread (`ctx.tasks.upsert`): again with the same values writes
/// nothing. `{task, applied, proposed, ignored, why?}` — `ignored`: the fields the user set, kept as they are.
pub fn plugin_upsert(project: &str, plugin: &str, name: &str, t: &Value) -> Result<Value> {
    let key = key_of(t["key"].as_str().unwrap_or(""))?;
    let set = wanted(name, t, true)?;
    write(project, plugin, name, &key, set, reason_of(t["reason"].as_str()), t["propose"] == true)
}

/// A plugin closes its task `key` (`ctx.tasks.close`): done, with its reason; a proposal of it not yet taken is
/// withdrawn.
pub fn plugin_close(project: &str, plugin: &str, name: &str, key: &str, reason: Option<&str>, propose: bool) -> Result<Value> {
    let key = key_of(key)?;
    let mut set = serde_json::Map::new();
    set.insert("state".into(), json!("done"));
    write(project, plugin, name, &key, set, reason_of(reason), propose)
}

fn write(project: &str, plugin: &str, name: &str, key: &str, set: serde_json::Map<String, Value>, reason: Option<String>, propose: bool) -> Result<Value> {
    let l = Ledger::of(project)?;
    let _lock = l.lock()?;
    ensure_first_epic(&l)?;
    let evs = l.events()?;
    let t = fold_events(&evs, l.evidence())?;
    let k = (plugin.to_string(), key.to_string());
    let now = Local::now().to_rfc3339();
    let mut e = json!({"type": "plugin", "op": "set", "by": "plugin", "plugin": plugin, "plugin_name": name, "key": key, "evidence": null, "date": &now[..10], "at": now, "reason": reason});
    if propose {
        e["propose"] = json!(true);
    }
    let pending = t.proposals.get(&k);
    let quiet = |set: &Value| -> Option<Value> {
        if propose && pending.is_some_and(|p| &p["set"] == set) {
            return Some(json!({"task": pending.and_then(|p| p["node"].as_str().filter(|_| p["op"] == "set")), "applied": false, "proposed": true, "ignored": []}));
        }
        (propose && t.refused.get(&k) == Some(set)).then(|| json!({"task": null, "applied": false, "proposed": false, "ignored": [], "why": "the user said no to this"}))
    };
    let Some(x) = t.nodes.values().find(|x| x.kind == "task" && x.plugin.as_ref().is_some_and(|p| p["id"] == plugin && p["key"] == key)) else {
        // A new task; closing one never made withdraws its proposal.
        if set.get("state").and_then(Value::as_str).is_some_and(closed) {
            if pending.is_some() {
                e["op"] = json!("withdraw");
                e.as_object_mut().unwrap().remove("propose");
                e["mid"] = json!(mid_of(&["plugin", plugin, key, &now, "withdraw"]));
                l.append(&[e])?;
            }
            return Ok(json!({"task": null, "applied": false, "proposed": false, "ignored": [], "why": "no open task of that key"}));
        }
        let set = Value::Object(set);
        if let Some(r) = quiet(&set) {
            return Ok(r);
        }
        let id = new_id("task", evs.iter().filter(|x| x["type"] == "create" && x["kind"] == "task").count() + 1);
        if !propose {
            plugin_story(&l, &evs, &t, plugin, name)?;
        }
        e["type"] = json!("create");
        e["id"] = json!(id);
        e["kind"] = json!("task");
        e.as_object_mut().unwrap().remove("op");
        e["fields"] = json!({"title": set["title"], "parent": null, "type": "action", "internal_owner": set.get("internal_owner"), "due": set.get("due"), "links": set.get("links")});
        e["set"] = set;
        e["mid"] = json!(mid_of(&["plugin", plugin, key, &now, &e["set"].to_string()]));
        l.append(&[e])?;
        return Ok(json!({"task": if propose { Value::Null } else { json!(id) }, "applied": !propose, "proposed": propose, "ignored": []}));
    };
    let gone = ["merged", "deleted"].contains(&x.state.as_str());
    let (mut change, mut ignored) = (serde_json::Map::new(), vec![]);
    for (f, v) in set {
        if current(x, &f) == v {
            continue;
        }
        if gone || x.pinned.contains(&f) {
            ignored.push(f);
        } else {
            change.insert(f, v);
        }
    }
    if change.is_empty() {
        let mut r = json!({"task": x.id, "applied": false, "proposed": false, "ignored": ignored});
        if gone {
            r["why"] = json!("the user merged or deleted it");
        }
        return Ok(r);
    }
    let change = Value::Object(change);
    if let Some(mut r) = quiet(&change) {
        r["task"] = json!(x.id);
        r["ignored"] = json!(ignored);
        return Ok(r);
    }
    e["node"] = json!(x.id);
    e["set"] = change;
    e["mid"] = json!(mid_of(&["plugin", plugin, key, &now, &e["set"].to_string()]));
    l.append(&[e])?;
    Ok(json!({"task": x.id, "applied": !propose, "proposed": propose, "ignored": ignored}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(name: &str) -> (crate::paths::testenv::Env, String) {
        let (env, _root) = crate::paths::testenv::folders(name);
        let id = crate::projects::create("Aurora", None, None).unwrap().id;
        (env, id)
    }

    fn nodes(id: &str) -> BTreeMap<String, Node> {
        fold(&Ledger::of(id).unwrap()).unwrap().nodes
    }

    fn events(id: &str) -> usize {
        Ledger::of(id).unwrap().events().unwrap().len()
    }

    #[test]
    fn a_plugins_task_lands_in_its_own_story_and_the_same_write_again_writes_nothing() {
        let (_env, id) = thread("plugin-task-story");
        let r = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-21", "url": "https://tracker.acme.example/p/7", "reason": "next step"})).unwrap();
        assert_eq!((r["applied"].as_bool(), r["proposed"].as_bool()), (Some(true), Some(false)), "{r}");
        let t = r["task"].as_str().unwrap().to_string();
        let n = nodes(&id);
        let task = &n[&t];
        let story = &n[task.parent.as_deref().unwrap()];
        assert_eq!((story.kind.as_str(), story.title.as_str()), ("story", "Acme Tracker"));
        assert_eq!(n[story.parent.as_deref().unwrap()].kind, "epic", "in the first workstream");
        assert_eq!((task.title.as_str(), task.due.as_deref(), task.state.as_str()), ("Send the quote", Some("2026-11-21"), "open"));
        assert_eq!(task.links, vec![json!({"title": "Acme Tracker", "url": "https://tracker.acme.example/p/7"})]);
        assert_eq!(task.plugin, Some(json!({"id": "tracker", "name": "Acme Tracker", "key": "P-7"})));
        assert_eq!(task.history[0]["reason"], "next step", "provenance in its history");
        // Again, the same: nothing written. A second task: the same story.
        let before = events(&id);
        let again = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-21", "url": "https://tracker.acme.example/p/7"})).unwrap();
        assert_eq!((again["task"].as_str(), again["applied"].as_bool()), (Some(t.as_str()), Some(false)));
        assert_eq!(events(&id), before);
        let other = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-8", "title": "Book the demo"})).unwrap();
        assert_eq!(nodes(&id)[other["task"].as_str().unwrap()].parent, task.parent);
        // A change: applied, in its history with the plugin's reason.
        plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-28", "reason": "moved a week"})).unwrap();
        let x = &nodes(&id)[&t];
        assert_eq!(x.due.as_deref(), Some("2026-11-28"));
        let h = x.history.last().unwrap();
        assert_eq!((h["plugin_name"].as_str(), h["reason"].as_str(), h["applied"].as_bool()), (Some("Acme Tracker"), Some("moved a week"), Some(true)));
        // Closed: done, with why.
        let c = plugin_close(&id, "tracker", "Acme Tracker", "P-7", Some("pursuit won"), false).unwrap();
        assert_eq!(c["applied"], true);
        assert_eq!(nodes(&id)[&t].state, "done");
    }

    #[test]
    fn the_users_edit_wins_a_field_they_set_and_a_task_they_closed_stay_theirs() {
        let (_env, id) = thread("plugin-task-pins");
        let t = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-21"})).unwrap()["task"].as_str().unwrap().to_string();
        human_event(&id, &json!({"op": "edit", "node": t, "set": {"due": "2026-12-01", "title": "Send the signed quote"}})).unwrap();
        let r = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-25", "owner": "Dana Lee"})).unwrap();
        assert_eq!(r["ignored"], json!(["due", "title"]), "{r}");
        let x = &nodes(&id)[&t];
        assert_eq!((x.title.as_str(), x.due.as_deref(), x.internal_owner.as_deref()), ("Send the signed quote", Some("2026-12-01"), Some("Dana Lee")));
        // Again: what the user set is still ignored, and nothing is written.
        let before = events(&id);
        plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-25", "owner": "Dana Lee"})).unwrap();
        assert_eq!(events(&id), before);
        // Closed by the user: the plugin never reopens it.
        human_event(&id, &json!({"op": "edit", "node": t, "set": {"state": "cancelled"}})).unwrap();
        let r = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "state": "in_progress"})).unwrap();
        assert_eq!(r["ignored"], json!(["state", "title"]));
        assert_eq!(nodes(&id)[&t].state, "cancelled");
        // Deleted by the user: nothing written to it, no new task of that key.
        human_event(&id, &json!({"op": "delete", "node": t})).unwrap();
        let r = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send it now"})).unwrap();
        assert_eq!((r["task"].as_str(), r["applied"].as_bool()), (Some(t.as_str()), Some(false)));
        assert_eq!(nodes(&id).values().filter(|x| x.kind == "task").count(), 1);
    }

    #[test]
    fn a_plugin_writes_only_its_own_keys_tasks() {
        let (_env, id) = thread("plugin-task-own");
        let mine = plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote"})).unwrap()["task"].as_str().unwrap().to_string();
        let theirs = plugin_upsert(&id, "board", "Globex Board", &json!({"key": "P-7", "title": "Review the board"})).unwrap()["task"].as_str().unwrap().to_string();
        assert_ne!(mine, theirs, "the same key of another plugin is another task");
        plugin_close(&id, "board", "Globex Board", "P-7", None, false).unwrap();
        let n = nodes(&id);
        assert_eq!((n[&mine].state.as_str(), n[&theirs].state.as_str()), ("open", "done"));
        assert_ne!(n[&mine].parent, n[&theirs].parent, "each plugin its own story");
        // A forged event naming another plugin's task changes nothing.
        let l = Ledger::of(&id).unwrap();
        l.append(&[json!({"type": "plugin", "op": "set", "by": "plugin", "plugin": "board", "plugin_name": "Globex Board", "key": "P-7", "node": mine, "set": {"state": "done"}, "mid": "m-forged"})]).unwrap();
        assert_eq!(nodes(&id)[&mine].state, "open");
        // Bad input: refused, named.
        for bad in [json!({"key": "", "title": "x"}), json!({"key": "k", "title": ""}), json!({"key": "k", "title": "x", "due": "21.11.2026"}), json!({"key": "k", "title": "x", "state": "deleted"}), json!({"key": "k", "title": "x", "url": "http://tracker.acme.example"})] {
            assert!(plugin_upsert(&id, "tracker", "Acme Tracker", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_proposal_counts_only_after_the_users_yes_and_after_no_the_same_stays_quiet() {
        let (_env, id) = thread("plugin-task-propose");
        let task = json!({"key": "P-7", "title": "Send the quote", "due": "2026-11-21", "propose": true, "reason": "next step"});
        let r = plugin_upsert(&id, "tracker", "Acme Tracker", &task).unwrap();
        assert_eq!((r["task"].is_null(), r["proposed"].as_bool(), r["applied"].as_bool()), (true, Some(true), Some(false)), "{r}");
        assert!(nodes(&id).values().all(|x| x.kind != "task" && x.kind != "story"), "nothing in the work before Yes");
        let tr = tree(&id).unwrap();
        let p = &tr["proposals"][0];
        assert_eq!((p["op"].as_str(), p["plugin_name"].as_str(), p["set"]["title"].as_str(), p["reason"].as_str()), (Some("create"), Some("Acme Tracker"), Some("Send the quote"), Some("next step")));
        // The same again: no second proposal.
        let before = events(&id);
        plugin_upsert(&id, "tracker", "Acme Tracker", &task).unwrap();
        assert_eq!(events(&id), before);
        // No: gone; the same again stays quiet; a change proposes again.
        human_event(&id, &json!({"op": "reject", "target": p["mid"]})).unwrap();
        assert_eq!(tree(&id).unwrap()["proposals"], json!([]));
        let r = plugin_upsert(&id, "tracker", "Acme Tracker", &task).unwrap();
        assert_eq!((r["proposed"].as_bool(), r["why"].as_str()), (Some(false), Some("the user said no to this")));
        assert_eq!(events(&id), before + 1, "only the No");
        let mut later = task.clone();
        later["due"] = json!("2026-11-28");
        plugin_upsert(&id, "tracker", "Acme Tracker", &later).unwrap();
        let p = tree(&id).unwrap()["proposals"][0].clone();
        assert_eq!(p["set"]["due"], "2026-11-28");
        // Yes: the task, in the plugin's story.
        human_event(&id, &json!({"op": "accept", "target": p["mid"]})).unwrap();
        let n = nodes(&id);
        let x = n.values().find(|x| x.kind == "task").unwrap().clone();
        assert_eq!((x.title.as_str(), x.due.as_deref()), ("Send the quote", Some("2026-11-28")));
        assert_eq!(n[x.parent.as_deref().unwrap()].title, "Acme Tracker");
        // A proposed close waits too; a proposal never taken is withdrawn by closing it.
        plugin_close(&id, "tracker", "Acme Tracker", "P-7", Some("pursuit won"), true).unwrap();
        assert_eq!(nodes(&id)[&x.id].state, "open");
        let p = tree(&id).unwrap()["proposals"][0].clone();
        assert_eq!((p["op"].as_str(), p["node"].as_str(), p["title"].as_str(), p["set"]["state"].as_str()), (Some("set"), Some(x.id.as_str()), Some("Send the quote"), Some("done")));
        human_event(&id, &json!({"op": "accept", "target": p["mid"]})).unwrap();
        assert_eq!(nodes(&id)[&x.id].state, "done");
        plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-9", "title": "Book the demo", "propose": true})).unwrap();
        assert_eq!(tree(&id).unwrap()["proposals"].as_array().unwrap().len(), 1);
        let r = plugin_close(&id, "tracker", "Acme Tracker", "P-9", None, true).unwrap();
        assert_eq!(r["why"], "no open task of that key");
        assert_eq!(tree(&id).unwrap()["proposals"], json!([]), "withdrawn");
    }

    #[test]
    fn the_users_merge_or_delete_of_the_plugins_story_holds_and_its_tasks_go_with_a_merge() {
        let (_env, id) = thread("plugin-task-story-moves");
        let up = |key: &str, title: &str| plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": key, "title": title})).unwrap()["task"].as_str().unwrap().to_string();
        let a = up("P-1", "Send the quote");
        let story = nodes(&id)[&a].parent.clone().unwrap();
        let mine = human_event(&id, &json!({"op": "create", "kind": "story", "parent": "E01", "title": "Sales"})).unwrap()["id"].as_str().unwrap().to_string();
        human_event(&id, &json!({"op": "merge", "node": story, "into": mine})).unwrap();
        let b = up("P-2", "Book the demo");
        let n = nodes(&id);
        assert_eq!((n[&a].parent.as_deref(), n[&b].parent.as_deref()), (Some(mine.as_str()), Some(mine.as_str())), "merged: the plugin's tasks, old and new, in the user's story");
        assert_eq!(n.values().filter(|x| x.kind == "story" && x.plugin.is_some() && x.state != "merged").count(), 0, "no new story of its own");
        // The story it was merged into deleted: a new task gets a story of its own again.
        human_event(&id, &json!({"op": "delete", "node": mine})).unwrap();
        let c = up("P-3", "Sign the order");
        let n = nodes(&id);
        let home = n[&c].parent.clone().unwrap();
        assert_eq!((n[&home].title.as_str(), n[&home].parent.as_deref()), ("Acme Tracker", Some("E01")));
        // Every workstream deleted: a new one, with a new story in it.
        human_event(&id, &json!({"op": "delete", "node": "E01"})).unwrap();
        let d = up("P-4", "Send the invoice");
        let n = nodes(&id);
        let story = &n[n[&d].parent.as_deref().unwrap()];
        let epic = &n[story.parent.as_deref().unwrap()];
        assert_eq!((story.title.as_str(), epic.kind.as_str(), epic.state.as_str()), ("Acme Tracker", "epic", "open"));
        assert_ne!(story.id, home);
    }

    #[test]
    fn only_the_waiting_proposal_is_decided_and_a_plugins_own_write_isnt_undone() {
        let (_env, id) = thread("plugin-task-proposal-edges");
        let ask = |due: &str, propose: bool| plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-7", "title": "Send the quote", "due": due, "propose": propose})).unwrap();
        ask("2026-11-21", true);
        let old = tree(&id).unwrap()["proposals"][0]["mid"].clone();
        ask("2026-11-28", true);
        assert!(human_event(&id, &json!({"op": "accept", "target": old})).unwrap_err().to_string().contains("replaced by a newer one"));
        assert!(human_event(&id, &json!({"op": "reject", "target": old})).is_err());
        // A direct write with the waiting proposal's values: applied, the proposal gone, one task.
        let r = ask("2026-11-28", false);
        assert_eq!((r["applied"].as_bool(), r["proposed"].as_bool()), (Some(true), Some(false)), "{r}");
        assert_eq!(tree(&id).unwrap()["proposals"], json!([]));
        assert_eq!(nodes(&id).values().filter(|x| x.kind == "task").count(), 1);
        let create = Ledger::of(&id).unwrap().events().unwrap().into_iter().rfind(|e| e["type"] == "create" && e["kind"] == "task").unwrap();
        assert!(human_event(&id, &json!({"op": "undo", "target": create["mid"]})).unwrap_err().to_string().contains("can't be undone"));
        // Its story deleted while a proposal waits: the Yes puts the task in a new story, never the deleted one.
        let story = nodes(&id).values().find(|x| x.kind == "task").unwrap().parent.clone().unwrap();
        plugin_upsert(&id, "tracker", "Acme Tracker", &json!({"key": "P-8", "title": "Book the demo", "propose": true})).unwrap();
        human_event(&id, &json!({"op": "delete", "node": story})).unwrap();
        let mid = tree(&id).unwrap()["proposals"][0]["mid"].clone();
        human_event(&id, &json!({"op": "accept", "target": mid})).unwrap();
        let n = nodes(&id);
        let x = n.values().find(|x| x.title == "Book the demo").unwrap();
        let home = &n[x.parent.as_deref().unwrap()];
        assert_ne!(home.id, story);
        assert_eq!((home.title.as_str(), home.state.as_str()), ("Acme Tracker", "open"));
    }
}

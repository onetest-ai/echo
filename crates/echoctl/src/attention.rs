//! What waits for the user's decision, in one list (the window's Needs you: the activity bar's
//! entry above Settings, opening the right panel). Collected from where each thing lives — the
//! places themselves keep showing it (a tab's count, a bar) — deterministic, no model: a profile
//! to sign in to, minutes that failed, senders waiting to be read, people suggestions, Found for
//! you, the Work tab's To review, topics or numbers to accept, missing permissions, an update.
//! An item is open while its cause stands; when it goes (decided here or in its place) it moves to
//! Done for a week (`<home>/.echo/attention.json`). Nothing to mark read.

use crate::paths;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A button on an item: the IPC message the window sends as it is (the same one its place sends).
fn act(label: &str, primary: bool, ipc: Value) -> Value {
    json!({"label": label, "primary": primary, "ipc": ipc})
}

/// An open item. `group` = a client id (`-`: none, `echo`: Echo itself), `go` = the window's hash
/// where it is decided in full.
#[allow(clippy::too_many_arguments)]
fn item(key: String, kind: &str, group: &str, title: String, why: String, urgent: bool, actions: Vec<Value>, go: Option<String>) -> Value {
    json!({"key": key, "kind": kind, "group": group, "title": title, "why": why, "urgent": urgent, "actions": actions, "go": go})
}

fn read(p: &std::path::Path) -> Value {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)
}

fn call_title(dir: &std::path::Path) -> String {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    read(&dir.join("meta.json"))["title"].as_str().filter(|t| !t.is_empty()).map(str::to_owned).unwrap_or_else(|| name.get(18..).map(|t| t.replace('-', " ")).unwrap_or_else(|| "Call".into()))
}

/// Calls of the last `days` days, newest first.
fn recent_calls(days: i64) -> Vec<std::path::PathBuf> {
    let since = (chrono::Local::now() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut v: Vec<_> = crate::calls::all_calls().into_iter().filter(|d| d.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.get(..10).is_some_and(|x| x >= since.as_str()))).collect();
    v.reverse();
    v
}

/// What a collection found, and what it couldn't judge: an item missing from `items` moves to Done
/// only when its source was read — one whose source failed to read now (or whose state isn't known
/// yet, a sign-in being checked) is `kept` open as it was; one whose source is switched off, or that
/// aged out, goes `quietly` (not decided, so not Done).
#[derive(Default)]
pub struct Collected {
    pub items: Vec<Value>,
    keep: BTreeSet<String>,
    keep_kinds: BTreeSet<&'static str>,
    quiet: BTreeSet<String>,
    quiet_kinds: BTreeSet<&'static str>,
}

/// Failed minutes are news for 14 days; after that they leave the list without counting as done.
const MINUTES_DAYS: i64 = 14;

/// Every open item now.
pub fn collect() -> Collected {
    let mut c = Collected::default();
    let mut out = vec![];
    let threads = crate::projects::list();
    let client_of = |thread: &str| threads.iter().find(|p| p.id == thread).and_then(|p| p.client.clone()).unwrap_or_else(|| "-".into());

    // Minutes: failed ones, ones still waiting their turn, and the profiles whose sign-in they wait for.
    let mut login_wait: BTreeMap<String, usize> = BTreeMap::new();
    let fresh = (chrono::Local::now() - chrono::Duration::days(MINUTES_DAYS)).format("%Y-%m-%d").to_string();
    for d in recent_calls(MINUTES_DAYS * 2) {
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if name.get(..10).is_some_and(|x| x < fresh.as_str()) {
            c.quiet.insert(format!("minutes:{}", crate::calls::stamp_of(&d)));
            c.quiet.insert(format!("minutes-wait:{}", crate::calls::stamp_of(&d)));
            continue;
        }
        let stamp = crate::calls::stamp_of(&d);
        // The state as the user sees it (`notes::status`): a run whose owner died is failed there.
        let Some(run) = crate::notes::status(&d) else {
            c.quiet.insert(format!("minutes-wait:{stamp}"));
            continue;
        };
        // Waiting for something it names (not the moment every run spends starting): what for, and
        // Write now where the run may start anyway. Once it starts it leaves quietly (not "Done").
        if run.state == "queued" && run.wait.is_some() {
            let acts = if run.may_start { vec![act("Write now", true, json!({"cmd": "notes_now", "call": name}))] } else { vec![] };
            out.push(item(
                format!("minutes-wait:{stamp}"),
                "minutes",
                "echo",
                format!("Minutes not written yet: {}", call_title(&d)),
                run.message.clone().unwrap_or_else(|| "They start as soon as the model is free".into()).chars().take(200).collect(),
                false,
                acts,
                Some(format!("call/{name}/minutes")),
            ));
        } else {
            c.quiet.insert(format!("minutes-wait:{stamp}"));
        }
        match run.state.as_str() {
            "needs_login" => *login_wait.entry(run.profile.clone().unwrap_or_else(crate::ai::default_id)).or_default() += 1,
            "failed" if !d.join("notes.md").exists() => out.push(item(
                format!("minutes:{stamp}"),
                "minutes",
                "echo",
                format!("Minutes failed: {}", call_title(&d)),
                run.message.as_deref().unwrap_or("Claude Code didn't finish them").chars().take(160).collect(),
                false,
                vec![act("Write again", true, json!({"cmd": "write_notes", "call": name})), act("Show log", false, json!({"cmd": "show_log", "call": name}))],
                Some(format!("call/{name}/minutes")),
            )),
            _ => {}
        }
    }
    let settings = crate::ai::Settings::load();
    if settings.is_err() {
        c.keep_kinds.insert("signin");
    }
    if let Ok(s) = settings {
        let default = s.default_id();
        for p in s.all() {
            let st = crate::signin::state(&p).map(|x| serde_json::to_value(x).unwrap_or(Value::Null)).unwrap_or(Value::Null);
            // Not known yet (a check due or running, a sign-in on): whatever was shown stays.
            if p.kind.signs_in() && !matches!(st["state"].as_str(), Some("signed_in" | "signed_out" | "other_account" | "failed")) {
                c.keep.insert(format!("signin:{}", p.id));
                continue;
            }
            let out_now = matches!(st["state"].as_str(), Some("signed_out" | "other_account" | "failed"));
            let waiting = login_wait.get(&p.id).copied().unwrap_or(0);
            if out_now && (waiting > 0 || p.id == default) {
                let why = match (waiting, st["state"].as_str()) {
                    (0, Some("other_account")) => "Signed in to another account than this profile's".to_string(),
                    (0, _) => "The default profile: minutes and live notes need it".to_string(),
                    (n, _) => format!("Minutes of {n} call{} wait for it", if n == 1 { "" } else { "s" }),
                };
                let mut acts = vec![act("Sign in…", true, json!({"cmd": "sign_in", "profile": p.id}))];
                if let Some(a) = st["account"].as_str().filter(|_| st["state"] == "other_account") {
                    acts.push(act(&format!("Use {a}"), false, json!({"cmd": "accept_account", "profile": p.id, "account": a})));
                }
                out.push(item(format!("signin:{}", p.id), "signin", "echo", format!("Sign in to {}", p.name), why, true, acts, Some("settings/ai".into())));
            }
        }
    }

    // People suggestions (the People tab's bar) and Found for you.
    for p in threads.iter().filter(|p| !p.stage.closed()) {
        let pending: Vec<Value> = crate::people::suggestions(&p.id).into_iter().filter(|x| x["state"] == "pending").collect();
        for x in &pending {
            let who = x["target"]["name"].as_str().unwrap_or("Someone");
            let v = x["value_name"].as_str().or(x["value"].as_str()).unwrap_or("");
            let q = match x["field"].as_str().unwrap_or("") {
                "stance" => format!("Is {who} a {v}?"),
                "role" => format!("{who}'s role here: {v}?{}", x["was"].as_str().map(|w| format!(" (was “{w}”)")).unwrap_or_default()),
                "person" => format!("Add {who}{}? Mentioned{}, {} time{}", x["target"]["org_name"].as_str().map(|o| format!(" ({o})")).unwrap_or_default(),
                    x["at"]["title"].as_str().map(|t| format!(" in {t}")).unwrap_or_default(), x["mentions"].as_u64().unwrap_or(1), if x["mentions"].as_u64().unwrap_or(1) == 1 { "" } else { "s" }),
                "left" => format!("Has {who} left?"),
                "reports_to" => format!("{who} reports to {v}?"),
                "title" => format!("{who} is {v}?"),
                "influence" | "interest" => format!("{who}'s {}: {}?", x["field"].as_str().unwrap_or(""), match v { "1" => "low", "2" => "mid", "3" => "high", o => o }),
                "relation" => format!("{who} {} {}?", v.replace('_', " "), x["target"]["to_name"].as_str().unwrap_or("")),
                "group" => format!("{who} is in {v}?"),
                f => format!("{who}: {f} = {v}?"),
            };
            let sid = x["sid"].as_str().unwrap_or("");
            let quote = x["quote"].as_str().map(|q| format!("“{}”", q.chars().take(120).collect::<String>())).unwrap_or_default();
            out.push(item(
                format!("person:{}:{sid}", p.id),
                "people",
                &client_of(&p.id),
                q,
                format!("{} · {quote}", p.name),
                false,
                {
                    let mut acts = vec![act("Yes", true, json!({"cmd": "project", "op": "people_decide", "id": p.id, "sid": sid, "yes": true})), act("No", false, json!({"cmd": "project", "op": "people_decide", "id": p.id, "sid": sid, "yes": false}))];
                    // Not quite right: the People tab, with this suggestion open for the user's own version.
                    if x["field"] != "left" {
                        acts.push(json!({"label": "Change…", "primary": false, "go": format!("project/{}/people", p.id), "edit": sid}));
                    }
                    acts
                },
                Some(format!("project/{}/people", p.id)),
            ));
        }
        // The Work tab's To review: pending moves / merges, and weak "done" claims.
        let tree = crate::ledger::tree(&p.id);
        if tree.is_err() {
            c.keep.insert(format!("review:{}", p.id));
        }
        if let Ok(t) = tree {
            let closed = ["done", "cancelled", "merged", "deleted"];
            let (mut moves, mut weak) = (0, 0);
            for n in t["nodes"].as_object().into_iter().flat_map(|o| o.values()) {
                moves += n["suggestions"].as_array().into_iter().flatten().filter(|s| s["status"] == "pending").count();
                let last = n["history"].as_array().into_iter().flatten().rfind(|h| h["type"] == "agent" && !h["effect"].is_null());
                if last.is_some_and(|h| matches!(h["effect"].as_str(), Some("done" | "held")) && h["verdict"] == "weak") && !closed.contains(&n["state"].as_str().unwrap_or("")) {
                    weak += 1;
                }
            }
            if moves + weak > 0 {
                let why = [(moves, "suggested move", "suggested moves"), (weak, "weak “done”", "weak “done”s")].iter().filter(|(n, _, _)| *n > 0).map(|(n, one, many)| format!("{n} {}", if *n == 1 { one } else { many })).collect::<Vec<_>>().join(" · ");
                out.push(item(format!("review:{}", p.id), "work", &client_of(&p.id), format!("{} to review in {}'s work", moves + weak, p.name), why, false, vec![], Some(format!("project/{}/work", p.id))));
            }
        }
    }
    let found = crate::projects::found();
    if found.is_err() {
        c.keep_kinds.insert("found");
    }
    if let Ok(found) = found {
        for f in &found {
            let key = f["key"].as_str().unwrap_or("");
            let client = f["client"].as_str().map(str::to_owned).unwrap_or_else(|| if f["new_client"].is_null() { String::new() } else { "new".into() });
            let n = f["calls"].as_array().map_or(0, |a| a.len());
            out.push(item(
                format!("found:{key}"),
                "found",
                f["client"].as_str().unwrap_or("-"),
                format!("New thread? “{}”", f["name"].as_str().unwrap_or("")),
                format!("Found for you · {n} calls · {}", f["why"].as_str().unwrap_or("")),
                false,
                vec![act("Create", true, json!({"cmd": "project", "op": "found_accept", "key": key, "name": f["name"], "client": client})), act("Not this", false, json!({"cmd": "project", "op": "found_reject", "key": key}))],
                Some("found".into()),
            ));
        }
    }

    // The Brain off (or not built): its items leave quietly.
    #[cfg(feature = "brain")]
    let brain_on = crate::brain::enabled();
    #[cfg(not(feature = "brain"))]
    let brain_on = false;
    // A thread's mail and meeting chats are read into its work whether or not the Brain is on, by the
    // same senders list: whom they wait for is asked here either way.
    #[cfg(feature = "brain")]
    let senders_asked = brain_on || crate::projects::list().iter().any(|p| !p.stage.closed() && p.sources.iter().any(|s| s.kind == "mail" || s.kind == "teams_chat"));
    #[cfg(not(feature = "brain"))]
    let senders_asked = false;
    if !brain_on {
        c.quiet_kinds.insert("brain");
    }
    if !senders_asked {
        c.quiet_kinds.insert("sender");
    }
    #[cfg(feature = "brain")]
    if senders_asked {
        // Senders whose mail or posts wait to be read (with the Brain off: decided right here).
        let go = brain_on.then(|| "brain/sources".to_string());
        for (client, l) in crate::brain::senders::load().clients {
            for w in &l.unverified {
                out.push(item(
                    format!("unverified:{client}:{}", w.key),
                    "sender",
                    &client,
                    format!("{}'s mail isn't authenticated", if w.name.is_empty() { &w.key } else { &w.name }),
                    format!("{} · {} message{} with no DMARC verdict (a mailing list, or a domain without it) — could be forged · “{}”", w.key, w.count, if w.count == 1 { "" } else { "s" }, w.last_about),
                    false,
                    vec![act("Read anyway", false, json!({"cmd": "brain_sender", "client": client, "key": w.key, "decision": "read_unverified"})), act("Keep out", false, json!({"cmd": "brain_sender", "client": client, "key": w.key, "decision": "keep_unverified_out"}))],
                    go.clone(),
                ));
            }
            for w in &l.waiting {
                let addr = if w.key.starts_with("teams:") { "Teams".to_string() } else { w.key.clone() };
                let auth = if w.kind == "mail" && !w.verified { " · not authenticated" } else { "" };
                out.push(item(
                    format!("sender:{client}:{}", w.key),
                    "sender",
                    &client,
                    format!("{} isn't read yet", if w.name.is_empty() { &w.key } else { &w.name }),
                    format!("{addr} · {} message{}{auth} · “{}”", w.count, if w.count == 1 { "" } else { "s" }, w.last_about),
                    false,
                    vec![act("Trust", w.kind != "mail" || w.verified, json!({"cmd": "brain_sender", "client": client, "key": w.key, "decision": "trust"})), act("Ignore", false, json!({"cmd": "brain_sender", "client": client, "key": w.key, "decision": "ignore"}))],
                    go.clone(),
                ));
            }
        }
    }
    #[cfg(feature = "brain")]
    if brain_on {
        if crate::brain::taxonomy::current().is_some_and(|t| t.provisional()) {
            out.push(item("taxonomy".into(), "brain", "echo", "Topics waiting for your review".into(), "Claude built topics nobody accepted yet: look through them in Brain → Topics, fix what's wrong, then Accept".into(), false, vec![], Some("brain/topics".into())));
        }
        // A topic in the wrong place (`taxonomy::moves`, no model): up to the practice, or down to a client.
        for m in crate::brain::taxonomy::moves().unwrap_or_default() {
            let key = m["key"].as_str().unwrap_or("").to_string();
            let (title, label) = if m["dir"] == "up" {
                (format!("“{}” belongs to your practice?", m["label"].as_str().unwrap_or("")), "Move it up")
            } else {
                (format!("“{}” is only {}'s?", m["label"].as_str().unwrap_or(""), m["to_name"].as_str().unwrap_or("")), "Move it down")
            };
            out.push(item(format!("move:{key}"), "brain", "echo", title, m["why"].as_str().unwrap_or("").to_string(), false,
                vec![act(label, true, json!({"cmd": "brain_topic_move", "key": key, "do": "apply"})), act("Keep it", false, json!({"cmd": "brain_topic_move", "key": key, "do": "dismiss"}))],
                Some("brain/topics".into())));
        }
        // A layer whose topics no longer fit its calls (`taxonomy::drift`, no model): rebuild it, or not now.
        for d in crate::brain::taxonomy::drift().unwrap_or_default().into_iter().filter(|d| d["stale"] == true) {
            let layer = d["layer"].as_str().unwrap_or("").to_string();
            let mut why: String = d["reasons"].as_array().into_iter().flatten().filter_map(|r| r["text"].as_str()).map(|t| {
                let mut c = t.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect::<String>() + ". ").unwrap_or_default()
            }).collect();
            why.push_str("Rebuild makes a new version from its calls for you to review; its topics keep their place.");
            out.push(item(format!("drift:{layer}"), "brain", if layer == crate::brain::taxonomy::SHARED { "echo" } else { layer.as_str() },
                format!("Topics for {} need a rebuild", d["name"].as_str().unwrap_or(&layer)), why, false,
                vec![act("Rebuild topics", true, json!({"cmd": "brain_rebuild_topics", "layer": layer})), act("Not now", false, json!({"cmd": "brain_topics_snooze", "layer": layer}))],
                Some("brain/topics".into())));
        }
        if let Ok(v) = crate::brain::numbers::overview() {
            for c in v["clients"].as_array().into_iter().flatten().filter(|c| c["status"] == "proposed") {
                let cl = c["client"].as_str().unwrap_or("-");
                out.push(item(format!("numbers:{cl}"), "brain", cl, "Reporting families proposed".into(), "Claude proposed how to read the workbooks — review, then accept and build".into(), false, vec![], Some("brain/numbers".into())));
            }
        }
    }

    // Meetings of your threads nobody recorded that Teams transcribed (`teams_transcript::scan`): offered once.
    for (id, v) in crate::teams_transcript::to_offer() {
        let t = &v["threads"][0];
        let when = v["start"].as_str().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()).map(|s| s.with_timezone(&chrono::Local).format("%a %-d %b, %H:%M").to_string()).unwrap_or_default();
        out.push(item(
            format!("transcript:{id}"),
            "transcript",
            t["client"].as_str().unwrap_or("-"),
            format!("Teams transcript: {}", v["subject"].as_str().unwrap_or("a meeting")),
            format!("{when} · {} — not recorded; import it as a call for its minutes and work", t["name"].as_str().unwrap_or("your thread")),
            false,
            vec![act("Import", true, json!({"cmd": "calendar_import", "id": id})), act("Not this", false, json!({"cmd": "teams_transcript_dismiss", "id": id}))],
            Some("calendar/day".into()),
        ));
    }

    // Echo itself: permissions a recording needs, an update.
    // Only what is known to be refused: macOS answers "not determined" for audio capture even when granted.
    let report = crate::permissions::report();
    if report.is_err() {
        c.keep_kinds.insert("health");
    }
    if let Ok(r) = report {
        for k in r.checks.iter().filter(|k| k.required && matches!(k.status, crate::permissions::Status::Denied)) {
            out.push(item(format!("permission:{}", k.id), "health", "echo", format!("{} isn't granted", k.name), k.why.to_string(), false, vec![act("Ask for it", true, json!({"cmd": "settings_action", "action": "ask_permissions", "arg": ""}))], Some("settings/health".into())));
        }
    }
    // An older Echo plugin in Claude Code: its terminal pane still claims calls for minutes itself.
    let stale = crate::app::stale_plugins();
    if !stale.is_empty() {
        let which = stale.iter().map(|s| format!("{} ({}{})", s["version"].as_str().unwrap_or("?"), s["scope"].as_str().unwrap_or("?"), s["project"].as_str().and_then(|p| std::path::Path::new(p).file_name()).map(|n| format!(", {}", n.to_string_lossy())).unwrap_or_default())).collect::<Vec<_>>().join(", ");
        out.push(item("plugin:stale".into(), "health", "echo", "Claude Code has an older Echo plugin".into(), format!("{which} — it can hold calls back from Write minutes; update it, then restart Claude Code"), false, vec![act("Update it", true, json!({"cmd": "update_plugins"}))], Some("settings/health".into())));
    }
    // Not checked yet looks the same as none: an update leaves quietly, never as "done".
    c.quiet_kinds.insert("update");
    if let Some(v) = crate::update::AVAILABLE.lock().ok().and_then(|a| a.as_ref().map(|r| r.version.clone())) {
        out.push(item("update".into(), "update", "echo", format!("Echo {v} is available"), "Installs after Echo saves and quits".into(), false, vec![act("Update…", true, json!({"cmd": "settings_action", "action": "update", "arg": ""}))], None));
    }
    // Routines whose last run failed (Scheduler plugin): open until a run goes through, or it's gone.
    if crate::routines::active() {
        let states = crate::routines::states();
        for r in crate::routines::list().into_iter().flatten() {
            let Some(last) = states.get(&r.id).and_then(|s| s.last.as_ref()) else { continue };
            if last["state"] != "failed" {
                continue;
            }
            let why: String = last["message"].as_str().unwrap_or("It didn't finish").chars().take(200).collect();
            let mut acts = vec![act("Run again", true, json!({"cmd": "routine_op", "op": "run", "id": r.id}))];
            if let Some(p) = last["needs_sign_in"].as_str() {
                acts.insert(0, act("Sign in…", true, json!({"cmd": "sign_in", "profile": p})));
                acts[1]["primary"] = false.into();
            }
            out.push(item(format!("routine:{}", r.id), "routine", "echo", format!("Routine failed: {}", r.name), why, false, acts, Some("settings/plugins".into())));
        }
    } else {
        c.quiet_kinds.insert("routine");
    }
    c.items = out;
    c
}

fn file() -> std::path::PathBuf {
    paths::state_dir().join("attention.json")
}

const DONE_DAYS: i64 = 7;
const DONE_MAX: usize = 100;

/// Fold a collection into the record and return the open list: items gone since last time are Done
/// (at now) when their source was read, kept open as they were when it couldn't be, dropped quietly
/// when it is off or they aged out; items back leave Done.
fn fold(rec: &mut Value, got: &Collected, now: chrono::DateTime<chrono::Utc>) -> Vec<Value> {
    if !rec.is_object() {
        *rec = json!({});
    }
    let before: Vec<Value> = match &rec["open"] {
        Value::Array(a) => a.clone(),
        _ => vec![],
    };
    let mut done: Vec<Value> = rec["done"].as_array().cloned().unwrap_or_default();
    let mut open = got.items.clone();
    let keys: BTreeSet<String> = open.iter().filter_map(|i| i["key"].as_str().map(str::to_owned)).collect();
    for b in before {
        let (k, kind) = (b["key"].as_str().unwrap_or("").to_string(), b["kind"].as_str().unwrap_or(""));
        if k.is_empty() || keys.contains(&k) {
            continue;
        }
        if got.keep.contains(&k) || got.keep_kinds.contains(kind) {
            open.push(b);
        } else if !(got.quiet.contains(&k) || got.quiet_kinds.contains(kind)) {
            done.push(json!({"key": k, "title": b["title"], "group": b["group"], "kind": kind, "at": now.to_rfc3339()}));
        }
    }
    let keys: BTreeSet<&str> = open.iter().filter_map(|i| i["key"].as_str()).collect();
    done.retain(|d| !keys.contains(d["key"].as_str().unwrap_or("")) && d["at"].as_str().and_then(|a| chrono::DateTime::parse_from_rfc3339(a).ok()).is_some_and(|a| (now - a.with_timezone(&chrono::Utc)).num_days() < DONE_DAYS));
    if done.len() > DONE_MAX {
        done.drain(..done.len() - DONE_MAX);
    }
    open.sort_by_key(|i| !i["urgent"].as_bool().unwrap_or(false));
    rec["open"] = json!(open);
    rec["done"] = json!(done);
    open
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
static CACHE: Mutex<Option<(Instant, Value)>> = Mutex::new(None);
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
static COLLECTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Moved by `stale`: a collection that saw it move while it ran is answered but not cached.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn shape(open: Vec<Value>, rec: &Value) -> Value {
    let mut done = rec["done"].as_array().cloned().unwrap_or_default();
    done.reverse();
    let clients: BTreeMap<String, String> = crate::projects::clients().unwrap_or_default().into_iter().map(|c| (c.id, c.name)).collect();
    let urgent = open.iter().filter(|i| i["urgent"] == true).count();
    json!({"open": open, "done": done, "clients": clients, "urgent": urgent})
}

/// Collect again at the next request: a job an item started has ended (the item may be gone now).
/// A collection running meanwhile may have read the old state: it isn't kept (`GENERATION`).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn stale() {
    GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut c) = CACHE.lock() {
        *c = None;
    }
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
/// The list for the window: open items (blocking first), Done this week (newest first), and the
/// groups' names. Collected at most every 20 s unless `fresh`, one collection at a time (a request
/// meanwhile gets the last answer); only the app writes the record.
pub fn view(fresh: bool) -> Value {
    use std::sync::atomic::Ordering;
    let cached = CACHE.lock().ok().and_then(|c| c.clone());
    if let Some((at, v)) = &cached {
        if (!fresh && at.elapsed() < Duration::from_secs(20)) || COLLECTING.load(Ordering::SeqCst) {
            return v.clone();
        }
    }
    if COLLECTING.swap(true, Ordering::SeqCst) {
        // Another request collects right now and there is no answer yet: wait for it.
        while COLLECTING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
        }
        if let Some((_, v)) = CACHE.lock().ok().and_then(|c| c.clone()) {
            return v;
        }
    }
    // Cleared however the collection ends (a panic too), so later requests never wait forever.
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            COLLECTING.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _done = Done;
    let generation = GENERATION.load(Ordering::SeqCst);
    let got = collect();
    let mut rec = read(&file());
    let open = fold(&mut rec, &got, chrono::Utc::now());
    let _ = paths::write_atomic(&file(), serde_json::to_string_pretty(&rec).unwrap_or_default().as_bytes());
    let v = shape(open, &rec);
    if let Ok(mut c) = CACHE.lock() {
        if GENERATION.load(Ordering::SeqCst) == generation {
            *c = Some((Instant::now(), v.clone()));
        }
    }
    v
}

/// The same for the CLI, read only: a terminal's process sees neither the app's sign-in checks nor
/// its update check, so it never writes the record (what it can't see would look done).
pub fn peek() -> Value {
    let got = collect();
    let mut rec = read(&file());
    let open = fold(&mut rec, &got, chrono::Utc::now());
    shape(open, &rec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn got(items: &[&Value]) -> Collected {
        Collected { items: items.iter().map(|v| (*v).clone()).collect(), ..Default::default() }
    }

    #[test]
    fn minutes_still_waiting_are_listed_with_write_now_where_it_helps() {
        let (_env, root) = crate::paths::testenv::folders("attention-minutes-wait");
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let call = |slug: &str, title: &str, run: Value| {
            let d = root.join(format!("home/calls/{day}_090000-{slug}"));
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("meta.json"), json!({"title": title}).to_string()).unwrap();
            std::fs::write(d.join(crate::notes::RUN), run.to_string()).unwrap();
        };
        let me = std::process::id();
        call("planning", "Planning", json!({"state": "queued", "profile": "system", "pid": me, "wait": "call", "may_start": true, "message": "Claude Code (as configured) waits for the recording of “Weekly sync” to end"}));
        call("review", "Review", json!({"state": "queued", "profile": "system", "pid": me, "wait": "model", "message": "Claude Code (as configured) waits for the model to finish another job"}));
        // Starting (every run passes through it): nothing to say yet. Its owner gone: failed.
        call("starting", "Starting", json!({"state": "queued", "profile": "system", "pid": me, "message": "Starting…"}));
        call("lost", "Lost", json!({"state": "queued", "profile": "system", "pid": 999_999_999u32, "wait": "call"}));
        let got = collect().items;
        assert!(!got.iter().any(|i| i["title"].as_str().is_some_and(|t| t.ends_with("Starting"))), "{got:?}");
        assert!(got.iter().any(|i| i["title"] == "Minutes failed: Lost"), "{got:?}");
        let find = |t: &str| got.iter().find(|i| i["title"] == format!("Minutes not written yet: {t}")).cloned().unwrap_or_else(|| panic!("no item for {t}: {got:?}"));
        let p = find("Planning");
        assert!(p["why"].as_str().unwrap().contains("“Weekly sync”"));
        assert_eq!(p["actions"][0]["label"], "Write now");
        assert_eq!(p["actions"][0]["ipc"]["cmd"], "notes_now");
        // Waiting for the model: nothing to start early.
        assert!(find("Review")["actions"].as_array().unwrap().is_empty());
    }

    #[test]
    fn gone_items_are_done_for_a_week() {
        let now = chrono::Utc::now();
        let a = item("a".into(), "sender", "acme", "A".into(), String::new(), false, vec![], None);
        let b = item("b".into(), "found", "-", "B".into(), String::new(), false, vec![], None);
        let mut rec = Value::Null;
        assert_eq!(fold(&mut rec, &got(&[&a, &b]), now).len(), 2);
        fold(&mut rec, &got(&[&b]), now);
        assert_eq!(rec["done"][0]["key"], "a");
        assert_eq!(rec["done"][0]["title"], "A");
        // Back again: open, not done.
        fold(&mut rec, &got(&[&a, &b]), now);
        assert!(rec["done"].as_array().unwrap().is_empty());
        fold(&mut rec, &got(&[]), now);
        assert_eq!(rec["done"].as_array().unwrap().len(), 2);
        fold(&mut rec, &got(&[]), now + chrono::Duration::days(8));
        assert!(rec["done"].as_array().unwrap().is_empty(), "a week later they are gone");
    }

    #[test]
    fn unread_sources_keep_their_items_and_switched_off_ones_go_quietly() {
        let now = chrono::Utc::now();
        let s = item("signin:work".into(), "signin", "echo", "Sign in".into(), String::new(), true, vec![], None);
        let f = item("found:x".into(), "found", "-", "F".into(), String::new(), false, vec![], None);
        let w = item("sender:acme:x".into(), "sender", "acme", "W".into(), String::new(), false, vec![], None);
        let mut rec = Value::Null;
        fold(&mut rec, &got(&[&s, &f, &w]), now);
        // The sign-in not known yet, Found for you unreadable, the Brain off.
        let mut c = got(&[]);
        c.keep.insert("signin:work".into());
        c.keep_kinds.insert("found");
        c.quiet_kinds.insert("sender");
        let open = fold(&mut rec, &c, now);
        assert_eq!(open.len(), 2);
        assert_eq!(open[0]["key"], "signin:work", "the blocking one first");
        assert!(rec["done"].as_array().unwrap().is_empty(), "nothing was decided");
    }
}

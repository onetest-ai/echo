//! Permission cards and Curate's edits (spec K12-08, K12-14, D24; rulings R24, R25). A card for an `Edit` or `Write`
//! shows Echo's own diff of the file on disk now — what the tool will do, never Claude Code's word for it —, or why Echo
//! can't show one (Allow stays possible: Claude Code's own tool then fails and says so). An allowed edit that comes back
//! done is a line of the vault's activity (`by: ask`, the conversation). When a turn ends and the vault commits, the
//! files it edited that had no uncommitted changes before it are committed together (slice 1's X-G2); a file the user
//! had changed is never committed.

use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;

use crate::knowledge::{activity, diff, git, page, scan, stream, Vault};

/// The file an edit request names, as Echo read it for the card.
pub(super) struct Edit {
    /// Vault-relative, with `/`.
    pub rel: String,
    /// Joined to the vault's root (`page::inside`: links followed and kept inside).
    pub full: PathBuf,
    pub old: Before,
}

/// A file before an edit.
pub(super) enum Before {
    /// There was none.
    Missing,
    Text(String),
    /// Echo can't read it as text (too large, not UTF-8, unreadable): why. The edit still has its activity line and
    /// commit, with nothing counted.
    Unread(String),
}

/// What a turn did, kept from its message to its end (K12-08, K12-14).
#[derive(Default)]
pub(super) struct Turn {
    /// The user's message: its first line names the commit.
    pub question: String,
    /// Claude Code refused a call it should have asked Echo about, and not because of the guard (K12-08, ruling R28).
    pub unasked: bool,
    /// Allowed edits waiting for their result, by tool call id.
    pub allowed: HashMap<String, Edit>,
    /// Files an allowed edit changed: vault-relative path → the file.
    pub edited: BTreeMap<String, PathBuf>,
    /// Per file, whether git had no uncommitted changes in it when the turn first asked to edit it — before any edit of
    /// the turn could be made. Whatever the commit switch says now: it is read at the turn's end.
    pub clean: HashMap<PathBuf, bool>,
}

/// What Echo adds to a permission event, and what it read for it.
#[derive(Default)]
pub(super) struct Card {
    /// `hunks` (the changes with 3 lines around them: `[[["=" | "-" | "+", line], …], …]`), `plus` and `minus`; or
    /// `error`, why Echo can't show the change.
    pub fields: Map<String, Value>,
    /// An `Edit` or `Write` of a file of the vault.
    pub edit: Option<Edit>,
    /// The file had no uncommitted changes.
    pub clean: Option<bool>,
}

/// The card of permission request `r` (the control request's `request`): for an `Edit` or `Write`, Echo's diff of the
/// file on disk now against what the tool will make of it (R24); nothing for other tools.
pub(super) fn card(v: &Vault, r: &Value) -> Card {
    let tool = r["tool_name"].as_str().unwrap_or("");
    if tool != "Edit" && tool != "Write" {
        return Card::default();
    }
    let input = &r["input"];
    let mut c = Card::default();
    let shown = on_disk(v, tool, input).and_then(|edit| {
        c.clean = Some(git::dirty(&v.root, std::slice::from_ref(&edit.full)).is_empty());
        let d = match &edit.old {
            Before::Unread(why) => Err(why.clone()),
            Before::Missing => proposed(tool, input, &edit.rel, None).map(|new| diff::lines("", &new)),
            Before::Text(cur) => proposed(tool, input, &edit.rel, Some(cur)).map(|new| diff::lines(cur, &new)),
        };
        c.edit = Some(edit);
        d
    });
    match shown {
        Ok(d) => {
            let (plus, minus) = diff::counts(&d);
            let line = |(k, l): &(char, String)| json!([k.to_string(), l]);
            c.fields.insert("hunks".into(), json!(diff::hunks(&d, 3).iter().map(|h| h.iter().map(line).collect::<Vec<_>>()).collect::<Vec<_>>()));
            c.fields.insert("plus".into(), json!(plus));
            c.fields.insert("minus".into(), json!(minus));
        }
        Err(why) => {
            c.fields.insert("error".into(), json!(why));
        }
    }
    c
}

/// The file the request names, through the vault's path check, and its text now.
fn on_disk(v: &Vault, tool: &str, input: &Value) -> Result<Edit, String> {
    let rel = stream::tool_path(tool, input, v).ok_or("Echo can't show this change: it isn't a file of this vault")?;
    let full = page::inside(v, &rel).map_err(|e| format!("Echo can't show this change: {e}"))?;
    let old = match std::fs::read(&full) {
        Ok(b) if b.len() as u64 > scan::READ_MAX => Before::Unread(format!("Echo can't show this change: {rel} is too large")),
        Ok(b) => String::from_utf8(b).map_or_else(|_| Before::Unread(format!("Echo can't show this change: {rel} isn't text")), Before::Text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Before::Missing,
        Err(e) => Before::Unread(format!("Echo can't show this change: {rel} can't be read ({e})")),
    };
    Ok(Edit { rel, full, old })
}

/// The file's text once the tool has run (`cur`: its text now, `None` when there is no file): `Write`'s content;
/// `Edit`'s `old_string` replaced once — it must be in the file exactly once — or, with `replace_all`, everywhere.
fn proposed(tool: &str, input: &Value, rel: &str, cur: Option<&str>) -> Result<String, String> {
    let s = |k: &str| input[k].as_str().unwrap_or("");
    if tool == "Write" {
        return Ok(s("content").to_owned());
    }
    let Some(cur) = cur else { return Err(format!("{rel} isn't there")) };
    let (old, new) = (s("old_string"), s("new_string"));
    let n = if old.is_empty() { 0 } else { cur.matches(old).count() };
    match (n, input["replace_all"] == true) {
        (0, _) => Err(format!("The text to replace isn't in {rel} now")),
        (_, true) => Ok(cur.replace(old, new)),
        (1, false) => Ok(cur.replacen(old, new, 1)),
        (n, false) => Err(format!("The text to replace is in {rel} {n} times, not once")),
    }
}

/// An allowed edit came back done: its line in the vault's activity, its text now against its text before — or, when
/// Echo can't read either as text, the line with nothing counted.
pub(super) fn record(v: &Vault, conversation: &str, e: &Edit) {
    let now = std::fs::read(&e.full).ok().filter(|b| b.len() as u64 <= scan::READ_MAX).and_then(|b| String::from_utf8(b).ok());
    match (&e.old, now) {
        (Before::Text(old), Some(new)) => activity::changed(v, &e.rel, "ask", Some(conversation), Some(old), &new, None),
        (Before::Missing, Some(new)) => activity::changed(v, &e.rel, "ask", Some(conversation), None, &new, None),
        (old, _) => {
            let kind = if matches!(old, Before::Missing) { "new" } else { "edit" };
            let line = activity::Entry { at: chrono::Local::now().to_rfc3339(), path: e.rel.clone(), kind: kind.into(), by: "ask".into(), r#ref: Some(conversation.into()), plus: 0, minus: 0, commit: None };
            if let Err(err) = activity::record(v, line) {
                eprintln!("knowledge: activity of {} in {}: {err:#}", e.rel, v.id);
            }
        }
    }
}

/// The turn is over: when the vault commits now (`v` read at the turn's end), the files it edited that were clean
/// before it are committed in one commit, `Ask: <the question's first line>`. The notices that say so — why something
/// wasn't committed only once a conversation (X-G4): the reasons `told` before are left out, and the new ones returned.
pub(super) fn commit(v: &Vault, t: &Turn, told: &BTreeSet<String>) -> (Vec<Value>, Vec<String>) {
    if !v.git.commit || t.edited.is_empty() {
        return (vec![], vec![]);
    }
    // The notices, and why a file wasn't committed (the reason, the notice's text).
    let (mut out, mut reasons): (Vec<Value>, Vec<(String, String)>) = (vec![], vec![]);
    let (clean, yours): (Vec<_>, Vec<_>) = t.edited.iter().partition(|(_, full)| t.clean.get(*full) == Some(&true));
    if !clean.is_empty() {
        let paths: Vec<PathBuf> = clean.iter().map(|(_, full)| (*full).clone()).collect();
        let first = t.question.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
        let message = format!("Ask: {}", first.chars().take(60).collect::<String>().trim_end());
        match git::commit_paths(&v.root, &paths, &message) {
            git::Commit::Done(id) => out.push(json!({"type": "notice", "kind": "committed", "text": format!("Committed {} file{} \u{b7} {id}", paths.len(), if paths.len() == 1 { "" } else { "s" })})),
            git::Commit::Skipped(why) => reasons.push((why.clone(), format!("Not committed: {why}"))),
            git::Commit::Off => {}
        }
    }
    if !yours.is_empty() {
        let names: Vec<&str> = yours.iter().map(|(rel, _)| rel.as_str()).collect();
        reasons.push(("changes of yours".into(), format!("Not committed: {} had changes of yours that weren't committed", names.join(", "))));
    }
    let fresh: Vec<(String, String)> = reasons.into_iter().filter(|(why, _)| !told.contains(why)).collect();
    out.extend(fresh.iter().map(|(_, text)| json!({"type": "notice", "kind": "not-committed", "text": text})));
    (out, fresh.into_iter().map(|(why, _)| why).collect())
}

#[cfg(all(test, unix))]
mod tests {
    use super::super::tests::{clean, file, of_type, script, signed_in, vault};
    use super::super::{events_until, permit, read, send, turn, turn_events, Mode};
    use super::*;
    use crate::ai::{self, fake};
    use crate::guard;
    use crate::knowledge::{activity, fixture, git, AddOpts};
    use serde_json::json;
    use std::path::Path;
    use std::time::Duration;

    /// Acme's page after the pilot line is added under its heading.
    const ACME_AFTER: &str = "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp\n\nPilot on 18 November.\n\nSee [[2026-01-01-a]].\n";

    /// Claude Code's `init` line, first in a process's output.
    fn init() -> Value {
        serde_json::from_str(turn("").lines().next().unwrap()).unwrap()
    }

    /// Claude Code asking the host whether `tool` may run with `input` (request `req`, tool call `call`).
    fn ask(req: &str, call: &str, tool: &str, input: Value) -> Value {
        json!({"type": "control_request", "request_id": req, "request": {"subtype": "can_use_tool", "tool_name": tool, "display_name": tool, "input": input, "tool_use_id": call}})
    }

    /// A tool call's result: done, or refused or failed.
    fn done(call: &str, ok: bool) -> Value {
        let text = if ok { "The file has been updated successfully." } else { "The user declined this change." };
        json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": call, "content": text, "is_error": !ok}]}, "parent_tool_use_id": null})
    }

    /// The turn's end, with the tools Claude Code counts as denied.
    fn end(denied: &[&str]) -> Value {
        let denials: Vec<Value> = denied.iter().map(|t| json!({"tool_name": t, "tool_use_id": "toolu_x", "tool_input": {}})).collect();
        json!({"type": "result", "subtype": "success", "is_error": false, "stop_reason": "end_turn", "duration_ms": 900, "usage": {"input_tokens": 12, "output_tokens": 7}, "permission_denials": denials})
    }

    fn lines(l: &[Value]) -> String {
        l.iter().map(|x| format!("{x}\n")).collect()
    }

    /// When the fake plays `{"_apply":n}`, `path` becomes `text` (Claude Code making the edit).
    fn apply(p: &ai::Profile, n: u32, path: &Path, text: &str) {
        std::fs::write(fake::file(p, &format!("apply.{n}")), text).unwrap();
        std::fs::write(fake::file(p, &format!("apply.{n}.to")), path.to_str().unwrap()).unwrap();
    }

    /// The answers to permission requests Claude Code read.
    fn answers(p: &ai::Profile) -> Vec<Value> {
        file(p, "chat.in.jsonl").lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|l| l["type"] == "control_response").collect()
    }

    fn all_lines(v: &Vault) -> Vec<activity::Entry> {
        activity::recent(v, "2000-01-01T00:00:00+00:00")
    }

    #[test]
    fn edit_requests_carry_echos_diff_and_wait() {
        let (_env, root) = crate::paths::testenv::folders("chat-edit-card");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let acme = v.root.join("knowledge/clients/acme.md");
        let input = json!({"file_path": acme, "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nPilot on 18 November.\n", "replace_all": false});
        apply(&p, 1, &acme, ACME_AFTER);
        script(&p, "chat.1.jsonl", &lines(&[init(), ask("req-1", "toolu_1", "Edit", input.clone()), json!({"_apply": 1}), done("toolu_1", true), end(&[])]));
        let id = send(&v.id, None, "Note the pilot date on Acme", Mode::Curate, &[]).unwrap();
        let ev = events_until(&v.id, &id, |_, c| c.waiting);
        let shown = of_type(&ev, "permission")[0].clone();
        assert_eq!(shown["path"], "knowledge/clients/acme.md");
        assert_eq!((shown["plus"].as_u64(), shown["minus"].as_u64()), (Some(2), Some(0)), "{shown}");
        let hunk = shown["hunks"][0].as_array().cloned().unwrap_or_default();
        assert!(hunk.contains(&json!(["+", "Pilot on 18 November."])) && hunk.contains(&json!(["=", "# Acme Corp"])), "Echo's diff of the file on disk: {shown}");
        // It waits for the user: nothing answered, the turn still going.
        std::thread::sleep(Duration::from_millis(300));
        let c = read(&v.id, &id, 0, Duration::ZERO).unwrap();
        assert!(c.waiting && c.busy);
        assert!(answers(&p).is_empty(), "nothing answered before the user");
        permit(&id, "req-1", true, false, None).unwrap();
        let ev = turn_events(&v.id, &id);
        let a = answers(&p);
        assert_eq!(a.len(), 1, "the answer written to Claude Code: {a:?}");
        assert_eq!(a[0]["response"]["subtype"], "success");
        assert_eq!(a[0]["response"]["request_id"], "req-1");
        assert_eq!(a[0]["response"]["response"], json!({"behavior": "allow", "updatedInput": input}));
        let answered = of_type(&ev, "answered");
        assert_eq!((answered[0]["request"].as_str(), answered[0]["allow"].as_bool(), answered[0]["always"].as_bool()), (Some("req-1"), Some(true), Some(false)));
        assert!(permit(&id, "req-1", true, false, None).is_err(), "a request is answered once");

        // Ruling R24: the card shows what the tool will do, or why Echo can't.
        fixture::write(&v.root, "knowledge/clients/beta.md", "# Beta\n\n- one\n- one\n");
        let beta = v.root.join("knowledge/clients/beta.md");
        let of = |tool: &str, input: Value| card(&v, &json!({"tool_name": tool, "input": input})).fields;
        let twice = of("Edit", json!({"file_path": beta, "old_string": "- one\n", "new_string": "- uno\n"}));
        assert!(twice.get("diff").is_none() && twice["error"].as_str().is_some_and(|e| e.contains("2 times")), "{twice:?}");
        let missing = of("Edit", json!({"file_path": beta, "old_string": "- two\n", "new_string": "- dos\n", "replace_all": false}));
        assert!(missing.get("diff").is_none() && missing["error"].as_str().is_some_and(|e| e.contains("isn't in")), "{missing:?}");
        let all = of("Edit", json!({"file_path": beta, "old_string": "- one\n", "new_string": "- uno\n", "replace_all": true}));
        assert_eq!((all["plus"].as_u64(), all["minus"].as_u64()), (Some(2), Some(2)), "every occurrence: {all:?}");
        let whole = of("Write", json!({"file_path": beta, "content": "# Beta\n"}));
        assert_eq!(whole["hunks"], json!([[["=", "# Beta"], ["-", ""], ["-", "- one"], ["-", "- one"]]]), "the whole file against the one on disk");
        let new = of("Write", json!({"file_path": v.root.join("knowledge/clients/gamma.md"), "content": "# Gamma\n\nNew.\n"}));
        assert_eq!(new["hunks"], json!([[["+", "# Gamma"], ["+", ""], ["+", "New."]]]), "a new file");
        // A long page: the changes with three lines around them, not every line of it.
        let long: String = (1..=40).map(|i| format!("Line {i}\n")).collect();
        fixture::write(&v.root, "knowledge/clients/long.md", &long);
        let one = of("Edit", json!({"file_path": v.root.join("knowledge/clients/long.md"), "old_string": "Line 20\n", "new_string": "Line twenty\n"}));
        assert_eq!(one["hunks"].as_array().map(Vec::len), Some(1), "{one:?}");
        assert_eq!(one["hunks"][0].as_array().map(Vec::len), Some(8), "{one:?}");
        assert_eq!((one["plus"].as_u64(), one["minus"].as_u64(), one.get("diff")), (Some(1), Some(1), None));
        let outside = of("Edit", json!({"file_path": root.join("outside.md"), "old_string": "a", "new_string": "b"}));
        assert!(outside.get("diff").is_none() && outside["error"].is_string(), "{outside:?}");
        let gone = of("Edit", json!({"file_path": v.root.join("knowledge/clients/none.md"), "old_string": "a", "new_string": "b"}));
        assert!(gone.get("diff").is_none() && gone["error"].is_string(), "{gone:?}");
    }

    #[test]
    fn always_allows_the_rest_of_the_conversation() {
        let (_env, root) = crate::paths::testenv::folders("chat-always");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let acme = v.root.join("knowledge/clients/acme.md");
        let beta = v.root.join("knowledge/clients/beta.md");
        let edit = ask("req-1", "toolu_1", "Edit", json!({"file_path": acme, "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nPilot on 18 November.\n"}));
        let write = ask("req-2", "toolu_2", "Write", json!({"file_path": beta, "content": "# Beta Inc\n\nA new client.\n"}));
        apply(&p, 1, &acme, ACME_AFTER);
        apply(&p, 2, &beta, "# Beta Inc\n\nA new client.\n");
        script(&p, "chat.1.jsonl", &lines(&[init(), edit, json!({"_apply": 1}), done("toolu_1", true), write, json!({"_apply": 2}), done("toolu_2", true), end(&[])]));
        let id = send(&v.id, None, "Note the pilot and add Beta Inc", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-1", true, true, None).unwrap();
        let ev = turn_events(&v.id, &id);
        // The Write was Echo's to answer: allowed, with its diff, no card left waiting.
        let a = answers(&p);
        assert_eq!(a.iter().map(|x| x["response"]["request_id"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["req-1", "req-2"], "{a:?}");
        assert!(a.iter().all(|x| x["response"]["response"]["behavior"] == "allow"));
        assert_eq!(a[1]["response"]["response"]["updatedInput"]["content"], "# Beta Inc\n\nA new client.\n");
        let cards = of_type(&ev, "permission");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[1]["hunks"], json!([[["+", "# Beta Inc"], ["+", ""], ["+", "A new client."]]]), "the diff still happens");
        let answered = of_type(&ev, "answered");
        assert_eq!(answered.len(), 2, "{ev:#?}");
        assert!(answered.iter().all(|x| x["allow"] == true && x["always"] == true));
        // Each allowed edit is a line of the vault's activity (K12-T4).
        let lines_now = all_lines(&v);
        assert_eq!(lines_now.len(), 2, "{lines_now:?}");
        assert!(lines_now.iter().all(|l| l.by == "ask" && l.r#ref.as_deref() == Some(id.as_str())));
        // The conversation keeps it across a restart: an Ask message, then Curate again.
        script(&p, "chat.2.jsonl", &turn("Acme's page has the pilot."));
        send(&v.id, Some(&id), "What is on Acme's page?", Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        let edit = ask("req-3", "toolu_3", "Edit", json!({"file_path": acme, "old_string": "Pilot on 18 November.\n", "new_string": "Pilot on 25 November.\n"}));
        script(&p, "chat.3.jsonl", &lines(&[init(), edit, done("toolu_3", false), end(&[])]));
        send(&v.id, Some(&id), "The pilot moves to 25 November", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 3);
        assert_eq!(answers(&p).len(), 3, "answered by Echo after the restart too");
        assert_eq!(all_lines(&v).len(), 2, "an edit that failed leaves no line");
    }

    #[test]
    fn deny_sends_the_message_to_claude() {
        let (_env, root) = crate::paths::testenv::folders("chat-deny");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let acme = v.root.join("knowledge/clients/acme.md");
        let before = std::fs::read_to_string(&acme).unwrap();
        let edit = |req: &str, call: &str| ask(req, call, "Edit", json!({"file_path": acme, "old_string": "# Acme Corp\n", "new_string": "# Acme Corp — pilot client\n"}));
        script(&p, "chat.1.jsonl", &lines(&[init(), edit("req-1", "toolu_1"), done("toolu_1", false), end(&["Edit"])]));
        let id = send(&v.id, None, "Rename Acme's heading", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-1", false, true, Some("Keep the heading as it is; add a line below it instead".into())).unwrap();
        let ev = turn_events(&v.id, &id);
        let a = answers(&p);
        assert_eq!(a.len(), 1, "{a:?}");
        let r = &a[0]["response"]["response"];
        assert_eq!(r["behavior"], "deny");
        assert!(r["message"].as_str().is_some_and(|m| m.contains("Keep the heading as it is; add a line below it instead")), "{r}");
        assert!(r.get("updatedInput").is_none());
        let answered = of_type(&ev, "answered");
        assert_eq!((answered[0]["allow"].as_bool(), answered[0]["always"].as_bool()), (Some(false), Some(false)), "always is only for allowing");
        assert!(of_type(&ev, "error").is_empty(), "a request came, so Claude Code hands its prompts to Echo: {ev:#?}");
        assert!(all_lines(&v).is_empty());
        assert_eq!(std::fs::read_to_string(&acme).unwrap(), before);
        // Without a message, Claude Code is still told no.
        script(&p, "chat.2.jsonl", &lines(&[edit("req-2", "toolu_2"), done("toolu_2", false), end(&["Edit"])]));
        send(&v.id, Some(&id), "Rename it after all", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-2", false, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
        let a = answers(&p);
        assert_eq!(a[1]["response"]["response"]["behavior"], "deny");
        assert!(a[1]["response"]["response"]["message"].as_str().is_some_and(|m| !m.trim().is_empty()));
    }

    /// Claude Code's own refusal of a tool (`system`/`permission_denied`) with `message`.
    fn refused(tool: &str, message: &str) -> Value {
        json!({"type": "system", "subtype": "permission_denied", "tool_name": tool, "tool_use_id": "toolu_x", "message": message})
    }

    #[test]
    fn a_host_that_keeps_its_prompts_points_to_the_terminal() {
        let (_env, root) = crate::paths::testenv::folders("chat-fallback");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let fallback = |ev: &[Value]| of_type(ev, "error").iter().filter(|e| e["action"] == "terminal").count();
        // The guard refusing an edit of an instruction file is no missing prompt.
        let guard_said = format!("{}: CLAUDE.md is an instruction file; a conversation doesn't change it", guard::VAULT_WHO);
        script(&p, "chat.1.jsonl", &lines(&[init(), refused("Edit", &guard_said), done("toolu_x", false), end(&["Edit"])]));
        let id = send(&v.id, None, "Add the pilot rule to CLAUDE.md", Mode::Curate, &[]).unwrap();
        let ev = turn_events(&v.id, &id);
        assert_eq!(fallback(&ev), 0, "the guard's refusal: {ev:#?}");
        // In Ask nothing is asked about: a refusal there is none either.
        script(&p, "chat.2.jsonl", &lines(&[init(), refused("Edit", "Claude requested permissions to write to knowledge/clients/acme.md, but you haven't granted it yet."), end(&["Edit"])]));
        send(&v.id, Some(&id), "Fix the typo on Acme's page", Mode::Ask, &[]).unwrap();
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
        assert_eq!(fallback(&ev), 0, "{ev:#?}");
        // Curate, and Claude Code refused the edit itself without asking Echo (`host-only-denied.jsonl`): K12-08.
        let acme = v.root.join("knowledge/clients/acme.md");
        script(&p, "chat.3.jsonl", &lines(&[init(), refused("Edit", &format!("Claude requested permissions to write to {}, but you haven't granted it yet.", acme.display())), done("toolu_x", false), end(&["Edit"])]));
        send(&v.id, Some(&id), "Add the pilot line to Acme's page", Mode::Curate, &[]).unwrap();
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 3);
        let errors: Vec<&Value> = of_type(&ev, "error").into_iter().filter(|e| e["action"] == "terminal").collect();
        assert_eq!(errors.len(), 1, "{ev:#?}");
        assert_eq!(errors[0]["text"], "This Claude Code doesn't hand permission prompts to Echo");
    }

    /// The twin vault in a git repository with one commit (`extra` files in it too), on profile `w`, committing what
    /// Echo writes. None without git.
    fn git_vault(root: &Path, extra: &[(&str, &[u8])]) -> Option<Vault> {
        git::program()?;
        let t = fixture::twin(&root.join("twin"));
        for (rel, bytes) in extra {
            std::fs::write(t.join(rel), bytes).unwrap();
        }
        for args in [&["init", "-q", "-b", "main"][..], &["add", "-A"], &["commit", "-q", "-m", "The vault"]] {
            assert!(git::run(&t, args).unwrap().status.success(), "{args:?}");
        }
        Some(crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), profile: Some("w".into()), commit: Some(true), ..Default::default() }).unwrap())
    }

    fn commits(v: &Vault) -> String {
        git::out(&v.root, &["rev-list", "--count", "HEAD"]).unwrap()
    }

    #[test]
    fn curate_commits_only_clean_files() {
        let (_env, root) = crate::paths::testenv::folders("chat-commit");
        let _iso = git::tests::isolated(&root, true);
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let Some(v) = git_vault(&root, &[]) else { return };
        let t = v.root.clone();
        let acme = v.root.join("knowledge/clients/acme.md");
        let second = v.root.join("knowledge/notes/2026-01-02-b.md");
        // The user's own change, not committed.
        std::fs::write(&second, "---\ntitle: Second sync\ntags: [meeting]\n---\n# Second sync\n\nMy draft.\n").unwrap();
        apply(&p, 1, &acme, ACME_AFTER);
        apply(&p, 2, &second, "---\ntitle: Second sync\ntags: [meeting]\n---\n# Second sync\n\nMy draft.\n\nPilot on 18 November.\n");
        let one = ask("req-1", "toolu_1", "Edit", json!({"file_path": acme, "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nPilot on 18 November.\n"}));
        let two = ask("req-2", "toolu_2", "Edit", json!({"file_path": second, "old_string": "My draft.\n", "new_string": "My draft.\n\nPilot on 18 November.\n"}));
        script(&p, "chat.1.jsonl", &lines(&[init(), one, json!({"_apply": 1}), done("toolu_1", true), two, json!({"_apply": 2}), done("toolu_2", true), end(&[])]));
        let question = "Add the pilot date to Acme Corp and to the notes of the second sync, please\nBoth say 18 November.";
        let id = send(&v.id, None, question, Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-1", true, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| c.waiting && of_type(ev, "permission").len() == 2);
        permit(&id, "req-2", true, false, None).unwrap();
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "notice").len() == 2);
        // One commit, of the page that was clean before the turn only, named by the question's first line.
        assert_eq!(git::out(&t, &["rev-list", "--count", "HEAD"]).unwrap(), "2");
        assert_eq!(git::out(&t, &["log", "-1", "--format=%s"]).unwrap(), "Ask: Add the pilot date to Acme Corp and to the notes of the seco");
        assert_eq!(git::out(&t, &["show", "--name-only", "--format=", "HEAD"]).unwrap().trim(), "knowledge/clients/acme.md");
        let status = git::out(&t, &["status", "--porcelain"]).unwrap();
        assert!(status.contains("knowledge/notes/2026-01-02-b.md") && !status.contains("acme.md"), "the user's file stays theirs: {status}");
        let notices = of_type(&ev, "notice");
        let short = git::out(&t, &["rev-parse", "--short", "HEAD"]).unwrap();
        assert!(notices.iter().any(|n| n["kind"] == "committed" && n["text"] == format!("Committed 1 file \u{b7} {short}").as_str()), "{notices:?}");
        assert!(notices.iter().any(|n| n["kind"] == "not-committed" && n["text"].as_str().is_some_and(|t| t.contains("2026-01-02-b.md"))), "{notices:?}");
        // The next turn edits both again: the clean one is committed; why the other isn't was said once (X-G4).
        apply(&p, 3, &acme, &ACME_AFTER.replace("18 November", "25 November"));
        apply(&p, 4, &second, "---\ntitle: Second sync\ntags: [meeting]\n---\n# Second sync\n\nMy draft.\n\nPilot on 25 November.\n");
        let one = ask("req-3", "toolu_3", "Edit", json!({"file_path": acme, "old_string": "18 November", "new_string": "25 November"}));
        let two = ask("req-4", "toolu_4", "Edit", json!({"file_path": second, "old_string": "18 November", "new_string": "25 November"}));
        script(&p, "chat.2.jsonl", &lines(&[one, json!({"_apply": 3}), done("toolu_3", true), two, json!({"_apply": 4}), done("toolu_4", true), end(&[])]));
        send(&v.id, Some(&id), "The pilot moves to 25 November", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-3", true, true, None).unwrap();
        // A turn's notices come together: once its second `committed` is there, all of them are.
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "notice").iter().filter(|n| n["kind"] == "committed").count() == 2);
        assert_eq!(commits(&v), "3");
        let not: Vec<&Value> = of_type(&ev, "notice").into_iter().filter(|n| n["kind"] == "not-committed").collect();
        assert_eq!(not.len(), 1, "said once in the conversation: {not:?}");
    }

    #[test]
    fn allowed_edits_leave_activity_lines() {
        let (_env, root) = crate::paths::testenv::folders("chat-activity");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let acme = v.root.join("knowledge/clients/acme.md");
        let page = |rel: &str| v.root.join(rel);
        apply(&p, 1, &acme, ACME_AFTER);
        let one = ask("req-1", "toolu_1", "Edit", json!({"file_path": acme, "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nPilot on 18 November.\n"}));
        let two = ask("req-2", "toolu_2", "Edit", json!({"file_path": page("knowledge/notes/2026-01-02-b.md"), "old_string": "# Second sync\n", "new_string": "# Second sync (moved)\n"}));
        let three = ask("req-3", "toolu_3", "Edit", json!({"file_path": page("knowledge/notes/2026-01-03-c.md"), "old_string": "# Third\n", "new_string": "# Third sync\n"}));
        script(&p, "chat.1.jsonl", &lines(&[init(), one, json!({"_apply": 1}), done("toolu_1", true), two, done("toolu_2", false), three, done("toolu_3", false), end(&["Edit"])]));
        let id = send(&v.id, None, "Tidy the client and the notes", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-1", true, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| c.waiting && of_type(ev, "permission").len() == 2);
        permit(&id, "req-2", true, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| c.waiting && of_type(ev, "permission").len() == 3);
        permit(&id, "req-3", false, false, None).unwrap();
        let ev = turn_events(&v.id, &id);
        // The edit that was allowed and done is a line; the one that failed and the one denied are none.
        let l = all_lines(&v);
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!((l[0].path.as_str(), l[0].kind.as_str(), l[0].by.as_str(), l[0].r#ref.as_deref()), ("knowledge/clients/acme.md", "edit", "ask", Some(id.as_str())));
        assert_eq!((l[0].plus, l[0].minus, l[0].commit.as_deref()), (2, 0, None));
        assert!(of_type(&ev, "notice").is_empty(), "the vault doesn't commit: nothing to say");
    }

    #[test]
    fn webfetch_always_is_per_domain() {
        let (_env, root) = crate::paths::testenv::folders("chat-webfetch");
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let v = vault(&root, "w");
        let fetch = |req: &str, call: &str, url: &str| ask(req, call, "WebFetch", json!({"url": url, "prompt": "What are the prices?"}));
        script(&p, "chat.1.jsonl", &lines(&[
            init(),
            fetch("req-1", "toolu_1", "https://example.com/pricing"), done("toolu_1", true),
            fetch("req-2", "toolu_2", "https://example.com/team"), done("toolu_2", true),
            fetch("req-3", "toolu_3", "https://Pat@EXAMPLE.com:443/about"), done("toolu_3", true),
            // Ruling R27: a URL whose host Echo can't read as the fetch does has no domain, and is never Echo's to allow.
            fetch("req-4", "toolu_4", "https://evil.com\\@example.com/"), done("toolu_4", false),
            fetch("req-5", "toolu_5", "https://example.com.evil.com/"), done("toolu_5", false),
            fetch("req-6", "toolu_6", "https://evilexample.com/"), done("toolu_6", false),
            fetch("req-7", "toolu_7", "https://contoso.example/rates"), done("toolu_7", true),
            end(&[]),
        ]));
        let id = send(&v.id, None, "Compare Northwind Retail's prices with the market", Mode::Research, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-1", true, true, None).unwrap();
        // example.com's next pages are Echo's to allow; the others wait for the user, one by one.
        for (n, req) in [(4, "req-4"), (5, "req-5"), (6, "req-6")] {
            events_until(&v.id, &id, |ev, c| c.waiting && of_type(ev, "permission").len() == n);
            permit(&id, req, false, false, None).unwrap();
        }
        let ev = events_until(&v.id, &id, |ev, c| c.waiting && of_type(ev, "permission").len() == 7);
        let cards = of_type(&ev, "permission");
        assert_eq!(cards.iter().map(|c| c["domain"].as_str().unwrap_or("")).collect::<Vec<_>>(), ["example.com", "example.com", "example.com", "", "example.com.evil.com", "evilexample.com", "contoso.example"]);
        assert_eq!(cards[3]["url"], "https://evil.com\\@example.com/", "the URL as it is, with no domain");
        let a = answers(&p);
        let allowed: Vec<&str> = a.iter().filter(|x| x["response"]["response"]["behavior"] == "allow").map(|x| x["response"]["request_id"].as_str().unwrap_or("")).collect();
        assert_eq!(allowed, ["req-1", "req-2", "req-3"], "{a:?}");
        assert_eq!(a.len(), 6, "the user answered the three others: {a:?}");
        assert_eq!(of_type(&ev, "answered").iter().filter(|x| x["allow"] == true).count(), 3);
        permit(&id, "req-7", true, false, None).unwrap();
        turn_events(&v.id, &id);
        assert_eq!(answers(&p).len(), 7);
    }

    #[test]
    fn the_commit_switch_is_read_at_each_turns_end() {
        let (_env, root) = crate::paths::testenv::folders("chat-commit-switch");
        let _iso = git::tests::isolated(&root, true);
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let Some(v) = git_vault(&root, &[]) else { return };
        let page = |rel: &str| v.root.join(rel);
        let edit = |n: u32, rel: &str, old: &str, new: &str| {
            let full = page(rel);
            apply(&p, n, &full, &std::fs::read_to_string(&full).unwrap().replace(old, new));
            vec![ask(&format!("req-{n}"), &format!("toolu_{n}"), "Edit", json!({"file_path": full, "old_string": old, "new_string": new})), json!({"_apply": n}), done(&format!("toolu_{n}"), true)]
        };
        let turn_of = |first: bool, steps: Vec<Value>| lines(&[if first { vec![init()] } else { vec![] }, steps, vec![end(&[])]].concat());
        script(&p, "chat.1.jsonl", &turn_of(true, edit(1, "knowledge/clients/acme.md", "# Acme Corp\n", "# Acme Corp\n\nPilot on 18 November.\n")));
        let id = send(&v.id, None, "Note the pilot on Acme", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-1", true, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| !c.busy && !of_type(ev, "notice").is_empty());
        assert_eq!(commits(&v), "2");
        // *Commit what Echo writes* turned off between two turns of the same process: nothing is committed (X-G4).
        crate::knowledge::set(&v.id, "commit", "false").unwrap();
        script(&p, "chat.2.jsonl", &turn_of(false, edit(2, "knowledge/notes/2026-01-03-c.md", "# Third\n", "# Third sync\n")));
        send(&v.id, Some(&id), "Name the third sync", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        permit(&id, "req-2", true, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
        // An Ask turn, then Curate again: its process starts with commits off, and they are turned on mid-turn.
        script(&p, "chat.3.jsonl", &turn("Acme has the pilot."));
        send(&v.id, Some(&id), "What is on Acme?", Mode::Ask, &[]).unwrap();
        events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 3);
        assert_eq!(commits(&v), "2", "the second turn's edit wasn't committed");
        script(&p, "chat.4.jsonl", &turn_of(true, edit(4, "knowledge/notes/2026-01-02-b.md", "# Second sync\n", "# Second sync (moved)\n")));
        send(&v.id, Some(&id), "Mark the second sync as moved", Mode::Curate, &[]).unwrap();
        events_until(&v.id, &id, |_, c| c.waiting);
        crate::knowledge::set(&v.id, "commit", "true").unwrap();
        permit(&id, "req-4", true, false, None).unwrap();
        events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "notice").iter().filter(|n| n["kind"] == "committed").count() == 2);
        assert_eq!(commits(&v), "3");
        assert_eq!(git::out(&v.root, &["show", "--name-only", "--format=", "HEAD"]).unwrap().trim(), "knowledge/notes/2026-01-02-b.md");
    }

    #[test]
    fn edits_of_files_echo_cant_show_are_still_recorded() {
        let (_env, root) = crate::paths::testenv::folders("chat-unshown");
        let _iso = git::tests::isolated(&root, true);
        let _fake = fake::install(&root);
        let _clean = clean();
        let p = signed_in();
        let Some(v) = git_vault(&root, &[("knowledge/clients/legacy.md", &b"# Legacy\n\xff\xfe old bytes\n"[..])]) else { return };
        let legacy = v.root.join("knowledge/clients/legacy.md");
        apply(&p, 1, &legacy, "# Legacy\n\nFixed.\n");
        script(&p, "chat.1.jsonl", &lines(&[init(), ask("req-1", "toolu_1", "Write", json!({"file_path": legacy, "content": "# Legacy\n\nFixed.\n"})), json!({"_apply": 1}), done("toolu_1", true), end(&[])]));
        let id = send(&v.id, None, "Rewrite the legacy page as text", Mode::Curate, &[]).unwrap();
        let ev = events_until(&v.id, &id, |_, c| c.waiting);
        let shown = of_type(&ev, "permission")[0].clone();
        assert!(shown.get("hunks").is_none() && shown["error"].as_str().is_some_and(|e| e.contains("isn't text")), "{shown}");
        // Allowed all the same: its line (no texts to count) and its commit.
        permit(&id, "req-1", true, false, None).unwrap();
        let ev = events_until(&v.id, &id, |ev, c| !c.busy && !of_type(ev, "notice").is_empty());
        let l = all_lines(&v);
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!((l[0].path.as_str(), l[0].by.as_str(), l[0].plus, l[0].minus), ("knowledge/clients/legacy.md", "ask", 0, 0));
        assert_eq!(commits(&v), "2", "{ev:#?}");
        assert_eq!(git::out(&v.root, &["show", "--name-only", "--format=", "HEAD"]).unwrap().trim(), "knowledge/clients/legacy.md");
    }
}

//! Claude Code's stream as Echo's events (spec K12-04, K12-05, K13-03), on the recorded lines of Claude Code 2.1.310
//! (`testdata/stream/`) and on lines made up here.

use super::*;
use crate::knowledge::{fixture, AddOpts};
use serde_json::json;
use std::time::Duration;

fn vault(root: &std::path::Path) -> Vault {
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "knowledge/clients/northwind-retail.md", "---\ntitle: Northwind Retail\n---\n# Northwind Retail\n");
    crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
}

fn recorded(name: &str) -> &'static str {
    match name {
        "ask-turn" => include_str!("../testdata/stream/ask-turn.jsonl"),
        "curate-edit" => include_str!("../testdata/stream/curate-edit.jsonl"),
        "curate-deny" => include_str!("../testdata/stream/curate-deny.jsonl"),
        "webfetch" => include_str!("../testdata/stream/webfetch.jsonl"),
        "interrupt" => include_str!("../testdata/stream/interrupt.jsonl"),
        "suggestion" => include_str!("../testdata/stream/suggestion.jsonl"),
        "host-only-denied" => include_str!("../testdata/stream/host-only-denied.jsonl"),
        other => panic!("no recording {other}"),
    }
}

/// The recorded lines with a `type` (a `_sent` line is the host's own write), `/VAULT` made the test vault's root,
/// fed 10 ms apart.
fn replay(name: &str, v: &Vault) -> Vec<Value> {
    let root = serde_json::to_string(&v.root.to_string_lossy()).unwrap();
    let root = root.trim_matches('"');
    let mut p = Parser::new(v);
    let t0 = Instant::now();
    let mut out = vec![];
    for (i, line) in recorded(name).lines().enumerate() {
        if serde_json::from_str::<Value>(line).map_or(true, |l| l.get("type").is_none()) {
            continue;
        }
        out.extend(p.feed_at(&line.replace("/VAULT", root), t0 + Duration::from_millis(10 * i as u64)));
    }
    out.extend(p.finish());
    out
}

fn types(ev: &[Value]) -> Vec<&str> {
    ev.iter().map(|e| e["type"].as_str().unwrap()).collect()
}

fn of_type<'a>(ev: &'a [Value], t: &str) -> Vec<&'a Value> {
    ev.iter().filter(|e| e["type"] == t).collect()
}

fn line(v: Value) -> String {
    v.to_string()
}

fn stream_ev(index: u64, event: Value) -> String {
    let mut e = event;
    e["index"] = json!(index);
    line(json!({"type": "stream_event", "event": e, "parent_tool_use_id": null}))
}

/// A whole streamed text block with the given deltas, 200 ms apart.
fn text_block(p: &mut Parser, index: u64, deltas: &[&str], t0: Instant) -> Vec<Value> {
    let mut out = p.feed_at(&stream_ev(index, json!({"type": "content_block_start", "content_block": {"type": "text", "text": ""}})), t0);
    for (i, d) in deltas.iter().enumerate() {
        let at = t0 + Duration::from_millis(200 * (i as u64 + 1));
        out.extend(p.feed_at(&stream_ev(index, json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": d}})), at));
    }
    out.extend(p.feed_at(&stream_ev(index, json!({"type": "content_block_stop"})), t0 + Duration::from_secs(60)));
    out
}

fn html_of(ev: &[Value]) -> String {
    let last = of_type(ev, "text").into_iter().last().expect("a text event");
    assert_eq!(last["done"], true);
    last["html"].as_str().unwrap().to_string()
}

#[test]
fn ask_turn_becomes_events() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-ask");
    let v = vault(&root);
    let ev = replay("ask-turn", &v);
    let t = types(&ev);
    let pos = |ty: &str| t.iter().position(|x| *x == ty).unwrap_or_else(|| panic!("no {ty} in {t:?}"));
    assert_eq!(t[0], "thinking", "{t:?}");
    assert!(pos("thinking") < pos("tool") && pos("tool") < pos("tool_result"), "{t:?}");
    assert!(of_type(&ev, "thinking").iter().any(|e| e["done"] == true), "the thinking block is closed: {t:?}");

    let tools = of_type(&ev, "tool");
    assert_eq!(tools.iter().map(|e| e["name"].as_str().unwrap()).collect::<Vec<_>>(), ["Grep", "Grep", "Read"]);
    assert_eq!(tools[0]["verb"], "Searched");
    assert_eq!(tools[0]["target"], "\"Northwind\"");
    assert_eq!(tools[0]["readonly"], true);
    let read = tools[2];
    assert_eq!(read["verb"], "Read");
    assert_eq!(read["target"], "knowledge/clients/northwind-retail.md");
    assert_eq!(read["path"], "knowledge/clients/northwind-retail.md");
    assert!(tools.iter().all(|e| e["id"].as_str().unwrap().starts_with("toolu_")));

    let results = of_type(&ev, "tool_result");
    assert_eq!(results.len(), 3, "{t:?}");
    assert!(results.iter().all(|r| r["ok"] == true));
    assert_eq!(results[2]["id"], read["id"]);
    assert_eq!(results[2]["lines"], json!([1, 14]));
    assert!(results[2]["preview"].as_str().unwrap().contains("Northwind Retail"));
    assert!(!results[0]["summary"].as_str().unwrap().contains('\n'));

    let texts = of_type(&ev, "text");
    let blocks: std::collections::BTreeSet<&str> = texts.iter().map(|e| e["block"].as_str().unwrap()).collect();
    for b in &blocks {
        assert_eq!(texts.iter().filter(|e| e["block"] == *b && e["done"] == true).count(), 1, "block {b} ends once");
    }
    let last_text = texts.iter().rposition(|_| true).map(|_| html_of(&ev)).unwrap();
    assert!(last_text.starts_with("<p>"), "{last_text}");
    let last_text_at = t.iter().rposition(|x| *x == "text").unwrap();
    assert!(pos("tool") < last_text_at, "the answer follows the tools: {t:?}");

    assert_eq!(*t.last().unwrap(), "result", "{t:?}");
    let r = ev.last().unwrap();
    assert_eq!(r["tokens_in"], 6 + 39607 + 8508);
    assert_eq!(r["tokens_out"], 987);
    assert!((r["cache_read"].as_f64().unwrap() - 0.82).abs() < 0.005, "{r}");
    assert!(r["cost"].as_f64().unwrap() > 0.0);
    assert_eq!(r["stop"], "end_turn");
    assert!(r["seconds"].is_number());
    assert!(of_type(&ev, "error").is_empty());
}

#[test]
fn model_text_is_rendered_safely() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-safe");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    let md = "See <script>alert(1)</script> [x](javascript:alert(1)) [[clients/northwind-retail]] `knowledge/clients/northwind-retail.md` https://example.com [ok](https://example.com/a)";
    let ev = text_block(&mut p, 0, &[md], Instant::now());
    let html = html_of(&ev);
    assert!(!html.contains("<script"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
    assert!(!html.contains("javascript:alert(1)\""), "no javascript href: {html}");
    assert!(!html.contains("href=\"javascript"), "{html}");
    assert!(html.contains("href=\"#kn:knowledge/clients/northwind-retail.md\""), "{html}");
    assert!(html.contains("href=\"https://example.com/a\""), "{html}");
    // The streamed text never reaches an attribute or a tag unescaped.
    let ev = text_block(&mut p, 1, &["<img src=x onerror=alert(1)> <b>hi</b>"], Instant::now());
    let html = html_of(&ev);
    assert!(!html.contains("<img") && !html.contains("<b>"), "{html}");
}

#[test]
fn vault_paths_in_answers_become_links() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-links");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    let ev = text_block(&mut p, 0, &["See `knowledge/clients/northwind-retail.md`, `clients/northwind-retail.md`, `knowledge/clients/nobody.md` and `ls -la`."], Instant::now());
    let html = html_of(&ev);
    assert_eq!(html.matches("href=\"#kn:knowledge/clients/northwind-retail.md\"").count(), 2, "vault-relative and pages-root-relative: {html}");
    assert!(html.contains("<code>knowledge/clients/nobody.md</code>"), "a missing page stays code: {html}");
    assert!(html.contains("<code>ls -la</code>"), "{html}");
    assert!(!html.contains("kn-new"), "a code span never makes a new-page link: {html}");
}

#[test]
fn steps_file_writes_become_a_todo_event() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-todo");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    let path = format!("{}/.echo-ingest/steps.json", v.root.display());
    let steps = r#"[{"text":"Read the memo","state":"active"},{"text":"Update the client page","state":"todo"}]"#;
    let input = line(json!({"file_path": path, "content": steps}));
    // The Write arrives as input fragments that cut the JSON in two places.
    let (a, b) = input.split_at(30);
    let mut ev = p.feed_at(&stream_ev(0, json!({"type": "content_block_start", "content_block": {"type": "tool_use", "id": "toolu_1", "name": "Write", "input": {}}})), Instant::now());
    ev.extend(p.feed_at(&stream_ev(0, json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": a}})), Instant::now()));
    ev.extend(p.feed_at(&stream_ev(0, json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": b}})), Instant::now()));
    assert!(of_type(&ev, "tool").is_empty(), "a tool event appears when its input is complete");
    ev.extend(p.feed_at(&stream_ev(0, json!({"type": "content_block_stop"})), Instant::now()));
    assert_eq!(types(&ev), ["tool", "todo"], "{ev:?}");
    assert_eq!(ev[1]["items"], json!([{"text": "Read the memo", "state": "active"}, {"text": "Update the client page", "state": "todo"}]));

    // An Edit of the file applies to the last known content.
    let edit = json!({"type": "assistant", "message": {"id": "m2", "content": [{"type": "tool_use", "id": "toolu_2", "name": "Edit", "input":
        {"file_path": path, "old_string": r#""text":"Read the memo","state":"active""#, "new_string": r#""text":"Read the memo","state":"done""#}}]}});
    let ev = p.feed(&line(edit));
    assert_eq!(types(&ev), ["tool", "todo"], "{ev:?}");
    assert_eq!(ev[1]["items"][0], json!({"text": "Read the memo", "state": "done"}));
    assert_eq!(ev[1]["items"][1]["state"], "todo");

    // Another file, a broken file and an unknown state.
    let other = json!({"type": "assistant", "message": {"id": "m3", "content": [{"type": "tool_use", "id": "toolu_3", "name": "Write", "input": {"file_path": "/x/notes.json", "content": steps}}]}});
    assert_eq!(types(&p.feed(&line(other))), ["tool"]);
    let broken = json!({"type": "assistant", "message": {"id": "m4", "content": [{"type": "tool_use", "id": "toolu_4", "name": "Write", "input": {"file_path": path, "content": "[{\"text\": \"a\", \"state\": \"weird\"}, 5]"}}]}});
    let ev = p.feed(&line(broken));
    assert_eq!(ev[1]["items"], json!([{"text": "a", "state": "todo"}]));
}

#[test]
fn suggestion_becomes_an_event() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-sugg");
    let v = vault(&root);
    let ev = replay("suggestion", &v);
    let t = types(&ev);
    assert_eq!(t[t.len() - 2..], ["result", "suggestion"], "{t:?}");
    assert_eq!(ev.last().unwrap(), &json!({"type": "suggestion", "text": "fix the northwind page"}));
}

#[test]
fn bad_lines_are_skipped_and_counted() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-bad");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    for l in ["not json", "", "   ", "{broken", "{\"type\":\"rate_limit_event\",\"rate_limit_info\":{}}", "{\"_sent\":{}}", "[1,2]", "{\"type\":\"nope\"}", "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":9,\"delta\":{\"type\":\"text_delta\",\"text\":\"orphan\"}}}",
        "{\"type\":\"user\",\"message\":{\"content\":\"plain\"}}", "{\"type\":\"assistant\"}", "{\"type\":\"control_request\"}", "{\"type\":\"result\"}"] {
        let ev = p.feed(l);
        assert!(ev.iter().all(|e| e["type"] != "text"), "{l}: {ev:?}");
    }
    assert_eq!(p.bad_lines(), 2, "only lines that aren't JSON count");
    let ev = p.feed(r#"{"type":"prompt_suggestion","suggestion":"go on"}"#);
    assert_eq!(types(&ev), ["suggestion"], "the parser goes on after bad lines");
}

#[test]
fn utf8_split_across_deltas() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-utf8");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    let ev = text_block(&mut p, 0, &["caf", "é ", "日", "本 ✓ 😀"], Instant::now());
    assert!(html_of(&ev).contains("café 日本 ✓ 😀"));
    // A path whose escape sequences and characters are cut between input fragments.
    let mut ev = p.feed_at(&stream_ev(1, json!({"type": "content_block_start", "content_block": {"type": "tool_use", "id": "toolu_9", "name": "Read", "input": {}}})), Instant::now());
    for frag in ["{\"file_path\": \"/x/caf\\u00", "e9 \\ud83d", "\\ude00 日", "本.md\"}"] {
        ev.extend(p.feed_at(&stream_ev(1, json!({"type": "content_block_delta", "delta": {"type": "input_json_delta", "partial_json": frag}})), Instant::now()));
    }
    ev.extend(p.feed_at(&stream_ev(1, json!({"type": "content_block_stop"})), Instant::now()));
    assert_eq!(ev.len(), 1, "{ev:?}");
    assert_eq!(ev[0]["target"], "/x/café 😀 日本.md");
}

#[test]
fn large_results_are_capped() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-big");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    for body in ["x".repeat(3 << 20), "é".repeat(1 << 20), "é".repeat(2047) + "日本"] {
        let l = line(json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_1", "content": body}]}}));
        let ev = p.feed(&l);
        assert_eq!(ev.len(), 1);
        let pv = ev[0]["preview"].as_str().unwrap();
        assert!(pv.len() <= 4096, "{}", pv.len());
        assert!(body.starts_with(pv.trim_end_matches('…')), "cut at a character boundary");
        assert!(ev[0]["summary"].as_str().unwrap().chars().count() <= 160);
    }
    // A giant text block is held to a bound too.
    let ev = text_block(&mut p, 0, &[&"y".repeat(3 << 20)], Instant::now());
    assert!(html_of(&ev).len() < (3 << 20) + 1000);
}

#[test]
fn renders_are_throttled() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-throttle");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    let t0 = Instant::now();
    let mut ev = p.feed_at(&stream_ev(0, json!({"type": "content_block_start", "content_block": {"type": "text", "text": ""}})), t0);
    for i in 0..100u64 {
        let d = stream_ev(0, json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": format!("w{i} ")}}));
        ev.extend(p.feed_at(&d, t0 + Duration::from_micros(500 * i)));
    }
    let live = of_type(&ev, "text");
    assert!(live.len() <= 2, "100 deltas in 50 ms: {} renders", live.len());
    assert!(live.iter().all(|e| e["done"] == false));
    ev.extend(p.feed_at(&stream_ev(0, json!({"type": "content_block_stop"})), t0 + Duration::from_millis(60)));
    let texts = of_type(&ev, "text");
    let last = texts.last().unwrap();
    assert_eq!(last["done"], true);
    assert!(last["html"].as_str().unwrap().contains("w99"), "the final render holds everything");
    assert_eq!(texts.iter().filter(|e| e["done"] == true).count(), 1);
    // Slower deltas render every time.
    let mut p = Parser::new(&v);
    let ev = text_block(&mut p, 0, &["a", "b", "c"], t0);
    assert_eq!(of_type(&ev, "text").len(), 4, "three deltas 200 ms apart and the final");
    // An unfinished block is closed by finish().
    let mut p = Parser::new(&v);
    p.feed_at(&stream_ev(0, json!({"type": "content_block_start", "content_block": {"type": "text", "text": ""}})), t0);
    p.feed_at(&stream_ev(0, json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": "half"}})), t0);
    p.feed_at(&stream_ev(0, json!({"type": "content_block_delta", "delta": {"type": "text_delta", "text": " done"}})), t0 + Duration::from_millis(5));
    let ev = p.finish();
    assert_eq!(types(&ev), ["text"]);
    assert!(ev[0]["html"].as_str().unwrap().contains("half done") && ev[0]["done"] == true);
}

#[test]
fn permission_requests_and_denials() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-perm");
    let v = vault(&root);
    let ev = replay("curate-edit", &v);
    let perm = of_type(&ev, "permission");
    assert_eq!(perm.len(), 1, "{:?}", types(&ev));
    let p = perm[0];
    assert_eq!(p["tool"], "Edit");
    assert_eq!(p["path"], "knowledge/clients/northwind-retail.md");
    assert_eq!(p["input"]["new_string"], "## Decisions\n- 2026-11-05 \u{2014} Fix forward, no rollback.\n");
    assert!(p["request"].as_str().unwrap().starts_with("00000000-"));
    assert!(p.get("diff").is_none(), "the diff is added later");
    assert_eq!(of_type(&ev, "tool").last().unwrap()["verb"], "Edited");
    assert_eq!(of_type(&ev, "tool").last().unwrap()["detail"], "+1 \u{2212}0");

    let ev = replay("webfetch", &v);
    let p = of_type(&ev, "permission")[0].clone();
    assert_eq!((p["tool"].as_str(), p["url"].as_str(), p["domain"].as_str()), (Some("WebFetch"), Some("https://example.com"), Some("example.com")));
    assert!(p.get("path").is_none());

    let ev = replay("host-only-denied", &v);
    let d = of_type(&ev, "denied");
    assert_eq!(d.len(), 1, "{:?}", types(&ev));
    assert_eq!(d[0]["tool"], "Edit");
    assert!(d[0]["reason"].as_str().unwrap().contains("haven't granted"));
    assert!(of_type(&ev, "tool_result").iter().any(|r| r["ok"] == false));

    // The control response to Echo's own interrupt is not an event; the aborted result is a stop, not an error.
    let ev = replay("interrupt", &v);
    assert!(of_type(&ev, "error").is_empty(), "{:?}", types(&ev));
    assert_eq!(ev.last().unwrap()["stop"], "aborted_streaming");
    assert!(ev.last().unwrap().get("cost").is_none(), "no cost when Claude Code reports none");
}

#[test]
fn a_failed_result_is_an_error_event() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-fail");
    let v = vault(&root);
    let mut p = Parser::new(&v);
    let ev = p.feed(r#"{"type":"result","subtype":"success","is_error":true,"stop_reason":"stop_sequence","result":"Please run /login","usage":{"input_tokens":1,"output_tokens":2},"total_cost_usd":0}"#);
    assert_eq!(types(&ev), ["error", "result"]);
    assert_eq!(ev[0]["text"], "Please run /login");
    assert_eq!(p.session_id(), None);
    p.feed(r#"{"type":"system","subtype":"init","session_id":"abc-1","tools":[]}"#);
    assert_eq!(p.session_id(), Some("abc-1"));
}

#[test]
fn tool_lines_use_the_k13_03_texts() {
    let (_env, root) = crate::paths::testenv::folders("kn-st-tools");
    let v = vault(&root);
    let r = &v.root;
    let abs = |rel: &str| format!("{}/{rel}", r.display());
    let tl = |name: &str, input: Value| tool_line(name, &input, &v);
    let s = |a: &str, b: &str, c: &str, d: bool| (a.to_string(), b.to_string(), c.to_string(), d);
    assert_eq!(tl("Read", json!({"file_path": abs("knowledge/CATALOG.md"), "offset": 1, "limit": 180})), s("Read", "knowledge/CATALOG.md", "lines 1\u{2013}180", true));
    assert_eq!(tl("Read", json!({"file_path": abs("knowledge/CATALOG.md")})), s("Read", "knowledge/CATALOG.md", "", true));
    assert_eq!(tl("Read", json!({"file_path": abs("knowledge/CATALOG.md"), "offset": 40})), s("Read", "knowledge/CATALOG.md", "from line 40", true));
    assert_eq!(tl("Read", json!({"file_path": "/etc/hosts"})), s("Read", "/etc/hosts", "", true));
    assert_eq!(tl("Grep", json!({"pattern": "northwind", "path": abs("knowledge/notes")})), s("Searched", "\"northwind\"", "in knowledge/notes/", true));
    assert_eq!(tl("Grep", json!({"pattern": "x", "path": r.to_string_lossy()})), s("Searched", "\"x\"", "", true));
    assert_eq!(tl("Glob", json!({"pattern": "**/*.md"})), s("Searched", "**/*.md", "", true));
    assert_eq!(tl("Skill", json!({"skill": "twin", "args": "x"})), s("Used skill", "twin", "", true));
    assert_eq!(tl("Edit", json!({"file_path": abs("knowledge/CATALOG.md"), "old_string": "a\nb\n", "new_string": "a\nc\nd\ne\nf\n"})), s("Edited", "knowledge/CATALOG.md", "+4 \u{2212}1", false));
    assert_eq!(tl("Write", json!({"file_path": abs("knowledge/new.md"), "content": "1\n2\n3\n"})), s("Created", "knowledge/new.md", "3 lines", false));
    assert_eq!(tl("Write", json!({"file_path": abs("knowledge/CATALOG.md"), "content": "1\n"})), s("Wrote", "knowledge/CATALOG.md", "1 line", false));
    assert_eq!(tl("WebSearch", json!({"query": "rust parse"})), s("Searched the web", "\"rust parse\"", "", true));
    assert_eq!(tl("WebFetch", json!({"url": "https://user:pw@Example.com/pricing/tiers?a=b#c", "prompt": "p"})), s("Opened", "example.com/pricing/tiers", "", true));
    assert_eq!(tl("WebFetch", json!({"url": "https://example.com"})), s("Opened", "example.com", "", true));
    let long = tl("WebFetch", json!({"url": format!("https://example.com/{}", "a".repeat(200))}));
    assert!(long.1.chars().count() <= 61 && long.1.ends_with('\u{2026}'), "{long:?}");
    assert_eq!(tl("Agent", json!({"description": "Fetch the page", "subagent_type": "web-fetch"})), s("Delegated", "Fetch the page", "", false));
    assert_eq!(tl("Frobnicate", json!({})), s("Frobnicate", "", "", false));
    assert_eq!(tl("Read", json!("not an object")), s("Read", "", "", true));
    assert_eq!(tool_path("Read", &json!({"file_path": abs("../outside.md")}), &v), None, "a path that leaves the vault has no vault path");
    assert_eq!(tool_path("Read", &json!({"file_path": "knowledge/CATALOG.md"}), &v).as_deref(), Some("knowledge/CATALOG.md"));

    let (sum, pv, lines) = tool_result_summary("Read", "1\ta\n2\tb\n3\tc", false);
    assert_eq!((sum.as_str(), pv.as_str(), lines), ("3 lines", "1\ta\n2\tb\n3\tc", Some((1, 3))));
    assert_eq!(tool_result_summary("Read", "41\ta\n42\tb", false).2, Some((41, 42)));
    assert_eq!(tool_result_summary("Grep", "a.md:1:x\na.md:4:y\nb.md:2:z", false).0, "3 matches in 2 files");
    assert_eq!(tool_result_summary("Grep", "a.md\nb.md\nc.md", false).0, "3 files");
    assert_eq!(tool_result_summary("Grep", "No files found", false).0, "no matches");
    assert_eq!(tool_result_summary("Glob", "a.md\nb.md", false).0, "2 files");
    let (sum, _, lines) = tool_result_summary("Edit", "Error: file changed\nsecond line", true);
    assert_eq!((sum.as_str(), lines), ("Error: file changed", None));
    assert_eq!(tool_result_summary("Skill", "", false).0, "done");
}

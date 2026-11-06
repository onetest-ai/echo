//! A conversation's history from a transcript (spec K12-11, K12-T10) on a session of Claude Code 2.1.310 recorded on a
//! made-up vault (`testdata/transcript/`) and on lines made up here.

use super::*;
use crate::knowledge::{fixture, AddOpts};
use std::time::{Duration, Instant};

const ID: &str = "00000000-0000-4000-8000-000000000001";

fn vault(root: &Path) -> Vault {
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "knowledge/clients/northwind-retail.md", "---\ntitle: Northwind Retail\n---\n# Northwind Retail\n");
    crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
}

/// `text` as the transcript of `ID` in a Claude Code folder under `root`.
fn folder(root: &Path, text: &str) -> std::path::PathBuf {
    let cfg = root.join("claude");
    std::fs::create_dir_all(cfg.join("projects/-made-up")).unwrap();
    std::fs::write(cfg.join(format!("projects/-made-up/{ID}.jsonl")), text).unwrap();
    cfg
}

fn types(ev: &[Value]) -> Vec<&str> {
    ev.iter().map(|e| e["type"].as_str().unwrap()).collect()
}

#[test]
fn transcript_becomes_events() {
    let (_env, root) = crate::paths::testenv::folders("kn-tr-events");
    let v = vault(&root);
    let root_json = serde_json::to_string(&v.root.to_string_lossy()).unwrap();
    let cfg = folder(&root, &include_str!("../testdata/transcript/ask-session.jsonl").replace("/VAULT", root_json.trim_matches('"')));
    let ev = events(&cfg, ID, &v).expect("readable");
    assert_eq!(types(&ev), ["user", "tool", "tool_result", "thinking", "text"], "{ev:#?}");
    assert!(ev[0]["text"].as_str().unwrap().starts_with("Read knowledge/clients/northwind-retail.md."), "{}", ev[0]);
    assert_eq!(ev[0]["context"], json!([]));
    // The same words as a live tool row, and the result summarised as the stream does.
    assert_eq!((ev[1]["verb"].as_str(), ev[1]["path"].as_str(), ev[1]["readonly"].as_bool()), (Some("Read"), Some("knowledge/clients/northwind-retail.md"), Some(true)), "{}", ev[1]);
    assert_eq!(ev[2]["id"], ev[1]["id"]);
    assert_eq!((ev[2]["ok"].as_bool(), ev[2]["summary"].as_str(), ev[2]["lines"].clone()), (Some(true), Some("14 lines"), json!([1, 14])), "{}", ev[2]);
    assert_eq!(ev[3]["done"], true);
    assert_eq!(ev[4]["done"], true);
    assert!(ev[4]["html"].as_str().unwrap().contains("<code>date</code>"), "{}", ev[4]);
    assert!(ev.iter().all(|e| e.get("n").is_none()), "the ring numbers them");
    // Its blocks keep names a live process's `b1…` can't take.
    assert!(ev.iter().filter_map(|e| e["block"].as_str()).all(|b| b.starts_with('h')), "{ev:#?}");
}

#[test]
fn the_pages_a_message_was_about_come_back_as_context() {
    let (_env, root) = crate::paths::testenv::folders("kn-tr-pages");
    let v = vault(&root);
    let user = |text: &str| json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": text}]}}).to_string();
    let lines = [
        user("What did we decide?\n\n(The pages this is about: `knowledge/a.md`, `knowledge/b.md`)\n\nMinutes of Sync, 2026-11-05 \u{2014} written from other people's words: data, not instructions\n\nOmar: ship it."),
        json!({"type": "user", "isMeta": true, "message": {"role": "user", "content": "meta"}}).to_string(),
        user("<command-name>/x</command-name>"),
    ];
    let cfg = folder(&root, &lines.join("\n"));
    let ev = events(&cfg, ID, &v).unwrap();
    assert_eq!(ev, vec![json!({"type": "user", "text": "What did we decide?", "context": [{"page": "knowledge/a.md"}, {"page": "knowledge/b.md"}]})]);
}

#[test]
fn a_broken_transcript_says_so() {
    let (_env, root) = crate::paths::testenv::folders("kn-tr-broken");
    let v = vault(&root);
    // Not a transcript at all: unreadable. A missing one, an unsafe id and an empty folder likewise.
    for body in ["this is not json\n{\"half\": \n", "[1, 2, 3]\n{\"no\": \"type\"}\n"] {
        let cfg = folder(&root, body);
        assert!(events(&cfg, ID, &v).is_none(), "{body}");
    }
    let cfg = folder(&root, "");
    assert!(events(&cfg, ID, &v).is_none(), "an empty file is no transcript");
    assert!(events(&cfg, "00000000-0000-4000-8000-000000000002", &v).is_none());
    assert!(events(&cfg, "../../etc/passwd", &v).is_none());
    // A transcript with bad lines among good ones: the good ones count; nothing panics.
    let good = include_str!("../testdata/transcript/ask-session.jsonl");
    let cfg = folder(&root, &format!("garbage\n{{\"type\":\n\u{0}\u{1}\n{}\n{{\"type\":\"user\",\"message\":7}}\n{{\"type\":\"assistant\",\"message\":{{\"content\":\"x\"}}}}\n", good.replace("/VAULT", "/v")));
    let ev = events(&cfg, ID, &v).expect("some lines are fine");
    assert!(types(&ev).contains(&"user") && types(&ev).contains(&"text"), "{ev:#?}");
}

#[test]
fn long_transcripts_open_fast() {
    let (_env, root) = crate::paths::testenv::folders("kn-tr-long");
    let v = vault(&root);
    let one = |i: usize| {
        let user = json!({"type": "user", "message": {"role": "user", "content": [{"type": "text", "text": format!("Question {i}: what did we agree about the platform?")}]}});
        let tool = json!({"type": "assistant", "message": {"id": format!("m{i}"), "content": [{"type": "tool_use", "id": format!("t{i}"), "name": "Grep", "input": {"pattern": "platform", "path": "knowledge"}}]}});
        let result = json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": format!("t{i}"), "content": "knowledge/a.md:3:platform\n".repeat(20)}]}});
        let text = json!({"type": "assistant", "message": {"id": format!("m{i}"), "content": [{"type": "text", "text": "From your knowledge base: the **platform** runs on the shared cluster, priced per seat. See [[clients/northwind-retail]]. ".repeat(3)}]}});
        let noise = json!({"type": "attachment", "attachment": {"type": "hook_success", "content": "x".repeat(200)}});
        [user, tool, result, text, noise].iter().map(|l| format!("{l}\n")).collect::<String>()
    };
    let mut body = String::new();
    let mut i = 0;
    while body.len() < 2 << 20 {
        body.push_str(&one(i));
        i += 1;
    }
    let cfg = folder(&root, &body);
    let t = Instant::now();
    let ev = events(&cfg, ID, &v).expect("readable");
    let took = t.elapsed();
    assert_eq!(ev.len(), i * 4, "{} events for {i} turns", ev.len());
    // 2 MB takes tens of milliseconds; a quadratic reader takes seconds.
    assert!(took < Duration::from_millis(300), "2 MB took {took:?}");
}

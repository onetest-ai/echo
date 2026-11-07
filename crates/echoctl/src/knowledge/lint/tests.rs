//! AI checks with the fake Claude Code (`ai::fake`), on made-up vaults, pages, people and companies.

use super::*;
use crate::ai::{self, fake};
use crate::knowledge::health::{self, By, Group};
use crate::knowledge::{self, fixture, git, ingest, scan, AddOpts};
use crate::{guard, paths};
use serde_json::json;
use std::path::{Path, PathBuf};

const CLIENT: &str = "knowledge/clients/northwind-retail.md";
const CLIENT_TEXT: &str = "---\ntitle: Northwind Retail\ndate: 2026-10-02\ntags: [client]\n---\n\n# Northwind Retail\n\nA made-up retail client. Contact: Maya Chen.\n\n## Roadmap\n- Phase 2 go-live: 12 December, before the holiday freeze.\n";
/// The client page's roadmap line (line 12).
const OLD_LINE: &str = "- Phase 2 go-live: 12 December, before the holiday freeze.";
const NEW_LINE: &str = "- Phase 2 go-live: 9 January — moved on 5 Nov because of the holiday freeze.";
const NOTE: &str = "knowledge/notes/2026-11-05-northwind-weekly-sync.md";
const NOTE_TEXT: &str = "---\ntitle: Northwind weekly delivery sync\ndate: 2026-11-05\ntags: [meeting]\n---\n\n# Northwind weekly delivery sync — 2026-11-05\n\n## Decisions\n\n- Phase 2 go-live moves to 9 January: no releases in the holiday freeze.\n";
const TOPIC: &str = "knowledge/topics/per-stream-pricing.md";
const TOPIC_TEXT: &str = "---\ntitle: Per-stream pricing\ndate: 2026-09-14\ntags: [concept]\n---\n\n# Per-stream pricing\n\nA price per active stream needs one way to count streams.\nEvery client counts per active stream differently today.\n";

/// Stops this test's AI checks and ingest runs when it ends (declared after the fake and the folders: dropped first).
struct Clean;

impl Drop for Clean {
    fn drop(&mut self) {
        reset();
        ingest::reset();
    }
}

fn clean() -> Clean {
    reset();
    ingest::reset();
    Clean
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(knowledge::join(root, rel)).unwrap_or_default()
}

/// A vault from Echo's starter with a client page, a meeting note and a topic page, read by the fake's signed-in
/// profile `w` (git: committed).
fn vault(root: &Path, git_on: bool) -> (Vault, ai::Profile) {
    let p = fake::profile("w", None, &fake::signed_in("pat@example.com"));
    let c = knowledge::starter::create("My twin", Some(root.join("vault")), git_on, "off").unwrap();
    for (rel, text) in [(CLIENT, CLIENT_TEXT), (NOTE, NOTE_TEXT), (TOPIC, TOPIC_TEXT)] {
        fixture::write(&c.vault.root, rel, text);
    }
    if git_on {
        git::out(&c.vault.root, &["add", "--", CLIENT, NOTE, TOPIC]).unwrap();
        git::out(&c.vault.root, &["commit", "-q", "-m", "Made-up pages"]).unwrap();
    }
    (knowledge::set(&c.vault.id, "profile", "w").unwrap(), p)
}

fn init() -> Value {
    json!({"type": "system", "subtype": "init", "session_id": "made-up", "tools": ["Read", "Grep", "Glob", "Write"]})
}

fn read_call(id: &str, path: &str) -> Value {
    json!({"type": "assistant", "message": {"id": format!("msg_{id}"), "role": "assistant", "content": [{"type": "tool_use", "id": id, "name": "Read", "input": {"file_path": path}}], "stop_reason": null}, "parent_tool_use_id": null})
}

fn result() -> Value {
    json!({"type": "result", "subtype": "success", "is_error": false, "stop_reason": "end_turn", "duration_ms": 240000, "total_cost_usd": 0.31, "usage": {"input_tokens": 52000, "output_tokens": 2100}})
}

fn script(p: &ai::Profile, name: &str, lines: &[Value]) {
    std::fs::write(fake::file(p, name), lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
}

/// The fake writes `findings` as the check's findings.json when it plays this line.
fn findings(p: &ai::Profile, k: usize, findings: Value) -> Value {
    std::fs::write(fake::file(p, &format!("findings.{k}.json")), json!({ "findings": findings }).to_string()).unwrap();
    json!({"_findings": k})
}

/// Wait until the fake has taken its `n`th AI check script.
fn playing(p: &ai::Profile, n: usize) {
    let t = std::time::Instant::now();
    while std::fs::read_to_string(fake::file(p, "lint.n")).ok().and_then(|x| x.trim().parse::<usize>().ok()) != Some(n) {
        assert!(t.elapsed() < Duration::from_secs(10), "the fake didn't start its script {n}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Start an AI check and wait for its end.
fn ran(v: &Vault) -> String {
    let run = start(v).unwrap();
    assert!(wait(&run, Duration::from_secs(20)), "the check ended: {:#}", status(v, &run, 0).unwrap());
    run
}

/// The contradiction between the client page and the meeting note, its fix on the client page.
fn contradiction() -> Value {
    json!({"kind": "contradiction", "title": "Northwind go-live: 12 Dec or 9 Jan", "note": "The note of 5 Nov moved go-live; the client page was written before it.",
        "pages": [{"path": CLIENT, "line": 12, "quote": "Phase 2 go-live: 12 December, before the holiday freeze"}, {"path": NOTE, "line": 12, "quote": "Phase 2 go-live moves to 9 January"}],
        "fix": {"path": CLIENT, "old": OLD_LINE, "new": NEW_LINE}})
}

/// K16-T5's three: one true, one with an invented quote, one whose fix's `old` is twice on its page.
fn three() -> Value {
    json!([
        contradiction(),
        {"kind": "stale", "title": "Prices rise", "pages": [{"path": TOPIC, "quote": "Prices rise 40 percent next year"}]},
        {"kind": "missing_summary", "title": "Per-stream pricing has no summary", "pages": [{"path": TOPIC, "quote": "A price per active stream needs one way"}], "fix": {"path": TOPIC, "old": "per active stream", "new": "per stream"}},
    ])
}

fn ai_listed(v: &Vault) -> Vec<Value> {
    health::view(v, None).unwrap()["findings"].as_array().unwrap().iter().filter(|f| f["by"] == "ai").cloned().collect()
}

#[cfg(unix)]
#[test]
fn three_findings_one_kept_two_dropped() {
    // K16-T5, K16-06, K16-07, K16-T2's second half (the score after an AI check)
    let (_env, root) = paths::testenv::folders("kn-lint-kept");
    let _fake = fake::install(&root);
    let _clean = clean();
    let (v, p) = vault(&root, true);
    let before = health::view(&v, None).unwrap();
    assert_eq!(before["score"]["code_only"], true);
    assert!(before["ai"].is_null() && before["lint"]["running"].is_null() && before["lint"]["last"].is_null());
    script(&p, "lint.jsonl", &[init(), read_call("toolu_1", CLIENT), findings(&p, 1, three()), result()]);
    let run = ran(&v);
    let st = status(&v, &run, 0).unwrap();
    assert_eq!((st["state"].as_str(), st["kept"].as_u64(), st["dropped"].as_u64(), st["cost"].as_f64()), (Some("done"), Some(1), Some(2), Some(0.31)), "{st:#}");
    let ev = st["events"].as_array().unwrap();
    assert_eq!(ev[0]["kind"], "scope");
    assert!(ev.iter().any(|e| e["type"] == "tool" && e["name"] == "Read"), "{ev:#?}");
    assert_eq!(ev.last().unwrap()["text"], "Kept 1 finding · 2 dropped: their quotes aren't in the pages");
    let (kept, dropped) = kept(&v, &run).unwrap();
    assert_eq!((kept.len(), dropped), (1, 2), "1 kept, 2 dropped and counted");
    let f = &kept[0];
    assert_eq!((f.kind.as_str(), f.by, f.group, f.fixable, f.fix_all), ("contradiction", By::Ai, Group::Needs, true, false), "an AI fix is never in Fix all");
    assert_eq!(f.title, "Northwind go-live: 12 Dec or 9 Jan");
    assert_eq!(f.pages, [(CLIENT.to_string(), Some(12)), (NOTE.to_string(), Some(11))], "the lines Echo found the quotes on (not the line the check gave)");
    assert!(f.key.starts_with(&format!("contradiction:{CLIENT},{NOTE}:#")), "{}", f.key);
    // Health lists it and counts AI kinds now (§5.7): Freshness counted, no longer "code checks only".
    let h = health::view(&v, None).unwrap();
    let listed: Vec<&Value> = h["findings"].as_array().unwrap().iter().filter(|x| x["by"] == "ai").collect();
    assert_eq!(listed.iter().map(|x| x["key"].as_str().unwrap()).collect::<Vec<_>>(), [f.key.as_str()]);
    assert_eq!(h["score"]["code_only"], false);
    let cats: Vec<&str> = h["score"]["categories"].as_array().unwrap().iter().map(|c| c[0].as_str().unwrap()).collect();
    assert_eq!(cats, ["Links", "Index", "Properties", "Consistency", "Freshness"]);
    assert!(h["score"]["categories"][3][1].as_f64().unwrap() < 25.0, "the contradiction's pages count in Consistency: {}", h["score"]);
    assert_eq!((h["ai"]["run"].as_str(), h["ai"]["kept"].as_u64(), h["ai"]["dropped"].as_u64(), h["ai"]["cost"].as_f64()), (Some(run.as_str()), Some(1), Some(2), Some(0.31)));
    assert_eq!((h["lint"]["running"].is_null(), h["lint"]["last"]["id"].as_str(), h["lint"]["last"]["state"].as_str()), (true, Some(run.as_str()), Some("done")));
    // Its detail (K16-07): both quotes with their lines and dates (older · newer is the page's to draw), the fix's diff.
    let d = health::detail(&v, &f.key, None).unwrap();
    assert_eq!(d["ai"]["quotes"], json!([
        {"path": CLIENT, "line": 12, "text": "Phase 2 go-live: 12 December, before the holiday freeze", "date": "2026-10-02"},
        {"path": NOTE, "line": 11, "text": "Phase 2 go-live moves to 9 January", "date": "2026-11-05"},
    ]));
    assert_eq!(d["ai"]["note"], "The note of 5 Nov moved go-live; the client page was written before it.");
    assert_eq!(d["choices"], json!([{"choice": "ai", "label": "The change the AI check suggests"}]));
    assert_eq!((d["fix"]["choice"].as_str(), d["fix"]["path"].as_str(), d["fix"]["plus"].as_u64(), d["fix"]["minus"].as_u64()), (Some("ai"), Some(CLIENT), Some(1), Some(1)));
    let lines: Vec<(String, String)> = d["fix"]["hunks"][0].as_array().unwrap().iter().map(|l| (l[0].as_str().unwrap().into(), l[1].as_str().unwrap().into())).collect();
    assert!(lines.contains(&("-".into(), OLD_LINE.into())) && lines.contains(&("+".into(), NEW_LINE.into())), "{lines:?}");
    // *Fix* through Health's own flow: written only as shown (base, sha), one commit.
    let fx = health::fix(&v, &f.key, None).unwrap();
    assert_eq!((fx.base.as_str(), page_sha(&fx.text)), (d["fix"]["base"].as_str().unwrap(), d["fix"]["sha"].as_str().unwrap().to_string()));
    let a = health::apply(&v, &[fx]).unwrap();
    assert_eq!((a.written.as_slice(), a.committed.as_slice()), ([CLIENT.to_string()].as_slice(), [CLIENT.to_string()].as_slice()), "{a:?}");
    assert!(read(&v.root, CLIENT).contains(NEW_LINE));
    assert_eq!(git::out(&v.root, &["log", "-1", "--format=%s"]).unwrap().trim(), format!("Echo: Health fix in {CLIENT}"));
    assert!(ai_listed(&v).is_empty(), "fixed: its quote and its fix's text aren't on the page any more");
}

#[test]
fn pages_in_scope_are_capped_at_300() {
    // K16-05: all pages when the vault has ≤ 300; else those changed in the last 30 days and the pages they link to,
    // 300 at most (K8-05's scope: README files and the index aren't counted; templates aren't pages).
    let (_env, root) = paths::testenv::folders("kn-lint-scope");
    let t = fixture::twin(&root.join("twin"));
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    let now = chrono::Local::now().timestamp_millis();
    const DAY: i64 = 86_400_000;
    let index = |pages: Vec<(String, String, i64)>| {
        let mut pages: Vec<(String, String, i64)> = pages;
        pages.push(("CATALOG.md".into(), "# Index\n\n[[new-0]]\n".into(), now));
        pages.push(("notes/README.md".into(), "# Notes\n".into(), now));
        Index { pages: pages.into_iter().map(|(p, text, mtime)| (p.clone(), scan::page(&p, &text, text.len() as u64, mtime))).collect(), rev: "made-up".into(), root: "knowledge".into() }
    };
    let old = |n: usize| (format!("notes/old-{n}.md"), format!("# Old {n}\n"), now - 90 * DAY - n as i64);
    // ≤ 300: every page Health judges, however old.
    let (pages, of) = pages_in_scope(&v, &index((0..250).map(old).collect()), now);
    assert_eq!((pages.len(), of), (250, 250));
    assert!(pages.iter().all(|p| p.starts_with("knowledge/notes/old-")), "vault-relative, no README or index: {:?}", &pages[..3]);
    // Over 300: the 20 changed lately, newest first, then the 21 they link to — and the index, linked, isn't one.
    let new = |n: usize, links: String| (format!("notes/new-{n}.md"), format!("# New {n}\n\n{links}\n"), now - DAY - n as i64 * 60_000);
    let mut ps: Vec<_> = (0..300).map(old).collect();
    ps.extend((0..20).map(|n| new(n, format!("See [[old-{n}]], [[old-{}]] and [[CATALOG]].", n + 1))));
    let (pages, of) = pages_in_scope(&v, &index(ps), now);
    assert_eq!(of, 320);
    assert_eq!(pages.len(), 41, "{pages:?}");
    assert_eq!(&pages[..2], ["knowledge/notes/new-0.md", "knowledge/notes/new-1.md"], "the newest first");
    assert_eq!(&pages[20..23], ["knowledge/notes/old-0.md", "knowledge/notes/old-1.md", "knowledge/notes/old-2.md"]);
    assert!(!pages.iter().any(|p| p.ends_with("CATALOG.md") || p.ends_with("README.md")));
    // More than 300 of those: capped, the changed ones first.
    let mut ps: Vec<_> = (0..200).map(old).collect();
    ps.extend((0..250).map(|n| new(n, format!("See [[old-{}]].", n % 200))));
    let (pages, of) = pages_in_scope(&v, &index(ps), now);
    assert_eq!((pages.len(), of), (300, 450));
    assert!(pages[..250].iter().all(|p| p.contains("/new-")) && pages[250..].iter().all(|p| p.contains("/old-")));
    // Nothing changed lately in a big vault: nothing to read.
    assert_eq!(pages_in_scope(&v, &index((0..301).map(old).collect()), now), (vec![], 301));
}

#[cfg(unix)]
#[test]
fn lint_never_writes_the_vault() {
    // X-T8, K16-05: only its findings file — the run in the vault itself, with §5.4's headless flags, read-only tools, and
    // the guard's `lint` scope in its environment and its own settings; Echo's files beside it, outside the vault.
    let (_env, root) = paths::testenv::folders("kn-lint-never");
    let _fake = fake::install(&root);
    let _clean = clean();
    let (v, p) = vault(&root, true);
    let before = fixture::snapshot(&v.root);
    script(&p, "lint.jsonl", &[init(), read_call("toolu_1", TOPIC), findings(&p, 1, three()), result()]);
    let run = ran(&v);
    health::view(&v, None).unwrap();
    health::detail(&v, &kept(&v, &run).unwrap().0[0].key, None).unwrap();
    assert_eq!(fixture::snapshot(&v.root), before, "nothing in the vault changed, its .git/ included");
    assert_eq!(git::out(&v.root, &["status", "--porcelain"]).unwrap(), "");
    let dir = dunce::canonicalize(knowledge::cache_dir(&v.id).join("lint").join(&run)).unwrap();
    let mut files: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert_eq!(files, ["findings.json", "kept.json", "log.jsonl", "prompt.md", "run.json", "settings.json"]);
    // The command line: §5.4's headless flags, read-only tools and one Write, never Bash, Edit or a skill.
    let argv: Vec<String> = std::fs::read_to_string(fake::file(&p, "lint.argv")).unwrap().lines().map(str::to_owned).collect();
    let value_of = |f: &str| argv.iter().position(|a| a == f).and_then(|i| argv.get(i + 1)).cloned();
    for flag in ["-p", "--verbose", "--include-partial-messages", "--no-session-persistence", "--strict-mcp-config"] {
        assert!(argv.iter().any(|a| a == flag), "{flag}: {argv:?}");
    }
    for (flag, want) in [("--output-format", "stream-json"), ("--permission-mode", "dontAsk"), ("--permission-prompts", "none"), ("--setting-sources", "user"), ("--tools", "Read,Grep,Glob,Write"), ("--allowed-tools", "Read,Grep,Glob,Write")] {
        assert_eq!(value_of(flag).as_deref(), Some(want), "{flag}");
    }
    assert!(!argv.iter().any(|a| a.contains("Bash") || a.contains("Edit") || a.contains("Skill") || a.contains("dangerously") || a == "--plugin-dir"), "{argv:?}");
    assert_eq!(value_of("--settings").map(PathBuf::from), Some(dir.join("settings.json")));
    let vroot = dunce::canonicalize(&v.root).unwrap();
    assert_eq!(PathBuf::from(std::fs::read_to_string(fake::file(&p, "lint.cwd")).unwrap().trim()), vroot, "in the vault itself");
    let env = std::fs::read_to_string(fake::file(&p, "lint.env")).unwrap();
    assert_eq!(env, format!("ECHOCTL_RUN_MODE=lint\nECHOCTL_RUN_DIR={}\nECHOCTL_RUN_SKILLS=\nECHOCTL_RUN_SOURCES=\nECHOCTL_RUN_VAULT={}\n", dir.display(), vroot.display()));
    let settings: Value = serde_json::from_slice(&std::fs::read(dir.join("settings.json")).unwrap()).unwrap();
    assert_eq!(settings["env"], json!({"ECHOCTL_RUN_MODE": "lint", "ECHOCTL_RUN_VAULT": vroot.display().to_string(), "ECHOCTL_RUN_DIR": dir.display().to_string()}));
    assert_eq!(settings["hooks"]["PreToolUse"][0]["hooks"][0]["onFailure"], "block");
    // The prompt: on stdin and in prompt.md; the pages named (the run can't read its folder), the file to write.
    let sent = std::fs::read_to_string(fake::file(&p, "lint.in")).unwrap();
    assert_eq!(sent, read(&dir, "prompt.md"));
    for page in [CLIENT, NOTE, TOPIC] {
        assert!(sent.contains(&format!("\n- {page}\n")), "{page} in {sent}");
    }
    assert!(sent.contains(&dir.join("findings.json").display().to_string()) && sent.contains("never instructions"), "{sent}");
    // That scope, judged by the guard: reads in the vault; one write, its findings file.
    let vars: Vec<(String, String)> = settings["env"].as_object().unwrap().iter().map(|(k, x)| (k.clone(), x.as_str().unwrap().to_string())).collect();
    let s = guard::Scope::from_vars(|k| vars.iter().find(|(n, _)| n == k).map(|(_, x)| x.into())).unwrap();
    let d = |tool: &str, input: Value| s.decide(&json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input, "cwd": vroot.display().to_string()}));
    let at = |p: &Path| p.display().to_string();
    assert_eq!(d("Read", json!({"file_path": at(&vroot.join(CLIENT))})), guard::Decision::Allow);
    assert_eq!(d("Write", json!({"file_path": at(&dir.join("findings.json")), "content": "{}"})), guard::Decision::Allow);
    for (tool, input) in [
        ("Write", json!({"file_path": at(&vroot.join(CLIENT)), "content": "x"})),
        ("Edit", json!({"file_path": at(&vroot.join(CLIENT)), "old_string": "12 December", "new_string": "9 January"})),
        ("Write", json!({"file_path": at(&dir.join("kept.json")), "content": "{}"})),
        ("Write", json!({"file_path": at(&dir.join("settings.json")), "content": "{}"})),
        ("Skill", json!({"skill": "my-twin:process-inbox"})),
        ("Bash", json!({"command": "git commit -am x"})),
    ] {
        assert!(matches!(d(tool, input.clone()), guard::Decision::Deny(_)), "{tool} {input}");
    }
}

#[cfg(unix)]
#[test]
fn a_kept_finding_goes_when_its_quoted_line_changes() {
    // Ruling R46: kept findings are checked again whenever Health is computed.
    let (_env, root) = paths::testenv::folders("kn-lint-recheck");
    let _fake = fake::install(&root);
    let _clean = clean();
    let (v, p) = vault(&root, false);
    let stale = json!({"kind": "stale", "title": "Pricing counted per active stream", "pages": [{"path": TOPIC, "quote": "Every client counts per active stream differently today"}]});
    script(&p, "lint.jsonl", &[init(), findings(&p, 1, json!([contradiction(), stale])), result()]);
    ran(&v);
    let keys = |v: &Vault| ai_listed(v).iter().map(|f| f["kind"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(keys(&v), ["contradiction", "stale"]);
    let freshness = |v: &Vault| health::view(v, None).unwrap()["score"]["categories"][4][1].as_f64().unwrap();
    assert!(freshness(&v) < 15.0);
    // An edit elsewhere on the page: still listed, its line followed.
    fixture::write(&v.root, TOPIC, &TOPIC_TEXT.replace("# Per-stream pricing\n", "# Per-stream pricing\n\nA made-up note on top.\n"));
    let f = ai_listed(&v).into_iter().find(|f| f["kind"] == "stale").unwrap();
    assert_eq!(f["pages"], json!([[TOPIC, 12]]));
    // Its quoted line changed: no longer listed, no longer counted.
    fixture::write(&v.root, TOPIC, &TOPIC_TEXT.replace("Every client counts per active stream differently today.", "Every client now counts streams the same way."));
    assert_eq!(keys(&v), ["contradiction"]);
    assert_eq!(freshness(&v), 15.0);
    // A fix whose text is on the page twice now: not listed either.
    fixture::write(&v.root, CLIENT, &format!("{CLIENT_TEXT}{OLD_LINE}\n"));
    assert!(keys(&v).is_empty());
    assert!(health::detail(&v, "contradiction:x", None).is_err());
}

#[cfg(unix)]
#[test]
fn a_finding_naming_a_path_outside_the_vault_is_dropped() {
    // K16-06, untrusted-text R4: findings.json's paths are the run's words — outside the vault, not a page, not there.
    let (_env, root) = paths::testenv::folders("kn-lint-paths");
    let _fake = fake::install(&root);
    let _clean = clean();
    let (v, p) = vault(&root, false);
    fixture::write(&root, "outside.md", "# Outside\n\nPhase 2 go-live moves to 9 January.\n");
    fixture::write(&v.root, "raw_inbox/memo.md", "# A source\n\nPhase 2 go-live moves to 9 January.\n");
    std::os::unix::fs::symlink(root.join("outside.md"), v.root.join("knowledge/notes/linked.md")).unwrap();
    let quote = "Phase 2 go-live moves to 9 January";
    let one = |path: String| json!({"kind": "stale", "title": "Go-live", "pages": [{"path": NOTE, "quote": quote}, {"path": path, "quote": quote}]});
    let outside = root.join("outside.md").display().to_string();
    let bad = json!([
        one("../outside.md".into()), one(outside), one(format!("knowledge/../../{}", "outside.md")), one("raw_inbox/memo.md".into()),
        one("knowledge/notes/gone.md".into()), one("knowledge/notes/linked.md".into()), one("knowledge\\notes\\x.md".into()),
        {"kind": "stale", "pages": [{"path": NOTE, "quote": quote}], "fix": {"path": "../outside.md", "old": "Phase 2", "new": "Phase 3"}},
        {"kind": "made_up_kind", "pages": [{"path": NOTE, "quote": quote}]},
        {"kind": "stale", "pages": []},
        "not a finding",
        {"kind": "stale", "title": "Go-live", "pages": [{"path": format!("./{NOTE}"), "quote": quote}]},
    ]);
    script(&p, "lint.jsonl", &[init(), findings(&p, 1, bad), result()]);
    let run = ran(&v);
    let (kept, dropped) = kept(&v, &run).unwrap();
    assert_eq!(dropped, 11);
    assert_eq!(kept.iter().map(|f| f.pages.clone()).collect::<Vec<_>>(), [vec![(NOTE.to_string(), Some(11))]], "a `./` path names a page all the same");
    assert_eq!(read(&root, "outside.md"), "# Outside\n\nPhase 2 go-live moves to 9 January.\n");
}

#[cfg(unix)]
#[test]
fn one_check_at_a_time_it_stops_and_times_out() {
    // R45 (one AI check per vault, its own lock; an ingest run may go meanwhile), Stop, the 20 minutes, no findings file.
    let (_env, root) = paths::testenv::folders("kn-lint-one");
    let _fake = fake::install(&root);
    let _clean = clean();
    let (v, p) = vault(&root, false);
    script(&p, "lint.1.jsonl", &[init(), json!({"_hang": true})]);
    let first = start(&v).unwrap();
    playing(&p, 1);
    assert_eq!(live(), std::slice::from_ref(&first));
    assert_eq!(health::view(&v, None).unwrap()["lint"]["running"], first.as_str());
    assert!(start(&v).unwrap_err().to_string().contains("An AI check of this vault is going"));
    {
        // Another Echo — the CLI — holding the lock refuses this one's too.
        let _cli = crate::app::FileLock::try_take(&knowledge::cache_dir(&v.id).join("lint").join("run.lock")).unwrap();
        assert!(_cli.is_none(), "held by the check going");
    }
    // An ingest run alongside: allowed.
    fixture::write(&v.root, "raw_inbox/a.md", "# A made-up source\n");
    script(&p, "ingest.jsonl", &[init(), result()]);
    let ing = ingest::start(&v, &[]).unwrap();
    assert!(ingest::wait(&ing.run, Duration::from_secs(20)));
    stop(&v, &first).unwrap();
    assert!(wait(&first, Duration::from_secs(15)));
    let st = status(&v, &first, 0).unwrap();
    assert_eq!((st["state"].as_str(), st["reason"].as_str()), (Some("stopped"), Some("Stopped")));
    assert!(health::load(&v).ai.is_none(), "a check that didn't end isn't the last AI check");
    assert!(stop(&v, &first).is_err(), "no longer going");
    crate::paths::testenv::eventually("the lease is released", || ai::lease(&p).is_ok());
    // Its 20 minutes.
    script(&p, "lint.2.jsonl", &[init(), json!({"_hang": true})]);
    let second = start(&v).unwrap();
    playing(&p, 2);
    ingest::skew(Duration::from_secs(21 * 60));
    assert!(wait(&second, Duration::from_secs(15)), "stopped at its 20 minutes");
    let st = status(&v, &second, 0).unwrap();
    assert_eq!((st["state"].as_str(), st["reason"].as_str()), (Some("failed"), Some("It ran for 20 minutes without finishing")));
    ingest::reset();
    // One that writes no findings: failed, saying so.
    script(&p, "lint.3.jsonl", &[init(), result()]);
    let third = ran(&v);
    let st = status(&v, &third, 0).unwrap();
    assert_eq!((st["state"].as_str(), st["reason"].as_str()), (Some("failed"), Some("It wrote no findings")));
    assert_eq!(health::view(&v, None).unwrap()["score"]["code_only"], true, "still code checks only");
}

#[test]
fn a_check_its_echo_left_running_failed() {
    // A check `run.json` calls running that nothing runs (its Echo quit, crashed, the CLI stopped).
    let (_env, root) = paths::testenv::folders("kn-lint-left");
    let t = fixture::twin(&root.join("twin"));
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    let dir = knowledge::cache_dir(&v.id).join("lint").join("2026-11-01_090000");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("run.json"), json!({"id": "2026-11-01_090000", "at": "2026-11-01T09:00:00+01:00", "state": "running", "profile": "w"}).to_string()).unwrap();
    let st = status(&v, "2026-11-01_090000", 0).unwrap();
    assert_eq!((st["state"].as_str(), st["reason"].as_str()), (Some("failed"), Some("It stopped when Echo did")));
    assert!(status(&v, "../ingest", 0).is_err() && status(&v, "2026-11-02_090000", 0).is_err());
}

fn page_sha(text: &str) -> String {
    crate::knowledge::page::sha(text.as_bytes())
}

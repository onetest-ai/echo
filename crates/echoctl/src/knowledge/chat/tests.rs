use super::*;
fn args(v: &Vault, p: &ai::Profile, mode: Mode, session: &Session, settings: &Path) -> Result<Vec<String>> {
    super::args(v, p, mode, session, settings, &skills::Catalog::load(v, p)?)
}

fn scope(v: &Vault, p: &ai::Profile, mode: Mode) -> Vec<(String, String)> {
    super::scope(v, mode, &skills::Catalog::load(v, p).unwrap())
}
use crate::ai::fake;
use crate::knowledge::{fixture, AddOpts};
use std::path::PathBuf;
use std::time::Instant;

const ID: &str = "0f6ac8d0-1111-4222-8333-444455556666";
const ASK: &str = "Read,Grep,Glob,Skill,Agent";

/// Stops this test's conversations when it ends (declared after the fake and the folders: dropped first).
pub(super) struct Clean;

impl Drop for Clean {
    fn drop(&mut self) {
        reset();
    }
}

pub(super) fn clean() -> Clean {
    reset();
    Clean
}

pub(super) fn vault(root: &Path, profile: &str) -> Vault {
    let t = fixture::twin(&root.join("twin"));
    crate::knowledge::add(&t, AddOpts { name: Some("Twin".into()), profile: Some(profile.into()), ..Default::default() }).unwrap()
}

fn profile(id: &str, kind: &str) -> ai::Profile {
    serde_json::from_value(json!({"id": id, "name": id, "kind": kind})).unwrap()
}

/// A managed profile signed in, answered by the fake.
pub(super) fn signed_in() -> ai::Profile {
    fake::profile("w", None, &fake::signed_in("pat@example.com"))
}

fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

pub(super) fn file(p: &ai::Profile, name: &str) -> String {
    std::fs::read_to_string(fake::file(p, name)).unwrap_or_default()
}

fn argv(p: &ai::Profile) -> Vec<String> {
    file(p, "argv").lines().map(str::to_owned).collect()
}

pub(super) fn script(p: &ai::Profile, name: &str, lines: &str) {
    std::fs::write(fake::file(p, name), lines).unwrap();
}

pub(super) fn of_type<'a>(ev: &'a [Value], t: &str) -> Vec<&'a Value> {
    ev.iter().filter(|e| e["type"] == t).collect()
}

fn gone(pid: &str) -> bool {
    unsafe { libc::kill(pid.trim().parse().unwrap(), 0) != 0 }
}

pub(super) fn until(what: &str, f: impl Fn() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(10), "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(30));
    }
}

#[test]
fn conversation_args_per_profile() {
    let (_env, root) = crate::paths::testenv::folders("chat-args");
    let v = vault(&root, "system");
    // The user's "no permission prompts" is for their own folder session, never a conversation (D24).
    let mut cfg = crate::config::Config::load().unwrap();
    cfg.folder_skip_permissions = true;
    cfg.save().unwrap();
    let settings = root.join("settings.json");
    let first = "What did Northwind Retail decide about pricing and the rollout plan?";
    let system = profile("system", "system");
    let a = args(&v, &system, Mode::Ask, &Session::New { id: ID.into(), first: first.into() }, &settings).unwrap();
    for (flag, want) in [
        ("--input-format", "stream-json"),
        ("--output-format", "stream-json"),
        ("--permission-mode", "default"),
        ("--permission-prompts", "host"),
        ("--permission-prompt-tool", "stdio"),
        ("--tools", ASK),
        ("--allowed-tools", ASK),
        ("--settings", settings.to_str().unwrap()),
        ("--session-id", ID),
        ("--name", "Twin: What did Northwind Retail decide about p"),
    ] {
        assert_eq!(value_of(&a, flag), Some(want), "{flag} in {a:?}");
    }
    for flag in ["-p", "--verbose", "--include-partial-messages", "--prompt-suggestions", "--strict-mcp-config"] {
        assert!(a.iter().any(|x| x == flag), "{flag} in {a:?}");
    }
    assert!(!a.iter().any(|x| x == "--dangerously-skip-permissions" || x == "--resume"), "{a:?}");
    assert!(!a.iter().any(|x| x == "--plugin-dir" || x == "--setting-sources"), "as configured, Claude Code loads the vault itself: {a:?}");
    // A managed profile: the vault's skills as a plugin (its own settings only come from `ai::build_command`).
    let managed = profile("work", "claude");
    let c = args(&v, &managed, Mode::Curate, &Session::Resume(ID.into()), &settings).unwrap();
    assert_eq!(value_of(&c, "--resume"), Some(ID));
    assert!(!c.iter().any(|x| x == "--session-id" || x == "--name" || x == "--dangerously-skip-permissions"), "{c:?}");
    assert!(value_of(&c, "--plugin-dir").is_some_and(|d| Path::new(d).join(".claude-plugin/plugin.json").is_file()), "{c:?}");
    assert_eq!(value_of(&c, "--tools"), Some("Read,Grep,Glob,Skill,Agent,Edit,Write"));
    assert_eq!(value_of(&c, "--allowed-tools"), Some(ASK), "Curate's edits are asked about");
    let r = args(&v, &managed, Mode::Research, &Session::Resume(ID.into()), &settings).unwrap();
    assert_eq!(value_of(&r, "--tools"), Some("Read,Grep,Glob,Skill,Agent,WebSearch,WebFetch"));
    assert_eq!(value_of(&r, "--allowed-tools"), Some("Read,Grep,Glob,Skill,Agent,WebSearch"), "each fetched page is asked about");
    for x in [&a, &c, &r] {
        let tools = value_of(x, "--tools").unwrap();
        assert!(!tools.split(',').any(|t| t == "Bash" || t == "NotebookEdit" || t.starts_with("mcp__")), "{tools}");
    }
}

#[test]
fn settings_env_carries_the_scope() {
    let (_env, root) = crate::paths::testenv::folders("chat-scope");
    let v = vault(&root, "system");
    let env = |p: &ai::Profile, mode: Mode, v: &Vault| guard::settings_for(&scope(v, p, mode), Path::new("/opt/echo/echoctl"))["env"].clone();
    let e = env(&profile("work", "claude"), Mode::Curate, &v);
    assert_eq!(e[guard::RUN_MODE], "vault");
    assert_eq!(e[guard::RUN_VAULT], v.root.to_str().unwrap());
    assert_eq!(e[guard::RUN_CHAT], "curate");
    assert_eq!(e[guard::RUN_SOURCES], "raw_inbox");
    assert_eq!(e[guard::RUN_SKILLS], "twin:c\ntwin:s", "a managed profile's skills come from the vault's plugin");
    assert_eq!(env(&profile("system", "system"), Mode::Ask, &v)[guard::RUN_SKILLS], "c\ns", "as configured, by their own names");
    // The vault's own sources folder, or raw_inbox when it has none (ruling R11).
    let other = root.join("notes");
    fixture::write(&other, "inbox/call.md", "# A call\n");
    fixture::write(&other, "a.md", "# A\n");
    let o = crate::knowledge::add(&other, AddOpts { name: Some("Notes".into()), ..Default::default() }).unwrap();
    assert_eq!(env(&profile("system", "system"), Mode::Research, &o)[guard::RUN_SOURCES], "inbox");
    let bare = root.join("bare");
    fixture::write(&bare, "a.md", "# A\n");
    let b = crate::knowledge::add(&bare, AddOpts { name: Some("Bare".into()), ..Default::default() }).unwrap();
    let e = env(&profile("system", "system"), Mode::Ask, &b);
    assert_eq!(e[guard::RUN_SOURCES], "raw_inbox");
    assert_eq!(e[guard::RUN_SKILLS], "");
    assert_eq!(e[guard::RUN_CHAT], "ask");
}

#[test]
fn conversations_explain_which_skills_the_guard_allows() {
    let (_env, root) = crate::paths::testenv::folders("chat-skill-prompt");
    let v = vault(&root, "system");
    for p in [profile("system", "system"), profile("work", "claude")] {
        for mode in [Mode::Ask, Mode::Curate, Mode::Research] {
            let a = args(&v, &p, mode, &Session::Resume(ID.into()), &root.join("settings.json")).unwrap();
            assert_eq!(a.iter().filter(|x| *x == "--append-system-prompt").count(), 1);
            let prompt = value_of(&a, "--append-system-prompt").expect("the conversation's skill limits");
            let names = serde_json::to_string(&skills::Catalog::load(&v, &p).unwrap().names()).unwrap();
            assert!(prompt.contains(&names), "{prompt}");
            assert!(prompt.contains("User-level skills and commands are supported directly"), "{prompt}");
            assert!(prompt.contains("Read, Grep and Glob"), "a question needs no skill: {prompt}");
            if p.kind.managed() {
                assert!(prompt.contains("AGENTS.md"), "keep the managed profile's vault rules: {prompt}");
            }
        }
    }
    std::fs::remove_file(v.root.join(".claude/skills/s/SKILL.md")).unwrap();
    std::fs::remove_file(v.root.join(".claude/commands/c.md")).unwrap();
    let a = args(&v, &profile("system", "system"), Mode::Ask, &Session::Resume(ID.into()), &root.join("settings.json")).unwrap();
    assert!(value_of(&a, "--append-system-prompt").unwrap().contains("user skills and this vault: []"));
}

#[test]
fn a_new_process_uses_current_vault_skills_for_its_prompt_plugin_and_guard() {
    let (_env, root) = crate::paths::testenv::folders("chat-current-skills");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    // The registry still has s and c: change the vault without asking Echo to detect it again.
    std::fs::remove_file(v.root.join(".claude/skills/s/SKILL.md")).unwrap();
    fixture::write(&v.root, ".claude/skills/twin/SKILL.md", "---\nname: twin\ndescription: Answer from this vault.\n---\nRead knowledge/CATALOG.md.\n");
    script(&p, "chat.jsonl", &turn("From the vault."));
    let id = send(&v.id, None, "What do I know about the flywheel?", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    let a = argv(&p);
    let settings: Value = serde_json::from_slice(&std::fs::read(value_of(&a, "--settings").unwrap()).unwrap()).unwrap();
    let env = settings["env"].as_object().unwrap();
    assert_eq!(env[guard::RUN_SKILLS], "twin:c\ntwin:twin");
    let prompt = value_of(&a, "--append-system-prompt").unwrap();
    assert!(prompt.contains(r#"["twin:c","twin:twin"]"#), "{prompt}");
    let plugin = Path::new(value_of(&a, "--plugin-dir").unwrap());
    assert!(plugin.join("skills/twin/SKILL.md").is_file());
    assert!(!plugin.join("skills/s/SKILL.md").exists());
    let guard = guard::Scope::from_vars(|k| env.get(k).and_then(Value::as_str).map(Into::into)).unwrap();
    let skill = |s: &str| json!({"tool_name": "Skill", "tool_input": {"skill": s}});
    assert!(guard.judge(&skill("twin:twin")).is_ok());
    for s in ["twin:s", "personal-twin", "twin"] {
        assert!(guard.judge(&skill(s)).is_err(), "{s}");
    }
    assert!(guard.judge(&json!({"tool_name": "Read", "tool_input": {"file_path": v.root.join("knowledge/CATALOG.md")}})).is_ok());
    assert!(guard.judge(&json!({"tool_name": "Bash", "tool_input": {"command": "pwd"}})).is_err());
}

#[test]
fn personal_skills_work_without_copying_and_keep_the_conversations_boundaries() {
    let (_env, root) = crate::paths::testenv::folders("chat-personal-skill");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let user = p.config_folder();
    fixture::write(&user, "commands/personal-twin.md", "Read the vault's catalog to answer $ARGUMENTS.");
    fixture::write(&user, "commands/other.md", "Another installed command.");
    fixture::write(&user, "skills/review/SKILL.md", "---\nname: review\n---\nRead references.md.");
    fixture::write(&user, "skills/review/references.md", "Use evidence from the vault.");
    fixture::write(&user, "credentials.json", "made-up token");
    fixture::write(&v.root, ".claude/commands/tools/review.md", "---\nname: ignored\n---\nReview a page.");
    let before = fixture::snapshot(&v.root);
    script(&p, "chat.jsonl", &turn("The flywheel is described in the catalog."));
    let id = send(&v.id, None, "/personal-twin explain the flywheel", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    let a = argv(&p);
    let settings: Value = serde_json::from_slice(&std::fs::read(value_of(&a, "--settings").unwrap()).unwrap()).unwrap();
    let env = settings["env"].as_object().unwrap();
    let plugin = Path::new(value_of(&a, "--plugin-dir").unwrap());
    assert!(plugin.join("commands/tools/review.md").is_file(), "nested command names match the generated plugin");
    let g = guard::Scope::from_vars(|k| env.get(k).and_then(Value::as_str).map(Into::into)).unwrap();
    let tool = |name: &str, input: Value| json!({"tool_name":name,"tool_input":input});
    assert!(g.judge(&tool("Skill", json!({"skill":"personal-twin"}))).is_ok());
    assert!(g.judge(&tool("Skill", json!({"skill":"twin:tools:review"}))).is_ok());
    for f in ["commands/personal-twin.md", "skills/review/references.md"] {
        assert!(g.judge(&tool("Read", json!({"file_path":user.join(f)}))).is_ok(), "{f}");
    }
    for f in [user.join("credentials.json"), root.join("outside.md")] {
        assert!(g.judge(&tool("Read", json!({"file_path":f}))).is_err(), "{}", f.display());
    }
    assert!(g.judge(&tool("Grep", json!({"path":user,"pattern":"token"}))).is_err(), "the config folder isn't a skill");
    assert!(g.judge(&tool("Write", json!({"file_path":user.join("skills/review/new.md"),"content":"x"}))).is_err());
    assert!(g.judge(&tool("Bash", json!({"command":"pwd"}))).is_err());
    assert!(g.judge(&tool("Write", json!({"file_path":v.root.join("knowledge/new.md"),"content":"x"}))).is_err(), "Ask stays read only");
    let sent: Value = serde_json::from_str(file(&p, "chat.in.jsonl").lines().next().unwrap()).unwrap();
    assert_eq!(sent["message"]["content"][0]["text"], "/personal-twin explain the flywheel");
    assert_eq!(fixture::snapshot(&v.root), before, "no skill was copied into the vault");
    assert!(send(&v.id, Some(&id), "/unknown hello", Mode::Ask, &[]).is_err());
    // A newly installed user command becomes usable in this conversation: restart with the same session id.
    fixture::write(&user, "commands/new-skill.md", "Answer from the vault.");
    assert_eq!(send(&v.id, Some(&id), "/new-skill hello", Mode::Ask, &[]).unwrap(), id);
    events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
    assert_eq!(file(&p, "pids").lines().count(), 2);
    assert_eq!(value_of(&argv(&p), "--resume"), Some(id.as_str()));
}

#[test]
fn a_conversation_streams_and_ends_its_turn() {
    let (_env, root) = crate::paths::testenv::folders("chat-turn");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let mut cfg = crate::config::Config::load().unwrap();
    cfg.folder_skip_permissions = true;
    cfg.save().unwrap();
    script(&p, "chat.1.jsonl", &turn("Northwind Retail fixes forward."));
    let id = send(&v.id, None, "What did we decide with Northwind Retail?", Mode::Ask, &[Context::Page("knowledge/clients/acme.md".into())]).unwrap();
    let ev = turn_events(&v.id, &id);
    assert_eq!(ev[0]["type"], "user");
    assert_eq!(ev[0]["text"], "What did we decide with Northwind Retail?");
    assert_eq!(ev[0]["context"], json!([{"page": "knowledge/clients/acme.md"}]));
    let texts = of_type(&ev, "text");
    assert!(texts.last().is_some_and(|t| t["done"] == true && t["html"].as_str().unwrap().contains("Northwind Retail fixes forward.")), "{ev:#?}");
    assert_eq!(of_type(&ev, "result").len(), 1);
    assert!(ev.iter().enumerate().all(|(i, e)| e["n"] == i as u64), "numbered in order: {ev:#?}");
    // §5.4's command, in the vault, on the managed profile's own settings, with the vault's skills.
    let a = argv(&p);
    assert_eq!(value_of(&a, "--session-id"), Some(id.as_str()));
    assert_eq!(value_of(&a, "--setting-sources"), Some("user"));
    assert!(value_of(&a, "--plugin-dir").is_some());
    assert!(!a.iter().any(|x| x == "--dangerously-skip-permissions"), "never, whatever the folder session's switch says");
    assert_eq!(file(&p, "cwd").trim(), v.root.to_str().unwrap());
    // The scope in the process's environment and in its own settings file (D40).
    let env = file(&p, "env");
    for want in ["ECHOCTL_RUN_MODE=vault".to_string(), "ECHOCTL_RUN_CHAT=ask".into(), format!("ECHOCTL_RUN_VAULT={}", v.root.display()), "ECHOCTL_RUN_SOURCES=raw_inbox".into(), "ECHOCTL_RUN_SKILLS=twin:c".into()] {
        assert!(env.lines().any(|l| l == want), "{want} in {env}");
    }
    let settings: PathBuf = value_of(&a, "--settings").unwrap().into();
    assert_eq!(settings, crate::knowledge::cache_dir(&v.id).join("chat").join(&id).join("settings.json"));
    let s: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
    assert_eq!(s["env"][guard::RUN_MODE], "vault");
    assert_eq!(s["env"][guard::RUN_CHAT], "ask");
    assert!(s["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap().ends_with("run-guard"));
    // What Claude Code read: one stream-json user message naming the page.
    let sent: Value = serde_json::from_str(file(&p, "chat.in.jsonl").lines().next().unwrap()).unwrap();
    assert_eq!(sent["type"], "user");
    let text = sent["message"]["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("What did we decide with Northwind Retail?") && text.contains("knowledge/clients/acme.md"), "{text}");
    // Listed under the vault's key; live; the next message goes to the same process.
    assert!(crate::terminal::listed(&crate::terminal::vault_store_key(&v), &id));
    assert_eq!(live(), vec![id.clone()]);
    script(&p, "chat.2.jsonl", &turn("And the pricing stays per seat."));
    assert_eq!(send(&v.id, Some(&id), "And pricing?", Mode::Ask, &[]).unwrap(), id);
    let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
    assert_eq!(of_type(&ev, "user").len(), 2);
    assert_eq!(file(&p, "pids").lines().count(), 1, "one process for the conversation");
    // Others are refused: an empty or oversized message, a page outside the vault, a conversation not the vault's.
    assert!(send(&v.id, Some(&id), "  ", Mode::Ask, &[]).is_err());
    assert!(send(&v.id, Some(&id), &"x".repeat(20_001), Mode::Ask, &[]).is_err());
    assert!(send(&v.id, Some(&id), "Hi", Mode::Ask, &[Context::Page("../outside.md".into())]).is_err());
    assert!(send(&v.id, Some("00000000-0000-4000-8000-000000000000"), "Hi", Mode::Ask, &[]).is_err());
    assert!(read(&v.id, "../../etc", 0, Duration::ZERO).is_none());
}

#[test]
fn a_message_during_a_turn_is_refused() {
    let (_env, root) = crate::paths::testenv::folders("chat-busy");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.1.jsonl", "{\"_hang\":true}\n");
    let id = send(&v.id, None, "Summarise Northwind Retail", Mode::Ask, &[]).unwrap();
    let e = send(&v.id, Some(&id), "And Acme?", Mode::Ask, &[]).unwrap_err().to_string();
    assert!(e.contains("Wait for this answer, or stop it"), "{e}");
}

#[test]
fn mode_change_restarts_with_resume() {
    let (_env, root) = crate::paths::testenv::folders("chat-mode");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.jsonl", &turn("Done."));
    let id = send(&v.id, None, "What is on the Acme page?", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    let first = file(&p, "pids");
    assert_eq!(send(&v.id, Some(&id), "Add a line about the pilot", Mode::Curate, &[]).unwrap(), id);
    let ev = events_until(&v.id, &id, |ev, c| !c.busy && of_type(ev, "result").len() == 2);
    assert_eq!(of_type(&ev, "user").len(), 2, "one conversation: {ev:#?}");
    let a = argv(&p);
    assert_eq!(value_of(&a, "--resume"), Some(id.as_str()), "{a:?}");
    assert_eq!(value_of(&a, "--tools"), Some("Read,Grep,Glob,Skill,Agent,Edit,Write"));
    assert!(file(&p, "env").lines().any(|l| l == "ECHOCTL_RUN_CHAT=curate"));
    let settings = crate::knowledge::cache_dir(&v.id).join("chat").join(&id).join("settings.json");
    assert_eq!(serde_json::from_slice::<Value>(&std::fs::read(settings).unwrap()).unwrap()["env"][guard::RUN_CHAT], "curate");
    let pids: Vec<String> = file(&p, "pids").lines().map(str::to_owned).collect();
    assert_eq!(pids.len(), 2);
    until("the first process ended", || gone(&first));
    assert_eq!(live(), vec![id.clone()]);
    // Research only when the vault allows it (ruling R3).
    crate::knowledge::set(&v.id, "ask.research", "off").unwrap();
    let e = send(&v.id, Some(&id), "Look it up on the web", Mode::Research, &[]).unwrap_err().to_string();
    assert!(e.contains("Research"), "{e}");
}

#[test]
fn stop_interrupts_then_ends() {
    let (_env, root) = crate::paths::testenv::folders("chat-stop");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let asks = json!({"type": "control_request", "request_id": "req-7", "request": {"subtype": "can_use_tool", "tool_name": "Edit", "input": {"file_path": format!("{}/knowledge/clients/acme.md", v.root.display()), "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nPilot on 18 November.\n"}}});
    let init = turn("").lines().next().unwrap().to_string(); // Claude Code's `init` comes first
    script(&p, "chat.1.jsonl", &format!("{init}\n{asks}\n{{\"_hang\":true}}\n"));
    let id = send(&v.id, None, "Note the pilot date on Acme", Mode::Curate, &[]).unwrap();
    let ev = events_until(&v.id, &id, |_, c| c.waiting);
    assert_eq!(of_type(&ev, "permission")[0]["request"], "req-7");
    stop(&id);
    // The waiting card is denied, then the turn interrupted; the fake ignores the interrupt.
    until("the deny and the interrupt", || file(&p, "chat.in.jsonl").contains("\"interrupt\""));
    let sent: Vec<Value> = file(&p, "chat.in.jsonl").lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let deny = sent.iter().find(|l| l["type"] == "control_response").expect("an answer to the request");
    assert_eq!(deny["response"]["request_id"], "req-7");
    assert_eq!(deny["response"]["response"]["behavior"], "deny");
    assert_eq!(sent.last().unwrap()["request"]["subtype"], "interrupt");
    let ev = events_until(&v.id, &id, |ev, c| !c.waiting && !of_type(ev, "answered").is_empty());
    assert_eq!(of_type(&ev, "answered")[0]["allow"], false);
    std::thread::sleep(Duration::from_millis(300));
    let c = read(&v.id, &id, 0, Duration::ZERO).unwrap();
    assert!(c.live && c.busy, "still going before its 5 s");
    // 5 s later the process is ended, and the conversation stays resumable.
    skew(Duration::from_secs(5));
    let t = Instant::now();
    let ev = events_until(&v.id, &id, |_, c| !c.busy && !c.live);
    assert!(t.elapsed() < Duration::from_secs(3), "ended by the stop's 5 s, not by waiting: {:?}", t.elapsed());
    assert_eq!(ev.last().unwrap()["type"], "notice");
    assert_eq!(ev.last().unwrap()["kind"], "stopped");
    until("its process ended", || gone(&file(&p, "pids")));
    script(&p, "chat.2.jsonl", &turn("The pilot is noted."));
    send(&v.id, Some(&id), "Try again", Mode::Curate, &[]).unwrap();
    turn_events(&v.id, &id);
    assert_eq!(value_of(&argv(&p), "--resume"), Some(id.as_str()));
}

#[test]
fn other_control_requests_get_an_error_answer() {
    let (_env, root) = crate::paths::testenv::folders("chat-control");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let other = json!({"type": "control_request", "request_id": "req-9", "request": {"subtype": "mcp_message", "server_name": "x"}});
    script(&p, "chat.1.jsonl", &format!("{other}\n{}", turn("Fine.")));
    let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
    let ev = turn_events(&v.id, &id);
    assert!(of_type(&ev, "permission").is_empty() && of_type(&ev, "result").len() == 1, "{ev:#?}");
    let answer: Value = file(&p, "chat.in.jsonl").lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).find(|l| l["type"] == "control_response").unwrap();
    assert_eq!(answer["response"]["subtype"], "error");
    assert_eq!(answer["response"]["request_id"], "req-9");
}

#[test]
fn three_live_and_idle_ones_stop() {
    let (_env, root) = crate::paths::testenv::folders("chat-three");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.jsonl", &turn("Noted."));
    let start = |q: &str| {
        let id = send(&v.id, None, q, Mode::Ask, &[]).unwrap();
        turn_events(&v.id, &id);
        id
    };
    let (a, b, c) = (start("one"), start("two"), start("three"));
    assert_eq!(live().len(), 3);
    // Ten minutes later a fourth starts: the longest idle stops first (D38).
    skew(Duration::from_secs(600));
    let d = start("four");
    until("the longest idle stopped", || !live().contains(&a));
    let l = live();
    assert!(l.contains(&b) && l.contains(&c) && l.contains(&d), "{l:?}");
    // Idle ten minutes: stopped; the one just used stays.
    sweep();
    until("the idle ones stopped", || live() == vec![d.clone()]);
    // Stopped ones resume.
    send(&v.id, Some(&a), "one again", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &a);
    assert_eq!(value_of(&argv(&p), "--resume"), Some(a.as_str()));
}

#[test]
fn a_fourth_is_refused_while_three_answer() {
    // Final review F4: idle ones make room (D38); one answering is never stopped mid-answer for another.
    let (_env, root) = crate::paths::testenv::folders("chat-three-busy");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.jsonl", "{\"_hang\":true}\n");
    let ids: Vec<String> = ["one", "two", "three"].iter().map(|q| send(&v.id, None, q, Mode::Ask, &[]).unwrap()).collect();
    let answering = || ids.iter().all(|id| read(&v.id, id, 0, Duration::ZERO).is_some_and(|c| c.live && c.busy));
    until("three answering", answering);
    // Long since they were sent: still answering, so none is the one to stop.
    skew(IDLE);
    let e = send(&v.id, None, "four", Mode::Ask, &[]).unwrap_err().to_string();
    assert_eq!(e, "3 conversations are answering. Wait for one, or stop one.");
    std::thread::sleep(Duration::from_millis(300));
    assert!(answering(), "none of the three is stopped");
    assert_eq!(live().len(), 3);
    assert_eq!(file(&p, "pids").lines().count(), 3, "no fourth process");
    assert!(list_value(&v).as_array().unwrap().iter().all(|c| ids.iter().any(|id| c["id"] == id.as_str())), "no fourth conversation left behind");
}

#[test]
fn a_conversation_live_in_the_panel_is_refused() {
    let (_env, root) = crate::paths::testenv::folders("chat-panel");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let store = crate::terminal::vault_store_key(&v);
    crate::terminal::remember(&store, ID);
    let mut cmd = portable_pty::CommandBuilder::new("/bin/sh");
    cmd.args(["-c", "sleep 30"]);
    let pty = crate::terminal::open_with("claude:vault", cmd, 80, 20, None, None, Some(ID.into())).unwrap();
    let e = send(&v.id, Some(ID), "Hello", Mode::Ask, &[]).unwrap_err().to_string();
    assert!(e.contains("open in the terminal panel"), "{e}");
    crate::terminal::close(pty);
    // And the panel refuses one live in Ask.
    script(&p, "chat.jsonl", &turn("Hi."));
    let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    let e = crate::terminal::start_vault("claude:vault:twin", &v.id, Some(&id), 80, 20).unwrap_err().to_string();
    assert!(e.contains("open in Ask"), "{e}");
    // Taken to the terminal panel from Ask: its process ends first; the line resumes it (a managed profile has none).
    let t = to_terminal(&v.id, &id).unwrap();
    assert_eq!(t["conversation"], id.as_str());
    assert!(t["command"].is_null());
    assert!(!live().contains(&id));
}

#[test]
fn a_crash_mid_turn_ends_the_turn_with_an_error() {
    let (_env, root) = crate::paths::testenv::folders("chat-crash");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let half: String = turn("Half an ans").lines().take(3).map(|l| format!("{l}\n")).collect();
    script(&p, "chat.1.jsonl", &format!("{half}{{\"_exit\":3}}\n"));
    let id = send(&v.id, None, "What is open with Acme?", Mode::Ask, &[]).unwrap();
    let ev = events_until(&v.id, &id, |_, c| !c.busy && !c.live);
    let last = ev.last().unwrap();
    assert_eq!(last["type"], "error", "{ev:#?}");
    assert!(last["text"].as_str().unwrap().contains("fake claude crashed"), "{last}");
    assert!(of_type(&ev, "text").last().is_some_and(|t| t["done"] == true), "the partial text is closed: {ev:#?}");
    script(&p, "chat.2.jsonl", &turn("Here it is."));
    send(&v.id, Some(&id), "Again", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    assert_eq!(value_of(&argv(&p), "--resume"), Some(id.as_str()), "still resumable");
}

#[test]
fn a_sign_in_needed_is_an_error_event_with_its_action() {
    let (_env, root) = crate::paths::testenv::folders("chat-signin-needed");
    let _fake = fake::install(&root);
    let _clean = clean();
    let _p = fake::profile("w", None, fake::SIGNED_OUT);
    let v = vault(&root, "w");
    let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
    let ev = events_until(&v.id, &id, |ev, _| !ev.is_empty());
    assert_eq!(ev[0]["type"], "error");
    assert_eq!(ev[0]["action"], "signin");
    assert_eq!(ev[0]["profile"], "w");
    assert!(live().is_empty());
}

#[test]
fn a_first_start_that_dies_before_its_session_starts_new_again() {
    let (_env, root) = crate::paths::testenv::folders("chat-first-dies");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    // Claude Code refused at once (a flag it doesn't know): no `init`, no session.
    script(&p, "chat.1.jsonl", "{\"_exit\":3}\n");
    let id = send(&v.id, None, "What is open with Acme?", Mode::Ask, &[]).unwrap();
    events_until(&v.id, &id, |_, c| !c.busy && !c.live);
    script(&p, "chat.2.jsonl", &turn("Here it is."));
    send(&v.id, Some(&id), "What is open with Acme?", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    let a = argv(&p);
    assert_eq!(value_of(&a, "--session-id"), Some(id.as_str()), "{a:?}");
    assert!(value_of(&a, "--resume").is_none(), "there is nothing to resume: {a:?}");
}

#[test]
fn the_tests_sweep_by_themselves() {
    let (_env, root) = crate::paths::testenv::folders("chat-no-sweeper");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.jsonl", &turn("Noted."));
    let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    // A sweep of its own could fire between a test's clock skew and its check.
    assert!(!SWEEPER.is_completed(), "no sweeper thread in tests");
}

#[test]
fn quitting_stops_chats_at_once() {
    let (_env, root) = crate::paths::testenv::folders("chat-quit");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.1.jsonl", "{\"_hang\":true}\n");
    let id = send(&v.id, None, "Summarise Northwind Retail", Mode::Ask, &[]).unwrap();
    until("the turn going", || file(&p, "chat.in.jsonl").contains("Northwind"));
    // Echo quits right after: only what `close_all` does before it returns counts.
    NO_ESCALATION.store(true, Ordering::SeqCst);
    close_all();
    until("its process stopped", || gone(&file(&p, "pids")));
    events_until(&v.id, &id, |_, c| !c.live);
}

#[test]
fn a_stop_whose_writes_block_still_ends_the_process() {
    let (_env, root) = crate::paths::testenv::folders("chat-stop-blocked");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.1.jsonl", "{\"_hang\":true}\n");
    let id = send(&v.id, None, "Summarise Northwind Retail", Mode::Ask, &[]).unwrap();
    let c = lock(&CHATS).get(&id).cloned().unwrap();
    let stdin = lock(&c.state).proc.as_ref().unwrap().stdin.clone();
    // Its input is busy (a write that doesn't get through): Stop's own writes wait.
    let held = lock(&stdin);
    let stopping = { let id = id.clone(); std::thread::spawn(move || stop(&id)) };
    std::thread::sleep(Duration::from_millis(200));
    skew(Duration::from_secs(5));
    until("the stop's 5 s ended it", || gone(&file(&p, "pids")));
    drop(held);
    stopping.join().unwrap();
    let ev = events_until(&v.id, &id, |_, c| !c.busy && !c.live);
    assert_eq!(ev.last().unwrap()["kind"], "stopped");
}

#[test]
fn a_block_rendered_many_times_keeps_one_copy() {
    let (_env, root) = crate::paths::testenv::folders("chat-one-copy");
    let v = vault(&root, "system");
    let c = Conv::new(ID, &v, false);
    let mut st = lock(&c.state);
    let long = "Northwind Retail fixes forward. ".repeat(200);
    for i in 1..=50 {
        st.push(json!({"type": "text", "block": "b1", "html": format!("<p>{}</p>", &long[..long.len() * i / 50]), "done": false}));
        st.push(json!({"type": "thinking", "block": "b2", "text": &long[..long.len() * i / 50], "done": false}));
    }
    st.push(json!({"type": "text", "block": "b1", "html": format!("<p>{long}</p>"), "done": true}));
    assert_eq!(st.events.iter().filter(|e| e.get("html").is_some()).count(), 1, "the latest render only");
    assert_eq!(st.events.iter().filter(|e| e["type"] == "thinking" && e.get("text").is_some()).count(), 1);
    assert_eq!(st.events.back().unwrap()["html"].as_str().unwrap().len(), long.len() + 7);
    assert!(st.events.iter().enumerate().all(|(i, e)| e["n"] == i as u64), "numbers stay as they were");
    assert!(st.events[0]["superseded"] == true);
    // A new process numbers its blocks again from b1: its first render supersedes nothing of the last one's.
    st.renders.clear();
    st.push(json!({"type": "text", "block": "b1", "html": "<p>Again</p>", "done": false}));
    assert!(st.events[100].get("html").is_some());
}

#[test]
fn the_chats_route_lists_the_vaults_conversations() {
    let (_env, root) = crate::paths::testenv::folders("chat-list");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    script(&p, "chat.jsonl", &turn("Noted."));
    let id = send(&v.id, None, "What is new at Northwind Retail?", Mode::Ask, &[]).unwrap();
    turn_events(&v.id, &id);
    let l = list_value(&v);
    let mine = l.as_array().unwrap().iter().find(|c| c["id"] == id.as_str()).expect("listed");
    assert_eq!(mine["live"], true);
    assert_eq!(mine["busy"], false);
    assert!(mine["at"].is_string());
}

/// `body` as conversation `ID`'s transcript in the profile's Claude Code folder, listed under the vault's key.
fn past(p: &ai::Profile, v: &Vault, body: &str) {
    let dir = p.config_folder().join("projects/-made-up");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{ID}.jsonl")), body).unwrap();
    crate::terminal::remember(&crate::terminal::vault_store_key(v), ID);
}

#[test]
fn a_past_conversation_opens_from_its_transcript_and_continues() {
    let (_env, root) = crate::paths::testenv::folders("chat-history");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    past(&p, &v, &include_str!("../testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
    // No process and no events: the page reads the transcript, numbered from 0 as live events are.
    let c = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    let t: Vec<&str> = c.events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(t, ["user", "tool", "tool_result", "thinking", "text"], "{:#?}", c.events);
    assert_eq!(c.events.iter().map(|e| e["n"].as_u64().unwrap()).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
    assert!((c.next, c.live, c.busy) == (5, false, false));
    // Read again: not twice.
    assert_eq!(read(&v.id, ID, 5, Duration::from_millis(50)).unwrap().events.len(), 0);
    // Continuing it: the live events follow, the history keeps its numbers.
    script(&p, "chat.jsonl", &turn("Fixed."));
    assert_eq!(send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap(), ID);
    let live = events_until(&v.id, ID, |ev, c| !c.busy && of_type(ev, "result").len() == 1);
    assert_eq!(live[..5], c.events[..], "the history is as it was");
    assert_eq!(live[5]["type"], "user");
    assert_eq!(live[5]["n"], 5);
    assert!(live.iter().skip(5).all(|e| e["n"].as_u64().unwrap() >= 5));
    assert_eq!(value_of(&argv(&p), "--resume"), Some(ID));
}

#[test]
fn an_idle_conversation_lets_its_events_go_and_reads_them_again() {
    // Final review F5: a conversation opened from the list doesn't keep its events for the app's life.
    let (_env, root) = crate::paths::testenv::folders("chat-let-go");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    past(&p, &v, &include_str!("../testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
    let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    assert_eq!((first.events.len(), first.next), (5, 5));
    let held = || lock(&CHATS).get(ID).map(|c| lock(&c.state).events.len());
    // Ten minutes idle with no process: its events go from memory.
    skew(IDLE);
    sweep();
    assert_eq!(held(), Some(0), "its events are let go");
    // A page still showing it polls on: nothing new, nothing twice.
    let c = read(&v.id, ID, 5, Duration::from_millis(50)).unwrap();
    assert!(c.events.is_empty() && c.next == 5, "{:#?}", c.events);
    // Opened again: the same events from its transcript, numbered after every number it gave (5 was its end).
    let again = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    let words = |ev: &[Value]| ev.iter().map(|e| e.as_object().unwrap().iter().filter(|(k, _)| *k != "n").map(|(k, x)| (k.clone(), x.clone())).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert_eq!(words(&again.events), words(&first.events));
    assert_eq!(again.events.iter().map(|e| e["n"].as_u64().unwrap()).collect::<Vec<_>>(), [6, 7, 8, 9, 10]);
    assert_eq!(again.next, 11);
    assert!(read(&v.id, ID, 11, Duration::from_millis(50)).unwrap().events.is_empty(), "and the poll goes on from there");
    // Let go again while a page shows it: a message from that page follows what it shows, its past not read twice.
    skew(IDLE);
    sweep();
    assert_eq!(held(), Some(0));
    script(&p, "chat.jsonl", &turn("Fixed."));
    send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap();
    let (ev, _) = poll_from(&v, 11);
    assert_eq!((ev[0]["type"].as_str(), ev[0]["n"].as_u64()), (Some("user"), Some(11)), "{ev:#?}");
    assert!(ev.iter().all(|e| e["n"].as_u64().unwrap() >= 11));
    // A conversation going on keeps its events.
    skew(IDLE);
    sweep();
    until("its idle process ended", || !live().contains(&ID.to_string()));
    assert!(held().unwrap() > 0, "its process has only just ended");
}

/// The page's poll from `from` until the turn has ended: its events and where it is then.
fn poll_from(v: &Vault, mut from: u64) -> (Vec<Value>, u64) {
    let (mut all, t) = (vec![], Instant::now());
    loop {
        let c = read(&v.id, ID, from, Duration::from_millis(200)).expect("the conversation");
        from = c.next;
        all.extend(c.events.iter().cloned());
        if !c.busy && all.iter().any(|e| e["type"] == "result") {
            return (all, from);
        }
        assert!(t.elapsed() < Duration::from_secs(10), "timed out: {all:#?}");
    }
}

/// The user's words and the finished answers' text in `ev`, in order: what a reader sees of the conversation.
fn said(ev: &[Value]) -> Vec<String> {
    ev.iter()
        .filter_map(|e| match e["type"].as_str() {
            Some("user") => e["text"].as_str().map(|t| format!("you: {}", t.chars().take(30).collect::<String>())),
            Some("text") if e["done"] == true => e["md"].as_str().map(|t| format!("claude: {}", t.chars().take(30).collect::<String>())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_conversation_continued_after_a_let_go_opens_whole_again() {
    // Final review X2: opened from before what Echo holds, it is read again from its transcript — the earlier
    // turns and the new one, each once — and a page that kept polling takes it whole once more, not twice.
    let (_env, root) = crate::paths::testenv::folders("chat-reopen-whole");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    past(&p, &v, &include_str!("../testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
    let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    let before = said(&first.events);
    assert_eq!(before.len(), 2, "{before:?}");
    skew(IDLE);
    sweep();
    // The page still showing it sends a follow-up and polls on from where it was.
    script(&p, "chat.jsonl", &turn("Fixed, with its date and tags."));
    send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap();
    let (shown, at) = poll_from(&v, first.next);
    assert_eq!(said(&shown), ["you: Yes, fix it", "claude: Fixed, with its date and tags."]);
    // Claude Code wrote the turn into its transcript (the fake doesn't: done here, as it would be).
    let file = p.config_folder().join(format!("projects/-made-up/{ID}.jsonl"));
    let turn_lines = [
        json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": [{"type": "text", "text": "Yes, fix it"}]}}),
        json!({"type": "assistant", "isSidechain": false, "message": {"id": "msg_follow", "type": "message", "role": "assistant", "content": [{"type": "text", "text": "Fixed, with its date and tags."}], "stop_reason": "end_turn"}}),
    ];
    let mut t = std::fs::read_to_string(&file).unwrap();
    turn_lines.iter().for_each(|l| t.push_str(&format!("{l}\n")));
    std::fs::write(&file, t).unwrap();
    // Opened again (another conversation and back, a reload): the whole of it, each message once.
    let again = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    let whole = [before.clone(), said(&shown)].concat();
    assert_eq!(said(&again.events), whole);
    // The page that kept polling: its next answer doesn't follow on from where it was (so it takes it whole), and
    // it is the whole conversation, each message once; then nothing more.
    let kept = read(&v.id, ID, at, Duration::from_millis(50)).unwrap();
    assert_ne!(kept.events.first().and_then(|e| e["n"].as_u64()), Some(at), "it can't pass for what follows");
    assert_eq!(said(&kept.events), whole);
    assert!(read(&v.id, ID, kept.next, Duration::from_millis(50)).unwrap().events.is_empty());
}

#[test]
fn opened_while_a_turn_goes_it_is_read_whole_when_the_turn_ends() {
    // Final review X2: a page opening it mid-turn after a let-go gets the whole conversation once the turn is over.
    let (_env, root) = crate::paths::testenv::folders("chat-reopen-busy");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    let history = include_str!("../testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy());
    past(&p, &v, &history);
    let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    skew(IDLE);
    sweep();
    // Its turn writes the transcript (as Claude Code does), then waits on a card.
    let follow = [
        json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": [{"type": "text", "text": "Yes, fix it"}]}}),
        json!({"type": "assistant", "isSidechain": false, "message": {"id": "msg_follow", "type": "message", "role": "assistant", "content": [{"type": "text", "text": "Fixed."}], "stop_reason": "end_turn"}}),
    ];
    let file = p.config_folder().join(format!("projects/-made-up/{ID}.jsonl"));
    std::fs::write(fake::file(&p, "apply.1"), format!("{history}{}", follow.iter().map(|l| format!("{l}\n")).collect::<String>())).unwrap();
    std::fs::write(fake::file(&p, "apply.1.to"), file.display().to_string()).unwrap();
    let asks = json!({"type": "control_request", "request_id": "req-1", "request": {"subtype": "can_use_tool", "tool_name": "Edit", "input": {"file_path": format!("{}/knowledge/clients/acme.md", v.root.display()), "old_string": "# Acme Corp\n", "new_string": "# Acme Corp\n\nFixed.\n"}}});
    let played = turn("Fixed.");
    let (init, rest) = played.split_once('\n').unwrap();
    script(&p, "chat.jsonl", &format!("{init}\n{{\"_apply\":1}}\n{asks}\n{rest}"));
    send(&v.id, Some(ID), "Yes, fix it", Mode::Ask, &[]).unwrap();
    until("the card", || read(&v.id, ID, first.next, Duration::ZERO).is_some_and(|c| c.waiting));
    // Opened meanwhile: what it holds; the transcript isn't read while the turn goes.
    let opened = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    assert_eq!(said(&opened.events), ["you: Yes, fix it"]);
    permit(ID, "req-1", false, false, None).unwrap();
    // That page polls on: once the turn is over, the whole conversation, each message once.
    let (mut from, mut seen, t) = (opened.next, vec![], Instant::now());
    loop {
        let c = read(&v.id, ID, from, Duration::from_millis(200)).unwrap();
        if c.events.first().and_then(|e| e["n"].as_u64()).is_some_and(|n| n != from) {
            seen.clear(); // answered from elsewhere than asked: the page takes it whole again
        }
        from = c.next;
        seen.extend(c.events.iter().cloned());
        if !c.busy && said(&seen).len() == 4 {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(10), "{seen:#?}");
    }
    assert_eq!(said(&seen)[2..], ["you: Yes, fix it".to_string(), "claude: Fixed.".to_string()]);
    assert_eq!(said(&seen)[..2], said(&first.events)[..]);
}

#[test]
fn a_sweep_right_after_opening_drops_nothing() {
    // Final review X2: opening counts as use.
    let (_env, root) = crate::paths::testenv::folders("chat-open-use");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    past(&p, &v, &include_str!("../testdata/transcript/ask-session.jsonl").replace("/VAULT", &v.root.to_string_lossy()));
    let first = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    skew(IDLE);
    sweep();
    let held = || lock(&CHATS).get(ID).map(|c| lock(&c.state).events.len());
    assert_eq!(held(), Some(0));
    // Opened again, and a sweep comes at once: what was just read stays for the page.
    let again = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    sweep();
    assert_eq!(held(), Some(first.events.len()), "nothing let go right after it was opened");
    assert_eq!(said(&read(&v.id, ID, 0, Duration::from_millis(50)).unwrap().events), said(&again.events));
}

#[test]
fn a_transcript_that_cant_be_read_is_said_in_the_conversation() {
    let (_env, root) = crate::paths::testenv::folders("chat-history-broken");
    let _fake = fake::install(&root);
    let _clean = clean();
    let p = signed_in();
    let v = vault(&root, "w");
    past(&p, &v, "this is not a transcript\n");
    let c = read(&v.id, ID, 0, Duration::from_millis(50)).unwrap();
    assert_eq!(c.events.len(), 1, "{:#?}", c.events);
    assert_eq!((c.events[0]["type"].as_str(), c.events[0]["n"].as_u64()), (Some("notice"), Some(0)));
    assert_eq!(c.events[0]["text"], "Its history can't be shown here; continuing works.");
    // A conversation never started has no past to show.
    script(&p, "chat.jsonl", &turn("Hi."));
    let id = send(&v.id, None, "Hello", Mode::Ask, &[]).unwrap();
    assert!(turn_events(&v.id, &id).iter().all(|e| e["type"] != "notice"));
}

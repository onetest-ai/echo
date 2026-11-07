//! Sources for Ingest (spec K14), on made-up vaults, calls, mail and chats (no real people or companies).

use super::*;
use crate::knowledge::{self, export, fixture, fm, git, AddOpts};
use serde_json::json;
use std::path::Path;

fn connect(folder: &Path) -> Vault {
    knowledge::add(folder, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
}

fn twin(root: &Path) -> Vault {
    connect(&fixture::twin(&root.join("twin")))
}

fn read(v: &Vault, rel: &str) -> String {
    std::fs::read_to_string(knowledge::join(&v.root, rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn local(at: &str, fmt: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(at).unwrap().with_timezone(&chrono::Local).format(fmt).to_string()
}

fn sha(b: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// The vault-relative paths `add_files` wrote, in order (a refusal's error text in its place).
fn added(r: &[(String, Result<Added>)]) -> Vec<String> {
    r.iter().map(|(_, a)| a.as_ref().map(|a| a.path.clone()).unwrap_or_else(|e| format!("refused: {e:#}"))).collect()
}

#[test]
fn files_are_copied_as_they_are() {
    // K14-T1
    let (_env, root) = crate::paths::testenv::folders("kn-src-files");
    let v = twin(&root);
    let picked = root.join("picked");
    std::fs::create_dir_all(&picked).unwrap();
    let bytes: Vec<u8> = (0..=255u8).cycle().take(70_000).collect();
    std::fs::write(picked.join("a.pdf"), &bytes).unwrap();
    let big = picked.join("big deck.pdf");
    std::fs::File::create(&big).unwrap().set_len(60 << 20).unwrap();
    let r = add_files(&v, &[picked.join("a.pdf"), picked.join("a.pdf"), big.clone(), picked.join("a.pdf")]);
    let paths = added(&r);
    assert_eq!(paths[..2], ["raw_inbox/a.pdf", "raw_inbox/a (2).pdf"], "{paths:?}");
    assert_eq!(paths[3], "raw_inbox/a (3).pdf", "{paths:?}");
    assert!(paths[2].starts_with("refused:") && paths[2].contains("big deck.pdf") && paths[2].contains("50 MB"), "{}", paths[2]);
    for p in ["raw_inbox/a.pdf", "raw_inbox/a (2).pdf"] {
        assert_eq!(std::fs::read(knowledge::join(&v.root, p)).unwrap(), bytes, "{p} byte for byte");
    }
    assert!(!v.root.join("raw_inbox/big deck.pdf").exists(), "the big one isn't copied");
    assert_eq!(r[0].1.as_ref().unwrap().committed, "off", "commits are off for a connected vault");
    let l = ledger(&v).unwrap();
    let new: Vec<&String> = l.sources.iter().filter(|(_, e)| e.state == "new" && e.by == "file").map(|(p, _)| p).collect();
    assert_eq!(new, ["raw_inbox/a (2).pdf", "raw_inbox/a (3).pdf", "raw_inbox/a.pdf"], "{l:?}");
    assert_eq!(l.sources["raw_inbox/a.pdf"].sha256, sha(&bytes));
    let acts = knowledge::activity::recent(&v, "2000-01-01T00:00:00+00:00");
    assert!(acts.iter().any(|a| a.path == "raw_inbox/a (2).pdf" && a.by == "source" && a.kind == "new"), "{acts:?}");
    // Nothing processed by adding: they wait as New.
    let q = queue(&v).unwrap();
    assert!(q.iter().filter(|s| s.path.starts_with("raw_inbox/a")).all(|s| s.state == "new" && s.kind == Kind::File), "{q:?}");
}

#[test]
fn a_call_becomes_a_source() {
    // K14-T2
    let (_env, root) = crate::paths::testenv::folders("kn-src-call");
    let v = twin(&root);
    let call = fixture::call("2026-11-05_090012-weekly-sync", "Weekly delivery sync", "system", fixture::MINUTES);
    std::fs::write(call.join("transcript.md"), "# Transcript\n\n- **Dana Reyes**: The quokka budget stays between us.\n").unwrap();
    std::fs::write(call.join("chat.json"), r#"[{"from": "Sam Hale", "text": "Slides are in the channel."}]"#).unwrap();
    let note = export::save(&call, &v.id, export::How::manual()).unwrap().path;
    let date = local("2026-11-05T09:00:12+02:00", "%Y-%m-%d");
    let a = add_call(&v, &call, Include::default(), false).unwrap();
    assert_eq!(a.path, format!("raw_inbox/echo/{date}-weekly-delivery-sync.md"));
    assert_eq!(
        read(&v, &a.path),
        format!(
            "---\ntitle: Weekly delivery sync\ndate: {date}\nkind: call\nparticipants: Alex Doe, Dana Reyes, Sam Hale\nduration: 46m\n\
             source: echo://call/2026-11-05_090012\nnote: {note}\norigin: echo\n---\n\n# Weekly delivery sync — minutes\n\n{}\n",
            fixture::MINUTES.trim_end()
        )
    );
    // Ticked: the transcript and the chat follow the minutes.
    let b = add_call(&v, &call, Include { transcript: true, chat: true }, false).unwrap();
    assert_eq!(b.path, format!("raw_inbox/echo/{date}-weekly-delivery-sync (2).md"));
    let text = read(&v, &b.path);
    assert!(text.contains("\n## Transcript\n\n# Transcript\n\n- **Dana Reyes**: The quokka budget stays between us.\n"), "{text}");
    assert!(text.ends_with("\n## Chat\n\n- **Sam Hale**: Slides are in the channel.\n"), "{text}");
    // A call on another profile: refused naming both; with *Add anyway*, added and recorded.
    let other = fixture::call("2026-11-06_100000-pricing", "Pricing review", "elsewhere", "# Pricing review\n\n- Per seat.\n");
    let e = add_call(&v, &other, Include::default(), false).unwrap_err();
    let m = e.downcast_ref::<Mismatch>().unwrap_or_else(|| panic!("{e:#}"));
    assert_eq!((m.call.as_str(), m.vault.as_str()), ("elsewhere", "system"));
    let said = e.to_string();
    assert!(said.contains("elsewhere") && said.contains("Claude Code (as configured)"), "{said}");
    assert!(!v.root.join("raw_inbox/echo").read_dir().unwrap().flatten().any(|f| f.file_name().to_string_lossy().contains("pricing")), "nothing written");
    let c = add_call(&v, &other, Include::default(), true).unwrap();
    let l = ledger(&v).unwrap();
    assert!(l.sources[&c.path].anyway && l.sources[&c.path].by == "call", "{l:?}");
    assert!(!l.sources[&a.path].anyway);
    assert!(!read(&v, &c.path).contains("\nnote:"), "no note of this call in the vault");
}

/// A mail as Graph gives it, made up.
fn graph_mail(id: &str, subject: &str, from: (&str, &str), to: &[(&str, &str)], at: &str, html: &str, files: &[&str]) -> serde_json::Value {
    let who = |(n, a): (&str, &str)| json!({"emailAddress": {"name": n, "address": a}});
    json!({
        "id": id, "conversationId": "conv-1", "subject": subject, "from": who(from), "toRecipients": to.iter().map(|x| who(*x)).collect::<Vec<_>>(),
        "ccRecipients": [], "receivedDateTime": at, "body": {"contentType": "html", "content": html}, "webLink": format!("https://outlook.example/owa/?ItemID={id}"),
        "hasAttachments": !files.is_empty(), "attachments": files.iter().map(|f| json!({"name": f, "isInline": f.starts_with("image0")})).collect::<Vec<_>>(),
    })
}

/// A mail as `graph::conversation_with_files` gives it: Echo's item with its attachments' names.
fn item(raw: &serde_json::Value) -> serde_json::Value {
    let mut m = crate::graph::mail_item(raw);
    m["files"] = json!(crate::graph::attachment_names(&raw["attachments"]));
    m
}

#[test]
fn a_mail_thread_becomes_a_source() {
    // K14-T3
    let (_env, root) = crate::paths::testenv::folders("kn-src-mail");
    let v = twin(&root);
    let (lena, omar, priya) = (("Lena Hartmann", "lena@northwind.example"), ("Omar Reyes", "omar@contoso.example"), ("Priya Nair", "priya@northwind.example"));
    let raw = [
        graph_mail("m1", "Northwind Retail — pilot plan", lena, &[omar], "2026-11-03T09:15:00Z", "<html><body><p>Hi Omar,</p><p>Attached is the <b>pilot plan</b> &amp; its timeline.</p><div>Lena</div></body></html>", &["pilot-plan.pdf", "image001.png"]),
        graph_mail("m2", "RE: Northwind Retail — pilot plan", omar, &[lena], "2026-11-03T14:40:00Z", "<div><p>Thanks Lena, looks good.</p><p>Who owns the data migration?</p></div><div>On Mon, Lena Hartmann wrote:</div><blockquote><p>Hi Omar,</p></blockquote>", &[]),
        graph_mail("m3", "RE: Northwind Retail — pilot plan", lena, &[omar, priya], "2026-11-04T08:05:00Z", "<p>Priya Nair will own it.<br>Kickoff is on 18 November.</p>", &["kickoff.ics", "budget.xlsx"]),
    ];
    let mut items: Vec<serde_json::Value> = raw.iter().map(item).collect();
    // A lookup that failed is said, not left out.
    items[1]["files_error"] = json!("Graph: ErrorAccessDenied");
    let a = add_thread(&v, &items[0], &items).unwrap();
    let date = local("2026-11-04T08:05:00Z", "%Y-%m-%d");
    assert_eq!(a.path, format!("raw_inbox/mail/{date}-northwind-retail-pilot-plan.md"));
    let text = read(&v, &a.path);
    let (head, body) = fm::split(&text);
    let props = fm::parse(head.unwrap());
    let prop = |k: &str| fm::get(&props, k).map(|p| p.text()).unwrap_or_default();
    assert_eq!((prop("title"), prop("kind"), prop("date")), ("Northwind Retail — pilot plan".to_string(), "mail".to_string(), date.clone()));
    assert_eq!(prop("people"), "Lena Hartmann, Omar Reyes, Priya Nair");
    assert_eq!(prop("source"), "https://outlook.example/owa/?ItemID=m1");
    assert_eq!(prop("origin"), "echo");
    assert_eq!(body.matches("\n### ").count(), 3, "{body}");
    assert!(body.contains(&format!("### Lena Hartmann · {}\n\nHi Omar,\nAttached is the pilot plan & its timeline.\nLena\n\nAttachments: pilot-plan.pdf\n", local("2026-11-03T09:15:00Z", "%Y-%m-%d %H:%M"))), "{body}");
    assert!(body.contains(&format!("### Omar Reyes · {}\n\nThanks Lena, looks good.\nWho owns the data migration?\n\nAttachments: Echo couldn't list them (Graph: ErrorAccessDenied)\n", local("2026-11-03T14:40:00Z", "%Y-%m-%d %H:%M"))), "{body}");
    assert!(!body.contains("image001.png"), "an inline image (a signature's) is no attachment: {body}");
    assert!(!body.contains("wrote:"), "the quoted mail is left out: {body}");
    assert!(body.contains("Priya Nair will own it.\nKickoff is on 18 November.\n\nAttachments: kickoff.ics, budget.xlsx"), "{body}");
    assert!(!body.contains('<') && !body.contains("&amp;"), "text without tags: {body}");
    assert_eq!(ledger(&v).unwrap().sources[&a.path].by, "mail");
    assert_eq!(queue(&v).unwrap().iter().find(|s| s.path == a.path).map(|s| (s.kind, s.title.clone())), Some((Kind::Mail, "Northwind Retail — pilot plan".into())));
}

fn chat_msg(from: &str, at: &str, html: &str, files: &[&str]) -> serde_json::Value {
    json!({"id": at, "messageType": "message", "createdDateTime": at, "deletedDateTime": null, "from": {"user": {"displayName": from}},
           "body": {"contentType": "html", "content": html}, "attachments": files.iter().map(|f| json!({"contentType": "reference", "name": f})).collect::<Vec<_>>()})
}

#[test]
fn a_teams_chat_becomes_a_source() {
    let (_env, root) = crate::paths::testenv::folders("kn-src-teams");
    let v = twin(&root);
    let chat = Chat { id: "19:made-up@thread.v2".into(), name: "Design / Ops: weekly".into(), url: Some("https://teams.example/l/chat/19:made-up".into()) };
    let (since, until) = (chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap(), chrono::NaiveDate::from_ymd_opt(2026, 11, 7).unwrap());
    let msgs = [
        chat_msg("Omar Reyes", "2026-11-02T10:00:00Z", "<p>Deck for <at id=\"0\">Lena</at> attached</p>", &["deck.pptx"]),
        chat_msg("Lena Hartmann", "2026-11-02T10:05:00Z", "<p>Thanks &amp; noted.</p>", &[]),
    ];
    let a = add_chat_messages(&v, &chat, since, until, &msgs).unwrap();
    assert_eq!(a.path, "raw_inbox/teams/2026-11-07-design-ops-weekly.md");
    let text = read(&v, &a.path);
    let (head, body) = fm::split(&text);
    let props = fm::parse(head.unwrap());
    let prop = |k: &str| fm::get(&props, k).map(|p| p.text()).unwrap_or_default();
    assert_eq!((prop("title"), prop("kind"), prop("since"), prop("until")), ("Design / Ops: weekly".into(), "teams".into(), "2026-11-01".into(), "2026-11-07".into()));
    assert_eq!(prop("source"), "https://teams.example/l/chat/19:made-up");
    assert!(body.contains(&format!("### Omar Reyes · {}\n\nDeck for Lena attached\n\nAttachments: deck.pptx\n", local("2026-11-02T10:00:00Z", "%Y-%m-%d %H:%M"))), "{body}");
    assert!(body.contains("Thanks & noted."), "{body}");
    assert!(add_chat_messages(&v, &chat, since, until, &[]).is_err(), "nothing said in those days: nothing added");
    assert!(add_chat_messages(&v, &chat, until, since, &msgs).is_err(), "a range that ends before it starts");
}

/// A name that is a file name on macOS and on Windows alike.
fn safe_everywhere(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || (stem.len() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")));
    !name.is_empty() && !reserved && !name.starts_with('.') && !name.ends_with(['.', ' ']) && name.len() <= 255 && name.encode_utf16().count() <= 255
        && !name.chars().any(|c| "<>:\"/\\|?*".contains(c) || c.is_control())
}

#[test]
fn odd_titles_make_safe_source_names() {
    // Review focus 4: Cyrillic, `/` and `:`, emoji, 300 characters, reserved names, a taken name.
    let (_env, root) = crate::paths::testenv::folders("kn-src-odd");
    let v = twin(&root);
    let long = "Quarterly planning for the northern stores ".repeat(7);
    let subjects = ["Встреча по проекту «Север»", "Plan / Q3: pricing?", "🚀🎉", long.as_str(), "CON", "  ", "Weekly sync", "Weekly sync", "Line one\nline two\ttabbed \"quoted\" \\ end"];
    let at = "2026-11-04T08:05:00Z";
    let date = local(at, "%Y-%m-%d");
    let mut names = vec![];
    for (i, s) in subjects.iter().enumerate() {
        let m = item(&graph_mail(&format!("m{i}"), s, ("Lena Hartmann", "lena@northwind.example"), &[], at, "<p>Made-up text.</p>", &[]));
        let a = add_thread(&v, &m, std::slice::from_ref(&m)).unwrap_or_else(|e| panic!("{s:?}: {e:#}"));
        let name = a.path.rsplit('/').next().unwrap().to_string();
        assert!(a.path.starts_with("raw_inbox/mail/") && safe_everywhere(&name), "{s:?} → {name:?}");
        // The frontmatter reads back: the title on one line.
        let text = read(&v, &a.path);
        let props = fm::parse(fm::split(&text).0.unwrap());
        let title = fm::get(&props, "title").map(|p| p.text()).unwrap_or_default();
        let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(title, if one.is_empty() { "(no subject)".to_string() } else { one.clone() }, "{s:?}");
        assert!(text.contains(&format!("\n# {}\n", crate::knowledge::extract::md_text(&title))), "{text}");
        names.push(name);
    }
    assert_eq!(names[0], format!("{date}-встреча-по-проекту-север.md"));
    assert_eq!(names[1], format!("{date}-plan-q3-pricing.md"));
    assert_eq!(names[2], format!("{date}-mail.md"));
    assert!(names[3].len() < 100 && names[3].starts_with(&format!("{date}-quarterly-planning")), "{}", names[3]);
    assert_eq!(names[4], format!("{date}-con.md"));
    assert_eq!(names[5], format!("{date}-mail (2).md"));
    assert_eq!((names[6].as_str(), names[7].as_str()), (format!("{date}-weekly-sync.md").as_str(), format!("{date}-weekly-sync (2).md").as_str()));
    // A chat named with `/` and `:`.
    let chat = Chat { id: "19:x@thread.v2".into(), name: "Ops/Infra: on-call".into(), url: None };
    let d = chrono::NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
    let a = add_chat_messages(&v, &chat, d, d, &[chat_msg("Omar Reyes", "2026-11-07T09:00:00Z", "<p>Paged twice.</p>", &[])]).unwrap();
    assert_eq!(a.path, "raw_inbox/teams/2026-11-07-ops-infra-on-call.md");
    // A picked file's own odd name is made safe too, its bytes kept.
    let picked = root.join("picked");
    std::fs::create_dir_all(&picked).unwrap();
    std::fs::write(picked.join("Q3: plan?.pdf"), b"%PDF made up").unwrap();
    std::fs::write(picked.join("README.md"), b"# not the folder's README").unwrap();
    std::fs::write(picked.join("CON.txt"), b"made up").unwrap();
    let r = add_files(&v, &[picked.join("Q3: plan?.pdf"), picked.join("README.md"), picked.join("CON.txt")]);
    let paths = added(&r);
    for p in &paths {
        assert!(p.starts_with("raw_inbox/") && safe_everywhere(p.rsplit('/').next().unwrap()), "{p}");
    }
    assert_eq!(paths[1], "raw_inbox/README (2).md", "the folder's README is never a source's name");
    assert_eq!(read(&v, "raw_inbox/README (2).md"), "# not the folder's README");
}

#[test]
fn sources_folder_is_created_with_its_readme() {
    // K14-01
    let (_env, root) = crate::paths::testenv::folders("kn-src-folder");
    let t = fixture::twin(&root.join("twin"));
    std::fs::remove_dir_all(t.join("raw_inbox")).unwrap();
    let v = connect(&t);
    assert_eq!(v.path("sources"), None);
    let shown = value(&v).unwrap();
    assert_eq!((shown["folder"].clone(), shown["exists"].clone()), (json!("raw_inbox"), json!(false)));
    assert_eq!(shown["create"], "Echo will create raw_inbox/ with a README");
    let picked = root.join("a.txt");
    std::fs::write(&picked, "made up").unwrap();
    let a = add_files(&v, std::slice::from_ref(&picked)).remove(0).1.unwrap();
    assert_eq!(a.path, "raw_inbox/a.txt");
    let readme = crate::knowledge::starter::FILES.iter().find(|(p, _)| *p == "raw_inbox/README.md").unwrap().1;
    assert_eq!(read(&v, "raw_inbox/README.md"), readme);
    // Echo knows the folder now: it is the vault's sources, never pages.
    let v = knowledge::get(&v.id).unwrap();
    assert_eq!(v.path("sources").as_deref(), Some("raw_inbox"));
    assert!(value(&v).unwrap()["create"].is_null());
    // A second add leaves the README as it is; the README is never in the queue.
    std::fs::write(v.root.join("raw_inbox/README.md"), "# My own words\n").unwrap();
    add_files(&v, std::slice::from_ref(&picked)).remove(0).1.unwrap();
    assert_eq!(read(&v, "raw_inbox/README.md"), "# My own words\n");
    let q: Vec<String> = queue(&v).unwrap().into_iter().map(|s| s.path).collect();
    assert_eq!(q.len(), 2, "{q:?}");
    assert!(!q.iter().any(|p| p.ends_with("README.md")), "{q:?}");
}

#[test]
fn processed_lines_in_log_count_as_done() {
    let (_env, root) = crate::paths::testenv::folders("kn-src-log");
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "raw_inbox/Aurora RFP.pdf", "%PDF made up");
    fixture::write(&t, "raw_inbox/notes.txt", "made up");
    fixture::write(&t, "log.md", "# Log\n\n- 2026-11-01 · processed · raw_inbox/deck.pdf → knowledge/topics/deck.md\n- 2026-11-02 · processed · raw_inbox/Aurora RFP.pdf → knowledge/clients/aurora.md\n- 2026-11-03 · added · raw_inbox/notes.txt\n- 2026-11-03 · processed · raw_inbox/gone.pdf → knowledge/x.md\n");
    let v = connect(&t);
    // Even with the ledger saying New, a processed line means done.
    update_ledger(&v, |l| {
        l.sources.insert("raw_inbox/deck.pdf".into(), Entry { state: "new".into(), by: "file".into(), ..Default::default() });
        Ok(())
    })
    .unwrap();
    let state = |p: &str| queue(&v).unwrap().into_iter().find(|s| s.path == p).map(|s| s.state).unwrap_or_default();
    assert_eq!(state("raw_inbox/deck.pdf"), "done");
    assert_eq!(state("raw_inbox/Aurora RFP.pdf"), "done");
    assert_eq!(state("raw_inbox/notes.txt"), "new", "an `added` line isn't `processed`");
    assert_eq!(queue(&v).unwrap().len(), 3, "a processed line for a file that isn't there adds nothing");
}

#[test]
fn the_first_open_offers_a_baseline() {
    // K14-T6
    let (_env, root) = crate::paths::testenv::folders("kn-src-baseline");
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "raw_inbox/notes.txt", "made up");
    fixture::write(&t, "raw_inbox/echo/2026-01-05-sync.md", "---\ntitle: Planning sync\nkind: call\n---\n\n# Planning sync — minutes\n");
    fixture::write(&t, "raw_inbox/web/2026-01-06-article.md", "---\ntitle: Store opening notes\nkind: web\n---\n\n# Store opening notes\n");
    fixture::write(&t, "raw_inbox/.DS_Store", "x");
    fixture::write(&t, "raw_inbox/README.md", "# raw_inbox\n");
    fixture::write(&t, "log.md", "# Log\n\n- 2026-01-07 · processed · raw_inbox/deck.pdf → knowledge/topics/deck.md\n");
    let v = connect(&t);
    let q = queue(&v).unwrap();
    let mut kinds: Vec<(String, Kind, String)> = q.iter().map(|s| (s.path.clone(), s.kind, s.state.clone())).collect();
    kinds.sort();
    assert_eq!(kinds, [
        ("raw_inbox/deck.pdf".into(), Kind::File, "done".into()),
        ("raw_inbox/echo/2026-01-05-sync.md".into(), Kind::Echo, "new".into()),
        ("raw_inbox/notes.txt".into(), Kind::File, "new".into()),
        ("raw_inbox/web/2026-01-06-article.md".into(), Kind::Web, "new".into()),
    ]);
    assert_eq!(q.iter().find(|s| s.kind == Kind::Echo).unwrap().title, "Planning sync");
    assert_eq!(q.iter().find(|s| s.path.ends_with("notes.txt")).unwrap().title, "notes.txt");
    let shown = value(&v).unwrap();
    assert_eq!(shown["offer"], json!({"count": 3, "folder": "raw_inbox"}));
    assert_eq!(shown["sources"].as_array().unwrap().len(), 4);
    // *Mark them done*: every one Done, and no offer again.
    assert_eq!(baseline(&v, true).unwrap(), 3);
    assert!(queue(&v).unwrap().iter().all(|s| s.state == "done"));
    assert!(value(&v).unwrap()["offer"].is_null());
    assert!(ledger(&v).unwrap().baseline.is_some_and(|b| b.done));
    // *Treat them as new* on another vault: New, and no offer again either.
    let t2 = fixture::twin(&root.join("second"));
    fixture::write(&t2, "raw_inbox/notes.txt", "made up");
    let v2 = knowledge::add(&t2, AddOpts { name: Some("Second".into()), ..Default::default() }).unwrap();
    assert_eq!(value(&v2).unwrap()["offer"]["count"], 2);
    assert_eq!(baseline(&v2, false).unwrap(), 2);
    assert!(queue(&v2).unwrap().iter().all(|s| s.state == "new"));
    assert!(value(&v2).unwrap()["offer"].is_null());
    // Skip: done without a run, with its note; a path outside the sources is refused.
    skip(&v2, "raw_inbox/notes.txt", Some("Already in the client page")).unwrap();
    let s = queue(&v2).unwrap().into_iter().find(|s| s.path == "raw_inbox/notes.txt").unwrap();
    assert_eq!((s.state.as_str(), s.meta["note"].as_str()), ("skipped", Some("Already in the client page")));
    for bad in ["knowledge/CATALOG.md", "../twin/raw_inbox/notes.txt", "raw_inbox/../AGENTS.md", "raw_inbox/nope.pdf", "/etc/hosts", "raw_inbox/README.md"] {
        assert!(skip(&v2, bad, None).is_err(), "{bad}");
    }
}

#[test]
fn a_source_is_committed_alone() {
    // K14-T7
    let (_env, root) = crate::paths::testenv::folders("kn-src-git");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let c = crate::knowledge::starter::create("My twin", Some(root.join("vault")), true, "auto").unwrap();
    assert_eq!(c.git, "committed");
    let v = c.vault;
    std::fs::write(v.root.join("knowledge/profile/DIGITAL_PROFILE.md"), "# my unfinished edit\n").unwrap();
    let picked = root.join("Aurora RFP.pdf");
    std::fs::write(&picked, b"%PDF made up").unwrap();
    let a = add_files(&v, std::slice::from_ref(&picked)).remove(0).1.unwrap();
    assert!(!a.committed.starts_with("skipped") && a.committed != "off", "{}", a.committed);
    let shown = git::out(&v.root, &["show", "--name-only", "--format=%s", "HEAD"]).unwrap();
    assert_eq!(shown.lines().filter(|l| !l.is_empty()).collect::<Vec<_>>(), ["Add source: Aurora RFP.pdf", "raw_inbox/Aurora RFP.pdf"]);
    assert!(git::out(&v.root, &["status", "--porcelain"]).unwrap().contains(" M knowledge/profile/DIGITAL_PROFILE.md"), "the user's edit stays theirs");
    let acts = knowledge::activity::recent(&v, "2000-01-01T00:00:00+00:00");
    assert!(acts.iter().any(|x| x.path == a.path && x.commit.as_deref() == Some(a.committed.as_str())), "{acts:?}");
    // Commits off: written, nothing committed.
    let head = git::out(&v.root, &["rev-parse", "HEAD"]).unwrap();
    let v = knowledge::set(&v.id, "commit", "off").unwrap();
    let b = add_files(&v, std::slice::from_ref(&picked)).remove(0).1.unwrap();
    assert_eq!((b.path.as_str(), b.committed.as_str()), ("raw_inbox/Aurora RFP (2).pdf", "off"));
    assert!(v.root.join("raw_inbox/Aurora RFP (2).pdf").is_file());
    assert_eq!(git::out(&v.root, &["rev-parse", "HEAD"]).unwrap(), head);
}

#[test]
fn long_names_fit_both_systems() {
    // Fix round 1, item 4: a name near the 255-byte limit, in scripts of 2, 3 and 4 bytes a character.
    let (_env, root) = crate::paths::testenv::folders("kn-src-long");
    let v = twin(&root);
    let picked = root.join("picked");
    std::fs::create_dir_all(&picked).unwrap();
    let names = [format!("{}.pdf", "会议记录".repeat(20)), format!("{}.pdf", "🚀".repeat(61)), format!("{}.txt", "Протокол".repeat(15) + "Про")];
    for n in &names {
        assert!(n.len() <= 255, "the test's own file can be made: {}", n.len());
        std::fs::write(picked.join(n), n.as_bytes()).unwrap();
    }
    let files: Vec<std::path::PathBuf> = names.iter().flat_map(|n| [picked.join(n), picked.join(n)]).collect();
    let fits = |p: &str| {
        let name = p.rsplit('/').next().unwrap();
        name.len() <= 200 && name.encode_utf16().count() <= 200 && safe_everywhere(name)
    };
    for (i, (_, r)) in add_files(&v, &files).into_iter().enumerate() {
        let a = r.unwrap_or_else(|e| panic!("{}: {e:#}", names[i / 2]));
        assert!(fits(&a.path), "{} → {}", names[i / 2], a.path);
        assert_eq!(std::fs::read(knowledge::join(&v.root, &a.path)).unwrap(), names[i / 2].as_bytes());
        let stem: String = names[i / 2].chars().take(5).collect();
        assert!(a.path.contains(&stem) && (i % 2 == 0 || a.path.contains(" (2)")), "{}", a.path);
    }
    // A title in a script of 4-byte letters makes a dated name that fits too.
    let subject = "𠀀𠀁𠀂𠀃".repeat(20);
    let m = item(&graph_mail("m-long", &subject, ("Lena Hartmann", "lena@northwind.example"), &[], "2026-11-04T08:05:00Z", "<p>Made-up text.</p>", &[]));
    for _ in 0..2 {
        let a = add_thread(&v, &m, std::slice::from_ref(&m)).unwrap();
        assert!(fits(&a.path), "{}", a.path);
    }
}

#[cfg(unix)]
#[test]
fn a_link_at_a_temporary_name_is_never_followed() {
    // Fix round 1, item 5: the copy never writes through a link planted where its temporary file goes.
    let (_env, root) = crate::paths::testenv::folders("kn-src-link");
    let v = twin(&root);
    let outside = root.join("outside.txt");
    std::fs::write(&outside, "keep").unwrap();
    let inbox = v.root.join("raw_inbox");
    let pid = std::process::id();
    std::os::unix::fs::symlink(&outside, inbox.join(format!(".a.pdf.{pid}.tmp"))).unwrap();
    for n in 0..200 {
        std::os::unix::fs::symlink(&outside, inbox.join(format!(".a.pdf.{pid}.{n}.tmp"))).unwrap();
    }
    let picked = root.join("a.pdf");
    std::fs::write(&picked, b"%PDF made up").unwrap();
    let r = add_files(&v, std::slice::from_ref(&picked)).remove(0).1;
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "keep", "nothing written through the link");
    let a = r.unwrap();
    let full = knowledge::join(&v.root, &a.path);
    assert!(!std::fs::symlink_metadata(&full).unwrap().file_type().is_symlink(), "the source is a file of its own");
    assert_eq!(std::fs::read(&full).unwrap(), b"%PDF made up");
}

#[test]
fn a_new_source_never_writes_over_a_file() {
    // Fix round 1, item 5: a file that appeared at the chosen name after it was chosen stays as it is.
    let (_env, root) = crate::paths::testenv::folders("kn-src-put");
    std::fs::write(root.join("x.md"), "theirs").unwrap();
    assert!(put_new(&root.join("x.md"), |f| std::io::Write::write_all(f, b"mine")).is_err());
    assert_eq!(std::fs::read_to_string(root.join("x.md")).unwrap(), "theirs");
    put_new(&root.join("y.md"), |f| std::io::Write::write_all(f, b"mine")).unwrap();
    assert_eq!(std::fs::read_to_string(root.join("y.md")).unwrap(), "mine");
    let left: Vec<String> = std::fs::read_dir(&root).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".tmp")).collect();
    assert!(left.is_empty(), "no temporary file left: {left:?}");
}

#[test]
fn a_name_the_ledger_or_the_log_knows_is_taken() {
    // Fix round 1, item 6: a new a.pdf where log.md once processed another isn't Done at once.
    let (_env, root) = crate::paths::testenv::folders("kn-src-reused");
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "log.md", "# Log\n\n- 2026-11-01 · processed · raw_inbox/a.pdf → knowledge/topics/a.md\n");
    let v = connect(&t);
    update_ledger(&v, |l| {
        l.sources.insert("raw_inbox/b.pdf".into(), Entry { state: "done".into(), by: "file".into(), ..Default::default() });
        Ok(())
    })
    .unwrap();
    let picked = root.join("picked");
    std::fs::create_dir_all(&picked).unwrap();
    std::fs::write(picked.join("a.pdf"), b"new a").unwrap();
    std::fs::write(picked.join("b.pdf"), b"new b").unwrap();
    let paths = added(&add_files(&v, &[picked.join("a.pdf"), picked.join("b.pdf")]));
    assert_eq!(paths, ["raw_inbox/a (2).pdf", "raw_inbox/b (2).pdf"]);
    let q = queue(&v).unwrap();
    assert!(q.iter().filter(|s| paths.contains(&s.path)).all(|s| s.state == "new"), "{q:?}");
}

#[test]
fn the_first_open_question_waits_for_its_answer() {
    // Ruling R40: an add or a skip before the answer doesn't take the question away.
    let (_env, root) = crate::paths::testenv::folders("kn-src-offer");
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "raw_inbox/notes.txt", "made up");
    fixture::write(&t, "raw_inbox/brief.txt", "made up");
    fixture::write(&t, "log.md", "# Log\n\n- 2026-01-07 · processed · raw_inbox/deck.pdf → knowledge/topics/deck.md\n");
    let v = connect(&t);
    assert_eq!(value(&v).unwrap()["offer"], json!({"count": 2, "folder": "raw_inbox"}));
    let page = add_page(&v, "https://contoso.example/notes", "Store opening notes", "The new store opens on 4 December.").unwrap();
    assert!(page.path.starts_with("raw_inbox/web/"));
    assert_eq!(value(&v).unwrap()["offer"], json!({"count": 2, "folder": "raw_inbox"}), "an add first: the question stands");
    skip(&v, "raw_inbox/notes.txt", None).unwrap();
    assert_eq!(value(&v).unwrap()["offer"]["count"], 1, "a skipped file is answered");
    assert_eq!(baseline(&v, true).unwrap(), 1);
    assert!(value(&v).unwrap()["offer"].is_null(), "answered: gone");
    let s = queue(&v).unwrap();
    assert_eq!(s.iter().find(|x| x.path == page.path).unwrap().state, "new", "what was added stays New");
    // *Treat them as new*, after a skip: gone too.
    let t2 = fixture::twin(&root.join("second"));
    let v2 = knowledge::add(&t2, AddOpts { name: Some("Second".into()), ..Default::default() }).unwrap();
    skip(&v2, "raw_inbox/deck.pdf", None).unwrap();
    fixture::write(&t2, "raw_inbox/later.txt", "made up");
    assert_eq!(value(&v2).unwrap()["offer"]["count"], 1);
    baseline(&v2, false).unwrap();
    assert!(value(&v2).unwrap()["offer"].is_null());
}

#[test]
fn a_chat_defaults_to_the_last_seven_days() {
    // Fix round 1, item 7: seven days, today included.
    let d = |m: u32, day: u32| chrono::NaiveDate::from_ymd_opt(2026, m, day).unwrap();
    assert_eq!(last_week(d(11, 7)), d(11, 1));
    assert_eq!(last_week(d(3, 2)), d(2, 24));
}

#[test]
fn only_chats_and_mail_can_be_added() {
    // Fix round 1, item 11: a channel's post carries a chat id too, and isn't a chat.
    assert!(addable(&json!({"kind": "mail", "id": "m1"})));
    assert!(addable(&json!({"kind": "chats", "chat": "19:a@thread.v2", "channel": null})));
    assert!(!addable(&json!({"kind": "chats", "chat": "19:general@thread.tacv2", "team": "t1", "channel": "19:general@thread.tacv2"})));
    assert!(!addable(&json!({"kind": "chats", "chat": null})));
    assert!(!addable(&json!({"kind": "files", "id": "f1"})));
}

//! Reading, editing, history and index rows of vault pages (spec K5, K6, K8-05 fix), on made-up vaults.

use super::page::*;
use super::{fixture, git, AddOpts};

fn twin(root: &std::path::Path) -> super::Vault {
    let t = fixture::twin(&root.join("twin"));
    super::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
}

#[test]
fn a_page_reads_with_its_links_and_backlinks() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-read");
    let v = twin(&root);
    let p = read(&v, "knowledge/clients/acme.md").unwrap();
    assert_eq!(p["title"], "Acme Corp");
    assert_eq!(p["tags"], serde_json::json!(["client"]));
    let html = p["html"].as_str().unwrap();
    assert!(html.contains("href=\"#kn:knowledge/notes/2026-01-01-a.md\""), "{html}");
    assert!(!html.contains("title: Acme Corp"), "frontmatter isn't in the body");
    let back: Vec<&str> = p["backlinks"].as_array().unwrap().iter().map(|b| b["path"].as_str().unwrap()).collect();
    assert_eq!(back, ["knowledge/CATALOG.md", "knowledge/notes/2026-01-01-a.md"]);
    assert_eq!(p["backlinks"][1]["line"], "With Acme.", "the line reads as text, not Markdown");
    assert_eq!(p["in_index"], true);
    assert!(read(&v, "knowledge/CATALOG.md").unwrap()["in_index"].is_null(), "the index isn't asked to list itself");
    assert!(read(&v, "knowledge/notes/README.md").unwrap()["in_index"].is_null(), "nor a folder's README");
    assert!(p["base"].as_str().unwrap().len() == 64);
    assert!(p["obsidian"].as_str().unwrap().starts_with("obsidian://open?path="));
    let local = &p["local"];
    assert!(local["nodes"].as_array().unwrap().len() >= 3);
}

#[test]
fn backlink_lines_read_as_text() {
    assert_eq!(plain_line("| [2026-10-29-sync.md](notes/2026-10-29-sync.md) | ~18m Sync — A, B |"), "2026-10-29-sync.md · ~18m Sync — A, B");
    assert_eq!(plain_line("- With [[acme|Acme Corp]] and **bold** `code`."), "With Acme Corp and bold code.");
    assert_eq!(plain_line("* See [[clients/acme]] and [[b#Part]]"), "See acme and b");
    assert_eq!(plain_line("> quoted [x](<a b.md>)"), "quoted x");
    assert_eq!(plain_line("## Heading"), "Heading");
    assert_eq!(plain_line("![chart](a.png) after"), "after");
    assert_eq!(plain_line("1. a_b_c and 2*3"), "a_b_c and 2*3", "lone marks stay");
    assert_eq!(plain_line("|---|---|"), "");
}

#[test]
fn line_breaks_read_as_obsidian_shows_them() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-breaks");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/topics/lines.md", "> one\n> two\n\nthree\nfour\n");
    let html = |v: &super::Vault| read(v, "knowledge/topics/lines.md").unwrap()["html"].as_str().unwrap().to_string();
    let h = html(&v);
    assert!(h.contains("one<br>") && h.contains("three<br>"), "Obsidian's default: a newline is a line break: {h}");
    fixture::write(&v.root, "knowledge/.obsidian/app.json", "{\"strictLineBreaks\": true}");
    let strict = super::reread(&v.id).unwrap();
    assert!(!html(&strict).contains("<br>"), "strict line breaks on: CommonMark");
    std::fs::remove_dir_all(v.root.join("knowledge/.obsidian")).unwrap();
    let plain = super::reread(&v.id).unwrap();
    assert!(!html(&plain).contains("<br>"), "not an Obsidian vault: CommonMark");
}

#[test]
fn an_index_row_describes_a_page_by_its_prose() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-rowdesc");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/clients/team.md", "---\ntitle: The team\n---\n# The team\n\n| Name | Role |\n|---|---|\n| A | B |\n\n```\ncode\n```\n\nWho works on **Acme** with [us](x.md), and how.\nSecond line.\n\nMore later.\n");
    fixture::write(&v.root, "knowledge/clients/table.md", "---\ntitle: Only a table\n---\n| a | b |\n|---|---|\n");
    fixture::write(&v.root, "knowledge/clients/described.md", "---\ntitle: Described\ndescription: Its own words\n---\nBody text.\n");
    let row = |rel: &str| index_rows(&v, &[rel.into()]).unwrap()[0]["row"].as_str().unwrap().to_string();
    assert!(row("knowledge/clients/team.md").contains("| The team — Who works on Acme with us, and how. Second line. |"), "{}", row("knowledge/clients/team.md"));
    assert!(row("knowledge/clients/table.md").contains("| Only a table |"), "no prose: the title alone");
    assert!(row("knowledge/clients/described.md").contains("| Described — Its own words |"), "a description property first");
}

#[test]
fn page_names_with_percent_signs_link_safely() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-percent");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/topics/50% plan.md", "# Half\n");
    fixture::write(&v.root, "knowledge/topics/a%20b.md", "# Literal\n");
    fixture::write(&v.root, "knowledge/topics/z.md", "# Z\n\n[[50% plan]] [[a%20b]] [[new 10%]]\n");
    let html = read(&v, "knowledge/topics/z.md").unwrap()["html"].as_str().unwrap().to_string();
    assert!(html.contains("href=\"#kn:knowledge/topics/50%25%20plan.md\""), "{html}");
    assert!(html.contains("href=\"#kn:knowledge/topics/a%2520b.md\""), "the % is the name's own: {html}");
    assert!(html.contains("href=\"#kn-new:new%2010%25\""), "{html}");
}

#[test]
fn new_page_properties_stay_valid_yaml() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-yaml");
    let c = super::starter::create("Fresh", Some(root.join("fresh")), false, "auto").unwrap();
    let v = c.vault;
    for (name, line) in [("Q4: pricing", "title: \"Q4: pricing\""), ("#2 retro", "title: \"#2 retro\""), ("[draft] plan", "title: \"[draft] plan\""), ("Plain name", "title: Plain name")] {
        let p = create(&v, "knowledge/topics", name, None, false).unwrap();
        let text = std::fs::read_to_string(v.root.join(&p)).unwrap();
        assert!(text.lines().any(|l| l == line), "{name}: {text}");
    }
}

#[test]
fn a_template_with_a_percent_sign_never_panics() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-tplpct");
    let c = super::starter::create("Fresh", Some(root.join("fresh")), false, "auto").unwrap();
    let v = c.vault;
    fixture::write(&v.root, "knowledge/templates/odd.md", "# {{title}}\n{{date:YYYY-MM-DD}} — 100% done, {{date:50% %Q}}\n");
    let p = create(&v, "knowledge/topics", "Odd", Some("knowledge/templates/odd.md"), false).unwrap();
    let text = std::fs::read_to_string(v.root.join(&p)).unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(text.contains(&format!("{today} — 100% done, 50% %Q")), "{text}");
}

#[test]
fn a_page_that_isnt_utf8_is_read_but_not_edited() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-latin1");
    let v = twin(&root);
    std::fs::create_dir_all(v.root.join("knowledge/topics")).unwrap();
    std::fs::write(v.root.join("knowledge/topics/latin.md"), b"# Caf\xe9\n").unwrap();
    let p = read(&v, "knowledge/topics/latin.md").unwrap();
    assert_eq!(p["editable"], false, "editing would change bytes it never showed");
    let e = save(&v, "knowledge/topics/latin.md", "# Cafe\n", p["base"].as_str().unwrap(), None).unwrap_err().to_string();
    assert!(e.contains("UTF-8"), "{e}");
    assert_eq!(std::fs::read(v.root.join("knowledge/topics/latin.md")).unwrap(), b"# Caf\xe9\n");
}

#[test]
fn restore_never_overwrites_uncommitted_changes() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-restore-dirty");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let v = twin(&root);
    assert!(git::tests::repo(&v.root));
    let v = super::set(&v.id, "commit", "true").unwrap();
    let path = "knowledge/clients/acme.md";
    let base = read(&v, path).unwrap()["base"].as_str().unwrap().to_string();
    save(&v, path, "# Acme v2\n", &base, None).unwrap();
    let first = history(&v, path)["versions"].as_array().unwrap().last().unwrap()["rev"].as_str().unwrap().to_string();
    std::fs::write(v.root.join(path), "# Acme, my edit not yet committed\n").unwrap();
    let e = restore(&v, path, &first).unwrap_err().to_string();
    assert!(e.contains("not committed"), "{e}");
    assert_eq!(std::fs::read_to_string(v.root.join(path)).unwrap(), "# Acme, my edit not yet committed\n");
}

#[cfg(unix)]
#[test]
fn a_link_above_a_missing_folder_cannot_lead_out() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-symlink-deep");
    let v = twin(&root);
    let outside = root.join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, v.root.join("knowledge/link")).unwrap();
    assert!(inside(&v, "knowledge/link/new/deeper.md").is_err());
    assert!(create(&v, "knowledge/link/new", "Escape", None, false).is_err());
    assert!(!outside.join("new").exists(), "nothing written outside the vault");
}

#[test]
fn hostile_markup_is_text() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-hostile");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/topics/x.md", "# X\n\n<script>alert(1)</script>\n\n[js](javascript:alert(1)) [file](file:///etc/passwd) [web](https://example.com) <img src=x onerror=alert(1)>\n\n`[[not]]` and\n\n```\n[[also not]]\n```\n");
    let html = read(&v, "knowledge/topics/x.md").unwrap()["html"].as_str().unwrap().to_string();
    assert!(!html.contains("<script>") && html.contains("&lt;script&gt;"), "{html}");
    assert!(!html.contains("javascript:") || !html.contains("href=\"javascript:"), "{html}");
    assert!(!html.contains("href=\"file:"), "{html}");
    assert!(html.contains("href=\"https://example.com\""), "{html}");
    assert!(!html.contains("<img src=x"), "{html}");
    assert!(!html.contains("kn-new:not") && !html.contains("kn-new:also"), "code stays code: {html}");
}

#[test]
fn page_links_route_inside_echo() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-route");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/topics/y.md", "# Y\n\n[[acme]] [[acme|the client]] [[missing page]] [deck](../../raw_inbox/deck.pdf) [call](echo://call/2026-11-05_090012-x)\n\n![[chart.png]]\n");
    fixture::write(&v.root, "knowledge/topics/chart.png", "\u{89}PNG fake");
    let html = read(&v, "knowledge/topics/y.md").unwrap()["html"].as_str().unwrap().to_string();
    assert!(html.contains("<a href=\"#kn:knowledge/clients/acme.md\" data-kn-link>acme</a>"), "{html}");
    assert!(html.contains(">the client</a>"), "{html}");
    assert!(html.contains("href=\"#kn-new:missing%20page\""), "a missing page offers to make it: {html}");
    assert!(html.contains("data-echo-link"), "{html}");
    assert!(html.contains("src=\"api/kn/twin/asset?path=knowledge%2Ftopics%2Fchart.png\""), "{html}");
}

#[test]
fn save_refuses_when_the_file_changed_on_disk() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-save");
    let v = twin(&root);
    let p = read(&v, "knowledge/clients/acme.md").unwrap();
    let base = p["base"].as_str().unwrap().to_string();
    std::fs::write(v.root.join("knowledge/clients/acme.md"), "# Changed in Obsidian\n").unwrap();
    let e = save(&v, "knowledge/clients/acme.md", "# Mine\n", &base, None).unwrap_err().to_string();
    assert!(e.contains("changed_on_disk"), "{e}");
    assert_eq!(std::fs::read_to_string(v.root.join("knowledge/clients/acme.md")).unwrap(), "# Changed in Obsidian\n");
    let fresh = read(&v, "knowledge/clients/acme.md").unwrap()["base"].as_str().unwrap().to_string();
    let r = save(&v, "knowledge/clients/acme.md", "# Mine\n", &fresh, None).unwrap();
    assert_eq!(std::fs::read_to_string(v.root.join("knowledge/clients/acme.md")).unwrap(), "# Mine\n");
    assert_eq!(r["commit"], "off");
    assert_ne!(r["base"], serde_json::json!(fresh));
}

#[test]
fn crlf_is_kept() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-crlf");
    let v = twin(&root);
    std::fs::write(v.root.join("knowledge/topics.md"), "# A\r\n\r\nline\r\n").unwrap();
    let base = read(&v, "knowledge/topics.md").unwrap()["base"].as_str().unwrap().to_string();
    // A textarea hands back \n.
    save(&v, "knowledge/topics.md", "# A\n\nline two\n", &base, None).unwrap();
    assert_eq!(std::fs::read(v.root.join("knowledge/topics.md")).unwrap(), b"# A\r\n\r\nline two\r\n");
}

#[test]
fn save_commits_one_file() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-commit");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let v = twin(&root);
    assert!(git::tests::repo(&v.root));
    let v = super::set(&v.id, "commit", "true").unwrap();
    std::fs::write(v.root.join("knowledge/notes/2026-01-02-b.md"), "# someone else's unsaved change\n").unwrap();
    let base = read(&v, "knowledge/clients/acme.md").unwrap()["base"].as_str().unwrap().to_string();
    let r = save(&v, "knowledge/clients/acme.md", "# Acme Corp\n\nNew line.\n", &base, Some("Edit knowledge/clients/acme.md: a line")).unwrap();
    assert!(!r["commit"].as_str().unwrap().starts_with("skipped"), "{r}");
    let files = git::out(&v.root, &["show", "--name-only", "--format=%s", "HEAD"]).unwrap();
    let lines: Vec<&str> = files.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, ["Edit knowledge/clients/acme.md: a line", "knowledge/clients/acme.md"]);
}

#[test]
fn new_page_from_template_with_a_row() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-new");
    let c = super::starter::create("Fresh", Some(root.join("fresh")), false, "auto").unwrap();
    let v = c.vault;
    let path = create(&v, "knowledge/topics", "Pricing model", Some("knowledge/templates/page.md"), true).unwrap();
    assert_eq!(path, "knowledge/topics/pricing-model.md");
    let text = std::fs::read_to_string(v.root.join(&path)).unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(text.contains("title: Pricing model\n") && text.contains(&format!("date: {today}\n")) && text.contains("# Pricing model\n"), "{text}");
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    let topics = &cat[cat.find("## Topics").unwrap()..cat.find("## Activities").unwrap()];
    assert!(topics.contains("| [pricing-model.md](topics/pricing-model.md) | Pricing model |"), "{topics}");
    assert!(create(&v, "knowledge/topics", "Pricing model", None, false).unwrap_err().to_string().contains("already"));
    // Without a template: the vault's page format.
    let p2 = create(&v, "knowledge/clients", "Acme Corp", None, true).unwrap();
    assert!(std::fs::read_to_string(v.root.join(&p2)).unwrap().starts_with("---\ntitle: Acme Corp\n"));
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    let clients = &cat[cat.find("## Clients").unwrap()..cat.find("## Notes").unwrap()];
    assert!(clients.contains("(clients/acme-corp.md)"), "{clients}");
}

#[test]
fn history_and_restore() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-history");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let v = twin(&root);
    assert!(git::tests::repo(&v.root));
    let v = super::set(&v.id, "commit", "true").unwrap();
    let path = "knowledge/clients/acme.md";
    for text in ["# Acme v2\n", "# Acme v3\n"] {
        let base = read(&v, path).unwrap()["base"].as_str().unwrap().to_string();
        save(&v, path, text, &base, None).unwrap();
    }
    let h = history(&v, path);
    let revs = h["versions"].as_array().unwrap();
    assert_eq!(revs.len(), 3, "{h}");
    assert_eq!(revs[0]["message"], format!("Edit {path}"));
    let first = revs[2]["rev"].as_str().unwrap().to_string();
    let ver = version(&v, path, &first).unwrap();
    assert!(ver["text"].as_str().unwrap().contains("title: Acme Corp"));
    assert!(ver["diff"].as_str().unwrap().contains("+title: Acme Corp") || ver["diff"].as_str().unwrap().contains("+# Acme Corp"), "{ver}");
    restore(&v, path, &first).unwrap();
    assert!(std::fs::read_to_string(v.root.join(path)).unwrap().contains("title: Acme Corp"));
    assert_eq!(history(&v, path)["versions"].as_array().unwrap().len(), 4, "a restore is a new commit");
}

#[test]
fn no_git_no_history() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-nohist");
    let v = twin(&root);
    assert_eq!(history(&v, "knowledge/clients/acme.md")["no_history"], true);
}

#[test]
fn index_rows_preview_then_add() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-rows");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/clients/beta.md", "---\ntitle: Beta Inc\ndate: 2026-02-01\nsummary: \"A made-up client\"\n---\n# Beta\n");
    let rows = index_rows(&v, &["knowledge/clients/beta.md".into(), "knowledge/notes/2026-01-03-c.md".into()]).unwrap();
    assert_eq!(rows[0]["section"], "Clients");
    assert_eq!(rows[0]["row"], "| [beta.md](clients/beta.md) | Beta Inc — A made-up client | 2026-02-01 |");
    assert_eq!(rows[1]["section"], "Notes");
    let r = index_add(&v, &["knowledge/clients/beta.md".into(), "knowledge/notes/2026-01-03-c.md".into()]).unwrap();
    assert_eq!(r["added"], 2);
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    assert!(cat.contains("(clients/beta.md)") && cat.contains("(notes/2026-01-03-c.md)"));
    let ix = super::scan::index(&v).unwrap();
    assert!(super::checks::run(&v, &ix).not_in_index.iter().all(|p| p != "clients/beta.md" && p != "notes/2026-01-03-c.md"));
}

#[test]
fn assets_only_images_inside_the_vault() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-asset");
    let v = twin(&root);
    fixture::write(&v.root, "knowledge/img/a.png", "\u{89}PNG fake");
    fixture::write(&v.root, "knowledge/img/a.svg", "<svg onload=alert(1)>");
    let (bytes, mime) = asset(&v, "knowledge/img/a.png").unwrap();
    assert_eq!(mime, "image/png");
    assert!(!bytes.is_empty());
    assert!(asset(&v, "knowledge/img/a.svg").is_err());
    assert!(asset(&v, "processing_log.json").is_err());
    assert!(asset(&v, "../outside.png").is_err());
}

#[test]
fn paths_cannot_leave_the_vault() {
    let (_env, root) = crate::paths::testenv::folders("kn-pg-paths");
    let v = twin(&root);
    fixture::write(&root, "outside.md", "# secret\n");
    for bad in ["../outside.md", "/etc/passwd", "knowledge/../../outside.md", "C:\\Windows\\win.ini", "knowledge/CON.md", ".git/config", "knowledge/.obsidian/app.json", "processing_log.json"] {
        assert!(read(&v, bad).is_err(), "{bad}");
        assert!(save(&v, bad, "x", "", None).is_err(), "{bad}");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("outside.md"), v.root.join("knowledge/link.md")).unwrap();
        assert!(read(&v, "knowledge/link.md").is_err(), "a link leading out");
    }
    // The schema files and skills are readable and editable.
    assert!(read(&v, "AGENTS.md").is_ok());
    assert!(read(&v, ".claude/skills/s/SKILL.md").is_ok());
}

#[test]
fn page_writes_leave_an_activity_line() {
    use super::activity::recent;
    let (_env, root) = crate::paths::testenv::folders("kn-pg-activity");
    let git_here = git::program().is_some();
    let _iso = git_here.then(|| git::tests::isolated(&root, true));
    let v = twin(&root);
    let all = |v: &super::Vault| {
        let mut e = recent(v, "2000-01-01T00:00:00+00:00");
        e.reverse();
        e
    };
    assert!(all(&v).is_empty());

    // A save: one "edit" by the editor, counted in lines.
    let path = "knowledge/clients/acme.md";
    let base = read(&v, path).unwrap()["base"].as_str().unwrap().to_string();
    save(&v, path, "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp\n\nSee [[2026-01-01-a]].\nOne more line.\n", &base, None).unwrap();
    let e = all(&v);
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].path.as_str(), e[0].kind.as_str(), e[0].by.as_str(), e[0].plus, e[0].minus), (path, "edit", "editor", 1, 0));
    assert_eq!(e[0].commit, None, "this vault doesn't commit");

    // A new page.
    let rel = create(&v, "knowledge/topics", "Pricing ideas", None, false).unwrap();
    let e = all(&v);
    assert_eq!(e.len(), 2);
    assert_eq!((e[1].path.as_str(), e[1].kind.as_str(), e[1].by.as_str(), e[1].minus), (rel.as_str(), "new", "new-page", 0));
    assert!(e[1].plus > 3);

    // Index rows for a page the index lacks.
    index_add(&v, &["knowledge/notes/2026-01-03-c.md".to_string()]).unwrap();
    let e = all(&v);
    assert_eq!(e.len(), 3);
    assert_eq!((e[2].path.as_str(), e[2].kind.as_str(), e[2].by.as_str()), ("knowledge/CATALOG.md", "edit", "index-rows"));
    assert!(e[2].plus >= 1);

    // A restore (needs history): the version is the ref, and the commit is the new commit.
    if git_here {
        assert!(git::tests::repo(&v.root));
        let v = super::set(&v.id, "commit", "true").unwrap();
        let p = "knowledge/notes/2026-01-02-b.md";
        let base = read(&v, p).unwrap()["base"].as_str().unwrap().to_string();
        save(&v, p, "# Second sync, changed\n", &base, None).unwrap();
        let first = history(&v, p)["versions"].as_array().unwrap().last().unwrap()["rev"].as_str().unwrap().to_string();
        restore(&v, p, &first).unwrap();
        let e = all(&v);
        assert_eq!(e.len(), 5);
        assert!(e[3].commit.is_some(), "a save that was committed names the commit");
        assert_eq!((e[4].path.as_str(), e[4].by.as_str(), e[4].r#ref.as_deref()), (p, "editor", Some(first.as_str())));
        assert!(e[4].commit.is_some());
    }

    // A minutes save in a vault Echo made.
    let m = super::starter::create("Activity twin", Some(root.join("act-twin")), false, "auto").unwrap().vault;
    let call = fixture::call("2026-11-05_090012-weekly-delivery-sync", "Weekly delivery sync", "system", fixture::MINUTES);
    super::export::save(&call, &m.id, super::export::How::Auto).unwrap();
    let e: Vec<_> = recent(&m, "2000-01-01T00:00:00+00:00").into_iter().filter(|e| e.by != "starter").collect();
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].by.as_str(), e[0].kind.as_str(), e[0].path.as_str()), ("minutes", "new", "knowledge/notes/2026-11-05-weekly-delivery-sync.md"));
    assert_eq!(e[0].r#ref.as_deref(), Some("2026-11-05_090012-weekly-delivery-sync"));
}

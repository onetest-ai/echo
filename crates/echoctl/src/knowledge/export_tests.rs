//! The minutes → vault tests (spec K3), on made-up calls and vaults.

use super::export::*;
use super::{fixture, git, AddOpts};
use std::path::{Path, PathBuf};

const CALL: &str = "2026-11-05_090012-weekly-delivery-sync";

fn starter(root: &Path, name: &str) -> super::Vault {
    let c = super::starter::create(name, Some(root.join(crate::recorder::slugify(name))), false, "auto").unwrap();
    // Connected-vault defaults unless a test turns commits on.
    super::set(&c.vault.id, "commit", "false").unwrap()
}

fn note_path(v: &super::Vault) -> PathBuf {
    v.root.join("knowledge/notes/2026-11-05-weekly-delivery-sync.md")
}

const GOLDEN: &str = "---\ntitle: \"Weekly delivery sync\"\ndate: 2026-11-05\nmeeting: \"Weekly delivery sync (MS Teams)\"\ntags: [meeting]\nparticipants: Alex Doe, Dana Reyes, Sam Hale\nduration: 46m\nsource: echo://call/2026-11-05_090012-weekly-delivery-sync\norigin: echo\necho_call: 2026-11-05_090012\nsummary: \"The team agreed to move the pilot | rollout to 18 November and to keep the pricing page as it is.\"\n---\n\n# Weekly delivery sync — 2026-11-05\n\n## Summary\n\nThe team agreed to move the pilot | rollout to 18 November and to keep the pricing page as it is.\n\n## Decisions\n\n- Pilot moves to 18 November.\n\n## Action items\n\n- [ ] Me — send the revised plan to Dana by Friday.\n\n## Open questions\n\n- Who owns the data migration?\n";

#[test]
fn a_call_becomes_a_note_and_a_row() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-golden");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    assert_eq!(call_value(&call)["profile"], "system", "the chip knows which vaults can take the call");
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert_eq!(s.state, "saved");
    assert_eq!(s.path, "knowledge/notes/2026-11-05-weekly-delivery-sync.md");
    assert_eq!(std::fs::read_to_string(note_path(&v)).unwrap(), GOLDEN);
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    assert!(cat.contains("## Notes\n\n| File | Description | Date Added |\n|------|-------------|------------|\n| [2026-11-05-weekly-delivery-sync.md](notes/2026-11-05-weekly-delivery-sync.md) | 46m Weekly delivery sync — The team agreed to move the pilot / rollout to 18 November and to keep the pricing page as it is. | 2026-11-05 |\n"), "{cat}");
    assert!(cat.contains(&format!("Last update: {}", chrono::Local::now().format("%Y-%m-%d"))));
    assert_eq!(s.index, "added");
    assert_eq!(s.commit, "off", "commits are off for this vault");
    let log = std::fs::read_to_string(v.root.join("log.md")).unwrap();
    assert!(log.trim_end().ends_with("· added · knowledge/notes/2026-11-05-weekly-delivery-sync.md · minutes of \"Weekly delivery sync\" from Echo"), "{log}");
    let rec = record(&call);
    assert_eq!(rec["saved"][0]["vault"], v.id);
    assert_eq!(rec["saved"][0]["how"], "auto");
}

#[test]
fn saving_twice_adds_nothing() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-twice");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    save(&call, &v.id, How::Auto).unwrap();
    let cat1 = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert_eq!(s.state, "unchanged");
    assert_eq!(std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap(), cat1);
    assert_eq!(record(&call)["saved"].as_array().unwrap().len(), 1);
}

#[test]
fn an_edited_note_is_kept() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-kept");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    save(&call, &v.id, How::Auto).unwrap();
    let mut edited = std::fs::read_to_string(note_path(&v)).unwrap();
    edited.push_str("\nMy own line.\n");
    std::fs::write(note_path(&v), &edited).unwrap();
    std::fs::write(call.join("notes.md"), fixture::MINUTES.replace("18 November", "25 November")).unwrap();
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert_eq!(s.state, "kept");
    assert_eq!(std::fs::read_to_string(note_path(&v)).unwrap(), edited);
    assert_eq!(record(&call)["saved"][0]["state"], "kept");
    // The user's choices: replace it…
    let r = save(&call, &v.id, How::manual().replace()).unwrap();
    assert_eq!(r.state, "updated");
    assert!(std::fs::read_to_string(note_path(&v)).unwrap().contains("25 November"));
    // …or, after another edit, keep it and save a new note beside it (which becomes this call's note here).
    std::fs::write(note_path(&v), "my own version\n").unwrap();
    std::fs::write(call.join("notes.md"), fixture::MINUTES.replace("18 November", "2 December")).unwrap();
    assert_eq!(save(&call, &v.id, How::Auto).unwrap().state, "kept");
    let n = save(&call, &v.id, How::manual().new_note()).unwrap();
    assert_eq!(n.path, "knowledge/notes/2026-11-05-weekly-delivery-sync-2.md");
    assert!(std::fs::read_to_string(v.root.join(&n.path)).unwrap().contains("2 December"));
    assert_eq!(std::fs::read_to_string(note_path(&v)).unwrap(), "my own version\n");
    assert!(std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap().contains("(notes/2026-11-05-weekly-delivery-sync-2.md)"));
    assert_eq!(record(&call)["saved"][0]["path"], "knowledge/notes/2026-11-05-weekly-delivery-sync-2.md");
}

#[test]
fn no_notes_table_skips_the_row_with_a_reason() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-notable");
    let v = starter(&root, "My twin");
    std::fs::write(v.root.join("knowledge/CATALOG.md"), "# Catalog\n\nNo tables here.\n").unwrap();
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert_eq!(s.state, "saved");
    assert!(s.index.starts_with("skipped: no Notes table"), "{}", s.index);
    assert!(note_path(&v).is_file());
}

#[test]
fn other_profiles_are_never_saved_automatically() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-profile");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "client-x", fixture::MINUTES);
    assert!(targets(&call).is_empty());
    assert!(save_by_rules(&call).is_empty());
    assert!(!note_path(&v).exists());
    let e = save(&call, &v.id, How::manual()).unwrap_err();
    let m = e.downcast_ref::<ProfileMismatch>().expect("a profile mismatch");
    assert_eq!((m.call.as_str(), m.vault.as_str()), ("client-x", "system"));
    let s = save(&call, &v.id, How::manual().anyway()).unwrap();
    assert_eq!(s.state, "saved");
    assert_eq!(record(&call)["saved"][0]["anyway"], true);
}

#[test]
fn scope_limits_which_calls() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-scope");
    let v = starter(&root, "My twin");
    let client = crate::projects::add_client("Northwind Retail", &["northwind.example".into()], false).unwrap();
    let thread = crate::projects::create("Checkout", Some(&client.id), None).unwrap();
    let a = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let b = fixture::call("2026-11-05_120000-other", "Other", "system", fixture::MINUTES);
    crate::projects::set_membership(&thread.id, &a, "include").unwrap();
    crate::projects::set_membership(&thread.id, &b, "exclude").unwrap();
    super::set(&v.id, "scope", "some").unwrap();
    super::set(&v.id, "threads", &thread.id).unwrap();
    assert_eq!(targets(&a).len(), 1);
    assert!(targets(&b).is_empty());
    // A client works the same (by id or by name).
    super::set(&v.id, "threads", "").unwrap();
    super::set(&v.id, "clients", "Northwind Retail").unwrap();
    assert_eq!(targets(&a).len(), 1);
    assert!(targets(&b).is_empty());
}

#[test]
fn this_call_opted_out() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-optout");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    choose(&call, Some("none")).unwrap();
    assert!(save_by_rules(&call).is_empty());
    assert!(!note_path(&v).exists());
    assert_eq!(record(&call)["choice"], "none");
    choose(&call, None).unwrap();
    assert_eq!(targets(&call).len(), 1);
}

#[test]
fn two_vaults_both_save() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-two");
    let a = starter(&root, "My twin");
    let b = starter(&root, "Team wiki");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let saved = save_by_rules(&call);
    assert_eq!(saved.len(), 2);
    assert!(note_path(&a).is_file() && note_path(&b).is_file());
    // Choosing one vault for this call saves only there.
    let c2 = fixture::call("2026-11-05_130000-second", "Second", "system", fixture::MINUTES);
    choose(&c2, Some(&b.id)).unwrap();
    assert_eq!(targets(&c2).iter().map(|v| v.id.clone()).collect::<Vec<_>>(), std::slice::from_ref(&b.id));
}

/// A starter vault in git with commits on and a test identity.
fn committing(root: &Path) -> Option<(super::Vault, git::tests::Isolated)> {
    git::program()?;
    let iso = git::tests::isolated(root, true);
    let c = super::starter::create("My twin", Some(root.join("vault")), true, "auto").unwrap();
    assert_eq!(c.git, "committed");
    Some((c.vault, iso))
}

#[test]
fn commit_holds_only_echos_files() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-commit");
    let Some((v, _iso)) = committing(&root) else { return };
    std::fs::write(v.root.join("knowledge/topics/draft.md"), "# staged by the user\n").unwrap();
    assert!(git::run(&v.root, &["add", "knowledge/topics/draft.md"]).unwrap().status.success());
    std::fs::write(v.root.join("knowledge/profile/DIGITAL_PROFILE.md"), "# my unfinished edit\n").unwrap();
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert!(!s.commit.starts_with("skipped") && s.commit != "off", "{}", s.commit);
    let files = git::out(&v.root, &["show", "--name-only", "--format=%s", "HEAD"]).unwrap();
    let mut lines: Vec<&str> = files.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.remove(0), "Echo: minutes of \"Weekly delivery sync\" (2026-11-05) → knowledge/notes/");
    lines.sort();
    assert_eq!(lines, ["knowledge/CATALOG.md", "knowledge/notes/2026-11-05-weekly-delivery-sync.md", "log.md"]);
    let status = git::out(&v.root, &["status", "--porcelain"]).unwrap();
    assert!(status.contains("A  knowledge/topics/draft.md"), "the user's staged file stays staged: {status}");
    assert!(status.contains(" M knowledge/profile/DIGITAL_PROFILE.md"), "the user's edit stays unstaged: {status}");
}

#[test]
fn a_dirty_index_is_written_not_committed() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-dirty");
    let Some((v, _iso)) = committing(&root) else { return };
    let cat = v.root.join("knowledge/CATALOG.md");
    let mut t = std::fs::read_to_string(&cat).unwrap();
    t.push_str("\nMy own unsaved line.\n");
    std::fs::write(&cat, &t).unwrap();
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    save(&call, &v.id, How::Auto).unwrap();
    let files = git::out(&v.root, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
    assert!(!files.contains("CATALOG.md"), "{files}");
    assert!(files.contains("2026-11-05-weekly-delivery-sync.md"));
    let now = std::fs::read_to_string(&cat).unwrap();
    assert!(now.contains("My own unsaved line.") && now.contains("2026-11-05-weekly-delivery-sync.md"));
}

#[test]
fn commit_is_skipped_with_a_reason() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-skip");
    let Some((v, iso)) = committing(&root) else { return };
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let gitdir = v.root.join(".git");
    std::fs::write(gitdir.join("MERGE_HEAD"), "0000000000000000000000000000000000000000\n").unwrap();
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert_eq!(s.commit, "skipped: a merge is in progress");
    assert!(note_path(&v).is_file(), "the note is written all the same");
    std::fs::remove_file(gitdir.join("MERGE_HEAD")).unwrap();
    // No identity.
    drop(iso);
    let _bare = git::tests::isolated(&root, false);
    let c2 = fixture::call("2026-11-05_140000-later", "Later", "system", fixture::MINUTES);
    let s2 = save(&c2, &v.id, How::Auto).unwrap();
    assert_eq!(s2.commit, "skipped: git has no user name and email set");
    // A failing pre-commit hook.
    drop(_bare);
    let _iso2 = git::tests::isolated(&root, true);
    std::fs::write(gitdir.join("hooks/pre-commit"), "#!/bin/sh\necho 'blocked by the hook' >&2\nexit 1\n").unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(gitdir.join("hooks/pre-commit"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let c3 = fixture::call("2026-11-05_150000-hooked", "Hooked", "system", fixture::MINUTES);
    let s3 = save(&c3, &v.id, How::Auto).unwrap();
    #[cfg(unix)]
    assert_eq!(s3.commit, "skipped: blocked by the hook");
    let status = git::out(&v.root, &["status", "--porcelain"]).unwrap();
    assert!(status.contains("?? knowledge/notes/2026-11-05-hooked.md"), "a refused new note isn't left staged: {status}");
}

#[test]
fn commits_are_off_by_default_for_connected_vaults() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-off");
    let _iso = git::program().map(|_| git::tests::isolated(&root, true));
    let t = fixture::twin(&root.join("twin"));
    if git::program().is_some() {
        assert!(git::tests::repo(&t));
    }
    let v = super::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let s = save(&call, &v.id, How::Auto).unwrap();
    assert_eq!(s.commit, "off");
    assert!(t.join("knowledge/notes/2026-11-05-weekly-delivery-sync.md").is_file());
    assert!(std::fs::read_to_string(t.join("knowledge/CATALOG.md")).unwrap().contains("| [2026-11-05-weekly-delivery-sync.md](notes/2026-11-05-weekly-delivery-sync.md) |"));
}

#[test]
fn client_links_its_page() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-client");
    let v = starter(&root, "My twin");
    std::fs::write(v.root.join("knowledge/clients/northwind-retail.md"), "---\ntitle: Northwind Retail\ntags: [client]\n---\n# Northwind Retail\n").unwrap();
    let client = crate::projects::add_client("Northwind Retail", &[], false).unwrap();
    let thread = crate::projects::create("Checkout & payments", Some(&client.id), None).unwrap();
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    crate::projects::set_membership(&thread.id, &call, "include").unwrap();
    save(&call, &v.id, How::Auto).unwrap();
    let text = std::fs::read_to_string(note_path(&v)).unwrap();
    assert!(text.contains("client: \"[[northwind-retail|Northwind Retail]]\"\n"), "{text}");
    assert!(text.contains("threads: [\"Checkout & payments\"]\n"), "{text}");
    // Without the page: plain text.
    let c2 = fixture::call("2026-11-05_160000-plain", "Plain", "system", fixture::MINUTES);
    crate::projects::set_membership(&thread.id, &c2, "include").unwrap();
    std::fs::remove_file(v.root.join("knowledge/clients/northwind-retail.md")).unwrap();
    save(&c2, &v.id, How::Auto).unwrap();
    let t2 = std::fs::read_to_string(v.root.join("knowledge/notes/2026-11-05-plain.md")).unwrap();
    assert!(t2.contains("client: \"Northwind Retail\"\n"), "{t2}");
}

#[test]
fn a_missing_vault_records_the_failure() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-missing");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let minutes_before = std::fs::read_to_string(call.join("notes.md")).unwrap();
    std::fs::rename(&v.root, root.join("moved")).unwrap();
    assert!(targets(&call).is_empty(), "the chip doesn't promise a vault whose folder is gone");
    // The automatic save is tried all the same, so its failure is seen (K3-12, K3-T15).
    assert_eq!(save_by_rules(&call).len(), 0);
    assert_eq!(std::fs::read_to_string(call.join("notes.md")).unwrap(), minutes_before);
    let f = failure(&call).expect("the automatic save's failure is recorded");
    assert_eq!(f.0, v.id);
    assert!(f.2.contains("folder isn't there"), "{}", f.2);
    let inbox = |id: &str| crate::attention::collect().items.iter().any(|i| i["key"] == id);
    let item = format!("knowledge:{}", crate::calls::stamp_of(&call));
    assert!(inbox(&item), "an Inbox item with Save again");
    std::fs::rename(root.join("moved"), &v.root).unwrap();
    save(&call, &v.id, How::Auto).unwrap();
    assert!(failure(&call).is_none(), "a later save clears it");
    assert!(!inbox(&item), "and the Inbox item");
}

#[test]
fn saves_at_the_same_time_make_one_note_each() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-race");
    let v = starter(&root, "My twin");
    let a = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    let b = fixture::call("2026-11-05_140000-design-review", "Design review", "system", fixture::MINUTES);
    let go = std::sync::Arc::new(std::sync::Barrier::new(6));
    let threads: Vec<_> = (0..6)
        .map(|i| {
            let (call, id, go) = (if i % 2 == 0 { a.clone() } else { b.clone() }, v.id.clone(), go.clone());
            std::thread::spawn(move || {
                go.wait();
                save(&call, &id, if i < 2 { How::Auto } else { How::Manual(Manual::default()) })
            })
        })
        .collect();
    for t in threads {
        t.join().unwrap().unwrap();
    }
    let notes: Vec<String> = std::fs::read_dir(v.root.join("knowledge/notes")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n != "README.md").collect();
    assert_eq!(notes.len(), 2, "one note per call, never a -2 beside it: {notes:?}");
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    for n in &notes {
        assert_eq!(cat.matches(&format!("(notes/{n})")).count(), 1, "one row each, none lost: {n}");
    }
}

#[test]
fn the_preview_is_what_the_save_writes() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-preview");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    // A note of that name that isn't this call's: the save takes the next free name.
    fixture::write(&v.root, "knowledge/notes/2026-11-05-weekly-delivery-sync.md", "# someone else's\n");
    let p = preview(&call, &v.id, None).unwrap();
    let s = save(&call, &v.id, How::Manual(Manual::default())).unwrap();
    assert_eq!(p["path"], s.path, "the preview names the file the save writes");
    assert_eq!(s.path, "knowledge/notes/2026-11-05-weekly-delivery-sync-2.md");
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    let row = p["row"].as_str().unwrap();
    assert!(cat.lines().any(|l| l == row), "the row as it is written: {row}");
    assert_eq!(p["note"].as_str().unwrap(), std::fs::read_to_string(v.root.join(&s.path)).unwrap(), "and the note");
    // A vault with no notes folder yet: the preview and the save agree on where it goes.
    let plain = root.join("plain");
    fixture::write(&plain, "knowledge/.obsidian/app.json", "{}");
    fixture::write(&plain, "knowledge/CATALOG.md", "# Index\n\n## Notes\n\n| File | Description | Date Added |\n|------|-------------|------------|\n");
    fixture::write(&plain, "knowledge/a.md", "# A\n");
    let v2 = super::add(&plain, super::AddOpts { name: Some("Plain".into()), ..Default::default() }).unwrap();
    assert_eq!(v2.path("notes"), None);
    let call2 = fixture::call("2026-11-05_140000-design-review", "Design review", "system", fixture::MINUTES);
    let p2 = preview(&call2, &v2.id, None).unwrap();
    let s2 = save(&call2, &v2.id, How::Manual(Manual::default())).unwrap();
    assert_eq!(p2["path"], s2.path);
    assert_eq!(s2.path, "knowledge/notes/2026-11-05-design-review.md");
}

#[test]
fn a_sample_is_the_newest_call_the_vault_reads() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-sample");
    let v = starter(&root, "My twin");
    assert!(sample(&v.id).unwrap().is_null(), "no call yet");
    fixture::call("2026-11-04_100000-older", "Older", "system", fixture::MINUTES);
    fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    fixture::call("2026-11-06_100000-other-profile", "Elsewhere", "client-x", fixture::MINUTES);
    let p = sample(&v.id).unwrap();
    assert_eq!(p["title"], "Weekly delivery sync", "the newest on the vault's own profile: {p}");
    assert!(p["note"].as_str().unwrap().contains("title: \"Weekly delivery sync\""));
    assert!(std::fs::read_dir(v.root.join("knowledge/notes")).unwrap().flatten().all(|e| e.file_name() == "README.md"), "nothing written");
}

#[test]
fn the_note_keeps_the_minutes_paragraphs() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-body");
    let v = starter(&root, "My twin");
    let plain = fixture::call(CALL, "Weekly delivery sync", "system", "# Weekly delivery sync\n\n2026-11-05, 46 min. Participants: Alex Doe.\n\nFirst paragraph.\n\nSecond paragraph.\n");
    let s = save(&plain, &v.id, How::Auto).unwrap();
    let t = std::fs::read_to_string(v.root.join(&s.path)).unwrap();
    assert!(t.contains("First paragraph.\n\nSecond paragraph."), "paragraphs stay apart: {t}");
    assert!(!t.contains("Participants: Alex Doe."), "the date line is in the properties: {t}");
    let lead = fixture::call("2026-11-05_140000-design-review", "Design review", "system", "# Design review\n2026-11-05, 30 min.\n\nA lead paragraph before the sections.\n\n## Decisions\n\n- Ship it.\n");
    let s = save(&lead, &v.id, How::Auto).unwrap();
    let t = std::fs::read_to_string(v.root.join(&s.path)).unwrap();
    assert!(t.contains("A lead paragraph before the sections.\n\n## Decisions"), "{t}");
}

#[test]
fn odd_titles_make_safe_files_and_valid_yaml() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-odd");
    let v = starter(&root, "My twin");
    let call = fixture::call("2026-11-05_170000-odd", "Q4: \"Plan\" / Синк #2 — review", "system", fixture::MINUTES);
    let s = save(&call, &v.id, How::Auto).unwrap();
    let name = s.path.rsplit('/').next().unwrap();
    assert!(!name.contains(['/', '\\', ':', '"', '#', '?', '*']), "{name}");
    assert!(name.starts_with("2026-11-05-") && name.contains("синк"), "{name}");
    let text = std::fs::read_to_string(v.root.join(&s.path)).unwrap();
    let props = super::fm::parse(super::fm::split(&text).0.unwrap());
    assert_eq!(super::fm::get(&props, "title").unwrap().text(), "Q4: \"Plan\" / Синк #2 — review");
    assert_ne!(props[0].0, "", "the frontmatter reads back");
}

#[test]
fn memos_follow_the_scope() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-memo");
    let v = starter(&root, "My twin");
    let memo = fixture::call("2026-11-05_180000-memo", "Thoughts on pricing", "system", fixture::MINUTES);
    let mut meta: serde_json::Value = serde_json::from_slice(&std::fs::read(memo.join("meta.json")).unwrap()).unwrap();
    meta["kind"] = "memo".into();
    std::fs::write(memo.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    super::set(&v.id, "scope", "calls").unwrap();
    assert!(targets(&memo).is_empty());
    super::set(&v.id, "scope", "all").unwrap();
    let s = save(&memo, &v.id, How::Auto).unwrap();
    let text = std::fs::read_to_string(v.root.join(&s.path)).unwrap();
    assert!(text.contains("meeting: \"Memo\"\n"), "{text}");
}

#[test]
fn only_rule_saved_notes_update() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-rules");
    let a = starter(&root, "My twin");
    let b = starter(&root, "Team wiki");
    super::set(&b.id, "minutes", "ask").unwrap();
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    save_by_rules(&call);
    save(&call, &b.id, How::manual()).unwrap();
    std::fs::write(call.join("notes.md"), fixture::MINUTES.replace("18 November", "25 November")).unwrap();
    let r = save_by_rules(&call);
    assert_eq!(r.len(), 1);
    assert!(std::fs::read_to_string(note_path(&a)).unwrap().contains("25 November"));
    assert!(std::fs::read_to_string(note_path(&b)).unwrap().contains("18 November"), "a note saved by hand isn't updated by itself");
    assert_eq!(stale(&call), vec![b.id.clone()], "it is offered instead");
}

#[test]
fn summary_falls_back() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-summary");
    let v = starter(&root, "My twin");
    let ru = "# Синк\n\n2026-11-05, 46 мин.\n\n## Итоги\n\n- Пилот переносится на 18 ноября\n- Цены не меняем\n\n## Решения\n\n- x\n";
    let c1 = fixture::call("2026-11-05_190000-sink", "Синк", "system", ru);
    let s1 = save(&c1, &v.id, How::Auto).unwrap();
    let t1 = std::fs::read_to_string(v.root.join(&s1.path)).unwrap();
    assert!(t1.contains("summary: \"Пилот переносится на 18 ноября; Цены не меняем\"\n"), "{t1}");
    let c2 = fixture::call("2026-11-05_200000-bare", "Bare", "system", "# Bare\n\nJust a line, no sections.\n");
    let s2 = save(&c2, &v.id, How::Auto).unwrap();
    let t2 = std::fs::read_to_string(v.root.join(&s2.path)).unwrap();
    assert!(t2.contains("summary: \"\"\n"), "{t2}");
    let cat = std::fs::read_to_string(v.root.join("knowledge/CATALOG.md")).unwrap();
    assert!(cat.contains("| 46m Bare | 2026-11-05 |"), "the title alone: {cat}");
}

#[test]
fn index_is_reread_before_the_row_goes_in() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-reread");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    // Changed by someone else just before the save: their change stays.
    let cat = v.root.join("knowledge/CATALOG.md");
    let t = std::fs::read_to_string(&cat).unwrap().replace("## Media", "## Media\n\nA line a skill added.");
    std::fs::write(&cat, t).unwrap();
    save(&call, &v.id, How::Auto).unwrap();
    let now = std::fs::read_to_string(&cat).unwrap();
    assert!(now.contains("A line a skill added.") && now.contains("2026-11-05-weekly-delivery-sync.md"));
}

#[test]
fn past_minutes_are_saved_with_one_commit() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-past");
    let Some((v, _iso)) = committing(&root) else { return };
    for (i, t) in ["One", "Two", "Three"].iter().enumerate() {
        fixture::call(&format!("2026-11-0{}_100000-{}", i + 1, t.to_lowercase()), t, "system", fixture::MINUTES);
    }
    fixture::call("2026-11-04_100000-other", "Other profile", "client-x", fixture::MINUTES);
    let p = past(&v.id).unwrap();
    assert_eq!(p["count"], 3);
    let before = git::out(&v.root, &["rev-list", "--count", "HEAD"]).unwrap();
    let r = save_past(&v.id).unwrap();
    assert_eq!(r["saved"], 3);
    let after = git::out(&v.root, &["rev-list", "--count", "HEAD"]).unwrap();
    assert_eq!(after.parse::<u32>().unwrap(), before.parse::<u32>().unwrap() + 1);
    assert_eq!(past(&v.id).unwrap()["count"], 0);
}

#[test]
fn minutes_written_saves_in_the_background() {
    let (_env, root) = crate::paths::testenv::folders("kn-ex-hook");
    let v = starter(&root, "My twin");
    let call = fixture::call(CALL, "Weekly delivery sync", "system", fixture::MINUTES);
    assert_eq!(minutes_written(&call), ["My twin"]);
    crate::paths::testenv::eventually("the note is saved", || record(&call)["saved"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(note_path(&v).is_file());
    // A call no vault takes: nothing, no thread.
    let other = fixture::call("2026-11-05_210000-x", "X", "client-x", fixture::MINUTES);
    assert!(minutes_written(&other).is_empty());
}

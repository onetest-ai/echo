//! Health's findings, score, fixes, ignored findings and history (spec K16), on made-up vaults.

use super::*;
use crate::knowledge::{self, checks, fixture, git, scan, AddOpts};
use std::path::Path;

fn twin(root: &Path) -> Vault {
    let t = fixture::twin(&root.join("twin"));
    knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
}

fn of_kind<'a>(f: &'a [Finding], kind: &str) -> Vec<&'a Finding> {
    f.iter().filter(|x| x.kind == kind).collect()
}

fn pages(f: &Finding) -> Vec<&str> {
    f.pages.iter().map(|(p, _)| p.as_str()).collect()
}

fn added(f: &Fix) -> Vec<&str> {
    f.diff.iter().filter(|(k, _)| *k == '+').map(|(_, l)| l.as_str()).collect()
}

fn removed(f: &Fix) -> Vec<&str> {
    f.diff.iter().filter(|(k, _)| *k == '-').map(|(_, l)| l.as_str()).collect()
}

const NEW_KINDS: [&str; 4] = ["dead_row", "usual_prop", "duplicate_title", "oversized_index"];

#[test]
fn one_of_each_new_code_finding() {
    // K16-T1: exactly those; README and templates not counted.
    let (_env, root) = crate::paths::testenv::folders("kn-health-new");
    let t = fixture::twin(&root.join("twin"));
    // An index row to a note that isn't there, and a glossary of 1,501 rows: an oversized index.
    let catalog = std::fs::read_to_string(t.join("knowledge/CATALOG.md")).unwrap();
    let glossary: String = (0..1501).map(|n| format!("| term-{n} | a made-up term |\n")).collect();
    fixture::write(&t, "knowledge/CATALOG.md", &format!("{catalog}| [2026-01-09-gone.md](notes/2026-01-09-gone.md) | gone | 2026-01-09 |\n\n## Glossary\n\n| Term | Meaning |\n|---|---|\n{glossary}"));
    // Five topics, four with a summary: the fifth misses the folder's usual property.
    for n in 1..=5 {
        let summary = if n < 5 { "summary: A made-up topic.\n" } else { "" };
        fixture::write(&t, &format!("knowledge/topics/topic-{n}.md"), &format!("---\ntitle: Topic {n}\ntags: [concept]\n{summary}---\n# Topic {n}\n"));
    }
    // One title twice, case and spacing aside.
    fixture::write(&t, "knowledge/clients/weekly-sync.md", "---\ntitle: Weekly Sync\ntags: [client]\n---\n# Weekly Sync\n");
    fixture::write(&t, "knowledge/notes/2026-01-04-weekly.md", "---\ntitle: weekly  sync\ntags: [meeting]\n---\n# weekly  sync\n");
    // Not counted: two READMEs with one title (one in topics/, without a summary), a template with that title too.
    fixture::write(&t, "knowledge/topics/README.md", "---\ntitle: Overview\n---\n# Overview\n");
    fixture::write(&t, "knowledge/clients/README.md", "---\ntitle: Overview\n---\n# Overview\n");
    fixture::write(&t, "knowledge/templates/page.md", "---\ntitle: Weekly Sync\n---\n# {{title}}\n");
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    assert_eq!(v.path("templates").as_deref(), Some("knowledge/templates"));
    let f = findings(&v, &scan::index(&v).unwrap());
    let mut new: Vec<(&str, Vec<&str>)> = f.iter().filter(|x| NEW_KINDS.contains(&x.kind.as_str())).map(|x| (x.kind.as_str(), pages(x))).collect();
    new.sort();
    assert_eq!(new, [
        ("dead_row", vec!["knowledge/CATALOG.md"]),
        ("duplicate_title", vec!["knowledge/clients/weekly-sync.md", "knowledge/notes/2026-01-04-weekly.md"]),
        ("oversized_index", vec!["knowledge/CATALOG.md"]),
        ("usual_prop", vec!["knowledge/topics/topic-5.md"]),
    ]);
    let dead = of_kind(&f, "dead_row")[0];
    assert_eq!(dead.key, "dead_row:knowledge/CATALOG.md:(notes/2026-01-09-gone.md)");
    assert_eq!(dead.pages[0].1, Some(17), "the row's line");
    assert!(of_kind(&f, "broken").is_empty(), "the index's rows are their own kind, not broken links too");
    assert!(of_kind(&f, "usual_prop")[0].title.contains("summary"));
    let groups: Vec<(&str, Group, By)> = ["dead_row", "usual_prop", "duplicate_title", "oversized_index"].iter().map(|k| { let x = of_kind(&f, k)[0]; (*k, x.group, x.by) }).collect();
    assert_eq!(groups, [("dead_row", Group::Should, By::Code), ("usual_prop", Group::Tidy, By::Code), ("duplicate_title", Group::Needs, By::Code), ("oversized_index", Group::Should, By::Code)]);
    let c = checks::run(&v, &scan::index(&v).unwrap());
    assert_eq!(c.oversized_index.map(|o| o.rows), Some(1505), "the notes and clients rows and the glossary");
}

fn cats(s: &Score) -> Vec<(&str, f64, u32)> {
    s.categories.iter().map(|(n, p, w)| (n.as_str(), *p, *w)).collect()
}

fn found(kind: &str, page: &str, by: By) -> Finding {
    Finding { key: format!("{kind}:{page}"), kind: kind.into(), group: Group::Should, by, pages: vec![(page.into(), None)], title: String::new(), why: String::new(), fixable: false, fix_all: false, subject: Subject::None }
}

#[test]
fn score_counts_the_pages_each_category_touches() {
    // K16-T2: 100 pages, 10 with broken links; then an AI check with 4 stale pages.
    let mut f: Vec<Finding> = (0..10).map(|n| found("broken", &format!("p{n}.md"), By::Code)).collect();
    f.push(Finding { key: "broken:p0.md:[[b]]".into(), ..found("broken", "p0.md", By::Code) });
    let s = score(&f, 100, false);
    assert_eq!(cats(&s), [("Links", 12.5, 25), ("Index", 20.0, 20), ("Properties", 15.0, 15), ("Consistency", 25.0, 25)], "a page with two broken links counts once; Freshness left out");
    assert_eq!((s.total, s.code_only), (85, true), "round(100 × 72.5 / 85)");
    f.extend((0..4).map(|n| found("stale", &format!("s{n}.md"), By::Ai)));
    let s = score(&f, 100, true);
    assert_eq!(cats(&s).last(), Some(&("Freshness", 12.0, 15)));
    assert_eq!((s.total, s.code_only), (85, false), "84.5 rounds half up");
    // Half the pages broken: the category is at 0, never below.
    let many: Vec<Finding> = (0..60).map(|n| found("broken", &format!("p{n}.md"), By::Code)).collect();
    assert_eq!(cats(&score(&many, 100, false))[0], ("Links", 0.0, 25));
    assert_eq!(score(&[], 0, false).total, 100, "nothing to judge");
}

#[test]
fn broken_links_point_to_the_closest_page() {
    // K16-T3.
    let (_env, root) = crate::paths::testenv::folders("kn-health-closest");
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "knowledge/clients/northwind-retail.md", "---\ntitle: Northwind Retail\n---\n# Northwind Retail\n");
    fixture::write(&t, "knowledge/notes/2026-11-05-a.md", "---\ntitle: A sync\n---\n# A sync\n");
    fixture::write(&t, "knowledge/topics/retro.md", "---\ntitle: Retro\n---\n# Retro\n\nAgreed with [[Northwind Retail]] to add a fallback.\nSee [the sync](notes/2026-1-05-a.md).\nAnd the [[summit-slides]].\n");
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    let f = findings(&v, &scan::index(&v).unwrap());
    let key = |link: &str| format!("broken:knowledge/topics/retro.md:{link}");
    let broken: Vec<&str> = of_kind(&f, "broken").iter().map(|x| x.key.as_str()).collect();
    assert_eq!(broken, [key("(notes/2026-1-05-a.md)"), key("[[Northwind Retail]]"), key("[[summit-slides]]")]);
    let to_client = fix(&v, &key("[[Northwind Retail]]"), None).unwrap();
    assert_eq!(to_client.path, "knowledge/topics/retro.md");
    assert_eq!(removed(&to_client), ["Agreed with [[Northwind Retail]] to add a fallback."]);
    assert_eq!(added(&to_client), ["Agreed with [[clients/northwind-retail|Northwind Retail]] to add a fallback."]);
    assert!(!to_client.guess);
    let to_note = fix(&v, &key("(notes/2026-1-05-a.md)"), None).unwrap();
    assert_eq!(added(&to_note), ["See [the sync](notes/2026-11-05-a.md)."]);
    assert!(to_note.guess, "one character apart: offered, but a guess");
    let fix_all = |k: &str| f.iter().find(|x| x.key == k).unwrap().fix_all;
    assert!(fix_all(&key("[[Northwind Retail]]")) && !fix_all(&key("(notes/2026-1-05-a.md)")));
    let choices = |k: &str| detail(&v, k, None).unwrap()["choices"].as_array().unwrap().iter().map(|c| c["choice"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    let d = detail(&v, &key("[[Northwind Retail]]"), None).unwrap();
    assert_eq!(d["choices"][0]["to"], "knowledge/clients/northwind-retail.md");
    assert_eq!(d["choices"][0]["why"], "the same name but for case or spacing");
    assert_eq!(choices(&key("[[Northwind Retail]]")), ["to", "remove", "create"]);
    assert_eq!(d["fix"]["plus"], 1);
    assert_eq!(d["fix"]["hunks"][0].as_array().unwrap().iter().filter(|l| l[0] == "+").count(), 1, "the diff as hunks of [sign, line]");
    // Nothing close: only removing the link or creating the page.
    let none = key("[[summit-slides]]");
    assert!(!of_kind(&f, "broken").iter().find(|x| x.key == none).unwrap().fixable);
    assert_eq!(choices(&none), ["remove", "create"]);
    assert_eq!(detail(&v, &none, None).unwrap()["new_page"], json!({"folder": "knowledge/topics", "name": "summit-slides"}));
    assert!(fix(&v, &none, None).unwrap_err().to_string().contains("Nothing close"));
    let unlinked = fix(&v, &none, Some("remove")).unwrap();
    assert_eq!(added(&unlinked), ["And the summit-slides."]);
    // Fixes write nothing.
    assert!(std::fs::read_to_string(t.join("knowledge/topics/retro.md")).unwrap().contains("[[Northwind Retail]]"));
}

#[test]
fn fix_all_writes_what_did_not_change_since_in_one_commit() {
    // K16-T4: three broken links, one page changed since; commits on.
    let (_env, root) = crate::paths::testenv::folders("kn-health-fixall");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "knowledge/clients/northwind-retail.md", "---\ntitle: Northwind Retail\n---\n# Northwind Retail\n");
    for n in 1..=3 {
        fixture::write(&t, &format!("knowledge/topics/t{n}.md"), &format!("---\ntitle: T{n}\n---\nWith [[Northwind Retail]] ({n}).\n"));
    }
    assert!(git::tests::repo(&t));
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), commit: Some(true), ..Default::default() }).unwrap();
    let keys: Vec<String> = findings(&v, &scan::index(&v).unwrap()).into_iter().filter(|f| f.kind == "broken").map(|f| f.key).collect();
    assert_eq!(keys.len(), 3);
    let fixes: Vec<Fix> = keys.iter().map(|k| fix(&v, k, None).unwrap()).collect();
    fixture::write(&t, "knowledge/topics/t2.md", "---\ntitle: T2\n---\nWith [[Northwind Retail]] (2), edited by Lena Hartmann.\n");
    let commits = || git::out(&t, &["rev-list", "--count", "HEAD"]).unwrap().parse::<u32>().unwrap();
    let before = commits();
    let a = apply(&v, &fixes).unwrap();
    assert_eq!(a.written, ["knowledge/topics/t1.md", "knowledge/topics/t3.md"]);
    assert_eq!(a.changed_since, ["knowledge/topics/t2.md"]);
    assert_eq!(commits(), before + 1, "one commit");
    assert!(crate::knowledge::activity::commit_id(&a.commit).is_some(), "{}", a.commit);
    let files = git::out(&t, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
    assert_eq!(files.lines().collect::<Vec<_>>(), ["knowledge/topics/t1.md", "knowledge/topics/t3.md"]);
    assert!(std::fs::read_to_string(t.join("knowledge/topics/t1.md")).unwrap().contains("[[clients/northwind-retail|Northwind Retail]] (1)"));
    assert!(std::fs::read_to_string(t.join("knowledge/topics/t2.md")).unwrap().contains("edited by Lena Hartmann"), "the changed page is left as it is");
    let lines = crate::knowledge::activity::recent(&v, "2000-01-01T00:00:00+00:00");
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|e| e.by == "health" && e.commit.is_some() && (e.plus, e.minus) == (1, 1)));
    // Echo's own write is a new revision: the links it fixed resolve, so their findings are gone.
    let after: Vec<String> = findings(&v, &scan::index(&v).unwrap()).into_iter().filter(|f| f.kind == "broken").map(|f| f.key).collect();
    assert_eq!(after, ["broken:knowledge/topics/t2.md:[[Northwind Retail]]"]);
}

#[test]
fn an_ignored_finding_is_neither_listed_nor_counted() {
    // K16-T7: ignore a finding; scan again.
    let (_env, root) = crate::paths::testenv::folders("kn-health-ignore");
    let v = twin(&root);
    let key = "not_in_index:knowledge/notes/2026-01-03-c.md";
    let listed = |j: &Value, list: &str| j[list].as_array().unwrap().iter().any(|f| f["key"] == key);
    let before = view(&v, None).unwrap();
    assert!(listed(&before, "findings"));
    assert_eq!(before["score"]["total"], 76, "Index at 0: one of four pages isn't listed");
    ignore(&v, key, true).unwrap();
    // Scan again: a page changes, the revision with it.
    fixture::write(&v.root, "knowledge/clients/acme.md", "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp\n\nSee [[2026-01-01-a]] and the plan.\n");
    let after = view(&v, None).unwrap();
    assert!(!listed(&after, "findings"), "not listed");
    assert_eq!(after["score"]["total"], 100, "not counted");
    assert!(listed(&after, "ignored"), "listed under Show ignored");
    assert_eq!(load(&v).ignored, [key]);
    ignore(&v, key, false).unwrap();
    assert!(listed(&view(&v, None).unwrap(), "findings"));
}

#[test]
fn check_cli_keeps_its_keys() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-cli");
    let v = twin(&root);
    let j = check(&v).unwrap();
    let keys: Vec<&str> = j.as_object().unwrap().keys().map(String::as_str).collect();
    let mut want = vec!["not_in_index", "broken", "isolated", "no_props", "unknown_tags", "meeting_tag_missing", "no_index", "dead_rows", "usual_props", "duplicate_titles", "oversized_index", "score"];
    want.sort();
    let mut keys = keys;
    keys.sort();
    assert_eq!(keys, want);
    assert_eq!(j["not_in_index"], json!(["notes/2026-01-03-c.md"]), "slice 1's shape");
    assert_eq!(j["isolated"], json!(["notes/2026-01-03-c.md"]));
    assert_eq!(j["no_index"], false);
    assert_eq!(j["score"]["total"], 76);
    assert_eq!(j["score"]["code_only"], true);
    assert_eq!(load(&v).history.len(), 1, "the day's point");
}

#[test]
fn findings_are_cached_per_revision() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-cache");
    let v = twin(&root);
    let a = cached(&v, &scan::index(&v).unwrap());
    let b = cached(&v, &scan::index(&v).unwrap());
    assert!(Arc::ptr_eq(&a, &b), "the same revision: checked once");
    assert!(of_kind(&a, "broken").is_empty());
    fixture::write(&v.root, "knowledge/clients/acme.md", "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp\n\nSee [[2026-01-01-a]] and [[missing-page]].\n");
    let c = cached(&v, &scan::index(&v).unwrap());
    assert!(!Arc::ptr_eq(&a, &c), "a new revision: checked again");
    assert_eq!(of_kind(&c, "broken").len(), 1);
    // The vault's settings are part of what was checked.
    let v = knowledge::set(&v.id, "tag", "standup").unwrap();
    let d = cached(&v, &scan::index(&v).unwrap());
    assert!(!Arc::ptr_eq(&c, &d));
    assert_eq!(of_kind(&d, "meeting_tag_missing").len(), 1);
}

#[test]
fn one_point_per_day() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-points");
    let v = twin(&root);
    let s = |total: u32| Score { total, categories: vec![("Links".into(), 25.0, 25)], code_only: true };
    let day = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap();
    let points = || load(&v).history.iter().map(|p| (p.day.clone(), p.score)).collect::<Vec<_>>();
    ignore(&v, "isolated:knowledge/notes/2026-01-03-c.md", true).unwrap();
    note_on(&v, day("2026-11-01"), &s(80)).unwrap();
    note_on(&v, day("2026-11-01"), &s(82)).unwrap();
    assert_eq!(points(), [("2026-11-01".to_string(), 82)], "the day's last");
    note_on(&v, day("2026-11-03"), &s(84)).unwrap();
    assert_eq!(points(), [("2026-11-01".to_string(), 82), ("2026-11-03".to_string(), 84)]);
    // 91 days after the first: it goes; the second, 89 days before, stays.
    note_on(&v, day("2027-01-31"), &s(90)).unwrap();
    assert_eq!(points(), [("2026-11-03".to_string(), 84), ("2027-01-31".to_string(), 90)]);
    assert_eq!(load(&v).ignored, ["isolated:knowledge/notes/2026-01-03-c.md"], "the rest of health.json is kept");
}

#[test]
fn index_rows_properties_and_tags_are_fixed_by_code() {
    // K16-03's other fixes, and several fixes of one file written together.
    let (_env, root) = crate::paths::testenv::folders("kn-health-fixes");
    let t = fixture::twin(&root.join("twin"));
    let catalog = std::fs::read_to_string(t.join("knowledge/CATALOG.md")).unwrap();
    fixture::write(&t, "knowledge/CATALOG.md", &format!("{catalog}| [2026-01-05-a.md](notes/2026-01-05-a.md) | moved | 2026-01-05 |\n| [old.md](notes/old.md) | gone | 2026-01-06 |\n"));
    fixture::write(&t, "knowledge/meetings/2026-01-05-a.md", "---\ntitle: Moved\n---\n# Moved\n");
    fixture::write(&t, "knowledge/topics/bare.md", "# Bare\n\nNo properties.\n");
    fixture::write(&t, "knowledge/notes/2026-01-07-plain.md", "# Plain\r\n\r\nWritten on Windows.\r\n");
    fixture::write(&t, "knowledge/clients/acme.md", "---\ntitle: Acme Corp\ntags: [client, invented]\n---\n# Acme Corp\n\nWorks with #invented teams. See [[2026-01-01-a]].\n");
    fixture::write(&t, "knowledge/topics/block.md", "---\ntitle: Block\ntags:\n  - invented\n  - concept\n---\n# Block\n");
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    // An index row to a page elsewhere: pointed there; to nothing close: removed.
    let moved = fix(&v, "dead_row:knowledge/CATALOG.md:(notes/2026-01-05-a.md)", None).unwrap();
    assert_eq!(moved.choice, "to");
    assert_eq!(added(&moved), ["| [2026-01-05-a.md](meetings/2026-01-05-a.md) | moved | 2026-01-05 |"]);
    let gone = fix(&v, "dead_row:knowledge/CATALOG.md:(notes/old.md)", None).unwrap();
    assert_eq!((gone.choice.as_str(), removed(&gone), added(&gone)), ("remove", vec!["| [old.md](notes/old.md) | gone | 2026-01-06 |"], vec![]));
    let row = fix(&v, "not_in_index:knowledge/notes/2026-01-03-c.md", None).unwrap();
    assert_eq!(row.path, "knowledge/CATALOG.md");
    assert!(added(&row).iter().any(|l| l.contains("(notes/2026-01-03-c.md)")), "{:?}", added(&row));
    // Properties: the H1 as title, the date from the name, the file's own line endings.
    let bare = fix(&v, "no_props:knowledge/topics/bare.md", None).unwrap();
    assert_eq!(added(&bare), ["---", "title: Bare", "tags: []", "---"]);
    let plain = fix(&v, "no_props:knowledge/notes/2026-01-07-plain.md", None).unwrap();
    assert!(plain.text.starts_with("---\r\ntitle: Plain\r\ndate: 2026-01-07\r\ntags: []\r\n---\r\n# Plain\r\n"), "{:?}", plain.text);
    // A tag not in the taxonomy: replaced by the one the user picks, in the properties and in the text.
    let key = "unknown_tag:knowledge/clients/acme.md:#invented";
    assert!(fix(&v, key, None).unwrap_err().to_string().contains("Pick"));
    assert!(fix(&v, key, Some("tag:nope")).is_err(), "only a tag the taxonomy allows");
    let concept = fix(&v, key, Some("tag:concept")).unwrap();
    assert_eq!(added(&concept), ["tags: [client, concept]", "Works with #concept teams. See [[2026-01-01-a]]."]);
    let client = fix(&v, key, Some("tag:client")).unwrap();
    assert_eq!(added(&client)[0], "tags: [client]", "never the same tag twice");
    let block = fix(&v, "unknown_tag:knowledge/topics/block.md:#invented", Some("tag:concept")).unwrap();
    assert_eq!((removed(&block), added(&block)), (vec!["  - invented"], vec![]));
    let picks: Vec<String> = detail(&v, key, None).unwrap()["choices"].as_array().unwrap().iter().map(|c| c["choice"].as_str().unwrap().to_string()).collect();
    assert_eq!(picks, ["tag:meeting", "tag:client", "tag:concept", "taxonomy"]);
    // Three fixes of the index at once: one write, each change in it.
    let a = apply(&v, &[moved, gone, row]).unwrap();
    assert_eq!((a.written, a.changed_since, a.commit.as_str()), (vec!["knowledge/CATALOG.md".to_string()], vec![], "off"));
    let now = std::fs::read_to_string(t.join("knowledge/CATALOG.md")).unwrap();
    assert!(now.contains("](meetings/2026-01-05-a.md)") && !now.contains("notes/old.md") && now.contains("(notes/2026-01-03-c.md)"), "{now}");
    assert_eq!(crate::knowledge::activity::recent(&v, "2000-01-01T00:00:00+00:00").len(), 1);
    // A fix computed before that write no longer matches the index.
    let stale = Fix { base: "0".repeat(64), ..fix(&v, "no_props:knowledge/topics/bare.md", None).unwrap() };
    let a = apply(&v, &[stale]).unwrap();
    assert_eq!((a.written.len(), a.changed_since), (0, vec!["knowledge/topics/bare.md".to_string()]));
}

#[test]
fn a_new_vault_scores_100_on_code_checks_only() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-starter");
    let c = knowledge::starter::create("Fresh", Some(root.join("fresh")), false, "auto").unwrap();
    let h = view(&c.vault, None).unwrap();
    assert_eq!(h["findings"], json!([]));
    assert_eq!((h["score"]["total"].as_u64(), h["score"]["code_only"].as_bool()), (Some(100), Some(true)));
    assert_eq!(h["history"].as_array().unwrap().len(), 1);
}

#[test]
fn a_dated_note_is_never_guessed_for_fix_all() {
    // Ruling R36: Echo's notes are named by date, so a deleted week's note is 2 edits from the next week's.
    let (_env, root) = crate::paths::testenv::folders("kn-health-guess");
    let t = fixture::twin(&root.join("twin"));
    let catalog = std::fs::read_to_string(t.join("knowledge/CATALOG.md")).unwrap();
    fixture::write(&t, "knowledge/CATALOG.md", &format!("{catalog}| [2026-01-09-weekly-sync.md](notes/2026-01-09-weekly-sync.md) | gone | 2026-01-09 |\n"));
    fixture::write(&t, "knowledge/notes/2026-01-16-weekly-sync.md", "---\ntitle: Weekly sync\n---\n# Weekly sync\n");
    fixture::write(&t, "knowledge/topics/plan.md", "---\ntitle: Plan\n---\nAs agreed in [[2026-01-09-weekly-sync]].\n");
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap();
    let f = findings(&v, &scan::index(&v).unwrap());
    // An index row: only the same name elsewhere, or but for case or spacing; else removed.
    let dead = "dead_row:knowledge/CATALOG.md:(notes/2026-01-09-weekly-sync.md)";
    let choices = |k: &str| detail(&v, k, None).unwrap()["choices"].as_array().unwrap().iter().map(|c| c["choice"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(choices(dead), ["remove"], "no guessed page for an index row");
    assert_eq!(fix(&v, dead, None).unwrap().choice, "remove");
    // A broken link: the near name is offered on its own finding, with its diff, but never in Fix all.
    let broken = "broken:knowledge/topics/plan.md:[[2026-01-09-weekly-sync]]";
    let b = f.iter().find(|x| x.key == broken).unwrap();
    assert!(b.fixable && !b.fix_all, "offered alone, not in Fix all");
    let alone = fix(&v, broken, None).unwrap();
    assert!(alone.guess);
    assert_eq!(added(&alone), ["As agreed in [[notes/2026-01-16-weekly-sync|2026-01-09-weekly-sync]]."]);
    let sheet = view(&v, Some("broken")).unwrap();
    assert!(!sheet["fixes"].as_array().unwrap().iter().any(|x| x["key"] == broken), "not in Fix all's sheet");
    assert!(view(&v, Some("dead_row")).unwrap()["fixes"].as_array().unwrap().iter().any(|x| x["key"] == dead && x["choice"] == "remove"));
}

#[test]
fn an_index_row_fix_writes_slice_1s_log_line_in_the_same_commit() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-log");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "log.md", "# Log\n\n- 2026-01-01 · created · the vault\n");
    assert!(git::tests::repo(&t));
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), commit: Some(true), ..Default::default() }).unwrap();
    assert_eq!(v.detected.log.as_deref(), Some("log.md"));
    let row = fix(&v, "not_in_index:knowledge/notes/2026-01-03-c.md", None).unwrap();
    let a = apply(&v, &[row]).unwrap();
    assert_eq!(a.written, ["knowledge/CATALOG.md", "log.md"]);
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(std::fs::read_to_string(t.join("log.md")).unwrap().ends_with(&format!("- {today} · changed · knowledge/CATALOG.md · 1 page added to the index by Echo\n")));
    let files = git::out(&t, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
    assert_eq!(files.lines().collect::<Vec<_>>(), ["knowledge/CATALOG.md", "log.md"], "one commit with both");
}

/// The twin with dead index rows: a wikilink on a table row, a link both on a row and in prose, two on one row.
fn dead_rows(root: &Path) -> Vault {
    let t = fixture::twin(&root.join("twin"));
    let catalog = std::fs::read_to_string(t.join("knowledge/CATALOG.md")).unwrap();
    fixture::write(&t, "knowledge/CATALOG.md", &format!("{catalog}| [[Northwind Retail]] | Northwind | 2026-01-02 |\n| [old.md](notes/old.md) | gone | 2026-01-06 |\n| [x.md](notes/x-gone.md) and [y.md](notes/y-gone.md) | two | 2026-01-07 |\n\nSee also [the old notes](notes/old.md) for history.\n"));
    fixture::write(&t, "knowledge/clients/northwind-retail.md", "---\ntitle: Northwind Retail\n---\n# Northwind Retail\n");
    knowledge::add(&t, AddOpts { name: Some("Twin".into()), ..Default::default() }).unwrap()
}

#[test]
fn a_relinked_table_row_keeps_its_cells() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-cells");
    let v = dead_rows(&root);
    // On a table row the alias's `|` is escaped, else it splits the row's cells.
    let to = fix(&v, "dead_row:knowledge/CATALOG.md:[[Northwind Retail]]", None).unwrap();
    assert_eq!(added(&to), ["| [[clients/northwind-retail\\|Northwind Retail]] | Northwind | 2026-01-02 |"]);
}

#[test]
fn a_dead_link_in_prose_keeps_its_words() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-prose");
    let v = dead_rows(&root);
    // Removing: the table row goes; a line of prose keeps its words.
    let old = fix(&v, "dead_row:knowledge/CATALOG.md:(notes/old.md)", Some("remove")).unwrap();
    assert_eq!(removed(&old), ["| [old.md](notes/old.md) | gone | 2026-01-06 |", "See also [the old notes](notes/old.md) for history."]);
    assert_eq!(added(&old), ["See also the old notes for history."]);
}

#[test]
fn a_fix_with_nothing_left_to_do_is_skipped_not_changed_since() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-skip");
    let v = dead_rows(&root);
    // Two dead links on one row: the first fix removes the row, the second has nothing left to do — skipped, with why.
    let x = fix(&v, "dead_row:knowledge/CATALOG.md:(notes/x-gone.md)", None).unwrap();
    let y = fix(&v, "dead_row:knowledge/CATALOG.md:(notes/y-gone.md)", None).unwrap();
    let a = apply(&v, &[x, y]).unwrap();
    assert_eq!((a.written, a.changed_since), (vec!["knowledge/CATALOG.md".to_string()], vec![]));
    assert_eq!(a.skipped.iter().map(|s| s.key.as_str()).collect::<Vec<_>>(), ["dead_row:knowledge/CATALOG.md:(notes/y-gone.md)"]);
    assert!(a.skipped[0].why.contains("isn't there"), "{}", a.skipped[0].why);
}

#[test]
fn a_file_left_out_of_the_commit_has_no_commit_in_its_activity() {
    let (_env, root) = crate::paths::testenv::folders("kn-health-act");
    if git::program().is_none() {
        return;
    }
    let _iso = git::tests::isolated(&root, true);
    let t = fixture::twin(&root.join("twin"));
    fixture::write(&t, "knowledge/clients/northwind-retail.md", "---\ntitle: Northwind Retail\n---\n# Northwind Retail\n");
    for n in 1..=2 {
        fixture::write(&t, &format!("knowledge/topics/t{n}.md"), &format!("---\ntitle: T{n}\n---\nWith [[Northwind Retail]] ({n}).\n"));
    }
    assert!(git::tests::repo(&t));
    // Lena Hartmann's own change, not committed, before Health looks.
    fixture::write(&t, "knowledge/topics/t2.md", "---\ntitle: T2\n---\nWith [[Northwind Retail]] (2), not committed yet.\n");
    let v = knowledge::add(&t, AddOpts { name: Some("Twin".into()), commit: Some(true), ..Default::default() }).unwrap();
    let fixes: Vec<Fix> = ["knowledge/topics/t1.md", "knowledge/topics/t2.md"].iter().map(|p| fix(&v, &format!("broken:{p}:[[Northwind Retail]]"), None).unwrap()).collect();
    let a = apply(&v, &fixes).unwrap();
    assert_eq!(a.written.len(), 2);
    let files = git::out(&t, &["show", "--name-only", "--format=", "HEAD"]).unwrap();
    assert_eq!(files.lines().collect::<Vec<_>>(), ["knowledge/topics/t1.md"], "only the file that was clean");
    let lines = crate::knowledge::activity::recent(&v, "2000-01-01T00:00:00+00:00");
    let commit_of = |p: &str| lines.iter().find(|e| e.path == p).unwrap().commit.clone();
    assert!(commit_of("knowledge/topics/t1.md").is_some());
    assert_eq!(commit_of("knowledge/topics/t2.md"), None, "it isn't in the commit");
}

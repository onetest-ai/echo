//! Made-up vaults and calls for the knowledge tests (no real people, companies or meetings).

use std::fs;
use std::path::{Path, PathBuf};

pub fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

/// A vault shaped like the owner's twin: `knowledge/` (Obsidian), `CATALOG.md` with a table per section, notes named by
/// date, a tag taxonomy, a sources folder, skills and a ledger.
pub fn twin(root: &Path) -> PathBuf {
    let v = root.to_path_buf();
    write(&v, "knowledge/.obsidian/app.json", "{\"useMarkdownLinks\": true, \"newLinkFormat\": \"relative\"}");
    write(&v, "knowledge/CATALOG.md", "# Knowledge Catalog\n\n> Last update: 2026-01-01\n\n## Clients\n\n| File | Description | Date Added |\n|------|-------------|------------|\n| [acme.md](clients/acme.md) | Acme Corp | 2026-01-02 |\n\n## Notes\n\n| File | Description | Date Added |\n|------|-------------|------------|\n| [2026-01-01-a.md](notes/2026-01-01-a.md) | first | 2026-01-01 |\n| [2026-01-02-b.md](notes/2026-01-02-b.md) | second | 2026-01-02 |\n");
    write(&v, "knowledge/TAG_TAXONOMY.md", "# Tags\n\n```\nmeeting, client, concept\n```\n");
    write(&v, "knowledge/clients/acme.md", "---\ntitle: Acme Corp\ntags: [client]\n---\n# Acme Corp\n\nSee [[2026-01-01-a]].\n");
    write(&v, "knowledge/notes/2026-01-01-a.md", "---\ntitle: First sync\ndate: 2026-01-01\ntags: [meeting]\n---\n# First sync\n\nWith [Acme](../clients/acme.md).\n");
    write(&v, "knowledge/notes/2026-01-02-b.md", "---\ntitle: Second sync\ntags: [meeting]\n---\n# Second sync\n");
    write(&v, "knowledge/notes/2026-01-03-c.md", "---\ntitle: Third sync\ntags: [meeting]\n---\n# Third\n");
    write(&v, "knowledge/notes/README.md", "# Notes\n");
    write(&v, "raw_inbox/deck.pdf", "%PDF-1.4 made up");
    write(&v, ".claude/skills/s/SKILL.md", "---\nname: s\ndescription: Does s\n---\n# S\n");
    write(&v, ".claude/commands/c.md", "---\ndescription: Does c\n---\nRun c.\n");
    write(&v, "AGENTS.md", "# Rules\n");
    write(&v, "processing_log.json", "{}");
    v
}

/// Every path under `root` with its size and modification time, for "nothing changed" checks.
pub fn snapshot(root: &Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut out = vec![];
    fn walk(base: &Path, d: &Path, out: &mut Vec<(String, u64, std::time::SystemTime)>) {
        let mut items: Vec<_> = fs::read_dir(d).unwrap().flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            let m = e.metadata().unwrap();
            let rel = e.path().strip_prefix(base).unwrap().display().to_string();
            out.push((rel, if m.is_dir() { 0 } else { m.len() }, m.modified().unwrap()));
            if m.is_dir() {
                walk(base, &e.path(), out);
            }
        }
    }
    walk(root, root, &mut out);
    out
}

/// A made-up recorded call with minutes, in the test's data folder: `name` like `2026-11-05_090012-weekly-sync`.
pub fn call(name: &str, title: &str, profile: &str, notes: &str) -> PathBuf {
    let dir = crate::paths::calls_dir().join(name);
    fs::create_dir_all(&dir).unwrap();
    let meta = serde_json::json!({"title": title, "app": "teams", "ai_profile": profile, "started_at": "2026-11-05T09:00:12+02:00", "ended_at": "2026-11-05T09:46:40+02:00"});
    fs::write(dir.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    let meeting = serde_json::json!({"subject": title, "attendees": [{"name": "Alex Doe", "email": "alex@example.com"}, {"name": "Dana Reyes", "email": "dana@northwind.example"}, {"name": "Sam Hale", "email": "sam@northwind.example"}], "organizer": {"name": "Dana Reyes", "email": "dana@northwind.example"}});
    fs::write(dir.join("meeting.json"), serde_json::to_vec(&meeting).unwrap()).unwrap();
    fs::write(dir.join("notes.md"), notes).unwrap();
    fs::write(dir.join("actions.json"), "[]").unwrap();
    dir
}

pub const MINUTES: &str = "# Weekly delivery sync\n\n2026-11-05, 46 min. Participants: Alex Doe (Me), Dana Reyes, Sam Hale\n\n## Summary\n\nThe team agreed to move the pilot | rollout to 18 November and to keep the pricing page as it is.\n\n## Decisions\n\n- Pilot moves to 18 November.\n\n## Action items\n\n- [ ] Me — send the revised plan to Dana by Friday.\n\n## Open questions\n\n- Who owns the data migration?\n";

//! A row in the vault's index (`CATALOG.md` and its kin), the way the owner's twin keeps it: the table of the notes'
//! section, one row per page, in date order (spec K3-03). Text in, text out — the caller reads the file just before and
//! writes it at once.

/// The row's cells as one Markdown table line, shaped to a table of `cols` columns: link · description · date.
pub fn row(cols: usize, link: &str, desc: &str, date: &str) -> String {
    let mut cells: Vec<String> = vec![link.to_string()];
    if cols >= 2 {
        cells.push(cell(desc));
    }
    if cols >= 3 {
        cells.push(date.to_string());
    }
    while cells.len() < cols {
        cells.push(String::new());
    }
    format!("| {} |", cells.join(" | "))
}

/// A cell's text: one line, no `|`.
pub fn cell(s: &str) -> String {
    s.replace('|', "/").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A Markdown link to `target` (relative to the index), its file name as the text; spaces as `%20`.
pub fn link(target: &str) -> String {
    let name = target.rsplit('/').next().unwrap_or(target);
    format!("[{}]({})", name.replace(']', "\\]"), target.replace('%', "%25").replace(' ', "%20").replace('(', "%28").replace(')', "%29"))
}

struct Table {
    heading: String,
    /// Line numbers: the header, and the rows after the separator.
    header: usize,
    rows: Vec<usize>,
    /// The line after the separator (where a first row goes).
    body: usize,
}

fn tables(lines: &[&str]) -> Vec<Table> {
    let mut out = vec![];
    let mut heading = String::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim_start();
        if t.starts_with('#') {
            heading = t.trim_start_matches('#').trim().to_string();
        }
        if t.starts_with('|') {
            let header = i;
            let mut j = i + 1;
            let sep = lines.get(j).is_some_and(|l| { let l = l.trim(); l.starts_with('|') && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) });
            if sep {
                j += 1;
            }
            let body = j;
            let mut rows = vec![];
            while j < lines.len() && lines[j].trim_start().starts_with('|') {
                rows.push(j);
                j += 1;
            }
            out.push(Table { heading: heading.clone(), header, rows, body });
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

fn columns(line: &str) -> usize {
    let t = line.trim();
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').count()
}

/// The last `YYYY-MM-DD` in a row.
fn row_date(line: &str) -> Option<&str> {
    let b = line.as_bytes();
    (0..b.len().saturating_sub(9)).rev().find(|&i| {
        b[i..i + 10].iter().enumerate().all(|(k, c)| if k == 4 || k == 7 { *c == b'-' } else { c.is_ascii_digit() })
    }).map(|i| &line[i..i + 10])
}

/// Put `[file](note)` in the index's notes table. `notes_dir` and `note` are relative to the index file's folder.
/// `Ok(Some(text))`: the new index; `Ok(None)`: it already lists the note; `Err(why)`: there is no table for notes.
pub fn insert(index: &str, notes_dir: &str, note: &str, desc: &str, date: &str, today: &str) -> Result<Option<String>, String> {
    let encoded = link(note);
    let target = &encoded[encoded.rfind("](").map(|i| i + 2).unwrap_or(0)..encoded.len() - 1];
    if index.contains(&format!("]({note})")) || index.contains(&format!("]({target})")) || index.contains(&format!("](<{note}>)")) {
        return Ok(None);
    }
    let lines: Vec<&str> = index.lines().collect();
    let ts = tables(&lines);
    // "action-items" ↔ "Action Items": compared as words.
    let words = |s: &str| s.to_lowercase().replace(['-', '_'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let folder = words(notes_dir.trim_end_matches('/').rsplit('/').next().unwrap_or(notes_dir));
    let notes_like = ["notes", "meetings", "meeting notes"];
    let named = |t: &&Table| { let h = words(&t.heading); notes_like.contains(&folder.as_str()) && notes_like.contains(&h.as_str()) };
    let links_in = |t: &Table| t.rows.iter().filter(|&&r| lines[r].contains(&format!("]({}/", notes_dir.trim_end_matches('/'))) || lines[r].contains(&format!("](<{}/", notes_dir.trim_end_matches('/')))).count();
    let table = ts.iter().find(|t| words(&t.heading) == folder).or_else(|| ts.iter().find(named)).or_else(|| ts.iter().filter(|t| links_in(t) > 0).max_by_key(|t| links_in(t)));
    let Some(t) = table else {
        let what = if notes_like.contains(&folder.as_str()) { "Notes".to_string() } else { format!("“{folder}”") };
        return Err(format!("no {what} table in the index — add a section for it"));
    };
    let cols = columns(lines[t.header]);
    let new_row = row(cols, &encoded, desc, date);
    let dated: Vec<(usize, &str)> = t.rows.iter().filter_map(|&r| row_date(lines[r]).map(|d| (r, d))).collect();
    let at = if dated.is_empty() {
        t.rows.last().map(|r| r + 1).unwrap_or(t.body)
    } else {
        match dated.iter().rev().find(|(_, d)| *d <= date) {
            Some((r, _)) => r + 1,
            None => dated[0].0,
        }
    };
    let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    out.insert(at.min(out.len()), new_row);
    for l in out.iter_mut().take(15) {
        if let Some(i) = l.find("Last update:") {
            let rest = &l[i + "Last update:".len()..];
            let trimmed = rest.trim_start();
            if trimmed.len() >= 10 && row_date(&trimmed[..10]).is_some() {
                *l = format!("{}Last update: {today}{}", &l[..i], &trimmed[10..]);
            }
        }
    }
    let mut text = out.join("\n");
    if index.ends_with('\n') {
        text.push('\n');
    }
    Ok(Some(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAT: &str = "# Catalog\n\n> Last update: 2026-01-01\n\n## Clients\n\n| File | Description | Date Added |\n|---|---|---|\n| [acme.md](clients/acme.md) | Acme | 2026-01-02 |\n\n## Notes\n\n| File | Description | Date Added |\n|------|-------------|------------|\n| [2026-10-01-a.md](notes/2026-10-01-a.md) | a | 2026-10-01 |\n| [2026-11-01-b.md](notes/2026-11-01-b.md) | b | 2026-11-01 |\n| [2026-12-01-c.md](notes/2026-12-01-c.md) | c | 2026-12-01 |\n\n## Media\n\n| File | Description |\n|---|---|\n";

    fn ins(index: &str, date: &str) -> String {
        insert(index, "notes", &format!("notes/{date}-x.md"), "46m Weekly sync — moved | the pilot", date, "2026-11-05").unwrap().unwrap()
    }

    #[test]
    fn rows_go_in_date_order() {
        let out = ins(CAT, "2026-11-05");
        let lines: Vec<&str> = out.lines().collect();
        let at = lines.iter().position(|l| l.contains("2026-11-05-x.md")).unwrap();
        assert!(lines[at - 1].contains("2026-11-01-b.md") && lines[at + 1].contains("2026-12-01-c.md"), "{out}");
        assert_eq!(lines[at], "| [2026-11-05-x.md](notes/2026-11-05-x.md) | 46m Weekly sync — moved / the pilot | 2026-11-05 |");
        assert!(out.contains("> Last update: 2026-11-05\n"), "the Last update line is today");
        // Before every dated row; after every one.
        let early = ins(CAT, "2026-09-01");
        assert!(early.find("2026-09-01-x").unwrap() < early.find("2026-10-01-a").unwrap());
        let late = ins(CAT, "2027-01-01");
        assert!(late.find("2027-01-01-x").unwrap() > late.find("2026-12-01-c").unwrap() && late.find("2027-01-01-x").unwrap() < late.find("## Media").unwrap());
    }

    #[test]
    fn an_empty_notes_table_takes_the_first_row() {
        let index = "# Catalog\n\n## Notes\n\n| File | Description | Date Added |\n|------|-------------|------------|\n\n## Media\n";
        let out = ins(index, "2026-11-05");
        assert!(out.contains("|------|-------------|------------|\n| [2026-11-05-x.md](notes/2026-11-05-x.md) | 46m Weekly sync — moved / the pilot | 2026-11-05 |\n\n## Media"), "{out}");
    }

    #[test]
    fn rows_fit_the_table_shape() {
        let two = "## Meetings\n\n| Note | What |\n|---|---|\n| [old.md](notes/old.md) | old |\n";
        assert!(ins(two, "2026-11-05").ends_with("| [old.md](notes/old.md) | old |\n| [2026-11-05-x.md](notes/2026-11-05-x.md) | 46m Weekly sync — moved / the pilot |\n"));
        let four = "## Notes\n\n| File | Description | Date | Owner |\n|---|---|---|---|\n";
        assert!(ins(four, "2026-11-05").contains("| [2026-11-05-x.md](notes/2026-11-05-x.md) | 46m Weekly sync — moved / the pilot | 2026-11-05 |  |"));
        // No heading named for the notes: the table that links into notes/.
        let by_links = "## Recent\n\n| File | Description |\n|---|---|\n| [a.md](notes/a.md) | a |\n\n## Other\n\n| File | Description |\n|---|---|\n| [x.md](clients/x.md) | x |\n";
        let out = ins(by_links, "2026-11-05");
        assert!(out.find("2026-11-05-x").unwrap() < out.find("## Other").unwrap());
    }

    #[test]
    fn no_notes_table_is_a_reason_not_a_guess() {
        let e = insert("# Catalog\n\nJust text.\n", "notes", "notes/a.md", "d", "2026-11-05", "2026-11-05").unwrap_err();
        assert!(e.contains("no Notes table"), "{e}");
        let only_clients = "## Clients\n\n| File | Description |\n|---|---|\n| [x.md](clients/x.md) | x |\n";
        assert!(insert(only_clients, "notes", "notes/a.md", "d", "2026-11-05", "2026-11-05").is_err());
    }

    #[test]
    fn a_listed_note_is_not_added_twice() {
        let once = ins(CAT, "2026-11-05");
        assert_eq!(insert(&once, "notes", "notes/2026-11-05-x.md", "again", "2026-11-05", "2026-11-05"), Ok(None));
    }

    #[test]
    fn links_and_cells_are_safe() {
        assert_eq!(link("notes/2026-03-01 Weekly.md"), "[2026-03-01 Weekly.md](notes/2026-03-01%20Weekly.md)");
        assert_eq!(cell("a | b\nc"), "a / b c");
        assert_eq!(row(3, "[x](y)", "d", "2026-01-01"), "| [x](y) | d | 2026-01-01 |");
        assert_eq!(row(1, "[x](y)", "d", "2026-01-01"), "| [x](y) |");
    }
}

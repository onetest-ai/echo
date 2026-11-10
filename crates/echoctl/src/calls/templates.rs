//! A call's minutes templates (split out of `calls.rs`, moved as it was).

use super::*;

pub(super) fn parse_template(text: &str, stem: &str, path: String) -> Option<Template> {
    let (front, body) = text.strip_prefix("---\n")?.split_once("\n---\n")?;
    let mut name = stem.to_string();
    let mut keywords = vec![];
    for line in front.lines() {
        if let Some(v) = line.strip_prefix("name:") {
            name = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("match:") {
            keywords = v
                .trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|k| k.trim().trim_matches('"').to_lowercase())
                .filter(|k| !k.is_empty())
                .collect();
        }
    }
    // The header: everything before the first `## ` line.
    let first = if body.starts_with("## ") { Some(0) } else { body.find("\n## ").map(|i| i + 1) };
    let (header, rest) = body.split_at(first.unwrap_or(body.len()));
    let mut sections = vec![];
    for chunk in rest.split("\n## ").map(|c| c.trim_start_matches("## ")) {
        let (heading, instruction) = chunk.split_once('\n').unwrap_or((chunk, ""));
        if !heading.trim().is_empty() {
            sections.push(json!({ "heading": heading.trim(), "instruction": instruction.trim() }));
        }
    }
    Some(Template { name, path, text: body.trim().to_string(), header: header.trim().to_string(), sections, keywords })
}

/// A memo's template: the user's own `memo.md` if they wrote one, else the built-in.
pub(super) fn memo_template() -> Option<Template> {
    let own = paths::state_dir().join("templates").join("memo.md");
    fs::read_to_string(&own).ok().and_then(|t| parse_template(&t, "memo", own.display().to_string())).or_else(|| {
        BUILTIN_TEMPLATES.iter().find(|(stem, _)| *stem == "memo").and_then(|(stem, text)| parse_template(text, stem, format!("builtin:{stem}")))
    })
}

/// The template for a meeting title among these, and why: the first with a word of the title (the user's first), with
/// that word, else Default (no word).
pub(super) fn pick_in<'a>(all: &'a [Template], title: Option<&str>) -> Option<(&'a Template, Option<String>)> {
    let t = title.unwrap_or("").to_lowercase();
    let all = || all.iter().filter(|t| t.name != "memo");
    all()
        .find_map(|tpl| tpl.keywords.iter().find(|k| t.contains(k.as_str())).map(|k| (tpl, Some(k.clone()))))
        .or_else(|| all().find(|tpl| tpl.name == "default").map(|t| (t, None)))
}

pub(super) fn templates_dir() -> PathBuf {
    paths::state_dir().join("templates")
}

/// Every template: the user's (`<home>/.echo/templates/*.md`) first, overriding a built-in of the same name.
/// Bumped by each save and removal of a template.
pub(super) static TEMPLATES_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub(super) fn all_templates() -> Vec<Template> {
    type Key = (u64, Vec<(PathBuf, Option<std::time::SystemTime>, u64)>);
    static CACHE: std::sync::Mutex<Option<(Key, Vec<Template>)>> = std::sync::Mutex::new(None);
    let mut files: Vec<PathBuf> = fs::read_dir(templates_dir())
        .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "md")).collect())
        .unwrap_or_default();
    files.sort();
    // Read again only when a file came, went or changed (a call's page asks on every refresh).
    // Echo's own saves count too, whatever the clock's grain.
    let key: Key = (TEMPLATES_GEN.load(std::sync::atomic::Ordering::SeqCst), files.iter().map(|f| (f.clone(), fs::metadata(f).ok().and_then(|m| m.modified().ok()), fs::metadata(f).map(|m| m.len()).unwrap_or(0))).collect());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, all)) = cache.as_ref() {
        if *k == key {
            return all.clone();
        }
    }
    let builtin_ids: Vec<&str> = BUILTIN_TEMPLATES.iter().map(|(s, _)| *s).collect();
    // A template is its file: the user's `<id>.md` is template `<id>` — or, when its frontmatter names a built-in
    // (`name: default`, how one was changed before Settings → Templates), that built-in. Saving, Reset and Delete go
    // to the file it came from (`path`).
    let user = files.iter().filter_map(|f| {
        let text = fs::read_to_string(f).ok()?;
        let stem = f.file_stem()?.to_string_lossy().to_string();
        let t = parse_template(&text, &stem, f.display().to_string())?;
        let id = if builtin_ids.contains(&t.name.as_str()) { t.name.clone() } else { stem };
        Some(Template { name: id, ..t })
    });
    let builtin = BUILTIN_TEMPLATES.iter().filter_map(|(stem, text)| parse_template(text, stem, format!("builtin:{stem}")));
    let mut all = vec![];
    let mut seen = std::collections::HashSet::new();
    for t in user.chain(builtin) {
        if seen.insert(t.name.clone()) {
            all.push(t);
        }
    }
    *cache = Some((key, all.clone()));
    all
}

/// The values a template may name in braces (`{date}`), each with where it comes from in the minutes' `context`: the
/// window lists them under a template's header, the skill fills them in. Any other `{…}` is an instruction in the
/// template's own words ("{one line on why we met}").
pub const TEMPLATE_VALUES: &[(&str, &str)] = &[
    ("title", "the meeting's title (its invite's subject, else the call's title)"),
    ("date", "the day of the call, as 2026-11-04 (`started_at`)"),
    ("time", "when it started, as 14:00 (`started_at`, local time)"),
    ("duration", "how long it was, as 43 min (`duration_min`)"),
    ("participants", "who took part: colleagues, then external, each linked from `links.people`"),
    ("colleagues", "the colleagues on the call, linked"),
    ("external", "the people from outside, linked"),
    ("organizer", "who sent the invite (`meeting.organizer`)"),
    ("type", "what kind of meeting it was, as settled in step 3b (`meeting_type.label`: Client, Internal…)"),
    ("thread", "the call's threads, linked from `links.threads`"),
    ("client", "the call's clients, linked from `links.clients`"),
    ("previous", "the previous meeting of this series, linked from `links.previous`"),
    ("me", "the user's name (`me.name`)"),
    ("language", "the language the minutes are written in (`dominant_language`)"),
];

pub(super) fn template_values() -> Value {
    json!(TEMPLATE_VALUES.iter().map(|(n, m)| json!({"name": n, "means": m})).collect::<Vec<_>>())
}

/// Settings → Templates → Minutes: each template — `kind` built-in, edited (the user's over a built-in) or yours —
/// with the words of a title it is used for and its sections. Built-ins first, in their order; then the user's by name.
pub fn templates_view() -> Value {
    let builtin: Vec<&str> = BUILTIN_TEMPLATES.iter().map(|(s, _)| *s).collect();
    let mut out: Vec<Value> = all_templates()
        .into_iter()
        .map(|t| {
            let kind = if t.path.starts_with("builtin:") { "builtin" } else if builtin.contains(&t.name.as_str()) { "edited" } else { "yours" };
            json!({"id": t.name, "kind": kind, "match": t.keywords, "header": t.header, "sections": t.sections, "memo": t.name == "memo"})
        })
        .collect();
    out.sort_by_key(|t| (builtin.iter().position(|b| t["id"] == *b).unwrap_or(usize::MAX), t["id"].as_str().unwrap_or("").to_string()));
    json!({"templates": out, "values": template_values(), "meeting_types": meeting_types(), "skills": crate::skills::list()})
}

/// The user's file a template comes from (none: it is Echo's own).
pub(super) fn file_of(id: &str) -> Option<PathBuf> {
    all_templates().into_iter().find(|t| t.name == id && !t.path.starts_with("builtin:")).map(|t| PathBuf::from(t.path))
}

/// A new template's id from its name: the call folders' slug (`Client kickoff` → `client-kickoff`), never a name
/// Windows keeps for its devices.
pub(super) fn template_id(name: &str) -> String {
    let id = crate::recorder::slugify(name).trim_end_matches('-').to_string();
    let reserved = ["con", "prn", "aux", "nul"].contains(&id.as_str())
        || (id.len() == 4 && (id.starts_with("com") || id.starts_with("lpt")) && id.ends_with(|c: char| c.is_ascii_digit()));
    if reserved {
        format!("{id}-template")
    } else {
        id
    }
}

/// Save a template the user made or changed: `<home>/.echo/templates/<id>.md`. A built-in keeps its id (so it is
/// edited, and Reset brings Echo's back); a new one takes its id from `name` and may not take an existing one.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn template_save(id: Option<&str>, name: &str, words: &[String], header: &str, sections: &[Value]) -> Result<String> {
    let known: Vec<String> = all_templates().into_iter().map(|t| t.name).collect();
    let id = match id.filter(|i| !i.is_empty()) {
        Some(i) => {
            anyhow::ensure!(known.iter().any(|k| k == i), "there is no template {i}");
            i.to_string()
        }
        None => {
            let i = template_id(name);
            anyhow::ensure!(!i.is_empty(), "give the template a name");
            // Not one there is, by name or by file (a file of the user's is never written over).
            anyhow::ensure!(!known.contains(&i) && !templates_dir().join(format!("{i}.md")).exists(), "a template named {i} exists already");
            i
        }
    };
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let words: Vec<String> = words.iter().map(|w| one_line(w).to_lowercase().replace([',', '[', ']', '"'], "")).filter(|w| !w.is_empty()).collect();
    let mut body = format!("---\nname: {id}\nmatch: [{}]\n---\n", words.join(", "));
    // Its header as written, but a line starting a section stays one level down (it is the header's).
    let header: Vec<String> = header.trim().lines().map(|l| if l.starts_with("## ") { format!("#{l}") } else { l.to_string() }).collect();
    if !header.is_empty() {
        body.push_str(&format!("{}\n\n", header.join("\n")));
    }
    let mut n = 0;
    for s in sections {
        let heading = one_line(s["heading"].as_str().unwrap_or("")).trim_start_matches('#').trim().to_string();
        if heading.is_empty() {
            continue;
        }
        // A line of its own words starting "## " would start another section: it stays one level down.
        let words: Vec<String> = s["instruction"].as_str().unwrap_or("").trim().lines().map(|l| if l.starts_with("## ") { format!("#{l}") } else { l.to_string() }).collect();
        body.push_str(&format!("## {heading}\n{}\n\n", words.join("\n")));
        n += 1;
    }
    anyhow::ensure!(n > 0, "a template needs at least one section");
    // Its own file; a built-in changed for the first time: `<id>.md` (its id is Echo's, no path in it).
    let file = file_of(&id).unwrap_or_else(|| templates_dir().join(format!("{id}.md")));
    fs::create_dir_all(templates_dir())?;
    paths::write_atomic(&file, body.trim_end().as_bytes())?;
    TEMPLATES_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ok(id)
}

/// Take the user's file of a template away: an edited built-in is Echo's own again, one of theirs is gone (kept in
/// `.echo/trash/templates/`).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn template_remove(id: &str) -> Result<()> {
    let f = file_of(id).ok_or_else(|| anyhow::anyhow!("{id} is Echo's own: nothing to take away"))?;
    let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let bin = trash_dir().join("templates");
    fs::create_dir_all(&bin)?;
    fs::rename(&f, paths::unused(&bin, &format!("{stem}-{}", chrono::Utc::now().format("%Y%m%d%H%M%S")), ".md"))?;
    TEMPLATES_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

/// Write a call's minutes again with another template: kept in its `meta.json` for the next runs too.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn choose_template(dir: &Path, id: &str) -> Result<()> {
    anyhow::ensure!(all_templates().iter().any(|t| t.name == id && t.name != "memo"), "there is no template {id}");
    update_meta(dir, |m| {
        m["template"] = json!(id);
        Ok(())
    })
}

/// The template a call's minutes follow and why: the user's choice for it (`meta.json` `template`, Write again
/// with…), a word of its title, or Default. A memo's is the memo template.
pub fn template_of(dir: &Path) -> Value {
    template_in(dir, &all_templates()).1
}

/// The template a call's minutes follow (`template_of`), whole.
pub(super) fn template_for(dir: &Path) -> Option<Template> {
    template_in(dir, &all_templates()).0
}

/// `template_of` among these templates (read once), with the template itself.
pub(super) fn template_in(dir: &Path, all: &[Template]) -> (Option<Template>, Value) {
    if crate::memo::is_memo(dir) {
        return (None, json!({"id": "memo", "why": "memo"}));
    }
    let chosen = read_json(&dir.join("meta.json"))["template"].as_str().and_then(|c| all.iter().find(|t| t.name == c && t.name != "memo"));
    if let Some(t) = chosen {
        return (Some(t.clone()), json!({"id": t.name, "why": "chosen"}));
    }
    match pick_in(all, subject_of(dir).as_deref()) {
        Some((t, Some(w))) => (Some(t.clone()), json!({"id": t.name, "why": "word", "word": w})),
        Some((t, None)) => (Some(t.clone()), json!({"id": t.name, "why": "default"})),
        None => (None, Value::Null),
    }
}

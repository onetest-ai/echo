//! A call's action items (split out of `calls.rs`, moved as it was).

use super::*;

// ── action items ────────────────────────────────────────────────────────────────────────────

pub const ACTIONS: &str = "actions.json";
/// The file an app run writes its new action list to (its prompt names it) before `echoctl actions
/// --apply`; only that run's (a session uses the skill's `actions.new.json`).
pub fn actions_new_of(run_id: &str) -> String {
    format!("actions.new-{run_id}.json")
}
/// Written by `apply_actions` only: which run last wrote the list (a status change never touches
/// it), so a run is `done` only when it wrote its own (`notes::finish_actions`).
pub const ACTIONS_GENERATION: &str = ".actions-generation";

/// The run that last wrote the call's action items (`ACTIONS_GENERATION`; "session" for a
/// Claude Code session's).
pub fn actions_written_by(dir: &Path) -> Option<String> {
    let v: Value = serde_json::from_slice(&fs::read(dir.join(ACTIONS_GENERATION)).ok()?).ok()?;
    v["run"].as_str().map(str::to_owned)
}
/// Set on an app run's Claude Code: `apply_actions` records it as the list's writer.
pub const RUN_ID: &str = "ECHOCTL_RUN_ID";

/// A new id for an action item: 8 lowercase letters and digits (no l, o, 0, 1).
pub(super) fn action_id() -> String {
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut b = [0u8; 8];
    if getrandom::fill(&mut b).is_err() {
        // No random source: the clock and a counter are unique enough within one list.
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        b = (t ^ n.rotate_left(32) ^ std::process::id() as u64).to_le_bytes();
    }
    b.iter().map(|x| ALPHABET[*x as usize % ALPHABET.len()] as char).collect()
}

pub(super) fn read_actions(dir: &Path) -> Result<Option<Vec<Value>>> {
    let path = dir.join(ACTIONS);
    match fs::read(&path) {
        Ok(b) => match serde_json::from_slice::<Value>(&b).with_context(|| format!("reading {}", path.display()))? {
            Value::Array(items) => Ok(Some(items)),
            _ => bail!("{} isn't a list", path.display()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

pub(super) fn write_actions(dir: &Path, items: &[Value]) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(items)?;
    bytes.push(b'\n');
    Ok(paths::write_atomic(&dir.join(ACTIONS), &bytes)?)
}

/// Before the call's action items are written again (a run's admission, `apply_actions`): a list
/// that isn't JSON, or isn't a list, is moved aside as `actions.unreadable-<HHMMSS>.json` — kept,
/// never deleted — so one bad file can't stop the minutes for good. True if one was.
pub(super) fn set_aside_unreadable(dir: &Path) -> Result<bool> {
    let path = dir.join(ACTIONS);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    if matches!(serde_json::from_slice::<Value>(&bytes), Ok(Value::Array(_))) {
        return Ok(false);
    }
    let aside = (0..100)
        .map(|n| dir.join(format!("actions.unreadable-{}{}.json", Local::now().format("%H%M%S"), if n == 0 { String::new() } else { format!("-{n}") })))
        .find(|p| !p.exists())
        .context("no free name for a copy of the action items")?;
    fs::rename(&path, &aside).with_context(|| format!("moving {} aside", path.display()))?;
    eprintln!("{} isn't a list of action items: kept as {}", path.display(), aside.display());
    Ok(true)
}

/// Give every item without one an id (in place); true if any was given.
pub(super) fn give_ids(items: &mut [Value]) -> bool {
    let mut given = false;
    for it in items.iter_mut().filter(|it| it["id"].as_str().is_none_or(str::is_empty)) {
        if it.is_object() {
            it["id"] = json!(action_id());
            given = true;
        }
    }
    given
}

/// A call's action items (None: none written), every one with an id. A list written before ids
/// existed gets them here, once, under the call's lock and written back — so neither the window,
/// the pane, the CLI nor a minutes run ever sees an item it couldn't address.
pub fn actions(dir: &Path) -> Result<Option<Vec<Value>>> {
    let items = read_actions(dir)?;
    if items.as_ref().is_none_or(|i| i.iter().all(|it| it["id"].as_str().is_some_and(|s| !s.is_empty()))) {
        return Ok(items);
    }
    let _lock = lock_meta(dir)?;
    actions_locked(dir)
}

/// `actions`, for a caller that holds the call's lock already (a run's admission: a list that
/// can't be read is set aside first — the run writes a new one).
pub(crate) fn actions_locked(dir: &Path) -> Result<Option<Vec<Value>>> {
    set_aside_unreadable(dir)?;
    let Some(mut items) = read_actions(dir)? else { return Ok(None) };
    if give_ids(&mut items) {
        write_actions(dir, &items)?;
    }
    Ok(Some(items))
}

/// Mark the action item `id` done or to do (the window, the pane, `echoctl action`). Under the
/// call's lock, so it lands in the list a minutes run applies at the same moment, or after it.
pub fn set_action_status(dir: &Path, id: &str, status: &str) -> Result<()> {
    anyhow::ensure!(status == "done" || status == "todo", "a status is done or todo, not {status}");
    let _lock = lock_meta(dir)?;
    let mut items = read_actions(dir)?.ok_or_else(|| anyhow!("this call has no action items"))?;
    let given = give_ids(&mut items);
    let Some(it) = items.iter_mut().find(|it| it["id"].as_str() == Some(id)) else {
        if given {
            write_actions(dir, &items)?;
        }
        bail!("that action item isn't in the list any more — look again");
    };
    it["status"] = json!(status);
    write_actions(dir, &items)
}

/// An action's task as compared across rewrites: lower case, single spaces.
pub(super) fn task_key(it: &Value) -> String {
    it["task"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Replace the call's action items with the list in `file` (what process-call writes): each item
/// whose `id` is in the current list keeps it and takes its current status (ticks made while the
/// minutes were being written survive); an item without a known id (one not in the current list
/// counts as none) takes the id and status of the one current item with the same owner and task,
/// if exactly one; the rest get new ids. A current list that can't be read is set aside first.
/// Records the writer in `.actions-generation` (`ECHOCTL_RUN_ID`, else a session's) and removes
/// `file` — so only a new list in the call's own folder (`actions.new.json`, a run's
/// `actions.new-<run>.json`) is taken: never `actions.json` itself, another call's or any other file.
pub fn apply_actions(dir: &Path, file: &Path) -> Result<usize> {
    let run = std::env::var(RUN_ID).ok().filter(|r| !r.is_empty()).unwrap_or_else(|| "session".into());
    apply_actions_as(dir, file, &run)
}

/// `apply_actions` recorded as `run`'s (the app finishing a run that left its list unapplied).
pub fn apply_actions_as(dir: &Path, file: &Path, run: &str) -> Result<usize> {
    // The file itself as it is on disk (a link named actions.new.json resolves to what it points
    // at), in the same folder however it is spelled; as written where neither can be resolved
    // (some Windows drives).
    let (real, in_call) = match (dunce::canonicalize(file), dunce::canonicalize(dir)) {
        (Ok(f), Ok(d)) => {
            let here = f.parent() == Some(d.as_path());
            (f, here)
        }
        (Err(_), Err(_)) => (file.to_path_buf(), file.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(".")) == dir),
        (Err(e), Ok(_)) => return Err(anyhow::Error::new(e).context(format!("reading {}", file.display()))),
        (Ok(_), Err(_)) => (file.to_path_buf(), false),
    };
    let name = real.file_name().and_then(|n| n.to_str()).unwrap_or("");
    anyhow::ensure!(
        name.starts_with("actions.new") && name.ends_with(".json") && in_call,
        "{} isn't a new action list in the call's folder (actions.new.json there)",
        file.display()
    );
    let text = fs::read_to_string(file).with_context(|| format!("reading {}", file.display()))?;
    let Value::Array(mut new) = serde_json::from_str::<Value>(&text).with_context(|| format!("{} isn't JSON", file.display()))? else {
        bail!("{} must be a JSON list of action items", file.display());
    };
    for (i, it) in new.iter().enumerate() {
        anyhow::ensure!(it.is_object(), "item {} isn't an object", i + 1);
        anyhow::ensure!(it["task"].as_str().is_some_and(|t| !t.trim().is_empty()), "item {} has no task", i + 1);
        if let Some(st) = it.get("status").filter(|v| !v.is_null()) {
            anyhow::ensure!(st == "todo" || st == "done", "item {}: status is todo or done", i + 1);
        }
    }
    let mut seen = std::collections::HashSet::new();
    for id in new.iter().filter_map(|it| it["id"].as_str().filter(|s| !s.is_empty())) {
        anyhow::ensure!(seen.insert(id.to_string()), "the id {id} is used twice");
    }
    let _lock = lock_meta(dir)?;
    set_aside_unreadable(dir)?;
    let mut current = read_actions(dir)?.unwrap_or_default();
    give_ids(&mut current);
    let by_id: std::collections::HashMap<String, &Value> = current.iter().filter_map(|it| Some((it["id"].as_str()?.to_string(), it))).collect();
    let mut taken: std::collections::HashSet<String> = new.iter().filter_map(|it| it["id"].as_str().filter(|id| by_id.contains_key(*id)).map(str::to_owned)).collect();
    for it in new.iter_mut() {
        match it["id"].as_str().map(str::to_owned).filter(|id| by_id.contains_key(id)) {
            Some(id) => it["status"] = by_id[&id]["status"].clone(),
            None => {
                let same: Vec<&&Value> = by_id
                    .values()
                    .filter(|c| task_key(c) == task_key(it) && c["owner"] == it["owner"] && !taken.contains(c["id"].as_str().unwrap_or("")))
                    .collect();
                if let [one] = same.as_slice() {
                    let id = one["id"].as_str().unwrap_or("").to_string();
                    it["status"] = one["status"].clone();
                    it["id"] = json!(id);
                    taken.insert(id);
                } else {
                    it["id"] = json!(action_id());
                }
            }
        }
        if !(it["status"] == "done" || it["status"] == "todo") {
            it["status"] = json!("todo");
        }
    }
    write_actions(dir, &new)?;
    paths::write_atomic(&dir.join(ACTIONS_GENERATION), serde_json::to_string(&json!({ "run": run, "at": Local::now().to_rfc3339(), "items": new.len() }))?.as_bytes())?;
    let _ = fs::remove_file(file);
    Ok(new.len())
}

/// `echoctl action <call> <id> done|todo`.
pub fn action_command(call: &str, id: &str, status: &str) -> Result<()> {
    set_action_status(&resolve_call(call)?, id, status)?;
    println!("{status}");
    Ok(())
}

/// `echoctl actions <call> [--apply <file>]`: the list (with ids) as JSON, or apply a new one.
pub fn actions_command(call: &str, apply: Option<&Path>) -> Result<()> {
    let dir = resolve_call(call)?;
    match apply {
        Some(file) => println!("applied {} action items", apply_actions(&dir, file)?),
        None => println!("{}", serde_json::to_string_pretty(&actions(&dir)?)?),
    }
    Ok(())
}

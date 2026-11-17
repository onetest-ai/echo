//! A plugin's own routines (`routines` in its manifest): each a prompt its package ships, run by Echo as a routine —
//! Claude Code on the user's default AI profile as the folder session (`claudejob::run`), acting on its own as the
//! routine itself says (`tools`, `skip` only with `claude.skip` on its sheet), else as the user set routines.
//!
//! Each is a line of the plugin's sheet, with the hash of its definition and prompt: an update that changes either asks
//! again, and the user's yes to it keeps the routine as they had it; one the update adds starts off. It is off until the
//! user switches it on — Settings → Plugins, `echoctl jobs set <plugin>.<routine> on=on` — never by a plugin or a run. Its settings (`on`, `every`, `night`, `model`) are kept with Echo's jobs'
//! (`jobs.json`) and its runs as theirs (`jobs-runs/<id>.jsonl`, in Echo's words and counts), each run's folder —
//! prompt, log, answer, the changes and sends it made — in `jobs-runs/<id>/<run>/`, the newest 20 kept. The app runs
//! one that is on every `every` minutes (`start_scheduler`); `echoctl jobs run` runs one now.

use super::*;

/// The routines one plugin may declare.
pub const MAX: usize = 4;
/// How often a routine may run, in minutes.
pub const EVERY: (u64, u64) = (15, 1440);
const PROMPT_MAX: usize = 16 * 1024;
/// Run folders kept a routine.
const KEEP_RUNS: usize = 20;
const MAY: &str = "it may change your Echo data and send as you, as your Claude · Echo folder session may";

/// A routine as its manifest declares it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutineDef {
    pub id: String,
    pub name: String,
    pub what: String,
    /// A `.md` file in the package: what Claude is asked.
    pub prompt: String,
    /// Minutes between runs, by default (the user may change it).
    #[serde(default)]
    pub every: u64,
    /// Paused at night, by default.
    #[serde(default = "yes")]
    pub night: bool,
    /// `auto`, `skip`, `none`, or nothing: the user's setting for routines (`claudejob::Tools`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tools: String,
    /// Minutes a run may take.
    #[serde(default = "twenty")]
    pub timeout: u64,
    /// The hash of its definition and its prompt, on the sheet. Always Echo's: `check` sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

fn yes() -> bool {
    true
}
fn twenty() -> u64 {
    20
}

/// Its manifest's routines, checked: the problems, each naming its routine. Each one that passes gets its `digest`.
pub(super) fn check(dir: &Path, m: &mut Manifest) -> Vec<anyhow::Error> {
    let mut out = vec![];
    if m.routines.len() > MAX {
        out.push(anyhow!("it declares more than {MAX} routines"));
    }
    let claude = m.uses.contains_key("claude");
    let mut ids = std::collections::HashSet::new();
    for r in &mut m.routines {
        r.digest = None;
        match check_one(dir, r, claude, &mut ids) {
            Ok(d) => r.digest = Some(d),
            Err(e) => out.push(e),
        }
    }
    out
}

fn check_one(dir: &Path, r: &RoutineDef, claude: bool, ids: &mut std::collections::HashSet<String>) -> Result<String> {
    let id = &r.id;
    if !valid_id(id) || !ids.insert(id.clone()) {
        bail!("routine “{id}” needs a plain, unique id (a–z, 0–9, -)");
    }
    let line = |s: &str, max: usize| !s.trim().is_empty() && s.chars().count() <= max && !s.chars().any(char::is_control);
    if !line(&r.name, 60) {
        bail!("routine “{id}”: its name is one line of 1–60 characters");
    }
    if !line(&r.what, 200) {
        bail!("routine “{id}”: its what is one line of 1–200 characters");
    }
    if !(EVERY.0..=EVERY.1).contains(&r.every) {
        bail!("routine “{id}”: every is {} to {} minutes", EVERY.0, EVERY.1);
    }
    if !(1..=60).contains(&r.timeout) {
        bail!("routine “{id}”: timeout is 1 to 60 minutes");
    }
    if !matches!(r.tools.as_str(), "" | "auto" | "skip" | "none") {
        bail!("routine “{id}”: tools is auto, skip or none (or left out: the user's setting for routines)");
    }
    if !claude {
        bail!("routine “{id}” runs Claude Code: the manifest needs “claude” in uses");
    }
    let text = prompt_text(dir, r).map_err(|e| anyhow!("routine “{id}”: {e:#}"))?;
    use sha2::Digest as _;
    let def = json!({"id": r.id, "name": r.name, "what": r.what, "every": r.every, "night": r.night, "tools": r.tools, "timeout": r.timeout});
    Ok(format!("{:x}", sha2::Sha256::digest(format!("{def}\n{text}").as_bytes()))[..8].to_string())
}

/// Its prompt: a `.md` file of the package, UTF-8, not empty, 16 KB at most.
fn prompt_text(dir: &Path, r: &RoutineDef) -> Result<String> {
    if !r.prompt.ends_with(".md") {
        bail!("its prompt is a .md file in the plugin");
    }
    let bytes = fs::read(inside_plugin(dir, &r.prompt)?)?;
    if bytes.len() > PROMPT_MAX {
        bail!("{} is longer than 16 KB", r.prompt);
    }
    let text = String::from_utf8(bytes).map_err(|_| anyhow!("{} isn't UTF-8 text", r.prompt))?;
    if text.trim().is_empty() {
        bail!("{} is empty", r.prompt);
    }
    Ok(text)
}

/// The sheet's line of each routine: its name, how often once the user switches it on, what its run may do, its hash.
pub(super) fn sheet_lines(m: &Manifest) -> Vec<String> {
    let skip = m.uses.get("claude").is_some_and(|c| c["skip"] == true);
    m.routines
        .iter()
        .map(|r| {
            let how = match r.tools.as_str() {
                "none" => "with no tools: it can only answer".to_string(),
                "auto" => format!("acting on its own in auto mode — {MAY}"),
                "skip" if skip => format!("acting on its own, skipping every permission check — {MAY}"),
                _ => format!("acting on its own as you set routines — {MAY}"),
            };
            let digest = r.digest.as_deref().map(|d| format!(" (its prompt: {d})")).unwrap_or_default();
            format!("Has a routine “{}”, off until you switch it on in Settings → Plugins: then every {} min Echo runs Claude Code on your default AI profile with the prompt it ships, {how}{digest}", r.name, r.every)
        })
        .collect()
}

/// A version of `old` about to replace it (an update, an install over it): each routine `new` adds starts off, whatever
/// an earlier version of it left in the user's settings; one it keeps, changed or not, stays as the user had it (their
/// yes to the update, whose sheet named the change, is their yes to it).
pub(super) fn switch_off_new(old: &Installed, new: &Manifest) -> Result<()> {
    let before: Vec<RoutineDef> = read_manifest(&plugin_dir(old)).map(|m| m.routines).unwrap_or_default();
    let added: Vec<String> = new.routines.iter().filter(|r| !before.iter().any(|b| b.id == r.id)).map(|r| r.id.clone()).collect();
    crate::jobs::switch_off_routines(&old.id, &added)
}

/// A routine's id among Echo's jobs and routines: `<plugin>.<routine>` (Echo's jobs have no dot).
pub fn id_of(plugin: &str, routine: &str) -> String {
    format!("{plugin}.{routine}")
}

/// The installed plugin, its manifest and the routine `id` names.
fn find(id: &str) -> Option<(Installed, Manifest, RoutineDef)> {
    let (plugin, routine) = id.split_once('.')?;
    let p = Registry::load().ok()?.installed.remove(plugin)?;
    let m = read_manifest(&plugin_dir(&p)).ok()?;
    let r = m.routines.iter().find(|r| r.id == routine)?.clone();
    Some((p, m, r))
}

/// Every installed plugin's routines (on or off).
pub fn all() -> Vec<(Installed, RoutineDef)> {
    let Ok(r) = Registry::load() else { return vec![] };
    r.installed.into_values().filter_map(|p| read_manifest(&plugin_dir(&p)).ok().map(|m| (p, m.routines))).flat_map(|(p, rs)| rs.into_iter().map(move |r| (p.clone(), r))).collect()
}

/// A routine's settings (`jobs::routine_fields`), while its plugin is installed.
pub fn fields(id: &str) -> Option<Vec<crate::jobs::Field>> {
    find(id).map(|(_, _, r)| crate::jobs::routine_fields(r.every, r.night))
}

fn runs_dir() -> PathBuf {
    paths::config_dir().join("jobs-runs")
}

fn lock_path(id: &str) -> PathBuf {
    runs_dir().join(format!(".{id}.lock"))
}

/// Is a run of `id` under way, in any process (it holds the routine's lock)?
pub fn running(id: &str) -> bool {
    let p = lock_path(id);
    p.exists() && matches!(crate::app::FileLock::try_take(&p), Ok(None))
}

/// Settings → Plugins: a plugin's routines, each with its switch and when it runs, in Echo's words.
pub fn view(plugin: &str) -> Vec<Value> {
    all()
        .into_iter()
        .filter(|(p, _)| p.id == plugin)
        .map(|(_, r)| {
            let id = id_of(plugin, &r.id);
            let on = crate::jobs::on(&id);
            let night = crate::jobs::settings()[&id]["night"].as_bool().unwrap_or(r.night);
            let when = format!("Every {} min{}{}", crate::jobs::number(&id, "every"), if night { " · paused at night" } else { "" }, if on { "" } else { " · off" });
            let last = crate::jobs::runs(&id, 1).ok().and_then(|v| v.into_iter().next()).map(|x| {
                let at = x["started"].as_str().and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
                format!("Last run {at}: {}", x["summary"].as_str().or(x["error"].as_str()).unwrap_or(if x["ok"] == false { "failed" } else { "done" }))
            });
            json!({"id": id, "routine": r.id, "name": r.name, "what": r.what, "on": on, "when": when, "last": last})
        })
        .collect()
}

/// The user's switch (Settings → Plugins): the routine on or off.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn switch(plugin: &str, routine: &str, on: bool) -> Result<()> {
    let id = id_of(plugin, routine);
    find(&id).ok_or_else(|| anyhow!("{plugin} has no routine “{routine}”"))?;
    crate::jobs::set(&id, &[format!("on={}", if on { "on" } else { "off" })]).map(|_| ())
}

/// A run's words in its record: Echo's own and counts, never the model's text.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn summary(actions: &[Value]) -> String {
    let sends = actions.iter().filter(|a| a["kind"] == "send").count();
    let changes = actions.len() - sends;
    let n = |k: usize, w: &str| format!("{k} {w}{}", if k == 1 { "" } else { "s" });
    let parts: Vec<String> = [(changes, "change"), (sends, "send")].into_iter().filter(|(k, _)| *k > 0).map(|(k, w)| n(k, w)).collect();
    if parts.is_empty() {
        "done".into()
    } else {
        format!("done: {} through Echo", parts.join(" and "))
    }
}

/// Run `id` now (the app's schedule, or `echoctl jobs run`): one run at a time across processes. Its record (the run in
/// the routines' one shape); a run that failed is recorded, then its error returned.
#[cfg(feature = "js")]
pub fn run(id: &str) -> Result<Value> {
    if crate::notes::headless() {
        bail!("refused inside a run Echo started: a routine runs only on its schedule or when you run it");
    }
    let (p, m, r) = find(id).ok_or_else(|| anyhow!("no routine “{id}” of an installed plugin"))?;
    if !p.enabled {
        bail!("{} is off: its routines don't run", p.name);
    }
    fs::create_dir_all(runs_dir())?;
    // A look at whether it runs (`running`) holds the lock a moment: tried a few times before it counts as running.
    let mut lock = None;
    for _ in 0..5 {
        lock = crate::app::FileLock::try_take(&lock_path(id))?;
        if lock.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let Some(_lock) = lock else { bail!("“{}” is running already", r.name) };
    let last = crate::jobs::runs(id, 1).ok().and_then(|v| v.into_iter().next()).and_then(|x| x["started"].as_str().map(str::to_owned));
    let started = chrono::Local::now();
    let base = started.format("%Y%m%d-%H%M%S").to_string();
    let folder = runs_dir().join(id);
    let stamp = (1..).map(|n| if n == 1 { base.clone() } else { format!("{base}-{n}") }).find(|s| !folder.join(s).exists()).unwrap_or(base);
    let run_dir = folder.join(&stamp);
    // A run that fails before Claude Code starts (its sheet, its prompt, the profile) is recorded too, its log saying why:
    // the schedule then waits its `every` before it tries again.
    let failed = |e: anyhow::Error| -> Result<Value> {
        let _ = fs::create_dir_all(&run_dir);
        let log = run_dir.join("log.txt");
        let before = fs::read_to_string(&log).unwrap_or_default();
        let _ = fs::write(&log, format!("{before}{}  {e:#}\n", chrono::Local::now().format("%H:%M:%S")));
        crate::jobs::record(id, started, Err("the run failed: its log says why".into()), json!({"run": stamp}));
        prune(&folder);
        Err(e)
    };
    // Only what its sheet said: Claude Code at all, and its own `skip` only with `claude.skip` (else the user's setting).
    let tools = match may_run_claude(&m.uses, &r.tools).and_then(crate::claudejob::Tools::parse) {
        Ok(t) => t,
        Err(e) => return failed(e),
    };
    let prompt = match prompt_text(&plugin_dir(&p), &r) {
        Ok(t) => t,
        Err(e) => return failed(e),
    };
    let text = format!(
        "This is the routine “{}” of the plugin “{}”, run by Echo for the user (every {} min while it is on). {}\nIts answer is kept for the user with its run; nobody is sent it.\n\n{}",
        r.name,
        p.name,
        crate::jobs::number(id, "every"),
        last.map_or("It hasn't run before.".to_string(), |t| format!("Its last run started {t}.")),
        prompt.trim()
    );
    // The user's model for it, as their default profile maps it (none on a local profile that maps no such name).
    let model = crate::ai::Settings::load().ok().and_then(|s| s.get(&crate::ai::default_id()).ok()).and_then(|pr| crate::jobs::model_for(id, &pr)).unwrap_or_default();
    let spec = crate::claudejob::Spec {
        title: format!("{} · {}", p.name, r.name),
        prompt: text,
        profile: String::new(),
        model,
        tools,
        run_dir: run_dir.clone(),
        timeout: std::time::Duration::from_secs(r.timeout * 60),
        session: tools != crate::claudejob::Tools::None,
        task: None,
        thread: None,
        yolo: false,
        folder: None,
    };
    let out = crate::claudejob::run(&spec);
    if let Err(e) = &out {
        // `claudejob::run` logs what Claude Code did; an error before it started (no profile, not signed in) is said here.
        let log = run_dir.join("log.txt");
        let before = fs::read_to_string(&log).unwrap_or_default();
        if !before.contains(&format!("{e:#}")) {
            let _ = fs::write(&log, format!("{before}{}  {e:#}\n", chrono::Local::now().format("%H:%M:%S")));
        }
    }
    let actions = crate::notes::run_actions(&run_dir.join("actions.jsonl"));
    let sends = actions.iter().filter(|a| a["kind"] == "send").count();
    let mut used = json!({"run": stamp, "changes": actions.len() - sends, "sends": sends});
    let result = match &out {
        Ok((_, model, u)) => {
            used["calls"] = json!(1);
            for (to, from) in [("input", "input_tokens"), ("output", "output_tokens"), ("cache_read", "cache_read_tokens"), ("cache_write", "cache_write_tokens"), ("cost_usd", "cost_usd")] {
                used[to] = u[from].clone();
            }
            used["model"] = json!(model);
            Ok(summary(&actions))
        }
        Err(_) => Err("the run failed: its log says why".to_string()),
    };
    crate::jobs::record(id, started, result, used);
    prune(&folder);
    out?;
    crate::jobs::routine_runs(id, 1)?.into_iter().next().ok_or_else(|| anyhow!("the run wasn't recorded"))
}

/// The newest `KEEP_RUNS` run folders kept.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn prune(folder: &Path) {
    let mut runs: Vec<PathBuf> = fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    runs.sort();
    for p in runs.iter().take(runs.len().saturating_sub(KEEP_RUNS)) {
        let _ = fs::remove_dir_all(p);
    }
}

/// `echoctl jobs outcome <id> [--run <run>]`: a run's answer, the changes and sends it made, its permissions and log —
/// the user's to read (the record a plugin reads keeps Echo's words only).
pub fn outcome(id: &str, run: Option<&str>) -> Result<Value> {
    let runs = crate::jobs::routine_runs(id, 500)?;
    let record = match run {
        Some(want) => runs.into_iter().find(|x| x["run"] == want).ok_or_else(|| anyhow!("{id} has no run “{want}”"))?,
        None => runs.into_iter().find(|x| x["run"].is_string()).ok_or_else(|| anyhow!("{id} hasn't run yet"))?,
    };
    let stamp = record["run"].as_str().unwrap_or("").to_string();
    if stamp.is_empty() || !stamp.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        bail!("{id} has no run “{stamp}”");
    }
    let dir = runs_dir().join(id).join(&stamp);
    if !dir.is_dir() {
        bail!("{id}'s run “{stamp}” is no longer kept (Echo keeps the newest {KEEP_RUNS})");
    }
    let read = |n: &str| fs::read_to_string(dir.join(n)).unwrap_or_default();
    Ok(json!({
        "id": id, "run": stamp, "record": record, "answer": read("answer.md"),
        "actions": crate::notes::run_actions(&dir.join("actions.jsonl")),
        // The mail drafts it wrote (`echoctl compose`), as Yolo's outcome gives them.
        "drafts": crate::drafts::of_run(&dir.join("drafts.jsonl"), |_| None),
        "permissions": serde_json::from_str::<Value>(&read("permissions.json")).unwrap_or(Value::Null),
        "log": read("log.txt"),
    }))
}

/// The routines due at `now`: switched on, their plugin on, none of theirs running, their time come (`jobs::routine_next`).
#[cfg_attr(not(all(feature = "js", feature = "tray")), allow(dead_code))]
fn due(now: chrono::DateTime<chrono::Local>) -> Vec<String> {
    all().into_iter()
        .filter(|(p, _)| p.enabled)
        .map(|(p, r)| id_of(&p.id, &r.id))
        .filter(|id| crate::jobs::on(id) && !running(id) && crate::jobs::routine_next(id, now) <= now)
        .collect()
}

/// The app: every 30 s, each routine that is due runs on its own thread.
#[cfg(all(feature = "js", feature = "tray"))]
pub fn start_scheduler() {
    std::thread::spawn(|| loop {
        std::thread::sleep(std::time::Duration::from_secs(30));
        if crate::notes::quitting() {
            continue;
        }
        for id in due(chrono::Local::now()) {
            std::thread::spawn(move || {
                if let Err(e) = run(&id) {
                    eprintln!("routine {id}: {e:#}");
                }
            });
        }
    });
}

/// For tests: plugin `id` with one routine (`routine` over a good one: `mail-notes`, every 60 min) and its prompt, in a
/// local marketplace at `market`, installed (its sheet agreed). Its marketplace's name.
#[cfg(test)]
pub(crate) fn test_install(market: &Path, id: &str, uses: Value, routine: Value, prompt: &str) -> String {
    test_package(&market.join("plugins").join(id), id, uses, routine, prompt);
    fs::write(market.join(CATALOG), json!({"name": "Test", "plugins": [{"id": id, "source": format!("./plugins/{id}")}]}).to_string()).unwrap();
    let name = market_add(market.to_str().unwrap(), Some(&format!("m-{id}"))).unwrap();
    refresh(&name).unwrap();
    let agreed = review(id, Some(&name)).unwrap().uses;
    install(id, Some(&name), &agreed).unwrap();
    name
}

#[cfg(test)]
fn test_package(dir: &Path, id: &str, uses: Value, routine: Value, prompt: &str) {
    let mut def = json!({"id": "mail-notes", "name": "Mail → notes", "what": "Reads the new mail into notes", "prompt": "routines/mail-notes.md", "every": 60});
    for (k, v) in routine.as_object().unwrap() {
        def[k] = v.clone();
    }
    fs::create_dir_all(dir.join("routines")).unwrap();
    fs::write(dir.join(MANIFEST), json!({"id": id, "name": "Acme routine", "version": "0.1.0", "uses": uses, "routines": [def]}).to_string()).unwrap();
    fs::write(dir.join("routines/mail-notes.md"), prompt).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(dir: &Path) -> Vec<String> {
        check_package(dir).unwrap().1.into_iter().map(|p| p.what).collect()
    }

    fn routine_line(m: &Manifest) -> String {
        sheet(m).unwrap().into_iter().find(|l| l.starts_with("Has a routine")).unwrap_or_default()
    }

    #[test]
    fn a_routine_is_checked_named_on_the_sheet_and_its_prompt_change_asks_again() {
        let (_env, root) = paths::testenv::folders("routines-check");
        let d = root.join("pkg");
        test_package(&d, "acme-routine", json!({"claude": {}}), json!({}), "Read the mail.");
        assert_eq!(problems(&d), Vec::<String>::new());
        let m = read_manifest(&d).unwrap();
        let digest = m.routines[0].digest.clone().unwrap();
        assert_eq!(digest.len(), 8);
        assert_eq!(
            routine_line(&m),
            format!("Has a routine “Mail → notes”, off until you switch it on in Settings → Plugins: then every 60 min Echo runs Claude Code on your default AI profile with the prompt it ships, acting on its own as you set routines — {MAY} (its prompt: {digest})")
        );
        // Its prompt or its definition changed: another hash, so another line (an update asks again).
        fs::write(d.join("routines/mail-notes.md"), "Read the mail and the chats.").unwrap();
        let other = read_manifest(&d).unwrap().routines[0].digest.clone().unwrap();
        assert_ne!(other, digest);
        test_package(&d, "acme-routine", json!({"claude": {}}), json!({"every": 30}), "Read the mail.");
        assert_ne!(read_manifest(&d).unwrap().routines[0].digest.clone().unwrap(), digest);
        // What its run may do, in Echo's words: its own Auto, no tools, and skip only when its sheet allows it.
        for (uses, tools, how) in [
            (json!({"claude": {}}), "auto", format!("acting on its own in auto mode — {MAY}")),
            (json!({"claude": {}}), "none", "with no tools: it can only answer".to_string()),
            (json!({"claude": {}}), "skip", format!("acting on its own as you set routines — {MAY}")),
            (json!({"claude": {"skip": true}}), "skip", format!("acting on its own, skipping every permission check — {MAY}")),
        ] {
            test_package(&d, "acme-routine", uses, json!({"tools": tools}), "Read the mail.");
            assert!(routine_line(&read_manifest(&d).unwrap()).contains(&format!("with the prompt it ships, {how} (its prompt: ")), "{tools}");
        }
        // A routine breaking a rule: named, with why.
        let big = "x".repeat(16 * 1024 + 1);
        for (uses, r, prompt, want) in [
            (json!({"claude": {}}), json!({"every": 5}), "p", "routine “mail-notes”: every is 15 to 1440 minutes"),
            (json!({"claude": {}}), json!({"every": 2000}), "p", "every is 15 to 1440 minutes"),
            (json!({}), json!({}), "p", "routine “mail-notes” runs Claude Code: the manifest needs “claude” in uses"),
            (json!({"claude": {}}), json!({"id": "Mail_Notes"}), "p", "routine “Mail_Notes” needs a plain, unique id"),
            (json!({"claude": {}}), json!({"name": ""}), "p", "its name is one line of 1–60 characters"),
            (json!({"claude": {}}), json!({"what": "two\nlines"}), "p", "its what is one line of 1–200 characters"),
            (json!({"claude": {}}), json!({"tools": "ask"}), "p", "tools is auto, skip or none"),
            (json!({"claude": {}}), json!({"timeout": 0}), "p", "timeout is 1 to 60 minutes"),
            (json!({"claude": {}}), json!({"prompt": "routines/missing.md"}), "p", "routine “mail-notes”: routines/missing.md isn't in the plugin"),
            (json!({"claude": {}}), json!({"prompt": "../outside.md"}), "p", "isn't a file inside the plugin"),
            (json!({"claude": {}}), json!({"prompt": "main.js"}), "p", "its prompt is a .md file in the plugin"),
            (json!({"claude": {}}), json!({}), big.as_str(), "routines/mail-notes.md is longer than 16 KB"),
            (json!({"claude": {}}), json!({}), " \n", "routines/mail-notes.md is empty"),
        ] {
            let _ = fs::remove_dir_all(&d);
            test_package(&d, "acme-routine", uses, r.clone(), prompt);
            let got = problems(&d);
            assert!(got.iter().any(|p| p.contains(want)), "{r}: {got:?}");
            assert!(read_manifest(&d).is_err(), "an install refuses it: {r}");
        }
        // Two with one id, and more than four.
        let _ = fs::remove_dir_all(&d);
        test_package(&d, "acme-routine", json!({"claude": {}}), json!({}), "p");
        let mut m: Value = serde_json::from_str(&fs::read_to_string(d.join(MANIFEST)).unwrap()).unwrap();
        let one = m["routines"][0].clone();
        m["routines"] = json!([one, one]);
        fs::write(d.join(MANIFEST), m.to_string()).unwrap();
        assert!(problems(&d).iter().any(|p| p.contains("routine “mail-notes” needs a plain, unique id")));
        m["routines"] = json!((0..5).map(|i| { let mut x = one.clone(); x["id"] = json!(format!("r{i}")); x }).collect::<Vec<_>>());
        fs::write(d.join(MANIFEST), m.to_string()).unwrap();
        assert!(problems(&d).iter().any(|p| p == "it declares more than 4 routines"));
        // Installed, then a version whose prompt changed: the routine's line is new, so the update asks again.
        let mk = root.join("market");
        let name = test_install(&mk, "acme-routine", json!({"claude": {}}), json!({}), "Read the mail.");
        let pkg = mk.join("plugins/acme-routine");
        let mut m: Value = serde_json::from_str(&fs::read_to_string(pkg.join(MANIFEST)).unwrap()).unwrap();
        m["version"] = json!("0.2.0");
        fs::write(pkg.join(MANIFEST), m.to_string()).unwrap();
        fs::write(pkg.join("routines/mail-notes.md"), "Read the mail and the meeting chats.").unwrap();
        refresh(&name).unwrap();
        let rv = review("acme-routine", Some(&name)).unwrap();
        assert_eq!(rv.new_uses.len(), 1, "{:?}", rv.new_uses);
        assert!(rv.new_uses[0].starts_with("Has a routine “Mail → notes”"));
        crate::jobs::set("acme-routine.mail-notes", &["on=on".into(), "every=30".into()]).unwrap();
        assert!(update("acme-routine", &[]).is_err(), "it waits for the user's yes");
        assert!(crate::jobs::on("acme-routine.mail-notes"));
        // Taken: the user's yes to the update, whose sheet named the change, keeps the routine as they had it.
        update("acme-routine", &rv.new_uses).unwrap().unwrap();
        assert_eq!((crate::jobs::on("acme-routine.mail-notes"), crate::jobs::number("acme-routine.mail-notes", "every")), (true, 30));
        // A version without it (installed over the installed one), then an update that adds it again: the added routine
        // starts off, whatever was left of before.
        let routine = m["routines"].clone();
        m["routines"] = json!([]);
        m["version"] = json!("0.3.0");
        fs::write(pkg.join(MANIFEST), m.to_string()).unwrap();
        refresh(&name).unwrap();
        install("acme-routine", Some(&name), &review("acme-routine", Some(&name)).unwrap().uses).unwrap();
        assert!(crate::jobs::on("acme-routine.mail-notes"), "no routine of that id now: its switch of before is left as it was");
        m["routines"] = routine;
        m["version"] = json!("0.4.0");
        fs::write(pkg.join(MANIFEST), m.to_string()).unwrap();
        refresh(&name).unwrap();
        let rv = review("acme-routine", Some(&name)).unwrap();
        update("acme-routine", &rv.new_uses).unwrap().unwrap();
        assert!(!crate::jobs::on("acme-routine.mail-notes"), "added again: off until the user switches it on");
    }

    #[test]
    fn a_routine_is_due_every_n_minutes_from_its_last_run_off_and_at_night_never() {
        let (_env, root) = paths::testenv::folders("routines-due");
        test_install(&root.join("market"), "acme-routine", json!({"claude": {}}), json!({}), "Read the mail.");
        let id = "acme-routine.mail-notes";
        let now = chrono::Local::now();
        let set = |a: &[&str]| crate::jobs::set(id, &a.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
        assert!(due(now).is_empty(), "off until the user switches it on");
        set(&["on=on", "night=off"]);
        assert_eq!(due(now), vec![id.to_string()], "on, never run: now");
        crate::jobs::record(id, now - chrono::Duration::minutes(61), Ok("done".into()), json!({}));
        assert_eq!(due(now), vec![id.to_string()], "its 60 minutes have passed");
        let last = now - chrono::Duration::minutes(30);
        crate::jobs::record(id, last, Ok("done".into()), json!({}));
        assert!(due(now).is_empty());
        assert_eq!(crate::jobs::routine_next(id, now).timestamp(), (last + chrono::Duration::minutes(60)).timestamp());
        set(&["every=15"]);
        assert_eq!(due(now), vec![id.to_string()], "the user's every");
        // At night, when it pauses then: not before the night's end.
        let hm = |t: chrono::DateTime<chrono::Local>| t.format("%H:%M").to_string();
        crate::jobs::set("night", &[format!("from={}", hm(now - chrono::Duration::hours(1))), format!("to={}", hm(now + chrono::Duration::hours(1)))]).unwrap();
        assert_eq!(due(now), vec![id.to_string()], "night off for it");
        set(&["night=on"]);
        assert!(due(now).is_empty());
        assert_eq!(crate::jobs::routine_next(id, now), crate::jobs::after_night(now));
        set(&["night=off"]);
        // One running (its lock held, in any process): not again.
        fs::create_dir_all(runs_dir()).unwrap();
        let held = crate::app::FileLock::try_take(&lock_path(id)).unwrap().unwrap();
        assert!(running(id) && due(now).is_empty());
        drop(held);
        assert!(!running(id));
        // Its plugin off: not run, its settings kept.
        set_enabled("acme-routine", false).unwrap();
        assert!(due(now).is_empty());
        assert!(crate::jobs::on(id));
    }

    #[test]
    fn a_runs_summary_counts_its_changes_and_sends_in_echos_words() {
        let a = |k: &str| json!({"kind": k, "command": "project work acme add “Ask Pat about Acme's budget”"});
        assert_eq!(summary(&[]), "done");
        assert_eq!(summary(&[a("change")]), "done: 1 change through Echo");
        assert_eq!(summary(&[a("change"), a("draft"), a("send")]), "done: 2 changes and 1 send through Echo");
        assert_eq!(summary(&[a("send"), a("send")]), "done: 2 sends through Echo");
    }

    #[test]
    #[cfg(all(unix, feature = "tray", feature = "js"))]
    fn a_routine_runs_as_the_folder_session_on_the_default_profile_and_is_recorded_in_echos_words() {
        let (_env, root) = paths::testenv::folders("routines-run");
        let _fake = crate::ai::fake::install(&root);
        // The fake answers a run at once (Echo writes nothing on its stdin).
        let script = root.join("fake-claude");
        let text = fs::read_to_string(&script).unwrap();
        let turns = r#"while IFS= read -r l; do printf '%s\n' "$l" >> "$d/p.turns"; cat "$d/p.reply"; echo; done"#;
        assert!(text.contains(turns));
        fs::write(&script, text.replace(turns, r#"cat "$d/p.reply""#)).unwrap();
        let p = crate::ai::fake::profile("work", None, &crate::ai::fake::signed_in("pat@example.com"));
        crate::ai::Settings::update(|s| {
            s.default = Some("work".into());
            Ok(())
        })
        .unwrap();
        let answer = json!({"type": "result", "subtype": "success", "is_error": false, "result": "Kept 2 notes from Acme's mail", "total_cost_usd": 0.0125, "duration_ms": 5, "num_turns": 1,
                            "usage": {"input_tokens": 100, "output_tokens": 20, "cache_read_input_tokens": 7, "cache_creation_input_tokens": 3}});
        fs::write(crate::ai::fake::file(&p, "p.reply"), answer.to_string()).unwrap();
        let args = || fs::read_to_string(crate::ai::fake::file(&p, "p.args")).unwrap_or_default().split('\0').filter(|a| !a.is_empty()).map(str::to_owned).collect::<Vec<String>>();
        let after = |a: &[String], flag: &str| a.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone());
        test_install(&root.join("market"), "acme-routine", json!({"claude": {}}), json!({"tools": "skip"}), "ROUTINE: read the new mail.");
        let id = "acme-routine.mail-notes";
        // Off, the user runs it: the folder session, in the data folder, on the default profile; its own `skip` not on
        // its sheet, so the routines' setting (Auto); Echo's lines around the prompt its package ships.
        let rec = run(id).unwrap();
        assert_eq!((rec["state"].clone(), rec["summary"].clone(), rec["calls"].clone(), rec["input"].clone(), rec["output"].clone(), rec["cache_read"].clone(), rec["cache_write"].clone(), rec["cost_usd"].clone()),
                   (json!("done"), json!("done"), json!(1), json!(100), json!(20), json!(7), json!(3), json!(0.0125)));
        assert_eq!((rec["changes"].clone(), rec["sends"].clone(), rec["trial"].clone()), (json!(0), json!(0), json!(false)));
        let a = args();
        assert_eq!(fs::read_to_string(crate::ai::fake::file(&p, "p.cwd")).unwrap().trim(), dunce::canonicalize(paths::home()).unwrap().display().to_string());
        assert_eq!(after(&a, "--permission-mode").as_deref(), Some("auto"));
        assert!(!a.contains(&"--dangerously-skip-permissions".to_string()));
        let prompt = after(&a, "-p").unwrap();
        assert!(prompt.contains("This is the routine “Mail → notes” of the plugin “Acme routine”, run by Echo for the user (every 60 min while it is on). It hasn't run before."), "{prompt}");
        assert!(prompt.ends_with("ROUTINE: read the new mail."), "{prompt}");
        assert!(fs::read_to_string(crate::ai::fake::file(&p, "p.env")).unwrap().contains("ECHOCTL_RUN_SESSION=acts"));
        // The answer is the user's, in its outcome; the record a plugin reads keeps Echo's words.
        let o = outcome(id, None).unwrap();
        assert_eq!((o["answer"].clone(), o["permissions"]["asked"].clone(), o["run"].clone()), (json!("Kept 2 notes from Acme's mail"), json!("setting"), rec["run"].clone()));
        assert!(o["log"].as_str().unwrap().contains("Echo runs “Acme routine · Mail → notes”"));
        assert!(!serde_json::to_string(&crate::jobs::routine_runs(id, 10).unwrap()).unwrap().contains("Kept 2"));
        assert!(!serde_json::to_string(&crate::jobs::routines()).unwrap().contains("Kept 2"));
        // Again: its last run named; the user's model as the profile maps it.
        crate::ai::set_model_names("work", &["opus=acme-gw/opus-large".into()]).unwrap();
        crate::jobs::set(id, &["model=opus".into()]).unwrap();
        run(id).unwrap();
        let a = args();
        assert!(after(&a, "-p").unwrap().contains("Its last run started "));
        assert_eq!(after(&a, "--model").as_deref(), Some("acme-gw/opus-large"));
        // Routines on Skip all permission checks: so it runs.
        let mut c = crate::config::Config::load().unwrap_or_default();
        c.routine_permissions = Some(crate::config::Permissions::Skip);
        c.save().unwrap();
        run(id).unwrap();
        assert!(args().contains(&"--dangerously-skip-permissions".to_string()));
        // A failed run: recorded in Echo's words, its error returned.
        fs::write(crate::ai::fake::file(&p, "p.reply"), "").unwrap();
        assert!(run(id).is_err());
        let last = crate::jobs::routine_runs(id, 1).unwrap()[0].clone();
        assert_eq!((last["state"].clone(), last["error"].clone()), (json!("failed"), json!("the run failed: its log says why")));
        // The newest 20 run folders kept.
        let folder = runs_dir().join(id);
        for i in 0..25 {
            fs::create_dir_all(folder.join(format!("20000101-0000{i:02}"))).unwrap();
        }
        assert!(run(id).is_err());
        assert_eq!(fs::read_dir(&folder).unwrap().count(), KEEP_RUNS);
        assert!(!folder.join("20000101-000000").exists());
        // Failed before Claude Code started (its prompt gone from the installed copy): recorded, its log saying why.
        let installed = plugin_dir(&Registry::load().unwrap().installed["acme-routine"]);
        let prompt_file = installed.join("routines/mail-notes.md");
        let kept = fs::read(&prompt_file).unwrap();
        fs::remove_file(&prompt_file).unwrap();
        assert!(run(id).is_err());
        let o = outcome(id, None).unwrap();
        assert_eq!(o["record"]["error"], "the run failed: its log says why");
        assert!(o["log"].as_str().unwrap().contains("routines/mail-notes.md isn't in the plugin"), "{}", o["log"]);
        fs::write(&prompt_file, kept).unwrap();
        assert!(outcome(id, Some("20000101-000000")).is_err(), "a run no longer kept");
        // Its plugin off: refused, nothing run.
        set_enabled("acme-routine", false).unwrap();
        assert!(run(id).unwrap_err().to_string().contains("Acme routine is off: its routines don't run"));
        // A plugin whose sheet allows `skip`: its routine's own `skip` is taken; `none` keeps no tools.
        c.routine_permissions = None;
        c.save().unwrap();
        fs::write(crate::ai::fake::file(&p, "p.reply"), answer.to_string()).unwrap();
        test_install(&root.join("market2"), "acme-skip", json!({"claude": {"skip": true}}), json!({"tools": "skip"}), "Read.");
        run("acme-skip.mail-notes").unwrap();
        assert!(args().contains(&"--dangerously-skip-permissions".to_string()));
        test_install(&root.join("market3"), "acme-none", json!({"claude": {}}), json!({"tools": "none"}), "Read.");
        run("acme-none.mail-notes").unwrap();
        let a: Vec<String> = fs::read_to_string(crate::ai::fake::file(&p, "p.args")).unwrap().split('\0').map(str::to_owned).collect();
        assert!(a.windows(2).any(|w| w[0] == "--tools" && w[1].is_empty()) && !a.contains(&"--settings".to_string()), "{a:?}");
    }
}

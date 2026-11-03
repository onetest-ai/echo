//! Routines (the Scheduler plugin, `docs/plugins-design.md`): prompts the user writes, run by Claude
//! Code headless on a schedule, the answer delivered to the user (`deliver.rs`). Nothing here runs
//! unless a plugin that uses `routines` is installed and on.
//!
//! A routine is `<home>/.echo/routines/<id>.md`: a little frontmatter, then the prompt.
//!
//! ```text
//! ---
//! name: Monday asks
//! schedule: mon 09:00        # daily HH:MM · weekdays HH:MM · mon,thu HH:MM · every 2h · every 30m
//! profile: default           # an AI profile id (empty: the default)
//! model:                     # empty: the profile's minutes model
//! tools: read                # read · folder · skip
//! deliver: telegram, teams   # telegram · teams · mail — to the user
//! mail_to:                   # mail only: other addresses (empty: the user)
//! skip_empty: true           # nothing worth sending → nothing sent
//! enabled: true
//! ---
//! What do I owe this week …
//! ```
//!
//! Permissions are the user's pick, since the prompt is their own: `read` = the guard
//! (`RUN_MODE=routine`: read the data folder, read-only `echoctl` commands, the Brain), `folder` =
//! the Echo-folder session's own permissions (Claude Code's settings; a headless run can't ask, so a
//! tool needing a prompt is refused), `skip` = `--dangerously-skip-permissions`.
//!
//! The schedule is kept by slots: a routine runs when its next slot after the last one handled has
//! come — once, also when Echo was off or asleep through several (`state.json`). Each run:
//! `routines/runs/<id>/<stamp>/` (prompt, log, answer, deliveries), the last 20 kept.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone, Weekday};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{guard, notes, paths};

/// A run's longest time before it is stopped.
const TIMEOUT: Duration = Duration::from_secs(20 * 60);
/// Runs kept per routine.
const KEEP_RUNS: usize = 20;
/// The answer that means "nothing worth sending" (with `skip_empty`).
const NOTHING: &str = "NOTHING";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tools {
    Read,
    Folder,
    Skip,
}

impl Tools {
    fn parse(s: &str) -> Result<Tools> {
        match s.trim() {
            "" | "read" => Ok(Tools::Read),
            "folder" => Ok(Tools::Folder),
            "skip" => Ok(Tools::Skip),
            o => bail!("tools “{o}”: read, folder or skip"),
        }
    }

    fn word(self) -> &'static str {
        match self {
            Tools::Read => "read",
            Tools::Folder => "folder",
            Tools::Skip => "skip",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub schedule: String,
    #[serde(default)]
    pub profile: String,
    #[serde(default)]
    pub model: String,
    pub tools: Tools,
    pub deliver: Vec<String>,
    /// Mail only: other recipients (empty: the user).
    #[serde(default)]
    pub mail_to: Vec<String>,
    pub skip_empty: bool,
    pub enabled: bool,
    pub prompt: String,
}

// ── schedule ──

#[derive(Debug, Clone, PartialEq)]
pub enum Schedule {
    /// At a time on some days of the week (Mon = 0).
    At { days: [bool; 7], at: NaiveTime },
    /// Every so many minutes (≥ 15), from midnight.
    Every(u32),
}

const DAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

impl Schedule {
    pub fn parse(s: &str) -> Result<Schedule> {
        let s = s.trim().to_ascii_lowercase();
        let bad = || anyhow!("schedule “{s}”: daily 09:00 · weekdays 09:00 · mon,thu 09:00 · every 2h · every 30m");
        if let Some(n) = s.strip_prefix("every ") {
            let n = n.trim();
            let mins = if let Some(h) = n.strip_suffix('h') {
                h.trim().parse::<u32>().map_err(|_| bad())? * 60
            } else if let Some(m) = n.strip_suffix('m') {
                m.trim().parse::<u32>().map_err(|_| bad())?
            } else {
                return Err(bad());
            };
            if !(15..=7 * 24 * 60).contains(&mins) {
                bail!("a routine runs at most every 15 minutes");
            }
            return Ok(Schedule::Every(mins));
        }
        let (days, at) = s.rsplit_once(' ').ok_or_else(bad)?;
        let at = NaiveTime::parse_from_str(at.trim(), "%H:%M").map_err(|_| bad())?;
        let mut set = [false; 7];
        match days.trim() {
            "daily" | "every day" => set = [true; 7],
            "weekdays" => set[..5].copy_from_slice(&[true; 5]),
            "weekends" => set[5..].copy_from_slice(&[true; 2]),
            list => {
                for d in list.split([',', ' ']).filter(|d| !d.is_empty()) {
                    let i = DAYS.iter().position(|x| d.starts_with(x)).ok_or_else(bad)?;
                    set[i] = true;
                }
            }
        }
        if !set.contains(&true) {
            return Err(bad());
        }
        Ok(Schedule::At { days: set, at })
    }

    /// The first slot strictly after `t`.
    pub fn next_after(&self, t: DateTime<Local>) -> DateTime<Local> {
        match self {
            Schedule::Every(m) => {
                let midnight = Local.from_local_datetime(&t.date_naive().and_hms_opt(0, 0, 0).unwrap()).earliest().unwrap_or(t);
                let step = chrono::Duration::minutes(*m as i64);
                let mut s = midnight;
                while s <= t {
                    s += step;
                }
                s
            }
            Schedule::At { days, at } => {
                for i in 0..8 {
                    let d = t.date_naive() + chrono::Duration::days(i);
                    if !days[d.weekday().num_days_from_monday() as usize] {
                        continue;
                    }
                    // A time skipped by a clock change runs at the first moment that exists after it.
                    let Some(s) = Local.from_local_datetime(&d.and_time(*at)).earliest().or_else(|| Local.from_local_datetime(&(d.and_time(*at) + chrono::Duration::hours(1))).earliest()) else { continue };
                    if s > t {
                        return s;
                    }
                }
                t + chrono::Duration::days(7)
            }
        }
    }

    /// In words, for the window ("Mondays at 09:00").
    pub fn words(&self) -> String {
        match self {
            Schedule::Every(m) if m % 60 == 0 => format!("Every {} h", m / 60),
            Schedule::Every(m) => format!("Every {m} min"),
            Schedule::At { days, at } => {
                let t = at.format("%H:%M");
                let picked: Vec<&str> = DAYS.iter().zip(days).filter(|(_, on)| **on).map(|(d, _)| *d).collect();
                match picked.len() {
                    7 => format!("Every day at {t}"),
                    5 if days[..5] == [true; 5] => format!("Weekdays at {t}"),
                    _ => format!("{} at {t}", picked.iter().map(|d| { let w: Weekday = d.parse().unwrap_or(Weekday::Mon); format!("{w:?}") }).collect::<Vec<_>>().join(", ")),
                }
            }
        }
    }
}

// ── files ──

pub fn dir() -> PathBuf {
    paths::state_dir().join("routines")
}

fn file_of(id: &str) -> PathBuf {
    dir().join(format!("{id}.md"))
}

fn valid_id(id: &str) -> bool {
    (1..=48).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') && !id.starts_with('-')
}

/// Cyrillic letters in Latin, for ids ("План на день" → `plan-na-den`).
fn translit(c: char) -> Option<String> {
    const T: [(&str, &str); 37] = [
        ("а", "a"), ("б", "b"), ("в", "v"), ("г", "g"), ("ґ", "g"), ("д", "d"), ("е", "e"), ("ё", "e"), ("є", "ie"), ("ж", "zh"), ("з", "z"),
        ("и", "i"), ("і", "i"), ("ї", "i"), ("й", "i"), ("к", "k"), ("л", "l"), ("м", "m"), ("н", "n"), ("о", "o"), ("п", "p"), ("р", "r"),
        ("с", "s"), ("т", "t"), ("у", "u"), ("ф", "f"), ("х", "kh"), ("ц", "ts"), ("ч", "ch"), ("ш", "sh"), ("щ", "shch"), ("ъ", ""),
        ("ы", "y"), ("ь", ""), ("э", "e"), ("ю", "iu"), ("я", "ia"),
    ];
    T.iter().find(|(k, _)| k.starts_with(c)).map(|(_, v)| v.to_string())
}

/// An id from a name ("Monday asks" → `monday-asks`), unique in the folder.
fn new_id(name: &str) -> String {
    let base: String = name.to_lowercase().chars().map(|c| translit(c).unwrap_or(if c.is_ascii_alphanumeric() { c.to_string() } else { "-".into() })).collect::<String>().split('-').filter(|w| !w.is_empty()).collect::<Vec<_>>().join("-");
    let base: String = if base.is_empty() { "routine".into() } else { base.chars().take(40).collect::<String>().trim_end_matches('-').to_string() };
    let mut id = base.clone();
    let mut n = 2;
    while file_of(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

pub fn parse(id: &str, text: &str) -> Result<Routine> {
    let text = text.replace("\r\n", "\n");
    let rest = text.strip_prefix("---\n").ok_or_else(|| anyhow!("{id}.md doesn't start with its settings (---)"))?;
    let (head, prompt) = rest.split_once("\n---").ok_or_else(|| anyhow!("{id}.md's settings aren't closed (---)"))?;
    let prompt = prompt.strip_prefix('\n').unwrap_or(prompt).trim().to_string();
    let mut f: BTreeMap<String, String> = BTreeMap::new();
    for line in head.lines() {
        let line = line.split(" #").next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once(':').ok_or_else(|| anyhow!("{id}.md: “{line}” isn't key: value"))?;
        f.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
    }
    let get = |k: &str| f.get(k).cloned().unwrap_or_default();
    let flag = |k: &str, d: bool| match get(k).as_str() {
        "" => Ok(d),
        "true" | "yes" => Ok(true),
        "false" | "no" => Ok(false),
        o => Err(anyhow!("{k}: “{o}” isn't true or false")),
    };
    let deliver: Vec<String> = get("deliver").trim_matches(['[', ']']).split(',').map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
    if let Some(d) = deliver.iter().find(|d| !crate::deliver::KINDS.contains(&d.as_str())) {
        bail!("deliver “{d}”: telegram, teams or mail");
    }
    let mail_to: Vec<String> = get("mail_to").trim_matches(['[', ']']).split([',', ';']).map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
    if let Some(a) = mail_to.iter().find(|a| !crate::deliver::valid_address(a)) {
        bail!("mail_to “{a}” isn't a mail address");
    }
    if mail_to.len() > 20 {
        bail!("mail_to: at most 20 addresses");
    }
    let r = Routine {
        id: id.into(),
        name: Some(get("name")).filter(|n| !n.is_empty()).unwrap_or_else(|| id.into()),
        schedule: get("schedule"),
        profile: get("profile"),
        model: get("model"),
        tools: Tools::parse(&get("tools"))?,
        deliver,
        mail_to,
        skip_empty: flag("skip_empty", true)?,
        enabled: flag("enabled", true)?,
        prompt,
    };
    Schedule::parse(&r.schedule)?;
    if r.prompt.is_empty() {
        bail!("{id}.md has no prompt");
    }
    Ok(r)
}

pub fn render(r: &Routine) -> String {
    let one = |s: &str| s.replace(['\n', '\r'], " ");
    format!(
        "---\nname: {}\nschedule: {}\nprofile: {}\nmodel: {}\ntools: {}\ndeliver: {}\nmail_to: {}\nskip_empty: {}\nenabled: {}\n---\n{}\n",
        one(&r.name),
        one(&r.schedule),
        one(&r.profile),
        one(&r.model),
        r.tools.word(),
        r.deliver.join(", "),
        one(&r.mail_to.join(", ")),
        r.skip_empty,
        r.enabled,
        r.prompt.trim()
    )
}

/// Every routine (one that can't be read comes back as its error, by id).
pub fn list() -> Vec<Result<Routine, (String, String)>> {
    let mut v = vec![];
    let Ok(rd) = fs::read_dir(dir()) else { return v };
    let mut names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "md")).collect();
    names.sort();
    for p in names {
        let id = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if !valid_id(&id) {
            continue;
        }
        v.push(fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| parse(&id, &t).map_err(|e| format!("{e:#}"))).map_err(|e| (id, e)));
    }
    v
}

pub fn get(id: &str) -> Result<Routine> {
    if !valid_id(id) {
        bail!("no routine “{id}”");
    }
    let t = fs::read_to_string(file_of(id)).map_err(|_| anyhow!("no routine “{id}”"))?;
    parse(id, &t)
}

/// Save a routine (new when `id` is empty): checked as it would be read back.
pub fn save(mut r: Routine) -> Result<Routine> {
    fs::create_dir_all(dir())?;
    if r.name.trim().is_empty() {
        bail!("a routine needs a name");
    }
    if r.id.is_empty() {
        r.id = new_id(&r.name);
    } else if !valid_id(&r.id) {
        bail!("no routine “{}”", r.id);
    }
    if !r.profile.is_empty() {
        crate::ai::Settings::load()?.get(&r.profile).with_context(|| format!("profile “{}”", r.profile))?;
    }
    let text = render(&r);
    let back = parse(&r.id, &text)?;
    paths::write_atomic(&file_of(&r.id), text.as_bytes())?;
    // A new or rescheduled routine starts counting from now: no run for slots already gone.
    with_state(|s| {
        let e = s.entry(back.id.clone()).or_default();
        if e.schedule != back.schedule {
            e.schedule = back.schedule.clone();
            e.slot = Some(Local::now().to_rfc3339());
        }
    });
    Ok(back)
}

pub fn remove(id: &str) -> Result<()> {
    fs::remove_file(file_of(id)).map_err(|_| anyhow!("no routine “{id}”"))?;
    let _ = fs::remove_dir_all(dir().join("runs").join(id));
    with_state(|s| {
        s.remove(id);
    });
    Ok(())
}

// ── state ──

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State {
    /// The schedule the slots count for (a change starts counting again).
    #[serde(default)]
    pub schedule: String,
    /// The last slot handled (run, or skipped as already gone).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<Value>,
}

static STATE_LOCK: Mutex<()> = Mutex::new(());

fn state_file() -> PathBuf {
    dir().join("state.json")
}

pub fn states() -> BTreeMap<String, State> {
    fs::read(state_file()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn with_state(f: impl FnOnce(&mut BTreeMap<String, State>)) {
    let _g = STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = states();
    f(&mut s);
    let _ = fs::create_dir_all(dir());
    let _ = paths::write_atomic(&state_file(), &serde_json::to_vec_pretty(&s).unwrap_or_default());
}

/// The slot a routine is due for now, if any: its next slot after the last one handled has come.
fn due_slot(r: &Routine, st: Option<&State>, now: DateTime<Local>) -> Option<DateTime<Local>> {
    let sched = Schedule::parse(&r.schedule).ok()?;
    let last = st.and_then(|s| s.slot.as_deref()).and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Local))?;
    let next = sched.next_after(last);
    (next <= now).then_some(next)
}

/// When it runs next (for the window).
pub fn next_run(r: &Routine) -> Option<DateTime<Local>> {
    let sched = Schedule::parse(&r.schedule).ok()?;
    let st = states().remove(&r.id);
    let last = st.and_then(|s| s.slot).and_then(|t| DateTime::parse_from_rfc3339(&t).ok()).map(|t| t.with_timezone(&Local)).unwrap_or_else(Local::now);
    Some(sched.next_after(last).max(Local::now()))
}

// ── running ──

/// Routines running now (in this process).
static RUNNING: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn running() -> Vec<String> {
    RUNNING.lock().map(|r| r.clone()).unwrap_or_default()
}

/// Is a plugin that uses routines installed and on?
pub fn active() -> bool {
    crate::plugins::enabled_with("routines")
}

fn fwd(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// What the run is told around the user's prompt.
fn prompt_for(r: &Routine, cli: &str) -> String {
    let now = Local::now();
    let tools = match r.tools {
        Tools::Read => format!(
            "You may read Echo's data folder (its calls' transcripts, minutes and action items, threads, people), search the Brain \
             (the echo-brain tools) when it is there, and run these read-only Echo commands as {cli}, one per Bash call, plain words \
             only: {}. You change nothing.",
            guard::ROUTINE_COMMANDS.iter().map(|c| format!("`echoctl {}`", c.replace('*', "<id>"))).collect::<Vec<_>>().join(", ")
        ),
        _ => format!("Echo's CLI is {cli}; the Brain (echo-brain tools) is there when it is on."),
    };
    let empty = if r.skip_empty { format!(" If there is nothing worth sending this time, answer with exactly {NOTHING} and nothing else.") } else { String::new() };
    format!(
        "This is “{}”, a routine the user wrote, run by Echo on its schedule ({}). Nobody is watching: don't ask anything, do the work \
         and answer. Your final answer is sent to the user as a message (Markdown, short, the useful part first).{empty} Now: {}. \
         The working folder is Echo's data folder. {tools}\n\n{}",
        r.name,
        Schedule::parse(&r.schedule).map(|s| s.words()).unwrap_or_default(),
        now.format("%A %Y-%m-%d %H:%M (UTC%:z, the user's local time)"),
        r.prompt
    )
}

/// Run a routine now: Claude Code on its profile, then its deliveries (`deliver: false`: none, the
/// answer only). The run's record comes back (also saved as the routine's last).
pub fn run(id: &str, deliver: bool) -> Result<Value> {
    let r = get(id)?;
    {
        let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if running.contains(&r.id) {
            bail!("“{}” is running already", r.name);
        }
        running.push(r.id.clone());
    }
    struct Done(String);
    impl Drop for Done {
        fn drop(&mut self) {
            if let Ok(mut r) = RUNNING.lock() {
                r.retain(|x| x != &self.0);
            }
        }
    }
    let _done = Done(r.id.clone());
    let started = Local::now();
    let stamp = started.format("%Y%m%d-%H%M%S").to_string();
    let run_dir = dir().join("runs").join(&r.id).join(&stamp);
    fs::create_dir_all(&run_dir)?;
    let mut log = fs::File::create(run_dir.join("log.txt"))?;
    let outcome = claude_run(&r, &run_dir, &mut log);
    let mut rec = json!({"at": started.to_rfc3339(), "finished_at": Local::now().to_rfc3339(), "run": stamp});
    match outcome {
        Err(e) => {
            let signin = e.downcast_ref::<crate::ai::SignInNeeded>().map(|s| s.profile.clone());
            rec["state"] = "failed".into();
            rec["message"] = format!("{e:#}").into();
            rec["needs_sign_in"] = json!(signin);
        }
        Ok((answer, model)) => {
            let _ = fs::write(run_dir.join("answer.md"), &answer);
            let empty = answer.trim().is_empty() || (r.skip_empty && answer.trim().trim_matches(['*', '`', '.']).eq_ignore_ascii_case(NOTHING));
            if empty {
                rec["state"] = "empty".into();
                rec["message"] = "Nothing worth sending this time".into();
            } else if !deliver || r.deliver.is_empty() {
                rec["state"] = "done".into();
                rec["message"] = answer.chars().take(300).collect::<String>().into();
            } else {
                let about = format!("Routine · {}", Schedule::parse(&r.schedule).map(|s| s.words()).unwrap_or_default());
                let mut sent = vec![];
                let mut failed = vec![];
                for k in &r.deliver {
                    match crate::deliver::ready(k).and_then(|_| crate::deliver::send(k, &crate::deliver::Msg { title: &r.name, markdown: &answer, about: Some(&about), ai: Some(&model), mail_to: &r.mail_to })) {
                        Ok(()) => sent.push(k.clone()),
                        Err(e) => failed.push(format!("{k}: {e:#}")),
                    }
                }
                let _ = writeln!(log, "{}  sent: {} {}", Local::now().format("%H:%M:%S"), sent.join(", "), failed.join("; "));
                rec["sent"] = json!(sent);
                rec["state"] = if failed.is_empty() { "done".into() } else { "failed".into() };
                rec["message"] = if failed.is_empty() { answer.chars().take(300).collect::<String>().into() } else { format!("Not sent — {}", failed.join("; ")).into() };
            }
        }
    }
    let _ = fs::write(run_dir.join("result.json"), serde_json::to_vec_pretty(&rec).unwrap_or_default());
    with_state(|s| {
        s.entry(r.id.clone()).or_default().last = Some(rec.clone());
    });
    prune_runs(&r.id);
    #[cfg(feature = "tray")]
    crate::attention::stale();
    Ok(rec)
}

fn prune_runs(id: &str) {
    let Ok(rd) = fs::read_dir(dir().join("runs").join(id)) else { return };
    let mut runs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    runs.sort();
    while runs.len() > KEEP_RUNS {
        let _ = fs::remove_dir_all(runs.remove(0));
    }
}

/// The answer, and the model that wrote it (as named to Claude Code; "" = its own default).
fn claude_run(r: &Routine, run_dir: &Path, log: &mut fs::File) -> Result<(String, String)> {
    let profile_id = if r.profile.is_empty() { crate::ai::default_id() } else { r.profile.clone() };
    let ready = crate::ai::ready(&profile_id)?;
    let profile = ready.profile();
    let model = Some(r.model.clone()).filter(|m| !m.is_empty()).or_else(|| profile.notes_model());
    let plugin = notes::plugin_dir().ok_or_else(|| anyhow!("the Echo plugin wasn't found"))?;
    let home = dunce::canonicalize(paths::home())?;
    let cwd = if profile.kind == crate::ai::Kind::System { crate::ai::system_folder()? } else { home.clone() };
    let clis = notes::run_clis(&plugin);
    // The launcher by its full path, told which Echo to run (`ECHOCTL_BIN`, below): this one — `echoctl` on
    // PATH may be another version.
    let launcher = clis.iter().find(|c| c.contains('/') && c.contains("scripts")).cloned().unwrap_or_else(|| "echoctl".into());
    let cli = format!("`{launcher}` (its full path, exactly so)");
    let prompt = prompt_for(r, &cli);
    let _ = fs::write(run_dir.join("prompt.md"), &prompt);
    let _ = writeln!(log, "{}  Echo runs “{}” on {}{} ({} permissions)", Local::now().format("%H:%M:%S"), r.name, profile.name, model.as_deref().map(|m| format!(", model {m}")).unwrap_or_default(), r.tools.word());
    let mut a: Vec<String> = vec!["-p".into(), prompt, "--output-format".into(), "stream-json".into(), "--verbose".into()];
    if let Some(m) = &model {
        a.extend(["--model".into(), m.clone()]);
    }
    let exe = std::env::current_exe().context("Echo's own program can't be found")?;
    // The Brain, narrowed to this profile's sources (strict routing).
    #[cfg(all(feature = "tray", feature = "brain"))]
    let brain = crate::terminal::brain_mcp_config(&exe, &["--profile".into(), profile.id.clone()]);
    #[cfg(not(all(feature = "tray", feature = "brain")))]
    let brain: Option<String> = None;
    let mut settings_file = None;
    match r.tools {
        Tools::Read => {
            crate::ai::hooks_conflict(profile, &cwd).map_err(|w| anyhow!(w))?;
            let folder = paths::config_dir().join("minutes-run");
            fs::create_dir_all(&folder)?;
            let f = folder.join(format!("routine-{}-{}.json", r.id, std::process::id()));
            fs::write(&f, serde_json::to_vec_pretty(&guard::settings(&exe, &fwd(&paths::calls_dir())))?)?;
            a.extend(["--settings".into(), f.display().to_string(), "--tools".into(), "Bash,Read,Glob,Grep".into(), "--strict-mcp-config".into()]);
            if let Some(b) = &brain {
                a.extend(["--mcp-config".into(), b.clone()]);
            }
            a.extend(["--permission-mode".into(), "dontAsk".into(), "--allowedTools".into()]);
            let abs = format!("/{}", notes::rule_path(&home));
            a.extend(["Read", "Glob", "Grep"].map(|t| format!("{t}({abs}/**)")));
            a.extend(clis.iter().flat_map(|cli| guard::ROUTINE_COMMANDS.iter().map(move |c| format!("Bash({cli} {}:*)", c.split(" *").next().unwrap_or(c)))));
            a.push("mcp__echo-brain".into());
            settings_file = Some(f);
        }
        Tools::Folder | Tools::Skip => {
            if let Some(b) = &brain {
                a.extend(["--mcp-config".into(), b.clone(), "--allowedTools".into(), "mcp__echo-brain".into()]);
            }
            if r.tools == Tools::Skip {
                a.push("--dangerously-skip-permissions".into());
            } else {
                a.extend(["--permission-mode".into(), "dontAsk".into()]);
            }
        }
    }
    let child = notes::admit_child(|| {
        let mut c = crate::ai::prepare_claude(profile, &a, &cwd, false)?;
        c.env("ECHOCTL_BIN", &exe);
        if r.tools == Tools::Read {
            c.env(guard::RUN_MODE, "routine").env(guard::RUN_HOME, &home).env(guard::RUN_CLIS, clis.join("\n"));
        }
        Ok(c.spawn()?)
    });
    let out = child.and_then(|c| crate::projectsync::finish_run(c, log, TIMEOUT, "run a routine", "Echo quit during the routine: it runs again next time"));
    if let Some(f) = settings_file {
        let _ = fs::remove_file(f);
    }
    out.map(|a| (a, model.unwrap_or_default()))
}

/// The app's scheduler: every 30 s, each routine whose slot has come runs (one at a time each, in
/// its own thread); slots that went by while Echo was off run once.
pub fn start_scheduler() {
    std::thread::spawn(|| loop {
        std::thread::sleep(Duration::from_secs(30));
        if notes::quitting() || !active() {
            continue;
        }
        tick(Local::now());
    });
}

fn tick(now: DateTime<Local>) {
    let st = states();
    for r in list().into_iter().flatten().filter(|r| r.enabled) {
        // First seen: counting starts now.
        if st.get(&r.id).and_then(|s| s.slot.as_ref()).is_none() || st.get(&r.id).is_some_and(|s| s.schedule != r.schedule) {
            with_state(|s| {
                let e = s.entry(r.id.clone()).or_default();
                e.schedule = r.schedule.clone();
                e.slot = Some(now.to_rfc3339());
            });
            continue;
        }
        let Some(slot) = due_slot(&r, st.get(&r.id), now) else { continue };
        if running().contains(&r.id) {
            continue;
        }
        // The latest slot that has come (several missed → one run).
        let sched = Schedule::parse(&r.schedule).ok();
        let mut last = slot;
        while let Some(n) = sched.as_ref().map(|s| s.next_after(last)).filter(|n| *n <= now) {
            last = n;
        }
        with_state(|s| s.entry(r.id.clone()).or_default().slot = Some(last.to_rfc3339()));
        let id = r.id.clone();
        std::thread::spawn(move || match run(&id, true) {
            Ok(rec) if rec["state"] == "failed" => crate::app::notify("Routine failed", rec["message"].as_str().unwrap_or("")),
            Ok(_) => {}
            Err(e) => crate::app::notify("Routine failed", &format!("{e:#}")),
        });
    }
}

// ── for the window and the CLI ──

pub fn view() -> Value {
    let st = states();
    let running = running();
    let routines: Vec<Value> = list()
        .into_iter()
        .map(|x| match x {
            Ok(r) => {
                let mut v = serde_json::to_value(&r).unwrap_or_default();
                v["when"] = Schedule::parse(&r.schedule).map(|s| s.words()).unwrap_or_default().into();
                v["next"] = json!(r.enabled.then(|| next_run(&r)).flatten().map(|t| t.to_rfc3339()));
                v["last"] = st.get(&r.id).and_then(|s| s.last.clone()).unwrap_or(Value::Null);
                v["running"] = running.contains(&r.id).into();
                v
            }
            Err((id, e)) => json!({"id": id, "error": e}),
        })
        .collect();
    let profiles: Vec<Value> = crate::ai::Settings::load().map(|s| s.all().into_iter().map(|p| json!({"id": p.id, "name": p.name})).collect()).unwrap_or_default();
    json!({"active": active(), "routines": routines, "deliver": crate::deliver::status(), "profiles": profiles, "default_profile": crate::ai::default_id()})
}

/// A routine from the window's editor (`routine_op save`).
pub fn from_value(v: &Value) -> Result<Routine> {
    let s = |k: &str| v[k].as_str().unwrap_or("").trim().to_string();
    Ok(Routine {
        id: s("id"),
        name: s("name"),
        schedule: s("schedule"),
        profile: s("profile"),
        model: s("model"),
        tools: Tools::parse(&s("tools"))?,
        deliver: v["deliver"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect()).unwrap_or_default(),
        mail_to: match &v["mail_to"] {
            Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect(),
            Value::String(t) => t.split([',', ';', ' ', '\n']).map(str::trim).filter(|x| !x.is_empty()).map(str::to_owned).collect(),
            _ => vec![],
        },
        skip_empty: v["skip_empty"].as_bool().unwrap_or(true),
        enabled: v["enabled"].as_bool().unwrap_or(true),
        prompt: v["prompt"].as_str().unwrap_or("").to_string(),
    })
}

/// The window's operations (`routine_op`): save, remove, enable, run (in the background).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn page_op(v: &Value) -> Result<()> {
    let id = v["id"].as_str().unwrap_or("").to_string();
    match v["op"].as_str().unwrap_or("") {
        "save" => save(from_value(&v["routine"])?).map(|_| ()),
        "remove" => remove(&id),
        "enable" => {
            let mut r = get(&id)?;
            r.enabled = v["on"].as_bool().unwrap_or(true);
            save(r).map(|_| ())
        }
        "run" => {
            get(&id)?;
            if !active() {
                bail!("install and switch on the Scheduler plugin first");
            }
            std::thread::spawn(move || {
                if let Err(e) = run(&id, true) {
                    crate::app::notify("Routine failed", &format!("{e:#}"));
                }
            });
            Ok(())
        }
        o => bail!("unknown routine operation “{o}”"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Local> {
        Local.from_local_datetime(&chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).earliest().unwrap()
    }

    #[test]
    fn schedules_parse_and_step() {
        // 2026-11-02 is a Monday.
        let s = Schedule::parse("mon,thu 09:00").unwrap();
        assert_eq!(s.next_after(at("2026-11-02 08:00")), at("2026-11-02 09:00"));
        assert_eq!(s.next_after(at("2026-11-02 09:00")), at("2026-11-05 09:00"));
        assert_eq!(s.words(), "Mon, Thu at 09:00");
        let w = Schedule::parse("weekdays 18:30").unwrap();
        assert_eq!(w.next_after(at("2026-11-06 19:00")), at("2026-11-09 18:30"), "Friday evening → Monday");
        assert_eq!(w.words(), "Weekdays at 18:30");
        let e = Schedule::parse("every 2h").unwrap();
        assert_eq!(e.next_after(at("2026-11-02 09:10")), at("2026-11-02 10:00"));
        assert_eq!(e.words(), "Every 2 h");
        assert!(Schedule::parse("every 5m").is_err());
        assert!(Schedule::parse("sometimes").is_err());
        assert!(Schedule::parse("daily 25:00").is_err());
    }

    #[test]
    fn a_routine_round_trips_and_is_checked() {
        let (_env, _) = crate::paths::testenv::folders("routines");
        let r = Routine { id: String::new(), name: "Monday asks".into(), schedule: "mon 09:00".into(), profile: String::new(), model: String::new(), tools: Tools::Read, deliver: vec!["telegram".into(), "mail".into()], mail_to: vec![], skip_empty: true, enabled: true, prompt: "What do I owe?\n\n---\nNot settings.".into() };
        let saved = save(r).unwrap();
        assert_eq!(saved.id, "monday-asks");
        assert_eq!(get("monday-asks").unwrap(), saved);
        assert!(saved.prompt.contains("Not settings"));
        assert!(parse("x", "---\nname: x\nschedule: daily 09:00\ndeliver: slack\n---\nhi").is_err());
        assert!(parse("x", "---\nname: x\nschedule: daily 09:00\n---\n").is_err(), "no prompt");
        assert!(parse("x", "---\nname: x\nschedule: daily 09:00\ntools: root\n---\nhi").is_err());
        assert!(parse("x", "---\nname: x\nschedule: daily 09:00\nmail_to: boss@acme.example, not-an-address\n---\nhi").is_err());
        assert_eq!(parse("x", "---\nname: x\nschedule: daily 09:00\nmail_to: boss@acme.example; pm@acme.example\n---\nhi").unwrap().mail_to.len(), 2);
        let again = save(Routine { id: String::new(), ..saved.clone() }).unwrap();
        assert_eq!(again.id, "monday-asks-2");
        assert_eq!(new_id("План на день"), "plan-na-den");
        // Due: not before its first slot after it was saved; once when several slots went by.
        let st = State { schedule: saved.schedule.clone(), slot: Some(at("2026-11-02 10:00").to_rfc3339()), last: None };
        assert!(due_slot(&saved, Some(&st), at("2026-11-08 12:00")).is_none());
        assert_eq!(due_slot(&saved, Some(&st), at("2026-11-23 12:00")), Some(at("2026-11-09 09:00")));
        remove("monday-asks").unwrap();
        assert!(get("monday-asks").is_err());
        assert!(get("../etc").is_err());
    }
}

//! The user's OneDrive on this computer (Settings → Calendar & chat): the folder its sync app keeps, so Echo and Claude
//! know a path there is the cloud's too — a folder made here by hand while OneDrive holds one of the same name online,
//! not synced to this computer, never reaches the cloud (#469). `echoctl m365 drive where <path>` says where a path is.

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The setting as stored: `None` finds it (`detect`), `"none"` turns it off, anything else is the folder.
pub fn setting(value: &str) -> Result<Option<String>> {
    match value.trim() {
        "" | "auto" | "default" => Ok(None),
        "none" | "off" => Ok(Some("none".into())),
        p => {
            let d = dunce::canonicalize(p).map_err(|e| anyhow::anyhow!("{p}: {e}"))?;
            if !d.is_dir() {
                bail!("{} isn't a folder", d.display());
            }
            Ok(Some(d.to_string_lossy().into_owned()))
        }
    }
}

/// The folder OneDrive keeps on this computer: as set, else found — macOS: the one `OneDrive-<organisation>` in
/// `~/Library/CloudStorage` (not its shared libraries); Windows: `OneDriveCommercial`, else `OneDrive`.
pub fn root() -> Option<PathBuf> {
    match crate::config::Config::load().ok().and_then(|c| c.onedrive) {
        Some(s) if s == "none" => None,
        Some(s) => Some(PathBuf::from(s)),
        None => detect(&crate::paths::user_home(), |k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from)),
    }
}

pub fn detect(home: &Path, var: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if cfg!(windows) {
        // The work OneDrive only: `OneDrive` alone may be a personal one, not the account Echo signs in to.
        return var("OneDriveCommercial").filter(|p| p.is_dir());
    }
    let found: Vec<PathBuf> = std::fs::read_dir(home.join("Library/CloudStorage"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            // An organisation's own: not its shared libraries, not a personal OneDrive beside it.
            n.starts_with("OneDrive-") && !n.starts_with("OneDrive-SharedLibraries") && n != "OneDrive-Personal" && e.path().is_dir()
        })
        .map(|e| e.path())
        .collect();
    // Two accounts' OneDrives: which is Echo's sign-in can't be told by the name — the user sets it.
    (found.len() == 1).then(|| found[0].clone())
}

/// A path's real form as far as it exists (its links and case as the disk has them), the rest as written.
fn real(p: &Path) -> PathBuf {
    let mut existing = p;
    let mut rest = vec![];
    while !existing.exists() {
        match (existing.file_name(), existing.parent()) {
            (Some(n), Some(up)) => {
                rest.push(n.to_os_string());
                existing = up;
            }
            _ => return p.to_path_buf(),
        }
    }
    let mut r = dunce::canonicalize(existing).unwrap_or_else(|_| existing.to_path_buf());
    for part in rest.iter().rev() {
        r.push(part);
    }
    r
}

/// `path` relative to OneDrive's folder (`/`-separated), when it is inside it.
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let root = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let rel = real(path).strip_prefix(&root).ok()?.to_path_buf();
    Some(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
}

/// How many names of a difference are shown: a folder may hold thousands; the counts say how many there are.
const SHOWN: usize = 20;

/// A folder's names here and online (one level), compared by Echo (not a model): how many only here (not uploaded),
/// only online (not synced to this computer) and in both, with the first few names of each difference. Names
/// compared as OneDrive does, ignoring case.
pub fn compare(here: &[String], online: &[String]) -> Value {
    let low = |v: &[String]| v.iter().map(|n| n.to_lowercase()).collect::<std::collections::BTreeSet<_>>();
    let (h, o) = (low(here), low(online));
    let mut only_here: Vec<&String> = here.iter().filter(|n| !o.contains(&n.to_lowercase())).collect();
    let mut only_online: Vec<&String> = online.iter().filter(|n| !h.contains(&n.to_lowercase())).collect();
    only_here.sort();
    only_online.sort();
    json!({"only_here": only_here.len(), "only_online": only_online.len(), "both": h.intersection(&o).count(),
           "only_here_first": only_here.iter().take(SHOWN).collect::<Vec<_>>(), "only_online_first": only_online.iter().take(SHOWN).collect::<Vec<_>>()})
}

/// What to do about a path in OneDrive, from whether it is on this computer and online (`None`: not known).
pub fn advice(local: bool, online: Option<bool>) -> &'static str {
    match (local, online) {
        (false, Some(true)) => "It is in OneDrive online but not on this computer: OneDrive doesn't sync it here. Don't create it here (a folder of the same name made here stays on this computer and conflicts online): ask the user to choose it in OneDrive → Settings → Account → Choose folders, then work with it here (faster); until then only online (echoctl m365 drive).",
        (true, Some(false)) => "It is on this computer but not in OneDrive online: what is in it isn't uploaded. Tell the user (OneDrive's sync issues may say why).",
        (false, Some(false)) => "Neither here nor online: creating it here is fine — OneDrive uploads it.",
        (true, Some(true)) => "Here and online: OneDrive syncs it — work with it here, on this computer (faster than online).",
        (_, None) => "Whether it is online isn't known (Microsoft 365 isn't signed in, or didn't answer): don't create a folder here before the user says it isn't already in OneDrive.",
    }
}

/// Whether a path (relative to OneDrive's folder) is in OneDrive online. Development builds and tests:
/// `ECHOCTL_TEST_ONEDRIVE_ONLINE` (paths online, one per line) answers instead of Microsoft 365 (never in a release).
fn online_exists(rel: &str) -> Result<bool> {
    #[cfg(debug_assertions)]
    if let Ok(list) = std::env::var("ECHOCTL_TEST_ONEDRIVE_ONLINE") {
        return Ok(list.lines().map(|l| l.trim().trim_matches('/')).any(|l| l.eq_ignore_ascii_case(rel) || l.to_lowercase().starts_with(&format!("{}/", rel.to_lowercase()))));
    }
    match crate::graph::request("GET", &format!("/me/drive/root:/{}?$select=id,name", crate::m365::enc_path(rel)), crate::graph::Body::None, None, &[]) {
        Ok(_) => Ok(true),
        Err(e) if format!("{e:#}").contains("404") || format!("{e:#}").contains("itemNotFound") => Ok(false),
        Err(e) => Err(e),
    }
}

/// `echoctl m365 drive where <path>`: whether `path` is in the user's OneDrive, and if so its path online, whether it
/// is here and online, and what to do. Reads only.
pub fn where_is(path: &Path) -> Result<Value> {
    let path = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };
    let Some(root) = root() else {
        return Ok(json!({"path": path, "in_onedrive": false, "why": "no OneDrive folder on this computer is known (Settings → Calendar & chat)"}));
    };
    let Some(rel) = relative(&root, &path) else {
        return Ok(json!({"path": path, "in_onedrive": false, "onedrive": root}));
    };
    let local = path.exists();
    // Not here: what counts is the first of its folders missing here — one OneDrive has online but doesn't sync
    // here makes everything under it "online only", whether the path itself is there yet or not.
    let missing = if local { None } else { missing_folder(&root, &path, true) };
    let ancestor_online = match &missing {
        Some((_, up)) if *up != rel => online_exists(up).ok(),
        _ => None,
    };
    let online: Result<bool> = if rel.is_empty() || ancestor_online == Some(true) { Ok(true) } else { online_exists(&rel) };
    let (online, why) = match online {
        Ok(o) => (Some(o), None),
        Err(e) => (None, Some(format!("{e:#}"))),
    };
    let mut out = json!({"path": path, "in_onedrive": true, "onedrive": root, "online_path": rel, "here": local, "online": online, "why": why, "advice": advice(local, online)});
    if let (Some((folder, up)), Some(true)) = (&missing, ancestor_online) {
        out["not_synced_here"] = json!({"folder": folder, "online_path": up});
    }
    // A folder in both places: what is in it here and online, compared (one level).
    if local && path.is_dir() && online == Some(true) {
        let here: Vec<String> = std::fs::read_dir(&path).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| !n.starts_with('.')).collect();
        match online_names(&rel) {
            Ok((o, cut)) => {
                out["contents"] = compare(&here, &o);
                if cut {
                    out["contents"]["online_cut"] = json!(format!("only the first {MAX_ONLINE} names online were read"));
                }
            }
            Err(e) => out["contents_why"] = json!(format!("{e:#}")),
        }
    }
    Ok(out)
}

const MAX_ONLINE: usize = 5000;

/// The names in a OneDrive folder online (page by page, `MAX_ONLINE` at most), and whether it stopped there.
fn online_names(rel: &str) -> Result<(Vec<String>, bool)> {
    let mut url = if rel.is_empty() { "/me/drive/root/children?$top=999&$select=name".to_string() } else { format!("/me/drive/root:/{}:/children?$top=999&$select=name", crate::m365::enc_path(rel)) };
    let mut names = vec![];
    loop {
        let v = crate::graph::request("GET", &url, crate::graph::Body::None, None, &[])?;
        names.extend(v["value"].as_array().into_iter().flatten().filter_map(|i| i["name"].as_str().map(str::to_owned)));
        match v["@odata.nextLink"].as_str() {
            Some(next) if names.len() < MAX_ONLINE => url = next.to_string(),
            Some(_) => {
                names.truncate(MAX_ONLINE);
                return Ok((names, true));
            }
            None => return Ok((names, false)),
        }
    }
}

/// The paths a tool call is about to write, each with whether it is a folder: a file Write/Edit names; in a plain Bash
/// command, every name `mkdir` (folders) or `touch` takes, and `cp`/`mv`/`ditto`'s last — sub-command by sub-command.
/// Anything else (variables, substitutions) is left alone.
pub fn targets(call: &Value) -> Vec<(PathBuf, bool)> {
    let input = &call["tool_input"];
    match call["tool_name"].as_str().unwrap_or("") {
        "Write" | "Edit" | "MultiEdit" | "NotebookEdit" => input["file_path"].as_str().or(input["notebook_path"].as_str()).map(|p| (PathBuf::from(p), false)).into_iter().collect(),
        "Bash" => {
            let mut out = vec![];
            for words in commands(input["command"].as_str().unwrap_or("")) {
                let args: Vec<&String> = words.iter().skip(1).filter(|w| !w.starts_with('-')).collect();
                match words.first().map(String::as_str) {
                    Some("mkdir") => out.extend(args.iter().map(|a| (PathBuf::from(a.as_str()), true))),
                    Some("touch") => out.extend(args.iter().map(|a| (PathBuf::from(a.as_str()), false))),
                    Some("cp" | "mv" | "ditto") if args.len() >= 2 => out.push((PathBuf::from(args[args.len() - 1].as_str()), false)),
                    _ => {}
                }
            }
            out
        }
        _ => vec![],
    }
}

/// A command line's sub-commands, each as its words: spaces split words, `'…'` and `"…"` quote (a `;`, `&` or `|`
/// inside quotes stays in the word), `\` escapes the next character (in double quotes only `$`, `` ` ``, `"`, `\` or a
/// line break, as Bash does), an unquoted `;`, `&`, `|` or line break ends a
/// sub-command, and a word starting with `~/` is under the home folder.
fn commands(s: &str) -> Vec<Vec<String>> {
    let (mut all, mut words, mut w) = (vec![], vec![], String::new());
    let (mut q, mut started) = (None::<char>, false);
    let mut chars = s.chars();
    let end_word = |w: &mut String, words: &mut Vec<String>, started: &mut bool| {
        if *started {
            let word = std::mem::take(w);
            words.push(word.strip_prefix("~/").map(|r| crate::paths::user_home().join(r).to_string_lossy().into_owned()).unwrap_or(word));
            *started = false;
        }
    };
    while let Some(c) = chars.next() {
        match (q, c) {
            (None, ' ' | '\t') => end_word(&mut w, &mut words, &mut started),
            (None, ';' | '&' | '|' | '\n') => {
                end_word(&mut w, &mut words, &mut started);
                if !words.is_empty() {
                    all.push(std::mem::take(&mut words));
                }
            }
            (None, '\'' | '"') => {
                q = Some(c);
                started = true;
            }
            (None, '\\') => {
                if let Some(n) = chars.next() {
                    w.push(n);
                    started = true;
                }
            }
            // In double quotes it escapes only these; before anything else it is itself (a Windows path's).
            (Some('"'), '\\') => {
                match chars.clone().next() {
                    Some(n @ ('$' | '`' | '"' | '\\' | '\n')) => {
                        chars.next();
                        w.push(n);
                    }
                    _ => w.push('\\'),
                }
                started = true;
            }
            (Some(x), c) if c == x => q = None,
            _ => {
                w.push(c);
                started = true;
            }
        }
    }
    end_word(&mut w, &mut words, &mut started);
    if !words.is_empty() {
        all.push(words);
    }
    all
}

/// For a path about to be written in OneDrive: the first of its folders that isn't on this computer (from OneDrive's
/// own folder down), with its path online — none when every folder it needs is here.
pub fn missing_folder(root: &Path, target: &Path, is_folder: bool) -> Option<(PathBuf, String)> {
    let rel = relative(root, target)?;
    let mut parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    if !is_folder {
        parts.pop();
    }
    let mut here = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut online = vec![];
    for p in parts {
        here.push(p);
        online.push(p);
        if !here.exists() {
            return Some((here, online.join("/")));
        }
    }
    None
}

/// `echoctl onedrive-hook`: Claude Code asks it before a tool call in Echo's sessions. A write that would make, here,
/// a folder OneDrive holds online but doesn't sync to this computer is refused (exit 2, the reason to Claude); any
/// other call goes on at once — OneDrive is asked only when a folder is missing here.
pub fn hook() -> i32 {
    use std::io::Read;
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    let Ok(call) = serde_json::from_str::<Value>(&raw) else { return 0 };
    let Some(root) = root() else { return 0 };
    let cwd = call["cwd"].as_str().map(PathBuf::from).or_else(|| std::env::current_dir().ok()).unwrap_or_default();
    for (t, is_folder) in targets(&call) {
        let t = if t.is_absolute() { t } else { cwd.join(t) };
        let Some((folder, online)) = missing_folder(&root, &t, is_folder) else { continue };
        // At most 10 s for OneDrive's answer: a slow network lets the write go on rather than hold it.
        let (tx, rx) = std::sync::mpsc::channel();
        let ask = online.clone();
        std::thread::spawn(move || {
            let _ = tx.send(online_exists(&ask).unwrap_or(false));
        });
        if rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap_or(false) {
            eprintln!(
                "Not here: “{}” is in the user's OneDrive online but OneDrive doesn't sync it to this computer — a folder made here would stay on this computer and conflict online. Ask the user to choose it in OneDrive → Settings → Account → Choose folders (then work with it here), or work with it online (echoctl m365 drive).",
                folder.display()
            );
            return 2;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onedrive_is_found_on_this_mac_and_a_path_in_it_is_told_apart() {
        let (_env, root) = crate::paths::testenv::folders("onedrive-detect");
        let home = root.join("home-of-sam");
        let cs = home.join("Library/CloudStorage");
        std::fs::create_dir_all(cs.join("OneDrive-Acme")).unwrap();
        std::fs::create_dir_all(cs.join("OneDrive-SharedLibraries-Acme")).unwrap();
        let none = |_: &str| None;
        if !cfg!(windows) {
            std::fs::create_dir_all(cs.join("OneDrive-Personal")).unwrap();
            assert_eq!(detect(&home, none), Some(cs.join("OneDrive-Acme")), "its own, not the shared libraries or a personal one");
            std::fs::create_dir_all(cs.join("OneDrive-Globex")).unwrap();
            assert_eq!(detect(&home, none), None, "two accounts: the user says which");
        }
        let od = cs.join("OneDrive-Acme");
        std::fs::create_dir_all(od.join("Projects")).unwrap();
        assert_eq!(relative(&od, &od.join("Projects/Acme/Rollout")).as_deref(), Some("Projects/Acme/Rollout"), "not there yet: by its real parent");
        assert_eq!(relative(&od, &od).as_deref(), Some(""));
        assert_eq!(relative(&od, &home.join("Documents/x")), None);
        assert!(setting("auto").unwrap().is_none() && setting("none").unwrap().as_deref() == Some("none"));
        assert!(setting(&root.join("no-such").to_string_lossy()).is_err());
    }

    /// As Bash reads it: in double quotes a backslash escapes only `$`, `` ` ``, `"`, `\` and a line break, else it is
    /// itself — a Windows path in quotes keeps its backslashes (it lost them, the 0.9.4 Windows check found).
    #[test]
    fn a_backslash_in_double_quotes_is_kept_unless_it_escapes() {
        let words = |s: &str| commands(s).into_iter().flatten().collect::<Vec<_>>();
        assert_eq!(words(r#"cp a "C:\Users\x\Synced/n.md""#), ["cp", "a", r"C:\Users\x\Synced/n.md"]);
        assert_eq!(words(r#"echo "a\"b" "c\\d" "e\$f""#), ["echo", "a\"b", r"c\d", "e$f"]);
        assert_eq!(words(r"echo 'g\h' i\ j"), ["echo", r"g\h", "i j"], "single quotes keep it; unquoted it escapes");
    }

    #[test]
    fn a_write_finds_the_folder_onedrive_doesnt_have_here() {
        let (_env, root) = crate::paths::testenv::folders("onedrive-hook");
        let od = root.join("OneDrive-Acme");
        std::fs::create_dir_all(od.join("Synced")).unwrap();
        let call = |tool: &str, input: Value| json!({"tool_name": tool, "tool_input": input});
        let t = targets(&call("Bash", json!({"command": format!("mkdir -p '{0}/Projects/Acme' && cp notes.md \"{0}/Synced/n.md\"; echo hi", od.display())})));
        assert_eq!(t, vec![(od.join("Projects/Acme"), true), (od.join("Synced/n.md"), false)]);
        // A folder made after another command is still a folder; quotes keep `&` in a name; `\ ` escapes a space.
        let t = targets(&call("Bash", json!({"command": format!("cd /tmp && mkdir \"{0}/Sales & Marketing\" ; mkdir '{1}'/My\\ Folder", od.display(), od.display())})));
        assert_eq!(t, vec![(od.join("Sales & Marketing"), true), (od.join("My Folder"), true)]);
        assert_eq!(targets(&call("Write", json!({"file_path": "/x/a.md", "content": "x"}))), vec![(PathBuf::from("/x/a.md"), false)]);
        assert!(targets(&call("Read", json!({"file_path": "/x/a.md"}))).is_empty());
        // The first folder missing here, from OneDrive's own down; none for a write into a folder that is here.
        assert_eq!(missing_folder(&od, &od.join("Projects/Acme/Rollout"), true).map(|(_, o)| o).as_deref(), Some("Projects"));
        assert_eq!(missing_folder(&od, &od.join("Synced/n.md"), false), None);
        assert_eq!(missing_folder(&od, &od.join("Synced/New/n.md"), false).map(|(_, o)| o).as_deref(), Some("Synced/New"));
        assert_eq!(missing_folder(&od, &root.join("elsewhere/x"), true), None, "not in OneDrive");
    }

    #[test]
    fn what_to_do_follows_whether_it_is_here_and_online() {
        assert!(advice(false, Some(true)).starts_with("It is in OneDrive online but not on this computer") && advice(false, Some(true)).contains("Don't create it here"));
        assert!(advice(true, Some(false)).contains("isn't uploaded"));
        assert!(advice(false, Some(false)).contains("creating it here is fine"));
        assert!(advice(true, Some(true)).contains("work with it here"), "synced: local is faster");
        let many: Vec<String> = (0..3000).map(|i| format!("file-{i:04}.md")).collect();
        let c = compare(&many, &[]);
        assert_eq!((c["only_here"].as_u64(), c["only_here_first"].as_array().map(Vec::len)), (Some(3000), Some(SHOWN)), "thousands counted, a few named");
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(compare(&s(&["Brief.docx", "JD.pdf", "notes.md"]), &s(&["brief.docx", "RFI.xlsx", "notes.md"])), json!({"only_here": 1, "only_online": 1, "both": 2, "only_here_first": ["JD.pdf"], "only_online_first": ["RFI.xlsx"]}), "names as OneDrive compares them, ignoring case");
        assert!(advice(false, None).contains("don't create a folder here"));
    }
}

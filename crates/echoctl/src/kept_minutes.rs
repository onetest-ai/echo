//! The minutes a minutes run started from: kept beside them while it runs, put back when it ends without `done`
//! after replacing them (`notes::run`).

use crate::calls;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// The copies: `notes.md` and `actions.json` as the run found them.
const KEPT: [(&str, &str); 2] = [("notes.md", ".notes-before-run.md"), ("actions.json", ".actions-before-run.json")];

/// Keep copies of the call's minutes and action items before a run may replace them. Copies a run left when
/// Echo stopped mid-run may be the only good minutes: they go to the trash, never overwritten.
pub fn keep(dir: &Path) {
    if dir.join(KEPT[0].1).exists() && fs::read(dir.join(KEPT[0].0)).ok() != fs::read(dir.join(KEPT[0].1)).ok() {
        let bin = trashed(dir, &format!("interrupted-{}", chrono::Local::now().format("%Y%m%dT%H%M%S")));
        for (_, copy) in KEPT {
            let _ = fs::rename(dir.join(copy), bin.join(copy.trim_start_matches(".").replace("-before-run", "")));
        }
    }
    for (file, copy) in KEPT {
        let _ = match fs::read(dir.join(file)) {
            Ok(b) => crate::paths::write_atomic(&dir.join(copy), &b),
            Err(_) => fs::remove_file(dir.join(copy)),
        };
    }
}

/// A run that didn't end `done` but changed the minutes: the minutes it started from go back, and its action
/// items when it applied them; its own go to the trash, and `message` says so. The copies go either way.
pub fn settle(dir: &Path, done: bool, message: &mut Option<String>, run_id: &str, log: &mut impl Write) {
    let [(notes, notes_copy), (actions, actions_copy)] = KEPT;
    let Ok(kept) = fs::read(dir.join(notes_copy)) else {
        let _ = fs::remove_file(dir.join(actions_copy));
        return;
    };
    let now = fs::read(dir.join(notes)).ok();
    if !done && now.as_deref() != Some(&kept[..]) {
        let bin = trashed(dir, run_id);
        if let Some(now) = &now {
            let _ = fs::write(bin.join(notes), now);
        }
        let mut back = crate::paths::write_atomic(&dir.join(notes), &kept).is_ok();
        if back && calls::actions_written_by(dir).as_deref() == Some(run_id) {
            let _ = fs::copy(dir.join(actions), bin.join(actions));
            back = match fs::read(dir.join(actions_copy)) {
                Ok(a) => crate::paths::write_atomic(&dir.join(actions), &a).is_ok(),
                Err(_) => fs::remove_file(dir.join(actions)).is_ok(),
            } && fs::remove_file(dir.join(calls::ACTIONS_GENERATION)).is_ok();
        }
        if back {
            let _ = writeln!(log, "{}  The previous minutes are back; what this run wrote is in {}.", chrono::Local::now().format("%H:%M:%S"), bin.display());
            *message = Some(kept_message(message.as_deref()));
        }
    }
    for (_, copy) in KEPT {
        let _ = fs::remove_file(dir.join(copy));
    }
}

/// A trash folder for what a run wrote over: `<trash>/<call>.notes-<what>/`.
fn trashed(dir: &Path, what: &str) -> PathBuf {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let bin = calls::trash_dir().join(format!("{name}.notes-{what}"));
    let _ = fs::create_dir_all(&bin);
    bin
}

/// The run's message, ending in what happened to the minutes; "the minutes were written" no longer holds.
fn kept_message(why: Option<&str>) -> String {
    let why = why.unwrap_or("").trim_end().trim_end_matches(':');
    if why.is_empty() || why.starts_with("The minutes were written") {
        return "The run didn't finish, so the previous minutes are kept — see the log, then Try again.".into();
    }
    let sep = if why.ends_with(['.', '!', '?']) { " " } else { ". " };
    format!("{why}{sep}The previous minutes are kept.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_that_fails_after_replacing_the_minutes_puts_the_previous_ones_back() {
        let (_env, root) = crate::paths::testenv::folders("notes-keep-minutes");
        let dir = root.join("home/calls/2026-11-09_100000-weekly");
        fs::create_dir_all(&dir).unwrap();
        let (notes, actions) = (dir.join("notes.md"), dir.join("actions.json"));
        let mut log = Vec::new();
        let mut run = |done: bool, wrote: Option<&str>, applied: bool| {
            fs::write(&notes, "# Good minutes\n").unwrap();
            fs::write(&actions, "[\"good\"]").unwrap();
            let _ = fs::remove_file(dir.join(calls::ACTIONS_GENERATION));
            keep(&dir);
            if let Some(w) = wrote {
                fs::write(&notes, w).unwrap();
            }
            if applied {
                fs::write(&actions, "[\"new\"]").unwrap();
                fs::write(dir.join(calls::ACTIONS_GENERATION), r#"{"run":"r1"}"#).unwrap();
            }
            let mut message = Some("stopped after 20 minutes without finishing".to_string());
            settle(&dir, done, &mut message, "r1", &mut log);
            assert!(KEPT.iter().all(|(_, c)| !dir.join(c).exists()), "the copies go (done: {done})");
            message
        };
        // Failed after writing the minutes and applying its items: both back, the run's in the trash.
        assert_eq!(run(false, Some("# Half\n"), true).as_deref(), Some("stopped after 20 minutes without finishing. The previous minutes are kept."));
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# Good minutes\n");
        assert_eq!(fs::read_to_string(&actions).unwrap(), "[\"good\"]");
        assert!(!dir.join(calls::ACTIONS_GENERATION).exists());
        let bin = calls::trash_dir().join("2026-11-09_100000-weekly.notes-r1");
        assert_eq!(fs::read_to_string(bin.join("notes.md")).unwrap(), "# Half\n");
        assert_eq!(fs::read_to_string(bin.join("actions.json")).unwrap(), "[\"new\"]");
        // Written but not applied: the minutes back, the items untouched.
        run(false, Some("# Half\n"), false);
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# Good minutes\n");
        // Failed without touching them, or done: nothing put back.
        assert_eq!(run(false, None, false).as_deref(), Some("stopped after 20 minutes without finishing"));
        run(true, Some("# Better minutes\n"), true);
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# Better minutes\n");
        assert_eq!(fs::read_to_string(&actions).unwrap(), "[\"new\"]");
        // A first run (no minutes before) that fails keeps what it wrote.
        fs::remove_file(&notes).unwrap();
        keep(&dir);
        fs::write(&notes, "# First\n").unwrap();
        settle(&dir, false, &mut None, "r2", &mut log);
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# First\n");
    }

    #[test]
    fn copies_left_by_a_run_echo_never_saw_end_go_to_the_trash() {
        let (_env, root) = crate::paths::testenv::folders("notes-keep-interrupted");
        let dir = root.join("home/calls/2026-11-09_100000-weekly");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("notes.md"), "# Good minutes\n").unwrap();
        keep(&dir);
        fs::write(dir.join("notes.md"), "# Half, Echo died\n").unwrap();
        keep(&dir);
        let bin = fs::read_dir(calls::trash_dir()).unwrap().flatten().map(|e| e.path()).find(|p| p.to_string_lossy().contains(".notes-interrupted-")).unwrap();
        assert_eq!(fs::read_to_string(bin.join("notes.md")).unwrap(), "# Good minutes\n");
        assert_eq!(fs::read_to_string(dir.join(KEPT[0].1)).unwrap(), "# Half, Echo died\n", "this run's copy");
    }

    #[test]
    fn the_message_says_the_previous_minutes_are_kept() {
        assert_eq!(kept_message(Some("Claude Code exited with 1")), "Claude Code exited with 1. The previous minutes are kept.");
        assert_eq!(kept_message(Some("Refused:")), "Refused. The previous minutes are kept.");
        assert!(kept_message(Some("The minutes were written but not the action items — see the log, then Try again.")).starts_with("The run didn't finish"));
    }
}

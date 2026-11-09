//! The minutes a minutes run started from: kept beside them while it runs, put back when it ends without `done`
//! after replacing them (`notes::run`).

use crate::calls;
use std::fs;
use std::io::Write;
use std::path::Path;

const BEFORE_RUN: &str = ".notes-before-run.md";

/// Keep a copy of the call's minutes before a run may replace them (a copy left by a run that never ended is
/// replaced here).
pub fn keep(dir: &Path) {
    let _ = match fs::read(dir.join("notes.md")) {
        Ok(b) => crate::paths::write_atomic(&dir.join(BEFORE_RUN), &b),
        Err(_) => fs::remove_file(dir.join(BEFORE_RUN)),
    };
}

/// A run that didn't end `done` but changed the minutes: the ones it started from go back, its own go to the
/// trash, and `message` says so. The copy goes either way.
pub fn settle(dir: &Path, done: bool, message: &mut Option<String>, run_id: &str, log: &mut impl Write) {
    let before = dir.join(BEFORE_RUN);
    let Ok(kept) = fs::read(&before) else { return };
    let notes = dir.join("notes.md");
    let now = fs::read(&notes).ok();
    if !done && now.as_deref() != Some(&kept[..]) {
        if let Some(now) = &now {
            let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let failed = calls::trash_dir().join(format!("{name}.notes-{run_id}.md"));
            let _ = fs::create_dir_all(calls::trash_dir()).and_then(|_| fs::write(&failed, now));
        }
        if crate::paths::write_atomic(&notes, &kept).is_ok() {
            let _ = writeln!(log, "{}  The previous minutes are back; the ones this run wrote are in the trash.", chrono::Local::now().format("%H:%M:%S"));
            let why = message.as_deref().unwrap_or("").trim_end();
            let sep = if why.is_empty() { "" } else if why.ends_with(['.', '!', '?']) { " " } else { ". " };
            *message = Some(format!("{why}{sep}The previous minutes are kept."));
        }
    }
    let _ = fs::remove_file(before);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_that_fails_after_replacing_the_minutes_puts_the_previous_ones_back() {
        let (_env, root) = crate::paths::testenv::folders("notes-keep-minutes");
        let dir = root.join("home/calls/2026-11-09_100000-weekly");
        fs::create_dir_all(&dir).unwrap();
        let notes = dir.join("notes.md");
        let mut log = Vec::new();
        let mut run = |done: bool, wrote: Option<&str>| {
            fs::write(&notes, "# Good minutes\n").unwrap();
            keep(&dir);
            if let Some(w) = wrote {
                fs::write(&notes, w).unwrap();
            }
            let mut message = Some("stopped after 20 minutes without finishing".to_string());
            settle(&dir, done, &mut message, "r1", &mut log);
            assert!(!dir.join(BEFORE_RUN).exists(), "the copy goes (done: {done})");
            message
        };
        // Failed after writing: the previous minutes are back, and the run's are in the trash.
        assert_eq!(run(false, Some("# Half\n")).as_deref(), Some("stopped after 20 minutes without finishing. The previous minutes are kept."));
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# Good minutes\n");
        let trashed = calls::trash_dir().join("2026-11-09_100000-weekly.notes-r1.md");
        assert_eq!(fs::read_to_string(trashed).unwrap(), "# Half\n");
        // Failed without touching them, or done: nothing put back.
        assert_eq!(run(false, None).as_deref(), Some("stopped after 20 minutes without finishing"));
        run(true, Some("# Better minutes\n"));
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# Better minutes\n");
        // A first run (no minutes before) that fails keeps what it wrote, even past a copy an earlier run left.
        fs::write(dir.join(BEFORE_RUN), "# Deleted since\n").unwrap();
        fs::remove_file(&notes).unwrap();
        keep(&dir);
        fs::write(&notes, "# First\n").unwrap();
        settle(&dir, false, &mut None, "r2", &mut log);
        assert_eq!(fs::read_to_string(&notes).unwrap(), "# First\n");
    }
}

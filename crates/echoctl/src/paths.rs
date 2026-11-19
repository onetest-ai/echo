//! Where Echo keeps things. Nothing depends on a Claude Code workspace or the cwd:
//!
//! - **config dir** (fixed per user): macOS `~/Library/Application Support/echo`, Windows
//!   `%APPDATA%\echo`. Holds `home.json` (the chosen data folder) and `models/`
//!   (machine-level, too big to live in a synced folder).
//! - **home** (chosen at setup): `calls/` (one folder per call) and `.echo/` (config,
//!   control files, events). Resolution: `ECHOCTL_HOME` → `home.json` → the config dir.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub(crate) fn user_home() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| ".".into())
}

/// The user's own Claude Code folder: `CLAUDE_CONFIG_DIR` when Echo has it, else `~/.claude`.
pub(crate) fn own_claude_folder() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| user_home().join(".claude"))
}

/// The variable a development build misses to run under Claude Code: the run's own folders, or none. A test agent's
/// `echoctl` without its data's env would otherwise read and write the user's real config and data.
fn missing_run_env(under_claude_code: bool, config_set: bool, home_set: bool) -> Option<&'static str> {
    match (under_claude_code, config_set, home_set) {
        (false, ..) => None,
        (true, false, _) => Some("ECHOCTL_CONFIG_DIR"),
        (true, true, false) => Some("ECHOCTL_HOME"),
        (true, true, true) => None,
    }
}

/// A development build started by Claude Code (`CLAUDECODE`) without the run's folders exits before it resolves the
/// real config dir or data folder, or changes the user's own files elsewhere (start at login, the shell's PATH, the
/// CLI link). A release build, and a person's own terminal, never stop here. A unit test outside `testenv` panics
/// here instead (`testenv::refuse_outside`).
pub(crate) fn refuse_real_folders() {
    #[cfg(test)]
    testenv::refuse_outside();
    if !cfg!(all(debug_assertions, not(test))) {
        return;
    }
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if let Some(k) = missing_run_env(set("CLAUDECODE"), set("ECHOCTL_CONFIG_DIR"), set("ECHOCTL_HOME")) {
        eprintln!(
            "echoctl (development build) under Claude Code: {k} is not set, so this would use your real Echo config and data. \
             Load the run's env first: set -a; . <dir>/env; set +a"
        );
        std::process::exit(2);
    }
}

/// Whether this run may use the real config and data folders: false only where `refuse_real_folders` would refuse
/// (a development build under Claude Code without the run's env), for a caller that can do without them.
pub fn real_folders_allowed() -> bool {
    if !cfg!(all(debug_assertions, not(test))) {
        return true;
    }
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    missing_run_env(set("CLAUDECODE"), set("ECHOCTL_CONFIG_DIR"), set("ECHOCTL_HOME")).is_none()
}

/// Per-user application directory (fixed; never moves with the data folder).
pub fn config_dir() -> PathBuf {
    refuse_real_folders();
    if let Some(d) = std::env::var_os("ECHOCTL_CONFIG_DIR") {
        return d.into();
    }
    // A test's helper thread whose test has just ended: never this computer's folder.
    #[cfg(test)]
    testenv::outside();
    #[cfg(windows)]
    if let Some(d) = std::env::var_os("APPDATA") {
        return PathBuf::from(d).join("echo");
    }
    #[cfg(target_os = "macos")]
    return user_home().join("Library/Application Support/echo");
    #[allow(unreachable_code)]
    user_home().join(".config/echo")
}

#[derive(Debug, Serialize, Deserialize)]
struct Pointer {
    home: PathBuf,
}

fn pointer_file() -> PathBuf {
    config_dir().join("home.json")
}

/// Where `home()` came from: "env", "chosen" or "default" (nothing chosen yet).
pub fn home_source() -> &'static str {
    if std::env::var_os("ECHOCTL_HOME").is_some() {
        "env"
    } else if chosen_home().is_some() {
        "chosen"
    } else {
        "default"
    }
}

fn chosen_home() -> Option<PathBuf> {
    let p: Pointer = serde_json::from_slice(&fs::read(pointer_file()).ok()?).ok()?;
    Some(dunce::simplified(&p.home).to_path_buf())
}

/// The user's Downloads folder (the window's exports go there): `ECHOCTL_DOWNLOADS` in its place when set (the test
/// data's env sets it), and on test data (`ECHOCTL_TEST_OPEN_LOG_ONLY`) without it the data folder's own `Downloads`,
/// never the user's.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn downloads_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ECHOCTL_DOWNLOADS").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if std::env::var_os("ECHOCTL_TEST_OPEN_LOG_ONLY").is_some_and(|v| !v.is_empty()) {
        return home().join("Downloads");
    }
    dirs::download_dir().unwrap_or_else(|| user_home().join("Downloads"))
}

/// The data folder: calls + state.
pub fn home() -> PathBuf {
    refuse_real_folders();
    std::env::var_os("ECHOCTL_HOME").map(PathBuf::from).or_else(chosen_home).unwrap_or_else(config_dir)
}

/// Has the user picked a data folder (setup done)?
pub fn home_chosen() -> bool {
    home_source() != "default"
}

/// Remember `dir` as the data folder (created if missing). Existing calls elsewhere stay put.
pub fn set_home(dir: &Path) -> Result<PathBuf> {
    let dir = if dir.is_absolute() { dir.to_path_buf() } else { std::env::current_dir()?.join(dir) };
    fs::create_dir_all(dir.join("calls")).with_context(|| format!("creating {}", dir.display()))?;
    fs::create_dir_all(dir.join(".echo"))?;
    // dunce: no `\?` verbatim paths on Windows (shown to the user, handed to other tools).
    let dir = dunce::canonicalize(&dir).unwrap_or(dir);
    fs::create_dir_all(config_dir())?;
    let tmp = config_dir().join("home.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&Pointer { home: dir.clone() })?)?;
    fs::rename(tmp, pointer_file())?;
    crate::models::adopt_downloaded();
    Ok(dir)
}

/// The default data folder offered at setup.
pub fn default_home() -> PathBuf {
    user_home().join("Echo")
}

/// `<home>/calls` — one folder per recorded call.
pub fn calls_dir() -> PathBuf {
    home().join("calls")
}

/// `<home>/.echo` — config, control files, events.
pub fn state_dir() -> PathBuf {
    home().join(".echo")
}

/// `<config dir>/models` — Whisper models, shared by every data folder.
pub fn models_dir() -> PathBuf {
    config_dir().join("models")
}

/// Models downloaded before they moved to the config dir (`<home>/.echo/models`).
pub fn legacy_models_dir() -> PathBuf {
    state_dir().join("models")
}

/// The folder of the running program, links followed: `/usr/local/bin/echoctl` (`app install-cli`)
/// is a link to `Echo.app/Contents/MacOS/echoctl`, and what ships beside it is found from there.
#[cfg(any(feature = "transcribe", feature = "intake"))]
pub fn exe_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    dunce::canonicalize(&exe).unwrap_or(exe).parent().map(Path::to_path_buf)
}

/// Replace a file in one step (a temporary file of its own beside it, then a rename), so a reader
/// never sees it half written and two writers never share a temporary file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
    if let Err(e) = f.write_all(bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    drop(f);
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// The finished file `tmp` put at `to` (its folder) only where nothing is there: never over a file, also when writers
/// race (one wins, the others get `AlreadyExists`). An exclusive rename (`renamex_np(RENAME_EXCL)` on macOS,
/// `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows); on a volume without it a hard link; on one with
/// neither (exFAT and FAT on macOS, SMB shares without hard links) `to` taken new (`O_EXCL`) and `tmp` renamed over it:
/// for that moment `to` is empty, another program writing it then is overwritten, and a crash then leaves it empty.
/// Never through a link: none of them follows one at `to`. `tmp` is gone once it is placed.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
pub fn place_new(tmp: &Path, to: &Path) -> std::io::Result<()> {
    match rename_excl(tmp, to) {
        Err(e) if unsupported(&e) => {}
        r => return r,
    }
    match link_new(tmp, to) {
        Err(e) if unsupported(&e) => claim_new(tmp, to),
        r => r,
    }
}

/// The volume can't do it: a mode `place_new` tries the next way for.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn unsupported(e: &std::io::Error) -> bool {
    #[cfg(unix)]
    let codes = [libc::ENOTSUP, libc::EOPNOTSUPP, libc::ENOSYS, libc::EINVAL, libc::EPERM];
    // ERROR_INVALID_FUNCTION, ERROR_NOT_SUPPORTED.
    #[cfg(not(unix))]
    let codes = [1, 50];
    e.kind() == std::io::ErrorKind::Unsupported || e.raw_os_error().is_some_and(|c| codes.contains(&c))
}

#[cfg_attr(not(feature = "js"), allow(dead_code))]
#[cfg(target_os = "macos")]
fn rename_excl(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes()).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput));
    let (from, to) = (c(from)?, c(to)?);
    // SAFETY: two NUL-terminated paths that outlive the call.
    match unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

#[cfg_attr(not(feature = "js"), allow(dead_code))]
#[cfg(windows)]
fn rename_excl(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVE_FILE_FLAGS};
    let w = |p: &Path| p.as_os_str().encode_wide().chain([0]).collect::<Vec<u16>>();
    let (from, to) = (w(from), w(to));
    // SAFETY: two NUL-terminated wide paths that outlive the call; no flags: a file at `to` is never replaced.
    unsafe { MoveFileExW(windows_core::PCWSTR(from.as_ptr()), windows_core::PCWSTR(to.as_ptr()), MOVE_FILE_FLAGS(0)) }.map_err(|e| {
        // An HRESULT made from a Win32 error carries it in its low 16 bits (ERROR_ALREADY_EXISTS → AlreadyExists).
        let c = e.code().0 as u32;
        std::io::Error::from_raw_os_error(if c & 0xFFFF_0000 == 0x8007_0000 { (c & 0xFFFF) as i32 } else { c as i32 })
    })
}

#[cfg_attr(not(feature = "js"), allow(dead_code))]
#[cfg(not(any(target_os = "macos", windows)))]
fn rename_excl(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// `place_new` on a volume with hard links but no exclusive rename.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn link_new(tmp: &Path, to: &Path) -> std::io::Result<()> {
    fs::hard_link(tmp, to)?;
    let _ = fs::remove_file(tmp);
    Ok(())
}

/// `place_new` on a volume with neither: the name taken first, so only one writer gets it, then filled by a rename.
#[cfg_attr(not(feature = "js"), allow(dead_code))]
fn claim_new(tmp: &Path, to: &Path) -> std::io::Result<()> {
    fs::OpenOptions::new().write(true).create_new(true).open(to)?;
    fs::rename(tmp, to).inspect_err(|_| {
        // Only the empty file claimed: one put there meanwhile is left as it is.
        if fs::symlink_metadata(to).is_ok_and(|m| m.is_file() && m.len() == 0) {
            let _ = fs::remove_file(to);
        }
    })
}

/// `<dir>/<name><ext>`, or `<name>-2<ext>`, `-3`… when that is taken: a place in the trash nothing has yet, so two
/// things trashed within the same second (or millisecond) keep both.
pub fn unused(dir: &Path, name: &str, ext: &str) -> PathBuf {
    (1..).map(|n| dir.join(if n == 1 { format!("{name}{ext}") } else { format!("{name}-{n}{ext}") })).find(|p| !p.exists()).unwrap_or_else(|| dir.join(name))
}

/// Tests that point the data folder and the config dir at temporary folders take this first, so
/// they don't move each other's folders (the variables are process-wide).
#[cfg(test)]
pub mod testenv {
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    static LOCK: Mutex<()> = Mutex::new(());

    thread_local! {
        /// This thread's test holds a test environment (the variables are process-wide: another test's say nothing
        /// about this one).
        static HELD: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn outside() {
        panic!("a unit test resolved Echo's real config or data folder: take paths::testenv::folders first (#824)");
    }

    /// A unit test never resolves this computer's config or data folder. A test's own thread (named for the test)
    /// holds a test environment itself; a thread it started (a plugin host, a reader) runs while one is held.
    pub(super) fn refuse_outside() {
        if HELD.get() {
            return;
        }
        let a_test = std::thread::current().name().is_some_and(|n| n.contains("::"));
        if a_test || std::env::var_os("ECHOCTL_CONFIG_DIR").is_none() {
            outside();
        }
    }

    /// Held by a test: on drop the variables are restored and the folder removed.
    pub struct Env {
        _lock: MutexGuard<'static, ()>,
        root: PathBuf,
        before: [(&'static str, Option<std::ffi::OsString>); 6],
    }

    impl Drop for Env {
        fn drop(&mut self) {
            // The test's background threads end first, and its plugins unload, while its folders are still the
            // current ones.
            crate::signin::wait_idle();
            #[cfg(feature = "js")]
            crate::jshost::unload_all();
            HELD.set(false);
            for (k, v) in &self.before {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// A child process of a test, run on the folders its parent's test environment set (`ECHOCTL_HOME`,
    /// `ECHOCTL_CONFIG_DIR`): this thread holds them as its own.
    pub fn given_by_parent() {
        assert!(std::env::var_os("ECHOCTL_CONFIG_DIR").is_some(), "the parent test sets the folders");
        HELD.set(true);
    }

    /// `f()` comes true soon (polled every 20 ms) — for a lock just released: while another test thread starts
    /// a process, its child holds a copy of the descriptor until it execs (macOS).
    pub fn eventually(what: &str, f: impl Fn() -> bool) {
        let t = std::time::Instant::now();
        while !f() {
            // A bound for a failure only: it returns as soon as `f` holds. A fake Claude Code is a process of its own,
            // which takes more than a second to answer while the Mac is busy (#283).
            assert!(t.elapsed() < std::time::Duration::from_secs(10), "{what}");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    /// A fresh `<tmp>/echo-test-<name>-<pid>` with `home/` (ECHOCTL_HOME) and `config/`
    /// (ECHOCTL_CONFIG_DIR), until the returned guard drops. The user's own home and Claude Code folder are its
    /// `user/` and `user/.claude` (HOME, CLAUDE_CONFIG_DIR; ZDOTDIR and GIT_CONFIG_GLOBAL unset): no test reads this
    /// computer's Claude Code setup, the user's shell files or a git settings file the test didn't name.
    pub fn folders(name: &str) -> (Env, PathBuf) {
        let lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        HELD.set(true);
        crate::livenotes::forget_all();
        let root = std::env::temp_dir().join(format!("echo-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home/calls")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        std::fs::create_dir_all(root.join("user/.claude")).unwrap();
        let before = ["ECHOCTL_HOME", "ECHOCTL_CONFIG_DIR", "HOME", "CLAUDE_CONFIG_DIR", "ZDOTDIR", "GIT_CONFIG_GLOBAL"].map(|k| (k, std::env::var_os(k)));
        std::env::set_var("ECHOCTL_HOME", root.join("home"));
        std::env::set_var("ECHOCTL_CONFIG_DIR", root.join("config"));
        std::env::set_var("HOME", root.join("user"));
        std::env::set_var("CLAUDE_CONFIG_DIR", root.join("user/.claude"));
        // A login shell a test starts reads no rc file of the user's own (zsh's from ZDOTDIR when set).
        std::env::remove_var("ZDOTDIR");
        // Git reads the home's settings, not those of the environment the tests started in (a test data's `env`).
        std::env::remove_var("GIT_CONFIG_GLOBAL");
        (Env { _lock: lock, root: root.clone(), before }, root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dev_build_under_claude_code_needs_both_of_the_runs_folders() {
        // (CLAUDECODE, ECHOCTL_CONFIG_DIR, ECHOCTL_HOME) set → the variable it misses.
        assert_eq!(missing_run_env(true, false, false), Some("ECHOCTL_CONFIG_DIR"));
        assert_eq!(missing_run_env(true, false, true), Some("ECHOCTL_CONFIG_DIR"));
        assert_eq!(missing_run_env(true, true, false), Some("ECHOCTL_HOME"));
        assert_eq!(missing_run_env(true, true, true), None);
        // A person's own terminal: as before.
        for (c, h) in [(false, false), (false, true), (true, false), (true, true)] {
            assert_eq!(missing_run_env(false, c, h), None);
        }
    }

    #[test]
    #[should_panic(expected = "take paths::testenv::folders first")]
    fn a_unit_test_outside_a_test_environment_never_resolves_the_config_dir() {
        config_dir();
    }

    #[test]
    #[should_panic(expected = "take paths::testenv::folders first")]
    fn a_unit_test_outside_a_test_environment_never_resolves_the_data_folder() {
        home();
    }

    #[test]
    fn an_unused_name_never_takes_one_that_is_there() {
        let dir = std::env::temp_dir().join(format!("echo-unused-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(unused(&dir, "mine-20261109120000", ".md"), dir.join("mine-20261109120000.md"));
        fs::write(dir.join("mine-20261109120000.md"), "first").unwrap();
        assert_eq!(unused(&dir, "mine-20261109120000", ".md"), dir.join("mine-20261109120000-2.md"));
        fs::create_dir_all(dir.join("mine-20261109120000-2.md")).unwrap();
        assert_eq!(unused(&dir, "mine-20261109120000", ".md"), dir.join("mine-20261109120000-3.md"));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Eight writers placing their own finished file at `to` the `place` way: exactly one wins and its whole text is
    /// there; the others get `AlreadyExists` and nothing of theirs is left; the winner's file is never replaced.
    fn one_of_racing_writers_wins(name: &str, place: fn(&Path, &Path) -> std::io::Result<()>) {
        let dir = std::env::temp_dir().join(format!("echo-place-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let to = dir.join("Race.md");
        let won: Vec<std::io::Result<()>> = (0..8)
            .map(|i| {
                let (dir, to) = (dir.clone(), to.clone());
                std::thread::spawn(move || {
                    let tmp = dir.join(format!(".Race.md.{i}.part"));
                    fs::write(&tmp, format!("writer {i}")).unwrap();
                    let r = place(&tmp, &to);
                    let _ = fs::remove_file(&tmp);
                    r
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect();
        assert_eq!(won.iter().filter(|r| r.is_ok()).count(), 1, "{name}: {won:?}");
        assert!(won.iter().filter_map(|r| r.as_ref().err()).all(|e| e.kind() == std::io::ErrorKind::AlreadyExists), "{name}: {won:?}");
        let text = fs::read_to_string(&to).unwrap();
        assert!(text.starts_with("writer "), "{name}: {text}");
        let tmp = dir.join(".late.part");
        fs::write(&tmp, "late").unwrap();
        assert_eq!(place(&tmp, &to).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists, "{name}");
        assert_eq!(fs::read_to_string(&to).unwrap(), text, "{name}: never replaced");
        assert!(tmp.exists(), "{name}: a refused file is left to its caller");
        fs::remove_file(&tmp).unwrap();
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "{name}: no part left");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_file_is_placed_only_where_nothing_is_and_one_of_racing_writers_wins() {
        one_of_racing_writers_wins("place", place_new);
    }

    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn the_exclusive_rename_refuses_a_file_that_is_there() {
        one_of_racing_writers_wins("rename", rename_excl);
    }

    #[test]
    fn the_fallbacks_for_volumes_without_an_exclusive_rename_never_replace_a_file_either() {
        one_of_racing_writers_wins("link", link_new);
        one_of_racing_writers_wins("claim", claim_new);
    }

    #[cfg(unix)]
    #[test]
    fn a_new_file_is_never_placed_through_a_link_at_its_name() {
        let dir = std::env::temp_dir().join(format!("echo-place-through-a-link-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (to, target) = (dir.join("Note.md"), dir.join("target.md"));
        std::os::unix::fs::symlink(&target, &to).unwrap();
        for place in [place_new, link_new, claim_new] {
            let tmp = dir.join(".Note.md.part");
            fs::write(&tmp, "x").unwrap();
            assert_eq!(place(&tmp, &to).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
            assert!(!target.exists() && fs::symlink_metadata(&to).unwrap().file_type().is_symlink());
            fs::remove_file(&tmp).unwrap();
        }
        let _ = fs::remove_dir_all(&dir);
    }
}

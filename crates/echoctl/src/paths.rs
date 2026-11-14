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

/// Per-user application directory (fixed; never moves with the data folder).
pub fn config_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("ECHOCTL_CONFIG_DIR") {
        return d.into();
    }
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
#[cfg(any(feature = "transcribe", feature = "brain"))]
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

/// `<dir>/<name><ext>`, or `<name>-2<ext>`, `-3`… when that is taken: a place in the trash nothing has yet, so two
/// things trashed within the same second (or millisecond) keep both.
pub fn unused(dir: &Path, name: &str, ext: &str) -> PathBuf {
    (1..).map(|n| dir.join(if n == 1 { format!("{name}{ext}") } else { format!("{name}-{n}{ext}") })).find(|p| !p.exists()).unwrap_or_else(|| dir.join(name))
}

/// Tests that point the data folder and the config dir at temporary folders take this first, so
/// they don't move each other's folders (the variables are process-wide).
#[cfg(test)]
pub mod testenv {
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    static LOCK: Mutex<()> = Mutex::new(());

    /// Held by a test: on drop the variables are restored and the folder removed.
    pub struct Env {
        _lock: MutexGuard<'static, ()>,
        root: PathBuf,
        before: [(&'static str, Option<std::ffi::OsString>); 2],
    }

    impl Drop for Env {
        fn drop(&mut self) {
            // The test's background threads end first, while its folders are still the current ones.
            crate::signin::wait_idle();
            for (k, v) in &self.before {
                match v {
                    Some(v) => std::env::set_var(k, v),
                    None => std::env::remove_var(k),
                }
            }
            let _ = std::fs::remove_dir_all(&self.root);
        }
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
    /// (ECHOCTL_CONFIG_DIR), until the returned guard drops.
    pub fn folders(name: &str) -> (Env, PathBuf) {
        let lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::livenotes::forget_all();
        let root = std::env::temp_dir().join(format!("echo-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("home/calls")).unwrap();
        std::fs::create_dir_all(root.join("config")).unwrap();
        let before = ["ECHOCTL_HOME", "ECHOCTL_CONFIG_DIR"].map(|k| (k, std::env::var_os(k)));
        std::env::set_var("ECHOCTL_HOME", root.join("home"));
        std::env::set_var("ECHOCTL_CONFIG_DIR", root.join("config"));
        (Env { _lock: lock, root: root.clone(), before }, root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

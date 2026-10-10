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

fn user_home() -> PathBuf {
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
    Some(p.home)
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
    let dir = dir.canonicalize().unwrap_or(dir);
    fs::create_dir_all(config_dir())?;
    let tmp = config_dir().join("home.json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&Pointer { home: dir.clone() })?)?;
    fs::rename(tmp, pointer_file())?;
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

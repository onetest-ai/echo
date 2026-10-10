//! Updates from GitHub Releases (onetest-ai/echo).
//!
//! The app checks at start and every 6 h; the tray offers "Update to x.y.z…". The release is
//! read from the public API, or — while the repository is private — through the `gh` CLI when it
//! is installed and signed in. Installing (macOS): download the .dmg, mount it, verify that the
//! new Echo.app is signed by our Developer ID team, then a small detached script swaps the
//! bundle once this process has exited and opens the new one. Never while recording.

use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "macos")]
use std::process::Stdio;
use std::sync::Mutex;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

const REPO: &str = "onetest-ai/echo";
#[cfg(target_os = "macos")]
const TEAM_ID: &str = "373F9YS3DJ";

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub tag: String,
    pub asset: String,
    pub url: Option<String>,
    pub notes_url: String,
}

/// The newest release found by the last check, if it is newer than this build.
pub static AVAILABLE: Mutex<Option<Release>> = Mutex::new(None);

/// A release version we accept: `1.2.3` or `1.2.3-beta.1`. Everything else (a tag carrying
/// shell syntax, say) is refused before it gets anywhere near a path or a script.
pub fn valid_version(v: &str) -> bool {
    let (core, pre) = v.split_once('-').unwrap_or((v, ""));
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|p| !p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_digit()))
        && pre.len() <= 32
        && pre.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.')
        && !(v.contains('-') && pre.is_empty())
}

/// "1.2.10" > "1.2.9"; pre-release suffixes are ignored.
pub fn newer(candidate: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim_start_matches('v').split(['-', '+']).next().unwrap_or("").split('.').map(|p| p.parse().unwrap_or(0)).collect()
    };
    parse(candidate) > parse(current)
}

fn asset_suffix() -> &'static str {
    if cfg!(windows) {
        "windows-x64.zip"
    } else if cfg!(target_arch = "aarch64") {
        "macos-arm64.dmg"
    } else {
        "macos-x86_64.dmg"
    }
}

fn latest_json() -> Result<Value> {
    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    if let Ok(out) = Command::new("curl").args(["-fsSL", "-H", "Accept: application/vnd.github+json", &api]).output() {
        if out.status.success() {
            return Ok(serde_json::from_slice(&out.stdout)?);
        }
    }
    // Private repository: anonymous requests get 404; use the GitHub CLI's login.
    let gh = crate::app::find_program("gh");
    let out = Command::new(&gh)
        .args(["api", &format!("repos/{REPO}/releases/latest")])
        .output()
        .context("no access to GitHub releases (install the GitHub CLI and run `gh auth login`)")?;
    if !out.status.success() {
        bail!("GitHub: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(serde_json::from_slice(&out.stdout)?)
}

/// Look for a newer release. Pre-releases count (the beta ships as one).
pub fn check() -> Result<Option<Release>> {
    let v = latest_json().or_else(|_| -> Result<Value> {
        // /latest skips pre-releases: fall back to the newest of all releases.
        let gh = crate::app::find_program("gh");
        let out = Command::new(&gh).args(["api", &format!("repos/{REPO}/releases?per_page=5")]).output()?;
        let all: Value = serde_json::from_slice(&out.stdout)?;
        all.as_array().and_then(|a| a.first().cloned()).ok_or_else(|| anyhow!("no releases"))
    })?;
    let tag = v["tag_name"].as_str().unwrap_or_default().to_string();
    let version = tag.trim_start_matches('v').to_string();
    if !valid_version(&version) {
        bail!("ignoring release with an unexpected tag {tag:?}");
    }
    let found = v["assets"].as_array().into_iter().flatten().find(|a| {
        a["name"].as_str().is_some_and(|n| {
            n.ends_with(asset_suffix()) && n.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
    });
    let release = found.map(|a| Release {
        version: version.clone(),
        tag: tag.clone(),
        asset: a["name"].as_str().unwrap_or_default().to_string(),
        url: a["browser_download_url"].as_str().map(str::to_owned),
        notes_url: v["html_url"].as_str().unwrap_or_default().to_string(),
    });
    let release = release.filter(|r| newer(&r.version, env!("CARGO_PKG_VERSION")) && can_install());
    *AVAILABLE.lock().unwrap() = release.clone();
    Ok(release)
}

/// Only offer what can be installed: on macOS that needs an Echo.app to swap. A development
/// build with no app installed updates with git.
fn can_install() -> bool {
    #[cfg(target_os = "macos")]
    return crate::app::installed_app().is_some();
    #[allow(unreachable_code)]
    true
}

fn download(r: &Release, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(&r.asset);
    let _ = std::fs::remove_file(&dest);
    if let Some(url) = &r.url {
        if Command::new("curl").args(["-fsSL", "-o"]).arg(&dest).arg(url).status().is_ok_and(|s| s.success()) {
            return Ok(dest);
        }
    }
    let gh = crate::app::find_program("gh");
    let ok = Command::new(&gh)
        .args(["release", "download", &r.tag, "--repo", REPO, "--pattern", &r.asset, "--dir"])
        .arg(dir)
        .status()
        .is_ok_and(|s| s.success());
    if !ok || !dest.exists() {
        bail!("could not download {}", r.asset);
    }
    Ok(dest)
}

/// Download, verify and stage the update. A detached script swaps it in once the app process
/// (`wait_pid`) has exited and starts the new version; the caller then stops the app.
pub fn install(r: &Release, wait_pid: u32) -> Result<()> {
    if crate::watch::running_watcher().is_some_and(|w| w.recording.is_some()) {
        bail!("a call is being recorded — update after it ends");
    }
    let work = std::env::temp_dir().join(format!("echo-update-{}", r.version));
    let file = download(r, &work)?;
    platform_install(&file, &work, wait_pid)
}

#[cfg(target_os = "macos")]
fn platform_install(dmg: &Path, work: &Path, wait_pid: u32) -> Result<()> {
    let target = crate::app::installed_app().ok_or_else(|| anyhow!("Echo.app is not installed (a development build updates with git)"))?;
    let mount = work.join("mnt");
    std::fs::create_dir_all(&mount)?;
    let ok = Command::new("hdiutil").args(["attach", "-nobrowse", "-readonly", "-mountpoint"]).arg(&mount).arg(dmg).stdout(Stdio::null()).status()?.success();
    if !ok {
        bail!("could not open {}", dmg.display());
    }
    let staged = work.join("Echo.app");
    let _ = std::fs::remove_dir_all(&staged);
    let copied = Command::new("ditto").arg(mount.join("Echo.app")).arg(&staged).status()?.success();
    let _ = Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount).status();
    if !copied {
        bail!("the update image has no Echo.app");
    }
    // Only our own signature is accepted.
    let ok = Command::new("codesign")
        .args(["--verify", "--deep", "--strict", "-R"])
        .arg(format!("=anchor apple generic and certificate leaf[subject.OU] = \"{TEAM_ID}\""))
        .arg(&staged)
        .status()?
        .success();
    if !ok {
        bail!("the downloaded app is not signed by the Echo developer — not installed");
    }
    // Swap once this process is gone, then start the new version. Paths go in as positional
    // arguments ("$2", "$3"), never into the script text; a failed move restores the old app.
    let script = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.3; done
rm -rf "$2.previous"
if mv "$2" "$2.previous"; then
  if mv "$3" "$2"; then rm -rf "$2.previous"; else mv "$2.previous" "$2"; fi
fi
open -g "$2"
"#;
    spawn_detached_sh(script, &[&wait_pid.to_string(), &target.display().to_string(), &staged.display().to_string()])
}

/// Windows builds aren't code-signed yet, so a downloaded exe can't be verified: open the
/// release page instead of installing it.
#[cfg(windows)]
fn platform_install(_: &Path, _: &Path, _: u32) -> Result<()> {
    bail!("automatic updates need a signed Windows build — download it from the release page")
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_install(_: &Path, _: &Path, _: u32) -> Result<()> {
    bail!("updates are not supported on this platform")
}

#[cfg(target_os = "macos")]
fn spawn_detached_sh(script: &str, args: &[&str]) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new("/bin/sh");
    cmd.args(["-c", script, "echo-update"]).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn()?;
    Ok(())
}

/// `echoctl app update [--check]`.
pub fn cli(check_only: bool) -> Result<()> {
    match check()? {
        None => println!("Echo {} is up to date", env!("CARGO_PKG_VERSION")),
        Some(r) if check_only => println!("update available: {} ({})", r.version, r.notes_url),
        Some(r) => {
            // The swap waits for the running app (or, with none running, for this process).
            let app = crate::watch::running_watcher().map(|w| w.pid);
            install(&r, app.unwrap_or(std::process::id()))?;
            if app.is_some() {
                crate::app::quit()?;
            }
            println!("updating to {} — Echo restarts by itself", r.version);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_versions_are_accepted() {
        assert!(valid_version("0.1.1") && valid_version("1.20.3-beta.2"));
        assert!(!valid_version("9.9.9$(touch x)") && !valid_version("1.2") && !valid_version("1.2.3-") && !valid_version("1.2.3 x"));
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(newer("0.2.0", "0.1.0"));
        assert!(newer("v0.1.10", "0.1.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.1.0-beta", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
    }
}

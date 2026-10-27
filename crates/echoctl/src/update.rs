//! Updates from GitHub Releases (onetest-ai/echo).
//!
//! The app checks at start and every 6 h; the tray offers "Update to x.y.z…". The release is
//! read from the public API, or — while the repository is private — through the `gh` CLI when it
//! is installed and signed in. Installing (macOS): download the .dmg, mount it, verify that the
//! new Echo.app is signed by our Developer ID team, then a small detached script swaps the
//! bundle once this process has exited and opens the new one. Windows (unsigned builds): the
//! release's per-user MSI, checked against the SHA-256 digest GitHub publishes for it, installed
//! silently by msiexec (the MSI quits Echo, replaces the files and starts it again). Every
//! download is checked against that digest. Never while recording.

use crate::app::NoWindow;
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
    /// The asset's SHA-256 (hex) from the release API, read over HTTPS.
    pub sha256: Option<String>,
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
        "windows-x64.msi"
    } else if cfg!(target_arch = "aarch64") {
        "macos-arm64.dmg"
    } else {
        "macos-x86_64.dmg"
    }
}

fn latest_json() -> Result<Value> {
    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    if let Ok(out) = Command::new("curl").no_window().args(["-fsSL", "-H", "Accept: application/vnd.github+json", &api]).output() {
        if out.status.success() {
            return Ok(serde_json::from_slice(&out.stdout)?);
        }
    }
    // Private repository: anonymous requests get 404; use the GitHub CLI's login.
    let gh = crate::app::find_program("gh");
    let out = Command::new(&gh).no_window()
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
        let out = Command::new(&gh).no_window().args(["api", &format!("repos/{REPO}/releases?per_page=5")]).output()?;
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
        sha256: a["digest"].as_str().and_then(parse_digest),
    });
    let release = release.filter(|r| newer(&r.version, env!("CARGO_PKG_VERSION")) && can_install());
    *AVAILABLE.lock().unwrap() = release.clone();
    Ok(release)
}

/// "sha256:<64 hex>" → the hex, lower case.
fn parse_digest(d: &str) -> Option<String> {
    let hex = d.strip_prefix("sha256:")?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then(|| hex.to_ascii_lowercase())
}

/// Only offer what can be installed: on macOS that needs an Echo.app to swap; on Windows, Echo
/// running from the MSI's folder (a copy elsewhere would end up next to a second install). A
/// development build updates with git.
fn can_install() -> bool {
    #[cfg(target_os = "macos")]
    return crate::app::installed_app().is_some();
    #[cfg(windows)]
    return msi_folder().is_some_and(|dir| {
        let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
        let canon = |p: &Path| dunce::canonicalize(p).ok();
        exe_dir.is_some_and(|e| canon(&e).is_some() && canon(&e) == canon(&dir))
    });
    #[allow(unreachable_code)]
    true
}

/// Where the MSI installs Echo (packaging/windows/echo.wxs).
#[cfg(windows)]
fn msi_folder() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs").join("echo"))
}

fn download(r: &Release, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(&r.asset);
    let _ = std::fs::remove_file(&dest);
    if let Some(url) = &r.url {
        if Command::new("curl").no_window().args(["-fsSL", "-o"]).arg(&dest).arg(url).status().is_ok_and(|s| s.success()) {
            return Ok(dest);
        }
    }
    let gh = crate::app::find_program("gh");
    let ok = Command::new(&gh).no_window()
        .args(["release", "download", &r.tag, "--repo", REPO, "--pattern", &r.asset, "--dir"])
        .arg(dir)
        .status()
        .is_ok_and(|s| s.success());
    if !ok || !dest.exists() {
        bail!("could not download {}", r.asset);
    }
    Ok(dest)
}

/// The download must match the digest the release API gave. Windows builds aren't signed, so
/// there the digest is required; macOS also checks the signature afterwards.
fn verify_digest(r: &Release, file: &PathBuf) -> Result<()> {
    let Some(want) = &r.sha256 else {
        if cfg!(windows) {
            bail!("the release gives no checksum for {} — not installed", r.asset);
        }
        return Ok(());
    };
    let got = crate::models::sha256_file(file)?;
    if &got != want {
        let _ = std::fs::remove_file(file);
        bail!("{} doesn't match the release's checksum — not installed", r.asset);
    }
    Ok(())
}

/// An update downloaded, verified and staged, not yet swapped in (`commit`).
#[derive(Debug)]
pub struct Staged {
    /// macOS: the verified Echo.app copied out of the image; Windows: the verified MSI.
    file: PathBuf,
    work: PathBuf,
}

/// Download, verify and stage the update — nothing is scheduled yet, so the app can still decide
/// not to stop (`commit` does that part).
pub fn prepare(r: &Release) -> Result<Staged> {
    if crate::watch::running_watcher().is_some_and(|w| w.recording.is_some()) {
        bail!("a call is being recorded — update after it ends");
    }
    let work = std::env::temp_dir().join(format!("echo-update-{}", r.version));
    let file = download(r, &work)?;
    verify_digest(r, &file)?;
    Ok(Staged { file: platform_stage(&file, &work)?, work })
}

/// Swap the staged update in: a detached script does it once the app process (`wait_pid`) has
/// exited and starts the new version (Windows: the MSI, which restarts Echo itself); the caller
/// stops the app — before (`app update`: quitting saves a recording) or right after (the tray). Not
/// while the app records a call: one may have started since `prepare` (the window's save, a
/// question to the user).
pub fn commit(s: &Staged, wait_pid: u32) -> Result<()> {
    if crate::watch::running_watcher().is_some_and(|w| w.recording.is_some()) {
        bail!("a call is being recorded — update after it ends");
    }
    platform_commit(&s.file, &s.work, wait_pid)
}

#[cfg(target_os = "macos")]
fn platform_stage(dmg: &Path, work: &Path) -> Result<PathBuf> {
    crate::app::installed_app().ok_or_else(|| anyhow!("Echo.app is not installed (a development build updates with git)"))?;
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
    Ok(staged)
}

#[cfg(target_os = "macos")]
fn platform_commit(staged: &Path, _work: &Path, wait_pid: u32) -> Result<()> {
    let target = crate::app::installed_app().ok_or_else(|| anyhow!("Echo.app is not installed (a development build updates with git)"))?;
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

/// Windows: the verified MSI is what gets installed.
#[cfg(windows)]
fn platform_stage(msi: &Path, _work: &Path) -> Result<PathBuf> {
    Ok(msi.to_path_buf())
}

/// Run the (checksum-verified) MSI silently. It is per-user, so no elevation; its own custom
/// actions quit the running Echo (`app quit` waits for it) before replacing the files and start
/// the new one afterwards, so `wait_pid` isn't needed. `ECHO_SELF_UPDATE`: Echo saved the window's
/// notes and is stopping already, so that quit's answer doesn't stop the installation (a slow
/// exit would roll the update back and leave Echo stopped). msiexec outlives this process.
#[cfg(windows)]
fn platform_commit(msi: &Path, work: &Path, _wait_pid: u32) -> Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let msiexec = std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join("msiexec.exe")).unwrap_or_else(|| "msiexec.exe".into());
    Command::new(msiexec)
        .arg("/i")
        .arg(msi)
        .args(["/qn", "/norestart", "ECHO_SELF_UPDATE=1", "/l*v"])
        .arg(work.join("install.log"))
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .context("could not start the Windows installer")?;
    Ok(())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_stage(_: &Path, _: &Path) -> Result<PathBuf> {
    bail!("updates are not supported on this platform")
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_commit(_: &Path, _: &Path, _: u32) -> Result<()> {
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
            let staged = prepare(&r)?;
            // The running app quits first (it saves the notes typed in its window, or the user
            // keeps it open: then nothing is scheduled); the swap then waits for it — or, with
            // none running, for this process.
            let app = crate::watch::running_watcher().map(|w| w.pid);
            if app.is_some() {
                crate::app::quit()?;
            }
            commit(&staged, app.unwrap_or(std::process::id()))?;
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
    fn digests_are_plain_sha256() {
        let hex = "60d5062f1013306fff356ce4a0ecb1c9ceca176f4ac65d53a7b31e020e96758b";
        assert_eq!(parse_digest(&format!("sha256:{}", hex.to_uppercase())).as_deref(), Some(hex));
        assert!(parse_digest(hex).is_none() && parse_digest("sha256:abc").is_none() && parse_digest(&format!("sha1:{hex}")).is_none());
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

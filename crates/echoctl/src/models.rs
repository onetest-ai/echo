//! Whisper model catalog, hardware-based recommendation and verified downloads.
//!
//! Only multilingual models are offered (calls mix languages). Files come from the
//! whisper.cpp Hugging Face repo; the expected SHA-256 is read from the Hub's API at download
//! time and checked after the download.

use crate::app::NoWindow;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::config::Config;
use crate::paths;

const REPO: &str = "ggerganov/whisper.cpp";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Model {
    pub name: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    pub summary: &'static str,
}

pub const CATALOG: &[Model] = &[
    Model {
        name: "large-v3-turbo-q5_0",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 574,
        summary: "best speed/accuracy balance; near large-v3 quality, multilingual",
    },
    Model {
        name: "large-v3-turbo-q8_0",
        file: "ggml-large-v3-turbo-q8_0.bin",
        size_mb: 874,
        summary: "slightly more accurate than q5_0, same speed class",
    },
    Model {
        name: "large-v3-q5_0",
        file: "ggml-large-v3-q5_0.bin",
        size_mb: 1081,
        summary: "most accurate (accents, noisy calls); several times slower than turbo",
    },
    Model {
        name: "small-q8_0",
        file: "ggml-small-q8_0.bin",
        size_mb: 264,
        summary: "fast on weak hardware; noticeably more errors",
    },
];

pub fn find(name: &str) -> Result<&'static Model> {
    CATALOG.iter().find(|m| m.name == name).ok_or_else(|| {
        let names: Vec<_> = CATALOG.iter().map(|m| m.name).collect();
        anyhow!("unknown model '{name}' (available: {})", names.join(", "))
    })
}

/// Where the model file is (or will be downloaded to). Models downloaded into a data folder
/// by older versions are still used from there.
pub fn model_path(m: &Model) -> PathBuf {
    let p = paths::models_dir().join(m.file);
    let legacy = paths::legacy_models_dir().join(m.file);
    if !p.exists() && legacy.exists() {
        legacy
    } else {
        p
    }
}

#[derive(Debug, Serialize)]
pub struct Hardware {
    pub os: &'static str,
    pub arch: &'static str,
    pub cpu_threads: usize,
    pub ram_gb: Option<u32>,
    /// Apple Silicon → whisper.cpp runs on the GPU via Metal
    pub gpu_accelerated: bool,
}

fn ram_bytes() -> Option<u64> {
    let out = if cfg!(target_os = "macos") {
        Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?
    } else if cfg!(windows) {
        Command::new("powershell").no_window()
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory"])
            .output()
            .ok()?
    } else {
        return None;
    };
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

pub fn hardware() -> Hardware {
    let arch = std::env::consts::ARCH;
    Hardware {
        os: std::env::consts::OS,
        arch,
        cpu_threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
        ram_gb: ram_bytes().map(|b| (b / (1 << 30)) as u32),
        gpu_accelerated: cfg!(target_os = "macos") && arch == "aarch64",
    }
}

/// Recommendation + one-line reason, shown by first run and the doctor skill.
pub fn recommend(hw: &Hardware) -> (&'static str, String) {
    let ram = hw.ram_gb.unwrap_or(8);
    if ram < 8 {
        return ("small-q8_0", format!("only {ram} GB RAM"));
    }
    if hw.gpu_accelerated {
        return ("large-v3-turbo-q5_0", "Apple Silicon GPU (Metal) makes turbo fast".into());
    }
    if hw.cpu_threads >= 8 {
        return ("large-v3-turbo-q5_0", format!("CPU-only, {} threads is enough for turbo", hw.cpu_threads));
    }
    ("small-q8_0", format!("CPU-only with {} threads; turbo would be slow", hw.cpu_threads))
}

#[derive(Serialize)]
struct ListEntry {
    #[serde(flatten)]
    model: Model,
    downloaded: bool,
    active: bool,
    recommended: bool,
}

#[derive(Serialize)]
struct ListReport {
    hardware: Hardware,
    recommended: &'static str,
    reason: String,
    models: Vec<ListEntry>,
}

pub fn list(json: bool) -> Result<()> {
    let cfg = Config::load()?;
    let hw = hardware();
    let (rec, reason) = recommend(&hw);
    let models = CATALOG
        .iter()
        .map(|m| ListEntry {
            model: *m,
            downloaded: model_path(m).exists(),
            active: cfg.model.as_deref() == Some(m.name),
            recommended: m.name == rec,
        })
        .collect();
    let r = ListReport { hardware: hw, recommended: rec, reason, models };
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
        return Ok(());
    }
    println!("recommended: {} ({})\n", r.recommended, r.reason);
    for e in &r.models {
        let flags = [(e.active, "active"), (e.downloaded, "downloaded"), (e.recommended, "recommended")]
            .iter()
            .filter(|(on, _)| *on)
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {:<22} {:>5} MB  {}{}", e.model.name, e.model.size_mb, e.model.summary,
            if flags.is_empty() { String::new() } else { format!("  [{flags}]") });
    }
    Ok(())
}

/// Expected SHA-256 for a repo file, from the Hugging Face API (LFS object id).
fn expected_sha256(file: &str) -> Result<String> {
    let out = Command::new("curl").no_window()
        .args(["-fsSL", &format!("https://huggingface.co/api/models/{REPO}/tree/main")])
        .output()
        .context("running curl (needed to download models)")?;
    if !out.status.success() {
        bail!("could not reach huggingface.co: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let tree: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    tree.as_array()
        .and_then(|a| a.iter().find(|f| f["path"] == file))
        .and_then(|f| f["lfs"]["oid"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("{file} not found in {REPO}"))
}

pub(crate) fn sha256_file(path: &PathBuf) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Percent of the download in progress (for the tray's Models menu).
pub static PROGRESS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Run curl. In a real terminal show curl's own progress bar; otherwise (Claude Code's `!`,
/// pipes) print a line every 10% so logs and the conversation stay readable.
fn curl_with_progress(part: &PathBuf, url: &str, expected_bytes: u64) -> Result<std::process::ExitStatus> {
    use std::io::IsTerminal;
    let mut cmd = Command::new("curl").no_window();
    cmd.args(["-fL", "-C", "-", "-o"]).arg(part).arg(url);
    if std::io::stderr().is_terminal() {
        return cmd.arg("--progress-bar").status().context("running curl");
    }
    let mut child = cmd.arg("-sS").spawn().context("running curl")?;
    let mut last = 0;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        let have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
        let exact = (have * 100 / expected_bytes.max(1)).min(99);
        PROGRESS.store(exact as u8, std::sync::atomic::Ordering::Relaxed);
        let pct = exact / 10 * 10;
        if pct > last {
            eprintln!("  {pct}%");
            last = pct;
        }
    }
}

pub fn download(name: &str) -> Result<()> {
    let m = find(name)?;
    let dest = model_path(m);
    fs::create_dir_all(dest.parent().unwrap_or(&paths::models_dir()))?;
    let expected = expected_sha256(m.file)?;

    if dest.exists() && sha256_file(&dest)? == expected {
        eprintln!("{} already downloaded and verified", m.name);
    } else {
        // Download to .part with resume (-C -), so an interrupted download continues.
        let part = dest.with_extension("bin.part");
        // A complete .part (the last run died before the rename) makes curl's resume fail with
        // 416 forever: check it first.
        if fs::metadata(&part).is_ok_and(|p| p.len() >= m.size_mb as u64 * 1_000_000) {
            if sha256_file(&part)? == expected {
                fs::rename(&part, &dest)?;
                return finish_download(m, &dest);
            }
            let _ = fs::remove_file(&part); // full size but wrong: start over
        }
        let url = format!("https://huggingface.co/{REPO}/resolve/main/{}", m.file);
        eprintln!("downloading {} ({} MB) → {}", m.name, m.size_mb, dest.display());
        let status = curl_with_progress(&part, &url, m.size_mb as u64 * 1_000_000)?;
        if !status.success() {
            bail!("download failed (re-run the same command to resume)");
        }
        eprintln!("verifying checksum…");
        let got = sha256_file(&part)?;
        if got != expected {
            let _ = fs::remove_file(&part);
            bail!("checksum mismatch for {} (expected {expected}, got {got}); partial file removed", m.file);
        }
        fs::rename(&part, &dest)?;
    }

    finish_download(m, &dest)
}

/// The first downloaded model becomes the active one.
fn finish_download(m: &Model, dest: &std::path::Path) -> Result<()> {
    let mut cfg = Config::load()?;
    if cfg.model.is_none() {
        cfg.model = Some(m.name.to_string());
        cfg.save()?;
        eprintln!("active model: {}", m.name);
    }
    println!("{}", dest.display());
    Ok(())
}

pub fn use_model(name: &str) -> Result<()> {
    let m = find(name)?;
    if !model_path(m).exists() {
        bail!("{} is not downloaded — run `echoctl model download {}`", m.name, m.name);
    }
    let mut cfg = Config::load()?;
    cfg.model = Some(m.name.to_string());
    cfg.save()?;
    println!("active model: {}", m.name);
    Ok(())
}

/// The turbo model also serves live transcription and language detection, whatever the
/// active model is.
pub const HELPER_MODEL: &str = "large-v3-turbo-q5_0";

/// Why a downloaded model is needed even when it isn't the active one.
pub fn helper_role(name: &str) -> Option<&'static str> {
    (name == HELPER_MODEL).then_some("it also powers the live transcript and language detection")
}

/// `echoctl model remove`: delete a downloaded model (not the active one; the helper model
/// only with `force`).
pub fn remove(name: &str, force: bool) -> Result<()> {
    let m = find(name)?;
    let cfg = Config::load()?;
    if cfg.model.as_deref() == Some(m.name) {
        bail!("{} is the active model — switch with `echoctl model use <other>` first", m.name);
    }
    if let (Some(role), false) = (helper_role(m.name), force) {
        bail!("{} is still used: {role}. Remove it anyway with --force", m.name);
    }
    let mut removed = false;
    for dir in [paths::models_dir(), paths::legacy_models_dir()] {
        for f in [dir.join(m.file), dir.join(m.file).with_extension("bin.part")] {
            removed |= fs::remove_file(&f).is_ok();
        }
    }
    println!("{}", if removed { format!("removed {}", m.name) } else { format!("{} was not downloaded", m.name) });
    Ok(())
}

/// A data folder without a model choice adopts a downloaded model (the catalog lists the
/// preferred ones first): models are shared by all data folders, the choice is per folder, so
/// a new folder would otherwise have none although one is on disk. Called when the data folder
/// is set (first run, `home set`, Settings → data folder).
pub fn adopt_downloaded() {
    let Ok(mut cfg) = Config::load() else { return };
    if cfg.model.is_some() {
        return;
    }
    if let Some(m) = CATALOG.iter().find(|m| model_path(m).exists()) {
        cfg.model = Some(m.name.to_string());
        let _ = cfg.save();
    }
}

/// The active model's file, or a message that sends the user to setup.
#[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
pub fn active_model_path() -> Result<PathBuf> {
    let cfg = Config::load()?;
    let name = cfg
        .model
        .ok_or_else(|| anyhow!("no transcription model yet — choose one in the menu → Speech models (or `echoctl model list`)"))?;
    let m = find(&name)?;
    let p = model_path(m);
    if !p.exists() {
        bail!("model {name} is configured but missing — run `echoctl model download {name}`");
    }
    Ok(p)
}

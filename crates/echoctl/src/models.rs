//! Speech model catalog, hardware-based recommendation and verified downloads.
//!
//! Only multilingual models are offered (calls mix languages). Each model names its engine:
//! whisper.cpp, or NVIDIA Nemotron through NeMo-Speech.cpp (`nemo.rs`) — choosing a model is
//! choosing the engine. Files come from Hugging Face; the expected SHA-256 is read from the
//! Hub's API at download time and checked after the download.

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

const WHISPER_REPO: &str = "ggerganov/whisper.cpp";

/// The speech engine a model runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// whisper.cpp (linked in).
    Whisper,
    /// NVIDIA NeMo-Speech.cpp (its runtime ships with the app, `nemo.rs`).
    Nemo,
}

impl Engine {
    /// Its name in menus.
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    pub fn label(self) -> &'static str {
        match self {
            Engine::Whisper => "Whisper (whisper.cpp)",
            Engine::Nemo => "NVIDIA Nemotron",
        }
    }
}

/// What a model is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Speech to text: one of these is the active model.
    Transcribe,
    /// Who speaks when on the remote track (`speakers.rs`): used whenever it is downloaded.
    Speakers,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Model {
    pub name: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    pub summary: &'static str,
    pub engine: Engine,
    /// Hugging Face repository the file is downloaded from.
    #[serde(skip)]
    pub repo: &'static str,
    pub role: Role,
}

pub const CATALOG: &[Model] = &[
    Model {
        name: "large-v3-turbo-q5_0",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 574,
        summary: "best speed/accuracy balance; near large-v3 quality, multilingual",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "large-v3-turbo-q8_0",
        file: "ggml-large-v3-turbo-q8_0.bin",
        size_mb: 874,
        summary: "slightly more accurate than q5_0, same speed class",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "large-v3-q5_0",
        file: "ggml-large-v3-q5_0.bin",
        size_mb: 1081,
        summary: "most accurate (accents, noisy calls); several times slower than turbo",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "small-q8_0",
        file: "ggml-small-q8_0.bin",
        size_mb: 264,
        summary: "fast on weak hardware; noticeably more errors",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "nemotron-3.5-0.6b",
        file: "nemotron-3.5-asr-streaming-0.6b.q8_0.gguf",
        size_mb: 742,
        summary: "NVIDIA Nemotron: fast on a CPU, 40 languages (en, es, ru, uk among the best)",
        engine: Engine::Nemo,
        repo: "nvidia/nemotron-3.5-asr-streaming-0.6b",
        role: Role::Transcribe,
    },
    Model {
        name: "speakers-nemotron-3",
        file: "Nemotron-3-Diarization.q8_0.gguf",
        size_mb: 107,
        summary: "tells the other people on the call apart (Speaker 1, 2, …; up to 8), with either engine",
        engine: Engine::Nemo,
        repo: "nvidia/Nemotron-3-Diarization",
        role: Role::Speakers,
    },
];

/// The speaker detection model, if downloaded and runnable here.
#[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
pub fn speakers_model() -> Option<PathBuf> {
    CATALOG.iter().find(|m| m.role == Role::Speakers && runnable(m)).map(model_path).filter(|p| p.exists())
}

/// The engine for a model file (by its catalog entry; unknown files are Whisper's).
#[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
pub fn engine_of(path: &std::path::Path) -> Engine {
    let file = path.file_name().and_then(|f| f.to_str()).unwrap_or_default();
    CATALOG.iter().find(|m| m.file == file).map(|m| m.engine).unwrap_or(Engine::Whisper)
}

/// Can this computer run the model's engine? (Nemotron needs the runtime the app ships.)
pub fn runnable(m: &Model) -> bool {
    match m.engine {
        Engine::Whisper => true,
        #[cfg(feature = "transcribe")]
        Engine::Nemo => crate::nemo::available(),
        #[cfg(not(feature = "transcribe"))]
        Engine::Nemo => false,
    }
}

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
    /// This build can run its engine (Nemotron needs the runtime the app ships).
    runnable: bool,
}

#[derive(Serialize)]
struct ListReport {
    hardware: Hardware,
    recommended: &'static str,
    reason: String,
    models: Vec<ListEntry>,
}

fn report() -> Result<ListReport> {
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
            runnable: runnable(m),
        })
        .collect();
    Ok(ListReport { hardware: hw, recommended: rec, reason, models })
}

/// The catalog as `echoctl model list --json` gives it (the window's Settings → Transcription).
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn list_value() -> Result<serde_json::Value> {
    Ok(serde_json::to_value(report()?)?)
}

pub fn list(json: bool) -> Result<()> {
    let r = report()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
        return Ok(());
    }
    println!("recommended: {} ({})\n", r.recommended, r.reason);
    for e in &r.models {
        let flags = [(e.active, "active"), (e.downloaded, "downloaded"), (e.recommended, "recommended"), (!e.runnable, "not in this build")]
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
fn expected_sha256(m: &Model) -> Result<String> {
    let (repo, file) = (m.repo, m.file);
    let out = Command::new("curl").no_window()
        .args(["-fsSL", &format!("https://huggingface.co/api/models/{repo}/tree/main")])
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
        .ok_or_else(|| anyhow!("{file} not found in {repo}"))
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
    if !runnable(m) {
        bail!("{} needs the NeMo-Speech runtime, which this build of Echo doesn't include", m.name);
    }
    let expected = expected_sha256(m)?;

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
        let url = format!("https://huggingface.co/{}/resolve/main/{}", m.repo, m.file);
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

/// The first downloaded transcription model becomes the active one.
fn finish_download(m: &Model, dest: &std::path::Path) -> Result<()> {
    let mut cfg = Config::load()?;
    if cfg.model.is_none() && m.role == Role::Transcribe {
        cfg.model = Some(m.name.to_string());
        cfg.save()?;
        eprintln!("active model: {}", m.name);
    }
    println!("{}", dest.display());
    Ok(())
}

pub fn use_model(name: &str) -> Result<()> {
    let m = find(name)?;
    if m.role != Role::Transcribe {
        bail!("{} isn't a transcription model: it is used whenever it is downloaded", m.name);
    }
    if !model_path(m).exists() {
        bail!("{} is not downloaded — run `echoctl model download {}`", m.name, m.name);
    }
    if !runnable(m) {
        bail!("{} needs the NeMo-Speech runtime, which this build of Echo doesn't include", m.name);
    }
    let mut cfg = Config::load()?;
    cfg.model = Some(m.name.to_string());
    cfg.save()?;
    println!("active model: {}", m.name);
    if let Some(note) = missing_helper_note(m) {
        eprintln!("{note}");
    }
    Ok(())
}

/// For a model the helper detects the language for, while the helper isn't downloaded.
fn missing_helper_note(m: &Model) -> Option<String> {
    let helper = find(HELPER_MODEL).ok()?;
    (needs_helper(m) && !model_path(helper).exists()).then(|| {
        format!(
            "{} detects the language itself, less reliably — `echoctl model download {}` adds the helper that detects it",
            m.name, helper.name
        )
    })
}

/// Turbo q5_0 also detects the language when the active model isn't turbo (large-v3, small).
pub const HELPER_MODEL: &str = "large-v3-turbo-q5_0";

/// A Whisper model file that detects its own language: turbo.
pub fn detects_own_language(file: &str) -> bool {
    file.contains("large-v3-turbo")
}

/// The helper detects the language for `m`: a Whisper model other than turbo.
fn needs_helper(m: &Model) -> bool {
    m.engine == Engine::Whisper && !detects_own_language(m.file)
}

/// Why a downloaded model is used even when it isn't the active one: the helper detects the language while the
/// active model is a Whisper model other than turbo (large-v3, small).
pub fn helper_role(name: &str, active: Option<&str>) -> Option<&'static str> {
    let needed = active.and_then(|a| find(a).ok()).is_some_and(needs_helper);
    (name == HELPER_MODEL && needed).then_some("it detects the language for the active model")
}

/// `echoctl model remove`: delete a downloaded model (not the active one; the helper only with `force` while the
/// active model uses it).
pub fn remove(name: &str, force: bool) -> Result<()> {
    let m = find(name)?;
    let cfg = Config::load()?;
    if cfg.model.as_deref() == Some(m.name) {
        bail!("{} is the active model — switch with `echoctl model use <other>` first", m.name);
    }
    if let (Some(role), false) = (helper_role(m.name, cfg.model.as_deref()), force) {
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
    if let Some(m) = CATALOG.iter().find(|m| m.role == Role::Transcribe && model_path(m).exists() && runnable(m)) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_use_notes_a_missing_helper_only_for_a_model_that_needs_it() {
        let (_env, _) = crate::paths::testenv::folders("models-helper-note");
        let note = |n: &str| missing_helper_note(find(n).unwrap());
        for n in ["large-v3-q5_0", "small-q8_0"] {
            assert!(note(n).is_some_and(|t| t.contains(HELPER_MODEL)), "{n}");
        }
        for n in ["large-v3-turbo-q8_0", HELPER_MODEL, "nemotron-3.5-0.6b"] {
            assert_eq!(note(n), None, "{n}");
        }
        fs::create_dir_all(paths::models_dir()).unwrap();
        fs::write(model_path(find(HELPER_MODEL).unwrap()), b"x").unwrap();
        assert_eq!(note("large-v3-q5_0"), None, "the helper is downloaded");
    }

    #[test]
    fn the_helper_is_used_only_beside_a_whisper_model_other_than_turbo() {
        for active in ["large-v3-q5_0", "small-q8_0"] {
            assert!(helper_role(HELPER_MODEL, Some(active)).is_some(), "{active}");
        }
        for active in ["large-v3-turbo-q8_0", "large-v3-turbo-q5_0", "nemotron-3.5-0.6b"] {
            assert_eq!(helper_role(HELPER_MODEL, Some(active)), None, "{active}");
        }
        assert_eq!(helper_role(HELPER_MODEL, None), None);
        assert_eq!(helper_role("large-v3-turbo-q8_0", Some("large-v3-q5_0")), None);
    }

    #[test]
    fn the_helper_is_kept_from_removal_only_while_the_model_in_use_needs_it() {
        let (_env, _) = crate::paths::testenv::folders("models-remove-helper");
        let helper = || paths::models_dir().join(find(HELPER_MODEL).unwrap().file);
        let put = || {
            fs::create_dir_all(paths::models_dir()).unwrap();
            fs::write(helper(), b"x").unwrap();
        };
        let in_use = |m: Option<&str>| Config { model: m.map(str::to_owned), ..Default::default() }.save().unwrap();
        for active in ["large-v3-q5_0", "small-q8_0"] {
            in_use(Some(active));
            put();
            assert!(remove(HELPER_MODEL, false).is_err(), "{active}");
            assert!(helper().exists(), "{active}");
            remove(HELPER_MODEL, true).unwrap();
            assert!(!helper().exists(), "{active}: --force removes it");
        }
        for active in [Some("large-v3-turbo-q8_0"), Some("nemotron-3.5-0.6b"), None] {
            in_use(active);
            put();
            remove(HELPER_MODEL, false).unwrap();
            assert!(!helper().exists(), "{active:?}");
        }
        in_use(Some(HELPER_MODEL));
        put();
        assert!(remove(HELPER_MODEL, true).is_err(), "the model in use, even with --force");
        assert!(helper().exists());
    }
}

//! Speech model catalog, hardware-based recommendation and verified downloads.
//!
//! Only multilingual models are offered (calls are in several languages, each call in one). Each model names its engine:
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
        summary: "Whisper, deprecated (heavy on the computer): still the one for calls not in English until a lighter model replaces it",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "large-v3-turbo-q8_0",
        file: "ggml-large-v3-turbo-q8_0.bin",
        size_mb: 874,
        summary: "Whisper, deprecated (heavy on the computer): slightly more accurate than turbo q5_0",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "large-v3-q5_0",
        file: "ggml-large-v3-q5_0.bin",
        size_mb: 1081,
        summary: "Whisper, deprecated (heavy on the computer): several times slower than turbo",
        engine: Engine::Whisper,
        repo: WHISPER_REPO,
        role: Role::Transcribe,
    },
    Model {
        name: "small-q8_0",
        file: "ggml-small-q8_0.bin",
        size_mb: 264,
        summary: "Whisper, deprecated: lighter than turbo, noticeably more errors",
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
        name: "parakeet-tdt-0.6b-v3",
        file: "parakeet-tdt-0.6b-v3.q8_0.gguf",
        size_mb: 714,
        summary: "NVIDIA Parakeet v3: the best for English calls here, fast on a CPU; 25 European languages",
        engine: Engine::Nemo,
        repo: "nvidia/parakeet-tdt-0.6b-v3",
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
    let english = Config::load().is_ok_and(|c| crate::call_language::default_language(&c) == "en");
    if english && find(PARAKEET).is_ok_and(runnable) {
        return (PARAKEET, "light on the computer, the most accurate on English calls here".into());
    }
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
/// A download that stalls (under 1 KB/s for a minute, or no connection in 30 s) ends: the `.part` stays for a resume,
/// and the one-at-a-time download slot is free again instead of held until Echo restarts.
const STALL: [&str; 6] = ["--connect-timeout", "30", "--speed-limit", "1024", "--speed-time", if cfg!(test) { "2" } else { "60" }];

fn curl_with_progress(part: &PathBuf, url: &str, expected_bytes: u64) -> Result<std::process::ExitStatus> {
    use std::io::IsTerminal;
    let mut cmd = Command::new("curl").no_window();
    cmd.args(STALL).args(["-fL", "-C", "-", "-o"]).arg(part).arg(url);
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
            bail!(
                "download {} (re-run the same command to resume)",
                if status.code() == Some(28) { "stalled: no connection, or no data for a minute" } else { "failed" }
            );
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
    Ok(())
}

/// The model that replaces Whisper (deprecated): Parakeet v3.
pub const PARAKEET: &str = "parakeet-tdt-0.6b-v3";

/// The downloaded model a call in `lang` is transcribed with, measured on Echo's test calls (2026-11-09): English →
/// Parakeet v3, else Nemotron (about 15× less GPU than Whisper turbo and as accurate or better, noise included); another
/// language → the active model when it is Whisper, else Whisper turbo (Nemotron and Parakeet make twice turbo's errors
/// on noisy Russian). None: nothing better downloaded, the active model.
#[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
pub fn for_language(lang: &str, active: Option<&str>) -> Option<&'static Model> {
    let ready = |n: &&str| find(n).ok().filter(|m| model_path(m).exists() && runnable(m));
    if lang == "en" {
        return [PARAKEET, "nemotron-3.5-0.6b"].iter().find_map(ready);
    }
    if active.and_then(|a| find(a).ok()).is_some_and(|m| m.engine == Engine::Whisper) {
        return None;
    }
    ["large-v3-turbo-q8_0", "large-v3-turbo-q5_0", "large-v3-q5_0", "small-q8_0"].iter().find_map(ready)
}

/// The model a call in `lang` is transcribed with: `for_language`'s, else the active one (`active`, when downloaded).
#[cfg_attr(not(feature = "transcribe"), allow(dead_code))]
pub fn for_call(lang: &str, active: Option<&str>) -> Option<&'static Model> {
    for_language(lang, active).or_else(|| active.and_then(|a| find(a).ok()).filter(|m| model_path(m).exists()))
}

/// Which calls a downloaded model other than the active one transcribes (`for_language`), for the model lists.
#[cfg_attr(not(any(feature = "transcribe", feature = "tray")), allow(dead_code))]
pub fn used_for(name: &str, active: Option<&str>) -> Option<&'static str> {
    if active == Some(name) {
        return None;
    }
    if for_language("en", active).is_some_and(|m| m.name == name) {
        Some("English calls")
    } else if for_language("ru", active).is_some_and(|m| m.name == name) {
        Some("calls in other languages than English")
    } else {
        None
    }
}

/// `echoctl model remove`: delete a downloaded model (not the active one).
pub fn remove(name: &str) -> Result<()> {
    let m = find(name)?;
    let cfg = Config::load()?;
    if cfg.model.as_deref() == Some(m.name) {
        bail!("{} is the active model — switch with `echoctl model use <other>` first", m.name);
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

static DOWNLOADING: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);

/// The model the app is downloading, with its progress in percent.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn downloading() -> Option<(&'static str, u8)> {
    let name = (*DOWNLOADING.lock().unwrap_or_else(|e| e.into_inner()))?;
    Some((name, PROGRESS.load(std::sync::atomic::Ordering::Relaxed)))
}

/// Download a model off the UI thread (one at a time); notifies when done.
pub fn download_in_background(name: &'static str) {
    download_then(name, move || crate::app::notify("Speech model ready", &format!("{name} is downloaded.")));
}

/// Make `name` the active model, downloaded first (off the UI thread) when it isn't on disk yet.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn switch_in_background(name: &'static str) {
    let use_it = move || match use_model(name) {
        Ok(()) => crate::app::notify("Speech model switched", &format!("Calls are transcribed with {name} from now on.")),
        Err(e) => crate::app::notify("Couldn't switch the speech model", &format!("{e:#}")),
    };
    match find(name) {
        Ok(m) if model_path(m).exists() => use_it(),
        _ => download_then(name, use_it),
    }
}

fn download_then(name: &'static str, then: impl FnOnce() + Send + 'static) {
    {
        let mut d = DOWNLOADING.lock().unwrap_or_else(|e| e.into_inner());
        if d.is_some() {
            return;
        }
        *d = Some(name);
    }
    PROGRESS.store(0, std::sync::atomic::Ordering::Relaxed);
    std::thread::spawn(move || {
        let r = download(name);
        *DOWNLOADING.lock().unwrap_or_else(|e| e.into_inner()) = None;
        match r {
            Ok(()) => then(),
            Err(e) => crate::app::notify("Model download failed", &format!("{e:#}")),
        }
    });
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
    fn a_call_gets_the_downloaded_model_best_at_its_language() {
        let (_env, _) = crate::paths::testenv::folders("models-for-language");
        fs::create_dir_all(paths::models_dir()).unwrap();
        let put = |n: &str| fs::write(model_path(find(n).unwrap()), b"x").unwrap();
        let pick = |lang: &str, active: &str| for_language(lang, Some(active)).map(|m| m.name);
        assert_eq!(pick("en", "large-v3-turbo-q5_0"), None, "nothing better downloaded: the active model");
        assert_eq!(pick("ru", "nemotron-3.5-0.6b"), None);
        put("large-v3-turbo-q5_0");
        assert_eq!(pick("ru", "nemotron-3.5-0.6b"), Some("large-v3-turbo-q5_0"), "Russian: Whisper over a CPU model");
        assert_eq!(pick("ru", "large-v3-q5_0"), None, "Russian on a chosen Whisper model: kept");
        put("large-v3-turbo-q8_0");
        assert_eq!(pick("uk", "nemotron-3.5-0.6b"), Some("large-v3-turbo-q8_0"));
        if runnable(find("nemotron-3.5-0.6b").unwrap()) {
            put("nemotron-3.5-0.6b");
            assert_eq!(pick("en", "large-v3-turbo-q5_0"), Some("nemotron-3.5-0.6b"), "English: a CPU model over Whisper");
            put("parakeet-tdt-0.6b-v3");
            assert_eq!(pick("en", "nemotron-3.5-0.6b"), Some("parakeet-tdt-0.6b-v3"), "English: Parakeet first");
            assert_eq!(used_for("parakeet-tdt-0.6b-v3", Some("large-v3-turbo-q5_0")), Some("English calls"));
            assert_eq!(used_for("large-v3-turbo-q8_0", Some("parakeet-tdt-0.6b-v3")), Some("calls in other languages than English"));
        } else {
            assert_eq!(pick("en", "large-v3-turbo-q5_0"), None, "no NeMo-Speech runtime in this build");
        }
        assert_eq!(used_for("large-v3-turbo-q5_0", Some("large-v3-turbo-q5_0")), None, "the active one");
        let call = |lang: &str, active: &str| for_call(lang, Some(active)).map(|m| m.name);
        assert_eq!(call("ru", "large-v3-turbo-q5_0"), Some("large-v3-turbo-q5_0"), "nothing better: the active one, by name");
        assert_eq!(call("ru", "large-v3-q5_0"), None, "the active one isn't downloaded");
    }

    #[test]
    fn a_download_that_stops_sending_ends_and_keeps_its_part() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model.bin", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            let _ = c.read(&mut [0u8; 4096]);
            let _ = c.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\n");
            let _ = c.write_all(&[7u8; 100]);
            std::thread::sleep(std::time::Duration::from_secs(30)); // a stalled connection
        });
        let dir = std::env::temp_dir().join(format!("echo-stall-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let part = dir.join("model.bin.part");
        let t = std::time::Instant::now();
        let status = curl_with_progress(&part, &url, 1_000_000).unwrap();
        assert_eq!(status.code(), Some(28), "curl's 'too slow'");
        assert!(t.elapsed() < std::time::Duration::from_secs(20), "it ended, not hung");
        assert_eq!(fs::metadata(&part).unwrap().len(), 100, "what arrived stays for a resume");
        let _ = fs::remove_dir_all(&dir);
    }

}

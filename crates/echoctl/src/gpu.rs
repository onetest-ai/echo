//! Whether whisper.cpp may use the GPU. macOS: Metal, always. Windows: a Vulkan driver can
//! take the whole process down inside whisper.cpp (seen on a laptop without a discrete GPU:
//! 0xC0000409 the moment a recording loaded the model), and nothing in-process survives that.
//! So the GPU is tried once per Echo version in a child process (`echoctl gpu-check`: load the
//! model on the GPU, decode a second of silence); if the child fails or dies, whisper runs on
//! the CPU. The answer is kept in `<config dir>/gpu.json`; `config.use_gpu` overrides it.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[cfg(windows)]
use crate::app::NoWindow;
use crate::config::Config;
use crate::{models, paths};

const FILE: &str = "gpu.json";
/// The child loads the model once; on a slow disk or CPU that can take a while.
const CHECK_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Serialize, Deserialize)]
struct Checked {
    version: String,
    ok: bool,
    detail: String,
}

/// This process's answer, once the check has run (not before a model exists).
static ANSWER: Mutex<Option<bool>> = Mutex::new(None);
/// A re-test (Health → Re-test GPU) is running.
static TESTING: AtomicBool = AtomicBool::new(false);

fn cached() -> Option<Checked> {
    std::fs::read(paths::config_dir().join(FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<Checked>(&b).ok())
        .filter(|c| c.version == env!("CARGO_PKG_VERSION"))
}

/// What whisper.cpp runs on, for the Health menu. Never runs the check itself.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn status() -> String {
    if let Some(v) = Config::load().ok().and_then(|c| c.use_gpu) {
        return format!("{} (set in config: use_gpu)", if v { "GPU" } else { "CPU" });
    }
    if cfg!(target_os = "macos") {
        return "GPU (Metal)".into();
    }
    if TESTING.load(Ordering::SeqCst) {
        return "testing the GPU…".into();
    }
    match cached() {
        Some(c) if c.ok => "GPU (Vulkan)".into(),
        Some(c) => format!("CPU — GPU test failed ({})", c.detail),
        None => "not tested yet (CPU until it is)".into(),
    }
}

/// The engine of the active model and what it runs on: "whisper.cpp, GPU (Metal)",
/// "Nemotron, CPU" (Health menu, the transcription server's hello).
pub fn engine_status() -> String {
    let nemo = models::active_model_path().is_ok_and(|p| models::engine_of(&p) == models::Engine::Nemo);
    if !nemo {
        return format!("whisper.cpp, {}", status());
    }
    // Nemotron: Metal on macOS unless use_gpu is off; the CPU runtime on Windows.
    let gpu = cfg!(target_os = "macos") && Config::load().ok().and_then(|c| c.use_gpu) != Some(false);
    format!("Nemotron, {}", if gpu { "GPU (Metal)" } else { "CPU" })
}

/// Health → Re-test GPU (after a driver update): forget the answer and test again. Returns the
/// new answer.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub fn retest() -> bool {
    TESTING.store(true, Ordering::SeqCst);
    let _ = std::fs::remove_file(paths::config_dir().join(FILE));
    *ANSWER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let ok = allowed();
    TESTING.store(false, Ordering::SeqCst);
    ok
}

/// Use the GPU for whisper.cpp?
pub fn allowed() -> bool {
    if let Some(v) = Config::load().ok().and_then(|c| c.use_gpu) {
        return v;
    }
    if cfg!(not(windows)) {
        return true;
    }
    let mut answer = ANSWER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(a) = *answer {
        return a;
    }
    let file = paths::config_dir().join(FILE);
    let version = env!("CARGO_PKG_VERSION");
    if let Some(c) = cached() {
        *answer = Some(c.ok);
        return c.ok;
    }
    // Nothing to check with yet: CPU for now, check again once a Whisper model is active (the
    // check is whisper.cpp's; Nemotron runs on the CPU on Windows).
    if !models::active_model_path().is_ok_and(|p| p.exists() && models::engine_of(&p) == models::Engine::Whisper) {
        return false;
    }
    let (ok, detail) = run_child();
    eprintln!("GPU check: {} ({detail})", if ok { "GPU" } else { "CPU only" });
    let _ = std::fs::create_dir_all(paths::config_dir());
    let _ = std::fs::write(&file, serde_json::to_vec_pretty(&Checked { version: version.into(), ok, detail }).unwrap_or_default());
    *answer = Some(ok);
    ok
}

fn run_child() -> (bool, String) {
    let exe = std::env::current_exe().unwrap_or_else(|_| "echoctl".into());
    let cmd = Command::new(exe);
    #[cfg(windows)]
    let cmd = cmd.no_window();
    let mut cmd = cmd;
    let child = cmd.arg("gpu-check").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return (false, format!("could not start the check: {e}")),
    };
    let t = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(s)) if s.success() => return (true, format!("checked in {:.0}s", t.elapsed().as_secs_f64())),
            Ok(Some(s)) => return (false, format!("the check failed: {s}")),
            Ok(None) if t.elapsed() > CHECK_TIMEOUT => {
                let _ = child.kill();
                return (false, "the check did not finish".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => return (false, format!("the check failed: {e}")),
        }
    }
}

/// `echoctl gpu-check` (the child): load the active model on the GPU and decode 1 s of silence.
pub fn check() -> Result<()> {
    let model = models::active_model_path()?;
    if !model.exists() || models::engine_of(&model) != models::Engine::Whisper {
        bail!("no Whisper model to check with");
    }
    whisper_rs::install_logging_hooks();
    let ctx = WhisperContext::new_with_params(&model, WhisperContextParameters { use_gpu: true, ..Default::default() })?;
    let mut state = ctx.create_state()?;
    let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    p.set_print_progress(false);
    p.set_print_realtime(false);
    state.full(p, &vec![0.0f32; crate::transcribe::RATE])?;
    println!("GPU ok");
    Ok(())
}

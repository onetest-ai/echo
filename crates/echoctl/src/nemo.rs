//! NVIDIA Nemotron speech recognition through NeMo-Speech.cpp's C ABI (`nemo_speech/asr.h`):
//! a second engine next to whisper.cpp, picked by choosing a Nemotron model (`models.rs`).
//!
//! The library is loaded at runtime (libloading), not linked: builds and checks don't need it,
//! and its own ggml (shared libraries) stays apart from whisper.cpp's (linked into echoctl).
//! Where it is: `ECHOCTL_NEMO_DIR`, else the app's own copy (macOS
//! `Echo.app/Contents/Frameworks/nemo-speech`, Windows `<app folder>\nemo-speech`, put there by
//! the packaging scripts from `scripts/fetch-nemo-speech.sh`), else `<config dir>/nemo-speech`.
//!
//! One recognizer per model; NVIDIA documents it as safe to call from several threads, so both
//! tracks share it. Each utterance is one offline `recognize` call with word times on.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};

use crate::paths;

// ---- The C ABI (v1), mirrored from nemo_speech/asr.h ----
// Structs start with `size`; the library defaults fields a shorter struct leaves out, so only
// what is used here is declared (and must match the header's order exactly).

#[repr(C)]
struct BackendConfig {
    size: usize,
    gpu: i32, // -1 = CPU
}

#[repr(C)]
struct ModelConfig {
    size: usize,
    path: *const c_char,
    name: *const c_char,
}

#[repr(C)]
struct RecognizerConfig {
    size: usize,
    backend: *const BackendConfig,
    model: *const ModelConfig,
    streaming: *const c_void,
    decoder: *const c_void,
    vad: *const c_void,
    endpointing: *const c_void,
    postproc: *const c_void,
    diar: *const c_void,
    batching: *const c_void,
}

#[repr(C)]
struct SpeechContext {
    size: usize,
    phrases: *const *const c_char,
    phrase_count: usize,
    boost: f32,
}

#[repr(C)]
struct RecognitionOptions {
    size: usize,
    request_id: *const c_char,
    language_code: *const c_char,
    interim_results: bool,
    enable_word_time_offsets: bool,
    enable_automatic_punctuation: bool,
    verbatim_transcripts: bool,
    profanity_filter: bool,
    stop_history_eou_ms: i32,
    speech_contexts: *const SpeechContext,
    speech_context_count: usize,
    max_alternatives: i32,
    enable_speaker_diarization: bool,
    max_speaker_count: i32,
}

#[repr(C)]
struct DiarModelConfig {
    size: usize,
    model_path: *const c_char,
    gpu: i32,
    preset: *const c_char,
    chunk_frames: i32,
    right_context_frames: i32,
    left_context_frames: i32, // < 0 = the preset's
    fifo_frames: i32,
    spkcache_frames: i32,
    update_period_frames: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DiarSegment {
    start_time: f64,
    end_time: f64,
    speaker: i32,
}

type Recognizer = c_void;
type DiarModel = c_void;
type DiarStream = c_void;
type AsrResult = c_void;
const OK: i32 = 0;

struct Api {
    _lib: libloading::Library,
    create: unsafe extern "C" fn(*const RecognizerConfig, *mut *mut Recognizer) -> i32,
    destroy: unsafe extern "C" fn(*mut Recognizer),
    options_default: unsafe extern "C" fn() -> RecognitionOptions,
    recognize: unsafe extern "C" fn(*mut Recognizer, *const RecognitionOptions, *const f32, usize, i32, *mut *mut AsrResult) -> i32,
    alternatives: unsafe extern "C" fn(*const AsrResult) -> usize,
    word_count: unsafe extern "C" fn(*const AsrResult, usize) -> usize,
    word_text: unsafe extern "C" fn(*const AsrResult, usize, usize) -> *const c_char,
    word_start: unsafe extern "C" fn(*const AsrResult, usize, usize) -> i32,
    word_end: unsafe extern "C" fn(*const AsrResult, usize, usize) -> i32,
    result_destroy: unsafe extern "C" fn(*mut AsrResult),
    last_error: unsafe extern "C" fn() -> *const c_char,
    version: unsafe extern "C" fn() -> *const c_char,
    diar_create: unsafe extern "C" fn(*const DiarModelConfig, *mut *mut DiarModel) -> i32,
    diar_destroy: unsafe extern "C" fn(*mut DiarModel),
    diar_open: unsafe extern "C" fn(*mut DiarModel, *mut *mut DiarStream) -> i32,
    diar_push: unsafe extern "C" fn(*mut DiarStream, *const f32, usize, i32) -> i32,
    diar_finish: unsafe extern "C" fn(*mut DiarStream) -> i32,
    diar_close: unsafe extern "C" fn(*mut DiarStream),
    diar_segments: unsafe extern "C" fn(*const DiarStream, *const c_void, *mut DiarSegment, usize, *mut usize) -> i32,
}

#[cfg(target_os = "macos")]
const LIB: &str = "libnemo_speech_asr_c.1.dylib";
#[cfg(windows)]
const LIB: &str = "nemo_speech_asr_c.dll";
#[cfg(not(any(target_os = "macos", windows)))]
const LIB: &str = "libnemo_speech_asr_c.so.1";

/// Folders that may hold the runtime, in order.
fn candidates() -> Vec<PathBuf> {
    let mut dirs = vec![];
    if let Some(d) = std::env::var_os("ECHOCTL_NEMO_DIR") {
        dirs.push(PathBuf::from(d));
    }
    if let Some(exe_dir) = paths::exe_dir() {
        if cfg!(target_os = "macos") {
            dirs.push(exe_dir.join("../Frameworks/nemo-speech"));
        }
        dirs.push(exe_dir.join("nemo-speech"));
    }
    dirs.push(paths::config_dir().join("nemo-speech"));
    dirs
}

/// The folder with the runtime, if this computer has one.
pub fn runtime_dir() -> Option<PathBuf> {
    candidates().into_iter().find(|d| d.join(LIB).exists())
}

/// Can Nemotron models run here?
pub fn available() -> bool {
    runtime_dir().is_some()
}

fn api() -> Result<&'static Api> {
    static API: OnceLock<std::result::Result<Api, String>> = OnceLock::new();
    API.get_or_init(|| load_api().map_err(|e| format!("{e:#}"))).as_ref().map_err(|e| anyhow!("{e}"))
}

fn load_api() -> Result<Api> {
    let dir = runtime_dir().ok_or_else(|| anyhow!("the NeMo-Speech runtime is not installed (looked in {})",
        candidates().iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ")))?;
    let path = dir.join(LIB);
    // Windows: its ggml and runtime DLLs sit next to it; let the loader look there.
    #[cfg(windows)]
    let lib: libloading::Library = unsafe {
        libloading::os::windows::Library::load_with_flags(&path, libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH)
    }
    .with_context(|| format!("loading {}", path.display()))?
    .into();
    #[cfg(not(windows))]
    let lib = unsafe { libloading::Library::new(&path) }.with_context(|| format!("loading {}", path.display()))?;
    unsafe {
        macro_rules! sym {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes()).with_context(|| format!("{} has no {}", path.display(), $name))?
            };
        }
        Ok(Api {
            create: sym!("nemo_speech_asr_create"),
            destroy: sym!("nemo_speech_asr_destroy"),
            options_default: sym!("nemo_speech_asr_recognition_options_default"),
            recognize: sym!("nemo_speech_asr_recognize_f32"),
            alternatives: sym!("nemo_speech_asr_result_alternative_count"),
            word_count: sym!("nemo_speech_asr_result_word_count"),
            word_text: sym!("nemo_speech_asr_result_word_text"),
            word_start: sym!("nemo_speech_asr_result_word_start_time"),
            word_end: sym!("nemo_speech_asr_result_word_end_time"),
            result_destroy: sym!("nemo_speech_asr_result_destroy"),
            last_error: sym!("nemo_speech_asr_last_error"),
            version: sym!("nemo_speech_asr_version"),
            diar_create: sym!("nemo_speech_diar_create"),
            diar_destroy: sym!("nemo_speech_diar_destroy"),
            diar_open: sym!("nemo_speech_diar_stream_open"),
            diar_push: sym!("nemo_speech_diar_stream_push_f32"),
            diar_finish: sym!("nemo_speech_diar_stream_finish"),
            diar_close: sym!("nemo_speech_diar_stream_close"),
            diar_segments: sym!("nemo_speech_diar_segments"),
            _lib: lib,
        })
    }
}

fn last_error(api: &Api) -> String {
    let p = unsafe { (api.last_error)() };
    if p.is_null() {
        "unknown error".into()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

/// The runtime's version, for logs and Health.
pub fn version() -> Option<String> {
    let api = api().ok()?;
    let p = unsafe { (api.version)() };
    (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
}

/// One recognized word, seconds from the start of the audio.
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

/// What one `recognize` call returns.
pub struct Recognized {
    pub words: Vec<Word>,
}

/// A loaded Nemotron model.
pub struct Model {
    api: &'static Api,
    rec: *mut Recognizer,
}

// The recognizer is documented as thread-safe ("One ASR recognizer can serve independent calls
// from multiple threads"); the handle is only freed in Drop.
unsafe impl Send for Model {}
unsafe impl Sync for Model {}

impl Model {
    /// Load `path` (a Nemotron GGUF) on the GPU (`gpu`) or the CPU.
    pub fn load(path: &Path, gpu: bool) -> Result<Model> {
        let api = api()?;
        let cpath = CString::new(path.to_string_lossy().as_bytes())?;
        let backend = BackendConfig { size: size_of::<BackendConfig>(), gpu: if gpu { 0 } else { -1 } };
        let model = ModelConfig { size: size_of::<ModelConfig>(), path: cpath.as_ptr(), name: std::ptr::null() };
        let n = std::ptr::null();
        let cfg = RecognizerConfig {
            size: size_of::<RecognizerConfig>(),
            backend: &backend,
            model: &model,
            streaming: n,
            decoder: n,
            vad: n,
            endpointing: n,
            postproc: n,
            diar: n,
            batching: n,
        };
        let mut rec = std::ptr::null_mut();
        if unsafe { (api.create)(&cfg, &mut rec) } != OK || rec.is_null() {
            bail!("loading {}: {}", path.display(), last_error(api));
        }
        Ok(Model { api, rec })
    }

    /// Recognize one utterance (16 kHz mono). `locale`: `uk-UA` etc., or None for the model's
    /// own detection. `phrases`: words to boost (people, products: the vocabulary).
    pub fn recognize(&self, audio: &[f32], locale: Option<&str>, phrases: &[String], boost: f32) -> Result<Recognized> {
        let api = self.api;
        let lang = CString::new(locale.unwrap_or("auto"))?;
        let owned: Vec<CString> = phrases.iter().filter_map(|p| CString::new(p.as_str()).ok()).collect();
        let ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        let ctx = SpeechContext { size: size_of::<SpeechContext>(), phrases: ptrs.as_ptr(), phrase_count: ptrs.len(), boost };
        let mut opts = unsafe { (api.options_default)() };
        opts.language_code = lang.as_ptr();
        opts.enable_word_time_offsets = true;
        opts.enable_automatic_punctuation = true;
        if !ptrs.is_empty() {
            opts.speech_contexts = &ctx;
            opts.speech_context_count = 1;
        }
        let mut res = std::ptr::null_mut();
        if unsafe { (api.recognize)(self.rec, &opts, audio.as_ptr(), audio.len(), crate::transcribe::RATE as i32, &mut res) } != OK {
            bail!("Nemotron: {}", last_error(api));
        }
        if res.is_null() {
            return Ok(Recognized { words: vec![] });
        }
        let text = |p: *const c_char| if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() };
        let mut out = Recognized { words: vec![] };
        unsafe {
            if (api.alternatives)(res) > 0 {
                for i in 0..(api.word_count)(res, 0) {
                    out.words.push(Word {
                        text: text((api.word_text)(res, 0, i)),
                        start: (api.word_start)(res, 0, i) as f64 / 1000.0,
                        end: (api.word_end)(res, 0, i) as f64 / 1000.0,
                    });
                }
            }
            (api.result_destroy)(res);
        }
        Ok(out)
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        unsafe { (self.api.destroy)(self.rec) }
    }
}

/// Who speaks when: a Sortformer model (Nemotron 3 Diarization, up to 8 speakers) over one
/// track. Its "v3-offline" geometry on the streaming state machine: offline accuracy, any
/// length of audio, fed in pieces.
pub struct Diarizer {
    api: &'static Api,
    model: *mut DiarModel,
}

unsafe impl Send for Diarizer {}

/// (start, end, speaker) — seconds, speaker 1-based.
pub type Turn = (f64, f64, u32);

impl Diarizer {
    pub fn load(path: &Path, gpu: bool) -> Result<Diarizer> {
        let api = api()?;
        let cpath = CString::new(path.to_string_lossy().as_bytes())?;
        let preset = CString::new("v3-offline")?;
        let cfg = DiarModelConfig {
            size: size_of::<DiarModelConfig>(),
            model_path: cpath.as_ptr(),
            gpu: if gpu { 0 } else { -1 },
            preset: preset.as_ptr(),
            chunk_frames: 0,
            right_context_frames: 0,
            left_context_frames: -1,
            fifo_frames: 0,
            spkcache_frames: 0,
            update_period_frames: 0,
        };
        let mut model = std::ptr::null_mut();
        if unsafe { (api.diar_create)(&cfg, &mut model) } != OK || model.is_null() {
            bail!("loading {}: {}", path.display(), last_error(api));
        }
        Ok(Diarizer { api, model })
    }

    /// Diarize audio delivered by `next` in pieces (16 kHz mono; an empty piece ends it).
    pub fn run(&self, mut next: impl FnMut() -> Result<Vec<f32>>) -> Result<Vec<Turn>> {
        let api = self.api;
        let mut stream = std::ptr::null_mut();
        if unsafe { (api.diar_open)(self.model, &mut stream) } != OK || stream.is_null() {
            bail!("speaker detection: {}", last_error(api));
        }
        struct Close<'a>(&'a Api, *mut DiarStream);
        impl Drop for Close<'_> {
            fn drop(&mut self) {
                unsafe { (self.0.diar_close)(self.1) }
            }
        }
        let _close = Close(api, stream);
        loop {
            let piece = next()?;
            if piece.is_empty() {
                break;
            }
            if unsafe { (api.diar_push)(stream, piece.as_ptr(), piece.len(), crate::transcribe::RATE as i32) } != OK {
                bail!("speaker detection: {}", last_error(api));
            }
        }
        if unsafe { (api.diar_finish)(stream) } != OK {
            bail!("speaker detection: {}", last_error(api));
        }
        let mut count = 0usize;
        if unsafe { (api.diar_segments)(stream, std::ptr::null(), std::ptr::null_mut(), 0, &mut count) } != OK {
            bail!("speaker detection: {}", last_error(api));
        }
        let mut segs = vec![DiarSegment { start_time: 0.0, end_time: 0.0, speaker: 0 }; count];
        if count > 0 && unsafe { (api.diar_segments)(stream, std::ptr::null(), segs.as_mut_ptr(), count, &mut count) } != OK {
            bail!("speaker detection: {}", last_error(api));
        }
        segs.truncate(count);
        Ok(segs.into_iter().filter(|s| s.speaker > 0).map(|s| (s.start_time, s.end_time, s.speaker as u32)).collect())
    }
}

impl Drop for Diarizer {
    fn drop(&mut self) {
        unsafe { (self.api.diar_destroy)(self.model) }
    }
}

/// Our two-letter language codes (Whisper's) → the model's locales. Only the languages it
/// transcribes well ("transcription-ready" on the model card) and the broad-coverage ones.
pub fn locale(code: &str) -> Option<&'static str> {
    Some(match code {
        "en" => "en-US",
        "es" => "es-ES",
        "fr" => "fr-FR",
        "it" => "it-IT",
        "pt" => "pt-BR",
        "nl" => "nl-NL",
        "de" => "de-DE",
        "tr" => "tr-TR",
        "ru" => "ru-RU",
        "ar" => "ar-AR",
        "hi" => "hi-IN",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        "vi" => "vi-VN",
        "uk" => "uk-UA",
        "pl" => "pl-PL",
        "sv" => "sv-SE",
        "cs" => "cs-CZ",
        "no" | "nb" => "nb-NO",
        "da" => "da-DK",
        "bg" => "bg-BG",
        "fi" => "fi-FI",
        "hr" => "hr-HR",
        "sk" => "sk-SK",
        "zh" => "zh-CN",
        "hu" => "hu-HU",
        "ro" => "ro-RO",
        "et" => "et-EE",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_round_trip() {
        for (c, l) in [("en", "en-US"), ("uk", "uk-UA"), ("ru", "ru-RU"), ("no", "nb-NO")] {
            assert_eq!(locale(c), Some(l));
        }
        assert_eq!(locale("xx"), None);
    }
}

//! On-device transcription of a recorded call with whisper.cpp.
//!
//! Each track is transcribed separately, which gives speaker attribution for free ("Me" =
//! mic, "Remote" = the app). Tracks are cut into utterances at pauses, and the language is
//! detected *per utterance* restricted to the workspace's `languages` list — Whisper's
//! default (one language per 30 s window, any of 99) mislabels Ukrainian as Russian and
//! translates code-switched speech instead of transcribing it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use crate::audio::{downmix, Resampler};
use crate::config::Config;
use crate::meeting::MeetingContext;
use crate::models;

pub(crate) const RATE: usize = 16_000;
/// Whisper's window. Utterances are kept shorter than this.
const MAX_UTTERANCE_S: f64 = 25.0;
/// Below this, language detection is unreliable; reuse the track's previous language.
const SHORT_UTTERANCE_S: f64 = 1.5;
/// Hinted utterances from this length are checked against an unhinted
/// decode: the hint is kept only if the text length stays within
/// GUARD_MIN_RATIO × … GUARD_MAX_RATIO × + GUARD_SLACK chars of the plain text — otherwise
/// the hint made Whisper add (listed names in silence) or drop words.
pub(crate) const GUARD_MIN_S: f64 = 1.0;
const GUARD_MIN_RATIO: f64 = 0.6;
const GUARD_MAX_RATIO: f64 = 1.8;
const GUARD_SLACK: f64 = 80.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// "me" | "remote"
    pub speaker: String,
    pub start: f64,
    pub end: f64,
    pub lang: String,
    pub text: String,
    /// Who it is, when known (remote: the other person on a 1:1 invite, or a detected speaker:
    /// "Speaker 2" until named, `speakers.rs`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Remote speaker number from speaker detection (1-based), when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<u32>,
    /// Word times (absolute), one per word of `text`: lets speaker detection split a sentence
    /// two people share. In memory only.
    #[serde(skip)]
    pub words: Vec<[f64; 2]>,
}

impl Segment {
    /// A decoded sentence of an utterance that starts `offset` seconds into the track.
    pub(crate) fn from_piece(speaker: &str, offset: f64, lang: &str, p: Piece, name: Option<&str>) -> Segment {
        Segment {
            speaker: speaker.into(),
            start: offset + p.start,
            end: offset + p.end,
            lang: lang.into(),
            text: p.text,
            name: name.map(str::to_owned),
            speaker_id: None,
            words: p.words.iter().map(|[a, b]| [offset + a, offset + b]).collect(),
        }
    }
}

pub use crate::calls::resolve_call;

/// Read any WAV (old recordings are 24/48 kHz float) as 16 kHz mono f32.
pub(crate) fn load_16k(path: &Path) -> Result<Vec<f32>> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("opening {}", path.display()))?;
    let spec = r.spec();
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 / scale)).collect::<Result<_, _>>()?
        }
    };
    let mono = downmix(&raw, spec.channels as usize);
    Ok(Resampler::new(spec.sample_rate, RATE as u32).process(&mono))
}

/// Energy-based voice activity: (start, end) sample ranges of speech.
pub(crate) fn utterances(audio: &[f32]) -> Vec<(usize, usize)> {
    const FRAME: usize = RATE * 30 / 1000;
    let db: Vec<f32> = audio
        .chunks(FRAME)
        .map(|f| {
            let rms = (f.iter().map(|s| s * s).sum::<f32>() / f.len() as f32).sqrt();
            20.0 * rms.max(1e-9).log10()
        })
        .collect();
    if db.is_empty() {
        return vec![];
    }
    // Adaptive threshold: 12 dB above the noise floor (20th percentile), kept within
    // [-55, -35] dBFS so near-silent tracks and wall-to-wall speech both work.
    let mut sorted = db.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let floor = sorted[sorted.len() / 5];
    let thresh = (floor + 12.0).clamp(-55.0, -35.0);

    // In 30 ms frames: an utterance ends after 800 ms of silence, is padded by ~250 ms on both
    // sides (word onsets/tails are quiet), and needs ≥ 200 ms of speech.
    let (hangover, pad, min_len) = (800 / 30, 256 / 30, 200 / 30);
    let mut spans: Vec<(usize, usize)> = vec![];
    let mut i = 0;
    while i < db.len() {
        if db[i] < thresh {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i;
        let mut quiet = 0;
        while i < db.len() && quiet <= hangover {
            if db[i] >= thresh {
                end = i;
                quiet = 0;
            } else {
                quiet += 1;
            }
            i += 1;
        }
        if end + 1 - start >= min_len {
            spans.push((start.saturating_sub(pad), (end + 1 + pad).min(db.len())));
        }
    }
    // Split long spans at their quietest frame so each fits Whisper's window.
    let max_frames = (MAX_UTTERANCE_S * 1000.0 / 30.0) as usize;
    let mut out = vec![];
    let mut stack: Vec<(usize, usize)> = spans.into_iter().rev().collect();
    while let Some((s, e)) = stack.pop() {
        if e - s <= max_frames {
            out.push((s * FRAME, (e * FRAME).min(audio.len())));
            continue;
        }
        let (lo, hi) = (s + max_frames / 3, s + max_frames);
        let cut = (lo..hi).min_by(|a, b| db[*a].total_cmp(&db[*b])).unwrap_or(hi);
        stack.push((cut, e));
        stack.push((s, cut));
    }
    out
}

/// Phrases Whisper inserts from its training subtitles, removed wherever they appear in a segment
/// (they also show up glued to real speech, e.g. "Subtitles by the Amara.org community How …").
const INLINE_HALLUCINATIONS: &[&str] = &[
    "subtitles by the amara.org community",
    "subtítulos realizados por la comunidad de amara.org",
    "субтитры сделал dimatorzok",
    "субтитры создавал dimatorzok",
    "редактор субтитров а.семкин корректор а.егорова",
    "продолжение следует...",
    "thanks for watching!",
    "thank you for watching.",
];

/// Remove inline hallucinated phrases and ALL-CAPS sound captions ("BELL RINGS", "MUSIC").
pub(crate) fn clean_text(text: &str) -> String {
    let mut t = text.to_string();
    for phrase in INLINE_HALLUCINATIONS {
        while let Some(i) = t.to_lowercase().find(phrase) {
            // byte offsets match: the phrases are lowercase-stable (ASCII / Cyrillic)
            t.replace_range(i..i + phrase.len(), "");
        }
    }
    // Sound captions, e.g. "BELL RINGS", "MUSIC PLAYING", "APPLAUSE": 2–3 all-caps words whose
    // last word ends in S/ING, or a lone caption word. Real acronym pairs ("IPAM SSO") stay.
    let words: Vec<&str> = t.split_whitespace().collect();
    let bare = |w: &str| w.chars().filter(|c| c.is_alphabetic()).collect::<String>();
    let caps = |w: &str| {
        let b = bare(w);
        b.chars().count() >= 3 && b.chars().all(|c| c.is_ascii_uppercase())
    };
    const LONE: &[&str] = &["MUSIC", "APPLAUSE", "LAUGHTER", "SILENCE", "INAUDIBLE"];
    let mut keep = vec![true; words.len()];
    let mut i = 0;
    while i < words.len() {
        let mut j = i;
        while j < words.len() && caps(words[j]) {
            j += 1;
        }
        let run = &words[i..j];
        let last = run.last().map(|w| bare(w)).unwrap_or_default();
        let caption = (2..=3).contains(&run.len()) && (last.ends_with('S') || last.ends_with("ING"))
            || (run.len() == 1 && LONE.contains(&last.as_str()));
        if caption {
            keep[i..j].iter_mut().for_each(|k| *k = false);
        }
        i = j.max(i + 1);
    }
    words
        .iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(w, _)| *w)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whisper's well-known hallucinations on silence/noise (incl. its Russian/Ukrainian classics).
fn is_hallucination(text: &str) -> bool {
    const KNOWN: &[&str] = &[
        "thank you.",
        "thanks for watching!",
        "thanks for watching.",
        "thank you for watching.",
        "thank you for watching!",
        "please subscribe.",
        "you",
        "продолжение следует...",
        "субтитры сделал dimatorzok",
        "субтитры создавал dimatorzok",
        "редактор субтитров а.семкин корректор а.егорова",
        "дякую за перегляд!",
        "дякую за увагу!",
        "спасибо за просмотр!",
        "¡gracias por ver!",
        "gracias.",
        "subtítulos realizados por la comunidad de amara.org",
    ];
    let t = text.trim().to_lowercase();
    // Lone filler words are what Whisper "hears" in near-silence (worse with a prompt).
    let bare: String = t.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    const FILLERS: &[&str] = &["you", "the", "a", "and", "so", "uh", "um", "bye", "ну", "и", "і", "y"];
    if FILLERS.contains(&bare.trim()) {
        return true;
    }
    // Sound annotations Whisper emits for noise/music: "*Song*", "[Music]", "(applause)", "♪".
    let annotation = |open: char, close: char| t.starts_with(open) && t.ends_with(close);
    t.is_empty()
        || KNOWN.iter().any(|k| t == *k)
        || t.chars().all(|c| !c.is_alphanumeric())
        || annotation('*', '*')
        || annotation('[', ']')
        || annotation('(', ')')
        || t.contains('♪')
}

/// whisper.cpp threads for the final pass: on macOS Metal does the work; a CPU-only build
/// scales to about the physical cores (half the logical ones).
pub(crate) fn final_threads() -> usize {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    if cfg!(target_os = "macos") { n.min(8) } else { (n / 2).clamp(4, 16) }
}

/// Allowed languages ranked by detection probability, for audio already in `state`'s mel.
fn rank_languages(state: &mut WhisperState, allowed: &[i32], threads: usize) -> Result<Vec<(i32, f32)>> {
    let (best_any, probs) = state.lang_detect(0, threads)?;
    if std::env::var_os("ECHOCTL_DEBUG").is_some() {
        let mut top: Vec<(i32, f32)> = probs.iter().enumerate().map(|(i, p)| (i as i32, *p)).collect();
        top.sort_by(|a, b| b.1.total_cmp(&a.1));
        let fmt: Vec<String> = top[..4.min(top.len())]
            .iter()
            .map(|(i, p)| format!("{}={:.2}", whisper_rs::get_lang_str(*i).unwrap_or("?"), p))
            .collect();
        eprintln!("\n  debug: lang probs {}", fmt.join(" "));
    }
    if allowed.is_empty() {
        return Ok(vec![(best_any, probs.get(best_any as usize).copied().unwrap_or(0.0))]);
    }
    let mut ranked: Vec<(i32, f32)> = allowed
        .iter()
        .filter(|id| (**id as usize) < probs.len())
        .map(|id| (*id, probs[*id as usize]))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    Ok(ranked)
}

/// A detection is "confident" when the top allowed language clearly wins. When it doesn't,
/// the track keeps its previous language if that is a close runner-up: forcing the wrong
/// language makes Whisper *translate* (accented English → fluent Ukrainian/Russian).
/// Note: comparing decode log-probs across languages does NOT work — translations and
/// hallucinations score higher than faithful transcriptions (tested, 2026-10-09).
const CONFIDENT_PROB: f32 = 0.6;
const CONFIDENT_MARGIN: f32 = 0.3;

/// Pick the language for one utterance from ranked candidates.
fn choose_language(ranked: &[(i32, f32)], prev: Option<i32>) -> Option<i32> {
    let (top, p1) = *ranked.first()?;
    let p2 = ranked.get(1).map(|r| r.1).unwrap_or(0.0);
    if p1 >= CONFIDENT_PROB && p1 - p2 >= CONFIDENT_MARGIN {
        return Some(top);
    }
    match prev {
        Some(prev) if ranked.iter().take(2).any(|(id, _)| *id == prev) => Some(prev),
        _ => Some(top),
    }
}

/// Whisper hint: a language-neutral, comma-separated term list (people, products, acronyms).
/// A sentence-style English prompt would bias Ukrainian/Russian utterances toward English
/// (i.e. translation), so only bare terms are passed. Capped: long prompts invite Whisper to
/// "hear" the listed names in silence.
pub(crate) fn build_prompt(cfg: &Config, meeting: Option<&MeetingContext>) -> Option<String> {
    const MAX_CHARS: usize = 400;
    const MAX_TERMS: usize = 30;
    const MAX_TERM_CHARS: usize = 50;
    let mut terms: Vec<String> = vec![];
    let mut add = |t: &str| {
        let t = t.trim();
        if !t.is_empty() && t.chars().count() <= MAX_TERM_CHARS && terms.len() < MAX_TERMS && !terms.iter().any(|x| x.eq_ignore_ascii_case(t)) {
            terms.push(t.to_string());
        }
    };
    if let Some(m) = meeting {
        m.people().iter().for_each(|p| add(p));
        m.vocabulary.iter().for_each(|v| add(v));
    }
    // The always-on terms, then the learned ones that belong to this meeting (`vocab.rs`).
    crate::vocab::for_call(cfg, meeting).iter().for_each(|v| add(v));
    let mut prompt = String::new();
    for t in terms {
        if prompt.len() + t.len() + 2 > MAX_CHARS {
            break;
        }
        if !prompt.is_empty() {
            prompt.push_str(", ");
        }
        prompt.push_str(&t);
    }
    (!prompt.is_empty()).then_some(prompt)
}

fn allowed_languages(cfg: &Config) -> Result<Vec<i32>> {
    cfg.languages
        .iter()
        .map(|l| whisper_rs::get_lang_id(l).ok_or_else(|| anyhow!("unknown language code '{l}' in config")))
        .collect()
}

/// whisper.cpp with the model loaded: the transcription model and, for another model than turbo,
/// turbo for language ID when downloaded.
pub(crate) struct Whisper {
    ctx: WhisperContext,
    /// Separate language-ID model (turbo): its detection is far more decisive than large-v3's.
    detector: Option<WhisperContext>,
    threads: usize,
}

/// Keep the hinted decode? Only when its length is in line with the unhinted one.
pub(crate) fn hint_is_sane(hinted: &str, plain: &str) -> bool {
    let (h, p) = (hinted.chars().count() as f64, plain.chars().count() as f64);
    h >= p * GUARD_MIN_RATIO && h <= p * GUARD_MAX_RATIO + GUARD_SLACK
}

/// One decoded sentence: seconds relative to the utterance, and its words' (start, end) —
/// one pair per whitespace-separated word of `text`, or none when they don't line up.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Piece {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<[f64; 2]>,
}

impl Piece {
    /// Keep the word times only if they still match the (cleaned) text.
    pub(crate) fn new(start: f64, end: f64, text: String, words: Vec<[f64; 2]>) -> Piece {
        let words = if text.split_whitespace().count() == words.len() { words } else { vec![] };
        Piece { start, end, text, words }
    }
}

/// Decoded sentences of one utterance.
pub(crate) type Decoded = Vec<Piece>;

/// whisper.cpp's decoding state, reused across utterances.
pub(crate) struct WhisperStates {
    state: WhisperState,
    det_state: Option<WhisperState>,
}

/// Flash attention on the Mac's GPU, Apple Silicon only. It can't be combined with DTW, which Echo
/// doesn't use.
fn context_params(use_gpu: bool) -> WhisperContextParameters<'static> {
    let flash_attn = flash_attention(use_gpu, cfg!(all(target_os = "macos", target_arch = "aarch64")));
    WhisperContextParameters { use_gpu, flash_attn, ..Default::default() }
}

fn flash_attention(use_gpu: bool, apple_silicon: bool) -> bool {
    use_gpu && apple_silicon
}

/// Turbo detects its own language; other Whisper models get turbo q5_0 as a separate detector.
fn needs_detector(model_path: &Path) -> bool {
    !model_path.file_name().and_then(|f| f.to_str()).is_some_and(models::detects_own_language)
}

/// The language-ID model to load beside `model_path`: the helper (turbo q5_0, `models::HELPER_MODEL`) when it is
/// downloaded and `model_path` needs it.
fn detector_path(model_path: &Path) -> Option<PathBuf> {
    let det = models::model_path(models::find(models::HELPER_MODEL).ok()?);
    (needs_detector(model_path) && det.exists() && det != model_path).then_some(det)
}

impl Whisper {
    /// Load `model_path` (GPU unless it failed its check, `gpu.rs`) and, for a model other than
    /// turbo, turbo for language ID when it is downloaded.
    fn load(model_path: &Path, threads: usize) -> Result<Whisper> {
        whisper_rs::install_logging_hooks(); // routes whisper.cpp's chatter to `log` (unset = silent)
        let use_gpu = crate::gpu::allowed();
        let cp = || context_params(use_gpu);
        let ctx = WhisperContext::new_with_params(model_path, cp()).context("loading model")?;
        if cp().flash_attn {
            eprintln!("  flash attention: on");
        }
        let detector = match detector_path(model_path) {
            Some(det) => {
                eprintln!("  language detection: {}", models::HELPER_MODEL);
                Some(WhisperContext::new_with_params(&det, cp())?)
            }
            None => None,
        };
        Ok(Whisper { ctx, detector, threads })
    }

    fn state(&self) -> Result<WhisperStates> {
        Ok(WhisperStates {
            state: self.ctx.create_state()?,
            det_state: match &self.detector {
                Some(d) => Some(d.create_state()?),
                None => None,
            },
        })
    }

    /// Detect the language (restricted to `allowed`, sticky: `prev` is the track's last one) and
    /// decode one utterance. Returns the detected language (None: uncertain), its code for the
    /// segments, and (start, end, text) relative to the chunk.
    fn utterance(
        &self,
        st: &mut WhisperStates,
        prev: Option<i32>,
        allowed: &[i32],
        prompt: Option<&str>,
        chunk: &[f32],
    ) -> Result<(Option<i32>, String, Decoded)> {
        let dur = chunk.len() as f64 / RATE as f64;
        let lang_id = match prev {
            Some(prev) if dur < SHORT_UTTERANCE_S => Some(prev),
            _ => {
                let s = st.det_state.as_mut().unwrap_or(&mut st.state);
                s.pcm_to_mel(chunk, self.threads)?;
                let ranked = rank_languages(s, allowed, self.threads)?;
                choose_language(&ranked, prev)
            }
        };
        let lang = lang_id.or(prev).and_then(whisper_rs::get_lang_str).unwrap_or("auto").to_string();
        let segs = match prompt {
            Some(hint) if dur >= GUARD_MIN_S => {
                let hinted = self.decode(&mut st.state, chunk, &lang, Some(hint))?;
                let plain = self.decode(&mut st.state, chunk, &lang, None)?;
                choose_hinted(hinted, plain, hint)
            }
            _ => self.decode(&mut st.state, chunk, &lang, None)?,
        };
        Ok((lang_id, lang, segs))
    }

    fn decode(&self, state: &mut WhisperState, chunk: &[f32], lang: &str, hint: Option<&str>) -> Result<Decoded> {
        let mut p = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
        p.set_n_threads(self.threads as i32);
        p.set_language(Some(lang));
        p.set_translate(false);
        p.set_no_context(true); // don't carry text (or a wrong language) across utterances
        p.set_temperature(0.0);
        // Per-token times: segments are split into sentences with their own start/end, so a
        // short answer isn't stamped with the start of the longer utterance it sits in.
        p.set_token_timestamps(true);
        p.set_suppress_blank(true);
        p.set_suppress_nst(true); // no "(music)", "[laughs]" or ♪ tokens
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_timestamps(false);
        p.set_print_special(false);
        if let Some(hint) = hint {
            p.set_initial_prompt(hint);
        }
        state.full(p, chunk)?;
        let mut segs = vec![];
        for seg in state.as_iter() {
            let text = clean_text(seg.to_str_lossy()?.trim());
            let tokens: Vec<Tok> = (0..seg.n_tokens())
                .filter_map(|i| seg.get_token(i))
                .filter_map(|t| {
                    let bytes = t.to_bytes().ok()?.to_vec();
                    let d = t.token_data();
                    Some(Tok { bytes, t0: d.t0 as f64 / 100.0, t1: d.t1 as f64 / 100.0, plog: d.plog })
                })
                .collect();
            let words: Vec<f32> = tokens.iter().filter(|t| !t.is_special()).map(|t| t.plog).collect();
            let confidence = if words.is_empty() { 0.0 } else { words.iter().sum::<f32>() / words.len() as f32 };
            let no_speech = seg.no_speech_probability();
            if std::env::var_os("ECHOCTL_DEBUG").is_some() {
                eprintln!("  debug: no_speech {no_speech:.2} logprob {confidence:.2} {text}");
            }
            if is_silence(no_speech, confidence) || is_hallucination(&text) {
                continue;
            }
            let (t0, t1) = (seg.start_timestamp() as f64 / 100.0, seg.end_timestamp() as f64 / 100.0);
            for p in split_sentences(&tokens, t0, t1) {
                let sentence = clean_text(&p.text);
                if !sentence.is_empty() && !is_hallucination(&sentence) && !is_word_list(&sentence) {
                    segs.push(Piece::new(p.start, p.end, sentence, shift(p.words, WHISPER_WORD_LAG)));
                }
            }
        }
        Ok(segs)
    }

}

/// A speech engine with its model loaded: whisper.cpp, or NVIDIA Nemotron (`nemo.rs`), picked by
/// the model (`models::Engine`). Shared by this computer's transcriber and the transcription
/// server (`remote.rs`). Another engine is one more variant here and a catalog entry.
pub(crate) enum Engine {
    Whisper(Whisper),
    Nemo(Nemo),
}

/// An engine's decoding state, reused across utterances (Nemotron keeps none).
pub(crate) enum EngineState {
    Whisper(WhisperStates),
    Nemo,
}

impl Engine {
    /// Load `model_path` with the engine its catalog entry names.
    pub(crate) fn load(model_path: &Path, threads: usize) -> Result<Engine> {
        Ok(match models::engine_of(model_path) {
            models::Engine::Whisper => Engine::Whisper(Whisper::load(model_path, threads)?),
            models::Engine::Nemo => Engine::Nemo(Nemo::load(model_path)?),
        })
    }

    pub(crate) fn state(&self) -> Result<EngineState> {
        Ok(match self {
            Engine::Whisper(w) => EngineState::Whisper(w.state()?),
            Engine::Nemo(_) => EngineState::Nemo,
        })
    }

    /// Detect the language (restricted to `allowed`, sticky: `prev` is the track's last one) and
    /// decode one utterance. Returns the detected language (None: uncertain), its code for the
    /// segments, and (start, end, text) relative to the chunk.
    pub(crate) fn utterance(
        &self,
        st: &mut EngineState,
        prev: Option<i32>,
        allowed: &[i32],
        prompt: Option<&str>,
        chunk: &[f32],
    ) -> Result<(Option<i32>, String, Decoded)> {
        match (self, st) {
            (Engine::Whisper(w), EngineState::Whisper(s)) => w.utterance(s, prev, allowed, prompt, chunk),
            (Engine::Nemo(n), _) => n.utterance(prev, allowed, prompt, chunk),
            (Engine::Whisper(w), st) => {
                *st = EngineState::Whisper(w.state()?);
                self.utterance(st, prev, allowed, prompt, chunk)
            }
        }
    }
}

/// Nemotron hears the language from this much of an utterance (a decode in `auto` mode); the
/// utterance is then decoded with that language pinned. `auto` alone mixes Russian into
/// Ukrainian ("Так я сгоден", tested 2026-10-13); pinned, it doesn't.
const NEMO_DETECT_S: f64 = 4.0;
/// Word times against the audio, measured on a two-speaker recording with reference turns
/// (2026-10-13): Whisper's token times run ~0.3–0.5 s early, Nemotron's streaming decoder
/// emits words ~0.4–0.5 s late. Corrected in the word times only, which serve speaker
/// detection (a word near a turn change otherwise goes to the wrong person); sentence times
/// are left as decoded (both tracks share the engine, so their order is unaffected).
const WHISPER_WORD_LAG: f64 = 0.3;
const NEMO_WORD_LAG: f64 = -0.4;

/// Weight of the vocabulary's words in Nemotron's decoder. Measured with a 24-term vocabulary
/// (2026-10-13): 0.5 fixes terms that were said ("service now" → ServiceNow, a client's surname)
/// and inserts nothing; 1 already garbles a sentence; 2 puts terms into speech that has none;
/// 3 (NVIDIA's example value, used at first) writes "Flow Studio" over every few words.
const NEMO_BOOST: f32 = 0.5;

/// NVIDIA Nemotron (NeMo-Speech.cpp) with the model loaded.
pub(crate) struct Nemo {
    model: crate::nemo::Model,
}

impl Nemo {
    fn load(model_path: &Path) -> Result<Nemo> {
        // Metal on macOS. Windows ships the CPU runtime (the point is laptops without a GPU).
        let gpu = cfg!(target_os = "macos") && crate::gpu::allowed();
        if let Some(v) = crate::nemo::version() {
            eprintln!("  NeMo-Speech {v} on the {}", if gpu { "GPU" } else { "CPU" });
        }
        Ok(Nemo { model: crate::nemo::Model::load(model_path, gpu)? })
    }

    fn utterance(&self, prev: Option<i32>, allowed: &[i32], prompt: Option<&str>, chunk: &[f32]) -> Result<(Option<i32>, String, Decoded)> {
        let dur = chunk.len() as f64 / RATE as f64;
        let phrases: Vec<String> = prompt.map(|p| p.split(", ").map(str::to_owned).collect()).unwrap_or_default();
        let codes: Vec<&str> = allowed.iter().filter_map(|l| whisper_rs::get_lang_str(*l)).collect();
        let prev_code = prev.and_then(whisper_rs::get_lang_str);
        let mut detected = None;
        let lang: Option<&str> = match (codes.as_slice(), prev_code) {
            ([only], _) => Some(only),
            (_, Some(p)) if dur < SHORT_UTTERANCE_S => Some(p),
            _ => {
                let n = chunk.len().min((NEMO_DETECT_S * RATE as f64) as usize);
                let r = self.model.recognize(&chunk[..n], None, &[], NEMO_BOOST)?;
                let heard: Vec<&str> = r.languages.iter().map(|l| crate::nemo::code(l)).collect();
                let ok = |c: &&str| codes.is_empty() || codes.contains(c);
                let pick = match prev_code {
                    // Sticky, as with Whisper: the track's language when the model heard it too.
                    Some(p) if heard.contains(&p) && ok(&p) => Some(p),
                    _ => heard.iter().copied().find(ok).or(prev_code).or(codes.first().copied()),
                };
                let text: String = r.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
                let pick = match (pick, cyrillic_hint(&text)) {
                    (Some("uk" | "ru"), Some(c)) if ok(&c) => Some(c),
                    (p, _) => p,
                };
                let pick = pick.and_then(|c| whisper_rs::get_lang_id(c).and_then(whisper_rs::get_lang_str));
                if n == chunk.len() && heard.len() == 1 && pick == heard.first().copied() {
                    detected = Some(r); // the whole utterance, in one language: no second pass
                }
                pick
            }
        };
        let locale = lang.and_then(crate::nemo::locale);
        let plain = match detected {
            Some(r) => r,
            None => self.model.recognize(chunk, locale, &[], NEMO_BOOST)?,
        };
        // The vocabulary boost, kept only when every term it adds sounds like what the plain
        // decode heard there: boosting alone put "Flow Studio" and "ServiceNow" into a call that
        // never mentioned them, dozens of times (2026-10-13).
        let r = if phrases.is_empty() {
            plain
        } else {
            let boosted = self.model.recognize(chunk, locale, &phrases, NEMO_BOOST)?;
            let text = |r: &crate::nemo::Recognized| r.words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ");
            if boost_is_heard(&text(&boosted), &text(&plain), &phrases) { boosted } else { plain }
        };
        if std::env::var_os("ECHOCTL_DEBUG").is_some() {
            eprintln!("  debug: nemotron {dur:.1}s {lang:?} heard {:?}, {} words: {}", r.languages, r.words.len(),
                r.words.iter().take(12).map(|w| format!("{}@{:.1}", w.text, w.start)).collect::<Vec<_>>().join(" "));
        }
        let lang_id = lang.and_then(whisper_rs::get_lang_id);
        let segs = nemo_sentences(&r.words)
            .into_iter()
            .map(|p| Piece::new(p.start, p.end, clean_text(&p.text), shift(p.words, NEMO_WORD_LAG)))
            .filter(|p| !p.text.is_empty() && !is_hallucination(&p.text) && !is_word_list(&p.text))
            .collect();
        Ok((lang_id, lang.unwrap_or("auto").to_string(), segs))
    }
}

/// Cyrillic (Ukrainian, Russian) spelled in Latin letters, lowercased, letters and digits only:
/// enough to compare how a word sounds across scripts ("сервис нау" ~ "ServiceNow").
fn latin(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        let t = match c {
            'а' => "a", 'б' => "b", 'в' => "v", 'г' | 'ґ' => "g", 'д' => "d", 'е' | 'ё' | 'є' | 'э' => "e",
            'ж' => "zh", 'з' => "z", 'и' | 'і' | 'ї' | 'й' | 'ы' => "i", 'к' => "k", 'л' => "l", 'м' => "m",
            'н' => "n", 'о' => "o", 'п' => "p", 'р' => "r", 'с' => "s", 'т' => "t", 'у' => "u", 'ф' => "f",
            'х' => "h", 'ц' => "ts", 'ч' => "ch", 'ш' => "sh", 'щ' => "sch", 'ю' => "yu", 'я' => "ya",
            'ъ' | 'ь' => "",
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => continue,
        };
        out.push_str(t);
    }
    out
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// Does the plain decode hold something that sounds like `term`? The best stretch of its
/// words (in Latin letters) within half the term's length of distance.
fn sounds_heard(term: &str, plain_words: &[String]) -> bool {
    let t: Vec<char> = latin(term).chars().collect();
    if t.is_empty() {
        return true;
    }
    for i in 0..plain_words.len() {
        let mut stretch = String::new();
        for w in &plain_words[i..] {
            stretch.push_str(w);
            let s: Vec<char> = stretch.chars().collect();
            if edit_distance(&t, &s) * 2 <= t.len() {
                return true;
            }
            if s.len() > t.len() + 3 {
                break;
            }
        }
    }
    false
}

/// Keep a vocabulary-boosted decode? Only when each vocabulary term it has and the plain decode
/// lacks replaced something that sounds like it ("сервис нау" → ServiceNow), not words that
/// don't ("ха они" → Flow Studio).
fn boost_is_heard(boosted: &str, plain: &str, phrases: &[String]) -> bool {
    let plain_l = plain.to_lowercase();
    let boosted_l = boosted.to_lowercase();
    let plain_words: Vec<String> = plain.split_whitespace().map(latin).filter(|w| !w.is_empty()).collect();
    phrases.iter().all(|p| {
        let term = p.to_lowercase();
        let added = boosted_l.matches(term.as_str()).count().saturating_sub(plain_l.matches(term.as_str()).count());
        added == 0 || sounds_heard(p, &plain_words)
    })
}

/// Ukrainian or Russian, from letters only one of them has (і ї є ґ / ы э ё ъ). Nemotron's own
/// detection tags Ukrainian speech `ru-RU, uk-UA` and writes its first words in Russian; the
/// letters it wrote still give the language away.
fn cyrillic_hint(text: &str) -> Option<&'static str> {
    let count = |set: &str| text.chars().flat_map(char::to_lowercase).filter(|c| set.contains(*c)).count();
    let (uk, ru) = (count("іїєґ"), count("ыэёъ"));
    if uk > ru * 2 && uk >= 1 {
        Some("uk")
    } else if ru > uk * 2 && ru >= 1 {
        Some("ru")
    } else {
        None
    }
}

fn shift(words: Vec<[f64; 2]>, by: f64) -> Vec<[f64; 2]> {
    words.into_iter().map(|[a, b]| [(a + by).max(0.0), (b + by).max(0.0)]).collect()
}

/// Nemotron's words → sentences, each timed by its own words (as `split_sentences` does for
/// Whisper's tokens).
fn nemo_sentences(words: &[crate::nemo::Word]) -> Vec<Piece> {
    let mut out = vec![];
    let mut cur: Option<Piece> = None;
    for w in words {
        let t = w.text.trim();
        if t.is_empty() || t.contains(char::is_whitespace) {
            continue;
        }
        let c = cur.get_or_insert_with(|| Piece { start: w.start, end: w.end, text: String::new(), words: vec![] });
        if !c.text.is_empty() {
            c.text.push(' ');
        }
        c.text.push_str(t);
        c.end = w.end.max(c.start);
        c.words.push([w.start, w.end.max(w.start)]);
        if t.ends_with(['.', '?', '!', '…']) {
            out.extend(cur.take());
        }
    }
    out.extend(cur);
    out
}

/// Where utterances are decoded: this computer, or a transcription server on the local network
/// (`remote.rs`) with this computer as the fallback while it can't be reached.
enum Backend {
    Local(Engine),
    Remote {
        server: crate::remote::Client,
        /// This computer's model, loaded the first time the server can't be reached.
        fallback: Option<PathBuf>,
        local: OnceLock<Option<Engine>>,
        threads: usize,
        /// The server failed: decode here until then.
        down_until: Mutex<Option<Instant>>,
    },
}

/// A server that failed is tried again after this.
const SERVER_RETRY: Duration = Duration::from_secs(60);
/// No local model to fall back on: how long to keep waiting for the server.
const SERVER_WAIT: Duration = Duration::from_secs(15 * 60);

pub(crate) struct Transcriber {
    backend: Backend,
    allowed: Vec<i32>,
    prompt: Option<String>,
}

/// Per-track decoding state, reused across utterances.
pub(crate) struct TrackState {
    /// Created when this computer decodes (always, or the first time the server fails).
    local: Option<EngineState>,
    prev_lang: Option<i32>,
}

impl Transcriber {
    pub(crate) fn track_state(&self) -> Result<TrackState> {
        let local = match &self.backend {
            Backend::Local(e) => Some(e.state()?),
            Backend::Remote { .. } => None,
        };
        Ok(TrackState { local, prev_lang: None })
    }

    /// Detect the language (restricted, sticky per track) and decode one utterance.
    /// Returns the language code and (start, end, text) relative to the chunk.
    pub(crate) fn utterance(&self, ts: &mut TrackState, chunk: &[f32]) -> Result<(String, Decoded)> {
        let (lang_id, lang, segs) = match &self.backend {
            Backend::Local(e) => self.local(e, ts, chunk)?,
            Backend::Remote { server, fallback, local, threads, down_until } => {
                let waiting = Instant::now();
                loop {
                    let down = down_until.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|t| Instant::now() < t);
                    if !down {
                        match server.utterance(chunk, ts.prev_lang, &self.allowed, self.prompt.as_deref()) {
                            Ok(r) => break r,
                            Err(e) if e.downcast_ref::<crate::remote::Refused>().is_some() => return Err(e),
                            Err(e) => {
                                eprintln!("  transcription server: {e:#} — this computer decodes for {}s", SERVER_RETRY.as_secs());
                                *down_until.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now() + SERVER_RETRY);
                            }
                        }
                    }
                    let engine = local.get_or_init(|| {
                        let path = fallback.as_ref()?;
                        Engine::load(path, *threads).map_err(|e| eprintln!("  this computer's model: {e:#}")).ok()
                    });
                    if let Some(e) = engine {
                        break self.local(e, ts, chunk)?;
                    }
                    // No model here: the WAVs keep the audio, so wait for the server.
                    if waiting.elapsed() < Duration::from_secs(1) {
                        eprintln!("  no speech model on this computer: waiting for the transcription server");
                    }
                    if waiting.elapsed() > SERVER_WAIT {
                        bail!("the transcription server can't be reached and this computer has no speech model");
                    }
                    std::thread::sleep(Duration::from_secs(10));
                    *down_until.lock().unwrap_or_else(|e| e.into_inner()) = None;
                }
            }
        };
        ts.prev_lang = lang_id.or(ts.prev_lang);
        Ok((lang, segs))
    }

    fn local(&self, e: &Engine, ts: &mut TrackState, chunk: &[f32]) -> Result<(Option<i32>, String, Decoded)> {
        if ts.local.is_none() {
            ts.local = Some(e.state()?);
        }
        let st = ts.local.as_mut().expect("just set");
        e.utterance(st, ts.prev_lang, &self.allowed, self.prompt.as_deref(), chunk)
    }

    pub(crate) fn track(&self, speaker: &str, audio: &[f32]) -> Result<Vec<Segment>> {
        let mut ts = self.track_state()?;
        let spans = utterances(audio);
        let mut out = vec![];
        let tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
        for (n, (s, e)) in spans.iter().enumerate() {
            let (lang, segs) = self.utterance(&mut ts, &audio[*s..*e])?;
            let offset = *s as f64 / RATE as f64;
            for p in segs {
                out.push(Segment::from_piece(speaker, offset, &lang, p, None));
            }
            if tty {
                eprint!("\r  {speaker}: {}/{} utterances", n + 1, spans.len());
            }
        }
        if tty && !spans.is_empty() {
            eprintln!();
        } else if !tty {
            eprintln!("  {speaker}: {} utterances", spans.len());
        }
        Ok(out)
    }
}

/// One decoded token: its raw bytes (a UTF-8 character can span tokens) and times (seconds).
pub(crate) struct Tok {
    bytes: Vec<u8>,
    t0: f64,
    t1: f64,
    /// Log-probability Whisper gave this token.
    plog: f32,
}

impl Tok {
    fn is_special(&self) -> bool {
        self.bytes.starts_with(b"[_") || self.bytes.starts_with(b"<|")
    }
}

/// Whisper's own test for "this was silence, the text is made up": a likely no-speech window
/// whose words are low-confidence (mean token log-prob below -1, Whisper's threshold), or a
/// very likely one regardless. Quiet or distant speech is where it invents lists of names.
fn is_silence(no_speech: f32, mean_logprob: f32) -> bool {
    no_speech > 0.6 || (no_speech > 0.2 && mean_logprob < -1.0)
}

/// A bare list of short items, no sentence ("Varkin, Node, Cage, Fork",
/// "Миро, Даша, Тайма, Аквара, …"): what Whisper invents from the vocabulary hint or noise.
/// Real lists come inside sentences, so a list is dropped only when it is nearly all there is.
fn is_word_list(text: &str) -> bool {
    let parts: Vec<&str> = text.trim_end_matches(['.', '!', '?', '…']).split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
    if parts.len() < 4 {
        return false;
    }
    let short = parts.iter().filter(|p| p.split_whitespace().count() <= 2).count();
    short * 4 >= parts.len() * 3 && text.split_whitespace().count() <= parts.len() * 2 + 2
}

/// The hinted decode, unless the hint distorted it (too long or short against the plain one, its
/// terms in place of other speech, a term nobody said): then the plain one. Any engine or server.
pub(crate) fn choose_hinted(hinted: Decoded, plain: Decoded, hint: &str) -> Decoded {
    let text = |d: &Decoded| d.iter().map(|p| p.text.as_str()).collect::<Vec<_>>().join(" ");
    let (ht, pt) = (text(&hinted), text(&plain));
    let terms: Vec<String> = hint.split(", ").map(str::to_owned).collect();
    if hint_is_sane(&ht, &pt) && !hint_adds_terms(&ht, &pt, hint) && boost_is_heard(&ht, &pt, &terms) {
        hinted
    } else {
        plain
    }
}

/// The hint made Whisper "hear" its own terms: most of the hinted text is hint words the plain
/// decode doesn't have ("Miro, IPAM, Ravi" over silence). A hint term in place of a misheard
/// word ("ServiceNow" for "service now") is the hint doing its job and is kept.
fn hint_adds_terms(hinted: &str, plain: &str, hint: &str) -> bool {
    let (h, p) = (words(hinted), words(plain));
    let terms: std::collections::HashSet<String> = hint.split(',').flat_map(words).collect();
    let added = h.iter().filter(|w| terms.contains(*w) && !p.contains(w)).count();
    !h.is_empty() && added * 2 >= h.len()
}

/// Split a segment into sentences, each timed by its own first and last token. Special tokens
/// (`[_BEG_]`, `<|…|>`) are skipped; a sentence ends at `.?!…` once it has ≥ 12 characters. If
/// the token times are unusable the segment comes back whole with its own times.
pub(crate) fn split_sentences(tokens: &[Tok], seg_t0: f64, seg_t1: f64) -> Vec<Piece> {
    let mut out: Vec<Piece> = vec![];
    let (mut buf, mut start, mut end): (Vec<u8>, Option<f64>, f64) = (vec![], None, seg_t0);
    // Word times: a token that starts with a space starts a word; others extend it.
    let mut words: Vec<[f64; 2]> = vec![];
    let sane = |t: f64| t >= seg_t0 - 0.5 && t <= seg_t1 + 0.5;
    let clamp = |t: f64| t.clamp(seg_t0, seg_t1);
    for t in tokens {
        if t.bytes.starts_with(b"[_") || t.bytes.starts_with(b"<|") {
            continue;
        }
        let mut blank = buf.last().is_none_or(|b| b.is_ascii_whitespace());
        for b in &t.bytes {
            if b.is_ascii_whitespace() {
                blank = true;
            } else if blank || words.is_empty() {
                words.push([clamp(t.t0), clamp(t.t1.max(t.t0))]);
                blank = false;
            } else if let Some(w) = words.last_mut() {
                w[1] = w[1].max(clamp(t.t1));
            }
        }
        buf.extend_from_slice(&t.bytes);
        let text = String::from_utf8_lossy(&buf);
        // The sentence starts at its first token with a letter (not at stray punctuation).
        if start.is_none() && text.chars().any(char::is_alphanumeric) && sane(t.t0) {
            start = Some(t.t0);
        }
        if sane(t.t1) {
            end = end.max(t.t1);
        }
        let trimmed = text.trim();
        if trimmed.chars().count() >= 12 && trimmed.ends_with(['.', '?', '!', '…']) {
            out.push(Piece::new(start.unwrap_or(seg_t0), end, trimmed.to_string(), std::mem::take(&mut words)));
            buf.clear();
            start = None;
        }
    }
    let rest = String::from_utf8_lossy(&buf).trim().to_string();
    if !rest.is_empty() {
        out.push(Piece::new(start.unwrap_or(seg_t0), end.max(seg_t1), rest, words));
    }
    // Times must not run backwards or past the segment.
    for o in out.iter_mut() {
        o.start = o.start.clamp(seg_t0, seg_t1);
        o.end = o.end.clamp(o.start, seg_t1.max(o.start));
    }
    out
}

/// Order the merged transcript. By start time, except that when two people's lines start
/// within ~half a second and overlap, the one that ends later goes second: a reply finishes
/// after the question it answers, while Whisper's start times are only accurate to ~1 s.
pub(crate) fn order_turns(segs: &mut [Segment]) {
    segs.sort_by(|a, b| a.start.total_cmp(&b.start));
    for _ in 0..3 {
        let mut swapped = false;
        for i in 0..segs.len().saturating_sub(1) {
            let (a, b) = (&segs[i], &segs[i + 1]);
            let near = b.start - a.start < 0.6;
            let overlap = b.start < a.end;
            if a.speaker != b.speaker && near && overlap && a.end > b.end + 0.3 {
                segs.swap(i, i + 1);
                swapped = true;
            }
        }
        if !swapped {
            break;
        }
    }
}

fn words(t: &str) -> Vec<String> {
    t.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Share of `a`'s words that also appear in `b`.
fn overlap(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() {
        return 0.0;
    }
    a.iter().filter(|w| b.contains(w)).count() as f64 / a.len() as f64
}

/// Drop mic segments that are echo of the remote side: when the user is on speakers, the mic
/// re-records the call, producing near-duplicate "Me" lines a moment after "Remote" ones.
fn remove_echo(segs: Vec<Segment>) -> (Vec<Segment>, usize) {
    const WINDOW_S: f64 = 4.0;
    const SIMILAR: f64 = 0.6;
    let remote: Vec<(f64, f64, Vec<String>)> = segs
        .iter()
        .filter(|s| s.speaker == "remote")
        .map(|s| (s.start, s.end, words(&s.text)))
        .collect();
    let before = segs.len();
    let kept: Vec<Segment> = segs
        .into_iter()
        .filter(|s| {
            if s.speaker != "me" {
                return true;
            }
            let w = words(&s.text);
            !remote.iter().any(|(rs, re, rw)| {
                s.start < re + WINDOW_S && s.end > rs - WINDOW_S && overlap(&w, rw) >= SIMILAR
            })
        })
        .collect();
    let dropped = before - kept.len();
    (kept, dropped)
}

fn hms(t: f64) -> String {
    let t = t.max(0.0) as u64;
    format!("{:02}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
}

pub(crate) fn render_markdown(
    dir: &Path,
    meta: &serde_json::Value,
    meeting: Option<&MeetingContext>,
    model: &str,
    segs: &[Segment],
) -> String {
    let title = meeting
        .and_then(|m| m.subject.as_deref())
        .or(meta["title"].as_str())
        .unwrap_or("Call");
    let started = meta["started_at"].as_str().unwrap_or("");
    let duration = segs.iter().map(|s| s.end).fold(0.0, f64::max);
    let mut md = format!(
        "# {title}\n\n- **Recorded:** {started}\n- **Duration (speech):** {}\n- **Model:** {model}\n- **Folder:** `{}`\n\n",
        hms(duration),
        dir.file_name().unwrap().to_string_lossy()
    );
    if let Some(people) = meeting.map(|m| m.people()).filter(|p| !p.is_empty()) {
        md.insert_str(md.len() - 1, &format!("- **Invited:** {}\n", people.join(", ")));
    }
    // A memo is one voice from start to end: paragraphs of ~10 s keep its times usable (the minutes cite them, its
    // threads are split by them); a call's turns are as long as the speaker's.
    let turn_cap = if crate::memo::is_memo(dir) { 10.0 } else { f64::INFINITY };
    // Merge consecutive segments of the same speaker (and language) into one paragraph.
    let mut i = 0;
    while i < segs.len() {
        let s = &segs[i];
        let mut text = s.text.clone();
        let mut j = i + 1;
        while j < segs.len() && segs[j].speaker == s.speaker && segs[j].name == s.name && segs[j].lang == s.lang && segs[j].start - segs[j - 1].end < 2.0
            && segs[j].start - s.start < turn_cap
        {
            text.push(' ');
            text.push_str(&segs[j].text);
            j += 1;
        }
        let who = if s.speaker == "me" { "Me" } else { s.name.as_deref().unwrap_or("Remote") };
        md.push_str(&format!("**{who}** [{}] _{}_\n{text}\n\n", hms(s.start), s.lang));
        i = j;
    }
    // The live notes kept during the call stay with its transcript (a draft, marked as such).
    md.push_str(&crate::livenotes::markdown_section(dir));
    md
}

/// Marks a call as being transcribed (shown as "transcribing…" in the tray).
pub const IN_PROGRESS: &str = ".transcribing";

pub(crate) struct InProgress(PathBuf);
impl Drop for InProgress {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Create the in-progress marker exclusively (it holds our pid); a marker left by a process
/// that is gone is taken over, a live one means another transcription is running.
pub(crate) fn take_marker(dir: &Path) -> Result<InProgress> {
    let path = dir.join(IN_PROGRESS);
    for _ in 0..2 {
        match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                use std::io::Write;
                write!(f, "{}", std::process::id())?;
                return Ok(InProgress(path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let pid = fs::read_to_string(&path).ok().and_then(|p| p.trim().parse::<u32>().ok());
                if pid.is_some_and(crate::app::process_alive) {
                    bail!("{} is already being transcribed (pid {})", dir.display(), pid.unwrap());
                }
                let _ = fs::remove_file(&path);
            }
            Err(e) => return Err(e.into()),
        }
    }
    bail!("could not mark {} as being transcribed", dir.display())
}

/// The final pass's transcriber: the configured model (or `model_override`), with turbo for
/// language ID beside a model other than turbo when it is downloaded — or the transcription server when one is set
/// (`echoctl server use`; never with `model_override`). Returns it with the model's name.
pub(crate) fn final_transcriber(cfg: &Config, model_override: Option<&str>, prompt: Option<String>, threads: usize) -> Result<(Transcriber, String)> {
    let allowed = allowed_languages(cfg)?;
    if let Some(p) = &prompt {
        eprintln!("  vocabulary hint: {p}");
    }
    if let (None, Some(server)) = (model_override, crate::remote::Client::configured()) {
        let fallback = models::active_model_path().ok().filter(|p| p.exists());
        let name = match server.hello() {
            Ok(h) => format!("{} on {}", h.model, h.name),
            Err(e) => {
                eprintln!("  transcription server: {e:#}");
                match &fallback {
                    Some(_) => format!("{} (server not reachable)", cfg.model.clone().unwrap_or_default()),
                    None => format!("transcription server {}", server.url()),
                }
            }
        };
        eprintln!("  model: {name}");
        let backend = Backend::Remote { server, fallback, local: OnceLock::new(), threads, down_until: Mutex::new(None) };
        return Ok((Transcriber { backend, allowed, prompt }, name));
    }
    let (model_name, model_path) = match model_override {
        Some(name) => {
            let m = models::find(name)?;
            (m.name.to_string(), models::model_path(m))
        }
        None => (cfg.model.clone().unwrap_or_default(), models::active_model_path()?),
    };
    if !model_path.exists() {
        bail!("model {model_name} is not downloaded — run `echoctl model download {model_name}`");
    }
    eprintln!("  model: {model_name}");
    let engine = Engine::load(&model_path, threads)?;
    Ok((Transcriber { backend: Backend::Local(engine), allowed, prompt }, model_name))
}

/// Order the segments, drop the mic's echo of the remote side, name the remote speaker when the
/// invite says who it is, and write `segments<suffix>.json` + `transcript<suffix>.md`. Then the
/// audio is deleted (text only) unless `keep_audio` / `config.keep_audio`.
pub(crate) fn write_transcript(dir: &Path, cfg: &Config, model_name: &str, mut segs: Vec<Segment>, suffix: &str, keep_audio: bool) -> Result<PathBuf> {
    order_turns(&mut segs);
    let (mut segs, echoes) = remove_echo(segs);
    let meeting = MeetingContext::load(dir)?;
    if let Some(name) = meeting.as_ref().and_then(|m| m.sole_other(cfg.me.as_ref())) {
        for s in segs.iter_mut().filter(|s| s.speaker == "remote") {
            s.name = Some(name.clone());
        }
    } else if let Err(e) = crate::speakers::label(dir, &mut segs, cfg.me.as_ref().map(|m| m.name.as_str())) {
        // A 1:1 call is named from the invite; anything bigger is split by voice when the
        // speaker model is downloaded.
        eprintln!("  speaker detection skipped: {e:#}");
    }
    if echoes > 0 {
        eprintln!("  dropped {echoes} mic segment(s) that echoed the remote side");
    }
    let meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json"))?)?;
    fs::write(dir.join(format!("segments{suffix}.json")), serde_json::to_vec_pretty(&segs)?)?;
    let md_path = dir.join(format!("transcript{suffix}.md"));
    fs::write(&md_path, render_markdown(dir, &meta, meeting.as_ref(), model_name, &segs))?;
    eprintln!("  {} segments", segs.len());
    // Text only: the audio goes once the transcript is safely on disk (kept on any error above).
    if !(keep_audio || cfg.keep_audio) {
        for f in ["mic.wav", "remote.wav"] {
            let _ = fs::remove_file(dir.join(f));
        }
        // Under the call's meta lock, read inside it: the AI profile may be changed meanwhile.
        crate::calls::update_meta(dir, |m| {
            m["audio"] = serde_json::json!("deleted after transcription");
            Ok(())
        })?;
        eprintln!("audio deleted (only text is kept; `--keep-audio` or `config --keep-audio true` to keep it)");
    }
    Ok(md_path)
}

pub fn transcribe(call: &str, model_override: Option<&str>, keep_audio: bool) -> Result<()> {
    let dir = resolve_call(call)?;
    if !dir.join("mic.wav").exists() && !dir.join("remote.wav").exists() {
        bail!("{} has no audio (it was already transcribed and the audio deleted)", dir.display());
    }
    // The pid lets the app tell a live transcription from one a crash left behind.
    // Never the call being recorded right now. One a crash left open is closed here first.
    let from_watcher = std::env::var_os(crate::watch::FROM_WATCHER).is_some_and(|d| Path::new(&d) == dir);
    let recording = crate::watch::running_watcher().and_then(|w| w.recording).map(PathBuf::from);
    if !from_watcher && recording.is_some_and(|r| r == dir || r.ends_with(dir.file_name().unwrap_or_default())) {
        bail!("{} is still being recorded — it is transcribed when it stops", dir.display());
    }
    crate::watch::close_unfinished(&dir)?;
    let _marker = take_marker(&dir)?;
    let mut cfg = Config::load()?;
    // A call can pin its own languages (chosen when it was armed).
    let call_meta: serde_json::Value = serde_json::from_slice(&fs::read(dir.join("meta.json"))?)?;
    if let Some(l) = call_meta["languages"].as_array().filter(|a| !a.is_empty()) {
        cfg.languages = l.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    }
    let meeting = MeetingContext::load(&dir)?;
    let t = Instant::now();
    eprintln!("transcribing {} (languages: {})", dir.display(),
        if cfg.languages.is_empty() { "any".into() } else { cfg.languages.join(", ") });
    let (tr, model_name) = final_transcriber(&cfg, model_override, build_prompt(&cfg, meeting.as_ref()), final_threads())?;

    let mut segs = vec![];
    let remote = dir.join("remote.wav").exists().then(|| load_16k(&dir.join("remote.wav"))).transpose()?;
    if dir.join("mic.wav").exists() {
        let mut mic = load_16k(&dir.join("mic.wav"))?;
        // On speakers the mic re-records the others: cancel that echo first (headphones: no-op).
        if let (Some(r), true) = (&remote, cfg.echo_cancel.unwrap_or(true)) {
            match crate::aec::cancel(&mic, r) {
                Ok(Some((clean, delay))) => {
                    eprintln!("  echo cancellation: on (speaker → mic delay ≈ {delay} ms)");
                    mic = clean;
                }
                Ok(None) => eprintln!("  echo cancellation: no echo (headphones)"),
                Err(e) => eprintln!("  echo cancellation skipped: {e:#}"),
            }
        }
        segs.extend(tr.track("me", &mic)?);
    }
    if let Some(r) = &remote {
        segs.extend(tr.track("remote", r)?);
    }
    let suffix = model_override.map(|m| format!(".{m}")).unwrap_or_default();
    // A comparison run (--model) must leave the audio for the real transcript.
    let md_path = write_transcript(&dir, &cfg, &model_name, segs, &suffix, keep_audio || model_override.is_some())?;
    eprintln!("done in {:.0}s", t.elapsed().as_secs_f64());
    println!("{}", md_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vad_finds_speech_between_silence() {
        let mut a = vec![0.0005f32; RATE]; // 1 s near-silence
        a.extend((0..RATE).map(|i| 0.3 * (i as f32 * 0.05).sin())); // 1 s tone
        a.extend(vec![0.0005f32; RATE]);
        let u = utterances(&a);
        assert_eq!(u.len(), 1);
        let (s, e) = u[0];
        assert!((s as i64 - RATE as i64).abs() < (RATE / 3) as i64, "start {s}");
        assert!((e as i64 - 2 * RATE as i64).abs() < (RATE / 3) as i64, "end {e}");
    }

    #[test]
    fn vad_splits_long_speech() {
        let a: Vec<f32> = (0..RATE * 60).map(|i| 0.3 * (i as f32 * 0.05).sin()).collect();
        assert!(utterances(&a).iter().all(|(s, e)| (e - s) as f64 / RATE as f64 <= MAX_UTTERANCE_S + 0.5));
    }

    #[test]
    fn uncertain_detection_keeps_track_language() {
        let (en, uk, ru) = (0, 1, 2);
        // large-v3 on accented English: uk 0.41 / ru 0.27 / en 0.25, track was English
        assert_eq!(choose_language(&[(uk, 0.41), (ru, 0.27), (en, 0.25)], Some(en)), Some(uk)); // en not top-2
        assert_eq!(choose_language(&[(uk, 0.36), (en, 0.34)], Some(en)), Some(en));
        assert_eq!(choose_language(&[(uk, 0.90), (en, 0.05)], Some(en)), Some(uk)); // confident switch
        assert_eq!(choose_language(&[], None), None);
    }

    #[test]
    fn replies_follow_the_question_they_answer() {
        // The answer stamped 0.27 s before the question, as Whisper does.
        let seg = |speaker: &str, start: f64, end: f64, text: &str| Segment {
            speaker: speaker.into(), start, end, lang: "en".into(), text: text.into(), name: None, speaker_id: None, words: vec![],
        };
        let mut s = vec![
            seg("me", 171.12, 176.28, "No, no, she's out this week, so..."),
            seg("remote", 171.39, 174.43, "Yeah, is Kim joining or not?"),
            seg("me", 182.0, 183.0, "Later."),
        ];
        order_turns(&mut s);
        assert_eq!(s[0].speaker, "remote");
        assert_eq!(s[1].speaker, "me");
        // Far apart starts keep plain start order.
        let mut t = vec![seg("me", 10.0, 20.0, "long"), seg("remote", 12.0, 13.0, "short")];
        order_turns(&mut t);
        assert_eq!(t[0].speaker, "me");
    }

    #[test]
    fn segments_split_into_timed_sentences() {
        let tok = |s: &str, t0: f64, t1: f64| Tok { bytes: s.as_bytes().to_vec(), t0, t1, plog: 0.0 };
        let toks = vec![
            tok("[_BEG_]", 0.0, 0.0), tok(" Yep", 0.2, 0.5), tok(", yep. That works", 0.5, 1.2), tok(".", 1.2, 1.3),
            tok(" No, no, she's on leave", 3.1, 4.6), tok(", so...", 4.6, 5.0),
        ];
        let out = split_sentences(&toks, 0.0, 5.2);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!(out[0].text, "Yep, yep. That works.");
        assert_eq!(out[0].words.len(), 4, "{out:?}");
        assert!((out[1].start - 3.1).abs() < 1e-6, "second sentence starts at its own first token");
        // Cyrillic split mid-character across tokens survives.
        let bytes = "Привіт, як справи?".as_bytes();
        let (a, b) = bytes.split_at(3);
        let out = split_sentences(&[Tok { bytes: a.to_vec(), t0: 0.0, t1: 0.3, plog: 0.0 }, Tok { bytes: b.to_vec(), t0: 0.3, t1: 1.0, plog: 0.0 }], 0.0, 1.0);
        assert_eq!(out[0].text, "Привіт, як справи?");
    }

    #[test]
    fn hint_guard_rejects_runaway_or_lost_text() {
        assert!(hint_is_sane("We use ServiceNow for operations", "We use as no for operations"));
        assert!(!hint_is_sane("", "We talked about the migration plan for a while"));
        assert!(!hint_is_sane(&"Pat O, Miro, IPAM, ".repeat(10), "yes"));
        assert!(hint_is_sane("Okay.", "Okay."));
    }

    #[test]
    fn made_up_lists_and_hint_words_are_caught() {
        assert!(is_word_list("Varkin, Node, Cage, Fork"));
        assert!(is_word_list("Миро, Даша, Тайма, Аквара, Атаке, Ападинья, и то и то, и меседник."));
        assert!(!is_word_list("We need the export, the report, the cluster and the budget done by Friday, okay?"));
        assert!(!is_word_list("Да, да, конечно."));
        assert!(!is_word_list("Ну, смотри, я думаю, что это надо сделать завтра утром, до встречи с ними."));
        let hint = "ServiceNow, Miro, IPAM, Marta Kowal";
        assert!(hint_adds_terms("Miro, IPAM, Marta Kowal", "", hint));
        assert!(!hint_adds_terms("We moved it to ServiceNow", "We moved it to service now", hint));
        assert!(hint_adds_terms("Marta Kowal, Miro.", "Okay.", hint));
        assert!(!hint_adds_terms("Open Miro and draw it", "Open Miro and draw it", hint));
        assert!(!hint_adds_terms("Давай посмотрим", "Давай посмотрим", hint));
        assert!(is_silence(0.7, -0.2) && is_silence(0.3, -1.3) && !is_silence(0.3, -0.4) && !is_silence(0.1, -1.5));
    }

    #[test]
    fn prompt_is_bare_deduplicated_terms() {
        let cfg = Config { vocabulary: vec!["ServiceNow".into(), "Miro".into()], ..Default::default() };
        let m = MeetingContext {
            organizer: Some(crate::meeting::Person { name: "Pat O".into(), email: None }),
            attendees: vec![crate::meeting::Person { name: "Pat O".into(), email: None }],
            vocabulary: vec!["miro".into(), "IPAM".into()],
            ..Default::default()
        };
        assert_eq!(build_prompt(&cfg, Some(&m)).unwrap(), "Pat O, miro, IPAM, ServiceNow");
        assert_eq!(build_prompt(&Config::default(), None), None);
    }

    #[test]
    fn echo_of_remote_on_mic_is_dropped() {
        let seg = |speaker: &str, start: f64, text: &str| Segment {
            speaker: speaker.into(), start, end: start + 2.0, lang: "en".into(), text: text.into(), name: None, speaker_id: None, words: vec![],
        };
        let (kept, dropped) = remove_echo(vec![
            seg("remote", 19.0, "No, we know where."),
            seg("me", 15.5, "No, we know"),
            seg("me", 30.0, "Okay, let's move on to the backlog."),
        ]);
        assert_eq!(dropped, 1);
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn inline_hallucinations_and_sound_captions_are_removed() {
        assert_eq!(
            clean_text("Subtitles by the Amara.org community How short the message should be. BELL RINGS If you're happy"),
            "How short the message should be. If you're happy"
        );
        assert_eq!(clean_text("We use IPAM and ServiceNow"), "We use IPAM and ServiceNow");
        assert_eq!(clean_text("Move IPAM SSO first"), "Move IPAM SSO first"); // acronym pair stays
        assert_eq!(clean_text("MUSIC PLAYING Hello"), "Hello");
    }

    #[test]
    fn known_hallucinations_are_dropped() {
        assert!(is_hallucination("Продолжение следует..."));
        assert!(is_hallucination("  ... "));
        assert!(is_hallucination("*Song*"));
        assert!(is_hallucination("[Music]"));
        assert!(is_hallucination("you."));
        assert!(is_hallucination(" The "));
        assert!(!is_hallucination("Okay."));
        assert!(!is_hallucination("Давайте почнемо з архітектури"));
    }

    #[test]
    fn a_boosted_term_must_sound_like_what_was_heard() {
        let v = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let vocab = v(&["ServiceNow", "Flow Studio", "Call Insights"]);
        // The boost corrected a misheard name: kept.
        assert!(boost_is_heard("мы заводим тикет в ServiceNow", "мы заводим тикет в сервис нау", &vocab));
        assert!(boost_is_heard("open it in ServiceNow", "open it in service now", &vocab));
        // The boost put a term over unrelated words: dropped.
        assert!(!boost_is_heard("и дальше Ха Flow Studio сами уточняют", "и дальше они сами уточняют", &vocab));
        assert!(!boost_is_heard("this is Flow Studio in Springfield", "this is Pat in Springfield", &vocab));
        // Nothing added: nothing to check.
        assert!(boost_is_heard("same words", "same words", &vocab));
        assert_eq!(latin("Сервіс Нау"), "servisnau");
    }

    #[test]
    fn flash_attention_only_on_the_macs_gpu() {
        assert!(flash_attention(true, true));
        assert!(!flash_attention(false, true), "the CPU on Apple Silicon");
        assert!(!flash_attention(true, false), "a GPU elsewhere (Windows, an Intel Mac)");
        assert!(!flash_attention(false, false));
        assert!(!context_params(false).flash_attn);
        assert_eq!(context_params(true).flash_attn, cfg!(all(target_os = "macos", target_arch = "aarch64")));
        assert!(context_params(true).use_gpu);
    }

    #[test]
    fn turbo_gets_no_second_detector_and_other_models_get_the_helper() {
        let (_env, _) = crate::paths::testenv::folders("transcribe-detector");
        let path = |n: &str| crate::paths::models_dir().join(models::find(n).unwrap().file);
        std::fs::create_dir_all(crate::paths::models_dir()).unwrap();
        let helper = path(models::HELPER_MODEL);
        assert_eq!(detector_path(&path("small-q8_0")), None, "the helper isn't downloaded");
        std::fs::write(&helper, b"x").unwrap();
        for m in ["small-q8_0", "large-v3-q5_0"] {
            assert_eq!(detector_path(&path(m)), Some(helper.clone()), "{m}");
        }
        for m in ["large-v3-turbo-q8_0", models::HELPER_MODEL] {
            assert_eq!(detector_path(&path(m)), None, "{m}");
        }
    }

    #[test]
    fn turbo_detects_languages_itself() {
        for f in ["ggml-large-v3-turbo-q5_0.bin", "ggml-large-v3-turbo-q8_0.bin"] {
            assert!(!needs_detector(Path::new(f)), "{f}");
        }
        for f in ["ggml-large-v3-q5_0.bin", "ggml-small-q8_0.bin", "my-own-whisper.bin"] {
            assert!(needs_detector(Path::new(f)), "{f}");
        }
    }

    #[test]
    fn ukrainian_and_russian_told_apart_by_letters() {
        assert_eq!(cyrillic_hint("Так я сгоден. Давайте перенесимо зустріч на четвер."), Some("uk"));
        assert_eq!(cyrillic_hint("Первый этап нужно закончить. Есть вопросы?"), Some("ru"));
        assert_eq!(cyrillic_hint("Добрый день, коллеги"), Some("ru"));
        assert_eq!(cyrillic_hint("Так, добре"), None);
        assert_eq!(cyrillic_hint("Good morning"), None);
    }

    #[test]
    fn nemotron_words_become_timed_sentences() {
        let w = |text: &str, start: f64, end: f64| crate::nemo::Word { text: text.into(), start, end };
        let words = [w("Good", 0.4, 0.6), w("morning.", 0.7, 1.2), w("Are", 2.0, 2.1), w("you", 2.2, 2.3), w("there?", 2.4, 2.8), w("Yes", 3.5, 3.7)];
        let s = nemo_sentences(&words);
        assert_eq!(s.len(), 3);
        assert_eq!((s[0].start, s[0].end, s[0].text.as_str()), (0.4, 1.2, "Good morning."));
        assert_eq!((s[1].start, s[1].end, s[1].text.as_str()), (2.0, 2.8, "Are you there?"));
        assert_eq!(s[1].words, [[2.0, 2.1], [2.2, 2.3], [2.4, 2.8]]);
        assert_eq!(s[2].text, "Yes");
    }
}

//! Transcription on another computer: a laptop without a usable GPU sends each utterance to a
//! transcription server and gets the text back, instead of decoding on its own CPU.
//!
//! - Client (`Client`): a server URL (`http://` or `https://`) and an API key, set with
//!   Settings → Connect to a transcription server… or `echoctl server use <url> --key <key>`,
//!   kept in `<config dir>/transcription-server.json`. Only utterances go out (16 kHz mono,
//!   ≤ 25 s each); voice detection, the filters, echo cancellation and the transcript stay on
//!   the laptop. While the server can't be reached this computer decodes (`transcribe.rs`).
//! - Server (`run_in_app`): any Echo with Settings → Transcribe for other computers on (or
//!   `echoctl server share on`) decodes for computers that have its URL and key, with its own
//!   model on its own GPU. Plain HTTP on the local network (put a TLS proxy in front for
//!   https); it keeps no audio and no text and logs no content.
//!
//! Protocol (ours, versioned by the path):
//! - `GET /v1/hello` → `{name, model, engine, version}`
//! - `POST /v1/utterance`: a little-endian u32 length, that many bytes of JSON
//!   (`{languages, prompt}`; an older client's `prev` is ignored), then the samples as 16-bit little-endian PCM →
//!   `{lang_id, lang, segs: [[start, end, text]], words, ranked}` (`ranked`: the detection, when several languages were allowed).
//!
//! Both need `Authorization: Bearer <key>`; a wrong key gets 401 after a 1 s pause.
//!
//! Or any OpenAI-compatible server (`api: openai`: Speaches, vLLM, LocalAI, parakeet-api, …;
//! `echoctl server use <url> --key --openai --model <m>`, Settings → Transcription): each utterance a WAV
//! to `POST /v1/audio/transcriptions` (`verbose_json`, word and segment timestamps), the language
//! pinned when the call has one, else detected by the server and checked against the call's list
//! (a language outside it is asked again pinned); the vocabulary hint as `prompt`, checked against a
//! plain decode as on this computer; Whisper's no-speech rule applied to segments that carry it.
//! What a server gives back (word times, honouring the language) is found when it is connected.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::Config;
use crate::call_language::Ranked;
use crate::transcribe::{Decoded, Engine, EngineState, Piece};
use crate::{models, paths};

const CLIENT_FILE: &str = "transcription-server.json";
const SERVE_FILE: &str = "transcription-serve.json";
pub const DEFAULT_PORT: u16 = 47800;
/// 25 s of 16-bit audio is 800 KB; the rest is the JSON part.
const MAX_BODY: usize = 4 << 20;
const MAX_HEADERS: usize = 16 << 10;
const MAX_CONNECTIONS: usize = 8;
/// The model is unloaded after this long without a request (it holds GPU memory).
const IDLE_UNLOAD: Duration = Duration::from_secs(10 * 60);
/// A queued utterance can wait behind others' on the server.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(180);

// ── client ──────────────────────────────────────────────────────────────────────────────────

/// The server refused the request itself (wrong key, bad request): retrying won't help.
#[derive(Debug)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

#[derive(Serialize, Deserialize, Clone, Default)]
struct ClientSettings {
    url: String,
    key: String,
    /// The server's name when it was connected (for menus).
    #[serde(default)]
    name: Option<String>,
    /// `openai` for an OpenAI-compatible server; none: an Echo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    api: Option<String>,
    /// The model an OpenAI-compatible server is asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    /// What the server was found to give: word timestamps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    words: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hello {
    pub name: String,
    pub model: String,
    pub engine: String,
    pub version: String,
}

pub struct Client {
    url: String,
    key: String,
    agent: ureq::Agent,
    /// An OpenAI-compatible server and the model it is asked for; none: an Echo.
    openai: Option<String>,
}

impl Client {
    pub fn new(url: &str, key: &str) -> Result<Client> {
        let url = url.trim().trim_end_matches('/').to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            bail!("the server URL must start with http:// or https:// (e.g. http://192.168.1.20:{DEFAULT_PORT})");
        }
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(5)))
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .tls_config(ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build())
            .build()
            .into();
        Ok(Client { url, key: key.trim().to_string(), agent, openai: None })
    }

    /// The server this computer uses, if one is set.
    pub fn configured() -> Option<Client> {
        let s = load_client()?;
        let mut c = Client::new(&s.url, &s.key).ok()?;
        if s.api.as_deref() == Some("openai") {
            c.openai = Some(s.model.unwrap_or_default());
        }
        Some(c)
    }

    /// `<url>/v1/<path>`, whether the URL was given with `/v1` or without.
    fn v1(&self, path: &str) -> String {
        if self.url.ends_with("/v1") {
            format!("{}/{path}", self.url)
        } else {
            format!("{}/v1/{path}", self.url)
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    fn check(status: u16, body: &str) -> Result<()> {
        match status {
            200 => Ok(()),
            401 | 403 => Err(Refused("the server rejected the API key".into()).into()),
            400 | 404 | 405 | 413 => Err(Refused(format!("the server refused the request ({status}): {}", body.trim())).into()),
            _ => bail!("the server answered {status}: {}", body.trim()),
        }
    }

    pub fn hello(&self) -> Result<Hello> {
        if let Some(model) = &self.openai {
            return self.openai_hello(model);
        }
        let mut r = self
            .agent
            .get(format!("{}/v1/hello", self.url))
            .header("Authorization", format!("Bearer {}", self.key))
            .config()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .call()
            .with_context(|| format!("could not reach {}", self.url))?;
        let body = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &body)?;
        serde_json::from_str(&body).context("not an Echo transcription server")
    }

    /// An OpenAI-compatible server: its models answer (any server lists them), named by its host.
    fn openai_hello(&self, model: &str) -> Result<Hello> {
        let mut r = self
            .agent
            .get(self.v1("models"))
            .header("Authorization", format!("Bearer {}", self.key))
            .config()
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .call()
            .with_context(|| format!("could not reach {}", self.url))?;
        let body = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &body)?;
        let host = self.url.split("://").nth(1).unwrap_or(&self.url).split('/').next().unwrap_or("").to_string();
        Ok(Hello { name: host, model: model.to_string(), engine: "OpenAI-compatible".into(), version: String::new() })
    }

    /// The models an OpenAI-compatible server lists.
    pub fn openai_models(&self) -> Result<Vec<String>> {
        let mut r = self.agent.get(self.v1("models")).header("Authorization", format!("Bearer {}", self.key)).call().with_context(|| format!("could not reach {}", self.url))?;
        let body = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &body)?;
        let v: serde_json::Value = serde_json::from_str(&body).context("not an OpenAI-compatible server (its /v1/models isn't JSON)")?;
        Ok(v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str().map(str::to_owned)).collect())
    }

    /// One request to `/v1/audio/transcriptions`: the verbose answer.
    fn openai_request(&self, model: &str, chunk: &[f32], language: Option<&str>, prompt: Option<&str>) -> Result<serde_json::Value> {
        let boundary = format!("echo{:x}", Instant::now().elapsed().as_nanos() ^ (chunk.len() as u128 * 2654435761));
        let mut body: Vec<u8> = vec![];
        let mut field = |name: &str, value: &str| {
            body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
        };
        field("model", model);
        field("response_format", "verbose_json");
        field("timestamp_granularities[]", "word");
        field("timestamp_granularities[]", "segment");
        field("temperature", "0");
        if let Some(l) = language {
            field("language", l);
        }
        if let Some(p) = prompt {
            field("prompt", p);
        }
        body.extend(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"utterance.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
        body.extend(wav16k(chunk));
        body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
        let mut r = self
            .agent
            .post(self.v1("audio/transcriptions"))
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", format!("multipart/form-data; boundary={boundary}"))
            .send(&body[..])
            .with_context(|| format!("could not reach {}", self.url))?;
        let text = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &text)?;
        serde_json::from_str(&text).with_context(|| format!("unexpected answer from the server: {}", text.chars().take(200).collect::<String>()))
    }

    /// Decode one utterance on an OpenAI-compatible server (`openai_decode`, one request per `decode`).
    fn openai_utterance(&self, model: &str, chunk: &[f32], allowed: &[i32], prompt: Option<&str>) -> Result<(Option<Ranked>, String, Decoded)> {
        let decode = |lang: Option<&str>, hint: Option<&str>| -> Result<(Option<i32>, Decoded)> {
            let v = self.openai_request(model, chunk, lang, hint)?;
            Ok((lang_of(&v["language"]), pieces(&v)))
        };
        openai_decode(decode, chunk.len() as f64 / 16000.0, allowed, prompt)
    }

    /// Decode one utterance on the server (see `Engine::utterance`): in the one language allowed, or detected there
    /// among several and decoded in its own pick (the transcriber asks again in the call's language when they differ).
    pub fn utterance(&self, chunk: &[f32], allowed: &[i32], prompt: Option<&str>) -> Result<(Option<Ranked>, String, Decoded)> {
        if let Some(model) = &self.openai {
            return self.openai_utterance(model, chunk, allowed, prompt);
        }
        let languages: Vec<&str> = allowed.iter().filter_map(|l| whisper_rs::get_lang_str(*l)).collect();
        let head = serde_json::to_vec(&json!({
            "languages": languages,
            "prompt": prompt,
        }))?;
        let mut body = Vec::with_capacity(4 + head.len() + chunk.len() * 2);
        body.extend((head.len() as u32).to_le_bytes());
        body.extend(&head);
        for s in chunk {
            body.extend(((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
        }
        let mut r = self
            .agent
            .post(format!("{}/v1/utterance", self.url))
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/octet-stream")
            .send(&body[..])
            .with_context(|| format!("could not reach {}", self.url))?;
        let text = r.body_mut().read_to_string()?;
        Self::check(r.status().as_u16(), &text)?;
        let v: Answer = serde_json::from_str(&text).context("unexpected answer from the server")?;
        let ranked = answer_ranked(&v, allowed);
        let mut words = v.words.into_iter();
        let segs = v.segs.into_iter().map(|(a, b, t)| Piece::new(a, b, t, words.next().unwrap_or_default())).collect();
        Ok((ranked, v.lang, segs))
    }
}

/// The server's answer. `words` (per sentence, its words' times) and `ranked` (its detection: each allowed language
/// with its probability, best first; empty when none ran) came later: older servers leave them out, older clients
/// ignore them.
#[derive(Serialize, Deserialize)]
struct Answer {
    lang_id: Option<String>,
    lang: String,
    segs: Vec<(f64, f64, String)>,
    #[serde(default)]
    words: Vec<Vec<[f64; 2]>>,
    #[serde(default)]
    ranked: Vec<(String, f32)>,
}

/// An answer's detection: its `ranked`; from an older server without it, the language it picked
/// (`lang_id`) with 1, when it detected (several languages allowed).
fn answer_ranked(a: &Answer, allowed: &[i32]) -> Option<Ranked> {
    let id = |l: &str| whisper_rs::get_lang_id(l);
    if !a.ranked.is_empty() {
        return Some(a.ranked.iter().filter_map(|(l, p)| Some((id(l)?, *p))).collect());
    }
    (allowed.len() != 1).then(|| a.lang_id.as_deref().and_then(id).map(|l| (l, 1.0)).into_iter().collect())
}

/// 16 kHz mono 16-bit PCM as a WAV file.
fn wav16k(chunk: &[f32]) -> Vec<u8> {
    let data = (chunk.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data as usize);
    w.extend(b"RIFF");
    w.extend((36 + data).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(16000u32.to_le_bytes());
    w.extend(32000u32.to_le_bytes());
    w.extend(2u16.to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend(data.to_le_bytes());
    for s in chunk {
        w.extend(((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    w
}

/// One utterance on an OpenAI-compatible server, `decode` being one request (language, hint): the
/// language pinned when the call has one; else the server's detection (its vote, with 1), asked again
/// pinned in the call's first language when it falls outside the call's list (no vote). From
/// `GUARD_MIN_S` on, the utterance is decoded plain and asked again with the hint only when a term may
/// have been heard (`transcribe::with_hint`); a shorter one is decoded with the hint, unchecked.
fn openai_decode(
    mut decode: impl FnMut(Option<&str>, Option<&str>) -> Result<(Option<i32>, Decoded)>,
    dur: f64,
    allowed: &[i32],
    prompt: Option<&str>,
) -> Result<(Option<Ranked>, String, Decoded)> {
    let code = |id: i32| whisper_rs::get_lang_str(id).map(str::to_owned);
    let pinned: Option<String> = if allowed.len() == 1 { code(allowed[0]) } else { None };
    let checked = prompt.filter(|_| dur >= crate::transcribe::GUARD_MIN_S);
    let first = if checked.is_some() { None } else { prompt };
    let (mut lang_id, mut segs) = decode(pinned.as_deref(), first)?;
    let mut ranked = if pinned.is_none() { Some(lang_id.map(|l| (l, 1.0)).into_iter().collect::<Ranked>()) } else { None };
    if pinned.is_none() && !allowed.is_empty() && lang_id.is_some_and(|l| !allowed.contains(&l)) {
        if let Some(c) = code(allowed[0]) {
            (_, segs) = decode(Some(&c), first)?;
            lang_id = Some(allowed[0]);
            ranked = Some(vec![]);
        }
    }
    let lang = lang_id.and_then(whisper_rs::get_lang_str).unwrap_or("auto").to_string();
    if let Some(hint) = checked {
        let lang_arg = pinned.or_else(|| lang_id.and_then(whisper_rs::get_lang_str).map(str::to_owned));
        segs = crate::transcribe::with_hint(segs, hint, || Ok(decode(lang_arg.as_deref(), Some(hint))?.1))?;
    }
    Ok((ranked, lang, segs))
}

/// A language as servers name it ("en", "english", "English") → whisper's id.
fn lang_of(v: &serde_json::Value) -> Option<i32> {
    let s = v.as_str()?.trim().to_lowercase();
    whisper_rs::get_lang_id(&s).or_else(|| (0..=whisper_rs::get_lang_max_id()).find(|&i| whisper_rs::get_lang_str_full(i).is_some_and(|f| f.eq_ignore_ascii_case(&s))))
}

/// A verbose answer's segments as sentences: Whisper's no-speech rule applied where the segment
/// carries it (no-speech > 0.6, or > 0.2 with a mean log-prob under −1); each segment's words are
/// the answer's words starting within it. No segments: the whole text as one.
fn pieces(v: &serde_json::Value) -> Decoded {
    let words: Vec<(f64, f64)> = v["words"].as_array().into_iter().flatten().filter_map(|w| Some((w["start"].as_f64()?, w["end"].as_f64()?))).collect();
    let segs = v["segments"].as_array().cloned().unwrap_or_default();
    if segs.is_empty() {
        let text = v["text"].as_str().unwrap_or("").trim().to_string();
        if text.is_empty() {
            return vec![];
        }
        let end = words.last().map(|w| w.1).unwrap_or(0.0);
        return vec![Piece::new(0.0, end, text, words.iter().map(|w| [w.0, w.1]).collect())];
    }
    let mut out = vec![];
    for s in segs {
        let (Some(a), Some(b)) = (s["start"].as_f64(), s["end"].as_f64()) else { continue };
        let text = s["text"].as_str().unwrap_or("").trim().to_string();
        let ns = s["no_speech_prob"].as_f64().unwrap_or(0.0);
        let lp = s["avg_logprob"].as_f64().unwrap_or(0.0);
        if text.is_empty() || ns > 0.6 || (ns > 0.2 && lp < -1.0) {
            continue;
        }
        let ws: Vec<[f64; 2]> = s["words"]
            .as_array()
            .map(|a| a.iter().filter_map(|w| Some([w["start"].as_f64()?, w["end"].as_f64()?])).collect())
            .unwrap_or_else(|| words.iter().filter(|w| w.0 >= a - 0.01 && w.0 < b + 0.01).map(|w| [w.0, w.1]).collect());
        out.push(Piece::new(a, b, text, ws));
    }
    out
}

fn load_client() -> Option<ClientSettings> {
    serde_json::from_slice(&std::fs::read(paths::config_dir().join(CLIENT_FILE)).ok()?).ok()
}

/// Write a settings file only this user can read (it holds a key).
fn write_private(name: &str, bytes: &[u8]) -> Result<()> {
    let dir = paths::config_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(name);
    let tmp = dir.join(format!("{name}.tmp"));
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    opts.open(&tmp)?.write_all(bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Connect to a server: check the URL and key, then keep them. Returns what the server said.
pub fn connect(url: &str, key: &str) -> Result<Hello> {
    let c = Client::new(url, key)?;
    let hello = c.hello()?;
    let s = ClientSettings { url: c.url.clone(), key: c.key.clone(), name: Some(hello.name.clone()), ..Default::default() };
    write_private(CLIENT_FILE, &serde_json::to_vec_pretty(&s)?)?;
    Ok(hello)
}

/// What connecting to an OpenAI-compatible server found.
#[derive(Debug, Serialize)]
pub struct OpenAiCheck {
    pub model: String,
    pub models: Vec<String>,
    /// Word timestamps came back (speaker splitting and the hint check use them).
    pub words: bool,
    pub seconds: f64,
}

/// Connect to an OpenAI-compatible server: its models listed (the one asked for, else the first
/// that looks like speech-to-text), a test transcription of a short tone, then kept. `name_it`: how the
/// user names a model where they asked (the CLI's `--model`, the window's model field).
pub fn connect_openai(url: &str, key: &str, model: Option<&str>, name_it: &str) -> Result<OpenAiCheck> {
    let mut c = Client::new(url, key)?;
    let models = c.openai_models()?;
    let model = match model.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => m.to_string(),
        None => stt_model(&models, name_it)?,
    };
    c.openai = Some(model.clone());
    // A second of a quiet tone: enough to show the answer's shape (segments, words).
    let tone: Vec<f32> = (0..16000).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 16000.0).sin() * 0.05).collect();
    let t = Instant::now();
    let v = c.openai_request(&model, &tone, Some("en"), None).with_context(|| format!("the test transcription with {model} failed"))?;
    let words = v.get("words").is_some() || v["segments"].as_array().is_some_and(|s| s.iter().any(|x| x.get("words").is_some()));
    let host = c.url.split("://").nth(1).unwrap_or(&c.url).split('/').next().unwrap_or("").to_string();
    let s = ClientSettings { url: c.url.clone(), key: c.key.clone(), name: Some(host), api: Some("openai".into()), model: Some(model.clone()), words: Some(words) };
    write_private(CLIENT_FILE, &serde_json::to_vec_pretty(&s)?)?;
    Ok(OpenAiCheck { model, models, words, seconds: t.elapsed().as_secs_f64() })
}

/// The first of a server's models that looks like speech-to-text, or why there is none.
fn stt_model(models: &[String], name_it: &str) -> Result<String> {
    models
        .iter()
        .find(|m| ["whisper", "parakeet", "canary", "stt", "asr", "transcri"].iter().any(|k| m.to_lowercase().contains(k)))
        .cloned()
        .ok_or_else(|| anyhow!("the server lists no speech-to-text model ({}); {name_it}", models.join(", ")))
}

/// The server in use, for the window: its address, kind, model, what it gives.
pub fn client_info() -> Option<serde_json::Value> {
    let s = load_client()?;
    Some(json!({"url": s.url, "name": s.name, "api": s.api.unwrap_or_else(|| "echo".into()), "model": s.model, "words": s.words}))
}

/// Stop using the server (transcribe on this computer again).
pub fn disconnect() -> Result<()> {
    match std::fs::remove_file(paths::config_dir().join(CLIENT_FILE)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// The server in use, for menus: "<name> (<url>)". Never touches the network.
pub fn client_label() -> Option<String> {
    let s = load_client()?;
    let what = match (&s.api, &s.model) {
        (Some(a), Some(m)) if a == "openai" => format!(" · {m}"),
        _ => String::new(),
    };
    Some(match s.name {
        Some(n) => format!("{n} ({}){what}", s.url),
        None => format!("{}{what}", s.url),
    })
}

/// `echoctl server status`.
pub fn print_status() -> Result<()> {
    match Client::configured() {
        Some(c) => match c.hello() {
            Ok(h) => println!("transcribing on {} — {} ({}, {}, Echo {})", c.url, h.name, h.model, h.engine, h.version),
            Err(e) => println!("transcribing on {} — not reachable now ({e:#}); this computer decodes meanwhile", c.url),
        },
        None => println!("transcribing on this computer"),
    }
    let s = serve_settings();
    if s.on {
        let state = if crate::watch::running_watcher().is_some() { "" } else { " (when Echo runs)" };
        println!("serving other computers{state}: URL {}  API key {}", serve_url(&s), s.key);
    } else {
        println!("not serving other computers (`echoctl server share on`)");
    }
    Ok(())
}

// ── server ──────────────────────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct ServeSettings {
    pub on: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub key: String,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

pub fn serve_settings() -> ServeSettings {
    std::fs::read(paths::config_dir().join(SERVE_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(ServeSettings { on: false, port: DEFAULT_PORT, key: String::new() })
}

/// Turn serving on or off (the running app picks it up within 2 s). A key is made the first
/// time; `new_key` replaces it (computers using the old one must connect again).
pub fn set_serving(on: bool, new_key: bool) -> Result<ServeSettings> {
    let mut s = serve_settings();
    s.on = on;
    if s.key.is_empty() || new_key {
        s.key = make_key()?;
    }
    write_private(SERVE_FILE, &serde_json::to_vec_pretty(&s)?)?;
    Ok(s)
}

/// 16 letters and digits without look-alikes, in groups of 4: easy to read out and type.
fn make_key() -> Result<String> {
    const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
    let mut raw = [0u8; 16];
    getrandom::fill(&mut raw).map_err(|e| anyhow!("no randomness: {e}"))?;
    let chars: Vec<char> = raw.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect();
    Ok(chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-"))
}

/// Keys compare without case, dashes or spaces (read out, typed by hand).
fn normalize_key(k: &str) -> String {
    k.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

fn key_matches(given: &str, key: &str) -> bool {
    let (a, b) = (normalize_key(given), normalize_key(key));
    !b.is_empty() && a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// This computer's name, for the client's menus.
fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("scutil").args(["--get", "ComputerName"]).output() {
        let n = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    std::env::var("COMPUTERNAME").ok().filter(|n| !n.is_empty()).unwrap_or_else(|| "Echo".into())
}

/// The address other computers on the network reach this one at (the interface the default
/// route uses; a UDP connect sends nothing).
fn lan_ip() -> Option<String> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// The URL to give other computers.
pub fn serve_url(s: &ServeSettings) -> String {
    format!("http://{}:{}", lan_ip().unwrap_or_else(|| "<this computer's address>".into()), s.port)
}

struct Loaded {
    engine: Engine,
    state: EngineState,
    model: String,
    used: Instant,
}

/// The server's model, loaded at the first request; one utterance at a time.
static LOADED: Mutex<Option<Loaded>> = Mutex::new(None);
/// Wrong keys are answered one at a time, each after a pause.
static AUTH_FAIL: Mutex<()> = Mutex::new(());
static CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

/// The app's server thread: follows `transcription-serve.json` (checked every 2 s), listening
/// while it says on. Never returns.
pub fn run_in_app() {
    let mut current: Option<(ServeSettings, TcpListener)> = None;
    loop {
        let want = serve_settings();
        let same = current.as_ref().is_some_and(|(s, _)| *s == want);
        if !same {
            if current.take().is_some() {
                eprintln!("transcription server: stopped");
            }
            if want.on && !want.key.is_empty() {
                match bind(want.port) {
                    Ok(l) => {
                        eprintln!("transcription server: listening at {}", serve_url(&want));
                        current = Some((want.clone(), l));
                    }
                    Err(e) => eprintln!("transcription server: {e:#}"),
                }
            }
        }
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until {
            match &current {
                Some((s, l)) => accept_ready(l, &s.key),
                None => std::thread::sleep(Duration::from_millis(250)),
            }
        }
        unload_if_idle();
        // Not serving and the settings file is gone or off: nothing else to do but keep looking.
        if current.is_none() && !want.on {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

/// `echoctl server run`: serve in the foreground (Ctrl-C stops), whatever the app does.
pub fn run_foreground(port: Option<u16>) -> Result<()> {
    let mut s = serve_settings();
    if s.key.is_empty() {
        s = set_serving(s.on, true)?;
    }
    let port = port.unwrap_or(s.port);
    let l = bind(port)?;
    let s = ServeSettings { port, ..s };
    println!("transcription server at {}  key {}", serve_url(&s), s.key);
    loop {
        accept_ready(&l, &s.key);
        unload_if_idle();
    }
}

fn bind(port: u16) -> Result<TcpListener> {
    let l = TcpListener::bind(("0.0.0.0", port)).with_context(|| format!("port {port} is in use"))?;
    l.set_nonblocking(true)?;
    Ok(l)
}

/// Accept what is waiting (each connection on its own thread), else sleep a moment.
fn accept_ready(l: &TcpListener, key: &str) {
    match l.accept() {
        Ok((stream, _)) => {
            if CONNECTIONS.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
                let _ = respond(&stream, 503, "busy");
                return;
            }
            let key = key.to_string();
            std::thread::spawn(move || {
                if let Err(e) = handle(&stream, &key) {
                    let _ = respond(&stream, 500, &format!("{e:#}"));
                }
                CONNECTIONS.fetch_sub(1, Ordering::SeqCst);
            });
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(100)),
        Err(_) => std::thread::sleep(Duration::from_millis(250)),
    }
}

fn unload_if_idle() {
    if let Ok(mut g) = LOADED.try_lock() {
        if g.as_ref().is_some_and(|l| l.used.elapsed() > IDLE_UNLOAD) {
            *g = None;
            eprintln!("transcription server: model unloaded (idle)");
        }
    }
}

fn respond(mut s: &TcpStream, status: u16, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let ctype = if body.starts_with('{') { "application/json" } else { "text/plain; charset=utf-8" };
    write!(s, "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())?;
    s.flush()?;
    Ok(())
}

fn handle(stream: &TcpStream, key: &str) -> Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    r.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
    let (mut auth, mut length, mut seen) = (String::new(), 0usize, line.len());
    loop {
        let mut h = String::new();
        if r.read_line(&mut h)? == 0 {
            break;
        }
        seen += h.len();
        if seen > MAX_HEADERS {
            return respond(stream, 400, "headers too large");
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((name, value)) = h.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "authorization" => auth = value.trim().strip_prefix("Bearer ").unwrap_or("").to_string(),
                "content-length" => length = value.trim().parse().unwrap_or(usize::MAX),
                _ => {}
            }
        }
    }
    if !key_matches(&auth, key) {
        let _g = AUTH_FAIL.lock().unwrap_or_else(|e| e.into_inner());
        std::thread::sleep(Duration::from_secs(1));
        return respond(stream, 401, "wrong API key");
    }
    match (method.as_str(), path.as_str()) {
        ("GET", "/v1/hello") => {
            let model = Config::load().ok().and_then(|c| c.model).unwrap_or_default();
            let hello = Hello { name: computer_name(), model, engine: crate::gpu::engine_status(), version: env!("CARGO_PKG_VERSION").into() };
            respond(stream, 200, &serde_json::to_string(&hello)?)
        }
        ("POST", "/v1/utterance") => {
            if length > MAX_BODY {
                return respond(stream, 413, "utterance too large");
            }
            let mut body = vec![0u8; length];
            r.read_exact(&mut body)?;
            match decode_request(&body) {
                Ok(answer) => respond(stream, 200, &serde_json::to_string(&answer)?),
                Err(e) if e.downcast_ref::<Refused>().is_some() => respond(stream, 400, &format!("{e:#}")),
                Err(e) => respond(stream, 503, &format!("{e:#}")),
            }
        }
        _ => respond(stream, 404, "not found"),
    }
}

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    languages: Vec<String>,
    // An older client also sends `prev` (its track's last language): ignored, a call has one language.
    prompt: Option<String>,
}

/// Split the body into its JSON part and samples.
fn parse_body(body: &[u8]) -> Result<(Request, Vec<f32>)> {
    let bad = |m: &str| anyhow::Error::new(Refused(m.into()));
    let n = body.get(..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| bad("empty request"))?;
    let head = body.get(4..4 + n).ok_or_else(|| bad("truncated request"))?;
    let req: Request = serde_json::from_slice(head).map_err(|e| bad(&format!("bad request: {e}")))?;
    let pcm = &body[4 + n..];
    if !pcm.len().is_multiple_of(2) {
        return Err(bad("odd number of sample bytes"));
    }
    Ok((req, pcm.as_chunks::<2>().0.iter().map(|&b| i16::from_le_bytes(b) as f32 / i16::MAX as f32).collect()))
}

fn decode_request(body: &[u8]) -> Result<Answer> {
    let (req, audio) = parse_body(body)?;
    let lang = |l: &str| whisper_rs::get_lang_id(l).ok_or_else(|| anyhow::Error::new(Refused(format!("unknown language '{l}'"))));
    let allowed = req.languages.iter().map(|l| lang(l)).collect::<Result<Vec<_>>>()?;
    let mut g = LOADED.lock().unwrap_or_else(|e| e.into_inner());
    let model = Config::load().ok().and_then(|c| c.model).unwrap_or_default();
    if g.as_ref().is_some_and(|l| l.model != model) {
        *g = None; // the active model was changed here
    }
    if g.is_none() {
        let path = models::active_model_path()?;
        if !path.exists() {
            bail!("the server has no speech model downloaded");
        }
        eprintln!("transcription server: loading {model}");
        let engine = Engine::load(&path, crate::transcribe::final_threads(), true)?;
        let state = engine.state()?;
        *g = Some(Loaded { engine, state, model, used: Instant::now() });
    }
    let l = g.as_mut().expect("just loaded");
    l.used = Instant::now();
    // The client keeps the call's language: detecting here, the server decodes in its own pick and says what it heard.
    let (ranked, lang, segs) = l.engine.utterance(&mut l.state, &allowed, req.prompt.as_deref(), &audio, &|r| r.first().map(|x| x.0))?;
    let words = segs.iter().map(|p| p.words.clone()).collect();
    let segs = segs.into_iter().map(|p| (p.start, p.end, p.text)).collect();
    let code = |l: i32| whisper_rs::get_lang_str(l).map(str::to_owned);
    let lang_id = ranked.as_ref().and_then(|r| r.first()).and_then(|x| code(x.0));
    let ranked = ranked.unwrap_or_default().into_iter().filter_map(|(l, p)| Some((code(l)?, p))).collect();
    Ok(Answer { lang_id, lang, segs, words, ranked })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each request's (language, hint).
    type Requests = Vec<(Option<String>, Option<String>)>;

    /// The requests `openai_decode` makes for an utterance whose plain answer is `plain` (a hinted
    /// request answers "We moved it to ServiceNow."; every answer names `heard` as its language), and
    /// the text it keeps.
    fn openai_requests(plain: &str, dur: f64, allowed: &[i32], heard: Option<&str>) -> (Requests, String, String) {
        let heard = heard.and_then(whisper_rs::get_lang_id);
        let mut calls = vec![];
        let (_, lang, segs) = openai_decode(
            |lang, hint| {
                calls.push((lang.map(str::to_owned), hint.map(str::to_owned)));
                let text = if hint.is_some() { "We moved it to ServiceNow." } else { plain };
                Ok((heard, vec![Piece::new(0.0, 1.0, text.to_string(), vec![])]))
            },
            dur,
            allowed,
            Some("ServiceNow, Kubernetes"),
        )
        .unwrap();
        (calls, segs[0].text.clone(), lang)
    }

    #[test]
    fn a_hinted_request_only_when_a_term_may_have_been_heard() {
        let hint = || Some("ServiceNow, Kubernetes".to_string());
        let (calls, text, _) = openai_requests("We will meet at ten.", 3.0, &[], Some("en"));
        assert_eq!(calls, vec![(None, None)], "no term heard: one plain request");
        assert_eq!(text, "We will meet at ten.");
        let (calls, text, _) = openai_requests("We moved it to service now.", 3.0, &[], Some("en"));
        assert_eq!(calls, vec![(None, None), (Some("en".into()), hint())], "plain, then hinted in the language heard");
        assert_eq!(text, "We moved it to ServiceNow.");
        let uk = whisper_rs::get_lang_id("uk").unwrap();
        let (calls, _, _) = openai_requests("We will meet at ten.", 3.0, &[uk], Some("uk"));
        assert_eq!(calls, vec![(Some("uk".into()), None)], "a pinned language: still one plain request");
        let (calls, _, _) = openai_requests("We moved it to service now.", 3.0, &[uk], None);
        assert_eq!(calls, vec![(Some("uk".into()), None), (Some("uk".into()), hint())], "the pin holds when the server names no language");
        let (calls, _, _) = openai_requests("Yes.", 0.5, &[], Some("en"));
        assert_eq!(calls, vec![(None, hint())], "a short utterance: one hinted request, as before");
    }

    #[test]
    fn a_language_outside_the_list_is_asked_again_plain_and_the_hint_follows_only_when_needed() {
        let hint = || Some("ServiceNow, Kubernetes".to_string());
        let (en, uk) = (whisper_rs::get_lang_id("en").unwrap(), whisper_rs::get_lang_id("uk").unwrap());
        let (calls, _, lang) = openai_requests("We moved it to service now.", 3.0, &[en, uk], Some("pl"));
        assert_eq!(calls, vec![(None, None), (Some("en".into()), None), (Some("en".into()), hint())]);
        assert_eq!(lang, "en");
        let (calls, _, _) = openai_requests("We will meet at ten.", 3.0, &[en, uk], Some("pl"));
        assert_eq!(calls, vec![(None, None), (Some("en".into()), None)], "no term heard: no hinted request");
        let (calls, _, _) = openai_requests("Yes.", 0.5, &[en, uk], Some("pl"));
        assert_eq!(calls, vec![(None, hint()), (Some("en".into()), hint())], "a short utterance keeps the hint on the re-ask");
    }

    #[test]
    fn a_detection_comes_back_as_a_vote_from_new_and_old_servers() {
        let (en, uk) = (whisper_rs::get_lang_id("en").unwrap(), whisper_rs::get_lang_id("uk").unwrap());
        let new: Answer = serde_json::from_str(r#"{"lang_id":"uk","lang":"uk","segs":[],"ranked":[["uk",0.6],["en",0.3]]}"#).unwrap();
        assert_eq!(answer_ranked(&new, &[en, uk]), Some(vec![(uk, 0.6), (en, 0.3)]));
        let old: Answer = serde_json::from_str(r#"{"lang_id":"uk","lang":"uk","segs":[]}"#).unwrap();
        assert_eq!(answer_ranked(&old, &[en, uk]), Some(vec![(uk, 1.0)]), "an older server: its pick votes with 1");
        assert_eq!(answer_ranked(&old, &[uk]), None, "one language sent: no detection to count");
        // OpenAI-compatible: the server's language votes; pinned, or outside the list, it doesn't.
        let vote = |allowed: &[i32], heard: &str| openai_decode(|_, _| Ok((whisper_rs::get_lang_id(heard), vec![])), 3.0, allowed, None).unwrap().0;
        assert_eq!(vote(&[en, uk], "uk"), Some(vec![(uk, 1.0)]));
        assert_eq!(vote(&[uk], "uk"), None);
        assert_eq!(vote(&[en, uk], "pl"), Some(vec![]));
    }

    #[test]
    fn openai_answers_become_sentences() {
        let v = serde_json::json!({"language": "english", "text": "Hello there. Budget's fine.",
            "segments": [{"start": 0.0, "end": 1.2, "text": " Hello there.", "no_speech_prob": 0.01, "avg_logprob": -0.2},
                         {"start": 1.3, "end": 2.5, "text": " Budget's fine.", "no_speech_prob": 0.05, "avg_logprob": -0.3},
                         {"start": 2.6, "end": 4.0, "text": " Thank you.", "no_speech_prob": 0.9, "avg_logprob": -0.4}],
            "words": [{"word": "Hello", "start": 0.0, "end": 0.4}, {"word": "there.", "start": 0.5, "end": 1.1}, {"word": "Budget's", "start": 1.3, "end": 1.8}, {"word": "fine.", "start": 1.9, "end": 2.4}]});
        let p = pieces(&v);
        assert_eq!(p.len(), 2, "the no-speech segment is dropped");
        assert_eq!(p[1].text, "Budget's fine.");
        assert_eq!(p[1].words, vec![[1.3, 1.8], [1.9, 2.4]]);
        assert_eq!(lang_of(&v["language"]), whisper_rs::get_lang_id("en"));
        assert_eq!(lang_of(&serde_json::json!("uk")), whisper_rs::get_lang_id("uk"));
        let w = wav16k(&[0.0, 0.5]);
        assert_eq!((&w[..4], w.len()), (&b"RIFF"[..], 48));
    }

    #[test]
    fn no_speech_model_says_how_to_name_one_where_the_user_asked() {
        let models = vec!["llama3".to_string(), "qwen3".to_string()];
        let e = stt_model(&models, "type one in the model field").unwrap_err().to_string();
        assert_eq!(e, "the server lists no speech-to-text model (llama3, qwen3); type one in the model field");
        let both = vec!["llama3".to_string(), "Whisper-1".to_string()];
        assert_eq!(stt_model(&both, "x").unwrap(), "Whisper-1");
    }

    #[test]
    fn keys_match_however_they_are_typed() {
        let k = make_key().unwrap();
        assert_eq!(k.len(), 19);
        assert!(key_matches(&k, &k));
        assert!(key_matches(&k.to_lowercase().replace('-', " "), &k));
        assert!(!key_matches("", &k));
        assert!(!key_matches("ABCD-EFGH-JKMN-PQRT", &k) || k == "ABCD-EFGH-JKMN-PQRT");
        assert!(!key_matches("", ""));
    }

    #[test]
    fn request_body_round_trips() {
        let head = br#"{"languages":["en","uk"],"prev":"uk","prompt":"Echo"}"#;
        let mut body = (head.len() as u32).to_le_bytes().to_vec();
        body.extend(head);
        body.extend(i16::MAX.to_le_bytes());
        body.extend(0i16.to_le_bytes());
        let (req, audio) = parse_body(&body).unwrap();
        assert_eq!(req.languages, ["en", "uk"]);
        // An older client's `prev` is accepted and ignored.
        assert_eq!(audio, [1.0, 0.0]);
        assert!(parse_body(&body[..3]).is_err());
        assert!(parse_body(&body[..body.len() - 1]).is_err());
    }
}

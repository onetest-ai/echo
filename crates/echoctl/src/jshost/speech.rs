//! `ctx.speech.transcribe(file, {words, language})`: an audio file (WAV, and with the `decode` feature webm/opus, ogg/opus,
//! m4a, mp3: `decode.rs`) the user handed the plugin (a handle, checked as
//! `ctx.files.read` checks it), transcribed with Echo's own speech model on the user's language — on this computer, or
//! on the transcription server the user connected (`transcribe::final_transcriber` routes it as it does a call). Run
//! off the plugin's slice, one file at a time, never while Echo records or transcribes a call. Only the host half gets
//! the text; the audio goes nowhere else.

use super::*;

/// The longest audio it transcribes (seconds).
const MAX_SECONDS: f64 = 3600.0;
/// The largest file it opens.
const MAX_BYTES: u64 = 200 << 20;

/// One file at a time across processes — the app's and every `echoctl plugin call`'s — as the other per-plugin locks are
/// (`.locks/` beside the stores): a second asks again once the first is done.
fn one_at_a_time() -> Result<crate::app::FileLock> {
    let dir = paths::state_dir().join("plugins").join(".locks");
    std::fs::create_dir_all(&dir)?;
    crate::app::FileLock::try_take(&dir.join("speech.lock"))?.ok_or_else(|| anyhow!("Echo is transcribing another file for a plugin — try again when it is done"))
}

/// Whether Echo records or transcribes a call now: the recorder's state, or a call's `.transcribing` marker whose
/// process is alive.
fn busy() -> bool {
    crate::watch::running_watcher().and_then(|w| w.recording).is_some()
        || crate::calls::all_calls().iter().any(|d| {
            std::fs::read_to_string(d.join(crate::transcribe_marker())).ok().and_then(|p| p.trim().parse::<u32>().ok()).is_some_and(crate::app::process_alive)
        })
}

/// The most raw samples (frames × channels) it decodes: an hour of 48 kHz mono, so the buffers decoding makes stay
/// within a few GB whatever the file's bit depth and rate (the 200 MB cap bounds the file, not what it decodes to).
const MAX_SAMPLES: u64 = 3600 * 48_000;

/// The file opened once, as a regular file (never waiting on a FIFO put in its place) within the size cap; everything
/// after reads from this one handle.
fn open_within_size(path: &Path) -> Result<std::fs::File> {
    let file = crate::grants::open_regular(path)?;
    let size = file.metadata()?.len();
    if size > MAX_BYTES {
        bail!("the file is {} MB — ctx.speech.transcribe takes at most {} MB", size >> 20, MAX_BYTES >> 20);
    }
    Ok(file)
}

/// Whether the file starts as a WAV does (its read position is put back).
fn is_wav(file: &mut std::fs::File) -> Result<bool> {
    use std::io::{Read, Seek};
    let mut head = [0u8; 12];
    let n = file.read(&mut head)?;
    file.rewind()?;
    Ok(n == 12 && &head[..4] == b"RIFF" && &head[8..] == b"WAVE")
}

/// A WAV within the caps (its length, the memory decoding it takes), else why not.
fn open_wav_within_caps(file: std::fs::File) -> Result<hound::WavReader<std::io::BufReader<std::fs::File>>> {
    let r = hound::WavReader::new(std::io::BufReader::new(file)).map_err(|_| anyhow!("this WAV can't be read"))?;
    let spec = r.spec();
    if !(100..=384_000).contains(&spec.sample_rate) || spec.channels == 0 || spec.bits_per_sample == 0 || spec.bits_per_sample > 32 {
        bail!("this WAV's format ({} Hz, {} channels, {} bits) isn't one ctx.speech.transcribe reads", spec.sample_rate, spec.channels, spec.bits_per_sample);
    }
    let seconds = r.duration() as f64 / spec.sample_rate as f64;
    if seconds > MAX_SECONDS {
        bail!("the audio is {} minutes — ctx.speech.transcribe takes at most {} minutes", (seconds / 60.0).ceil() as u64, (MAX_SECONDS / 60.0) as u64);
    }
    if r.duration() as u64 * spec.channels as u64 > MAX_SAMPLES {
        bail!("the audio is too much to decode ({} channels at {} Hz) — ctx.speech.transcribe takes at most an hour at 48 kHz mono", spec.channels, spec.sample_rate);
    }
    Ok(r)
}

/// A file past its cheap checks (its size, a WAV's header caps), before anything is loaded: decoded after the lock, the
/// busy check and the model, so a refusal of the file never waits for those.
#[cfg(feature = "transcribe")]
enum Input {
    Wav(hound::WavReader<std::io::BufReader<std::fs::File>>),
    #[cfg(feature = "decode")]
    Other(std::fs::File),
}

#[cfg(feature = "transcribe")]
fn open_input(path: &Path) -> Result<Input> {
    let mut file = open_within_size(path)?;
    if is_wav(&mut file)? {
        return Ok(Input::Wav(open_wav_within_caps(file)?));
    }
    #[cfg(not(feature = "decode"))]
    bail!("ctx.speech.transcribe reads WAV files only");
    #[cfg(feature = "decode")]
    Ok(Input::Other(file))
}

/// The input as 16 kHz mono: a WAV by its header, other audio decoded (`decode.rs`, its caps measured on the decoded audio).
#[cfg(feature = "transcribe")]
fn load_audio(input: Input) -> Result<Vec<f32>> {
    match input {
        Input::Wav(r) => crate::transcribe::load_16k_from(r),
        #[cfg(feature = "decode")]
        Input::Other(f) => crate::decode::load_16k(f, MAX_SECONDS),
    }
}

/// `{text, language, segments: [{start, end, text}], words?: [{start, end, word}]}` from decoded `segs` (seconds from
/// the start of the audio); `words` only when asked and the engine timed every sentence's words.
#[cfg(feature = "transcribe")]
fn shape(segs: &[crate::transcribe::Segment], language: &str, words: bool) -> Value {
    let text = segs.iter().map(|s| s.text.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
    let rounded = |x: f64| (x * 1000.0).round() / 1000.0;
    let mut out = json!({
        "text": text,
        "language": language,
        "segments": segs.iter().map(|s| json!({"start": rounded(s.start), "end": rounded(s.end), "text": s.text.trim()})).collect::<Vec<_>>(),
    });
    let timed = !segs.is_empty() && segs.iter().all(|s| !s.words.is_empty());
    if words && timed {
        out["words"] = segs
            .iter()
            .flat_map(|s| s.text.split_whitespace().zip(s.words.iter()).map(|(w, t)| json!({"start": rounded(t[0]), "end": rounded(t[1]), "word": w})))
            .collect();
    }
    out
}

#[cfg(feature = "transcribe")]
pub(super) fn transcribe(id: &str, file: &Value, opts: &Value) -> Result<Value> {
    if uses(id, "speech")? != Value::Bool(true) {
        bail!("{id} didn't ask for “speech” when it was installed");
    }
    let (path, _) = files::own(id, file)?;
    let input = open_input(&path)?;
    let cfg = crate::config::Config::load()?;
    let language = match opts["language"].as_str().filter(|l| !l.is_empty()) {
        Some(l) if !(l.len() <= 8 && l.bytes().all(|b| b.is_ascii_alphabetic())) => bail!("“{}” isn't a language code Echo transcribes in", l.chars().take(20).collect::<String>()),
        Some(l) => Some(whisper_rs::get_lang_id(&l.to_lowercase()).map(|n| (n, whisper_rs::get_lang_str(n).unwrap_or("en"))).ok_or_else(|| anyhow!("“{l}” isn't a language code Echo transcribes in"))?),
        None => None,
    };
    let _one = one_at_a_time()?;
    if busy() {
        bail!("Echo is recording or transcribing a call — a plugin's audio is transcribed once it is done");
    }
    let (tr, _) = crate::transcribe::final_transcriber(&cfg, None, None, crate::transcribe::final_threads(), None)?;
    if let Some((n, _)) = language {
        if n != tr.language() {
            tr.set_language(n);
        }
    }
    let audio = load_audio(input)?;
    let segs = tr.track("speech", &audio)?;
    let code = whisper_rs::get_lang_str(tr.language()).unwrap_or("en");
    Ok(shape(&segs, code, opts["words"].as_bool() == Some(true)))
}

#[cfg(not(feature = "transcribe"))]
pub(super) fn transcribe(id: &str, file: &Value, _opts: &Value) -> Result<Value> {
    if uses(id, "speech")? != Value::Bool(true) {
        bail!("{id} didn't ask for “speech” when it was installed");
    }
    let (path, _) = files::own(id, file)?;
    let _ = (open_within_size(&path)?, is_wav, open_wav_within_caps, one_at_a_time, busy);
    bail!("this Echo is built without a speech model")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WAV as `load_audio` opens it.
    fn open_within_caps(path: &Path) -> Result<hound::WavReader<std::io::BufReader<std::fs::File>>> {
        open_wav_within_caps(open_within_size(path)?)
    }

    fn wav(path: &Path, rate: u32, seconds: u32) {
        let mut w = hound::WavWriter::create(path, hound::WavSpec { channels: 1, sample_rate: rate, bits_per_sample: 16, sample_format: hound::SampleFormat::Int }).unwrap();
        for _ in 0..rate * seconds {
            w.write_sample(0i16).unwrap();
        }
        w.finalize().unwrap();
    }

    fn install(root: &Path, id: &str, uses: Value) {
        let mk = root.join("market").join(id);
        let d = mk.join("plugins").join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("echo-plugin.json"), json!({"id": id, "name": id, "version": "0.1.0", "main": "main.js", "uses": uses}).to_string()).unwrap();
        std::fs::write(d.join("main.js"), "export function apply(ctx) { ctx.command('say', (a, { files }) => ctx.speech.transcribe(a.file || files[0], a.opts)) }").unwrap();
        std::fs::write(mk.join("echo-marketplace.json"), json!({"name": format!("m-{id}"), "plugins": [{"id": id, "source": format!("./plugins/{id}")}]}).to_string()).unwrap();
        let name = plugins::market_add(mk.to_str().unwrap(), Some(&format!("m-{id}"))).unwrap();
        plugins::refresh(&name).unwrap();
        let agreed = plugins::review(id, Some(&name)).unwrap().uses;
        plugins::install(id, Some(&name), &agreed).unwrap();
    }

    fn run(id: &str, a: Value, files: &[Value]) -> std::result::Result<Value, String> {
        for _ in 0..100 {
            match call_with(id, "say", &a, files, CALL_TIMEOUT) {
                Err(e) if e.to_string().contains("no command") || e.to_string().contains("isn't loaded") => std::thread::sleep(Duration::from_millis(50)),
                r => return r.map_err(|e| format!("{e:#}")),
            }
        }
        Err("never loaded".into())
    }

    #[test]
    fn only_a_plugin_that_asked_for_speech_transcribes_and_only_a_file_it_was_handed_before_anything_is_decoded() {
        let (_env, root) = paths::testenv::folders("speech-gate");
        install(&root, "talker", json!({"speech": true}));
        install(&root, "quiet", json!({}));
        let f = root.join("note.wav");
        wav(&f, 8000, 1);
        let mine = files::make("talker", &f).unwrap();
        // It didn't ask: refused, whatever it holds.
        let theirs = files::make("quiet", &f).unwrap();
        let e = run("quiet", json!({}), &[theirs]).unwrap_err();
        assert!(e.contains("didn't ask for “speech”"), "{e}");
        // A path, a made-up id, another plugin's handle: no file of its own.
        let others = files::make("quiet", &f).unwrap();
        for file in ["/etc/hosts", "0123456789abcdef", others["file"].as_str().unwrap()] {
            let e = run("talker", json!({"file": file}), &[]).unwrap_err();
            assert!(e.contains("isn't a file handed to talker"), "{file}: {e}");
        }
        // Its own, a language Echo doesn't know: refused before a model is looked for.
        #[cfg(feature = "transcribe")]
        {
            let e = run("talker", json!({"opts": {"language": "xx-nope"}}), std::slice::from_ref(&mine)).unwrap_err();
            assert!(e.contains("isn't a language code"), "{e}");
        }
        let _ = mine;
        plugins::remove("talker", true).unwrap();
        plugins::remove("quiet", true).unwrap();
    }

    #[test]
    fn a_wav_is_taken_within_its_caps_and_anything_else_is_named() {
        let (_env, root) = paths::testenv::folders("speech-caps");
        let ok = root.join("short.wav");
        wav(&ok, 8000, 3);
        assert_eq!(open_within_caps(&ok).unwrap().duration(), 3 * 8000);
        // Over an hour: refused by its header alone, before any of it is decoded.
        let long = root.join("long.wav");
        wav(&long, 800, 3601);
        let e = open_within_caps(&long).err().unwrap().to_string();
        assert!(e.contains("61 minutes") && e.contains("at most 60 minutes"), "{e}");
        // Not a WAV.
        let mp3 = root.join("talk.mp3");
        std::fs::write(&mp3, b"ID3 not audio samples").unwrap();
        #[cfg(feature = "transcribe")]
        {
            let e = open_input(&mp3).and_then(load_audio).unwrap_err().to_string();
            assert!(e.contains("isn't audio") || e.contains("WAV files only"), "{e}");
        }
        // Over 200 MB: refused by its size, before it is read.
        let big = root.join("big.wav");
        std::fs::File::create(&big).unwrap().set_len(MAX_BYTES + 1).unwrap();
        assert!(open_within_caps(&big).err().unwrap().to_string().contains("at most 200 MB"));
        // An hour of 96 kHz audio fits in the file cap in 8 bits, not in memory: refused too. A rate of 0, never read.
        let mut bytes = std::fs::read(&ok).unwrap();
        bytes[22..24].copy_from_slice(&2u16.to_le_bytes());
        bytes[24..28].copy_from_slice(&48_000u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&96_000u32.to_le_bytes());
        bytes[32..34].copy_from_slice(&2u16.to_le_bytes());
        bytes[34..36].copy_from_slice(&8u16.to_le_bytes());
        bytes[40..44].copy_from_slice(&(48_000u32 * 2 * 1800 + 2).to_le_bytes());
        let dense = root.join("dense.wav");
        std::fs::write(&dense, bytes).unwrap();
        let e = open_within_caps(&dense).err().unwrap().to_string();
        assert!(e.contains("too much to decode"), "{e}");
        let mut bytes = std::fs::read(&ok).unwrap();
        bytes[24..28].copy_from_slice(&0u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&0u32.to_le_bytes());
        let zero = root.join("zero.wav");
        std::fs::write(&zero, bytes).unwrap();
        assert!(open_within_caps(&zero).err().unwrap().to_string().contains("0 Hz"));
    }

    #[test]
    fn a_call_being_transcribed_or_recorded_holds_it_back_and_a_gone_process_holds_nothing() {
        let (_env, _root) = paths::testenv::folders("speech-busy");
        assert!(!busy());
        let call = paths::calls_dir().join("2026-11-17_100000_acme");
        std::fs::create_dir_all(&call).unwrap();
        std::fs::write(call.join("meta.json"), "{}").unwrap();
        std::fs::write(call.join(crate::transcribe_marker()), std::process::id().to_string()).unwrap();
        assert!(busy(), "a live transcription");
        std::fs::write(call.join(crate::transcribe_marker()), "999999999").unwrap();
        assert!(!busy(), "a marker left by a process that is gone");
        std::fs::create_dir_all(crate::paths::state_dir()).unwrap();
        std::fs::write(
            crate::paths::state_dir().join("watch.json"),
            json!({"pid": std::process::id(), "heartbeat": chrono::Utc::now().to_rfc3339(), "app": "teams", "in_call": true, "armed": false, "recording": call.display().to_string()}).to_string(),
        )
        .unwrap();
        assert!(busy(), "a recording");
    }

    #[cfg(feature = "transcribe")]
    #[test]
    fn words_come_only_when_asked_and_when_the_engine_timed_every_sentence() {
        let seg = |t: &str, start: f64, words: Vec<[f64; 2]>| crate::transcribe::Segment::from_piece("speech", 0.0, "en", crate::transcribe::Piece::new(start, start + 1.0, t.into(), words), None);
        let timed = vec![seg("Hello there.", 0.0, vec![[0.0, 0.4], [0.5, 0.9]]), seg("Fine.", 1.2, vec![[1.2, 1.6]])];
        let v = shape(&timed, "en", true);
        assert_eq!(v["text"], "Hello there. Fine.");
        assert_eq!(v["language"], "en");
        assert_eq!(v["segments"][1], json!({"start": 1.2, "end": 2.2, "text": "Fine."}));
        assert_eq!(v["words"][2], json!({"start": 1.2, "end": 1.6, "word": "Fine."}));
        assert_eq!(v["words"].as_array().unwrap().len(), 3);
        assert!(shape(&timed, "en", false).get("words").is_none(), "not asked");
        let untimed = vec![timed[0].clone(), seg("No times here", 1.2, vec![])];
        assert!(shape(&untimed, "en", true).get("words").is_none(), "an engine that gave no times for a sentence gives none");
        assert_eq!(shape(&[], "en", true)["text"], "");
    }

    #[cfg(feature = "decode")]
    #[test]
    fn compressed_audio_of_each_kind_comes_out_as_16_khz_mono_of_its_own_length_and_is_capped_on_what_it_decodes_to() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/audio/compressed");
        for name in ["very-short.opus.webm", "very-short.live.opus.webm", "very-short.opus.ogg", "very-short.m4a", "very-short.mp3", "very-short.stereo48.opus.webm"] {
            let audio = crate::decode::load_16k(std::fs::File::open(dir.join(name)).unwrap(), MAX_SECONDS).unwrap_or_else(|e| panic!("{name}: {e:#}"));
            let seconds = audio.len() as f64 / 16_000.0;
            assert!((9.0..9.8).contains(&seconds), "{name}: {seconds} s");
            // Not silence: the made-up voice is in it, between 1 s and 4.5 s.
            let rms = |a: &[f32]| (a.iter().map(|x| x * x).sum::<f32>() / a.len() as f32).sqrt();
            assert!(rms(&audio[16_000..70_000]) > 0.01, "{name}: heard nothing");
            // The cap is on the decoded audio, whatever the file says.
            let e = crate::decode::load_16k(std::fs::File::open(dir.join(name)).unwrap(), 5.0).unwrap_err().to_string();
            assert!(e.contains("takes at most"), "{name}: {e}");
        }
        // Not audio, and a damaged one: refused with a reason, never a panic.
        let (_env, root) = paths::testenv::folders("speech-decode");
        let junk = root.join("junk.webm");
        std::fs::write(&junk, vec![0x1au8; 4096]).unwrap();
        assert!(crate::decode::load_16k(std::fs::File::open(&junk).unwrap(), MAX_SECONDS).is_err());
        let mut cut = std::fs::read(dir.join("very-short.m4a")).unwrap();
        cut.truncate(cut.len() / 3);
        let cut_path = root.join("cut.m4a");
        std::fs::write(&cut_path, cut).unwrap();
        let _ = crate::decode::load_16k(std::fs::File::open(&cut_path).unwrap(), MAX_SECONDS);
        // An mp3 under a 2 MB ID3v2 tag (cover art): the tag is skipped, never parsed.
        let mut tagged = b"ID3\x03\x00\x00".to_vec();
        let size = 2usize << 20;
        tagged.extend([(size >> 21) as u8 & 0x7f, (size >> 14) as u8 & 0x7f, (size >> 7) as u8 & 0x7f, size as u8 & 0x7f]);
        tagged.extend(vec![0u8; size]);
        tagged.extend(std::fs::read(dir.join("very-short.mp3")).unwrap());
        let tagged_path = root.join("tagged.mp3");
        std::fs::write(&tagged_path, tagged).unwrap();
        let audio = crate::decode::load_16k(std::fs::File::open(&tagged_path).unwrap(), MAX_SECONDS).unwrap();
        assert!((9.0..9.8).contains(&(audio.len() as f64 / 16_000.0)));
        // Through the service's own path: a WAV by its header, anything else decoded.
        assert!(open_input(&dir.join("very-short.mp3")).and_then(load_audio).unwrap().len() > 140_000);
    }

    #[test]
    fn a_second_file_waits_for_the_first_across_processes_by_the_lock_file() {
        let (_env, _root) = paths::testenv::folders("speech-lock");
        let first = one_at_a_time().unwrap();
        let e = one_at_a_time().err().unwrap().to_string();
        assert!(e.contains("transcribing another file"), "{e}");
        drop(first);
        one_at_a_time().unwrap();
    }
}

//! Compressed audio — webm/opus (a browser's MediaRecorder), ogg (Opus), m4a/mp4 (AAC), mp3 — decoded to
//! 16 kHz mono for a plugin's `ctx.speech.transcribe` (Symphonia, pure Rust; Opus through `symphonia-opus`). The caps are
//! measured on what comes out of the decoder, packet by packet, so a header that lies or a tiny file that expands
//! never gets past them, and only the 16 kHz mono result is kept in memory.

use anyhow::{anyhow, bail, Result};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::common::Tier;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::audio::Resampler;
use crate::transcribe::RATE;

/// The codecs this build decodes: Symphonia's enabled ones and Opus.
fn codecs() -> &'static CodecRegistry {
    static REGISTRY: std::sync::OnceLock<CodecRegistry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut r = CodecRegistry::new();
        symphonia::default::register_enabled_codecs(&mut r);
        r.register_audio_decoder_at_tier::<symphonia_opus::OpusDecoder>(Tier::Fallback);
        r
    })
}

/// The file from `start` on: a leading ID3v2 tag (an mp3's title, cover art — Symphonia's own tag reader is left off,
/// its chapter frames recurse without a limit) is skipped here, so the probe finds the audio however large the tag.
struct From {
    file: std::fs::File,
    start: u64,
}

impl std::io::Read for From {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(b)
    }
}

impl std::io::Seek for From {
    fn seek(&mut self, to: std::io::SeekFrom) -> std::io::Result<u64> {
        use std::io::SeekFrom::*;
        let at = match to {
            Start(n) => self.file.seek(Start(self.start + n))?,
            other => self.file.seek(other)?,
        };
        Ok(at.saturating_sub(self.start))
    }
}

impl symphonia::core::io::MediaSource for From {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        self.file.metadata().ok().map(|m| m.len().saturating_sub(self.start))
    }
}

/// Where the audio starts: past an ID3v2 tag when the file begins with one (10 bytes: "ID3", version, flags, a 28-bit
/// size in 7-bit bytes, a footer flag adding 10).
fn audio_start(file: &mut std::fs::File) -> Result<u64> {
    use std::io::{Read, Seek};
    let mut h = [0u8; 10];
    let n = file.read(&mut h)?;
    file.rewind()?;
    if n == 10 && &h[..3] == b"ID3" && h[6..].iter().all(|b| b & 0x80 == 0) {
        let size = h[6..].iter().fold(0u64, |a, b| a << 7 | *b as u64);
        return Ok(10 + size + if h[5] & 0x10 != 0 { 10 } else { 0 });
    }
    Ok(0)
}

/// The most this reads of the file in packets (every track, decoded or not), twice the file cap, and the longest it
/// works: a damaged or hostile container that makes it read the same bytes again and again is stopped, not waited for.
const MAX_PACKET_BYTES: u64 = 400 << 20;
const MAX_WORK: std::time::Duration = std::time::Duration::from_secs(300);
/// The most raw samples (frames × channels) it decodes: an hour of 48 kHz mono, as a WAV's.
const MAX_SAMPLES: u64 = 3600 * 48_000;

/// The audio of `file` as 16 kHz mono, at most `max_seconds` of it (of the decoded audio). Errors say what wasn't
/// readable: a file that isn't audio of a kind listed, more than `max_seconds`, a stream that changes its rate.
pub(crate) fn load_16k(file: std::fs::File, max_seconds: f64) -> Result<Vec<f32>> {
    // A damaged stream can make a decoder panic: that is a file that can't be read, not the app's end.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decode(file, max_seconds))).unwrap_or_else(|_| bail!("this audio couldn't be decoded"))
}

fn decode(mut file: std::fs::File, max_seconds: f64) -> Result<Vec<f32>> {
    let start = audio_start(&mut file)?;
    let began = std::time::Instant::now();
    let mut read = 0u64;
    let mut samples = 0u64;
    let mss = MediaSourceStream::new(Box::new(From { file, start }), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(&Hint::new(), mss, FormatOptions::default(), MetadataOptions::default())
        .map_err(|_| anyhow!("this isn't audio ctx.speech.transcribe reads (WAV, webm/opus, ogg, m4a, mp3)"))?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| anyhow!("this file holds no audio"))?;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or_else(|| anyhow!("this file holds no audio"))?;
    let mut decoder = codecs().make_audio_decoder(params, &AudioDecoderOptions::default()).map_err(|_| anyhow!("the audio's codec isn't one ctx.speech.transcribe reads (Opus, AAC, MP3)"))?;
    let track_id = track.id;
    let mut out: Vec<f32> = vec![];
    let mut rate = 0u32;
    let mut resampler: Option<Resampler> = None;
    let mut frames = 0u64;
    let mut buf: Vec<f32> = vec![];
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(Error::ResetRequired) => bail!("this audio changes its tracks part-way, which ctx.speech.transcribe doesn't read"),
            // A recording cut off (a stream that stopped): what came before is the audio.
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => bail!("the audio is damaged: {e}"),
        };
        read += packet.data.len() as u64;
        if read > MAX_PACKET_BYTES || began.elapsed() > MAX_WORK {
            bail!("this audio takes too long to read — ctx.speech.transcribe doesn't read it");
        }
        if packet.track_id != track_id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(a) => a,
            // A packet that doesn't decode is skipped, as a player does.
            Err(Error::DecodeError(_) | Error::IoError(_)) => continue,
            Err(e) => bail!("the audio can't be decoded: {e}"),
        };
        let (r, ch, n) = (audio.spec().rate(), audio.spec().channels().count(), audio.frames());
        if !(100..=384_000).contains(&r) || ch == 0 {
            bail!("this audio's format ({r} Hz, {ch} channels) isn't one ctx.speech.transcribe reads");
        }
        if rate == 0 {
            rate = r;
            resampler = Some(Resampler::new(r, RATE as u32));
        } else if r != rate {
            bail!("this audio changes its sample rate part-way, which ctx.speech.transcribe doesn't read");
        }
        frames += n as u64;
        samples += (n * ch) as u64;
        if samples > MAX_SAMPLES {
            bail!("the audio is too much to decode ({ch} channels at {r} Hz) — ctx.speech.transcribe takes at most an hour at 48 kHz mono");
        }
        if frames as f64 / rate as f64 > max_seconds {
            bail!("the audio is over {} minutes — ctx.speech.transcribe takes at most {} minutes", (max_seconds / 60.0) as u64, (max_seconds / 60.0) as u64);
        }
        buf.resize(audio.samples_interleaved(), 0.0);
        audio.copy_to_slice_interleaved(&mut buf);
        let mono: Vec<f32> = buf.chunks_exact(ch).map(|f| f.iter().sum::<f32>() / ch as f32).collect();
        out.extend(resampler.as_mut().expect("set with the rate").process(&mono));
    }
    if out.is_empty() {
        bail!("no audio could be decoded from this file");
    }
    Ok(out)
}

---
id: TC-011
title: An English call is transcribed with Parakeet v3 and a Russian one with Whisper, from the downloaded models
priority: high
type: functional
module: transcription
requirements: [F-STT-08, F-STT-02]
tags: [transcription, languages, models]
run: agent
platform: macos
data: small
---

# TC-011: The model follows the call's language

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- Small data set built right before the run (`tests/fixtures/make_data.py <dir> --set small`); every command runs as `target/debug/echoctl …` after `. <dir>/env`
- The speech models and the NeMo-Speech runtime of an installed Echo in the run's config: `mkdir -p "$ECHOCTL_CONFIG_DIR/models"; for f in "$HOME/Library/Application Support/echo/models/"*; do ln -sf "$f" "$ECHOCTL_CONFIG_DIR/models/"; done; ln -sfn /Applications/Echo.app/Contents/Frameworks/nemo-speech "$ECHOCTL_CONFIG_DIR/nemo-speech"`. `large-v3-turbo-q5_0` must be there; `parakeet-tdt-0.6b-v3` is downloaded into the run's models folder in step 1 (714 MB, once per computer when kept in a scratch folder linked the same way)
- The known conversations are built (`tests/fixtures/audio/make_audio.py`, no network needed)

## Test Data

| Call | Built from |
|---|---|
| `<day>_140000-acme-sync-en` | `acme-sync` (6 turns, English), the call's language `en` |
| `<day>_141000-russian-call-ru` | `russian-call` (2 turns, Russian), the call's language `ru` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `echoctl model use large-v3-turbo-q5_0; echoctl model list` (then `echoctl model download parakeet-tdt-0.6b-v3` when it isn't downloaded) | The list has `parakeet-tdt-0.6b-v3` downloaded and runnable (no "not in this build"), recommended; the Whisper models' text says "deprecated"; the active model is turbo q5_0 |
| 2 | `make_audio.py build acme-sync $ECHOCTL_HOME/calls/<day>_140000-acme-sync-en --languages en; echoctl transcribe <that folder>` | Exit 0; "language: en" and "model: parakeet-tdt-0.6b-v3" in its output (not the active turbo); no line about detecting a language |
| 3 | `make_audio.py score <that folder>; head -6 <that folder>/transcript.md` | `pass` true, `wer` ≤ 0.15; the header reads "**Model:** parakeet-tdt-0.6b-v3" |
| 4 | `echoctl model use parakeet-tdt-0.6b-v3; make_audio.py build russian-call $ECHOCTL_HOME/calls/<day>_141000-russian-call-ru --languages ru; echoctl transcribe <that folder>` | Exit 0; "language: ru" and "model: large-v3-turbo-q5_0": a Russian call goes to Whisper even with Parakeet active |
| 5 | `make_audio.py score <that folder>` | `pass` true, `wer` ≤ 0.15, both turns `lang_ok` (ru) |

## Expected Final State
Each call is transcribed in its own language with the downloaded model best at it: English with Parakeet v3, Russian with Whisper turbo, whatever the active model.

## Teardown
- `echoctl model use large-v3-turbo-q5_0` (the run's own config)

## Notes
- The choice is `models::for_language` (measured 2026-11-09 on these conversations, clean and with noise): English → Parakeet v3, else Nemotron (about 15× less GPU than turbo, as accurate or better); another language → the active Whisper model, else turbo (Nemotron and Parakeet make about twice turbo's errors on noisy Russian).
- Replaces "Turbo q8_0 detects its own language" (TC-008): nothing detects a call's language any more.

---
id: TC-008
title: With turbo q8_0 in use, a call in three languages is transcribed by that model alone, each sentence in its own language
priority: medium
type: functional
module: transcription
requirements: [F-STT-02]
tags: [transcription, languages, models]
run: agent
platform: macos
data: small
---

# TC-008: Turbo q8_0 detects its own language

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set built right before the run (`tests/fixtures/make_data.py <dir> --set small`); every command runs as `target/debug/echoctl …` after `. <dir>/env`
- Both turbo models in the run's config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"` (an installed Echo with `large-v3-turbo-q5_0` and `large-v3-turbo-q8_0` downloaded; without q8_0, BLOCKED)
- The known conversations are built (`tests/fixtures/audio/make_audio.py`, no network needed). This case transcribes on purpose: the q8_0 path is what it checks (the prepared recordings use q5_0)

## Test Data

| Call | Built from |
|---|---|
| `<day>_140000-mixed-languages-q8` | `mixed-languages` (31 s, 5 turns: Ukrainian, Russian, English) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `echoctl model use large-v3-turbo-q8_0` | "active model: large-v3-turbo-q8_0" |
| 2 | `make_audio.py build mixed-languages $ECHOCTL_HOME/calls/<day>_140000-mixed-languages-q8` | It prints the folder, "5 turns" |
| 3 | `echoctl transcribe <that folder>` | Exit 0, "flash attention: on" (Apple Silicon), "model: large-v3-turbo-q8_0", "done in …s"; **no** line "language detection: large-v3-turbo-q5_0" (turbo detects its own language; no second turbo is loaded) |
| 4 | `make_audio.py score <that folder>` | `pass` true, `wer` ≤ 0.15, every turn `lang_ok` (uk, ru, uk, uk, en), `transcribed_in_silence` [] |
| 5 | `head -6 <that folder>/transcript.md` | The header reads "**Model:** large-v3-turbo-q8_0" |

## Expected Final State
The call is transcribed by q8_0 alone, each sentence in the language it was spoken in.

## Teardown
- `echoctl model use large-v3-turbo-q5_0` (the run's own config)

## Notes
- Before 2026-11-08 a downloaded q5_0 was loaded beside q8_0 as the language detector, and step 3 printed "language detection: large-v3-turbo-q5_0": a second turbo on the GPU, about 640 MB more peak memory.
- Run 2026-11-08 (branch `feat/whisper-flash-attention`, debug build): PASS — "flash attention: on", no detector line, `wer` 0.056, all five turns `lang_ok`, done in 6 s.

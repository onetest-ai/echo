---
id: TC-008
title: With turbo q8_0 in use and three languages allowed, a call in Ukrainian is transcribed by that model alone, in Ukrainian
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
| `<day>_140000-ukrainian-call-q8` | `ukrainian-call` (3 turns, Ukrainian only), the call's languages en, uk, ru |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | `echoctl model use large-v3-turbo-q8_0` | "active model: large-v3-turbo-q8_0" |
| 2 | `make_audio.py build ukrainian-call $ECHOCTL_HOME/calls/<day>_140000-ukrainian-call-q8 --languages en,uk,ru` | It prints the folder, "3 turns" |
| 3 | `echoctl transcribe <that folder>` | Exit 0, "flash attention: on" (Apple Silicon), "model: large-v3-turbo-q8_0", "done in …s"; **no** line "language detection: large-v3-turbo-q5_0" (turbo detects its own language; no second turbo is loaded); one line "language: uk (from … s of speech in … utterances)" |
| 4 | `make_audio.py score <that folder>` | `pass` true, `wer` ≤ 0.15, every turn `lang_ok` (uk), `transcribed_in_silence` [] |
| 5 | `head -6 <that folder>/transcript.md` | The header reads "**Model:** large-v3-turbo-q8_0" |

## Expected Final State
The call is transcribed by q8_0 alone, in the one language it is in, detected by q8_0 itself.

## Teardown
- `echoctl model use large-v3-turbo-q5_0` (the run's own config)

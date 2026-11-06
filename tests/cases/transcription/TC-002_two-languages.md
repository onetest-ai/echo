---
id: TC-002
title: Each sentence stays in the language it was spoken in
priority: high
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, languages]
external_id: TC-TRN-003
---

# TC-002: Each sentence stays in the language it was spoken in

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; the shell has run `. <dir>/env`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0`
- The clips are in `tests/fixtures/audio/clips/` (no network needed)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | mixed-languages: 5 turns, uk/ru/en |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl config --languages uk,ru,en` | Exit code 0 |
| 2 | Run `python3 tests/fixtures/audio/make_audio.py build mixed-languages "$ECHOCTL_HOME/calls/2026-11-06_100000-initech-planning"` | It prints the call folder, its length and its turns |
| 3 | Run `target/debug/echoctl transcribe 2026-11-06_100000-initech-planning` | "done in …"; the folder has `transcript.md` and `segments.json`, and no `.wav` files |
| 4 | Run `python3 tests/fixtures/audio/make_audio.py score "$ECHOCTL_HOME/calls/2026-11-06_100000-initech-planning"` | Exit code 0; `"pass": true` |
| 5 | In the score, read each turn's `lang_ok` | true for all five turns (uk, ru, uk, uk, en); no turn translated |

## Expected Final State
A call switching between Ukrainian, Russian and English keeps every sentence in its own language, at most 15% word errors.

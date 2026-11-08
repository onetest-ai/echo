---
id: TC-002
title: Each sentence stays in the language it was spoken in
priority: high
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, languages, recordings]
external_id: TC-TRN-003
---

# TC-002: Each sentence stays in the language it was spoken in

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `mixed-languages` is the captured mixed-languages call transcribed once with the workspace languages uk, ru, en and no pin of its own
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`

## Test Data

| Field | Value |
|-------|-------|
| Recording | `mixed-languages`: 5 turns, uk/ru/en, title "TRN-002 three languages" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -m1 "(languages:" "$(python3 tests/fixtures/recordings.py path)/mixed-languages/transcribe.out"` | "(languages: uk, ru, en)" (or that set in another order) |
| 2 | `CALL="$(python3 tests/fixtures/recordings.py copy mixed-languages --title "TRN-002 three languages")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; top-level `wer` at most 0.15 |
| 3 | In the score, read each turn's `lang_ok` | true for all five turns (uk, ru, uk, uk, en); no turn translated |

## Expected Final State
A call switching between Ukrainian, Russian and English keeps every sentence in its own language, at most 15% word errors over the whole call.

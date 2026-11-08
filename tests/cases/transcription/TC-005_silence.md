---
id: TC-005
title: Silence produces no made-up text
priority: medium
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, recordings]
external_id: TC-TRN-006
---

# TC-005: Silence produces no made-up text

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `long-silence` was captured and transcribed once
- Test data built by `tests/fixtures/make-data.sh <dir>`; every command after `. <dir>/env`

## Test Data

| Field | Value |
|-------|-------|
| Recording | `long-silence`: 3 turns with 20 s of silence, title "TRN-005 silence" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `CALL="$(python3 tests/fixtures/recordings.py copy long-silence --title "TRN-005 silence")"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; top-level `wer` at most 0.15 |
| 2 | In the score, read `transcribed_in_silence` | An empty list: nothing in the 20 s without speech |

## Expected Final State
Twenty seconds without speech produce no text; the words around them are transcribed.

## Notes
- `recording/TC-017` reads the same recording's tracks and capture log (no restart in the silence); the text is checked here only.

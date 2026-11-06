---
id: TC-001
title: The transcript has the words, speakers and order of the call
priority: critical
type: functional
requirements: [F-TR-01, F-STT-04, F-STT-03]
module: transcription
run: agent
platform: any
tags: [transcription]
external_id: TC-TRN-001
---

# TC-001: The transcript has the words, speakers and order of the call

**Module:** Transcription | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; the shell has run `. <dir>/env`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0`
- The clips are in `tests/fixtures/audio/clips/` (no network needed)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync: 6 turns, 36 s, Me + Dana (English) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl config --languages en` | Exit code 0 |
| 2 | Run `python3 tests/fixtures/audio/make_audio.py build acme-sync "$ECHOCTL_HOME/calls/2026-11-06_090000-acme-weekly-sync"` | It prints the call folder, its length and its turns |
| 3 | Run `target/debug/echoctl transcribe 2026-11-06_090000-acme-weekly-sync` | "done in …"; the folder has `transcript.md` and `segments.json`, and no `.wav` files |
| 4 | Run `python3 tests/fixtures/audio/make_audio.py score "$ECHOCTL_HOME/calls/2026-11-06_090000-acme-weekly-sync"` | Exit code 0; `"pass": true` |
| 5 | Read `transcript.md` | Turns in time order, the user's as Me and Dana's as Remote; "Berlin first" and "SSO checklist" spelled as said |

## Expected Final State
The six turns of the Acme call come out with at most 15% word errors, each with the right speaker and language, in order; the audio is gone.

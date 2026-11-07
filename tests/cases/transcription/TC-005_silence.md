---
id: TC-005
title: Silence produces no made-up text
priority: medium
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription]
external_id: TC-TRN-006
---

# TC-005: Silence produces no made-up text

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; the shell has run `. <dir>/env`
- A models folder holding the model: linked from an installed Echo's `~/Library/Application Support/echo/models` (`ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`), or downloaded into the test config with `target/debug/echoctl model download large-v3-turbo-q5_0`; then `target/debug/echoctl model use large-v3-turbo-q5_0`
- The clips are in `tests/fixtures/audio/clips/` (no network needed)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | long-silence: 3 turns with 20 s of silence |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl config --languages en` | Exit code 0 |
| 2 | Run `python3 tests/fixtures/audio/make_audio.py build long-silence "$ECHOCTL_HOME/calls/2026-11-06_120000-waiting-for-the-demo"` | It prints the call folder, its length and its turns |
| 3 | Run `target/debug/echoctl transcribe 2026-11-06_120000-waiting-for-the-demo` | "done in …"; the folder has `transcript.md` and `segments.json`, and no `.wav` files |
| 4 | Run `python3 tests/fixtures/audio/make_audio.py score "$ECHOCTL_HOME/calls/2026-11-06_120000-waiting-for-the-demo"` | Exit code 0; `"pass": true` (the 15% word-error limit is on the whole call: the score's top-level `wer` is at most `0.15`, not each turn's `wer`) |
| 5 | In the score, read `transcribed_in_silence` | An empty list: nothing in the 20 s without speech |

## Expected Final State
Twenty seconds without speech produce no text; the words around them are transcribed.

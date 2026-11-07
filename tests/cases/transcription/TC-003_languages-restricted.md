---
id: TC-003
title: With one language set, an English call comes out in English only
priority: high
type: functional
requirements: [F-STT-02, F-SET-07]
module: transcription
run: agent
platform: macos
tags: [transcription, languages]
external_id: TC-TRN-004
---

# TC-003: With one language set, an English call comes out in English only

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; the shell has run `. <dir>/env`
- A models folder holding the model: linked from an installed Echo's `~/Library/Application Support/echo/models` (`ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`), or downloaded into the test config with `target/debug/echoctl model download large-v3-turbo-q5_0`; then `target/debug/echoctl model use large-v3-turbo-q5_0`
- The clips are in `tests/fixtures/audio/clips/` (no network needed)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | two-remotes: 6 turns, Me + Dana + Omar on one remote track |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl config --languages en` | Exit code 0 |
| 2 | Run `python3 tests/fixtures/audio/make_audio.py build two-remotes "$ECHOCTL_HOME/calls/2026-11-06_110000-globex-pilot-kickoff"` | It prints the call folder, its length and its turns |
| 3 | Run `target/debug/echoctl transcribe 2026-11-06_110000-globex-pilot-kickoff` | "done in …"; the folder has `transcript.md` and `segments.json`, and no `.wav` files |
| 4 | Run `python3 tests/fixtures/audio/make_audio.py score "$ECHOCTL_HOME/calls/2026-11-06_110000-globex-pilot-kickoff"` | Exit code 0; `"pass": true` (the 15% word-error limit is on the whole call: the score's top-level `wer` is at most `0.15`, not each turn's `wer`) |
| 5 | Run `python3 -c "import json,os;print({s['lang'] for s in json.load(open(os.environ['ECHOCTL_HOME']+'/calls/2026-11-06_110000-globex-pilot-kickoff/segments.json'))})"` | `{'en'}` |

## Expected Final State
With English as the only language, every segment of a two-voice remote call is English, with at most 15% word errors over the whole call.

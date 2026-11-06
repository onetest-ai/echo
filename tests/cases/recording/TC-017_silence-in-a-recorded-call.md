---
id: TC-017
title: Twenty seconds of silence in a recorded call give no text and no restart
priority: medium
type: functional
requirements: [F-STT-03, F-REC-07]
module: recording
run: agent
platform: macos
tags: [recording, test-call, silence]
---

# TC-017: Twenty seconds of silence in a recorded call give no text and no restart

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps

## Test Data

| Field | Value |
|-------|-------|
| Conversation | long-silence: Me, 20 s of nobody speaking, Dana, Me (36 s) |
| Title | REC-017 silence |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build long-silence "$CONV"` | It prints the folder, the length and the turns of long-silence; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 42 --title "REC-017 silence" 2> "$D/capture.err"; wait` | Exit code 0; the last line printed is the call folder `…-rec-017-silence` |
| 3 | Read `meta.json` in that folder | Both tracks (`mic.wav`, `remote.wav`) have `frames` ÷ `sample_rate` within 1.0 s of each other, and neither has an event of kind `restarting` or `write_failed` |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-017-silence)"; cp "$CONV/expected.json" "$CALL/"; target/debug/echoctl transcribe "$CALL" --keep-audio` | "done in …"; `transcript.md` and `segments.json` exist |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-017-silence)"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit code 0; `"pass": true`; `transcribed_in_silence` is an empty list (nothing between 6 s and 26 s); the three turns have the right speakers (Me, Remote, Me) |

## Expected Final State
The silent stretch is in the tracks as silence, the transcript has nothing there, and the words around it are right.

## Teardown
- Nothing to undo: the call lives in the run's own data folder

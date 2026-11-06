---
id: TC-003
title: No audio is kept once a call is transcribed
priority: critical
type: functional
requirements: [F-PRIV-03]
module: privacy
run: agent
platform: macos
tags: [privacy, audio, test-call]
external_id: TC-PRIV-003, TC-TRN-002
---

# TC-003: No audio is kept once a call is transcribed

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- `keep_audio` is off (the default of the test data): `target/debug/echoctl config` prints `"keep_audio": false`

## Test Data

| Field | Value |
|-------|-------|
| Conversation | very-short (9 s) |
| Title | REC-P003 no audio |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$CONV"` | It prints the folder, the length and the turns of very-short; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 16 --title "REC-P003 no audio" 2> "$D/capture.err" & CAP=$!; sleep 8; ls "$ECHOCTL_HOME"/calls/*-rec-p003-no-audio; wait` | While the call records, the folder lists `mic.wav` and `remote.wav` (audio exists while recording) |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-p003-no-audio)"; target/debug/echoctl transcribe "$CALL"; ls "$CALL"` | "done in …"; the folder lists `meta.json`, `segments.json` and `transcript.md`, and no `.wav` |
| 4 | Run the prelude, then `find "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR" -name '*.wav'` | Nothing is printed (audio exists only while a call records or transcribes) |

## Expected Final State
Once transcribed, only text stays: not one `.wav` under the data folder or the config folder.

## Teardown
- Nothing to undo: the call lives in the run's own data folder

## Notes
- `transcribe --keep-audio`, used by the other recording cases to inspect tracks, keeps the files on purpose; this case never uses it.

---
id: TC-016
title: Stopping a capture by hand (Ctrl-C) keeps what was said so far
priority: high
type: functional
requirements: [F-REC-05, F-REC-07]
module: recording
run: agent
platform: macos
tags: [recording, test-call, stop]
---

# TC-016: Stopping a capture by hand (Ctrl-C) keeps what was said so far

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync, stopped by SIGINT about 20 s into the recording |
| Title | REC-016 stop |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --title "REC-016 stop" 2> "$D/capture.err" & CAP=$!; sleep 20; kill -INT $CAP; wait` | `capture.err` says it is recording; after the `kill -INT` the process ends by itself (the call goes on until it is over, about 16 s more) and `capture.err` has a `mic.wav` line and a `remote.wav` line with seconds between 18 and 26 |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-016-stop)"; cat "$CALL/meta.json"` | `ended_at` is set and `stop_reason` is "user" (not "duration"); both tracks are listed with `frames` above 0 |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-016-stop)"; target/debug/echoctl transcribe "$CALL"` | "done in …"; `transcript.md` exists and no `.wav` is left in the folder |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-016-stop)"; grep -c "checklist" "$CALL/transcript.md"; grep -c -e "Madrid" -e "training room" "$CALL/transcript.md"` | The first count is 1 or more (said at 6 s to 18 s); the second is 0 (said after 24 s, after the stop) |

## Expected Final State
A capture ended by Ctrl-C has `stop_reason` "user", both tracks closed, and a transcript that ends where it was stopped.

## Teardown
- Nothing to undo: the call lives in the run's own data folder

## Notes
- Run on 2026-11-07 (S-18): PASS (`capture` exits 134 after saving; see `recording/TC-014`).
- This is the capture command's version of Stop. The app's Stop (`echoctl stop`, the pill's "Stop recording?") is `recording/TC-015`; the pill's confirmation is person-run in `recording/TC-005`.

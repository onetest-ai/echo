---
id: TC-011
title: A crash while recording is repaired, and transcribed up to the crash
priority: low
type: functional
requirements: [F-REC-11]
module: recording
run: agent
platform: macos
tags: [recording, lifecycle, test-call]
external_id: TC-REC-012
---

# TC-011: A crash while recording is repaired, and transcribed up to the crash

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s); `echoctl capture` killed with SIGKILL 15 s into the recording |
| Title | REC-011 crash |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --title "REC-011 crash" 2> "$D/capture.err" & CAP=$!; sleep 15; kill -9 $CAP; wait` | The capture process is gone after the `kill -9` and printed no call folder; the call (TestCall) plays on to its end |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; ls "$CALL"; cat "$CALL/meta.json"` | The folder holds `meta.json`, `mic.wav` and `remote.wav`, no `transcript.md`; `meta.json` has `ended_at` null and `stop_reason` null (the recording was never closed) |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; target/debug/echoctl transcribe "$CALL"; cat "$CALL/meta.json"; ls "$CALL"` | "done in …" with no error; `meta.json` now has `ended_at` set and `stop_reason` "recovered"; the folder has `transcript.md` and `segments.json` and no `.wav` |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; grep -c "checklist" "$CALL/transcript.md"; grep -c -e "Madrid" -e "training room" "$CALL/transcript.md"` | The first count is 1 or more (said before 12 s); the second is 0 (said after 24 s, after the crash) |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; cat "$CALL/segments.json"` | Every segment's `start` is below 17.0: nothing is dated after the crash |

## Expected Final State
The unfinished recording was closed as "recovered", its audio repaired and transcribed up to the crash, and no audio kept.

## Teardown
- Nothing to undo: the call lives in the run's own data folder

## Notes
- Repair here goes through `transcribe` (`watch::close_unfinished`, called from `transcribe.rs`). The repair when the app starts again (event `recording_recovered`, `watch.rs`) is not covered by an agent case yet; it needs the second app instance.
- The original step 3, a failed transcription showing its error instead of retrying forever, needs a way to make a transcription fail and is not covered here.
- `meta.json` of a recovered call keeps `tracks` empty: the tracks are written into it only when a recording closes normally (`recorder.rs`, `finish`).

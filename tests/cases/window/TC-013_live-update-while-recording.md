---
id: TC-013
title: The window updates on its own when a recording ends
priority: medium
type: functional
requirements: [F-CALLS-18]
module: window
run: agent
platform: macos
tags: [calls, recording, test-call]
external_id: TC-WIN-007
---

# TC-013: The window updates on its own when a recording ends

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- The window is served on the same test data at `{{base_url}}` (`tests/fixtures/serve.sh up`, or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Call that stays open | Initech planning (test data), on its Transcript tab |
| Recorded call | very-short (9 s), title "WIN-013 live update" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[role=option][data-call$="initech-planning"]`, open its Transcript tab | The two turns of Initech planning are shown; the tab Transcript is selected |
| 2 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$CONV"` | It prints the folder, the length and the turns of very-short; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 3 | Run the prelude, then `nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 12 --title "WIN-013 live update" > "$D/win013.out" 2> "$D/win013.err"; wait' > /dev/null 2>&1 &` | Returns at once; the call and the capture run in the background |
| 4 | Without reloading, wait up to 15 s (`browser_wait_for` the text "WIN-013 live update") | A new row `[role=option][data-call$="win-013-live-update"]` appears in the Calls list without a reload |
| 5 | Wait until `cat "$D/win013.out"` prints a folder path (about 15 s after step 3), then run the prelude and `target/debug/echoctl transcribe "$(cat "$D/win013.out")"`, and click the new row | "done in …"; the new call opens, and within 15 s its Transcript tab shows the line "is the call at three still on" (Whisper's punctuation may differ: "Quick one. Is …") and Me's answer, without a reload |
| 6 | Look at the Initech planning row and, after clicking it, its tab | The row is still in the list; it opens on Transcript as before (the refresh did not reset the tab you had chosen) |

## Expected Final State
A call recorded and transcribed behind the window's back shows up and fills in by itself; what was open stays as it was.

## Notes
- Run on 2026-11-07 (S-17): PASS. `capture` prints the folder on stdout and aborts at exit with 134 (see `recording/TC-014`), so its backtrace goes to stderr: keep the two apart. 12 s keeps the capture close to the 9 s call (after the hang-up the mic falls back to the room).
- Original step 2 (scroll position and selected text of the other call) is not scripted: Initech planning has two turns, nothing to scroll. The tab check stands in for it.

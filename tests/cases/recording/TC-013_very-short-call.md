---
id: TC-013
title: A very short call gets a transcript and no automatic minutes
priority: low
type: functional
requirements: [F-MIN-08]
module: recording
run: agent
platform: macos
tags: [recording, minutes, test-call]
external_id: TC-REC-013
---

# TC-013: A very short call gets a transcript and no automatic minutes

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- The window is served on the same test data at `{{base_url}}` (`tests/fixtures/serve.sh up` or an already running scenario server), the browser open on it

## Test Data

| Field | Value |
|-------|-------|
| Conversation | very-short: Dana, then Me (9 s) |
| Title | REC-013 very-short |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$CONV"` | It prints the folder, the length and the turns of very-short; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --auto-minutes true` | Exit code 0; the printed config has `auto_minutes` true (it is set before the call so the call counts) |
| 3 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 12 --title "REC-013 very-short" 2> "$D/capture.err"; wait` | Exit code 0; the last line printed is the call folder `…-rec-013-very-short` |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-013-very-short)"; cp "$CONV/expected.json" "$CALL/"; target/debug/echoctl transcribe "$CALL"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | "done in …"; the score exits 0 with `"pass": true`; `transcript.md` holds "Quick one: is the call at three still on?" as Remote and Me's answer |
| 5 | Navigate to `{{base_url}}/`, then to `{{base_url}}/#calls` and click the row `[role=option][data-call$="rec-013-very-short"]` ("REC-013 very-short") | The call opens; its length reads "1 min" (rounded up) |
| 6 | Open the Minutes tab | The heading "No minutes written automatically" and the line "The call is too thin for them (shorter than 3 minutes)"; a "Write minutes" button is shown (not pressed); no error text |

## Expected Final State
A 9-second call has a transcript and the Minutes tab says no minutes are written automatically because it is shorter than 3 minutes, with no error.

## Teardown
- Run the prelude, then `target/debug/echoctl config --auto-minutes false`

## Notes
- Run on 2026-11-07 (S-17), with `--seconds 16`: steps 5–6 PASS; step 4 FAIL — `transcribed_in_silence` held a line at 14 s, said in the room after the call: once TestCall hung up (#140), the mic track reopened on the Mac's microphone (see `recording/TC-014`). 12 s leaves less room for that; the line is a defect of the mic fallback, not of this case.
- Per `calls::gate`, every call under 3 minutes is gated, so the acme-sync test call (36 s) would show the same text.

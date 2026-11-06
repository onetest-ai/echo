---
id: TC-002
title: Record a call already in progress
priority: critical
type: functional
requirements: [F-REC-01]
module: recording
run: agent
platform: macos
tags: [recording, second-instance]
external_id: TC-REC-002
---

# TC-002: Record a call already in progress

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s); armed 16 s after the dial |
| Arm title | REC-002 mid call |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns of acme-sync; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 8; target/debug/echoctl status; sleep 8; target/debug/echoctl arm --title "REC-002 mid call"; sleep 6; target/debug/echoctl status; wait` | At 8 s: `watcher.in_call` true, `watcher.recording` null, `armed` null. arm prints "armed — call in progress, recording starts within a second". At 22 s: `watcher.recording` is a folder path ending `-rec-002-mid-call` |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-002-mid-call)"; for i in $(seq 40); do [ -f "$CALL/transcript.md" ] && break; sleep 3; done; ls "$CALL"` | Within 2 minutes the folder has `transcript.md` and `segments.json`, and no `.wav` file (the app transcribed it and deleted the audio) |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-002-mid-call)"; grep -c -e "single sign-on" -e "SSO checklist" "$CALL/transcript.md"` | 0: what was said in the first 12 s, before the arm, is not there |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-002-mid-call)"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"` | 2 or more: what was said after the arm is there |
| 7 | Run the prelude, then `grep -o '"event":"[a-z_]*"' "$ECHOCTL_HOME/.echo/events.jsonl"` | `call_started` comes before `recording_started`, and there is one `recording_started` only |

## Expected Final State
A call recorded from the middle covers only what was said after the arm.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- The original steps used the menu item "Record this call"; `echoctl arm` during a call is the same request (`watch::arm`).

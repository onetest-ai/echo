---
id: TC-003
title: Cancelling an arm records nothing
priority: high
type: functional
requirements: [F-REC-02, F-REC-03]
module: recording
run: agent
platform: macos
tags: [recording, arm, second-instance]
external_id: TC-REC-003
---

# TC-003: Cancelling an arm records nothing

**Module:** Recording | **Priority:** High | **Type:** Functional

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
| Conversation | very-short (9 s) |
| Arm title | REC-003 cancelled |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$CONV"` | It prints the folder, the length and the turns of very-short; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-003 cancelled"`, then `target/debug/echoctl status` | `armed` has `title` "REC-003 cancelled" |
| 4 | Run the prelude, then `target/debug/echoctl stop`, then `target/debug/echoctl status` | stop prints "disarmed (nothing was recording)"; `armed` is null and `watcher.armed` false |
| 5 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 5; target/debug/echoctl status; wait` | At 5 s `watcher.in_call` is true (the watcher sees the call) and `watcher.recording` is null |
| 6 | Run the prelude, then `sleep 15; ls "$ECHOCTL_HOME/calls"; grep -c recording_started "$ECHOCTL_HOME/.echo/events.jsonl"` | No folder ends in `rec-003-cancelled`; the count is 0 |

## Expected Final State
Cancel disarms; the call that follows is seen but not recorded.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- The menu's "Cancel" and `echoctl stop` with nothing recording do the same (`watch::stop` removes `armed.json`).

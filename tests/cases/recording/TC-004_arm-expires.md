---
id: TC-004
title: An arm expires after 15 minutes
priority: low
type: functional
requirements: [F-REC-02]
module: recording
run: agent
platform: macos
tags: [recording, arm, second-instance]
external_id: TC-REC-004
---

# TC-004: An arm expires after 15 minutes

**Module:** Recording | **Priority:** Low | **Type:** Functional

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
| Arm title | REC-004 expiring |
| Age given to the arm | 16 minutes |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$CONV"` | It prints the folder, the length and the turns of very-short; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-004 expiring"` | arm prints "armed — recording starts when the call begins (expires in 15 min)" |
| 4 | Run the prelude, then (this edit of the arm's time replaces waiting 16 minutes) `sed -i '' "s/\"armed_at\": *\"[^\"]*\"/\"armed_at\":\"$(date -u -v-16M +%Y-%m-%dT%H:%M:%SZ)\"/" "$ECHOCTL_HOME/.echo/armed.json"; sleep 4; target/debug/echoctl status` | `armed` is null within 4 s and the run's `.echo/events.jsonl` ends with an `arm_expired` event with `title` "REC-004 expiring" |
| 5 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 5; target/debug/echoctl status; wait` | At 5 s `watcher.in_call` is true and `watcher.recording` is null |
| 6 | Run the prelude, then `sleep 15; ls "$ECHOCTL_HOME/calls"` | No folder ends in `rec-004-expiring` |

## Expected Final State
An arm older than 15 minutes is dropped and the next call is not recorded.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Only the arm's time is edited; the 15 minutes themselves are `ARM_EXPIRES` in `watch.rs`. A person can still check the full wait once.

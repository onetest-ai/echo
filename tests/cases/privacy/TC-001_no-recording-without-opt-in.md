---
id: TC-001
title: Nothing records without opting in
priority: critical
type: functional
requirements: [F-PRIV-01, F-REC-03]
module: privacy
run: agent
platform: macos
tags: [privacy, opt-in, second-instance]
external_id: TC-PRIV-001
---

# TC-001: Nothing records without opting in

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- Echo idle: not armed, nothing scheduled, no series set to record always (true of the test data)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | very-short (9 s) |
| Mode | "Only when I click Record" (`config --unscheduled manual`, set by step 2): no dialog opens |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$CONV"` | It prints the folder, the length and the turns of very-short; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `ls "$ECHOCTL_HOME/calls" > "$D/before.txt"; wc -l < "$D/before.txt"; "$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & $W 15 in-call; target/debug/echoctl status; wait` | Within 15 s `watcher.in_call` is true (the watcher sees the call), `watcher.recording` null, `watcher.armed` false, `armed` null |
| 4 | Run the prelude, then `$W 30 event call_ended; sleep 12; ls "$ECHOCTL_HOME/calls" > "$D/after.txt"; diff "$D/before.txt" "$D/after.txt"; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -c call_started; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -c recording_started` | The folder list is unchanged (diff prints nothing); `call_started` count is 1, `recording_started` count is 0 (12 s after the call's end: past the watcher's 10 s grace, when a recording would have closed) |
| 5 | Run the prelude, then `tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep call_started` | The event says `"armed":false` and `"ask":false` |
| 6 | Run the prelude, then `find "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR" -name '*.wav' -newer "$D/before.txt"` | Nothing is printed (`-newer` leaves out audio earlier stages of a scenario kept with `--keep-audio`) |

## Expected Final State
A test call came and went on an idle Echo; no call was recorded, no folder was added and no audio exists.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Run on 2026-11-07 (S-18): PASS.
- The "Ask when they start" dialog (Echo's own "Record?") opens a native window, so the ask mode is `recording/TC-012` (person); this case runs the manual mode, where nothing is asked and nothing is recorded.

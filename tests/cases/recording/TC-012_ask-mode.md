---
id: TC-012
title: Ask when calls start: Echo's own dialog decides per call
priority: high
type: functional
requirements: [F-REC-03, F-SET-06]
module: recording
run: agent
platform: macos
tags: [recording, ask, second-instance, test-call]
external_id: TC-REC-014
---

# TC-012: Ask when calls start: Echo's own dialog decides per call

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The run's terminal may use System Events (Accessibility granted to it; the user's choice). The dialog is Echo's own (`app::ask`: an `osascript` child of the run's app, "A Teams call started. Record and transcribe it?", buttons "Not now" / "Record"). It is found by its parent pid, never by name: the user's own Echo may show dialogs too, and two `echoctl` processes can't be told apart by name

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), twice |
| Mode | `config --unscheduled ask` ("Ask when they start") |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"` | It prints the folder, the length and the turns |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | The config calls exit 0. `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl config --unscheduled ask; ls "$ECHOCTL_HOME/calls" > "$D/before.txt"`; start a call: `nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`; then `for i in $(seq 20); do O=$(pgrep -P "$(cat "$D/app.pid")" osascript) && break; sleep 1; done; echo "$O" > "$D/dialog.pid"; ps -o command= -p "$O"` | `unscheduled` is "ask". Within 5 s the app has an `osascript` child running `display dialog "A Teams call started.…Record and transcribe it?" with title "Echo" buttons {"Not now", "Record"}` |
| 4 | `O=$(cat <dir>/dialog.pid); osascript -e "tell application \"System Events\" to tell (first process whose unix id is $O) to click button \"Not now\" of window 1"`; then run the prelude and `target/debug/echoctl status` | The dialog closes; `watcher.in_call` true, `watcher.recording` null |
| 5 | Wait for the call to end and 15 s more; start the same call again and find the dialog as in step 3; click "Record" the same way; 4 s later run the prelude and `target/debug/echoctl status` | `watcher.recording` is a new folder under the run's `calls/` |
| 6 | Run the prelude, then wait for the call to end and 25 s more; `diff "$D/before.txt" <(ls "$ECHOCTL_HOME/calls"); tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -e call_started -e recording_started` | One new folder (the second call); two `call_started` events with `"ask":true`, one `recording_started`, after the second |
| 7 | Run the prelude, then `target/debug/echoctl config --unscheduled manual` | `unscheduled` is "manual": no more questions (`privacy/TC-001` runs a call in this mode) |

## Expected Final State
Ask mode asked per call with Echo's own dialog; "Not now" recorded nothing, "Record" recorded only that call; no Claude Code involved.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- The tray's Settings → "Calls not on the calendar" sets the same `unscheduled` value as `config --unscheduled`.
- Run on 2026-11-07 (S-18): PASS — dialog found as the app's child in 4 s; "Not now" → nothing recorded; "Record" → `recording_started` 1 s after the second `call_started` (`ask: true`).

---
id: TC-012
title: Ask when calls start: Echo's own dialog decides per call
priority: high
type: functional
requirements: [F-REC-03, F-SET-06]
module: recording
run: agent
platform: macos
tags: [recording, ask, second-instance, file-fed]
user_steps: ['allow the run''s terminal to use System Events (Accessibility), if it may not yet: it clicks only Echo''s own dialog, by its pid']
external_id: TC-REC-014
---

# TC-012: Ask when calls start: Echo's own dialog decides per call

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The run's terminal may use System Events (Accessibility granted to it; the user's choice: if it may not, the case is BLOCKED; the run never changes the setting). The dialog is Echo's own (`app::ask`: an `osascript` child of the run's app, "A Teams call started. Record and transcribe it?", buttons "Not now" / "Record"). It is found by its parent pid and clicked by its own pid, never by name: the user's own Echo may show dialogs too, and two `echoctl` processes can't be told apart by name
- No call is on (`$C status`: `in_call` false), nothing armed or recording
- **In S-18:** the app is the prelude's (step 1 skipped). The teardown is skipped: the second call and its recording go on for stage 11 (`recording/TC-006`, a drop and rejoin of that recording)

## Test Data

| Field | Value |
|-------|-------|
| First call | very-short (9 s), held; answered "Not now" |
| Second call | acme-sync (36 s), held; answered "Record" |
| Mode | `config --unscheduled ask` ("Ask when they start") |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `recording` null and `armed` false; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled ask; cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; ls "$ECHOCTL_HOME/calls" > "$D/before.txt"; $C start very-short --hold; for i in $(seq 20); do O=$(pgrep -P "$(cat "$D/app.pid")" osascript) && break; sleep 1; done; echo "$O" > "$D/dialog.pid"; ps -o command= -p "$O"` | `unscheduled` is "ask". Within 5 s the app has an `osascript` child running `display dialog "A Teams call started.…Record and transcribe it?" with title "Echo" buttons {"Not now", "Record"}` |
| 3 | Run the prelude, then `O=$(cat "$D/dialog.pid"); osascript -e "tell application \"System Events\" to tell (first process whose unix id is $O) to click button \"Not now\" of window 1"; sleep 2; target/debug/echoctl status` | The dialog closes; `watcher.in_call` true, `watcher.recording` null |
| 4 | Run the prelude, then `$C end; $W 10 no-call; $C start acme-sync --hold; for i in $(seq 20); do O=$(pgrep -P "$(cat "$D/app.pid")" osascript) && break; sleep 1; done; osascript -e "tell application \"System Events\" to tell (first process whose unix id is $O) to click button \"Record\" of window 1"; $W 10 recording; target/debug/echoctl status` | A new dialog for the second call; after "Record", `watcher.recording` is a new folder under the run's `calls/` |
| 5 | Run the prelude, then `diff "$D/before.txt" <(ls "$ECHOCTL_HOME/calls"); tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -e call_started -e recording_started` | One new folder (the second call's recording); two `call_started` events with `"ask":true`, one `recording_started`, after the second |
| 6 | Run the prelude, then `target/debug/echoctl config --unscheduled manual` | `unscheduled` is "manual": no more questions (`privacy/TC-001` runs a call in this mode) |

## Expected Final State
Ask mode asked per call with Echo's own dialog; "Not now" recorded nothing, "Record" recorded only that call; no Claude Code involved.

## Teardown
- Run the prelude, then `$C end; $W 30 event recording_stopped` (in S-18 skipped: the call goes on for stage 11)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- The tray's Settings → "Calls not on the calendar" sets the same `unscheduled` value as `config --unscheduled`.
- Run on 2026-11-07 (S-18): PASS — dialog found as the app's child in 4 s; "Not now" → nothing recorded; "Record" → `recording_started` 1 s after the second `call_started` (`ask: true`).
- 2026-11-08: the second call is a new call as soon as the first has ended (no recording ran: no rejoin grace to wait out, `watch.rs`); what the recording holds is not this case's check (the wait for its transcription is gone). In S-18 it goes on into `recording/TC-006`.

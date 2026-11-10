---
id: TC-004
title: Arming and stopping from the terminal drives the recording like the menu
priority: medium
type: functional
requirements: [F-CLI-11, F-REC-02, F-REC-13]
module: cli
run: agent
platform: macos
tags: [cli, recording, second-instance, file-fed]
external_id: TC-CLI-002
auto: [tests/auto/specs/S-18.spec.mjs]
---

# TC-004: Arming and stopping from the terminal drives the recording like the menu

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` (the app transcribes the recording after the call; in S-18: the prelude did)
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- No call is on (`$C status`: `in_call` false), nothing armed or recording
- **In S-18:** the app is the prelude's (step 1 skipped). The teardown is skipped: the call goes on, unrecorded, for stage 8 (`recording/TC-015`), which checks what the stop did to the call and the recording, with `$D/ev.start` as step 2 wrote it

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held; stopped 20 s into the call |
| Arm title | CLI-004 terminal |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; ls "$ECHOCTL_HOME/calls" > "$D/before.txt"; target/debug/echoctl arm --title "CLI-004 terminal"`, `$W 10 armed` (the watcher takes the arm at its next tick), then `target/debug/echoctl status` | arm prints "armed — recording starts when the call begins (expires in 15 min)"; `status` has `watcher.armed` true and `armed.title` "CLI-004 terminal" |
| 3 | Run the prelude, then `$C start acme-sync --hold; $W 20 recording cli-004-terminal; target/debug/echoctl status; sleep 18; target/debug/echoctl stop; $W 10 not-recording; target/debug/echoctl status; $C status` (the `sleep 18` is the call's timeline: the stop comes about 20 s in) | Once it records (within a second or two), `watcher.recording` is a folder path ending `-cli-004-terminal` and `watcher.armed` false. stop prints "stopping" and that folder; within seconds `watcher.recording` is null, `armed` null, and the call is still on (`in_call` true, `for_s` about 20) |
| 4 | Run the prelude, then `diff "$D/before.txt" <(ls "$ECHOCTL_HOME/calls"); tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'` | One new folder, ending in `cli-004-terminal`; the events include `call_started`, `recording_started` then `recording_stopped` |
| 5 | Run the prelude, then `target/debug/echoctl stop` | "nothing to stop" (nothing is armed or recording) |

## Expected Final State
The terminal armed, and stopped, a recording exactly as the menu does; the call goes on without it.

## Teardown
- Run the prelude, then `$C end` (in S-18 skipped: the call goes on for stage 8)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- Run on 2026-11-07 (S-18): PASS; right after `arm`, `watcher.armed` was still false — it is true a tick later (hence `$W 10 armed`).
- 2026-11-08: the transcript's existence (old step 6) is left to `recording/TC-015`, which in S-18 reads this recording's transcript; the arm's message and the stop's output, checked here, are no longer repeated in `recording/TC-015`.
- Original steps read the menu-bar icon; `echoctl status` shows the same states (`watcher.armed`, `watcher.recording`). The icon itself is `recording/TC-019` (person).

---
id: TC-001
title: Nothing records without opting in
priority: critical
type: functional
requirements: [F-PRIV-01, F-REC-03, F-REC-02]
module: privacy
run: agent
platform: macos
tags: [privacy, opt-in, second-instance]
external_id: TC-PRIV-001
---

# TC-001: Nothing records without opting in

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- Echo idle: no call on, not armed, nothing scheduled, no series set to record always (true of the test data). The call follows an arm that was cancelled (step 2), so a cancelled arm is seen not to record it either
- **In S-18:** the app is the prelude's (step 1 skipped); stages 1–2 left an arm cancelled (`recording/TC-003`) and an arm expired (`recording/TC-004`) just before (step 2 skipped). The call this case starts is **the main call**: it stays on for stages 4–6 (teardown skipped)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), the call held (`--hold`) |
| Mode | "Only when I click Record" (`config --unscheduled manual`, set by step 1): no dialog opens |
| Cancelled arm | PRIV-001 cancelled |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "PRIV-001 cancelled"; $W 10 armed; target/debug/echoctl stop; $W 10 disarmed` | "armed — …", then "disarmed (nothing was recording)"; "waited … for disarmed" |
| 3 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; ls "$ECHOCTL_HOME/calls" > "$D/before.txt"; $C start acme-sync --hold; $W 15 in-call; sleep 6; target/debug/echoctl status; diff "$D/before.txt" <(ls "$ECHOCTL_HOME/calls"); tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -e call_started -e recording_started` (the `sleep 6` is a check that nothing happens: a recording starts within a second of the call) | `call.py` prints the call; within 15 s `watcher.in_call` is true; 6 s later `watcher.recording` null, `watcher.armed` false, `armed` null; the folder list is unchanged (diff prints nothing); the events are one `call_started` with `"armed":false` and `"ask":false`, and no `recording_started` |
| 4 | Run the prelude, then `find "$ECHOCTL_HOME" "$ECHOCTL_CONFIG_DIR" -name '*.wav' -newer "$D/before.txt"` | Nothing is printed (`-newer` leaves out audio earlier stages of a scenario kept) |

## Expected Final State
A call came on an idle Echo, right after an arm was cancelled; nothing was recorded, no folder was added and no audio exists.

## Teardown
- Run the prelude, then `$C end` (in S-18 skipped: the main call goes on)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- Run on 2026-11-07 (S-18): PASS (its own TestCall call then, checked 12 s after its end).
- 2026-11-08: the check moved here from `recording/TC-003` and `TC-004` ("the call after a cancelled or expired arm isn't recorded": step 2 and, in S-18, stages 1–2). The check is made while the call is on (a watcher recording starts within a second of the call, app_profile.md), not after its end: in S-18 the same call goes on to be armed mid-call (stage 6).
- The "Ask when they start" dialog (Echo's own "Record?") is `recording/TC-012`; this case runs the manual mode, where nothing is asked and nothing is recorded.

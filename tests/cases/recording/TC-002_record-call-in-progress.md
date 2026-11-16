---
id: TC-002
title: Record a call already in progress
priority: critical
type: functional
requirements: [F-REC-01]
module: recording
run: agent
platform: macos
tags: [recording, second-instance, file-fed]
external_id: TC-REC-002
auto: [tests/auto/specs/S-18.spec.mjs]
---

# TC-002: Record a call already in progress

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` (in S-18: the prelude did)
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- Nothing armed, nothing recording
- **In S-18:** the app is the prelude's (step 1 skipped); the main call is on, unarmed (started by stage 3 and kept by stages 4–5): step 2's `$C end` and `$C start` back to back play a new conversation into it (the watcher sees the same call: its 2 s debounce), and step 4 hangs it up. On its own: step 2 starts the call (`$C end` prints "no call")

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held; armed about 13 s in, hung up after the conversation |
| Arm title | REC-002 mid call |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; $C end; $C start acme-sync --hold; $W 15 in-call; target/debug/echoctl status; sleep 12; target/debug/echoctl arm --title "REC-002 mid call"; $W 10 recording rec-002-mid-call; target/debug/echoctl status; sleep 23` (the sleeps are the call's timeline: the arm about 13 s in, then the rest of the conversation) | Once the watcher sees the call: `watcher.in_call` true, `watcher.recording` null, `armed` null. arm prints "armed — call in progress, recording starts within a second". Within 10 s: `watcher.recording` is a folder path ending `-rec-002-mid-call` |
| 3 | Run the prelude, then `tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -c recording_started; grep -n -e '"event":"call_started"' -e '"event":"recording_started"' "$ECHOCTL_HOME/.echo/events.jsonl" | tail -n 2` | The count is 1; the last `call_started` in the log comes before that `recording_started` (the call began unarmed, the recording later) |
| 4 | Run the prelude, then `$C end; CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-002-mid-call)"; $W 30 event recording_stopped; $W 120 file "$CALL/transcript.md"; ls "$CALL"` | "hung up"; within 2 minutes the folder has `transcript.md` and `segments.json`, and no `.wav` file (the app transcribed it and deleted the audio) |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-002-mid-call)"; grep -c -e "single sign-on" -e "SSO checklist" "$CALL/transcript.md"` | 0: what was said in the first 12 s, before the arm, is not there |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-002-mid-call)"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"` | 2 or more: what was said after the arm is there |

## Expected Final State
A call recorded from the middle covers only what was said after the arm.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")` (in S-18 skipped: the prelude's app goes on)

## Notes
- Run on 2026-11-07 (S-18): PASS.
- The original steps used the menu item "Record this call"; `echoctl arm` during a call is the same request (`watch::arm`).
- The arm and the call's words share one clock (the conversation starts at step 2's `$C start`), so step 2 is one command: a runner's seconds between steps would move the arm.

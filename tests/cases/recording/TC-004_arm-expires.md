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
auto: [tests/auto/specs/S-18.spec.mjs]
---

# TC-004: An arm expires after 15 minutes

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- No call is on (`python3 tests/fixtures/call.py status`: `in_call` false): an arm made during a call records at once
- In S-18: the app is the prelude's (step 1 skipped); the call that follows the expiry is the main call of stage 3 (`privacy/TC-001`), which checks that it isn't recorded

## Test Data

| Field | Value |
|-------|-------|
| Arm title | REC-004 expiring |
| Age given to the arm | 16 minutes |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `target/debug/echoctl arm --title "REC-004 expiring"` | arm prints "armed — recording starts when the call begins (expires in 15 min)" |
| 3 | Run the prelude, then (this edit of the arm's time replaces waiting 16 minutes) `sed -i '' "s/\"armed_at\": *\"[^\"]*\"/\"armed_at\":\"$(date -u -v-16M +%Y-%m-%dT%H:%M:%SZ)\"/" "$ECHOCTL_HOME/.echo/armed.json"; $W 10 event arm_expired; $W 10 disarmed; target/debug/echoctl status; tail -n 1 "$ECHOCTL_HOME/.echo/events.jsonl"` | `armed` is null and `watcher.armed` false within a few seconds; the run's `.echo/events.jsonl` ends with an `arm_expired` event with `title` "REC-004 expiring" |

## Expected Final State
An arm older than 15 minutes is dropped: nothing is armed for the next call.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Run on 2026-11-07 (S-18): PASS (the app kept from the stage before; with its own call then).
- 2026-11-08: "the next call is not recorded" moved to `privacy/TC-001` (in S-18 it is the call right after this stage); this case no longer needs a call.
- Only the arm's time is edited; the 15 minutes themselves are `ARM_EXPIRES` in `watch.rs`. A person can still check the full wait once.

---
id: TC-004
title: Only one Echo runs at a time
priority: high
type: functional
requirements: [F-LIFE-04]
module: lifecycle
run: agent
platform: macos
tags: [lifecycle, second-instance]
external_id: TC-LIFE-004
---

# TC-004: Only one Echo runs at a time

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` right before the run (hermetic: the app's own Claude Code jobs reach no account); `target/debug/echoctl` is the debug build from this checkout; every command after `. <dir>/env` (the prelude of `recording/TC-001`, with `D="$(dirname "$ECHOCTL_HOME")"`)
- `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false` and `target/debug/echoctl ai live off` (no dialog, no run of its own)
- The dev app is started as `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, beside the user's own Echo (its own config folder, so its own `app.lock`); never `app start` (it opens the installed Echo.app)
- The dev app running (started as above, then `python3 tests/fixtures/wait.py 30 app`). In S-21: the scenario's app, started in its prelude

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl app run --no-tray; echo "exit=$?"` | At once: "Error: Echo is already running (pid N) — `echoctl app quit` stops it", N the running app's pid; exit 1 |
| 2 | `for p in $(pgrep -f "echoctl app run"); do ps eww -o command= -p $p \| grep -q "ECHOCTL_CONFIG_DIR=$ECHOCTL_CONFIG_DIR" && echo "app $p"; done` | One line: the first app's pid |
| 3 | `target/debug/echoctl app quit` (in S-21: skipped — `lifecycle/TC-003` step 1 quits the same app, with the same check) | "Echo stopped" |

## Expected Final State
A second start never makes a second app on the same config.

## Notes
- The lock is `<config dir>/app.lock` (`app.rs`, `single_instance`). Opening Echo.app again from Applications ("at most the window opens") and `app start` need an installed build under test: a person's check (here the installed Echo is the user's own).
- Run 2026-11-08 evening (S-21, the prelude's app): PASS — "Error: Echo is already running (pid N)", exit 1, one app on the run's config.
- Run on 2026-11-07 (S-21): PASS.

---
id: TC-003
title: Quit leaves no process and restarts normally
priority: high
type: functional
requirements: [F-LIFE-03]
module: lifecycle
run: agent
platform: macos
tags: [lifecycle, second-instance]
external_id: TC-LIFE-003
---

# TC-003: Quit leaves no process and restarts normally

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` right before the run (hermetic: the app's own Claude Code jobs reach no account); `target/debug/echoctl` is the debug build from this checkout; every command after `. <dir>/env` (the prelude of `recording/TC-001`, with `D="$(dirname "$ECHOCTL_HOME")"`)
- `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false` and `target/debug/echoctl ai live off` (no dialog, no run of its own)
- The dev app is started as `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, beside the user's own Echo (its own config folder, so its own `app.lock`); never `app start` (it opens the installed Echo.app)
- The dev app running (started as above, then `python3 tests/fixtures/wait.py 30 app`: it waits for the app to answer, with a timeout)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `P=$(cat "$D/app.pid"); target/debug/echoctl app quit; python3 tests/fixtures/wait.py 10 sh "! ps -p $P > /dev/null"; ps -p $P` | "Echo stopped (was pid $P)"; `ps` finds no such process |
| 2 | `for p in $(pgrep -f "echoctl app run"); do ps eww -o command= -p $p \| grep -q "ECHOCTL_CONFIG_DIR=$ECHOCTL_CONFIG_DIR" && echo "left: $p"; done; target/debug/echoctl status` | Nothing "left" on this run's config (the user's own Echo is not counted); `watcher` is null |
| 3 | Start the app again (as above), `python3 tests/fixtures/wait.py 30 app`, then `target/debug/echoctl status`; `pgrep -P "$(cat "$D/app.pid")" osascript`; `target/debug/echoctl up` | `watcher.pid` is the new app; no `osascript` child (no first-run dialog); `up` has `home_chosen` true, `ready` true, `next` "nothing — Echo is running" |
| 4 | `target/debug/echoctl app quit` | "Echo stopped" |

## Expected Final State
Quit is clean, and the app comes back without first-run questions.

## Notes
- `echoctl app quit` asks the app to quit as the tray's "Quit Echo" does. The tray's item, and opening Echo from Applications, are a person's check on an installed Echo: here the installed app is the user's own.
- Run on 2026-11-07 (S-21): PASS.

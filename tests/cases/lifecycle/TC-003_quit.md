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
- The dev app running (started as above, then `python3 tests/fixtures/wait.py 30 app`: it waits for the app to answer, with a timeout). In S-21: the scenario's app, started in its prelude and left running by `lifecycle/TC-004`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `P=$(cat "$D/app.pid"); target/debug/echoctl app quit; python3 tests/fixtures/wait.py 10 sh "! ps -p $P > /dev/null"; ps -p $P` | "Echo stopped (was pid $P)"; `ps` finds no such process |
| 2 | `for p in $(pgrep -f "echoctl app run"); do ps eww -o command= -p $p \| grep -q "ECHOCTL_CONFIG_DIR=$ECHOCTL_CONFIG_DIR" && echo "left: $p"; done; target/debug/echoctl status` | Nothing "left" on this run's config (the user's own Echo is not counted); `watcher` is null |
| 3 | Start the app again (as above), `python3 tests/fixtures/wait.py 30 app`, then `target/debug/echoctl status`; `pgrep -P "$(cat "$D/app.pid")" osascript`; `target/debug/echoctl up` | `watcher.pid` is the new app; no `osascript` child (no first-run dialog); `up` has `home_chosen` true and `app_pid` the new app; `ready` true and `next` "nothing — Echo is running" when the process running the steps has System Audio Recording, else `failing` is `["system-audio"]` only and `next` "run the doctor skill" (a file-fed run needs no permission: that check is the live one's) |
| 4 | `target/debug/echoctl app quit` | "Echo stopped" |

## Expected Final State
Quit is clean, and the app comes back without first-run questions.

## Notes
- `echoctl app quit` asks the app to quit as the tray's "Quit Echo" does. The tray's item, and opening Echo from Applications, are a person's check on an installed Echo: here the installed app is the user's own.
- Run `up` only while the run's app is running: with none, a dev build's `up` starts Echo with `open … /Applications/Echo.app` (the user's installed Echo here), unless `ECHOCTL_TEST_OPEN_LOG_ONLY=1` is set.
- Run 2026-11-08 evening (S-21): steps 1–2 and 4 PASS; step 3: no `osascript`, `home_chosen` true, but `ready` false, `failing` `["system-audio"]` (the agent's shell has no System Audio Recording: before, TestCall runs had it granted) — the expected result now says so.
- Run on 2026-11-07 (S-21): PASS.

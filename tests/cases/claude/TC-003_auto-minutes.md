---
id: TC-003
title: Minutes are started automatically at the delay after a call, not before
priority: critical
type: functional
requirements: [F-MIN-14, F-MIN-07]
module: claude
run: agent
platform: macos
tags: [minutes, auto, second-instance, recordings, test-clock]
external_id: TC-CLA-003
auto: [tests/auto/specs/S-17.spec.mjs]
---

# TC-003: Minutes are started automatically at the delay after a call, not before

**Module:** Claude | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake --clock fast` (or `serve.sh up <dir> <port> --ai fake --clock fast`): the small set, hermetic (every Claude Code run is refused at once by the fake `claude`), and `<dir>/env` sets the debug build's test clock `ECHOCTL_TEST_AUTO_MINUTES_DELAY_S=90`: automatic minutes come **90 s** after the call or the last note instead of 15 minutes (`notes::auto_delay`; a release build always waits 15 minutes). Every process — the app, the served window, the CLI — runs after `. <dir>/env`, so all of them use the same delay
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"); `minutes-call` (213 s, a decision and action items) is long enough for automatic minutes
- The prelude of the recording cases (`recording/TC-001`), with `W=tests/fixtures/wait.py`
- **Needs a second app instance** (`recording/TC-001`'s Preconditions). In S-17: the scenario's app, running since its prelude (automatic minutes off until step 1 turns them on; the running app reads the change): step 1 leaves out its `nohup … app run …` and `$W 30 app` and checks `$W 5 app` instead. On its own: step 1 starts it
- The window served on the same data with clicks reaching the app (`ECHOCTL_SERVE_IPC=1`), the page loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Call | a copy of `minutes-call`, ending when it is copied, title "MIN-A catch-up" |
| Delay | 90 s (`--clock fast`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `echo $ECHOCTL_TEST_AUTO_MINUTES_DELAY_S`; `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes true`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app` | `90`; `auto_minutes` true (switched on now: calls that ended before stay without automatic minutes); "waited … for app"; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `CALL="$(python3 tests/fixtures/recordings.py copy minutes-call --title "MIN-A catch-up")"; echo "$CALL" > "$D/min-a"; grep -o '"ended_at": *"[^"]*"' "$CALL/meta.json"` | A call that ended just now, with its transcript; `ended_at` is set (call it E) |
| 3 | In the window: `{{base_url}}/`, then `#calls`, click the row `[role=option][data-call$="min-a-catch-up"]`, open its Minutes tab | "Minutes in 2 min (HH:MM)" (or "1 min") with HH:MM = E + 90 s rounded to the minute, the line "Echo writes them 15 minutes after the call or your last note…" (the text keeps the release delay), the profile, "Write now" (`[data-cmd="write-notes"]`) and "Not yet ▾"; nothing is pressed |
| 4 | Run the prelude, then `sleep 45; ls "$(cat "$D/min-a")"` (half the delay: a check that nothing came early, not a wait for a condition) | No `notes-run.json` yet: nothing starts before the time |
| 5 | Run the prelude, then `$W 120 file "$(cat "$D/min-a")/notes-run.json"; cat "$(cat "$D/min-a")/notes-run.json"` | Within about a minute more: `"auto": true`, `started_at` no earlier than E + 90 s and within 30 s after it (the scheduler ticks every 15 s) |
| 6 | Run the prelude, then `$W 60 sh 'grep -q -e "\"state\": *\"failed\"" -e "\"state\": *\"done\"" "$(cat "$D/min-a")/notes-run.json"'; cat "$(cat "$D/min-a")/notes-run.json"; head -3 "$(cat "$D/min-a")/claude.log"` | The run ended (on the fake, "failed": no model here); `claude.log`'s first line says "Echo starts Claude Code to write the minutes (automatically)"; the Minutes tab then says why it didn't finish, with "Try again" |

## Expected Final State
With no one asking, the minutes run started once, at the time Echo named (the call's end plus the delay), never before.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`. In S-17: skipped (`claude/TC-010` goes on with the same app and quits it)

## Notes
- The delay is `notes::AUTO_DELAY` (15 minutes) in a release build. The debug build's `ECHOCTL_TEST_AUTO_MINUTES_DELAY_S` changes only how long; the rule (the call's end or the last note, plus the delay; checked every 15 s) is the same code. A person can still check the real 15 minutes once a release.
- What a real automatic run writes and runs (the minutes' parts with owners and dates, the log's commands, nothing sent) is checked on the local model in `claude/TC-007` (S-19), whose run is automatic too. Before 2026-11-08 this case waited the real 15 minutes and ran the local model itself.
- The "Transcript ready" and "Minutes ready" notifications are not checked here (macOS shows them to the user: `lifecycle/TC-006`).
- Run 2026-11-08 evening (S-17, CLI part, the scenario's app kept from the main call): PASS — call ended 19:29:23, the Minutes tab's `auto` (`/api/call/<name>`) `at` 19:30:53, nothing at 45 s, the run started 19:31:00 (`auto: true`), `failed` on the fake at once, `claude.log` "…to write the minutes (automatically)".
- Run on 2026-11-07 (S-17, the network model server, real clock): PASS — call ended 12:35:21, the tab said "Minutes in 15 min (12:50 PM)", the run started 12:50:26 (`auto: true`).

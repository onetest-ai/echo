---
id: TC-015
title: Automatic minutes off: nothing is written until asked
priority: medium
type: functional
requirements: [F-MIN-14]
module: claude
run: agent
platform: macos
tags: [minutes, auto, second-instance, recordings]
ai: local
external_id: TC-CLA-016
---

# TC-015: Automatic minutes off: nothing is written until asked

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local --clock fast` (`ai: local`; the test clock: automatic minutes 90 s after a call in this debug build, `claude/TC-003`): the small set, hermetic, with the AI profile "Qwen" (Ollama here, or the faster server of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused; Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D` (the env sets the file-fed call sources and the call app's id: nothing here records)
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"); `W=tests/fixtures/wait.py` in the prelude
- The dev app is started as `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"` (beside the user's own Echo, on the run's own config) after `config --unscheduled manual --scheduled manual` and `ai live off`; the window served on the data with clicks reaching it (`ECHOCTL_SERVE_IPC=1 … window --serve <port>`)

## Test Data

| Field | Value |
|-------|-------|
| Call | a copy of `minutes-call` (213 s), title "CLA-015 auto off" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --auto-minutes false` | `auto_minutes` false (the menu's "Write minutes automatically" off) |
| 2 | Run the prelude, then `python3 tests/fixtures/recordings.py copy minutes-call --title "CLA-015 auto off" > "$D/rec.out"`; then start the dev app (`$W 30 app`); open the call in the window → Minutes | "No minutes yet", "The transcript is ready.", "Write minutes": no time, no countdown; the API's `auto` is `{"due": "off"}` |
| 3 | With the app running, wait past the time automatic minutes would have come: `sleep 120` after the copy (the delay, 90 s, plus two of the scheduler's 15 s ticks: a check that nothing happens, not a wait for a condition); `ls "$(cat "$D/rec.out")"` | No `notes.md`, no `notes-run.json`: nothing written, by Echo or anyone |
| 4 | Run the prelude, then `target/debug/echoctl config --auto-minutes true`; read the call's `auto` (`curl -s {{base_url}}/api/call/<folder>`) | `auto_minutes` true and `auto_minutes_since` now; the call's `auto` is `{"due": "before"}`: a call that ended while the switch was off still gets none automatically (the next one does: `claude/TC-007` in S-19, or any call recorded now) |
| 5 | In the window → the call → Minutes → "Write minutes"; wait until its `notes-run.json` is "done" | Written as usual (`notes.md`, `actions.json`); the run is not marked `auto` |

## Expected Final State
With the switch off Echo writes minutes only when asked, and turning it on doesn't backfill.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit` (in S-19 skipped: the app stays for `cli/TC-005`, and `claude/TC-007`'s step 1 quits it)

## Notes
- The "click Write minutes" notification after the transcript is not checked (macOS shows it to the user).
- Before 2026-11-08 the case recorded a 213 s call and waited 20 minutes on the real clock.
- Run on 2026-11-07 (S-19): PASS — call ended 12:56:57; at 13:17 nothing written (app running since 12:57); switched on at 13:17: `due: before`; Write minutes at 13:38, done in 2 min.

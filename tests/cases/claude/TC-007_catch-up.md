---
id: TC-007
title: Calls missed while Echo was quit get their minutes on the next start
priority: medium
type: functional
requirements: [F-MIN-18, F-MIN-14]
module: claude
run: agent
platform: macos
tags: [minutes, auto, second-instance, test-call]
ai: local
external_id: TC-CLA-006
---

# TC-007: Calls missed while Echo was quit get their minutes on the next start

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (Ollama here, or the faster server of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused; Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model linked into the test config (`recording/TC-014`); TestCall, BlackHole 2ch and ffmpeg
- The dev app is started as `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"` (beside the user's own Echo, on the run's own config) after `config --unscheduled manual --scheduled manual` and `ai live off`; the window served on the data with clicks reaching it (`ECHOCTL_SERVE_IPC=1 … window --serve <port>`)
- `target/debug/echoctl config --auto-minutes true` set before the call (in S-19, `claude/TC-015` step 4)

## Test Data

| Field | Value |
|-------|-------|
| Call | minutes-call (213 s), title "CLA-007 missed" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl app quit` (if running); `target/debug/echoctl status` | `watcher` null: Echo is quit |
| 2 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build minutes-call "$D/mc"; "$APP" serve --port 7792 --play "$D/mc/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$D/mc/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 220 --title "CLA-007 missed" > "$D/rec.out" 2> "$D/rec.err"; wait; target/debug/echoctl transcribe "$(cat "$D/rec.out")"`; read the call's `auto` (`curl -s {{base_url}}/api/call/<folder>`) | `{"due": "at", "at": <the call's end + 15 minutes>}` |
| 3 | Leave Echo quit until that time has passed (16 minutes after the call; other stages may run meanwhile); `ls` the call's folder | No `notes-run.json` yet: nothing ran while Echo was quit |
| 4 | Start the dev app; `until ls "$ECHOCTL_HOME"/calls/*/notes-run.json …` for this call, then read it | Within a minute of the start: `"auto": true`, `started_at` within seconds of the start; `claude.log` starts "Echo starts Claude Code to write the minutes (automatically)" |
| 5 | Run the prelude, then `target/debug/echoctl pending --for "$ECHOCTL_HOME"` | `[]`: a session is never given the call (the app writes it) |
| 6 | When the run is "done": `ls` the call's folder (the run may rename it) | `notes.md` and `actions.json` |

## Expected Final State
A missed call is caught up by Echo at its next start, not by a session.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`

## Notes
- "An hour later" became "past the minutes' time": the scheduler only asks whether the time has passed (`notes::auto_due`).
- Run on 2026-11-07 (S-19): PASS — call ended 13:20:54 with Echo quit, due 13:35:54; app started 13:36:33, the run started the same second (`auto: true`), done in 85 s; `pending --for` empty.

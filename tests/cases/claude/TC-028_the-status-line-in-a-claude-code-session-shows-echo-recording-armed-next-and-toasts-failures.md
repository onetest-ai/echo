---
id: TC-028
title: The status line in a Claude Code session shows Echo, recording / armed / next meeting, and toasts a failed transcription or an expired arm
priority: medium
type: functional
requirements: [F-CC-07]
module: claude
run: person
platform: macos
data: small
tags: [claude, status-line, plugin, second-instance]
---

# TC-028: The status line shows Echo, recording / armed / the next meeting, and toasts a failed transcription or an expired arm

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in (your own account); the checkout built (`cargo build`)
- **Made-up data, never `~/echo`.** Shell A (the app, on the data's fake Claude Code):
  1. `D="$(mktemp -d)/echo-test"; python3 tests/fixtures/make_data.py "$D" --ai fake` — build it before 22:00 so the agenda's next meeting is today
  2. `. "$D/env"; W=tests/fixtures/wait.py; C=tests/fixtures/call.py`
  3. `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`
  4. `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app`
- Shell B (the session): `. "$D/env"; export PATH="${PATH#"$D/bin:"}"; unset CLAUDE_CONFIG_DIR ECHOCTL_NOTES_NO_SHELL; export ECHOCTL_BIN="$PWD/target/debug/echoctl"`, then `claude --plugin-dir "$PWD/plugin"` (the plugin's launcher then uses this build and the test data folder; the app is already running, so the mod starts nothing)
- The session's line above the prompt is read within about 2 seconds of each change (the mod polls every second)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In shell B start the session, wait for the prompt | A line above the prompt: a button "Echo" and the text "Echo · next: HH:MM Globex design review" (the time of that meeting today, as `target/debug/echoctl calendar events` lists it) |
| 2 | In shell A: `printf '{"event":"arm_expired"}\n' >> "$ECHOCTL_HOME/.echo/events.jsonl"` | Within 2 s a toast: "echo: the arm expired — no call started within 15 min" |
| 3 | In shell A: `printf '{"event":"transcribed","ok":false,"error":"no speech model"}\n' >> "$ECHOCTL_HOME/.echo/events.jsonl"` | A toast: "echo: transcription failed — no speech model" |
| 4 | In shell A: `printf '{"event":"transcribed","ok":true}\n' >> "$ECHOCTL_HOME/.echo/events.jsonl"` | No toast; the line is unchanged |
| 5 | In shell A: `target/debug/echoctl arm --title "STATUS-001"; $W 10 armed` | The line reads "◌ Armed — recording starts with the next Teams call" |
| 6 | In shell A: `$C start acme-sync --hold; $W 30 recording` | The line reads "● Recording · STATUS-001 · 0 min" (the minutes grow by one a minute) and the button "Echo" is the primary (filled) one |
| 7 | In shell A: `$C end; target/debug/echoctl stop; $W 30 not-recording` | The line goes back to "Echo · next: HH:MM Globex design review" (the recording text is gone); the transcription may fail for lack of a speech model: if so a toast "echo: transcription failed — …" names its error |
| 8 | In shell A: `target/debug/echoctl app quit`; wait 6 s | The line above the prompt disappears (the app's heartbeat is older than 5 s) |

## Expected Final State
The session's line followed the app through next meeting, armed, recording and back, showed the two toasts' exact words, ignored a successful transcription, and went away when the app quit.

## Teardown
- Exit the session in shell B (`/exit`). Shell A: `$C end` (ignore "no call"), `target/debug/echoctl app quit` (if it still runs after 30 s: `kill $(cat "$D/app.pid")`), then `rm -rf "$(dirname "$D")"`.
- Never run the case against `~/echo`; never use `echoctl app start` (it may open the installed app).

## Notes
- Person-run: the line is a mod inside an interactive Claude Code session (nothing an agent can look at from outside) and a real Claude account loads the plugin. The mod's pure helpers (`parseLines`, `newLines`, `hm`, `minutesSince`) are covered by `plugin/hooks/view.test.ts`.
- Steps 2–4 write the events the app itself would write (`arm_expired` after 15 minutes, `transcribed` with `ok`/`error`): the mod reads only lines added after the session started.
- A run Echo starts itself (`ECHOCTL_HEADLESS`) shows no line; not checked here.

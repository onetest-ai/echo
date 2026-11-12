---
id: TC-031
title: Chrome holding the mic with no Meet window and no Meet meeting is no call — nothing is offered or recorded, an arm waits
priority: high
type: functional
requirements: [F-REC-21, F-REC-03]
module: recording
run: agent
platform: macos
data: small
tags: [recording, google-meet, chrome, watcher, privacy]
---

# TC-031: Chrome on the mic without a Meet call is no call

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"). `call.py start <conversation> --meet` makes the call Chrome's instead of Teams'; `--title <text>` is the title of Chrome's window (none here: no window shows a Meet call); no meeting with a Meet link is in the small data's agenda
- Every Bash call starts with the prelude of `recording/TC-001`
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running". In S-18 the app is the one stage 25's case left running: skip step 1

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `target/debug/echoctl config --languages en --unscheduled ask --scheduled manual --auto-minutes false`; `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null \| wc -l > "$D/ev.start"`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app` | "waited … for app"; no "already running" in `$D/app.out` |
| 2 | Prelude; `target/debug/echoctl arm --title "REC-031 next call"`; `$W 10 armed`; `$C start acme-sync --meet --hold`; wait 8 s; `target/debug/echoctl status`; `pgrep -P "$(cat "$D/app.pid")" osascript \|\| echo "no dialog"` | `call.py` prints `"app": "meet"`; `status`: `watcher.in_call` false, `watcher.recording` null, `armed` still "REC-031 next call"; "no dialog" (Ask mode asked nothing) |
| 3 | Prelude; `tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" \| grep -c -e '"call_started"' -e '"recording_started"'`; `$C end` | `0`: no call started, nothing recorded; "hung up" |

## Expected Final State
Nothing recorded; the arm is still there for the next call (`recording/TC-032` uses it).

## Teardown
- Leave the app and the arm for `recording/TC-032`. On its own: `target/debug/echoctl stop` (disarms) and `kill $(cat "$D/app.pid")`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from the feature's text (F-REC-21): Chrome takes the mic for a voice message, a dictation or a web app as well as for a call; only a Meet sign (its window, its meeting, or an arm for its meeting) makes it one.

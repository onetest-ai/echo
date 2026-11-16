---
id: TC-035
title: A Google Meet call that takes the mic right after a recorded Teams call hung up is a new call — the Teams recording ends, the Meet call is recorded on its own
priority: critical
type: functional
requirements: [F-REC-21, F-REC-08]
module: recording
run: agent
platform: macos
data: small
tags: [recording, google-meet, chrome, teams, watcher, back-to-back]
---

# TC-035: A Meet call right after a recorded Teams call is a new recording

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); no speech model needed (nothing here is checked in a transcript)
- Every Bash call starts with the prelude of `recording/TC-001`
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running". Step 1 starts it

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; `target/debug/echoctl ai live off`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app` | "waited … for app"; no "already running" in `$D/app.out` |
| 2 | Prelude; `target/debug/echoctl arm --title "REC-035 teams"`; `$W 10 armed`; `$C start very-short --hold`; `$W 20 recording rec-035-teams`; `target/debug/echoctl arm --title "REC-035 meet"` (the arm for the next call, made while the Teams call records) | Recording "REC-035 teams", `watcher.app` "teams"; the second arm is taken ("REC-035 meet") |
| 3 | Prelude; the Teams call hangs up and Chrome takes the mic for a Meet call at once (well within the 10 s grace): `$C end; $C start acme-sync --meet --title "Meet – tst-wxyz-abc - Google Chrome"`; `$W 40 recording rec-035-meet`; `target/debug/echoctl status` | Within 40 s a recording "REC-035 meet" runs (folder ending `-rec-035-meet`), `watcher.app` "meet" — not the Teams recording going on |
| 4 | Prelude; `$C end`; `$W 40 not-recording`; for both folders read `meta.json`: `for c in "$ECHOCTL_HOME"/calls/*-rec-035-*; do python3 -c 'import json,sys;m=json.load(open(sys.argv[1]+"/meta.json"));print(sys.argv[1].rsplit("-",1)[-1],m["app"],m["stop_reason"])' "$c"; done`; `grep -c call_resumed "$ECHOCTL_HOME/.echo/events.jsonl"` | Two folders: `teams teams call_ended` and `meet meet call_ended`; `0` `call_resumed` (the Meet call was never taken for the Teams call coming back) |

## Expected Final State
Two recordings: the Teams call alone, then the Meet call alone, each with its own app.

## Teardown
- `target/debug/echoctl app quit`; if it doesn't stop within 30 s, `kill $(cat "$D/app.pid")`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Written from the feature's text (F-REC-21: another app taking the mic during a recording's grace is a new call, recorded once that recording has ended — its first seconds may be missed). Found by the review of #230: before the fix the Meet call was glued onto the Teams recording as a rejoin, on Teams' tap.

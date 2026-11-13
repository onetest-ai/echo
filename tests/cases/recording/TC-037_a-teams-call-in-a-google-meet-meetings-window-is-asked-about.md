---
id: TC-037
title: A Teams call during a Google Meet meeting's offer window is asked about ("Record?"), as any call — the Meet meeting offers nothing to a Teams call
priority: medium
type: functional
requirements: [F-REC-21, F-REC-03]
module: recording
run: agent
platform: macos
data: small
tags: [recording, google-meet, teams, ask, agenda]
---

# TC-037: A Teams call in a Meet meeting's window is asked about

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); no speech model needed
- Every Bash call starts with the prelude of `recording/TC-001`
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running". Step 1 starts it
- The agenda is replaced by step 2 (`$ECHOCTL_HOME/.echo/agenda.json`), kept as `$D/agenda.bak.json` and put back at the end

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `target/debug/echoctl config --languages en --unscheduled ask --scheduled manual --auto-minutes false`; `target/debug/echoctl ai live off`; `cp "$ECHOCTL_HOME/.echo/agenda.json" "$D/agenda.bak.json" 2>/dev/null; true`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app` | "waited … for app"; no "already running" in `$D/app.out` |
| 2 | Prelude; a Google Meet meeting starts now (not set to record): `python3 -c "import json,datetime as d,os; n=d.datetime.now(d.timezone.utc); json.dump([{'id':'ev-tc037','subject':'Globex sync','start':n.isoformat(),'end':(n+d.timedelta(minutes=30)).isoformat(),'join_url':'https://meet.google.com/tst-wxyz-abc'}], open(os.environ['ECHOCTL_HOME']+'/.echo/agenda.json','w'))"`; `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null \| wc -l > "$D/ev.start"`; `$C start very-short --hold`; `$W 20 event call_started` | The call started |
| 3 | Prelude; `tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" \| grep '"call_started"'`; then `pkill -P "$(cat "$D/app.pid")" osascript; $C end` | The event has `"app":"teams"`, `"armed":false`, `"ask":true`: the Teams call is asked about (before #500 `"ask":false`: the Meet meeting's offer window swallowed it); "hung up" |

## Expected Final State
No recording; the agenda as before.

## Teardown
- `target/debug/echoctl app quit`; `cp "$D/agenda.bak.json" "$ECHOCTL_HOME/.echo/agenda.json" 2>/dev/null`; `tests/fixtures/serve.sh down <dir>` on its own

## Notes
- From #500 (review of #230): a meeting's offer window ("Record?" on the meeting, not on the call) counts only meetings of the call's app; a Meet meeting's arm can't start on a Teams call.

---
id: TC-033
title: A Google Meet meeting set to record when it starts records the call when Chrome takes the mic, as that meeting — a Teams call then isn't it
priority: high
type: functional
requirements: [F-REC-21, F-CAL-18]
module: recording
run: agent
platform: macos
data: small
tags: [recording, google-meet, chrome, schedule, arm]
---

# TC-033: A Google Meet meeting set to record records its Chrome call

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- As `recording/TC-031` (its app running; on its own, TC-031 step 1). No Chrome window title is given (`--title` left out): the arm for the Meet meeting is the sign
- Every Bash call starts with the prelude of `recording/TC-001`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; the meeting to record begins: `python3 -c "import json,datetime as d,os; n=d.datetime.now(d.timezone.utc); f=lambda t: t.isoformat(); json.dump([{'id':'ev-tc033','subject':'Globex roadmap chat','start':f(n-d.timedelta(seconds=30)),'end':f(n+d.timedelta(minutes=30)),'join_url':'https://meet.google.com/tst-wxyz-abc'}], open(os.environ['ECHOCTL_HOME']+'/.echo/scheduled.json','w'))"`; `$W 20 armed`; `$C start very-short --hold`; wait 8 s; `target/debug/echoctl status` | Armed for "Globex roadmap chat"; a Teams call goes on (`watcher.in_call` true, `watcher.app` "teams") and `recording` is null: a Meet meeting's arm doesn't start on a Teams call |
| 2 | Prelude; `$C end`; wait 4 s; `$C start acme-sync --meet`; `$W 20 recording globex-roadmap-chat`; `target/debug/echoctl status` | Recording "Globex roadmap chat" (folder ending `-globex-roadmap-chat`), `watcher.app` "meet" |
| 3 | Prelude; `$W 60 not-recording` (not `event recording_stopped`: an earlier stage's is in the log already); `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-globex-roadmap-chat)"`; read `"$CALL/meta.json"` and `"$CALL/meeting.json"` | `meta.json` `app` "meet", `stop_reason` "call_ended"; `meeting.json` `join_url` "https://meet.google.com/tst-wxyz-abc"; one folder for the meeting, none for the Teams call |

## Expected Final State
One recording, of the Meet meeting's Chrome call; the Teams call before it was not recorded.

## Teardown
- `target/debug/echoctl app quit`; if it doesn't stop within 30 s, `kill $(cat "$D/app.pid")`; `tests/fixtures/serve.sh down <dir>` on its own

## Notes
- Written from the feature's text (F-REC-21: a meeting's arm starts only on a call of its own app; an arm for a Meet meeting is a Meet sign).
- The 4 s between the two calls is more than the watcher's 2 s release debounce: the Teams call ends before Chrome's starts.

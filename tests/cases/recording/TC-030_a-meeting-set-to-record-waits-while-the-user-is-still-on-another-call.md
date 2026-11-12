---
id: TC-030
title: A meeting set to record when it starts doesn't record the call the user is still on — it waits, says so, and records once they are on its call
priority: critical
type: functional
requirements: [F-REC-20]
module: recording
run: agent
platform: macos
data: small
tags: [recording, schedule, arm, watcher]
---

# TC-030: Still on another call when the meeting to record begins

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); no speech model needed (nothing here is transcribed)
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; export ECHOCTL_TEST_CALL_WINDOW="$D/window.json"` — a development build reads the call window from that file (`{"began_s_ago": …, "title": …}`) for a call fed from files, as it reads Teams' timer and title
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running"

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `echo '{"began_s_ago": 1800, "title": "Equiniti weekly \| Microsoft Teams"}' > "$D/window.json"`; `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; `target/debug/echoctl ai live off`; `$C start acme-sync --hold`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app`; `$W 15 in-call` | The user is on a call that began half an hour ago ("Equiniti weekly"); the app sees it; nothing records |
| 2 | Prelude; the meeting to record begins: `python3 -c "import json,datetime as d,os; n=d.datetime.now(d.timezone.utc); f=lambda t: t.isoformat(); json.dump([{'id':'ev-tc030','subject':'Acme design review','start':f(n-d.timedelta(seconds=30)),'end':f(n+d.timedelta(minutes=30)),'join_url':'https://teams.microsoft.com/l/meetup-join/acme-tc030'}], open(os.environ['ECHOCTL_HOME']+'/.echo/scheduled.json','w'))"`; `$W 20 armed`; wait 12 s; `target/debug/echoctl status`; `grep -c arm_waits_other_call "$ECHOCTL_HOME/.echo/events.jsonl"` | Armed for "Acme design review"; `recording` null — the call going on is not recorded; one `arm_waits_other_call` event (subject "Acme design review", `on` "Equiniti weekly") |
| 3 | Prelude; the user is now on the meeting's call: `echo '{"began_s_ago": 1, "title": "Acme design review \| Microsoft Teams"}' > "$D/window.json"`; `$W 20 recording acme-design-review`; `target/debug/echoctl status` | Recording "Acme design review" (its folder ends `-acme-design-review`) — started on its own call, not before |
| 4 | Prelude; `$C end`; `$W 30 event recording_stopped`; `ls -d "$ECHOCTL_HOME"/calls/*-acme-design-review \| wc -l` | One call folder for the meeting; no other recording was made while the user was on the earlier call |

## Expected Final State
One recording, of the meeting's own call; the call before it was never recorded.

## Teardown
- `kill $(cat "$D/app.pid")`; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-REC-20), not from the code. Found by the user (#483): at a meeting's start Echo recorded the call they were still on, as that meeting, and stopped when they switched.
- A call window that can't be read (Teams' window not readable, capture of the window off) records as before: the arm starts on the call going on.

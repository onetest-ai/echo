---
id: TC-038
title: A call recorded with no meeting chosen, while a calendar meeting goes on, is filed as that meeting only when it is that meeting's call — an ad-hoc call meanwhile keeps its own name
priority: critical
type: functional
requirements: [F-REC-23]
module: recording
run: agent
platform: macos
data: small
tags: [recording, arm, watcher, calendar]
---

# TC-038: Another call during a calendar meeting isn't filed as that meeting

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); no speech model needed (nothing here is transcribed)
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; export ECHOCTL_TEST_CALL_WINDOW="$D/window.json"` — a development build reads the call window from that file (`{"began_s_ago": …, "title": …}`) for a call fed from files, as it reads Teams' timer and title
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running"

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; `target/debug/echoctl ai live off`; the calendar's meeting going on, not joined: `python3 -c "import json,datetime as d; n=d.datetime.now(d.timezone.utc); f=lambda t: t.isoformat(); json.dump([{'id':'ev-tc038','subject':'Acme design review','start':f(n-d.timedelta(minutes=2)),'end':f(n+d.timedelta(minutes=30)),'join_url':'https://teams.microsoft.com/l/meetup-join/acme-tc038','attendees':[{'name':'Dana Lee','email':'dana@acme.example'}]}], open('$D/agenda.json','w'))"`; `target/debug/echoctl agenda set "$D/agenda.json"`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app` | The app runs; the agenda holds "Acme design review", going on now |
| 2 | Prelude; the user is on another call meanwhile: `echo '{"began_s_ago": 1, "title": "Sam Roe, Kim Park \| Microsoft Teams"}' > "$D/window.json"`; `$C start acme-sync --hold`; `$W 15 in-call`; `target/debug/echoctl arm` (Record next call: no meeting chosen); `$W 20 recording`; `target/debug/echoctl status` | Recording; its folder doesn't end `-acme-design-review`, its title isn't "Acme design review" |
| 3 | Prelude; `$C end`; `$W 30 event recording_stopped`; `N=$(ls -td "$ECHOCTL_HOME"/calls/*/ \| head -1)`; `echo "$N"`; `cat "$N/meeting.json" 2>/dev/null \| grep -c "Acme design review\|dana@acme.example"` | The newest call's folder (step 2's) isn't named `-acme-design-review`; the count is `0` (no `meeting.json`, or one without the meeting's title and attendees) |
| 4 | Prelude; now the meeting's own call: `echo '{"began_s_ago": 1, "title": "Acme design review \| Microsoft Teams"}' > "$D/window.json"`; `$C start acme-sync --hold`; `$W 15 in-call`; `target/debug/echoctl arm`; `$W 20 recording acme-design-review` | Recording "Acme design review": the call window names the meeting, so the meeting going on is attached |
| 5 | Prelude; `$C end`; `$W 30 event recording_stopped 2`; `python3 -c "import json,glob,os; print([json.load(open(p)).get('subject') for p in glob.glob(os.environ['ECHOCTL_HOME']+'/calls/*-acme-design-review/meeting.json')])"` | `['Acme design review']`: the meeting's call has its meeting |

## Expected Final State
Two calls: the ad-hoc one under its own name with no meeting attached, and the meeting's own call as "Acme design review".

## Teardown
- `kill $(cat "$D/app.pid")`; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-REC-23), not from the code. Found by the user (#896): an ad-hoc call during a calendar meeting's time was recorded and summarized as that meeting.
- A call window that can't be read (capture of the window off), or one naming nothing, attaches the meeting going on as before.

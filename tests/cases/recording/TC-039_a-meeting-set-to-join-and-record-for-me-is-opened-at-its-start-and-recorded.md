---
id: TC-039
title: A meeting set to "Join and record for me" is opened in Teams by Echo at its start, once, and its call is recorded as that meeting — the window offers it and says what happens
priority: high
type: functional
requirements: [F-REC-24]
module: recording
run: agent
platform: macos
data: small
tags: [recording, schedule, arm, watcher, join]
---

# TC-039: Join and record for me

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); no speech model needed. `<dir>/env` sets `ECHOCTL_TEST_OPEN_LOG_ONLY`: a link Echo opens is appended to `<dir>/open.log`, nothing opens
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running"

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; `target/debug/echoctl ai live off`; a Teams meeting starting in 2 minutes: `python3 -c "import json,datetime as d,os; n=d.datetime.now(d.timezone.utc); f=lambda t: t.isoformat(); json.dump([{'id':'ev-tc039','subject':'Acme weekly sync','start':f(n+d.timedelta(minutes=2)),'end':f(n+d.timedelta(minutes=32)),'join_url':'https://teams.microsoft.com/l/meetup-join/acme-tc039','attendees':[{'name':'Dana Lee','email':'dana@acme.example'}]}], open(os.environ['D']+'/agenda.json','w'))"`; `target/debug/echoctl agenda set "$D/agenda.json"`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app` | The app runs; the agenda holds "Acme weekly sync" |
| 2 | Open `{{base_url}}/#calendar`, click "Acme weekly sync" | Its card offers "Record when it starts" and **"Join and record for me"** (its tooltip: you'll be on your phone, Echo joins it in Teams on this computer and records it, silent on the speakers) |
| 3 | Click "Join and record for me" | The card now offers "Don't record", and its line says: at its start Echo joins it in Teams on this computer and records it, silent on the speakers — you can be on your phone. `python3 -c "import json,os; print([(m['subject'], m.get('join_for_me')) for m in json.load(open(os.environ['ECHOCTL_HOME']+'/.echo/scheduled.json'))])"` → `[('Acme weekly sync', True)]` |
| 4 | Prelude; before the start: `$W 70 armed`; `grep -c acme-tc039 "$D/open.log" 2>/dev/null \|\| echo 0` | Armed (a minute before its start); the meeting **not opened yet**: `0` |
| 5 | Prelude; at its start: `$W 90 event joined_for_me`; `grep -c acme-tc039 "$D/open.log"` | The event `joined_for_me` with subject "Acme weekly sync"; the meeting's Teams link opened once: `1` |
| 6 | Prelude; Teams joins its call (as it would on "Add this device"): `$C start acme-sync --hold`; `$W 20 recording acme-weekly-sync`; wait 10 s; `grep -c acme-tc039 "$D/open.log"` | Recording "Acme weekly sync"; still `1`: opened once only, not again while armed or recording |
| 7 | Prelude; `$C end`; `$W 30 event recording_stopped`; `N=$(ls -td "$ECHOCTL_HOME"/calls/*-acme-weekly-sync/ \| head -1)`; `python3 -c "import json,sys; print(json.load(open(sys.argv[1]+'/meeting.json')).get('join_for_me'))" "$N"` | `True`: the newest call (this one; the small set has an older "Acme weekly sync") is the meeting's, marked joined for the user |

## Expected Final State
One call "Acme weekly sync", its meeting opened by Echo once at its start.

## Teardown
- `kill $(cat "$D/app.pid")`; close the tab; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-REC-24), not from the code (#900).
- That Teams is silent on the speakers while such a call records is the tap's mute (`CATapMuteBehavior::Muted`), checked by `audio::macos::tests::a_muted_tap_still_hears_the_process` (ignored, run by hand: it plays a tone) — a file-fed call has no tap.
- What Echo clicks in Teams ("Add this device", the speaker on, Leave) comes in a later step of #900; until then the user presses "Add this device" on the Mac.

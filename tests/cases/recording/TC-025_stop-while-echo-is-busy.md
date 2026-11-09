---
id: TC-025
title: A Stop while Echo is busy for a moment ends the recording once it is free and leaves the next meeting armed
priority: high
type: functional
requirements: [F-REC-05]
module: recording
run: agent
platform: macos
tags: [recording, stop, file-fed]
---

# TC-025: A Stop while Echo is busy ends the recording once it is free

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- The same setup as `recording/TC-015`: test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files, the debug build, the speech model linked, a second app instance on the test data. The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=rec-025-busy`
- No call is on, nothing armed or recording

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held |
| Arm titles | REC-025 busy (this call); REC-025 next (armed during the recording, for the next call) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"`, `target/debug/echoctl arm --title "REC-025 busy"`, `$W 10 armed` | "waited … for app", then "waited … for armed" |
| 2 | Run the prelude, then `$C start acme-sync --hold; $W 20 recording $T; target/debug/echoctl arm --title "REC-025 next"` | The recording runs; the second arm prints "armed" |
| 3 | Run the prelude, then `kill -STOP $(cat "$D/app.pid"); sleep 8; target/debug/echoctl stop; ls "$ECHOCTL_HOME/.echo/stop.request" "$ECHOCTL_HOME/.echo/armed.json"` (the app is frozen as if a device call hung; its heartbeat is 8 s old) | `stop` prints "stopping", the folder and "Echo is busy for the moment"; never "nothing to stop" or "disarmed". Both files are listed |
| 4 | Run the prelude, then `kill -CONT $(cat "$D/app.pid"); $W 15 event recording_stopped; $W 10 app; target/debug/echoctl status` (`status` reads a watcher whose heartbeat is fresh again) | "waited … for event recording_stopped"; `watcher.recording` is null, `watcher.armed` true (REC-025 next is still armed) |
| 5 | Read `meta.json` of `$(ls -d "$ECHOCTL_HOME"/calls/*-$T)` | `stop_reason` is "user" |

## Expected Final State
The Stop given while the app was frozen ended the recording as soon as the app ran again; the next meeting's arm survived.

## Teardown
- Run the prelude, then `kill -CONT $(cat "$D/app.pid") 2>/dev/null; $C end; target/debug/echoctl stop; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- `kill -STOP` stands in for the watcher blocked on a device (a device listing or a stream close that hangs while a Bluetooth headset reconnects): the loop and its heartbeat stop alike.

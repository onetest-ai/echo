---
id: TC-026
title: A Stop while a capture device hangs ends the recording within seconds, and Echo stays responsive
priority: high
type: functional
requirements: [F-REC-05]
module: recording
run: agent
platform: macos
tags: [recording, stop, file-fed]
---

# TC-026: A Stop while a capture device hangs ends the recording within seconds

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- The same setup as `recording/TC-015`: test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files, the debug build, the speech model linked, a second app instance on the test data. The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=rec-026-hang`
- No call is on, nothing armed or recording
- **A device that hangs:** while `$ECHOCTL_TEST_CALL_FILE.stall` exists, each test track's device check blocks until it is gone, as a device listing does while a Bluetooth headset reconnects

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held |
| Arm titles | REC-026 hang (this call); REC-026 next (armed during the recording, for the next call) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null \| wc -l > "$D/ev.start"`, `target/debug/echoctl arm --title "REC-026 hang"`, `$W 10 armed` | "waited … for app", then "waited … for armed" |
| 2 | Run the prelude, then `$C start acme-sync --hold; $W 20 recording $T; target/debug/echoctl arm --title "REC-026 next"` | The recording runs; the second arm prints "armed" |
| 3 | Run the prelude, then `touch "$ECHOCTL_TEST_CALL_FILE.stall"; sleep 8; $W 3 app` (the devices have hung for 8 s) | "waited … for app": the app still answers |
| 4 | Run the prelude, then `target/debug/echoctl stop; $W 5 event recording_stopped; ls "$ECHOCTL_TEST_CALL_FILE.stall"` | `stop` prints "stopping" and the folder, without "Echo is busy"; "waited … for event recording_stopped" within 5 s; the stall file is still listed (the devices still hang) |
| 5 | Run the prelude, then `target/debug/echoctl status` | `watcher.recording` is null, `watcher.armed` true (REC-026 next is still armed) |
| 6 | Run the prelude, then `rm "$ECHOCTL_TEST_CALL_FILE.stall"; CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; $W 120 file "$CALL/transcript.md"` and read `$CALL/meta.json` | The transcript is written once the devices answer again; `stop_reason` is "user" |

## Expected Final State
The Stop given while the devices hung ended the recording at once; the app answered throughout; the next meeting's arm survived.

## Teardown
- Run the prelude, then `rm -f "$ECHOCTL_TEST_CALL_FILE.stall"; $C end; target/debug/echoctl stop; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- `recording/TC-025` is the other half: the whole app frozen (`kill -STOP`), where the Stop waits until it is free.

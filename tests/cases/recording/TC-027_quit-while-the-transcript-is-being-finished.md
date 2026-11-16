---
id: TC-027
title: Quitting Echo while a call's transcript is being finished ends cleanly, and the transcript is made at the next start
priority: medium
type: functional
requirements: [F-REC-19]
module: recording
run: agent
platform: macos
tags: [recording, quit, transcription, file-fed]
---

# TC-027: Quitting while the transcript is being finished ends cleanly

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- The same setup as `recording/TC-015`: test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files, the debug build, a local speech model linked, a second app instance on the test data. The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=rec-027-quit`
- No call is on, nothing armed or recording

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held |
| Arm title | REC-027 quit |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `ls ~/Library/Logs/DiagnosticReports \| grep -c '^echoctl' > "$D/crashes.before"`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl arm --title "REC-027 quit"`, `$W 10 armed` | "waited … for app", then "waited … for armed" |
| 2 | Run the prelude, then `$C start acme-sync --hold; $W 20 recording $T; sleep 12; target/debug/echoctl stop; target/debug/echoctl app quit; $W 30 no-app` (the quit right after the stop: the call's transcript is still being finished) | "Echo stopped", then "waited … for no-app" |
| 3 | Run the prelude, then `sleep 6; echo $(( $(ls ~/Library/Logs/DiagnosticReports \| grep -c '^echoctl') - $(cat "$D/crashes.before") )); ls "$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"` | `0`: no crash report from the quit; the folder has `mic.wav` and `remote.wav`, and no `transcript.md` yet (if the last pass was done before the quit, `transcript.md` is there already: the step still passes with `0`, and step 4 checks the same) |
| 4 | Run the prelude, then `nohup target/debug/echoctl app run --no-tray --background > "$D/app2.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app; CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; $W 180 file "$CALL/transcript.md"` | "waited … for file …/transcript.md": the next start transcribed the call |

## Expected Final State
The quit during the transcript's last pass left no crash report; the call's transcript was made at the next start.

## Teardown
- Run the prelude, then `$C end; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- Other processes may write crash reports of their own meanwhile: step 3 counts only `echoctl`'s, and a count above 0 is checked against its report (`ggml_metal_device_free` at exit is this case's failure).

---
id: TC-016
title: Stopping a capture by hand (Ctrl-C) keeps what was said so far
priority: high
type: functional
requirements: [F-REC-05, F-REC-07]
module: recording
run: agent
platform: macos
tags: [recording, stop, file-fed]
---

# TC-016: Stopping a capture by hand (Ctrl-C) keeps what was said so far

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`); `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed. `capture` needs no app; an app running on the same data (in S-18) records nothing meanwhile and doesn't disturb it
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- **In S-18:** the main call is on (started by stage 3, `privacy/TC-001`, unarmed, the app watching it): step 1's `$C end` and `$C start` back to back play a new conversation into it (the watcher sees the same call: its 2 s debounce), and the teardown is skipped (the main call goes on). On its own: step 1 starts the call (`$C end` prints "no call")

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held; the capture starts 2 s in and is stopped by SIGINT 20 s later |
| Title | REC-016 stop |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `$C end; $C start acme-sync --hold; sleep 2; target/debug/echoctl capture --title "REC-016 stop" > "$D/capture016.out" 2> "$D/capture016.err" & CAP=$!; sleep 20; kill -INT $CAP; wait $CAP; echo "exit $?"; $C status` (the sleeps are the call's timeline) | After the `kill -INT` the capture ends by itself (exit 0, or 134 after it has saved: app_profile.md); `capture016.err` says it is recording, then has a `mic.wav` line and a `remote.wav` line with seconds between 18 and 26; the call is still on (`in_call` true) |
| 2 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-016-stop)"; cat "$CALL/meta.json"` | `ended_at` is set and `stop_reason` is "user" (not "duration"); both tracks are listed with `frames` above 0 |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-016-stop)"; ls "$CALL"; python3 -c "import wave,array,sys;w=wave.open(sys.argv[1]);a=array.array('h',w.readframes(w.getnframes()));print(round(w.getnframes()/w.getframerate(),1),max(map(abs,a)))" "$CALL/remote.wav"` | `mic.wav` and `remote.wav` are kept for the transcription that would follow; `remote.wav` is 18–26 s long and not silent (its peak above 1000): what was said up to the stop, and nothing after |

## Expected Final State
A capture ended by Ctrl-C has `stop_reason` "user" and both tracks closed at the stop, holding what was said so far.

## Teardown
- Run the prelude, then `$C end` (in S-18 skipped: the main call goes on)

## Notes
- Run on 2026-11-07 (S-18): PASS (`capture` exits 134 after saving; see `recording/TC-014`).
- Since 2026-11-08 no transcription here: the tracks show where the recording ended. A recording stopped by hand and then transcribed (ending where it was stopped) is `recording/TC-015`, through the app.
- This is the capture command's version of Stop. The app's Stop (`echoctl stop`) is `recording/TC-015`, the pill's "Stop recording?" `recording/TC-005`.

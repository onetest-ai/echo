---
id: TC-011
title: A crash while recording is repaired, and transcribed up to the crash
priority: low
type: functional
requirements: [F-REC-11]
module: recording
run: agent
platform: macos
tags: [recording, lifecycle, file-fed]
external_id: TC-REC-012
---

# TC-011: A crash while recording is repaired, and transcribed up to the crash

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`); `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed. `capture` needs no app; an app running on the same data (in S-18) records nothing meanwhile
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en` (in S-18: the prelude did)
- **In S-18:** the main call is on (stage 3): step 1's `$C end` and `$C start` back to back play a new conversation into it, and the teardown is skipped (the main call goes on). On its own: step 1 starts the call

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held; `echoctl capture` starts 2 s in and is killed with SIGKILL 15 s later |
| Title | REC-011 crash |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `$C end; $C start acme-sync --hold; sleep 2; target/debug/echoctl capture --title "REC-011 crash" > "$D/capture011.out" 2> "$D/capture011.err" & CAP=$!; sleep 15; kill -9 $CAP; wait $CAP; echo "exit $?"; cat "$D/capture011.out"` (the sleeps are the call's timeline) | `exit 137`: the capture process is gone after the `kill -9` and printed no call folder |
| 2 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; ls "$CALL"; cat "$CALL/meta.json"` | The folder holds `meta.json`, `mic.wav` and `remote.wav`, no `transcript.md`; `meta.json` has `ended_at` null and `stop_reason` null (the recording was never closed) |
| 3 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; target/debug/echoctl transcribe "$CALL"; cat "$CALL/meta.json"; ls "$CALL"` | "done in …" with no error; `meta.json` now has `ended_at` set and `stop_reason` "recovered"; the folder has `transcript.md` and `segments.json` and no `.wav` |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; grep -c "checklist" "$CALL/transcript.md"; grep -c -e "Madrid" -e "training room" "$CALL/transcript.md"` | The first count is 1 or more (said before 17 s on the call's clock); the second is 0 (said after 18 s, after the crash) |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-011-crash)"; cat "$CALL/segments.json"` | Every segment's `start` is below 17.0: nothing is dated after the crash |

## Expected Final State
The unfinished recording was closed as "recovered", its audio repaired and transcribed up to the crash, and no audio kept.

## Teardown
- Run the prelude, then `$C end` (in S-18 skipped: the main call goes on)

## Notes
- Run on 2026-11-07 (S-18): PASS — `stop_reason` "recovered", last segment at 12.5 s.
- Repair here goes through `transcribe` (`watch::close_unfinished`, called from `transcribe.rs`). The repair when the app starts again (event `recording_recovered`, `watch.rs`) is not covered by an agent case yet; it needs the second app instance started after the crash.
- The original step 3, a failed transcription showing its error instead of retrying forever, needs a way to make a transcription fail and is not covered here.
- `meta.json` of a recovered call keeps `tracks` empty: the tracks are written into it only when a recording closes normally (`recorder.rs`, `finish`).

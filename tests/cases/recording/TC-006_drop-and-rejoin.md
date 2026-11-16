---
id: TC-006
title: A short drop and rejoin stays one recording
priority: high
type: functional
requirements: [F-REC-08]
module: recording
run: agent
platform: macos
tags: [recording, second-instance, file-fed]
external_id: TC-REC-006
auto: [tests/auto/specs/S-18.spec.mjs]
---

# TC-006: A short drop and rejoin stays one recording

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` (in S-18: the prelude did)
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- No call is on, nothing armed or recording
- **In S-18:** the recording is the one `recording/TC-012` (stage 10) started with "Record" in Echo's dialog, in a call of acme-sync that is still on; steps 1–2 are skipped (the app, the arm and the call), `$D/ev.start` and `$D/before.txt` are stage 10's, and the stage's glue writes `$D/rec.call` (the recording's folder) as step 2 does

## Test Data

| Field | Value |
|-------|-------|
| First conversation | acme-sync (36 s), held until it has run out (38 s) |
| After the drop | two-remotes (34 s), started 4 s after the hang-up (within the 10 s `REJOIN_GRACE`, `watch.rs`), held until it has run out |
| Arm title | REC-006 rejoin |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `recording` null and `armed` false; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; ls "$ECHOCTL_HOME/calls" > "$D/before.txt"; target/debug/echoctl arm --title "REC-006 rejoin"; $W 10 armed; $C start acme-sync --hold; $W 20 recording rec-006-rejoin; target/debug/echoctl status | python3 -c 'import json,sys;print(json.load(sys.stdin)["watcher"]["recording"])' > "$D/rec.call"; cat "$D/rec.call"` | The recording's folder, ending `-rec-006-rejoin` |
| 3 | Run the prelude, then `sleep $(python3 -c "import json,subprocess;print(max(0,38-json.loads(subprocess.check_output(['python3','$C','status']))['for_s']))"); $C end; sleep 4; $C start two-remotes --hold; sleep 5; target/debug/echoctl status; cat "$D/rec.call"` (the sleeps are the call's timeline: the first conversation runs out, the drop, the rejoin 4 s later, 5 s into it) | At 5 s into the second call `watcher.in_call` is true and `watcher.recording` is still the folder in `$D/rec.call` |
| 4 | Run the prelude, then `sleep 31; $C end; $W 30 event recording_stopped; $W 120 event transcribed; diff "$D/before.txt" <(ls "$ECHOCTL_HOME/calls"); tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'` (the `sleep 31` lets the second conversation run out) | One new folder only (the recording's); after its `recording_started` the events are, in order, `call_ended`, `call_resumed`, `call_ended`, `recording_stopped`, `transcribed`: one `recording_started`, one `recording_stopped` |
| 5 | Run the prelude, then `CALL="$(cat "$D/rec.call")"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"; grep -c -e "Omar is joining" -e "pilot plan" "$CALL/transcript.md"; python3 -c "import json,sys;[print(round(s['start']),s['text'][:50]) for s in json.load(open(sys.argv[1]))]" "$CALL/segments.json"` | Both counts 2 or more: both conversations are in the one transcript; the first conversation's last line ("…training room for Thursday") is followed by the second's first ("Hi everyone, Omar is joining us from the security team today.", one segment or split in two) with no other segment between them: the gap is silence in the timeline |

## Expected Final State
A call dropped and rejoined within seconds is one recording, one folder and one transcript, with the gap as silence.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when step 4 hung up)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")` (in S-18 too: the last stage that needs the app)

## Notes
- The "rejoin" is a second call started within the grace; a real Teams drop (the same meeting, the same window) remains a person's check once.
- Run on 2026-11-07 (S-18, TestCall): PASS — `call_ended` 13:12:32, `call_resumed` 13:12:35, one recording of 93 s; nothing transcribed between 36 s and 44 s.
- **File-fed calls (2026-11-08):** the call file made anew at the rejoin reopens the recording's sources on the new
  conversation and its clock (`audio/testfeed.rs`), so the words after the rejoin reach the same recording.
- Run 2026-11-08 (S-18, file-fed, on `recording/TC-012`'s recording): steps 3–4 PASS (one folder; `call_ended`, `call_resumed`, `call_ended`, `recording_stopped`, `transcribed`; `stop_reason` "call_ended"); step 5 FAIL — "Berlin first"/"training room" 2, "Omar is joining"/"pilot plan" 0: the test sources didn't follow the new call file yet (fixed the same day; a rejoin by hand then had both conversations in one folder).

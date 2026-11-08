---
id: TC-006
title: A short drop and rejoin stays one recording
priority: high
type: functional
requirements: [F-REC-08]
module: recording
run: agent
platform: macos
tags: [recording, second-instance, test-call]
external_id: TC-REC-006
---

# TC-006: A short drop and rejoin stays one recording

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; W=tests/fixtures/wait.py; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed

## Test Data

| Field | Value |
|-------|-------|
| First call | acme-sync (36 s) |
| After the drop | two-remotes (34 s), dialled 4 s after the first call hung up (within the 10 s `REJOIN_GRACE`, `watch.rs`) |
| Arm title | REC-006 rejoin |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build acme-sync "$CONV"; python3 tests/fixtures/audio/make_audio.py build two-remotes "$D/conv2"` | Both print their folder, length and turns |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `ls "$ECHOCTL_HOME/calls" > "$D/before.txt"; target/debug/echoctl arm --title "REC-006 rejoin"; $W 10 armed; "$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait; sleep 4; "$APP" serve --port 7792 --play "$D/conv2/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$D/conv2/mic.wav" --me-device "BlackHole 2ch" & sleep 5; target/debug/echoctl status; wait` | At 5 s into the second call `watcher.in_call` is true and `watcher.recording` is still the folder ending `-rec-006-rejoin` |
| 4 | Run the prelude, then `$W 120 event transcribed; diff "$D/before.txt" <(ls "$ECHOCTL_HOME/calls"); tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'` | One new folder only (`…-rec-006-rejoin`); events in order `call_started`, `recording_started`, `call_ended`, `call_resumed`, `call_ended`, `recording_stopped`, `transcribed`: one `recording_started`, one `recording_stopped` |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-006-rejoin)"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"; grep -c -e "Omar is joining" -e "pilot plan" "$CALL/transcript.md"; python3 -c "import json,sys; print([round(s['start']) for s in json.load(open(sys.argv[1]))])" "$CALL/segments.json"` | Both counts 2 or more: both calls are in the one transcript; no segment starts in the gap between them (about 37–43 s): it is silence in the timeline |

## Expected Final State
A call dropped and rejoined within seconds is one recording, one folder and one transcript, with the gap as silence.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- TestCall serves one call and exits, so the "rejoin" is a second TestCall call started within the grace; a real Teams drop (the same meeting, the same window) remains a person's check once.
- Run on 2026-11-07 (S-18): PASS — `call_ended` 13:12:32, `call_resumed` 13:12:35, one recording of 93 s; nothing transcribed between 36 s and 44 s.

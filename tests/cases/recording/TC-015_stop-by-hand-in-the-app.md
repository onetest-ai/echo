---
id: TC-015
title: echoctl stop ends the recording, not the call, and the call isn't recorded again
priority: critical
type: functional
requirements: [F-REC-05]
module: recording
run: agent
platform: macos
tags: [recording, stop, second-instance, file-fed]
---

# TC-015: echoctl stop ends the recording, not the call, and the call isn't recorded again

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=rec-015-stop` (`T`: the recording's folder suffix)
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- No call is on, nothing armed or recording
- **In S-18:** the recording is `cli/TC-004`'s (stage 7): armed before the call from the terminal, stopped with `echoctl stop` 20 s into the call, the call still on. Steps 1–3 are skipped (stage 7 started the app, armed, started the call, stopped it and checked the arm's and the stop's output), `T=cli-004-terminal`, and `$D/ev.start` is stage 7's

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held; stopped 20 s into the call, hung up after the conversation |
| Arm title | REC-015 stop |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `recording` null and `armed` false; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; target/debug/echoctl arm --title "REC-015 stop"; $W 10 armed` | "waited … for armed" |
| 3 | Run the prelude, then `$C start acme-sync --hold; $W 20 recording $T; sleep 19; target/debug/echoctl stop` (the `sleep 19` is the call's timeline: the stop about 20 s in) | stop prints "stopping" and the folder |
| 4 | Run the prelude, then `$W 10 not-recording; target/debug/echoctl status; $C status` | `watcher.recording` is null, `watcher.in_call` is still true (the call goes on), `armed` is null; `call.py status` has `in_call` true |
| 5 | Run the prelude, then `sleep $(python3 -c "import json,subprocess;print(max(0,38-json.loads(subprocess.check_output(['python3','$C','status']))['for_s']))"); target/debug/echoctl status; $C end; $W 30 event call_ended; ls "$ECHOCTL_HOME/calls" | grep -c -- "-$T\$"; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -c recording_started; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -c recording_stopped` (the sleep lets the conversation run out: 38 s on the call's clock) | Before the hang-up `watcher.recording` is still null; after it, the folder count is 1 and both event counts are 1: the call ran on after the stop to the conversation's end and was not recorded again; Echo did not re-arm |
| 6 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; $W 120 file "$CALL/transcript.md"; ls "$CALL"` | Within 2 minutes the folder has `transcript.md` and `segments.json`, and no `.wav` file (the app transcribed it and deleted the audio) |
| 7 | Read `meta.json` of that folder | `stop_reason` is "user" |
| 8 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; grep -c "checklist" "$CALL/transcript.md"; grep -c -e "Madrid" -e "training room" -e "Thursday" "$CALL/transcript.md"` | The first count is 1 or more; the second is 0: the transcript ends where it was stopped |

## Expected Final State
Stop ended that recording only; the rest of the same call was not recorded and nothing was re-armed.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when step 5 hung up)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")` (in S-18 skipped: the prelude's app goes on)

## Notes
- Run on 2026-11-07 (S-18): PASS.
- 2026-11-08: the arm's message ("armed — recording starts when the call begins") and the stop's output are checked in `cli/TC-004`, the terminal's case, whose recording this case reads in S-18; here steps 2–3 only reach the state (a recording stopped by hand 20 s in).
- The pill's confirmation "Stop recording?" (Stop / Keep recording) is `recording/TC-005`. A capture stopped with Ctrl-C is `recording/TC-016`.

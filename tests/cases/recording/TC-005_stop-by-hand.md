---
id: TC-005
title: Stopping by hand ends the recording, not the call
priority: critical
type: functional
requirements: [F-REC-05]
module: recording
run: agent
platform: macos
tags: [recording, stop, second-instance, file-fed]
external_id: TC-REC-005
auto: [tests/auto/specs/S-18.spec.mjs]
---

# TC-005: Stopping by hand ends the recording, not the call

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Calls are fed from files (app_profile.md → "Recording with file-fed calls"): `<dir>/env` points the debug build's mic, call-app audio and call detection at files that `tests/fixtures/call.py` fills; no device is opened, no sound played, no TestCall or permission needed
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` (in S-18: the prelude did)
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 1 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The window is served on the same test data at `{{base_url}}` with clicks reaching the app (`ECHOCTL_SERVE_IPC=1`, `tests/fixtures/serve.sh up` or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first
- No call is on (`$C status`: `in_call` false), nothing armed or recording
- **In S-18:** the app and the page are the prelude's (step 1 skipped)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), held; stopped from the window before 28 s on the call's clock ("training room" is said from 30 s) |
| Arm title | REC-005 pill stop |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status`; load `{{base_url}}/` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `recording` null and `armed` false; no "already running" in `$D/app.out` |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null | wc -l > "$D/ev.start"; target/debug/echoctl arm --title "REC-005 pill stop"; $W 10 armed; $C start acme-sync --hold; $W 20 recording rec-005-pill-stop; target/debug/echoctl status` | `watcher.recording` is a folder ending `-rec-005-pill-stop` |
| 3 | In the top bar's group "Recording" click the stop button `[data-cmd="stop-ask"]` (no reload: the page follows the app) | The pill turns into "Stop recording?" with the buttons "Stop" (`.pill.rec [data-cmd="stop"]`) and "Keep recording" (`[data-cmd="stop-keep"]`) |
| 4 | Click "Keep recording", wait 3 s | The pill shows the recording again (its clock still running); `echoctl status` still has `watcher.recording` |
| 5 | Click the stop button again, then "Stop"; within 4 s run the prelude and `target/debug/echoctl status; $C status` | `watcher.recording` is null, `watcher.in_call` is still true (the call goes on), `armed` is null; `for_s` of `call.py status` is below 30 (else the stop came too late for step 7's check: record it and redo from step 2 after `$C end`) |
| 6 | Run the prelude, then `sleep $(python3 -c "import json,subprocess;print(max(0,38-json.loads(subprocess.check_output(['python3','$C','status']))['for_s']))"); $C end; $W 30 event call_ended; $W 120 event transcribed; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" | grep -o '"event":"[a-z_]*"'; ls "$ECHOCTL_HOME/calls" | grep -c rec-005` (the sleep lets the conversation run out) | `call_started`, `recording_started`, `recording_stopped` in that order, then `call_ended` and `transcribed` (either order): one recording only, nothing re-armed after the stop; one folder |
| 7 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-005-pill-stop)"; grep -o '"stop_reason": *"[a-z_]*"' "$CALL/meta.json"; grep -c -e "training room" -e "Thursday" "$CALL/transcript.md"` | `"stop_reason": "user"`; the count is 0: the transcript ends where it was stopped |

## Expected Final State
The window's Stop asked first, "Keep recording" kept it, "Stop" ended the recording while the call went on, and nothing more of that call was recorded.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when step 6 hung up)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")` (in S-18 skipped: the prelude's app goes on)

## Notes
- "The call goes on" is `watcher.in_call` staying true and `call_ended` coming after the stop. The tray's "Stop recording" is `recording/TC-019` (person); `echoctl stop` is `recording/TC-015`, Ctrl-C of a capture `recording/TC-016`.
- Run on 2026-11-07 (S-18): PASS — stop at 13:03:12 after a "Keep recording", call ended 13:03:36, one recording, `stop_reason` "user", no Madrid line.
- 2026-11-08: the words checked after the stop are the last line's ("training room", "Thursday", from 30 s), not "Madrid" (from 23 s): the clicks of steps 3–5 take a runner about 20 s.
- The pill's "Stop recording?" and "Keep recording" are the page's own (`S.stopAsk`, `90-render.js`); only "Stop" reaches the app (`POST /ipc` `{"cmd":"stop"}` → `watch::stop`).

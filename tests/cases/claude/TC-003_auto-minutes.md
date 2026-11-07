---
id: TC-003
title: Minutes are written automatically 15 minutes after a call
priority: critical
type: functional
requirements: [F-MIN-14, F-MIN-07]
module: claude
run: agent
platform: macos
tags: [minutes, auto, second-instance, test-call]
ai: local
external_id: TC-CLA-003
---

# TC-003: Minutes are written automatically 15 minutes after a call

**Module:** Claude Code | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (a local model server: Ollama here, or the faster one of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- The window is served on the same test data at `{{base_url}}` with clicks reaching the app (`ECHOCTL_SERVE_IPC=1`, `tests/fixtures/serve.sh up` or the scenario's server), the browser open on it; the page is loaded from `{{base_url}}/` first

## Test Data

| Field | Value |
|-------|-------|
| Conversation | minutes-call (213 s; a decision and action items with owners and dates) |
| Arm title | MIN-A catch-up (the call of `claude/TC-009` when both run in S-17) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude | — |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes true`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `sleep 6`, `target/debug/echoctl status` | `auto_minutes` true (switched on now: calls that ended before stay without automatic minutes); `status` prints `watcher` with a `pid`; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build minutes-call "$D/mc"`, `target/debug/echoctl arm --title "MIN-A catch-up"`, `sleep 2`, `CONV="$D/mc" nohup bash -c '"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & wait' > /dev/null 2>&1 &`, `sleep 10`, `target/debug/echoctl status` | minutes-call is 213 s (36 turns); `watcher.recording` is a folder ending `-min-a-catch-up` |
| 4 | Run the prelude, then `until [ -f "$(ls -d "$ECHOCTL_HOME"/calls/*-min-a-catch-up)/transcript.md" ]; do sleep 5; done; grep -o '"ended_at": *"[^"]*"' "$(ls -d "$ECHOCTL_HOME"/calls/*-min-a-catch-up)/meta.json"` | The transcript is there within about 5 s of the call's end; `ended_at` is set (call it E) |
| 5 | In the window: `{{base_url}}/`, then `#calls`, click the row `[role=option][data-call$="min-a-catch-up"]`, open its Minutes tab | "Minutes in 15 min (HH:MM)" with HH:MM = E + 15 minutes, the line "Echo writes them 15 minutes after the call or your last note…", the profile ("Qwen · …"), "Write now" (`[data-cmd="write-notes"]`) and "Not yet ▾"; nothing is pressed |
| 6 | Wait until E + 15 minutes without touching the call (`until [ -f "$(ls -d "$ECHOCTL_HOME"/calls/*-min-a-catch-up)/notes-run.json" ]; do sleep 5; done`), then read that `notes-run.json` | `"auto": true`, `started_at` within 30 s after E + 15 minutes (the scheduler ticks every 15 s), never before |
| 7 | Wait until its `state` is "done" (a few minutes on the model server); `ls` the call's folder; read `notes.md` | `notes.md` and `actions.json` are there; Summary, Decisions and Action items with time chips (`[mm:ss]`), the action items with owners and dates said in the call (Me — the SSO checklist today before 4; Dana — the training room on Thursday; Omar — the pilot plan by Friday) |
| 8 | Read the call's `claude.log` (the window's "Show log") | Its first line: "Echo starts Claude Code to write the minutes (automatically)", with the model, the folder and "guard: every tool call is checked…"; the commands it ran are `echoctl context`, `type`, `actions` (and `title`) on this call, the reads and writes in its folder only; no `m365`, `compose`, `deliver` or other send; it ends "Minutes written." |

## Expected Final State
With no one asking, the minutes were written once, at the time Echo named, from the call alone, and nothing was sent.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`

## Notes
- The 15 minutes are `notes::AUTO_DELAY`; no setting shortens them: the case waits them out (run it beside other stages).
- The "Transcript ready" and "Minutes ready" notifications are not checked here (macOS shows them to the user; a check needs a look at the screen: `lifecycle/TC-006`).
- Run on 2026-11-07 (S-17, the network model server): PASS — call ended 12:35:21, the tab said "Minutes in 15 min (12:50 PM)", the run started 12:50:26 (`auto: true`) and wrote the minutes in 116 s; no send.

---
id: TC-019
title: Live notes don't run on a model on this computer while the call records
priority: high
type: functional
requirements: [F-LIVE-07]
module: claude
run: agent
platform: macos
tags: [live, ai-profiles, performance, file-fed]
---

# TC-019: Live notes skip a model on this computer while recording

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Test data from `tests/fixtures/make_data.py <dir> --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); `target/debug/echoctl` built from this checkout. The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- The speech model linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0`
- A second app instance on the test data, as in `recording/TC-015` (if `$D/app.out` says "already running", BLOCKED)

## Test Data

| Field | Value |
|-------|-------|
| AI profile | `here`: a model server on this computer at `http://127.0.0.1:9` (nothing listens there), the default |
| Conversation | minutes-call (213 s), held |
| Arm title | LIVE-019 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl ai add-local here --url http://127.0.0.1:9 --model m`, `target/debug/echoctl ai default here`, `target/debug/echoctl ai live on`, `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false` | "added m · this computer (here)"; "new calls use m · this computer"; "live notes during calls: on" |
| 2 | Run the prelude, then `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app; target/debug/echoctl arm --title "LIVE-019"; $C start minutes-call --hold; $W 20 recording live-019` | "waited … for recording live-019" |
| 3 | Run the prelude, then `sleep 5; target/debug/echoctl live show` | The state is already "local" (decided when the recording started); no notes |
| 4 | Run the prelude, then `sleep 100; target/debug/echoctl live show; ls "$(ls -d $ECHOCTL_HOME/calls/*-live-019)" \| grep -c live-notes.json` (past the first summary's 90 s) | Still "local", no notes; 0: no summary was written |
| 5 | Run the prelude, then `ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve 48090 > "$D/serve.log" 2>&1 & echo $! > "$D/serve.pid"`; load `http://127.0.0.1:48090/`, then navigate to `http://127.0.0.1:48090/#settings/ai` and read the row "Live notes during calls" | Its description ends "Not with m · this computer: its model runs on this computer, where it would slow your calls down." |

## Expected Final State
The call recorded with live notes on and no summary was made: the profile's model is on this computer.

## Teardown
- Run the prelude, then `kill $(cat "$D/serve.pid"); $C end; $W 60 not-recording; target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- The app window's My notes box says "No live notes: this recording's model runs on this computer, where it would slow your call down…" with the state "Off: the model is on this computer" (F-LIVE-02, person). A window served with `--serve` can't show it: the live state lives in the app's process, not the serving one.
- A profile on another computer, or Claude, still makes summaries during the call: Rust test `livenotes::a_summary_runs_end_to_end_and_failures_keep_the_draft`.

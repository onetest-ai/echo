---
id: TC-036
title: Google Meet installed as an app from Chrome is a call source of its own — the app open makes Chrome on the mic its call ("meet-app"); a Meet tab's title wins over the app open beside it
priority: high
type: functional
requirements: [F-REC-21, F-REC-02]
module: recording
run: agent
platform: macos
data: small
tags: [recording, google-meet, chrome, meet-app, watcher]
---

# TC-036: The Meet app is a call source of its own

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data: `tests/fixtures/serve.sh up <dir> <port> --set small --ai fake`; calls fed from files (app_profile.md → "Recording with file-fed calls"); no speech model needed (nothing here is checked in a transcript)
- Every Bash call starts with the prelude of `recording/TC-001`
- **Needs a second app instance** (as recording/TC-002): BLOCKED, not failed, if `$D/app.out` says "already running". Step 1 starts it
- `--meet-app` stands for Google Meet installed from Chrome being open (its own process, `Google Meet.app`): the call file's `meet_app=1`; its window shows no meeting, so the call has no `--title`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; `target/debug/echoctl ai live off`; `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`; `$W 30 app` | "waited … for app"; no "already running" in `$D/app.out` |
| 2 | Prelude; `target/debug/echoctl arm --title "REC-036 app"`; `$W 10 armed`; `$C start very-short --meet --meet-app --hold`; `$W 20 recording rec-036-app`; `target/debug/echoctl status` | `call.py` prints `"app": "meet"`; within 20 s `watcher.app` "meet-app", `watcher.recording` a folder ending `-rec-036-app` |
| 3 | Prelude; `$C end`; `$W 40 not-recording`; `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-036-app)"`; read `"$CALL/meta.json"` | `app` "meet-app", `stop_reason` "call_ended"; `tracks` lists `mic.wav` and `remote.wav`, each with `frames` above 0 |
| 4 | Prelude; `target/debug/echoctl arm --title "REC-036 tab"`; `$W 10 armed`; `$C start very-short --meet --meet-app --title "Meet – tst-wxyz-abc - Google Chrome" --hold`; `$W 20 recording rec-036-tab`; `target/debug/echoctl status` | `watcher.app` "meet": a Meet tab's title wins over the app open beside it |
| 5 | Prelude; `$C end`; `$W 40 not-recording`; `python3 -c 'import json,sys;print(json.load(open(sys.argv[1]+"/meta.json"))["app"])' "$(ls -d "$ECHOCTL_HOME"/calls/*-rec-036-tab)"` | `meet` |

## Expected Final State
Two recordings: one from the Meet app ("meet-app"), one from a Meet tab ("meet").

## Teardown
- `target/debug/echoctl app quit`; if it doesn't stop within 30 s, `kill $(cat "$D/app.pid")`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The app's sound is Chrome's, as a tab's (one audio process for every Chrome window): only the source's name, its sign and its warning differ. The real app is checked by `recording/TC-034` step 7 (person).

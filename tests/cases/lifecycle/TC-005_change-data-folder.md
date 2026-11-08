---
id: TC-005
title: Changing the data folder restarts on it and leaves the old one alone
priority: low
type: functional
requirements: [F-LIFE-05]
module: lifecycle
run: agent
platform: macos
tags: [lifecycle, data-folder, second-instance]
external_id: TC-LIFE-005
---

# TC-005: Changing the data folder restarts on it and leaves the old one alone

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` right before the run (hermetic: the app's own Claude Code jobs reach no account); `target/debug/echoctl` is the debug build from this checkout; every command after `. <dir>/env` (the prelude of `recording/TC-001`, with `D="$(dirname "$ECHOCTL_HOME")"`)
- `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false` and `target/debug/echoctl ai live off` (no dialog, no run of its own)
- The dev app is started as `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, beside the user's own Echo (its own config folder, so its own `app.lock`); never `app start` (it opens the installed Echo.app)
- **This case runs with `ECHOCTL_CONFIG_DIR` only**: after `. <dir>/env`, `unset ECHOCTL_HOME; export ECHOCTL_TEST_OPEN_LOG_ONLY=1` in every Bash call (the variable would pin the data folder; without it the folder is the one `<config>/home.json` names, else the config folder itself — never the user's). `ECHOCTL_TEST_OPEN_LOG_ONLY` keeps the test `open` (`<dir>/bin/open`) from opening anything: on a Mac with Echo installed the app's restart opens `/Applications/Echo.app` (`app::spawn_app`)
- The speech model linked into the test config (`recording/TC-001` step 1); the call in step 4 is fed from files (`app_profile.md` → "Recording with file-fed calls"): `tests/fixtures/call.py` needs `ECHOCTL_HOME` for the conversation's folder, so it runs in a subshell with the env as it is (`(. <dir>/env; tests/fixtures/call.py start very-short)`), which changes nothing for the app
- **Its own app.** It can't reuse an app started with `ECHOCTL_HOME` set (the variable pins the data folder): in S-21 the app of stages 1–2 is quit, and this case starts its own twice (step 2, and again after the restart in step 3)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl home set "$D/home"`; `target/debug/echoctl home`; `(cd "$D/home" && find calls -type f \| sort \| xargs shasum) > "$D/old-home.sha"` | "data folder: …/home"; `home_source` "chosen" |
| 2 | Start the dev app (as above), `python3 tests/fixtures/wait.py 30 app`, `target/debug/echoctl status` | `watcher.pid` set |
| 3 | `: > "$D/open.log"; target/debug/echoctl home set "$D/home2"; python3 tests/fixtures/wait.py 15 sh "! ps -p $(cat "$D/app.pid") > /dev/null"; ps -p "$(cat "$D/app.pid")"; cat "$D/open.log"` | "Echo stopped (was pid …)", "data folder: …/home2"; the app quit to restart. On a Mac with Echo installed the restart is `open -n --env ECHOCTL_WAIT_FOR_LOCK=1 -g /Applications/Echo.app` (in `open.log`, not run), so the command ends "Error: Echo did not start"; start the dev app again by hand (as above) |
| 4 | `target/debug/echoctl home`; arm and run a call: `target/debug/echoctl arm --title "LIFE-005 new folder"; python3 tests/fixtures/wait.py 10 armed; (. <dir>/env; tests/fixtures/call.py start very-short); python3 tests/fixtures/wait.py 60 file "$D/home2/calls/*-life-005-new-folder/transcript.md"; ls "$D/home2/calls"` | `home` is …/home2; the 9 s call is recorded, stopped and transcribed (within about 25 s) in one folder ending `-life-005-new-folder` in `home2/calls` |
| 5 | `(cd "$D/home" && find calls -type f \| sort \| xargs shasum) \| diff - "$D/old-home.sha"` | No difference: the old folder's calls are untouched |
| 6 | `target/debug/echoctl app quit` | "Echo stopped" |

## Expected Final State
The data folder moved without losing or mixing calls.

## Notes
- The tray's Settings → "Change data folder…" and `echoctl home choose` open a native folder dialog; `home set` is the same change (`paths::set_home`).
- Run 2026-11-08 evening (S-21, the call fed from files): PASS — the folder and its transcript in `home2/calls` 24 s after the call started; the old folder's 16 files unchanged; the restart logged `-n --env ECHOCTL_WAIT_FOR_LOCK=1 -g /Applications/Echo.app` (#142). The wait is for `transcript.md`, not `meta.json`: the app is quit right after, and must not be quit mid-recording.
- Run on 2026-11-07 (S-21): PASS on these steps — but a dev build's restart went to the installed Echo.app (step 3), #142, a defect for dev builds: with the user's Echo installed it would start a second copy of it.

---
id: TC-217
title: A plugin that asked reads what Echo records now — idle, a call transcribing, a memo recording — with the call's folder and stamp
priority: medium
type: functional
requirements: [F-PLUG-128]
module: plugins
run: agent
platform: macos
data: small
tags: [plugins, host-half, recording, second-instance, file-fed]
---

# TC-217: A plugin that asked reads what Echo records now

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set built right before the run, nothing served: `tests/fixtures/make_data.py <dir> --ai fake`; every Bash call starts with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D` and aborts unless `$ECHOCTL_HOME` is inside `<dir>`; `echoctl` is `target/debug/echoctl`, built right before the run
- Run demo installed (it asks `uses.echo.read: recording`): `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install run-demo --yes`
- **Needs a second app instance** for step 4 (the watcher records the memo), as in `recording/TC-029`; if `$D/app.out` says "already running" the case is BLOCKED, not failed. The memo's mic is the debug build's file feed (no device opened)

**Changes:** `<config dir>/plugins/`; a memo folder in `$ECHOCTL_HOME/calls/`

## Test Data

| Field | Value |
|---|---|
| Call | `<stamp>-acme-weekly-sync` (today's, its stamp the run's start time) |
| Memo | `<stamp>-memo`, recorded from the file feed |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call run-demo recording` | `{"call": null, "stamp": null, "state": "idle"}` |
| 2 | `F="$(ls -d "$ECHOCTL_HOME"/calls/*-acme-weekly-sync)"; nohup sleep 120 >/dev/null 2>&1 & echo $! > "$F/.transcribing"; echoctl plugin call run-demo recording` (the marker Echo keeps while a call transcribes, naming a live process) | `state` "transcribing", `call` the folder's name (`…-acme-weekly-sync`), `stamp` its first 17 characters (`YYYY-MM-DD_HHMMSS`) |
| 3 | `kill "$(cat "$F/.transcribing")"; echoctl plugin call run-demo recording; rm "$F/.transcribing"` | `state` "idle", `call` and `stamp` null: a marker whose process is gone is no transcription |
| 4 | `$C start acme-sync --hold; $C end; echoctl config --unscheduled manual --scheduled manual; echoctl ai live off; nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app; echoctl memo start; $W 20 recording memo; echoctl plugin call run-demo recording` | "waited … for recording memo"; `state` "recording", `call` the memo's folder (`…-memo`), `stamp` its first 17 characters |
| 5 | `echoctl memo stop; $W 15 not-recording; echoctl plugin call run-demo recording` | `state` "idle" (or "transcribing" with the memo's `call` while its transcript is made) |

## Expected Final State
Nothing records; the app quit in the teardown.

## Teardown
- Run the prelude, then `$C end`; `echoctl app quit` (if it doesn't stop within 30 s, `kill $(cat "$D/app.pid")`); `echoctl plugin remove run-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; remove `<dir>`

## Notes
- A plugin that didn't ask is refused (even one hearing `transcribed`), and the read has its line on the sheet: Rust, `jshost::a_plugin_that_asked_reads_what_echo_records_now_and_no_other_plugin_does`.

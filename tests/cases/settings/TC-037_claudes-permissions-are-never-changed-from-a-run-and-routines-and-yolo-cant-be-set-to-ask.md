---
id: TC-037
title: "Claude's permissions: `echoctl config` refuses them inside any run Echo started, and refuses Ask each time for routines and Yolo; nothing is written"
priority: high
type: functional
requirements: [F-SET-24]
module: settings
run: agent
platform: any
data: standard
tags: [settings, claude, permissions, cli]
---

# TC-037: Claude's permissions are never changed from a run, and routines and Yolo can't be set to ask

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data (`tests/fixtures/serve.sh up <dir> <port> --set standard`; no window step)
- CLI steps in Bash after `. <dir>/env`; `E=target/debug/echoctl`, built right before the run; `C="$ECHOCTL_HOME/.echo/config.json"`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -E '"(folder\|routine\|yolo)_permissions"' "$C"` | What the settings are now (on fresh data only `"folder_permissions": "ask",`); noted for step 4 |
| 2 | `$E config --routine-permissions ask; echo "exit $?"`; `$E config --yolo-permissions ask; echo "exit $?"` | Each refused by its own options: `invalid value 'ask'` … `[possible values: auto, skip]`, exit 2 — nobody is there to answer a routine's or Yolo's prompt |
| 3 | As inside a run Echo started: `ECHOCTL_HEADLESS=1 $E config --yolo-permissions skip; echo "exit $?"`; `ECHOCTL_HEADLESS=1 $E config --folder-permissions skip; echo "exit $?"` | Each says `refused inside a run Echo started: how Claude handles permissions is the user's to choose`, exit 1 |
| 4 | The `grep` of step 1 again | The same lines as in step 1: nothing written |

## Expected Final State
The settings as they were.

## Teardown
- On its own: `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `grep` and `echo`.
- That the window offers routines and Yolo no "Ask each time": `settings/TC-014` step 3.

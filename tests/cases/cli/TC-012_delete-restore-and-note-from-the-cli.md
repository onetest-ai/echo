---
id: TC-012
title: delete moves a call to the trash and restore brings it back, as the window shows; note without a recording says no watcher runs
priority: high
type: functional
requirements: [F-CLI-08]
module: cli
run: agent
platform: any
data: standard
tags: [cli, calls, trash]
---

# TC-012: delete moves a call to the trash and restore brings it back, as the window shows; note without a recording says no watcher runs

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `C=2026-11-05_090000-tyrell-follow-up`; no Echo app runs on this data folder (`window --serve` only)
- Reload the page fully (`{{base_url}}/`) after a CLI change before reading the window

## Test Data

| Field | Value |
|---|---|
| Call | Tyrell follow-up (Thursday 09:00, transcript, in no thread) |
| Already in the trash | `2026-11-04_170000-acme-draft-call` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, type `Tyrell` in "Search calls" | The row "Tyrell follow-up" is listed (`[data-call="2026-11-05_090000-tyrell-follow-up"]`) |
| 2 | Run `target/debug/echoctl delete $C` | Exit code 0; one JSON line with `deleted` "2026-11-05_090000-tyrell-follow-up", `restore` "echoctl restore 2026-11-05_090000-tyrell-follow-up" and `trash` ending `/.echo/trash/2026-11-05_090000-tyrell-follow-up` |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/trash"` | Lists `2026-11-04_170000-acme-draft-call` and `2026-11-05_090000-tyrell-follow-up` (and a `project-…` folder for each thread deleted earlier on this data, e.g. by `TC-009`) |
| 4 | Reload, search `Tyrell` in Calls | No row "Tyrell follow-up" |
| 5 | Run `target/debug/echoctl delete $C` again | Exit code 1; "Error: 2026-11-05_090000-tyrell-follow-up is not a call folder (expected meta.json inside)" |
| 6 | Run `target/debug/echoctl restore $C` | Exit code 0; prints the path ending `/calls/2026-11-05_090000-tyrell-follow-up` |
| 7 | Reload, search `Tyrell` in Calls | The row "Tyrell follow-up" is back |
| 8 | Run `target/debug/echoctl restore $C` again | Exit code 1; "Error: 2026-11-05_090000-tyrell-follow-up is not in <data folder>/.echo/trash" |
| 9 | Run `target/debug/echoctl note "Check the Tyrell budget"` | Exit code 1; "Error: no watcher running"; no call's `my-notes.md` changed |

## Expected Final State
Tyrell follow-up is back in Calls; it is no longer in the trash.

## Notes
- `note` adds to the call being recorded (the watcher's), so without the app it refuses; adding a note to a recording is `window/TC-014` (test call).
- Deleting and restoring from the window is `calls/TC-035`, `calls/TC-036`.

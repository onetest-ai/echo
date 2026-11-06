---
id: TC-036
title: echoctl restore brings the trashed call back into the list
priority: medium
type: functional
requirements: [F-CALLS-13]
module: calls
run: agent
platform: any
data: standard
tags: [calls, delete, cli]
---

# TC-036: echoctl restore brings the trashed call back into the list

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- The data has one call in the trash: `2026-11-03_170000-acme-draft-call` ("Acme draft call"), listed by `ls "$ECHOCTL_HOME/.echo/trash"`

## Test Data

| Field | Value |
|---|---|
| Call in the trash | Acme draft call (`2026-11-03_170000-acme-draft-call`) |
| Command | `echoctl restore 2026-11-03_170000-acme-draft-call` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync" and type "draft" into `[aria-label="Search calls"]` | The list says "Nothing matches "draft"." |
| 2 | Run `ls "$ECHOCTL_HOME/.echo/trash"` | It lists `2026-11-03_170000-acme-draft-call` |
| 3 | Run `echoctl restore 2026-11-03_170000-acme-draft-call` | Exit code 0 |
| 4 | Run `ls "$ECHOCTL_HOME/calls"` and `ls "$ECHOCTL_HOME/.echo/trash"` | `calls` has the folder; the trash no longer has it |
| 5 | Reload `{{base_url}}/#calls` and type "draft" into the search box | The row "Acme draft call" is listed |

## Expected Final State
Acme draft call is back in the list.

## Teardown
- Run `echoctl delete 2026-11-03_170000-acme-draft-call` to put it back in the trash

## Notes
- Moves a folder inside the run's data folder.

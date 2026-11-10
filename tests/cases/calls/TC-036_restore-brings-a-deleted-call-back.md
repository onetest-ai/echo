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
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-036: echoctl restore brings the trashed call back into the list

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- The data has one call in the trash, "Acme draft call", listed by `ls "$ECHOCTL_HOME/.echo/trash"`. Its folder is `<D>_170000-acme-draft-call` with D three days before the build date (built 2026-11-08: `2026-11-05_170000-acme-draft-call`); below, `<folder>` is that name as step 2 lists it

## Test Data

| Field | Value |
|---|---|
| Call in the trash | Acme draft call (`<folder>`: `<D>_170000-acme-draft-call`, D = the build date − 3 days) |
| Command | `echoctl restore <folder>` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync" and type "draft" into `[aria-label="Search calls"]` | The list says "Nothing matches "draft"." |
| 2 | Run `ls "$ECHOCTL_HOME/.echo/trash"` | It lists one folder, `<D>_170000-acme-draft-call` with D three days before the build date: that is `<folder>` |
| 3 | Run `echoctl restore <folder>` | Exit code 0 |
| 4 | Run `ls "$ECHOCTL_HOME/calls"` and `ls "$ECHOCTL_HOME/.echo/trash"` | `calls` has the folder; the trash no longer has it |
| 5 | Reload `{{base_url}}/#calls` and type "draft" into the search box | The row "Acme draft call" is listed |

## Expected Final State
Acme draft call is back in the list.

## Teardown
- Run `echoctl delete <folder>` to put it back in the trash

## Notes
- Moves a folder inside the run's data folder.

---
id: TC-035
title: Delete asks first; Keep cancels; Delete takes the call out of the list and into the trash folder
priority: high
type: functional
requirements: [F-CALLS-12]
module: calls
run: agent
platform: any
data: standard
tags: [calls, delete]
---

# TC-035: Delete asks first; Keep cancels; Delete takes the call out of the list and into the trash folder

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Field | Value |
|---|---|
| Call | Guest call 6 (`guest-call-6`), minutes, 2 people |
| Trash | `$ECHOCTL_HOME/.echo/trash/` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Guest call 6, then "Call actions" › "Delete…" | A bar reads "Delete "Guest call 6"? It goes to the trash in the data folder for 30 days." with the buttons "Keep" and "Delete" |
| 2 | Click "Keep" | The bar is gone; the call is still open and its row is in the list |
| 3 | Run `ls "$ECHOCTL_HOME/.echo/trash"` | The output has no folder ending `-guest-call-6` |
| 4 | Click "Call actions" › "Delete…" and then "Delete" | The row Guest call 6 is gone from the list |
| 5 | Run `ls "$ECHOCTL_HOME/.echo/trash"` and `ls "$ECHOCTL_HOME/calls"` | The trash holds the folder ending `-guest-call-6` (with the already trashed `…-acme-draft-call`); `calls` does not |

## Expected Final State
Guest call 6 is in the trash and not in the list.

## Teardown
- Run `echoctl restore <the folder name of Guest call 6>` and reload `{{base_url}}/#calls`: the row is back

## Notes
- Moves a folder inside the run's data folder; recoverable (`TC-036`).

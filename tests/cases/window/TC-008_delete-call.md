---
id: TC-008
title: Deleting a call asks first, then moves it to the trash
priority: high
type: functional
module: window
run: agent
platform: any
tags: [calls, delete]
external_id: TC-WIN-006
---

# TC-008: Deleting a call asks first, then moves it to the trash

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Initech planning |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Initech planning" | The call is open |
| 2 | Open the call's "…" menu and choose "Delete…" | A line asks "Delete "Initech planning"? It goes to the trash in the data folder for 30 days." with "Keep" and "Delete" |
| 3 | Click "Keep" | The question goes; the call is still listed |
| 4 | Choose "Delete…" again and click "Delete" | "Initech planning" is no longer in the Calls list |
| 5 | Run `target/debug/echoctl restore <its folder>` (the folder name is in `ls "$ECHOCTL_HOME/.echo/trash"`) | Exit code 0; after a reload the call is listed again |

## Expected Final State
Delete asks first; a confirmed delete moves the call to `.echo/trash`, from where `echoctl restore` brings it back.

---
id: TC-006
title: A column's menu renames it, sets a limit, hides or deletes it; + Column adds one; "N hidden" shows it again
priority: low
type: functional
requirements: [F-WORK-06]
module: work
run: agent
platform: any
tags: [work, board, columns]
---

# TC-006: A column's menu renames it, sets a limit, hides or deletes it; + Column adds one; "N hidden" shows it again

**Module:** Work | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |
| New column | Review, limit 1 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work` | Five columns; "To do" shows 1 (the card T001) |
| 2 | Click "+ Column" | A sixth column "New column" appears and its menu opens with the input "Column name", colours, "Cards in it are" select, "Limit" input, "Move left", "Hide", "Delete column" |
| 3 | Set the name to `Review`, the select "Cards in it are" to `to do`, the "Limit" to `1` (Enter after each) | The header reads "Review" with "0 / 1" |
| 4 | Open the menu of "Done" and click "Hide" | The Done column is gone; a button "1 hidden" is shown |
| 5 | Click "1 hidden" | The column "Done" is back |
| 6 | Open the menu of "Review" and click "Delete column" | "Review" is gone; five columns remain |
| 7 | Open the menu of "Done" and click "Delete column" | The notice "Keep a column for “Done” — its cards need somewhere to go." is shown and the column stays |

## Expected Final State
Globex pilot has the five default columns.

## Teardown
- Delete "Review" (step 6); unhide any hidden column

## Changes
- Adds and removes a column in `globex-pilot`'s board settings.

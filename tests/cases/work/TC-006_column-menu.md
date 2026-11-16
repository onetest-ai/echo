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
auto: [tests/auto/specs/S-06.spec.mjs]
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
| 3 | Set the name to `Review` and press Enter (Enter in the name or the limit closes the menu); open the menu again ("Column options" on "Review"), set the select "Cards in it are" (`[data-col-state]`) to `to do` with `browser_select_option` (the menu stays open), type `1` in "Limit" and press Enter | The header reads "Review" with "0 / 1"; the menu is closed |
| 4 | Open the menu of "Done". With `browser_evaluate` run `() => { const m = document.querySelector('.bd-col-h .bd-menu'); const on = (t) => { const e = [...m.querySelectorAll('.menu-item')].find((x) => x.textContent.trim() === t); const r = e.getBoundingClientRect(); return document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2) === e }; return { hide: on('Hide'), del: on('Delete column') } }`, then click "Hide" | The evaluate returns `hide` and `del` true (the whole menu shows over the short board, #109); after the click the Done column is gone and a button "1 hidden" is shown |
| 5 | Click "1 hidden" | The column "Done" is back |
| 6 | Open the menu of "Review" and click "Delete column" | "Review" is gone; five columns remain |
| 7 | Open the menu of "Done" and click "Delete column" | The notice "Keep a column for “Done” — its cards need somewhere to go." is shown and the column stays |

## Expected Final State
Globex pilot has the five default columns.

## Teardown
- Delete "Review" (step 6); unhide any hidden column

## Changes
- Adds and removes a column in `globex-pilot`'s board settings.

## Notes
- Globex pilot's board has one card, so it is short (about 140 px at 900 px window height): before #109 its column menus were cut off by the board's own box, and "Hide" and "Delete column" could not be clicked. Step 4 checks the items are the topmost element at their centres. Uses `evaluate` only to read geometry.

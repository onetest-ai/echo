---
id: TC-004
title: The board shows five columns and cards with id, story, owner, due (late in red)
priority: high
type: functional
requirements: [F-WORK-04]
module: work
run: agent
platform: any
tags: [work, board]
---

# TC-004: The board shows five columns and cards with id, story, owner, due (late in red)

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Give T002 a past due date: `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T002","set":{"due":"2026-01-05"}}'`

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` (Board layout) | Column headers in order: "To do" 2, "In progress" 0, "Waiting" 0, "Blocked" 0, "Done" 0, then "+ Column" |
| 2 | Read the card T001 | `T001 · SSO for the Berlin pilot`, the avatar "SR" (title "Sam Rivera"), the title "Send Acme the SSO checklist", a due chip with today's date (title "Due") |
| 3 | Read the card T002 | `T002 · SSO for the Berlin pilot`, the avatar "OH", the title "Answer the security questionnaire", a due chip "Jan 5" with the title "Overdue" (class `late`) |
| 4 | Click "Fields ▾" | A menu of checkboxes: Story, Kind, Owner, Due date, Waiting on, Holds up, Last call; all but "Last call" are checked |
| 5 | Uncheck "Due date" | Neither card shows a due chip |
| 6 | Check "Due date" again | The due chips are back |
| 7 | Press Escape | The Fields menu (`.bd-menu`) closes, as "Call actions" and "Thread actions" close on Escape |
| 8 | If the menu is still open, click an empty part of the board header | The menu closes |

## Expected Final State
The board shows both cards with their due chips; T002's is overdue.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T002","set":{"due":null}}'`

## Changes
- Sets and clears the due date of T002 (its history keeps both).

## Notes
- Step 7 fails until the bug is filed and fixed: the board's own menus (Fields, a column's menu, the view menu) close only on a click outside them (`app.js`, the click handler that resets `S.pj.fieldsMenu`); the Escape handler closes the call menu and others but not these. Step 8 closes it so the stages after it start clean.

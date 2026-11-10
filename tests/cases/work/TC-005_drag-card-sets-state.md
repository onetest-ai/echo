---
id: TC-005
title: Dragging a card to another column sets its state as the user's edit, undoable; Waiting takes no drop
priority: high
type: functional
requirements: [F-WORK-05]
module: work
run: agent
platform: any
tags: [work, board, state]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-005: Dragging a card to another column sets its state as the user's edit, undoable; Waiting takes no drop

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Card | T002 Answer the security questionnaire |
| Column | In progress |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` | "To do" shows 2 and holds T001 and T002 |
| 2 | Drag the card T002 (`.bd-card[data-drag="T002"]`) onto the column "In progress" (`.bd-col[data-drop="doing"]`) | "To do" shows 1 and "In progress" shows 1 and holds T002 |
| 3 | Click the card T002 and read History in the Details panel | The newest entry reads "you set column, state" with a link "undo" |
| 4 | Run `echoctl project tasks` | T002 has `"state": "in_progress"` (in the `others` pile) |
| 5 | Click "undo" on that entry. Wait up to 3 s after an action for the window to redraw before reading. | T002 is back in "To do"; "In progress" shows 0 |
| 6 | Check the column "Waiting" | Its cell has an empty `data-drop` attribute (`.bd-col` of Waiting): it accepts no drop |

## Expected Final State
T002 is in To do again.

## Teardown
- If step 5 was not reached: click "undo" or choose Status `to do` in the Details panel

## Changes
- Sets the column and state of T002 and undoes it.

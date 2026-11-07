---
id: TC-012
title: Closing a task from Today asks first and keeps the comment
priority: high
type: functional
requirements: [F-TASK-13]
module: tasks
run: agent
platform: any
tags: [tasks, today, close]
---

# TC-012: Closing a task from Today asks first and keeps the comment

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The task is open: "Send Acme the SSO checklist" shows in Today's "My work · this week"

## Test Data

| Field | Value |
|-------|-------|
| Task | Send Acme the SSO checklist (Acme rollout, T001) |
| Comment | Sent the checklist by email |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for "My work" | The card "Send Acme the SSO checklist" is listed |
| 2 | Click its close button (`[data-today-close="T001"][data-project="acme-rollout"]`, label "Close “Send Acme the SSO checklist”…") | A dialog "Close this task?" with the task title, a field labelled "A comment, if you like — why it's done, or where", and the buttons "Cancel" and "Close it" |
| 3 | Click "Cancel" | The dialog closes; the card is still in My work |
| 4 | Open the dialog again, type "Sent the checklist by email" in the comment and click "Close it" | The dialog closes; the card "Send Acme the SSO checklist" is gone from My work |
| 5 | Run `echoctl project ledger acme-rollout tree` | In `nodes.T001`: `state` is `done`; its last `history` entry has `op: edit`, `set.state: done` and `note: "Sent the checklist by email"`; note its `mid` |
| 6 | Navigate to `{{base_url}}/#tasks` | "I owe 1" with only "Draft the Globex pilot plan" |

## Expected Final State
The task is done with the user's comment kept in its history; it is out of Today and Tasks.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"undo","target":"<the mid of step 5>"}'` reopens the task

## Notes
- Changes Acme rollout T001 (undone in Teardown).
- Suspected gap, not asserted: the Details History row for the close shows "you set state" but not the comment (`workPanel` renders only `h.quote`, not `h.note`), so the comment is checked through the CLI.

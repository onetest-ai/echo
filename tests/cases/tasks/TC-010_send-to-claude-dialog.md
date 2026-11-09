---
id: TC-010
title: Send to Claude opens the dialog with the task and a skill; Escape, Cancel and leaving the view close it
priority: high
type: functional
requirements: [F-TASK-10]
module: tasks
run: agent
platform: any
tags: [tasks, claude]
---

# TC-010: Send to Claude opens the dialog with the task and a skill; Escape, Cancel and leaving the view close it

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- A task whose words fit the one skill ("follow-up-mail" on the data): `echoctl project ledger acme-rollout edit '{"op":"create","kind":"task","parent":"S001","title":"Write the follow-up mail to Acme","internal_owner":"Sam Rivera"}'` (it prints the new id, T003). Every `echoctl` command runs after `. <dir>/env` (the run's env file)

## Test Data

| Field | Value |
|-------|-------|
| Task | Write the follow-up mail to Acme |
| Skill | follow-up-mail |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe" lists "Write the follow-up mail to Acme" |
| 2 | Click that card, then the Details' button "Send to" with the Claude mark (its visible text is "Send to"; its accessible name is "Send to Claude": `getByRole('button', {name: 'Send to Claude'})` or `[aria-label="Send to Claude"]`) | A dialog (`role=dialog`) with the heading "Claude", the thread "Acme rollout" and the task title; buttons "Work on it" and "Cancel"; no "Yolo" button (it is from Today only) |
| 3 | Read the skill field | A skill is already chosen: its description starts "Use when a task is to write a follow-up mail"; a cross with the label "No skill — just the task" |
| 4 | Click the cross | The field is now an input with the placeholder "Just the task — or type to find a skill (1)" |
| 5 | Read the list under the input (the cross's click opened it; click the input if it is closed) | The list shows the skill with the mark "fits best" |
| 6 | Press Escape, read the dialog, click the skill field, press Escape, then press Escape again | After the first Escape: the dialog is still open, the list folded (no "fits best"); the click opens the list again; the next Escape folds it, and the last one closes the dialog: no dialog; nothing was sent (the panel "Claude" at the bottom is not opened) (#108) |
| 7 | Click the "Send to" button (`[aria-label="Send to Claude"]`) again and click "Cancel" | The dialog closes |
| 8 | Click the "Send to" button (`[aria-label="Send to Claude"]`) again, then navigate to `{{base_url}}/#today` | Today shows with no dialog (no `role=dialog`) over it (#108) |

## Expected Final State
The dialog was opened three times and closed by Escape, Cancel and leaving the view; no Claude session was started.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T003","set":{"state":"cancelled"}}'` takes the task out of the piles
- Close the panel

## Notes
- Never press "Work on it" or "Yolo": they start Claude Code. Creates task T003 on Acme rollout (cancelled in Teardown).

---
id: TC-010
title: Send to Claude opens the dialog with the task and a skill, and Cancel closes it
priority: high
type: functional
requirements: [F-TASK-10]
module: tasks
run: agent
platform: any
tags: [tasks, claude]
---

# TC-010: Send to Claude opens the dialog with the task and a skill, and Cancel closes it

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
| 2 | Click that card, then "Send to Claude" (`[aria-label="Send to Claude"]`) in the Details | A dialog (`role=dialog`) with the heading "Claude", the thread "Acme rollout" and the task title; buttons "Work on it" and "Cancel"; no "Yolo" button (it is from Today only) |
| 3 | Read the skill field | A skill is already chosen: its description starts "Use when a task is to write a follow-up mail"; a cross with the label "No skill — just the task" |
| 4 | Click the cross | The field is now an input with the placeholder "Just the task — or type to find a skill (1)" |
| 5 | Click the input | A list opens with the skill and the mark "fits best" |
| 6 | Press Escape twice | The list folds, then the dialog closes; nothing was sent (the panel "Claude" at the bottom is not opened) |
| 7 | Click "Send to Claude" again and click "Cancel" | The dialog closes |

## Expected Final State
The dialog was opened and cancelled twice; no Claude session was started and the Details are still open.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"edit","node":"T003","set":{"state":"cancelled"}}'` takes the task out of the piles
- Close the panel

## Notes
- Never press "Work on it" or "Yolo": they start Claude Code. Creates task T003 on Acme rollout (cancelled in Teardown).

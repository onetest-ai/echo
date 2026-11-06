---
id: TC-003
title: A task card in My work opens the task's details on the right
priority: high
type: functional
module: today
run: agent
platform: any
tags: [today, tasks]
---

# TC-003: A task card in My work opens the task's details on the right

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Task | Send Acme the SSO checklist |
| Thread › story | Acme rollout › SSO for the Berlin pilot |
| Owner | Sam Rivera |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` | "My work · this week" shows the card "Send Acme the SSO checklist" |
| 2 | Click the card on its empty area (not on its title text) | A Details panel opens on the right; the URL stays `#today` |
| 3 | Read the panel | Title "Send Acme the SSO checklist", path "Acme rollout › SSO for the Berlin pilot", Owner "Sam Rivera", Status "to do" |
| 4 | Check the panel's head | It has "Send to" (with the Claude mark) and "Edit" |

## Expected Final State
The task's details are open in the right panel over Today, with its title, thread and story, owner and status; the view is still Today.

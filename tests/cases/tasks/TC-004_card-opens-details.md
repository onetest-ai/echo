---
id: TC-004
title: A task card opens its Details on the right and Tasks stays
priority: high
type: functional
requirements: [F-TASK-07]
module: tasks
run: agent
platform: any
tags: [tasks, details]
auto: [tests/auto/specs/S-08.spec.mjs]
---

# TC-004: A task card opens its Details on the right and Tasks stays

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- No change has been made to the tasks on the data

## Test Data

| Field | Value |
|-------|-------|
| Task | Draft the Globex pilot plan (Globex pilot; its id T001 is also Acme rollout's) |
| Thread › story | Globex pilot › Pilot plan |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe 2" |
| 2 | Click the card "Draft the Globex pilot plan" (`[data-tk="open"][data-project="globex-pilot"][data-node="T001"]`) | The URL stays `#tasks`; the right panel opens on the tab "Details"; the card is highlighted |
| 3 | Read the Details | Title "Draft the Globex pilot plan", the path "Globex pilot › Pilot plan" (not Acme rollout), Owner "Sam Rivera", Due "no date", Status "to do" |
| 4 | Click the card "Send Acme the SSO checklist" (`[data-project="acme-rollout"][data-node="T001"]`) | The Details change to "Send Acme the SSO checklist" with the path "Acme rollout › SSO for the Berlin pilot" and Owner "Sam Rivera" |
| 5 | Look at the head of the Details | The buttons "Send to Claude" and "Edit"; a row "Link" with "Copy link" |

## Expected Final State
The Details of the last clicked task are open on the right; the view is still Tasks.

## Teardown
- Close the panel (`[aria-label="Close the panel"]`)

## Notes
- Two threads both have a task T001: the card must pair the thread with the id.

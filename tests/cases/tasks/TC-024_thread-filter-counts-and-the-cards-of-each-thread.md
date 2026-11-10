---
id: TC-024
title: The Thread filter counts each open thread's tasks, offers no closed thread and narrows every pile
priority: medium
type: functional
requirements: [F-TASK-04, F-TASK-01]
module: tasks
run: agent
platform: any
tags: [tasks, filters, thread]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-024: The Thread filter counts each open thread's tasks, offers no closed thread and narrows every pile

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

| Option of the Thread filter | I owe | Owed to me | Waiting for others | Nobody named | Others | Cards in all |
|---|---|---|---|---|---|---|
| Acme renewal · 11 | 3 | 1 | 2 | 1 | 5 | 12 |
| Acme rollout · 17 | 3 | 0 | 0 | 3 | 11 | 17 |
| Globex pilot · 10 | 2 | 2 | 1 | 3 | 2 | 10 |
| Initech hiring · 9 | 2 | 1 | 1 | 2 | 3 | 9 |
| Umbrella lead · 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| Umbrella proposal · 10 | 4 | 1 | 1 | 2 | 3 | 11 |
| Internal tooling · 0 | 0 | 0 | 0 | 0 | 0 | 0 |

Not in the filter: Tyrell bid, Legacy migration and Acme hackathon (closed threads).

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks`, wait for "I owe" and read the options of `[aria-label="Thread"]` | "All threads", then in this order "Acme renewal · 11", "Acme rollout · 17", "Globex pilot · 10", "Initech hiring · 9", "Umbrella lead · 0", "Umbrella proposal · 10", "Internal tooling · 0"; no closed thread is offered |
| 2 | Select "Acme renewal · 11" | The chip reads "Acme renewal ▾"; the piles hold 3, 1, 2, 1 and 5 cards (Others · 5): 12 cards, one more than the 11 in the option, because the person waited on (Rita Gomez) gets a card that is not a task |
| 3 | Select "Globex pilot · 10" | The piles hold 2, 2, 1, 3 and 2 cards (Others · 2): 10 cards, the same as the option |
| 4 | Select "Umbrella lead · 0" | Every pile heading shows 0 and "Nothing here"; the button "Others" is gone; the page does not show "No open tasks" (other threads have tasks) |
| 5 | Select "Internal tooling · 0" | The same: four empty piles |
| 6 | Select "All threads" | The five piles are back with 14, 5, 5, 11 and 24 cards |

## Expected Final State
The filter is on All threads.

## Teardown
- None: the case only reads

## Notes
- Extends `TC-003` (two threads) to seven; threads with no task are still listed with 0.
- The count of an option is the number of open tasks; the wait cards of Waiting for others are not tasks, so Acme renewal shows 12 cards for 11 tasks.

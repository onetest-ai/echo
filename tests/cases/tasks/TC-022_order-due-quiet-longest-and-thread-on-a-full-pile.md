---
id: TC-022
title: Order puts late first then nearest date then the quietest, or the quietest, or by thread, on 14 cards
priority: medium
type: functional
requirements: [F-TASK-03, F-TASK-05]
module: tasks
run: agent
platform: any
tags: [tasks, order, quiet, due]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-022: Order puts late first then nearest date then the quietest, or the quietest, or by thread, on 14 cards

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

**I owe, 14 cards, with their due and quiet days**

| Thread | Id | Title | Due | Quiet |
|---|---|---|---|---|
| Acme renewal | T003 | Send the revised timeline | today | 3 |
| Acme renewal | T017 | Send the revised timeline (2) | no date | 3 |
| Acme renewal | T019 | Share the architecture diagram (2) | no date | 3 |
| Acme rollout | T010 | Book the workshop room | 3 days ago (late) | 0 |
| Acme rollout | T016 | Map the user groups | 3 days ago (late) | 0 |
| Acme rollout | T023 | Send the meeting notes (2) | no date | 0 |
| Globex pilot | T001 | Collect the user list | tomorrow | 0 |
| Globex pilot | T012 | Send the welcome mail (2) | no date | 0 |
| Initech hiring | T010 | Prepare the handover pack | no date | 4 |
| Initech hiring | T012 | Agree the support hours (2) | no date | 4 |
| Umbrella proposal | T005 | Prepare the demo script | no date | 5 |
| Umbrella proposal | T009 | Close the open tickets | in 20 days | 5 |
| Umbrella proposal | T012 | Draft the status report (2) | no date | 5 |
| Umbrella proposal | T013 | Prepare the demo script (2) | no date | 5 |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks`, wait for "I owe" and click "+ 8" in "I owe" | The Order chip reads "Due ▾"; 14 cards |
| 2 | Read the titles of the "I owe" cards from the top (Order "Due") | First the two late ones (Acme rollout T010 "Book the workshop room" and T016 "Map the user groups", in either order), then the one due today (Acme renewal T003), tomorrow (Globex pilot T001) and in 20 days (Umbrella proposal T009); then the 9 undated cards from the quietest: Umbrella proposal T005, T012, T013 (5 days, any order), Initech hiring T010, T012 (4 days), Acme renewal T017, T019 (3 days), then Acme rollout T023 and Globex pilot T012 (0 days) |
| 3 | Select "Quiet longest" in `[aria-label="Order"]` | The chip reads "Quiet longest ▾"; the column runs in groups by days of quiet, never rising: the four Umbrella proposal cards (5), Initech hiring T010 and T012 (4), Acme renewal T003, T017, T019 (3), then Acme rollout T010, T016, T023 and Globex pilot T001, T012 (0) |
| 4 | Select "Thread" | The chip reads "Thread ▾"; the cards are grouped by thread name from A to Z: Acme renewal (3), Acme rollout (3), Globex pilot (2), Initech hiring (2), Umbrella proposal (4) |
| 5 | Select "Due" again | The order of the second step is back |

## Expected Final State
Order is "Due" and the pile is in the order of the second step.

## Teardown
- None: the case only reads

## Notes
- Cards that tie (same due and same quiet days) may swap places: the steps name the groups, not the places inside a group.
- Extends `TC-006` (order on 2 cards) to a pile that folds.

---
id: TC-023
title: Who does it keeps only the user's tasks Claude can do, or the ones by hand, and narrows with the Thread filter
priority: medium
type: functional
requirements: [F-TASK-04, F-TASK-06]
module: tasks
run: agent
platform: any
tags: [tasks, filters, mode, pairwise]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-023: Who does it keeps only the user's tasks Claude can do, or the ones by hand, and narrows with the Thread filter

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task has had its mode changed since the data was built

## Test Data

| Thread | Id | Title | Mode icon (aria-label) | Set by |
|---|---|---|---|---|
| Acme rollout | T016 | Map the user groups | You do it by hand | you |
| Acme renewal | T003 | Send the revised timeline | Claude can do it | you |
| Globex pilot | T001 | Collect the user list | You do it by hand | you |
| Umbrella proposal | T009 | Close the open tickets | Claude can do it | you |
| Umbrella proposal | T005 | Prepare the demo script | Claude can do it | you |
| Initech hiring | T010 | Prepare the handover pack | You do it by hand | you |

The other 8 cards of I owe have no mode (no icon); a card outside I owe never shows one.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks`, wait for "I owe", click "+ 8" in "I owe" | The chip "Who does it" reads "Anyone ▾"; 14 cards |
| 2 | Count the `.md-ico` icons of the column and read their `aria-label` | 6 icons: 3 "Claude can do it" (Claude's mark) and 3 whose label starts with "You do it by hand" (a hand; it goes on with how, e.g. "You do it by hand: A call on Monday morning"), those of the table; none is faded (class `guess`), because every mode is set by the user |
| 3 | Select "Claude can do" in `[aria-label="Who does it"]` | The chip reads "Claude can do ▾"; "I owe 3" with Acme renewal T003, Umbrella proposal T005 and T009; "Owed to me 0", "Waiting for others 0", "Nobody named 0"; the button "Others" is gone |
| 4 | Select "By hand" | "I owe 3" with Acme rollout T016, Globex pilot T001 and Initech hiring T010; the other piles show "Nothing here" |
| 5 | Select "Claude can do", then select the thread "Umbrella proposal · 10" in `[aria-label="Thread"]` | "I owe 2" with Umbrella proposal T005 and T009: the two filters narrow together |
| 6 | Select "Anyone" and "All threads" | "I owe 14", "Owed to me 5", "Waiting for others 5", "Nobody named 18" and "Others · 24" again |

## Expected Final State
Both filters are back on Anyone and All threads.

## Teardown
- None: the case only reads

## Notes
- Extends `TC-009`, which starts from a board with no mode set. Here three tasks are "Claude can" and three "By hand" from the start.
- Echo's guessed mode (a faded icon) cannot be seen on test data: the only guess is on a done task (Acme rollout T009).

---
id: TC-021
title: A card says "quiet N d" only when it has no date and nobody said a word about it for five days
priority: medium
type: functional
requirements: [F-TASK-06, F-TASK-03]
module: tasks
run: agent
platform: any
tags: [tasks, cards, quiet]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-021: A card says "quiet N d" only when it has no date and nobody said a word about it for five days

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

| Thread | Id | Title | Pile | Last word | Due | Quiet mark |
|---|---|---|---|---|---|---|
| Umbrella proposal | T005 | Prepare the demo script | I owe | 5 days | no date | quiet 5 d |
| Umbrella proposal | T012 | Draft the status report (2) | I owe | 5 days | no date | quiet 5 d |
| Umbrella proposal | T013 | Prepare the demo script (2) | I owe | 5 days | no date | quiet 5 d |
| Umbrella proposal | T004 | Draft the status report (kept) | Nobody named | 5 days | no date | quiet 5 d |
| Umbrella proposal | T008 | Archive the old tenant (kept) | Nobody named | 5 days | no date | quiet 5 d |
| Umbrella proposal | T002 | Unblock: review the test plan | Others | 5 days | no date | quiet 5 d |
| Umbrella proposal | T011 | After: help desk readiness | Others | 5 days | no date | quiet 5 d |
| Umbrella proposal | T014 | After: prepare the demo script (2) | Others | 5 days | no date | quiet 5 d |
| Umbrella proposal | T009 | Close the open tickets | I owe | 5 days | in 20 days | none |
| Umbrella proposal | T010 | Check the licence count | Owed to me | 5 days | in 20 days | none |
| Initech hiring | T010 | Prepare the handover pack | I owe | 4 days | no date | none |
| Initech hiring | T012 | Agree the support hours (2) | I owe | 4 days | no date | none |
| Acme renewal | T017 | Send the revised timeline (2) | I owe | 3 days | no date | none |
| Acme rollout | T023 | Send the meeting notes (2) | I owe | 0 days | no date | none |

The wait card of Umbrella proposal T012 ("an answer on draft the status report (2)", Waiting for others) is undated and 5 days quiet: it carries "quiet 5 d" too.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks`, wait for "I owe", unfold every pile ("+ 8", "+ 12", "Others · 24") | All piles are unfolded |
| 2 | For each row of the table find the card and read the mark `.tk-quiet` (text and `title`) | The mark matches the last column; its tooltip reads "No word about it in 5 days" where there is one |
| 3 | Count the `.tk-quiet` marks on the page | 9: the eight undated Umbrella proposal cards of the table and the wait card of Umbrella proposal T012; no card of another thread has one (their last word is 4, 3 or 0 days old) |

## Expected Final State
9 quiet marks, all on Umbrella proposal cards.

## Teardown
- None: the case only reads

## Notes
- Umbrella proposal's last processed call is five days old, so all its items are 5 days quiet; Initech hiring is 4, Acme renewal 3, the others 0.

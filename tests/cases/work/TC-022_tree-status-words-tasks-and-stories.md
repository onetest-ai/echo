---
id: TC-022
title: The tree words a task or story by state, type and wait: waiting, held, blocked, to do, and shows no status on a decision
priority: high
type: functional
requirements: [F-WORK-07]
module: work
run: agent
platform: any
tags: [work, tree, status, pairwise]
---

# TC-022: The tree words a task or story by state, type and wait: waiting, held, blocked, to do, and shows no status on a decision

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- The Tree view of each thread has no kind or owner filter

## Test Data

| Thread | Thread id | Id | Kind | Type tag | State | Waiting for | Word | Class `wk-st …` |
|---|---|---|---|---|---|---|---|---|
| Umbrella proposal | umbrella-proposal | T001 | task | - | done | - | done | done |
| Umbrella proposal | umbrella-proposal | T009 | task | - | open | - | to do | open |
| Umbrella proposal | umbrella-proposal | T013 | task | session | in_progress | - | in progress | in_progress |
| Umbrella proposal | umbrella-proposal | T010 | task | - | blocked | - | blocked | blocked |
| Umbrella proposal | umbrella-proposal | T012 | task | dependency | open | Zoe Martin | waiting | waiting |
| Umbrella proposal | umbrella-proposal | T015 | task | session | done | - | held | done |
| Umbrella proposal | umbrella-proposal | T006 | task | - | cancelled | - | cancelled | cancelled |
| Umbrella proposal | umbrella-proposal | S002 | story | - | in_progress | Zoe Martin | waiting | waiting |
| Umbrella proposal | umbrella-proposal | S003 | story | - | merged | - | not shown | - |
| Globex pilot | globex-pilot | T004 | task | - | blocked | Kim Park | waiting | waiting |
| Globex pilot | globex-pilot | T005 | task | - | open | Kim Park | waiting | waiting |
| Globex pilot | globex-pilot | T008 | task | - | done | Kim Park | done | done |
| Globex pilot | globex-pilot | S002 | story | - | cancelled | - | cancelled | cancelled |
| Globex pilot | globex-pilot | S003 | story | - | open | - | to do | open |
| Acme renewal | acme-renewal | T003 | task | - | in_progress | Rita Gomez | waiting | waiting |
| Acme renewal | acme-renewal | T017 | task | session | blocked | - | blocked | blocked |
| Acme renewal | acme-renewal | T001 | task | - | done | - | done | done |
| Acme renewal | acme-renewal | T019 | task | session | open | - | to do | open |
| Acme renewal | acme-renewal | T020 | task | dependency | cancelled | - | cancelled | cancelled |
| Acme renewal | acme-renewal | S003 | story | - | deleted | - | not shown | - |
| Acme renewal | acme-renewal | D001 | decision | - | open | - | no status | - |
| Acme renewal | acme-renewal | D002 | decision | - | done | - | no status | - |
| Acme rollout | acme-rollout | S004 | story | - | done | - | done | done |
| Initech hiring | initech-hiring | S002 | story | - | blocked | - | blocked | blocked |
| Initech hiring | initech-hiring | D002 | decision | - | in_progress | - | no status | - |
| Initech hiring | initech-hiring | D001 | decision | - | deleted | - | not shown | - |

Workstreams (epics) with nothing under them:

| Thread | Id | Title | State |
|---|---|---|---|
| Acme renewal | E02 | Phase two | in progress |
| Acme renewal | E03 | Reporting | done |
| Globex pilot | E02 | Hypercare | open |
| Globex pilot | E03 | Onboarding | cancelled |
| Umbrella proposal | E02 | Integrations | deleted |
| Acme rollout | E02 | Security hardening | blocked |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | For each thread id of the table navigate to `{{base_url}}/#project/<thread id>/work`, click the view "Tree" and turn "Open only" off if it is on | The tree lists closed items too |
| 2 | For each row of the table find the tree row of the item and read its tag (`.wk-k`) and its status (`.wk-st`) | The tag is the Type tag of the row (none for "-") and the status is "● " + the Word with the class of the last column; a decision row has no status; a row whose Word is "not shown" does not exist |
| 3 | Read Globex pilot T004 "Check the firewall rules" (state blocked, waiting for Kim Park) | The tree reads "● waiting" (amber), whereas the board puts the same card in "Blocked" (`TC-018`) |
| 4 | Read Umbrella proposal T010 "Check the licence count" (blocked, holds up S001, no person waited for) | It reads "● blocked" (red) and carries the tag "holds up 1" |
| 5 | Look for the workstream headers of the second table in the trees of the four threads | None is shown: a workstream with nothing under it has no header; each tree has the header "General · workstream" |

## Expected Final State
Every item of the table shows the word of its row.

## Teardown
- In each thread turn "Open only" on again (the button) and click the view "Board"

## Changes
- Turns "Open only" off and on again in five threads (a saved view setting).

## Notes
- A task or story that is waiting for a person reads "waiting" whatever its open state, blocked included (`workStatus`); a done session reads "held"; waiting for an item changes no word.
- The tree and the board disagree on a blocked task that waits for a person: this case records the tree's word and `TC-018` the board's column. Verify at run time whether one should follow the other.

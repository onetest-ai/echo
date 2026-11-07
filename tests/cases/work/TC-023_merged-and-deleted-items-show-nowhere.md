---
id: TC-023
title: Merged and deleted items are not on the tree, the board or in the kind counts
priority: medium
type: functional
requirements: [F-WORK-02, F-WORK-07]
module: work
run: agent
platform: any
tags: [work, tree, state, filters]
---

# TC-023: Merged and deleted items are not on the tree, the board or in the kind counts

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Acme rollout has no filter set

## Test Data

| Id | Kind | Title | State |
|---|---|---|---|
| T020 | task | Agree the acceptance criteria | merged |
| T013 | task | Order the licences | deleted |
| T025 | task | Agree the acceptance criteria (2) | deleted |
| R002 | risk | Training too late | deleted |

Kind counts of Acme rollout: all 32, stories 4, tasks 22, decisions 2, risks 1, questions 3.
Owners: Alex Chen, Dana Lee, Me, Omar Haddad, Rita Gomez, Sam Rivera, Tom Becker.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click the view "Tree" and turn "Open only" off | The tree lists closed items too |
| 2 | Read the options of `[aria-label="Kind"]` | "All kinds · 32", "Stories · 4", "Tasks · 22", "Decisions · 2", "Risks · 1", "Questions · 3": merged and deleted items and the workstreams are not counted |
| 3 | For each row of the table look for `[data-pj="node"][data-node=<Id>]` in the tree | None of the four items is shown |
| 4 | Click the view "Board" and look for the same four items | None is on the board |
| 5 | Read the options of `[aria-label="Owner"]` | "Anyone", "Nobody named", then "Alex Chen", "Dana Lee", "Me", "Omar Haddad", "Rita Gomez", "Sam Rivera", "Tom Becker" |

## Expected Final State
Acme rollout is on its board again.

## Teardown
- Turn "Open only" on again in the Tree view and click the view "Board"

## Changes
- Turns "Open only" off and on again (a saved view setting).

## Notes
- The Owner filter lists both "Me" and "Sam Rivera" although they are the same person (Tasks counts both as the user); a filter on "Sam Rivera" leaves out Acme rollout T001 "Send the SSO checklist", which is owned by "Me". Verify at run time.

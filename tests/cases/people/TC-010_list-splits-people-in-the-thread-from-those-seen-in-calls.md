---
id: TC-010
title: The list shows the people placed in a thread apart from those only seen in its calls, who can be added
priority: medium
type: functional
requirements: [F-PPL-04, F-PPL-01]
module: people
run: agent
platform: any
tags: [people, list, seen, pairwise]
---

# TC-010: The list shows the people placed in a thread apart from those only seen in its calls, who can be added

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No one has been added to a thread since the data was built

## Test Data

**In the thread**

| Person | Organisation column | Role | Stance | Calls | Tag |
|---|---|---|---|---|---|
| Kim Park | Globex | sponsor | supporter | 3/8 | new |
| Alex Chen | your company | - | - | 0/8 | - |
| Lena Vogel | your company | - | - | 0/8 | - |

**Seen in calls (not in the thread)**

| Person | Organisation column | Role | Stance | Calls | Button |
|---|---|---|---|---|---|
| Ben Okafor | Globex | - | - | 3/8 | Add |
| Noor Aziz | Globex | - | - | 3/8 | Add |
| Kai Larsen | Vandelay | - | - | 2/8 | Add |
| Dana Lee | Globex | - | - | 1/8 | Add |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/people` and press "List" in the segmented control | A table with the columns Person, Organisation, Role, Stance, Calls, Talk, Owes, Last seen |
| 2 | Read the rows of the first block (the card) | Kim Park, Alex Chen and Lena Vogel, with the Organisation, Role, Stance, Calls and tag of the first table |
| 3 | Read the block "Seen in calls" | The heading "Seen in calls", the text "4 aren't in this thread yet", the button "Add all" and a row each for Ben Okafor, Noor Aziz, Kai Larsen and Dana Lee, each with an "Add" button (not pressed) and the Organisation of the second table |
| 4 | Run `echoctl project people globex-pilot` | JSON in which the three first people have `in_thread` true and the four others false |
| 5 | Press "Map" again | The map is back (the choice is remembered) |

## Expected Final State
The People view is Map; nobody was added.

## Teardown
- Press "Map" in the segmented control if the list is still shown

## Changes
- Changes the remembered People view in the browser.

## Notes
- Dana Lee here is a different person from the Dana Lee of Acme (her address is at globex-labs.example): the list shows her organisation as Globex and the Acme panel offers to merge them (`TC-007`).
- Extends `TC-001`, which has two seen people and none placed.

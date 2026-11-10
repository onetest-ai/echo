---
id: TC-021
title: The tree words a question answered or open and a risk handled or with no response, by its text and state
priority: high
type: functional
requirements: [F-WORK-07]
module: work
run: agent
platform: any
tags: [work, tree, status, pairwise]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-021: The tree words a question answered or open and a risk handled or with no response, by its text and state

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- The Tree view of each thread has no kind or owner filter

## Test Data

| Thread | Thread id | Id | Kind | State | Text | Word | Class `wk-st …` |
|---|---|---|---|---|---|---|---|
| Acme rollout | acme-rollout | Q001 | question | done | has answer | answered | done |
| Acme rollout | acme-rollout | Q002 | question | done | no answer | answered | done |
| Acme rollout | acme-rollout | Q003 | question | open | has answer | answered | done |
| Acme rollout | acme-rollout | R001 | risk | open | no response | no response | waiting |
| Globex pilot | globex-pilot | Q001 | question | blocked | no answer | open | open |
| Globex pilot | globex-pilot | R001 | risk | in_progress | has response | handled | done |
| Initech hiring | initech-hiring | R001 | risk | blocked | has response | handled | done |
| Initech hiring | initech-hiring | R002 | risk | open | no response | no response | waiting |
| Initech hiring | initech-hiring | R003 | risk | cancelled | has response | cancelled | cancelled |
| Umbrella proposal | umbrella-proposal | Q001 | question | cancelled | no answer | cancelled | cancelled |
| Umbrella proposal | umbrella-proposal | R001 | risk | done | has response | done | done |
| Acme renewal | acme-renewal | Q001 | question | in_progress | no answer | open | open |

Colours by class: `done` green, `open` grey, `waiting` amber, `blocked` red, `in_progress` purple, `cancelled` and `merged` default grey.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/umbrella-proposal/work`, click the view "Tree" if it is not the active one, and turn "Open only" off if it is on (it is a button, `on` when pressed) | The tree lists closed items too (done ones in grey) |
| 2 | For each Umbrella proposal row of the table find the row `.wk-row` of the item (`[data-pj="node"][data-node=<Id>]` in the tree) and read its status `.wk-st` | The status reads "● " and the Word of the row, with the class of the last column |
| 3 | Do steps 1 and 2 in `globex-pilot`, then `initech-hiring`, then `acme-renewal`, then `acme-rollout` (each thread once, Acme rollout last) | The same for each thread's rows of the table |
| 4 | In Acme rollout (still open) look at the rows of the decisions D001 and D002 | A decision row has no status |

## Expected Final State
Every question and risk of the table shows the word of its row.

## Teardown
- In each thread turn "Open only" on again (the button) and click the view "Board"

## Changes
- Turns "Open only" off and on again in five threads (a saved view setting).

## Notes
- A question is "answered" as soon as it has an answer or is done; a risk is "handled" with a response and "no response" without (unless it is closed, then its state is the word).
- Acme rollout Q002 is done but has no answer text and still reads "answered"; Globex pilot Q001 is blocked and reads "open"; Initech hiring R001 is blocked and reads "handled": these follow `workStatus`.

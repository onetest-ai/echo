---
id: TC-024
title: The owner and kind filters of a board hold in every column, the Done column included
priority: high
type: functional
requirements: [F-WORK-02, F-WORK-04]
module: work
run: agent
platform: any
tags: [work, board, filters, done]
---

# TC-024: The owner and kind filters of a board hold in every column, the Done column included

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Acme rollout's view "Board" is active, no filter is set

## Test Data

| Filter | Expected cards |
|---|---|
| Owner "Omar Haddad" | T002 "Get the access list" (Backlog), T007 "Answer the security questionnaire" (In progress); nothing in Done |
| Kind "Tasks", Owner Anyone | Done holds T001, T009 and T022 only (tasks); Q001 and Q002 (questions) are not shown |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click the view "Board" | Headers "Backlog" 14 / 3, "In progress" 3 / 10, "In review" 0, "Blocked" 1, "Done" 5 |
| 2 | Select "Omar Haddad" in `[aria-label="Owner"]` | Backlog 1 / 3 holds T002; "In progress" 1 / 10 holds T007; "In review" 0, "Blocked" 0 and "Done" 0: every column follows the owner filter |
| 3 | Select "Anyone" in Owner and "Tasks" in `[aria-label="Kind"]` | No question is on the board: "Done" holds T001, T009 and T022 only (3) |
| 4 | Select "All kinds" in Kind | The headers of the first step are back |

## Expected Final State
No filter is set on the board.

## Teardown
- Select "Anyone" in Owner and "All kinds" in Kind

## Changes
- Sets and clears two filters of the view "Board" (a saved view setting).

## Notes
- Per the feature description every filter narrows the board. The code (`workBoard`) keeps `pass(n) || n.state === 'done'`, so done cards of other owners and kinds may stay in Done: this case is expected to FAIL at steps 2 and 3 until that is decided. Verify at run time.

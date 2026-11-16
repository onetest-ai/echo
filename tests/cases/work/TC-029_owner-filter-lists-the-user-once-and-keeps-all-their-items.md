---
id: TC-029
title: The Owner filter lists the user once, "Me (Sam Rivera)", and keeps the items of either name
priority: medium
type: functional
requirements: [F-WORK-02]
module: work
run: agent
platform: any
tags: [work, board, filters, owner]
auto: [tests/auto/specs/S-25.spec.mjs]
---

# TC-029: The Owner filter lists the user once, "Me (Sam Rivera)", and keeps the items of either name

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- The user is Sam Rivera (About me); Acme rollout's view "Board" is active, no filter is set, "Open only" is on

## Test Data

| Id | Owner as recorded | State |
|---|---|---|
| T001 | Me | done |
| T010 | Sam Rivera | open |
| T016 | Sam Rivera | open |
| T023 | Sam Rivera | open |
| T009, T022 | Sam Rivera | done |
| T019 | Sam Rivera | cancelled (on the tree with Open only off; on no board column) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click the view "Board" | The board is shown |
| 2 | Read the options of `[aria-label="Owner"]` | "Anyone", "Nobody named", "Me (Sam Rivera)", then the other owners by name; neither "Me" nor "Sam Rivera" is an option of its own |
| 3 | Select "Me (Sam Rivera)" in Owner | The chip reads "Me (Sam Rivera) ▾"; the board holds T010, T016 and T023 in the open columns and T001, T009 and T022 in "Done"; every task card's avatar is "SR" (Sam Rivera) or "M" (Me) |
| 4 | Click the view "Tree" if it is not the active one (clicking the active view opens its name box: Escape closes it), turn "Open only" off if it is on, and select "Me (Sam Rivera)" in Owner (filters are the view's own) | The tree shows T001, T009, T010, T016, T019, T022 and T023, with the stories above them; every task row's owner reads "Sam Rivera" or "Me" |

## Expected Final State
No filter is set on Acme rollout.

## Teardown
- In the Tree view select "Anyone" in Owner and turn "Open only" on again; in the Board view select "Anyone" in Owner

## Changes
- Sets and clears the Owner filter and "Open only" (saved view settings).

## Notes
- The user's names are "Me" (the transcripts' word) and their name, first name and email in About me, as Tasks counts them (`tasks::Me`, #114).

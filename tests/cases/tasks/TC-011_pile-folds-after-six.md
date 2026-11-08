---
id: TC-011
title: A pile shows six cards, then "+ N" and "Show fewer"
priority: low
type: functional
requirements: [F-TASK-05]
module: tasks
run: agent
platform: any
tags: [tasks, piles]
---

# TC-011: A pile shows six cards, then "+ N" and "Show fewer"

**Module:** Tasks | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Run `echoctl project tasks` and count `piles.mine`; create tasks on Acme rollout until "I owe" holds exactly 7, each: `echoctl project ledger acme-rollout edit '{"op":"create","kind":"task","parent":"S001","title":"Fold test <n>","internal_owner":"Sam Rivera"}'` (n = 1, 2, …; note every printed id). Every `echoctl` command runs after `. <dir>/env` (the run's env file)
- In S-08 "Write the follow-up mail to Acme" (T003, from stage 10, tasks/TC-010) is still open and counts in `piles.mine`; cancel it in the Teardown with the tasks created here

## Test Data

| Field | Value |
|-------|-------|
| Titles | Fold test 1 … Fold test N |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "I owe" | "I owe 7" |
| 2 | Count the cards in "I owe" | Six cards; below them a button "+ 1" |
| 3 | Click "+ 1" | Seven cards; the button now reads "Show fewer" |
| 4 | Click "Show fewer" | Six cards and "+ 1" again |

## Expected Final State
The pile is folded to six cards with "+ 1".

## Teardown
- For each id printed in the preconditions: `echoctl project ledger acme-rollout edit '{"op":"edit","node":"<id>","set":{"state":"cancelled"}}'`

## Notes
- Creates N tasks on Acme rollout (cancelled in Teardown).

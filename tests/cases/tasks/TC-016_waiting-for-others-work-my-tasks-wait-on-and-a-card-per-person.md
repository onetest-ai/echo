---
id: TC-016
title: Waiting for others holds the work my tasks wait for and a card for each person they wait on
priority: high
type: functional
requirements: [F-TASK-01, F-TASK-02, F-TASK-06]
module: tasks
run: agent
platform: any
tags: [tasks, piles, waiting, pairwise]
---

# TC-016: Waiting for others holds the work my tasks wait for and a card for each person they wait on

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built
- "Others · 24" is folded (on its own). In S-25 `TC-018` (the stage before) has opened it: the scenario skips the click in step 3, since a second click would fold it

## Test Data

| Thread | Id | Card title | What it is | Line under the title | Due chip |
|---|---|---|---|---|---|
| Initech hiring | T013 | Unblock: agree the support hours (2) | work of someone else my task waits for | Alex Chen · stops T012 | no date |
| Globex pilot | T002 | Unblock: collect the user list | work of someone else my task waits for | Alex Chen · stops T001 | no date |
| Acme renewal | T018 | Unblock: send the revised timeline (2) | work of someone else my task waits for | Alex Chen · stops T017 | no date |
| Umbrella proposal | T012 | an answer on draft the status report (2) | a card for the person waited on | Zoe Martin · stops T012 | no date |
| Acme renewal | T003 | an answer on send the revised timeline | a card for the person waited on | Rita Gomez · stops T003 | no date |

Staying in Others (the task they hold up is not an open task of the user):

| Thread | Id | Title |
|---|---|---|
| Umbrella proposal | T002 | Unblock: review the test plan |
| Initech hiring | T005 | Unblock: agree the support hours |
| Initech hiring | T009 | Unblock: write the faq page |
| Acme renewal | T008 | Unblock: write the cut-over runbook |
| Acme renewal | T010 | Unblock: fix the sso redirect |
| Globex pilot | T007 | Unblock: schedule the steering call |
| Globex pilot | T010 | Unblock: reporting dashboard |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "Waiting for others" | The heading reads "Waiting for others 5"; the column shows five cards and no "+ N" button |
| 2 | For each row of the first table find the card by `[data-tk="open"][data-project=<thread id>][data-node=<Id>]` in the "Waiting for others" column | Every row exists once; each card shows its title, thread, line under the title and "no date"; the two cards for a person (their titles start with "an answer on") carry the class `tk-wait` |
| 3 | Click "Others · 24" and look for the titles of the second table | All seven "Unblock: …" cards are in Others and none of them is in "Waiting for others" |
| 4 | Look in "Waiting for others" for a card for Kim Park | None: Kim Park is waited on only by tasks that are closed or not the user's (Globex pilot T003, T004, T005, T008, T011) |
| 5 | Run `echoctl project tasks` and read `piles.waiting` | Five entries; the two with `"wait": true` have `who` "Zoe Martin" (Umbrella proposal T012) and "Rita Gomez" (Acme renewal T003); `blocks` of each wait card names the user's task it stops |

## Expected Final State
Waiting for others holds three tasks of Alex Chen and two cards for people; the other "Unblock" tasks are still in Others.

## Teardown
- None: the case only reads

## Notes
- A task of the user goes here when it waits on another open task (`waiting_on`) or on a person (`waiting_for`). The person's card takes its title from what is waited for ("an answer on …").
- Asks lists the same people: `TC-025`.

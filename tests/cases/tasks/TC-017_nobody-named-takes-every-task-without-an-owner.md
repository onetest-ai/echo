---
id: TC-017
title: Nobody named takes every open task without an owner, whoever handed it over
priority: high
type: functional
requirements: [F-TASK-01, F-TASK-02, F-TASK-06]
module: tasks
run: agent
platform: any
tags: [tasks, piles, nobody, pairwise]
---

# TC-017: Nobody named takes every open task without an owner, whoever handed it over

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

| Thread | Id | Title | State | Handed over | Due chip | Line under the title |
|---|---|---|---|---|---|---|
| Globex pilot | T004 | Check the firewall rules | blocked | handed over by Priya Nair | 3 days ago (late) | Day-to-day work |
| Globex pilot | T006 | Schedule the steering call | open | handed over by Priya Nair | today | Day-to-day work |
| Acme rollout | T011 | Confirm the go-live date | in_progress | handed over by Me | tomorrow | Day-to-day work |
| Umbrella proposal | T004 | Draft the status report (kept) | open | no one named | no date | Day-to-day work |
| Umbrella proposal | T008 | Archive the old tenant (kept) | open | no one named | no date | Day-to-day work |
| Initech hiring | T006 | Agree the support hours (kept) | open | no one named | no date | Day-to-day work |
| Initech hiring | T015 | Update the contact list (2) (kept) | open | no one named | no date | Day-to-day work |
| Acme renewal | T006 | Share the architecture diagram (kept) | open | no one named | no date | Day-to-day work |
| Acme rollout | T005 | Hold the help desk training session | open | handed over by Me | no date | SSO for the Berlin pilot |
| Acme rollout | T021 | Agree the acceptance criteria (kept) | open | no one named | no date | Day-to-day work |
| Globex pilot | T014 | Review the access matrix (2) (kept) | open | no one named | no date | Day-to-day work |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "Nobody named" | The heading reads "Nobody named 11" |
| 2 | Click the button "+ 5" at the foot of the "Nobody named" column | The column holds 11 cards and the button reads "Show fewer" |
| 3 | For each row of the table find the card `[data-tk="open"][data-project=<thread id>][data-node=<Id>]` in the column | Every row exists once with its title, thread name, line under the title (only the story, never "blocked", never an owner) and due chip |
| 4 | Read the cards of Globex pilot T004 "Check the firewall rules" and T006 "Schedule the steering call" | Both are there although one is in state blocked and the other waits for an item; the first has a red due chip (tooltip "Overdue"), the second a due chip for today that is not red |
| 5 | Run `echoctl project tasks` and read `piles.nobody` | 11 entries, each with `owner` null; `delegated_by` is "Me" for Acme rollout T005 and T011, "Priya Nair" for Globex pilot T004 and T006, and null for the others |

## Expected Final State
Nobody named holds the 11 tasks of the table, unfolded.

## Teardown
- None: the case only reads

## Notes
- A task without an owner is in Nobody named even when the user handed it over (Acme rollout T005, T011) or it is blocked (Globex pilot T004).

---
id: TC-018
title: Another person's task is Owed to me only when the user handed it over, else it is in Others
priority: high
type: functional
requirements: [F-TASK-01, F-TASK-02, F-TASK-06]
module: tasks
run: agent
platform: any
tags: [tasks, piles, owed, others, pairwise]
---

# TC-018: Another person's task is Owed to me only when the user handed it over, else it is in Others

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

**Owed to me (5 cards)**

| Thread | Id | Title | Line under the title | Handed over by | Due chip |
|---|---|---|---|---|---|
| Acme renewal | T011 | Test the password reset | Alex Chen · blocked | Me | today |
| Globex pilot | T005 | Book the training slots | Alex Chen · blocked | Me | tomorrow |
| Umbrella proposal | T010 | Check the licence count | Zoe Martin · client · blocked | Me | in 20 days |
| Globex pilot | T009 | Send the welcome mail | Kim Park · client | Me | in 20 days |
| Initech hiring | T008 | Write the FAQ page | Alex Chen · blocked | Me | no date |

**Others (24 cards; these 11 are read)**

| Thread | Id | Title | Line under the title | Handed over by | Due chip |
|---|---|---|---|---|---|
| Acme rollout | T014 | Send the meeting notes | Dana Lee · client | no one named | in 20 days |
| Umbrella proposal | T002 | Unblock: review the test plan | Alex Chen | no one named | no date |
| Initech hiring | T003 | After: set up the test tenant | Alex Chen | no one named | no date |
| Acme renewal | T005 | After: share the architecture diagram | Alex Chen · blocked | no one named | no date |
| Acme rollout | T002 | Get the access list | Omar Haddad · client | Dana Lee | no date |
| Acme rollout | T003 | Prepare the training deck | Rita Gomez · client · blocked | Dana Lee | no date |
| Acme rollout | T004 | Write the help desk guide | Rita Gomez · client | Dana Lee | no date |
| Acme rollout | T006 | Plan the rollback rehearsal | Tom Becker | Tom Becker | no date |
| Acme rollout | T007 | Answer the security questionnaire | Omar Haddad · client | Omar Haddad | no date |
| Acme rollout | T008 | Run the data migration dry run | Rita Gomez · client | Dana Lee | no date |
| Acme rollout | T012 | After: confirm the go-live date | Alex Chen · blocked | no one named | no date |

Others per thread: Acme rollout 11, Acme renewal 5, Initech hiring 3, Umbrella proposal 3, Globex pilot 2.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks` and wait for the text "Owed to me" | The heading reads "Owed to me 5" and the column holds five cards, no "+ N" button |
| 2 | For each row of the first table find the card in the "Owed to me" column | Every row exists once with its title, thread, line under the title ("Alex Chen · blocked", "Zoe Martin · client · blocked", "Kim Park · client", …) and due chip |
| 3 | Click the button "Others · 24" | The folded section opens and holds 24 cards; its note says "other people's work in your threads: not handed over by you, not stopping you" |
| 4 | For each row of the second table find the card in Others | Every row exists once; the client-owed ones (Dana Lee, Omar Haddad, Rita Gomez) read "· client"; Tom Becker's T006 reads "Tom Becker" only |
| 5 | Count the Others cards per thread | Acme rollout 11, Acme renewal 5, Initech hiring 3, Umbrella proposal 3, Globex pilot 2 |
| 6 | Look for Acme rollout T002 "Get the access list" (owner Omar Haddad) in "Owed to me" | It is not there: it was handed over by Dana Lee, not by the user, so it is in Others |
| 7 | Run `echoctl project tasks` and read `piles.owed` and `piles.others` | `owed` has 5 entries all with `delegated_by` "Me" and an owner who is not the user; `others` has 24 entries, none with `delegated_by` "Me" |

## Expected Final State
Owed to me holds 5 cards; Others is open with its 24 cards.

## Teardown
- None: the case only reads

## Notes
- Extends `TC-007` (one task handed over) to the full board: owner (client, colleague) by handed over by (the user, another person, no one) by state.

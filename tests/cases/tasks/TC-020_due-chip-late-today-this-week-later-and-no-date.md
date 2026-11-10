---
id: TC-020
title: A due chip is red when late, shows the weekday this week, and reads "no date" when there is none
priority: high
type: functional
requirements: [F-TASK-06, F-TASK-02]
module: tasks
run: agent
platform: any
tags: [tasks, cards, due, late, pairwise]
---

# TC-020: A due chip is red when late, shows the weekday this week, and reads "no date" when there is none

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No task or setting has been changed since the data was built

## Test Data

| Thread | Id | Title | Pile | Due | Chip |
|---|---|---|---|---|---|
| Acme rollout | T010 | Book the workshop room | I owe | 3 days ago (late) | `tk-due late`, tooltip "Overdue", no weekday |
| Acme rollout | T016 | Map the user groups | I owe | 3 days ago (late) | `tk-due late`, tooltip "Overdue", no weekday |
| Globex pilot | T004 | Check the firewall rules | Nobody named | 3 days ago (late) | `tk-due late`, tooltip "Overdue", no weekday |
| Acme renewal | T003 | Send the revised timeline | I owe | today | `tk-due`, tooltip "Due", with weekday |
| Acme renewal | T011 | Test the password reset | Owed to me | today | `tk-due`, tooltip "Due", with weekday |
| Globex pilot | T006 | Schedule the steering call | Nobody named | today | `tk-due`, tooltip "Due", with weekday |
| Globex pilot | T001 | Collect the user list | I owe | tomorrow | `tk-due`, tooltip "Due", with weekday |
| Globex pilot | T005 | Book the training slots | Owed to me | tomorrow | `tk-due`, tooltip "Due", with weekday |
| Acme rollout | T011 | Confirm the go-live date | Nobody named | tomorrow | `tk-due`, tooltip "Due", with weekday |
| Umbrella proposal | T009 | Close the open tickets | I owe | in 20 days | `tk-due`, tooltip "Due", no weekday |
| Umbrella proposal | T010 | Check the licence count | Owed to me | in 20 days | `tk-due`, tooltip "Due", no weekday |
| Globex pilot | T009 | Send the welcome mail | Owed to me | in 20 days | `tk-due`, tooltip "Due", no weekday |
| Acme rollout | T014 | Send the meeting notes | Others | in 20 days | `tk-due`, tooltip "Due", no weekday |
| Acme renewal | T019 | Share the architecture diagram (2) | I owe | no date | `tk-due none`, text "no date" |
| Acme rollout | T005 | Hold the help desk training session | Nobody named | no date | `tk-due none`, text "no date" |
| Acme rollout | T002 | Get the access list | Others | no date | `tk-due none`, text "no date" |

The Due column is the date relative to the build; the chip shows the date itself, never "3 days ago" or "tomorrow". Its text follows the browser's locale ("Nov 7", "Tue, Nov 10", "Wed, Nov 11", "Nov 30" in en-US, the Playwright browser's), so only the weekday is asserted: it is written (for example "Fri") when the date is today or within the next 7 days, and not written for a date more than a day ago or more than 7 days ahead. Dates in the data: 3 days ago, today, tomorrow, in 20 days.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#tasks`, wait for the text "I owe", click "+ 8" in "I owe", "+ 5" in "Nobody named" and "Others · 24" | All piles are unfolded |
| 2 | For each row of the table find the card and read its due chip `.tk-due` (class, `title` attribute, text) | The chip matches the Chip column of its row |
| 3 | Count the chips with the class `late` on the whole page | Exactly 3: Acme rollout T010, Acme rollout T016, Globex pilot T004; the chips for today and tomorrow are not red |
| 4 | Count the cards whose chip is not `none` | 13 cards have a date (3 late, 3 today, 3 tomorrow, 4 in 20 days); the other 46 read "no date" |

## Expected Final State
Every card of the table shows the chip of its row; 3 red chips on the page.

## Teardown
- None: the case only reads

## Notes
- This is how Tasks shows the rule "this week": a date of today or the coming 7 days gets a weekday, an older or a farther one does not; a task due today is not late.
- Today's "My work · this week" block is being reworked and is not checked here.
- Late is decided by the app on its local date (`tasks.rs`); the Work board decides it in UTC (`work/TC-025`).

---
id: TC-025
title: A board card shows its owner avatar, a late due in red, who it waits on and what it holds up
priority: medium
type: functional
requirements: [F-WORK-04]
module: work
run: agent
platform: any
tags: [work, board, cards, late]
---

# TC-025: A board card shows its owner avatar, a late due in red, who it waits on and what it holds up

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- Globex pilot's view "Board" is active with the default fields (story, kind, owner, due, waiting on, holds up)

## Test Data

| Id | Title | Column | Avatar | Chips |
|---|---|---|---|---|
| T005 | Book the training slots | Waiting | "AC" titled "Alex Chen" | due tomorrow, waiting on Kim Park |
| T004 | Check the firewall rules | Blocked | "?" titled "Nobody named" | due 3 days ago (late), waiting on Kim Park |
| T002 | Unblock: collect the user list | To do | "AC" titled "Alex Chen" | holds up 1 |
| T001 | Collect the user list | Blocked | "SR" titled "Sam Rivera" | due tomorrow |
| T009 | Send the welcome mail | In progress | "KP" titled "Kim Park" (the client owes it) | due in 20 days, holds up 1 |
| T006 | Schedule the steering call | To do | "?" titled "Nobody named" | due today |
| Q001 | When does finance decide? | Blocked | none (a question has no avatar) | kind "question" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work`, click the view "Board" | The five columns are shown |
| 2 | For each row of the table find the card and read its top line (id and story), its avatar `.bd-av`, its title and its chips `.bd-chip` | The card is in the Column of its row; its top line reads the id and "· Day-to-day work" (the story) for the tasks; the avatar and chips are those of the row; the waiting chip reads "⏳" and the name; the holds-up chip reads "holds up N" |
| 3 | Read the due chip of T004 and of T006 | T004's has the class `late` and the tooltip "Overdue"; T006's (today) has no `late` class and the tooltip "Due" |
| 4 | Read the avatar of T009 "Send the welcome mail" | It has the class `owed` and the tooltip "Kim Park (the client owes it)" |

## Expected Final State
Each card of the table shows what its row says.

## Teardown
- None: the case only reads

## Notes
- The board decides "late" with the UTC date (`new Date().toISOString()`), Tasks with the local date: between local midnight and the UTC offset a task due today is late on Tasks and not on the board, or the other way round. Run the case at midday.
- Extends `TC-004` (two cards) to client-owed, nobody-named, waiting and holds-up cards.

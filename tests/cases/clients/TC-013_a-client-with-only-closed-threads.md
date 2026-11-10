---
id: TC-013
title: A client whose threads are all closed shows them as closed cards, no open tasks, and cannot be removed
priority: medium
type: functional
requirements: [F-CLIENT-01, F-CLIENT-02, F-CLIENT-04, F-CLIENT-06]
module: clients
run: agent
platform: any
tags: [clients, closed, threads, empty]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-013: A client whose threads are all closed shows them as closed cards, no open tasks, and cannot be removed

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No stage has been changed since the data was built

## Test Data

| Field | Value |
|---|---|
| Client | Tyrell (tyrell.example) |
| Threads | Tyrell bid, stage Lost, closed 5 days ago, 1 call |
| Person seen | Eldon Ward |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#projects` and read the group "Tyrell" | The name "Tyrell" has no thread row under it and no fold arrow; its thread "Tyrell bid" is in the fold "Closed · 3" |
| 2 | Click the name "Tyrell" | The route is `#client/tyrell/overview`; the head says "Tyrell", the tag "client" and "1 thread · tyrell.example" |
| 3 | Read the tabs | "Overview", "Calls" with the number 1, "Tasks" with no number, "People", "Profile" |
| 4 | Read the section "Threads" of the Overview | One card, closed (class `closed`): "Tyrell bid" with the tag "Lost", "1 call · last …", the text "No open tasks" and nothing about decisions, risks or questions; no "N of M stories done" |
| 5 | Click the tab "Calls" | One row: "Tyrell bid review" with the chip "Tyrell bid" |
| 6 | Click the tab "Tasks" | Four piles "I owe", "Owed to me", "Waiting for others", "Nobody named", each with 0 and "Nothing here"; no button "Others" |
| 7 | Click the tab "People" | The map shows "Nobody in this client's threads yet" with "1 people were in its calls — add them from List, then drag them where they stand." (the plural is not adapted) and below it "1 more was in the calls and isn't in a thread yet — List." |
| 8 | Click the tab "Profile" | The row "Threads" holds "Tyrell bid" and the line "A client with threads stays: move or delete them first."; there is no button "Remove this client…" |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Covers org.threads = closed threads only. The sentence "1 people were in its calls" and its advice to add people from List do not fit a client page (no person can be added there). Verify at run time.
- Extends `TC-007` (an open thread blocks removal) to a closed one.

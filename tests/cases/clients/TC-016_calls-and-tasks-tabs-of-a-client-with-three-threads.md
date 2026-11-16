---
id: TC-016
title: The Calls tab of Acme lists the calls of its three threads and the Tasks tab sums their piles
priority: medium
type: functional
requirements: [F-CLIENT-03, F-CLIENT-04]
module: clients
run: agent
platform: any
tags: [clients, calls, tasks]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-016: The Calls tab of Acme lists the calls of its three threads and the Tasks tab sums their piles

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No call or task has been changed since the data was built

## Test Data

**Calls tab (newest first)**

| Call | Thread chip | Minutes |
|---|---|---|
| Memo | Acme rollout | - |
| Acme weekly sync | Acme rollout | - |
| Acme security review | Acme rollout | no minutes yet |
| Acme renewal negotiation | Acme renewal | - |
| Acme renewal pricing | Acme renewal | - |
| Acme weekly sync | Acme rollout | - |
| Globex pilot scoping | Acme rollout | - |
| Acme weekly sync | Acme rollout | - |
| Acme hackathon planning | Acme hackathon | - |

**Tasks tab**: I owe 6, Owed to me 1, Waiting for others 2, Nobody named 4, Others · 16 (tab number 13 = 6 + 1 + 2 + 4).

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#client/acme/calls` and wait for "Memo" | The tab "Calls" carries 9; the rows are those of the table, newest first, each with its thread chip; "Acme security review" says "no minutes yet" |
| 2 | Click the row "Globex pilot scoping" | The call opens (`#call/…`) |
| 3 | Navigate to `{{base_url}}/#client/acme/tasks` | The tab "Tasks" carries 13; the headings read "I owe 6", "Owed to me 1", "Waiting for others 2", "Nobody named 4" and the button "Others · 16" |
| 4 | Read the cards in "Waiting for others" | Two: "Unblock: send the revised timeline (2)" (Acme renewal) and "an answer on send the revised timeline" (the card for Rita Gomez, Acme renewal) |
| 5 | Click "Others · 16" | Sixteen cards: Acme rollout 11 and Acme renewal 5 |
| 6 | Compare with `{{base_url}}/#tasks` filtered by thread | The sums of "Acme rollout" and "Acme renewal" there give the same cards: I owe 3 + 3, Owed 0 + 1, Waiting 0 + 2, Nobody 3 + 1, Others 11 + 5 (the Overview counts waiting as 1 for Acme renewal: it does not count the card for a person) |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Extends `TC-003` (small data). "Globex pilot scoping" is in Acme through the pin of Acme rollout and in Globex through its people rule: `threads/TC-020`.

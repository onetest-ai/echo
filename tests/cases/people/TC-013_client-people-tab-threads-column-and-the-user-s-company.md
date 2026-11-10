---
id: TC-013
title: A client's People tab lists the people of all its threads with a Threads column; the user's company has only colleagues
priority: medium
type: functional
requirements: [F-PPL-01, F-PPL-04, F-CLIENT-07]
module: people
run: agent
platform: any
tags: [people, client, list, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-013: A client's People tab lists the people of all its threads with a Threads column; the user's company has only colleagues

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No person has been added or merged since the data was built

## Test Data

**Acme**

| Person | Where | Threads column | Organisation |
|---|---|---|---|
| Dana Lee | in a thread | Acme renewal, Acme rollout | Acme |
| Omar Haddad | in a thread | Acme renewal, Acme rollout | Acme |
| Rita Gomez | in a thread | Acme renewal, Acme rollout | Acme |
| Ivan Petrov | in a thread | Acme renewal, Acme rollout | Acme |
| Yuki Tanaka | in a thread | Acme hackathon, Acme renewal, Acme rollout | Acme |
| Alex Chen | in a thread | Acme renewal, Acme rollout | your company · Delivery |
| Lena Vogel | in a thread | Acme rollout | your company |
| Ola Svensson | in a thread | Acme rollout | Acme |
| Tom Becker | in a thread | Acme rollout | your company |
| Jo Weber | seen in calls | (empty) | Acme |
| Priya Nair | seen in calls | (empty) | your company |
| Finance Desk | seen in calls | (empty) | Acme |
| Noor Aziz | seen in calls | (empty) | Globex |
| Tom Becker | seen in calls | (empty) | outside |

**Initech (the user's company)**: Guest 2 (fill2.example), Guest 7 (fill7.example), Lena Vogel (your company), Priya Nair (your company), Tom Becker (your company) — none in a thread.

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#client/acme/people`, press "List" | The tab "People" shows 9; the table has the column "Threads" in place of "Role" and no "+ Person" button; the card holds the 9 people of the first block |
| 2 | For each row of the Acme table read the Threads column and the Organisation column | The thread names (one to three chips; none for a person seen in calls only: they are in no thread) and the Organisation of the row |
| 3 | Read the block "Seen in calls" | 5 people, with no "Add" buttons and no "Add all" (a client has no people list of its own) |
| 4 | Press "Map" | The chips are in the quadrants of their strongest place in any Acme thread (Dana Lee and Alex Chen in "Manage closely"); the line under them says "5 more were in the calls and aren't in a thread yet"; chips are not draggable |
| 5 | Navigate to `{{base_url}}/#client/initech/people`, press "List" | "Nobody in this client's threads yet." and a block "Seen in calls" with 5 people; the Organisation column reads "your company" for Lena Vogel, Priya Nair and Tom Becker and a bare domain for Guest 2 and Guest 7 |

## Expected Final State
Nothing changed.

## Teardown
- Press "Map" in the segmented control

## Changes
- Changes the remembered People view in the browser.

## Notes
- The user (Sam Rivera) is never in the list.
- The data places Acme's people in three of its threads: Acme rollout (nine, with their map places), Acme renewal (Dana Lee, Omar Haddad, Rita Gomez, Ivan Petrov, Yuki Tanaka, Alex Chen) and Acme hackathon (Yuki Tanaka), the last two without levels (`build_people` in `tests/fixtures/make_data.py`).
- Extends `clients/TC-008` (the company without threads) with the thread Initech hiring, which has no one placed.

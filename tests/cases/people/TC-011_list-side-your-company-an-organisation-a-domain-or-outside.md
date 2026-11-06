---
id: TC-011
title: The Organisation column of the list says your company, the known organisation, the bare domain or outside
priority: medium
type: functional
requirements: [F-PPL-04, F-PPL-07, F-CLIENT-07]
module: people
run: agent
platform: any
tags: [people, list, side, pairwise]
---

# TC-011: The Organisation column of the list says your company, the known organisation, the bare domain or outside

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No organisation has been named since the data was built

## Test Data

| Person | Where | Side in the data | Organisation column |
|---|---|---|---|
| Dana Lee | in the thread | org | Acme |
| Alex Chen | in the thread | yours | your company |
| Lena Vogel | in the thread | yours | your company |
| Tom Becker | in the thread | yours | your company |
| Priya Nair | seen in calls | yours | your company |
| Jo Weber | seen in calls | org | Acme |
| Noor Aziz | seen in calls | org | Globex |
| Tom Becker | seen in calls | unknown | outside |
| Guest 2 | seen in calls (Initech hiring) | other | fill2.example |
| Guest 7 | seen in calls (Initech hiring) | other | fill7.example |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/people`, press "List" | The list is shown |
| 2 | For each row of the table that says Acme rollout (all but the two Guests) find the row of the person (`tr[data-key]`) and read the column Organisation | It reads the last column of the row: "your company" for a colleague, "Acme" or "Globex" for an outside person known by its mail domain, "outside" for the second Tom Becker (no address at all) |
| 3 | Navigate to `{{base_url}}/#project/initech-hiring/people`, press "List" | The card says "Nobody in this thread yet."; the block "Seen in calls" lists 5 people |
| 4 | Read the rows of Guest 2 and Guest 7 | Their Organisation column reads the domain, "fill2.example" and "fill7.example" (a domain no organisation has) |
| 5 | Click the row of Guest 2 | The right panel shows the person with a link "Name this organisation (fill2.example)"; it is not pressed |
| 6 | Navigate to `{{base_url}}/#project/initech-hiring/people` and press "Map" | The map is empty: "Nobody in this thread yet" and "5 people were in its calls — add them from List, then drag them where they stand." |

## Expected Final State
The People view is Map.

## Teardown
- Press "Map" in the segmented control

## Changes
- Changes the remembered People view in the browser.

## Notes
- person.side: yours (a colleague, whose organisation is the user's company), org (a known organisation), other (only a domain), unknown (no address). Colleagues never count as a client's people.

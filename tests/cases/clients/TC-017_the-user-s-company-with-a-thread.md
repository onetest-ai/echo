---
id: TC-017
title: The user's company is "your company" even when it has a thread, which keeps it from being removed
priority: medium
type: functional
requirements: [F-CLIENT-07, F-CLIENT-02, F-THR-01]
module: clients
run: agent
platform: any
tags: [clients, internal, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-017: The user's company is "your company" even when it has a thread, which keeps it from being removed

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- The switch "Your own company" has not been changed since the data was built

## Test Data

| Field | Value |
|---|---|
| Company | Initech (initech.example), the user's company |
| Thread | Initech hiring, stage Wrap-up, 3 calls |
| People | 5 seen in calls, none in the thread: Lena Vogel, Priya Nair, Tom Becker, Guest 2, Guest 7 |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#projects` and read the group of Initech | It reads "Initech · your company" and sits after Duff and before "No client"; its row is "Initech hiring" with the tag "Wrap-up" and "3 calls" |
| 2 | Click the name "Initech · your company" | The route is `#client/initech/overview`; the tag reads "your company" (not "client") and the line "1 thread · initech.example" |
| 3 | Read the Overview | One card, Initech hiring: "Wrap-up", "3 calls · last …", "0 of 2 stories done", "2 I owe", "1 owed to me", "1 waiting", "2 nobody named"; Decided "Keep the old tenant read-only"; Risks "Key user on leave" (no response yet, likelihood high · impact medium) before "Licences run out" (with its response) |
| 4 | Click the tab "Profile" | The switch "Your own company" is on with "Your own company (internal work)"; the line under the domains says "Its people are your colleagues: they never count as a client's people."; the row "Threads" holds "Initech hiring" and the line "A client with threads stays: move or delete them first." |
| 5 | Click the tab "People", then "List" in the segmented control "View" (People opens on the view last chosen, Map by default) | "Nobody in this client's threads yet" and 5 people in "Seen in calls" (see `people/TC-013`) |

## Expected Final State
Nothing changed.

## Teardown
- Press "Map" in the segmented control (the case changes no data)

## Notes
- Covers org.internal (the user's company) by org.threads (open threads). Extends `TC-008` (no threads).

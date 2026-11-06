---
id: TC-026
title: Details name the item of another thread that a task waits for, holds up, shares or mirrors
priority: medium
type: functional
requirements: [F-WORK-15, F-WORK-14]
module: work
run: agent
platform: any
tags: [work, details, xthread, pairwise]
---

# TC-026: Details name the item of another thread that a task waits for, holds up, shares or mirrors

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No link has been made or removed since the data was built

## Test Data

| Thread | Id | Title | Details row | Link | Button |
|---|---|---|---|---|---|
| Globex pilot | T005 | Book the training slots | Waits for | Acme rollout · T002 Get the access list · Omar Haddad | Unlink |
| Acme rollout | T002 | Get the access list | Holds up | Globex pilot · T005 in another thread (and in the thread: S001 and T003) | Unlink |
| Acme rollout | T009 | Prepare the renewal quote | Also in | Acme renewal · T001 Prepare the renewal quote | Unlink |
| Acme renewal | T001 | Prepare the renewal quote | Same as | Acme rollout · T009 Prepare the renewal quote — done there: its status, owner and due show here | Unlink |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/work`, click the view "Board" and click the card T005 "Book the training slots" | Details shows the title and a row "Waits for" with the link "Acme rollout · T002 Get the access list", the text "· Omar Haddad" and a button "Unlink" |
| 2 | Click the link "Acme rollout · T002 Get the access list" | The route is `#project/acme-rollout/work` and Details shows T002 "Get the access list" with a row "Holds up" that has a link "Globex pilot · T005" and the words "in another thread" |
| 3 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click the view "Board" and click the card T009 "Prepare the renewal quote" (in Done) | Details has a row "Also in" with the link "Acme renewal · T001 Prepare the renewal quote" and a button "Unlink" |
| 4 | Click that link | The route is `#project/acme-renewal/work` and Details shows T001 with the row "Same as": "Acme rollout · T009 Prepare the renewal quote" and "— done there: its status, owner and due show here" |
| 5 | Do not press "Unlink" | Nothing changes: `echoctl project ledger acme-renewal tree` still lists the link on T001 |

## Expected Final State
The four links are shown and still in place.

## Teardown
- None: the case only reads

## Notes
- Covers node.xthread: waits for an item elsewhere, holds up an item elsewhere, master with mirrors, mirror of a master. `TC-017` makes and removes a link; here the data already holds them.
- The mirror T001 and its master T009 are both done, so neither is on Tasks and Tasks shows no "also in" mark on any card.

---
id: TC-010
title: Not this on a found thread drops the item; an emptied client group disappears
priority: medium
type: functional
requirements: [F-INBOX-04, F-INBOX-07]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, found, decision]
---

# TC-010: Not this on a found thread drops the item; an emptied client group disappears

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), within 10 seconds, no reload: the sign-in check of the AI profiles settles in the background and the window reads the Inbox again as soon as it does (#513); before it, the answer lists 24 open items and none blocking

## Test Data

| Field | Value |
|---|---|
| Item | New thread? “Duff calls” (group Duff, the only item of the group) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox"; note the numbers on the chips "Open" (N) and "Done" (D) | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown (on fresh data N = 25, D = 2; earlier decisions in a scenario lower N and raise D) |
| 2 | Read the header "Duff" | The group "Duff" with 1 item "New thread? “Duff calls”" with the buttons "Create", "Not this", "Open" |
| 3 | Click "Not this" | Within 5 seconds the item is gone and the header "Duff" with it; the chip reads "Open N−1" |
| 4 | Click the chip "Done" | "Done D+1" holds "New thread? “Duff calls”" |
| 5 | Run `echoctl project found` | No entry for `domain:duff.example`; the list still has Northwind, Umbrella lead notes 1, Initrode and Design guild |

## Expected Final State
The Duff group is gone.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Writes the rejection (`found_reject`); no network.

---
id: TC-013
title: Not this on a Teams transcript item dismisses it and lists it as done
priority: medium
type: functional
requirements: [F-INBOX-04, F-INBOX-07]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, transcript, decision]
---

# TC-013: Not this on a Teams transcript item dismisses it and lists it as done

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Wait 10 seconds after the server starts: the sign-in check of the AI profiles settles in the background, and only then the blocking item "Sign in to Claude · out" is in the Inbox (the very first answer lists 24 open items and none blocking)

## Test Data

| Field | Value |
|---|---|
| Item | Teams transcript: Acme design review |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Click "Not this" on "Teams transcript: Acme design review" | Within 5 seconds the item is gone from Acme; "Open" counts one less |
| 3 | Click the chip "Done" | The list holds "Teams transcript: Acme design review" struck through, with "today" |
| 4 | Navigate to `{{base_url}}/#calendar/day`, click `[data-ev="ev-m01"]` | The card still offers "Import Teams transcript" (dismissing the item does not hide the meeting's own button) |

## Expected Final State
The item is under Done.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Writes the dismissal (`teams_transcript_dismiss`); no network.

---
id: TC-012
title: Import on a Teams transcript item fails cleanly: the item keeps its buttons and says why
priority: high
type: functional
requirements: [F-INBOX-06, F-INBOX-04]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, transcript, failure]
---

# TC-012: Import on a Teams transcript item fails cleanly: the item keeps its buttons and says why

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), within 10 seconds, no reload: the sign-in check of the AI profiles settles in the background and the window reads the Inbox again as soon as it does (#513); before it, the answer lists 24 open items and none blocking

## Test Data

| Field | Value |
|---|---|
| Item | Teams transcript: Acme design review (group Acme) |
| Message | Not imported: not signed in to Microsoft 365 — `echoctl calendar login` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Click "Import" on "Teams transcript: Acme design review" and wait up to 10 seconds | The item is still listed; under its title a line (`.ny-err`) reads "Not imported: not signed in to Microsoft 365 — `echoctl calendar login`" |
| 3 | Read the item's buttons | "Import", "Not this" and "Open" are back (no "Working…") |
| 4 | Click "Open" on the item | The URL becomes `#calendar/day`; the panel stays open |
| 5 | Click the block `[data-ev="ev-m01"]` (Acme design review) in the calendar | The card's line reads "not signed in to Microsoft 365 — `echoctl calendar login`" |

## Expected Final State
The import failed; the item and the meeting are unchanged.

## Teardown
- Click "Close" on the card and "Inbox" to close the panel

## Notes
- Nothing is written but the in-memory error. If `calendar/TC-014` ran before, the message is already there after the page loads: it still must show after the click.

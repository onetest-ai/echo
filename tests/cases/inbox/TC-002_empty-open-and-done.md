---
id: TC-002
title: An empty Inbox says Nothing needs you; Done says nothing was decided this week
priority: medium
type: functional
requirements: [F-INBOX-02]
module: inbox
run: agent
platform: any
tags: [inbox, empty]
---

# TC-002: An empty Inbox says Nothing needs you; Done says nothing was decided this week

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Nothing waits for Sam and nothing was decided this week

## Test Data

| Field | Value |
|-------|-------|
| Chips | Open, Done (no Blocking chip while nothing blocks) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day", and click "Inbox" | The panel opens on "Inbox" |
| 2 | Read the chips at the top | "Open" and "Done" with the text "this week" beside them; no chip "Blocking"; "Open" is selected (`.fchip.on`) |
| 3 | Read the body | "Nothing needs you" with the line "Echo counts it here when something does." |
| 4 | Click the chip "Done" | "Done" is selected; the body says "Nothing decided this week yet." |
| 5 | Click the chip "Open" | "Nothing needs you" is back |

## Expected Final State
The Inbox panel is open on Open and says Nothing needs you.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Creates or changes nothing. Blocking chip, groups and Done rows need items: `standard` data set.

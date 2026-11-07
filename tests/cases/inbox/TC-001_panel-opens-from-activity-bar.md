---
id: TC-001
title: The Inbox button opens and closes the right panel on its tab, with no badge when nothing waits
priority: high
type: functional
requirements: [F-INBOX-01]
module: inbox
run: agent
platform: any
tags: [inbox, panel]
---

# TC-001: The Inbox button opens and closes the right panel on its tab, with no badge when nothing waits

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Nothing waits for Sam on the data (`echoctl attention` lists no open item)

## Test Data

| Field | Value |
|-------|-------|
| Button | Inbox (`[data-panel-flip="needs"]`, at the bottom of the activity bar) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for "Your day" | The activity bar shows the button "Inbox" with no number badge on it (no element labelled "… open") |
| 2 | Click "Inbox" | The right panel (`[aria-label="Right panel"]`) opens on the tab "Inbox"; the button is pressed (`aria-pressed="true"`); the URL stays `#today` |
| 3 | Navigate to `{{base_url}}/#calls` | The panel is still open on "Inbox" (it follows the user from view to view) |
| 4 | Click "Inbox" again | The panel closes; the button is not pressed |

## Expected Final State
The right panel is closed and the Inbox button shows no badge.

## Notes
- The badge with the open count, in warn colour when one blocks, needs open items: waits for the `standard` data set (F-INBOX-01 is only half covered here).

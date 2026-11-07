---
id: TC-004
title: The Inbox button's badge counts the open items and is in warn colour while one blocks work
priority: high
type: functional
requirements: [F-INBOX-01]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, badge]
---

# TC-004: The Inbox button's badge counts the open items and is in warn colour while one blocks work

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Wait 10 seconds after the server starts: the sign-in check of the AI profiles settles in the background, and only then the blocking item "Sign in to Claude · out" is in the Inbox (the very first answer lists 24 open items and none blocking)

## Test Data

| Field | Value |
|---|---|
| Open items | 25 (1 blocking: Sign in to Claude · out) |
| Button | `[data-panel-flip="needs"]` with `.act-badge`, `aria-label="25 open"` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today` and wait for "Your day" | The button "Inbox" carries a badge reading 25 with `aria-label="25 open"` |
| 2 | Read the badge's class | `act-badge` without `new` (the warn colour is used because an item blocks; a non-blocking badge has `new`) |
| 3 | Click "Inbox" | The panel opens on "Inbox"; the button is pressed; the badge still reads 25 |
| 4 | Click "Inbox" again | The panel closes; the badge is still there |

## Expected Final State
The panel is closed and the badge shows 25 in warn colour.

## Notes
- Creates or changes nothing.
- The neutral badge (open items, none blocking) needs a data set without the signed-out profile.

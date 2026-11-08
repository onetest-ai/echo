---
id: TC-001
title: The activity bar lists Today, Calls, Calendar, Threads, Tasks and Settings
priority: medium
type: functional
requirements: [F-WS-01]
module: shell
run: agent
platform: any
tags: [shell, activity-bar]
---

# TC-001: The activity bar lists Today, Calls, Calendar, Threads, Tasks and Settings

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for the text "Your day" | The navigation "Activities" shows |
| 2 | Read the buttons of the navigation "Activities" from top to bottom | Today, Calls, Calendar, Threads, Tasks, then (at the bottom) Settings; no "Inbox" (it is in the top bar) |
| 3 | Look for a button "Knowledge" | None (Recall is off on test data) |
| 4 | Read the button "Expand the sidebar" in the title strip | It shows; the buttons carry icons and the names are not drawn beside them |

## Expected Final State
The bar is folded to icons with the six buttons.

## Notes
- First paint can show an extra "Knowledge" button for a moment: wait for the text first, then read.

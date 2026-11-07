---
id: TC-002
title: Expand the sidebar shows the names and the choice is remembered
priority: medium
type: functional
requirements: [F-WS-01]
module: shell
run: agent
platform: any
tags: [shell, activity-bar]
---

# TC-002: Expand the sidebar shows the names and the choice is remembered

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/window-layout.json` key `bar`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`; click "Expand the sidebar" | The button now reads "Fold the sidebar"; the names Today, Calls, Calendar, Threads, Tasks, Inbox, Settings are drawn beside the icons |
| 2 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | The file has `"bar": {"open": true}` |
| 3 | Reload `{{base_url}}/#today` | The button still reads "Fold the sidebar" and the names are drawn |
| 4 | Click "Fold the sidebar" | The button reads "Expand the sidebar"; the names are gone |
| 5 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | The file has `"bar": {"open": false}` |

## Expected Final State
The sidebar is folded, as it began.

## Teardown
- If the button reads "Fold the sidebar", click it.

---
id: TC-015
title: Under 900 px the sidebar opens over the page and Escape closes it
priority: low
type: functional
requirements: [F-WS-05, F-WS-01]
module: shell
run: agent
platform: any
tags: [shell, responsive]
---

# TC-015: Under 900 px the sidebar opens over the page and Escape closes it

**Module:** Window shell | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Resize the browser to 800 x 700, then navigate to `{{base_url}}/#today` | The button "Expand the sidebar" shows; the icons are drawn without names |
| 2 | Click "Expand the sidebar" | The button reads "Fold the sidebar"; the names Today, Calls, Calendar, Threads, Tasks are drawn over the page |
| 3 | Press Escape | The button reads "Expand the sidebar"; the names are gone |
| 4 | Click the top bar's button "Inbox" | The panel opens over the page: "Close the panel" shows |
| 5 | Click "Close the panel" | The panel closes |

## Expected Final State
The narrow window opened both over the page.

## Teardown
- Resize the browser back to 1201 x 868.

## Notes
- At this width the choice is for this session only: `window-layout.json` is not written for the bar.

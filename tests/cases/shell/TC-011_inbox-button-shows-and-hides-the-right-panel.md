---
id: TC-011
title: The Inbox button opens the right panel on the Inbox tab and closes it
priority: medium
type: functional
requirements: [F-WS-04]
module: shell
run: agent
platform: any
tags: [shell, panels]
---

# TC-011: The Inbox button opens the right panel on the Inbox tab and closes it

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`; look for "Close the panel" | None: the right panel is closed |
| 2 | Click the button "Inbox" at the right end of the top bar | The right panel opens with the tab "Inbox" selected and the text "Nothing needs you"; buttons "Move to the bottom panel" and "Close the panel" show |
| 3 | Click the tab "Vocabulary" in the panel | The panel shows "Names and terms the transcription should hear right." |
| 4 | Click the tab "Inbox", then the top bar's button "Inbox" again | The right panel closes |

## Expected Final State
The right panel opened on Inbox and closed.

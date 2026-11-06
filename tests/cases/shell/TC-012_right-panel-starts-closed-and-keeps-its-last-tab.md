---
id: TC-012
title: The right panel is closed at every window open but keeps its last tab
priority: medium
type: functional
requirements: [F-WS-04]
module: shell
run: agent
platform: any
tags: [shell, panels]
---

# TC-012: The right panel is closed at every window open but keeps its last tab

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/window-layout.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and click the activity button "Inbox", then the tab "Vocabulary" | The right panel is open on "Vocabulary" |
| 2 | Wait 1 s and reload `{{base_url}}/#today` | No "Close the panel" button: the panel is closed |
| 3 | Click the activity button "Inbox" | The panel opens; the tab "Vocabulary" is the one selected (the last tab is kept) |
| 4 | Run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | `"right"` has `"active": "vocabulary"` |

## Expected Final State
The panel opens on the tab it had last.

## Teardown
- Click "Close the panel"; Navigate to `{{base_url}}/#settings/layout` and click "Reset" in the card "Reset the layout".

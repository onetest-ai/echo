---
id: TC-023
title: Layout: a panel view is moved to Bottom and the choice is saved
priority: medium
type: functional
requirements: [F-SET-16, F-WS-04]
module: settings
run: agent
platform: any
tags: [settings, layout]
auto: [tests/auto/specs/S-13.spec.mjs]
---

# TC-023: Layout: a panel view is moved to Bottom and the choice is saved

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/window-layout.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/layout` | The heading "Layout" shows; the card "Panel views" lists 13 rows in this order: "My notes", "Also discussed", "Inbox", "Claude", "What moved", "Meeting", "Vocabulary", "Details", "Asks", "Person", "Source", "Topic", "Add a widget" (every registered view is listed, whatever is on screen; "Also discussed", "Source" and "Topic" only open in a panel with Recall on, "What moved" only for a call in a thread (#157), "Person" only on a People tab, "Add a widget" only while Today is being edited); "Vocabulary" has "Right" pressed, "Claude" has "Bottom" pressed |
| 2 | Click "Bottom" in the row "Vocabulary" | "Bottom" is pressed in that row; the bottom panel opens (`Bottom panel`) with a tab "Vocabulary" selected |
| 3 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | The file has `"views"` with `"vocabulary": {"area": "bottom"}` |

## Expected Final State
Vocabulary lives in the bottom panel, and `window-layout.json` says so.

## Teardown
- Click "Reset" in the card "Reset the layout".

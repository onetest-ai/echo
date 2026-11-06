---
id: TC-024
title: Layout: Reset puts every view back where it began
priority: medium
type: functional
requirements: [F-SET-16, F-WS-04]
module: settings
run: agent
platform: any
tags: [settings, layout]
---

# TC-024: Layout: Reset puts every view back where it began

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/window-layout.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/layout` and click "Bottom" in the row "Vocabulary" | "Bottom" is pressed in that row |
| 2 | Click "Reset" in the card "Reset the layout" | "Right" is pressed in the row "Vocabulary" again, "Bottom" in the row "Claude"; the sidebar is folded (button "Expand the sidebar") |
| 3 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | `"views": {}` and `"bar": {"open": false}` |

## Expected Final State
Every view is back in its own place.

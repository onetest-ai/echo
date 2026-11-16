---
id: TC-013
title: Move to the bottom panel puts the view in the bottom panel and Reset brings it back
priority: medium
type: functional
requirements: [F-WS-04, F-SET-16]
module: shell
run: agent
platform: any
tags: [shell, panels, layout]
auto: [tests/auto/specs/S-13.spec.mjs]
---

# TC-013: Move to the bottom panel puts the view in the bottom panel and Reset brings it back

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/window-layout.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and click the top bar's button "Inbox" | The right panel shows the tab "Inbox" |
| 2 | Click "Move to the bottom panel" | The bottom panel opens with a tab "Inbox" selected; the button now reads "Move to the right panel" |
| 3 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | `"views"` has `"needs": {"area": "bottom"}` |
| 4 | Click "Move to the right panel" | The right panel holds the tab "Inbox" again |
| 5 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/window-layout.json"` | `"views"` has `"needs": {"area": "right"}` |

## Expected Final State
Inbox is back in the right panel.

## Teardown
- Click the right panel's "Close the panel" (`[data-panel-toggle="right"]`: the bottom panel has its own "Close the panel" and stays open on its tab "Claude" after the move back); Navigate to `{{base_url}}/#settings/layout` and click "Reset" in the card "Reset the layout".

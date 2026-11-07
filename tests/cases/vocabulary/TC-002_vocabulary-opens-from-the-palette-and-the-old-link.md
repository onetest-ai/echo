---
id: TC-002
title: The Vocabulary panel opens from the actions palette and from #settings/vocabulary
priority: low
type: functional
requirements: [F-VOC-01, F-WS-03]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, panel]
---

# TC-002: The Vocabulary panel opens from the actions palette and from #settings/vocabulary

**Module:** Vocabulary | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, press `ControlOrMeta+k`, type `Vocabulary` | The palette lists "Vocabulary" in the group ECHO as the first item |
| 2 | Press Enter | The palette closes; the right panel opens with the tab "Vocabulary" selected |
| 3 | Click "Close the panel", then navigate to `{{base_url}}/#settings/vocabulary` | The right panel opens again with the tab "Vocabulary" selected (the old Settings link opens the panel, not a section) |

## Expected Final State
Both ways open the Vocabulary panel.

## Notes
- app_profile.md lists Vocabulary as not a route; `#settings/vocabulary` is kept as an old link that opens the panel.

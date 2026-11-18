---
id: TC-002
title: The Vocabulary panel opens from the actions palette, and #settings/vocabulary opens Settings → Vocabulary
priority: low
type: functional
requirements: [F-VOC-01, F-WS-03]
module: vocabulary
run: agent
platform: any
tags: [vocabulary, panel]
auto: [tests/auto/specs/S-13.spec.mjs]
---

# TC-002: The Vocabulary panel opens from the actions palette, and #settings/vocabulary opens Settings → Vocabulary

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
| 3 | Click "Close the panel", then navigate to `{{base_url}}/#settings/vocabulary` | Settings shows with "Vocabulary" selected in the left list and the heading "Vocabulary"; the right panel stays closed |

## Expected Final State
The palette opens the panel; the link opens the Settings section.

## Notes
- The Vocabulary panel is not a route; `#settings/vocabulary` is the Settings section (with the dictionary, `vocabulary/TC-010`).

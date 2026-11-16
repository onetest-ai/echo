---
id: TC-009
title: Marketplaces lists the built-in marketplace, not removable, with Read again and the Add form
priority: medium
type: functional
requirements: [F-PLUG-07]
module: plugins
run: agent
platform: any
tags: [plugins, marketplaces]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-009: Marketplaces lists the built-in marketplace, not removable, with Read again and the Add form

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins` and click the tab "Marketplaces" | The tab reads "Marketplaces" with the count `2`; the text "Where plugins come from." shows |
| 2 | Read the row "onetest" (My plugins has its own row, TC-028) | "onetest" with the tag "built in" and the source `onetest-ai/echo-plugins`; a button "Read again" (not pressed) and a button "⋯" |
| 3 | Click "⋯" on the row "onetest" | A menu with one item "Hide its plugins from Discover"; no "Remove" item |
| 4 | Click the heading "Plugins" outside the menu | The menu closes |
| 5 | Read the foot of the page | "Add a marketplace" with the boxes "Marketplace" (placeholder "owner/repo, a git address, or a folder") and "Name" (placeholder "Name (optional)") and a button "Add" |

## Expected Final State
The built-in marketplace is listed and cannot be removed.

## Notes
- How many plugins it offers and when it was read come from the network; not asserted.

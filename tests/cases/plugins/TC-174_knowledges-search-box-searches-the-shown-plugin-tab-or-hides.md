---
id: TC-174
title: Knowledge's search box searches the plugin tab shown, keeps each tab's words, and is hidden on a tab that has no search
priority: low
type: functional
requirements: [F-KNOW-10]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, knowledge, search]
---

# TC-174: The search box of a plugin tab

**Module:** Plugins | **Priority:** Low | **Type:** Functional

## Preconditions
- As TC-172 (Vault demo with its vault, Recall off)

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#brain/vault-demo:pages` | A search box "Search Pages" at the end of the tab row, before the gear |
| 2 | Type `q3` into it, one key at a time | The Pages list holds only "Page 07 Q3 plan/draft"; the box keeps the focus and its text |
| 3 | Click the tab "Inbox" | No search box (Inbox has none) |
| 4 | Click "Pages" | The box is back holding `q3`; the list is still filtered |
| 5 | Clear the box | All 60 pages are listed |

## Expected Final State
Unchanged data.

## Teardown
- In S-41: none. On its own: as TC-170.

## Notes
- With Recall on, a Recall tab has no such box in its row (its own finders are inside its page), so Recall is never searched from a plugin tab.

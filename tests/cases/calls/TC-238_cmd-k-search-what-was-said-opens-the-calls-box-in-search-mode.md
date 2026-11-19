---
id: TC-238
title: ⌘K's "Search what was said…" opens Calls with the words in the box in search mode and the results listed
priority: medium
type: functional
requirements: [F-CALLS-24]
module: calls
run: agent
platform: any
tags: [calls, search, recall, palette]
---

# TC-238: ⌘K's "Search what was said…"

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-233: standard data served at `{{base_url}}`, Recall on and synced, the page loaded again

## Test Data

| Field | Value |
|-------|-------|
| Words | sso cutover |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`, press ⌘K (Ctrl+K on Windows) | The palette opens; with nothing typed it has no "Search what was said…" item |
| 2 | Type `sso cutover` | The first group is "SEARCH" with the item "Search what was said… “sso cutover”" (picked), hint "calls, mail, channels, documents" |
| 3 | Press Enter | The palette closes; Calls opens with `sso cutover` in the box (it has the focus), the kind icons and the filter icon under it, and the results: "Said in calls, mail, channels · N" with "Re: SSO cutover window" among the sources |

## Expected Final State
⌘K started the same search the Calls box runs, in the Calls list.

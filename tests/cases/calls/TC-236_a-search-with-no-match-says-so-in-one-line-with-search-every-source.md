---
id: TC-236
title: A search with no match says so in one quiet line naming what was searched, and Search every source drops the filters
priority: medium
type: functional
requirements: [F-CALLS-23]
module: calls
run: agent
platform: any
tags: [calls, search, recall]
---

# TC-236: No match is one line, with Search every source

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-233: standard data served at `{{base_url}}`, Recall on and synced, the page loaded again

## Test Data

| Field | Value |
|-------|-------|
| Words | kangaroo invoice |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `kangaroo invoice`, press Enter | The group "Said in calls, mail, channels" (no count) holds one grey line "Nothing said matches “kangaroo invoice”." with no link; no banner or dialog |
| 2 | Click the Mail icon, then the filter icon and "Acme" under Client | The group reads "Said in mail"; its line: "Nothing in Acme’s mail says “kangaroo invoice”." followed by the link "Search every source"; a chip "Acme" |
| 3 | Click "Search every source" | The chip goes, no kind is pressed, and the line is again "Nothing said matches “kangaroo invoice”." |

## Expected Final State
Nothing found is one line inside its group; a narrowed search offers to search every source.

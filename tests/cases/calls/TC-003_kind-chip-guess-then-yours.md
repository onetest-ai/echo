---
id: TC-003
title: The kind-of-meeting chip shows a guess dashed, and a choice becomes the user's
priority: medium
type: functional
requirements: [F-CALLS-05, F-CALLS-08]
module: calls
run: agent
platform: any
tags: [calls, type]
---

# TC-003: The kind-of-meeting chip shows a guess dashed, and a choice becomes the user's

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The call's kind is still the guess (fresh data)

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync (guessed Client) |
| Other call | Initech planning (guessed 1:1) |
| Kind chosen | Partner |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | In the header the chip "Client ▾" (`select[aria-label="Kind of meeting"]`) has the class `guess` (dashed) and the tooltip "Guessed from who was invited — the minutes settle it" |
| 2 | Read the select's options | "Client", "Partner", "1:1", "Internal", "Industry & Analyst", "Other" |
| 3 | Select "Partner" (`browser_select_option`) | The chip reads "Partner ▾", is no longer dashed, and its tooltip is "You set this" |
| 4 | Navigate to `{{base_url}}/` (a reload) and open "Acme weekly sync" again | The chip still reads "Partner ▾" with the tooltip "You set this" |
| 5 | Open "Initech planning" | Its chip reads "1:1 ▾" and is dashed (still a guess) |

## Expected Final State
Acme weekly sync is typed Partner by the user; Initech planning is still a guess.

## Teardown
- Open "Acme weekly sync" and select "Client" (it stays marked "You set this"; there is no way back to a guess, so give every run fresh data)

## Notes
- Changes the kind of Acme weekly sync (Teardown restores the value, not the guess mark).

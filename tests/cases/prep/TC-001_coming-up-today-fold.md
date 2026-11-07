---
id: TC-001
title: Coming up today counts the meetings still to come and unfolds into rows
priority: high
type: functional
requirements: [F-PREP-01]
module: prep
run: agent
platform: any
tags: [prep, calls]
---

# TC-001: Coming up today counts the meetings still to come and unfolds into rows

**Module:** Prep | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Run before 22:00 local: the data puts the two meetings 1 h and 2.5 h ahead but keeps them inside today (never later than 23:50).

## Test Data

| Field | Value |
|-------|-------|
| Meetings to come | Globex design review (Kim Park), Acme demo (Dana Lee) |
| Not listed | Industry webinar (not accepted) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" | The list starts with a row "Coming up today" with the number 2 |
| 2 | Read that row | It names the next meeting: "Next HH:MM" (the time of Globex design review) and "Globex design review"; no meeting row is shown under it (folded) |
| 3 | Click the row "Coming up today" | `aria-expanded` becomes true; two rows appear: "Globex design review" with "30 min" and "Kim Park", and "Acme demo" with "30 min" and "Dana Lee", each with a start time; "Industry webinar" is not among them |
| 4 | Read a row's tooltip (`title`) | "No prep yet" |
| 5 | Type "Acme" into `[aria-label="Search calls"]` | The row "Coming up today" is gone while the search is on; clear the search and it returns |

## Expected Final State
Coming up today is unfolded with two meetings and the search box is empty.

## Teardown
- Click "Coming up today" to fold it again

## Notes
- Creates or changes nothing.

---
id: TC-001
title: The Calls list groups calls by day, newest first
priority: high
type: functional
module: window
run: agent
platform: any
tags: [calls]
external_id: TC-WIN-002
---

# TC-001: The Calls list groups calls by day, newest first

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Today | Initech planning, Acme weekly sync |
| Yesterday | Globex pilot kickoff |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` | The Calls list is on the left with the search box "Search titles and people" |
| 2 | Read the day headings and the rows under them | "Today": Initech planning, then Acme weekly sync; "Yesterday": Globex pilot kickoff |
| 3 | Read the row of Acme weekly sync | Its start time, "30 min" and its people starting "Sam Rivera, Dana Lee" |
| 4 | Read the item above the days | "Coming up today" with the next meeting, Globex design review |

## Expected Final State
The list shows the three calls under Today and Yesterday, newest first, each with time, length and people, under a Coming up today item.

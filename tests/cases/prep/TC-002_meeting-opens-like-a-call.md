---
id: TC-002
title: A coming meeting opens like a call with a Prep tab and the rest disabled
priority: medium
type: functional
requirements: [F-PREP-02]
module: prep
run: agent
platform: any
tags: [prep, calls]
---

# TC-002: A coming meeting opens like a call with a Prep tab and the rest disabled

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Run before 22:00 local: the data puts the two meetings 1 h and 2.5 h ahead but keeps them inside today (never later than 23:50).

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click "Coming up today" and then the row "Globex design review" | The URL is `#upcoming/<id>`; the row is selected |
| 2 | Read the header | Title "Globex design review", today's weekday and date with the start time, "30 min", the person "Kim Park", and a button "+ Thread" |
| 3 | Read the tabs | "Prep" is selected; "Transcript" and "Minutes" are disabled |
| 4 | Look for "Copy link" | Not shown (it comes with a written prep) |

## Expected Final State
The meeting is open on its Prep tab with Transcript and Minutes disabled.

## Teardown
- Click "Coming up today" to fold it

## Notes
- Creates or changes nothing.

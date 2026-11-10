---
id: TC-009
title: Calendar Day and Week show today's meetings and the line now
priority: high
type: functional
requirements: [F-CAL-01, F-CAL-02, F-CAL-03, F-CAL-04, F-CAL-06]
module: window
run: agent
platform: any
tags: [calendar]
external_id: TC-WIN-008
auto: [tests/auto/specs/S-02.spec.mjs]
---

# TC-009: Calendar Day and Week show today's meetings and the line now

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- Microsoft 365 is not signed in (test data)

## Test Data

| Field | Value |
|-------|-------|
| Today's meetings | Acme weekly sync, Initech planning, Globex design review, Industry webinar, Acme demo |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calendar/day` | Today's date as the title, "Day" selected, a notice that only today's meetings show until Microsoft 365 is connected |
| 2 | Read the meetings in the grid | All five of today's meetings at their times; the two recorded ones (Acme weekly sync, Initech planning) in the recorded (green) style |
| 3 | Find the current-time line | A line in the recording colour (coral) with a dot at the current time, between Initech planning and Globex design review |
| 4 | Click "Week" | Seven columns from yesterday to 5 days ahead; today's column highlighted with the same meetings and line |
| 5 | Click "›" then "Today" | The next week shows, then today's week again |

## Expected Final State
The calendar shows today's meetings in Day and Week with the line now, and moves by week and back to Today.

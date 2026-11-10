---
id: TC-001
title: Record when it starts schedules a coming meeting, and Don't record undoes it
priority: critical
type: functional
requirements: [F-CAL-09]
module: calendar
run: agent
platform: any
tags: [calendar, today, schedule]
external_id: TC-CAL-004
auto: [tests/auto/specs/S-02.spec.mjs]
---

# TC-001: Record when it starts schedules a coming meeting, and Don't record undoes it

**Module:** Calendar | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review (+1 h) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`; over Globex design review's line in Your day, click its ⋯ | Its menu: "Prepare" and "Record when it starts" |
| 2 | Click "Record when it starts" | `scheduled.json` holds it; its ⋯ menu now says "Don't record"; Acme demo's menu still says "Record when it starts" |
| 3 | Run `cat "$ECHOCTL_HOME/.echo/scheduled.json"` | It holds one meeting, subject "Globex design review" |
| 4 | Navigate to `{{base_url}}/#calendar/day` and click "Globex design review" | It is drawn as set to record; its card shows "Don't record" and "Echo starts recording a minute before it begins, transcribed as English." (the test data records in English) |
| 5 | Click "Don't record" on the card | The card offers "Record when it starts" again; `scheduled.json` no longer holds it |

## Expected Final State
A meeting set to record shows so in Today and the Calendar and is kept in `scheduled.json`; Don't record removes it from both.

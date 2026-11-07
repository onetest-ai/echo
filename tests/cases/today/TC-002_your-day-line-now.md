---
id: TC-002
title: Your day draws the line "now" between past and coming meetings
priority: medium
type: regression
requirements: [F-TODAY-02]
module: today
run: agent
platform: any
tags: [today]
---

# TC-002: Your day draws the line "now" between past and coming meetings

**Module:** Today | **Priority:** Medium | **Type:** Regression

## Preconditions
- Test data built by `tests/fixtures/make-data.sh` less than 30 minutes ago, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Last past meeting | Initech planning |
| First coming meeting | Globex design review |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` | Your day is visible |
| 2 | Find the line "now" in Your day (a time in the accent colour with a rule after it) | It shows the current time (HH:MM AM/PM) |
| 3 | Check what is directly above and below the line | "Initech planning" is above it, "Globex design review" directly below it |
| 4 | Check the rows below the line | "Globex design review" and "Acme demo" each show a "Prepare" button and "Record when it starts" |

## Expected Final State
The line "now" sits between Initech planning and Globex design review, and both coming meetings are visible under it with Prepare and Record when it starts.

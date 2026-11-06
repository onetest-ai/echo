---
id: TC-001
title: Your day lists only the meetings the user accepted
priority: high
type: regression
requirements: [F-TODAY-02, F-TODAY-01]
module: today
run: agent
platform: any
tags: [today, calendar]
---

# TC-001: Your day lists only the meetings the user accepted

**Module:** Today | **Priority:** High | **Type:** Regression

## Preconditions
- Test data built by `tests/fixtures/make-data.sh` less than 30 minutes ago, window served at `{{base_url}}`
- Today's agenda has 5 meetings; "Industry webinar" is not accepted (no response)

## Test Data

| Field | Value |
|-------|-------|
| Accepted meetings | Acme weekly sync, Initech planning, Globex design review, Acme demo |
| Not accepted | Industry webinar |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` | Today opens with the heading "Your day" |
| 2 | Read the meeting titles in Your day | Exactly: Acme weekly sync, Initech planning, Globex design review, Acme demo — in that order |
| 3 | Look for "Industry webinar" anywhere in Your day | It is not there |
| 4 | Read the line under the greeting | It says "4 meetings" |

## Expected Final State
Your day shows the four accepted meetings in time order and no "Industry webinar"; the greeting's line counts 4 meetings.

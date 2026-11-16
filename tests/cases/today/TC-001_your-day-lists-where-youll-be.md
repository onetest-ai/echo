---
id: TC-001
title: Your day lists where you'll be — the meetings you accepted, and one you haven't answered dashed with "no reply"; the greeting counts the ones you go to
priority: high
type: regression
requirements: [F-TODAY-02, F-TODAY-01]
module: today
run: agent
platform: any
tags: [today, calendar]
auto: [tests/auto/specs/S-02.spec.mjs]
---

# TC-001: Your day lists where you'll be

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
| 2 | Read the meeting titles in Your day (its list "Where you'll be") | Exactly: Acme weekly sync, Initech planning, Globex design review, Industry webinar, Acme demo — in time order (your work in a free gap, if any, is a line "Your work: …" between them) |
| 3 | Read Industry webinar's line | Its bar dashed grey, its reason "no reply" (you haven't answered the invite); it is never the next |
| 4 | Read the line under the greeting | It says "4 meetings" |

## Expected Final State
Your day shows the four accepted meetings in time order and no "Industry webinar"; the greeting's line counts 4 meetings.

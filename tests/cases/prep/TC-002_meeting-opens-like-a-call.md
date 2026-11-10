---
id: TC-002
title: A coming meeting opens like a call with its one tab, Prep
priority: medium
type: functional
requirements: [F-PREP-02]
module: prep
run: agent
platform: any
tags: [prep, calls]
auto: [tests/auto/specs/S-10.spec.mjs]
---

# TC-002: A coming meeting opens like a call with its one tab, Prep

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The data puts the meetings to come 1 h, 1.5 h and 2.5 h ahead; built after about 20:20 the later ones are closer, squeezed before 23:50 (tests/fixtures/README.md).

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click "Coming up today" and then the row "Globex design review" | The URL is `#upcoming/<id>`; the row is selected |
| 2 | Read the header | Title "Globex design review", today's weekday and date with the start time, "30 min", the person "Kim Park", and a button "+ Thread" |
| 3 | Read the tabs | One tab, "Prep", selected; no "Transcript" or "Minutes" (they are the call's, once it is recorded) |
| 4 | Look for "Copy link" | Not shown (it comes with a written prep) |

## Expected Final State
The meeting is open on its one tab, Prep.

## Teardown
- Click "Coming up today" to fold it

## Notes
- Creates or changes nothing.

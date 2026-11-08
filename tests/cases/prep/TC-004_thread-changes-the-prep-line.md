---
id: TC-004
title: Putting a meeting in a thread changes the No prep yet line and shows the thread
priority: medium
type: functional
requirements: [F-PREP-02, F-PREP-03]
module: prep
run: agent
platform: any
tags: [prep, calls, threads]
---

# TC-004: Putting a meeting in a thread changes the No prep yet line and shows the thread

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The data puts the two meetings 1 h and 2.5 h ahead; built after about 20:45 the later one is closer, squeezed before 23:50 (tests/fixtures/README.md).

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review |
| Thread | Globex pilot |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click "Coming up today" and then the row "Globex design review" | The pane says "This meeting is in no thread" |
| 2 | Click "+ Thread" | A menu lists "Acme rollout" and "Globex pilot" and "New thread from this meeting…"; do not choose the last |
| 3 | Choose "Globex pilot" | A chip "Globex pilot" with a "×" (label "Take it out of Globex pilot") appears in the header; the pane now says "Echo writes it this morning for every meeting of a thread." |
| 4 | Read the row "Globex design review" in the Calls list | It shows the thread's icon before the time |
| 5 | Click the "×" on the chip | The chip is gone; the pane says "This meeting is in no thread" again |

## Expected Final State
The meeting is in no thread again and its pane has the no-thread line.

## Teardown
- If the chip is still there: click its "×" (the × removes the pin)
- Click "Coming up today" to fold it

## Notes
- Pins the meeting to Globex pilot and removes the pin (a calendar event pin, no Claude Code).

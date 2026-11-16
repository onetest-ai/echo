---
id: TC-003
title: A meeting in no thread says No prep yet and offers Prepare now
priority: medium
type: functional
requirements: [F-PREP-03]
module: prep
run: agent
platform: any
tags: [prep, calls]
auto: [tests/auto/specs/S-10.spec.mjs]
---

# TC-003: A meeting in no thread says No prep yet and offers Prepare now

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The data puts the meetings to come 1 h, 1.5 h and 2.5 h ahead; built after about 20:20 the later ones start sooner, in the same order and over by 23:59, the accepted ones still 30 min (the unanswered Industry webinar then shorter); built after about 21:50 all three start sooner, and at 22:50 Globex design review starts 8 minutes after the build (tests/fixtures/README.md, #364).

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Acme demo (in no thread on the data) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click "Coming up today" and then the row "Acme demo" | The meeting opens on the Prep tab |
| 2 | Read the pane | The heading "No prep yet" and the text "This meeting is in no thread: Echo prepares it when you ask, from its people's mail and Teams." |
| 3 | Look for the button "Prepare now" | It is there (`[data-prep-write]`); do not press it (it starts Claude Code) |

## Expected Final State
The Prep tab says No prep yet and offers Prepare now; nothing was written.

## Teardown
- Click "Coming up today" to fold it

## Notes
- Creates or changes nothing. "Writing the prep…" and a written prep need a Claude Code run: not on `small`.

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
---

# TC-003: A meeting in no thread says No prep yet and offers Prepare now

**Module:** Prep | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`
- The data puts the two meetings 1 h and 2.5 h ahead; built after 22:00 they are closer, squeezed into the time left before 23:50 (tests/fixtures/README.md).

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

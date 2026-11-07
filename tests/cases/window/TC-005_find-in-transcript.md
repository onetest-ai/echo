---
id: TC-005
title: Find in transcript highlights the turns that match
priority: medium
type: functional
requirements: [F-TR-01, F-TR-02]
module: window
run: agent
platform: any
tags: [transcript, search]
external_id: TC-WIN-003
---

# TC-005: Find in transcript highlights the turns that match

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |
| Word | checklist |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click "Acme weekly sync", then the "Transcript" tab | Five turns, each with its time and speaker (Me or Remote) |
| 2 | Type `checklist` into "Find in transcript" | The two turns with "checklist" (0:13 and 0:21) are highlighted, the other three are not |
| 3 | Clear the box | No highlight is left |

## Expected Final State
Find highlights exactly the turns that contain the word and clears when emptied.

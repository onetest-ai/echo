---
id: TC-010
title: A call's minutes show only the sections they hold
priority: medium
type: functional
requirements: [F-MIN-01]
module: calls
run: agent
platform: any
tags: [minutes]
auto: [tests/auto/specs/S-09.spec.mjs]
---

# TC-010: A call's minutes show only the sections they hold

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Globex pilot kickoff (yesterday) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Globex pilot kickoff" | The call opens on Minutes inside the frame `iframe#minutes-frame`; the thread chip "Globex pilot" |
| 2 | Read the frame in the snapshot (its content is listed there; `browser_wait_for` does not see it) | The sections "Summary" with "Globex wants a pilot plan in two weeks" and "Action items" with "Me — draft the Globex pilot plan", each with a time chip ("00:05", "00:13") |
| 3 | Look for the other sections | No "Decisions" and no "Open questions" (these minutes have none) |

## Expected Final State
The minutes of Globex pilot kickoff show Summary and Action items only.

## Notes
- Creates or changes nothing. Do not act inside the frame with the sidebar unfolded and a panel open (see app_profile Fragile areas).

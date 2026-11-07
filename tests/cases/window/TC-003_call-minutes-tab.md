---
id: TC-003
title: A call with minutes opens on its Minutes with their sections
priority: critical
type: functional
requirements: [F-CALLS-05, F-CALLS-06, F-CALLS-07, F-MIN-01, F-MIN-02, F-MIN-04]
module: window
run: agent
platform: any
tags: [calls, minutes]
external_id: TC-WIN-003
---

# TC-003: A call with minutes opens on its Minutes with their sections

**Module:** Window | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Call | Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` and click "Acme weekly sync" | The call opens; the URL ends with `/minutes`; tabs "Minutes" and "Transcript" |
| 2 | Read the call's header | Title "Acme weekly sync", today's date and time, "30 min", the people, the thread chip "Acme rollout" |
| 3 | Read the minutes | A notice that AI wrote them from an automatic transcript, then the sections Summary, Decisions, Action items, Open questions |
| 4 | Read Action items | "Me — send Acme the SSO checklist today" and "Omar Haddad — answer the security questionnaire by Friday", each with a time chip |
| 5 | Read the panel beside the minutes | "What moved" lists the thread "Acme rollout" |

## Expected Final State
The call shows its minutes with all four sections, the AI notice, its thread, and What moved beside them.

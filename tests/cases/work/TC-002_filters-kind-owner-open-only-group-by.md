---
id: TC-002
title: The Work bar filters by kind, owner and Open only; the board groups by story or owner
priority: high
type: functional
requirements: [F-WORK-02]
module: work
run: agent
platform: any
tags: [work, filters]
---

# TC-002: The Work bar filters by kind, owner and Open only; the board groups by story or owner

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout's view is Board; no filter is set; add a decision: `echoctl project ledger acme-rollout edit '{"op":"create","kind":"decision","parent":"S001","title":"Pilot starts in Berlin"}'` (note the id it prints)

## Test Data

| Field | Value |
|-------|-------|
| Owner | Omar Haddad |
| Kind | Tasks |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` | The bar shows the selects "Layout", "Kind", "Owner", the button "Open only" (on) and "Group by", "Fields ▾" |
| 2 | Read the options of "Kind" | "All kinds · 4", "Stories · 1", "Tasks · 2", "Decisions · 1", "Risks · 0", "Questions · 0" |
| 3 | Read the options of "Owner" | "Anyone", "Nobody named", "Omar Haddad", "Sam Rivera" |
| 4 | Choose Owner `Omar Haddad` | The board has one card, "Answer the security questionnaire" (T002); the column "To do" shows 1 |
| 5 | Choose Owner `Anyone`, then "Group by" `Story` | A lane titled "S001" with "SSO for the Berlin pilot" and the progress "0/1" holds both cards |
| 6 | Choose "Group by" `Owner` | Lanes "Omar Haddad" (1) and "Sam Rivera" (1) |
| 7 | Choose "Group by" `No grouping`; click "Layout" → `Tree`, choose Kind `Decisions` | The tree lists only the decision row "Pilot starts in Berlin" (with its story above it) |
| 8 | Choose Kind `All kinds`, switch Layout back to `Board` | The board shows T001 and T002 again |

## Expected Final State
No filter is set on Acme rollout; the board shows both tasks.

## Teardown
- Delete the decision with the id the create printed: `echoctl project ledger acme-rollout edit '{"op":"delete","node":"<id>"}'`
- Set Kind `All kinds`, Owner `Anyone`, Group by `No grouping`, Layout `Board`

## Changes
- Creates then deletes a decision; changes view settings.

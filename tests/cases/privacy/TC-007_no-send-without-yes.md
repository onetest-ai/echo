---
id: TC-007
title: Follow-ups are never sent without an explicit yes
priority: critical
type: functional
requirements: [F-PRIV-09]
module: privacy
run: person
platform: any
tags: [privacy, sending]
external_id: TC-PRIV-007
---

# TC-007: Follow-ups are never sent without an explicit yes

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- A session that wrote minutes and offers follow-ups (claude TC-005)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reply with something unrelated, or nothing | No mail, Teams message or task is created |
| 2 | Say yes to one specific item | Only that item is sent or created |

## Expected Final State
Nothing goes out on the user's behalf without their yes to that item.

## Notes
- Stays person-run: follow-ups are offered in a signed-in session after minutes; Echo's own runs send nothing (`claude/TC-003` step 8, `claude/TC-013`, `hostile/TC-018`).

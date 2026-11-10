---
id: TC-013
title: A story's criteria are ticked in "Done when · m of n"; a risk's likelihood and impact are set
priority: high
type: functional
requirements: [F-WORK-11]
module: work
run: agent
platform: any
tags: [work, details, criteria]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-013: A story's criteria are ticked in "Done when · m of n"; a risk's likelihood and impact are set

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create a risk: `echoctl project ledger acme-rollout edit '{"op":"create","kind":"risk","parent":"S001","title":"SSO vendor delay"}'` (note the id it prints)

## Test Data

| Field | Value |
|-------|-------|
| Story | S001 SSO for the Berlin pilot, criterion "SAML works for Berlin users" |
| Risk | SSO vendor delay |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work`, click "Tree" and click the row S001 | Details: "Done when · 0 of 1" with an unticked checkbox "SAML works for Berlin users" |
| 2 | Tick the checkbox. Wait up to 3 s after an action for the window to redraw before reading. | "Done when · 1 of 1"; the tree row S001 reads "criteria 1 / 1"; S001's Status becomes "done" (a story with every criterion met is done: `ledger.rs`, "all acceptance criteria met"); the newest History line reads "you ticked criterion 1" (not the raw `you: ac_set`) |
| 3 | Click the row "SSO vendor delay" | Details: tag "risk", rows Likelihood "not rated · Set" and Impact "not rated · Set", a Response "No response yet · Add" |
| 4 | Click "Set" at Likelihood and choose `high`; click "Set" at Impact and choose `medium` | Likelihood "high", Impact "medium" |

## Expected Final State
S001's criterion is met and the risk is rated high / medium.

## Teardown
- Untick the checkbox; S001's Status goes back to "to do"
- Delete the risk with the id the create printed (`{"op":"delete","node":"<id>"}`)
- Click "Board"

## Changes
- Ticks a criterion; creates then deletes a risk.

## Notes
- Step 2's History line checks #153: a human ledger op other than `edit` and `create` was once shown raw (`you: ac_set`). The Status change is intended (`ledger.rs`, "all acceptance criteria met").

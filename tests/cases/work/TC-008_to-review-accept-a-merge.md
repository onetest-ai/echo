---
id: TC-008
title: To review: a suggested merge is accepted with Yes and the duplicate disappears
priority: high
type: functional
requirements: [F-WORK-08]
module: work
run: agent
platform: any
tags: [work, review, suggestions]
---

# TC-008: To review: a suggested merge is accepted with Yes and the duplicate disappears

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `echoctl project ledger acme-rollout sync`
- Add a near-duplicate through the agent path: `echoctl project ledger acme-rollout apply --evidence <id from sync's pending list, 2026-MM-DD_HHMMSS> --proposals /dev/stdin` with the standard input `[{"op":"create","kind":"task","parent":"S001","type":"action","title":"Send Acme the SSO checklist today","internal_owner":"Sam Rivera","quote":"I'll send the SSO checklist this afternoon."}]` → `by_op` has `suggest_merge: 1`

## Test Data

| Field | Value |
|-------|-------|
| Duplicate | Send Acme the SSO checklist today |
| Original | T001 Send Acme the SSO checklist |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` | The sync line shows "1 to review"; a banner "To review" reads "Same work as Send Acme the SSO checklist? Send Acme the SSO checklist today — Same as T001 “Send Acme the SSO checklist”? Their titles share most words (Echo's check, no model)." with buttons "Yes" and "No"; the board has three cards (T001, T002 and the new one) |
| 2 | Click "Yes". Wait up to 3 s after an action for the window to redraw before reading. | The banner is gone; the sync line has no "to review"; the board shows two cards (T001, T002) and no "Send Acme the SSO checklist today" |
| 3 | Run `echoctl project ledger acme-rollout tree` | The new task's `state` is `merged` |

## Expected Final State
The duplicate is merged into T001; nothing is left to review.

## Teardown
- None possible: the merge is kept. In a scenario the data are fresh

## Changes
- Adds task T003 and merges it into T001.

## Notes
Applying twice does nothing (the proposal is already known): run the precondition once per data set.

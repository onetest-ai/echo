---
id: TC-009
title: To review: No turns a suggestion down and both items stay
priority: high
type: functional
requirements: [F-WORK-08]
module: work
run: agent
platform: any
tags: [work, review, suggestions]
---

# TC-009: To review: No turns a suggestion down and both items stay

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `echoctl project ledger acme-rollout sync` (if not run yet: in S-06 stage 15, work/TC-003, ran it; the evidence id is the one it printed under `pending`, also `evidence` of Acme weekly sync in `manifest.json` → `calls`)
- `echoctl project ledger acme-rollout apply --evidence <the call's evidence id> --proposals /dev/stdin` with the standard input `[{"op":"create","kind":"task","parent":"S001","type":"action","title":"Answer the security questionnaire by Friday","internal_owner":"Omar Haddad","quote":"Omar will answer the security questionnaire by Friday."}]`

## Test Data

| Field | Value |
|-------|-------|
| Duplicate | Answer the security questionnaire by Friday |
| Original | T002 Answer the security questionnaire |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` | A "To review" banner "Same work as Answer the security questionnaire?" with "Yes" and "No" |
| 2 | Click "No". Wait up to 3 s after an action for the window to redraw before reading. | The banner is gone; the board still shows the card "Answer the security questionnaire by Friday" beside T002 |

## Expected Final State
Both tasks stay; there is nothing to review.

## Teardown
- None possible; use fresh data

## Changes
- Adds one task and rejects its suggestion.

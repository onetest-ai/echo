---
id: TC-011
title: Details → Status changes the state; History says "you set state · undo" and the CLI agrees
priority: critical
type: functional
requirements: [F-WORK-10]
module: work
run: agent
platform: any
tags: [work, details, state]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-011: Details → Status changes the state; History says "you set state · undo" and the CLI agrees

**Module:** Work | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Task | T001 Send Acme the SSO checklist |
| Status | in progress |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T001 | Details shows the Status "● to do" |
| 2 | Click the Status value and choose `in progress` in the select named "Status". Wait up to 3 s after an action for the window to redraw before reading. | The Status reads "● in progress"; the card is in the column "In progress" |
| 3 | Read History | The newest entry reads "you set state" with the link "undo" |
| 4 | Run `echoctl project tasks` | T001 of `acme-rollout` has `"state": "in_progress"` |
| 5 | Click "undo". Wait up to 3 s after an action for the window to redraw before reading. | The Status reads "● to do" again; `echoctl project tasks` shows `"state": "open"` |

## Expected Final State
T001 is to do again.

## Teardown
- If step 5 was not reached, choose Status `to do`

## Changes
- Sets T001 to in progress and undoes it.

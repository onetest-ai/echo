---
id: TC-013
title: Profile → Calls adds and removes a rule; the matching count follows
priority: high
type: functional
requirements: [F-THR-14, F-THR-19]
module: threads
run: agent
platform: any
tags: [threads, profile, rules]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-013: Profile → Calls adds and removes a rule; the matching count follows

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout has no rules

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |
| Rule | Subject contains `planning` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/profile` | The Calls row shows "+ Add" and the line "1 call match · 1 pinned" |
| 2 | Click "+ Add" in the Calls row | A select "Kind of rule" (Meeting series, The client's people, Subject contains, Teams chat id) and an input with the placeholder "a meeting's subject (empty for the client's people)" |
| 3 | Choose `Subject contains`, type `planning` and press Enter. Wait up to 3 s after an action for the window to redraw before reading. | A token `subject “planning”` with a button "Remove"; the line reads "2 calls match · 1 pinned" |
| 4 | Click the tab with `data-tab-id="calls"` | Rows "Acme weekly sync" (added by hand) and "Initech planning" (its subject) |
| 5 | Back on Profile click the "Remove" button of the token `subject “planning”`. Wait up to 3 s after an action for the window to redraw before reading. | The token is gone; the line reads "1 call match · 1 pinned" |

## Expected Final State
Acme rollout has no rules and one call again.

## Teardown
- If step 5 was not reached: `echoctl project rule acme-rollout --remove 1`

## Changes
- Adds then removes a subject rule of `acme-rollout`.

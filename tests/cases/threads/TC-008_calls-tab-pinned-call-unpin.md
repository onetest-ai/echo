---
id: TC-008
title: The Calls tab says why a call belongs; Unpin takes a pinned call out and the CLI can pin it again
priority: high
type: functional
requirements: [F-THR-11]
module: threads
run: agent
platform: any
tags: [threads, calls]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-008: The Calls tab says why a call belongs; Unpin takes a pinned call out and the CLI can pin it again

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |
| Call | Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | One row: "Acme weekly sync", the line "Today, <time> · added by hand" (no "no minutes yet"), and a button "Unpin" |
| 2 | Click the title "Acme weekly sync" | The URL contains `#call/` and ends with `-acme-weekly-sync/minutes` |
| 3 | Navigate back to `{{base_url}}/#project/acme-rollout/calls` and click "Unpin". Wait up to 3 s after an action for the window to redraw before reading. | The row disappears; the text "No calls match yet. Add a rule on Profile — a meeting series, the client's people, a Teams chat — or open a call and add it from its header."; the tab "Calls" shows count 0 |
| 4 | Run `echoctl project show acme-rollout` | `"call_count": 0` and `"calls": []` |
| 5 | Run `echoctl project include acme-rollout <folder of Acme weekly sync>`, then reload and open the Calls tab | The row "Acme weekly sync" with "added by hand" and "Unpin" is back |

## Expected Final State
Acme rollout holds Acme weekly sync again, pinned.

## Teardown
- If step 5 was not reached: `echoctl project include acme-rollout <folder of Acme weekly sync>`

## Changes
- Unpins and re-pins the call Acme weekly sync in `acme-rollout`.

---
id: TC-009
title: A call a rule put in can be taken out by hand; the rules do not put it back
priority: high
type: functional
requirements: [F-THR-11, F-THR-14]
module: threads
run: agent
platform: any
tags: [threads, calls, rules]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-009: A call a rule put in can be taken out by hand; the rules do not put it back

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout has no rules (Profile → Calls shows only "+ Add")

## Test Data

| Field | Value |
|-------|-------|
| Rule | subject contains `planning` |
| Call put in by the rule | Initech planning |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl project rule acme-rollout --subject planning` | JSON whose `rules` holds `{"kind": "subject", "contains": "planning"}` |
| 2 | Navigate to `{{base_url}}/#project/acme-rollout/calls` | Two rows: "Acme weekly sync" with "added by hand" and "Unpin"; "Initech planning" with "its subject" and a button "Take out" |
| 3 | Click "Take out" on the row "Initech planning". Wait up to 3 s after an action for the window to redraw before reading. | The row "Initech planning" disappears; the line "1 call taken out by hand." is shown; the tab "Calls" shows count 1 |
| 4 | Run `echoctl project show acme-rollout` | `"exclusions"` has one entry; `"calls"` lists only Acme weekly sync |
| 5 | Navigate to `{{base_url}}/#project/acme-rollout/profile` | The Calls row reads "1 call match · 1 pinned, 1 taken out by hand" |

## Expected Final State
Initech planning is taken out of Acme rollout although the subject rule still matches it.

## Teardown
- `echoctl project reset acme-rollout <folder of Initech planning>`
- `echoctl project rule acme-rollout --remove 1`

## Changes
- Adds a rule and an exclusion to `acme-rollout`.

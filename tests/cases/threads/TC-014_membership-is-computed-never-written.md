---
id: TC-014
title: Membership is computed from rules and pins; no thread is written into a call
priority: critical
type: functional
requirements: [F-THR-19]
module: threads
run: agent
platform: any
tags: [threads, membership, cli]
---

# TC-014: Membership is computed from rules and pins; no thread is written into a call

**Module:** Threads | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout has no rules

## Test Data

| Field | Value |
|-------|-------|
| Rule | subject contains `planning` |
| Calls | Initech planning, Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl project of <folder of Initech planning>` | JSON listing no thread as the call's (`projects` empty) before the rule |
| 2 | Run `echoctl project rule acme-rollout --subject planning` | Succeeds; `rules` holds the subject rule |
| 3 | Run `echoctl project of <folder of Initech planning>` again | The thread `acme-rollout` is listed for the call (a rule put it in) |
| 4 | Run `echoctl project show acme-rollout` | `"call_count": 2`; the call Initech planning has `"why": "subject"`, Acme weekly sync `"why": "pinned"` |
| 5 | Run `grep -l acme-rollout "$ECHOCTL_HOME"/calls/*/meta.json "$ECHOCTL_HOME"/calls/*/meeting.json` | No file is listed (the thread is not written into any call) |
| 6 | Run `echoctl project rule acme-rollout --remove 1` and then `echoctl project of <folder of Initech planning>` | The thread is no longer listed for the call |

## Expected Final State
The call belongs to the thread only while the rule exists; the call's files never name the thread.

## Teardown
- Step 6 removes the rule; if it was not reached run `echoctl project rule acme-rollout --remove 1`

## Changes
- Adds then removes a subject rule of `acme-rollout`.

## Notes
The exact JSON key of `project of` may differ (`projects` / `suggested`): read the output for the thread id.

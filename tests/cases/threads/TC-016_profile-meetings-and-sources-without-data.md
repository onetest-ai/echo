---
id: TC-016
title: Profile → Meetings and Sources say what is and is not read when there is nothing to read
priority: medium
type: functional
requirements: [F-THR-16, F-THR-17]
module: threads
run: agent
platform: any
tags: [threads, profile]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-016: Profile → Meetings and Sources say what is and is not read when there is nothing to read

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Recall (the Brain) is off and Microsoft 365 is not signed in (the test data's default)

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/profile` | The Profile card is shown |
| 2 | Read the Meetings row | "No meeting series yet. A meeting that recurs in this thread shows here." |
| 3 | Read the Sources row | "The Brain is off, so channels, mail and folders aren't read into it — turn it on in Settings → Brain. Meeting chats, and mail already added here, are read into the work either way." |
| 4 | Read the footer of the card | "Saved as you change it · projects/acme-rollout/ · Delete in ⋯" |

## Expected Final State
Meetings and Sources show their empty explanations.

## Teardown
- None needed

## Changes
- Nothing: read only.

## Notes
The × that unlinks a series, the chat offers and the mail/folder sources need a series, a Teams chat or Recall; they stay untested on this data.

---
id: TC-016
title: Waits for / Holds up link to the other item; Copy link carries echo://task/<thread>/<id>
priority: medium
type: functional
requirements: [F-WORK-14]
module: work
run: agent
platform: any
tags: [work, details, links]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-016: Waits for / Holds up link to the other item; Copy link carries echo://task/<thread>/<id>

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- `echoctl project ledger acme-rollout edit '{"op":"link","node":"T002","depends_on":"T001"}'`

## Test Data

| Field | Value |
|-------|-------|
| Pair | T002 waits for T001 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T002 | Details has the row "Waits for" with the link `T001 Send Acme the SSO checklist` |
| 2 | Read the card T001 on the board | A chip "holds up 1" |
| 3 | Click the card T001 | Details has the row "Holds up" with the link `T002 Answer the security questionnaire` |
| 4 | Click the link `T002 Answer the security questionnaire` | Details shows T002 |
| 5 | Read the "Link" row of T002 | A button "Copy link" whose `data-copy` attribute is `echo://task/acme-rollout/T002` |

## Expected Final State
T002 waits for T001.

## Teardown
- `echoctl project ledger acme-rollout edit '{"op":"unlink","node":"T002","depends_on":"T001"}'`

## Changes
- Links then unlinks T002 to T001.

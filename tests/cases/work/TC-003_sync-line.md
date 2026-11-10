---
id: TC-003
title: The sync line says Not synced yet and how many calls wait and items wait for review
priority: medium
type: functional
requirements: [F-WORK-03]
module: work
run: agent
platform: any
tags: [work, sync]
auto: [tests/auto/specs/S-06.spec.mjs]
---

# TC-003: The sync line says Not synced yet and how many calls wait and items wait for review

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Run `echoctl project ledger acme-rollout sync` (writes the call as evidence; starts no Claude run)

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` | The bar's right side reads "Not synced yet · 1 call to read" and a button "Sync now" (do not press it) |
| 2 | Run `echoctl project ledger acme-rollout status` | The output is `null` (the project has never synced) |

## Expected Final State
The sync line reads "Not synced yet · 1 call to read".

## Teardown
- None needed (the evidence file stays; it is harmless)

## Changes
- Writes the evidence file of the call in `acme-rollout`'s ledger.

## Notes
"Synced HH:MM", "Reading…" and "Sync failed" need a Claude Code run and stay manual. The "N to review" part is checked in TC-008.

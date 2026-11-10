---
id: TC-007
title: A client with threads says to move or delete them first, in the window and in the CLI
priority: medium
type: functional
requirements: [F-CLIENT-06]
module: clients
run: agent
platform: any
tags: [clients, remove]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-007: A client with threads says to move or delete them first, in the window and in the CLI

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Client | Acme |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#client/acme/profile` | The last row reads "A client with threads stays: move or delete them first."; there is no button "Remove this client…" |
| 2 | Run `echoctl client remove acme` | Exit with the error "acme still has projects: move or delete them first" |

## Expected Final State
Acme still exists.

## Teardown
- None needed

## Changes
- Nothing: read only.

---
id: TC-009
title: Organisations that are not clients are kept with echoctl org add/set/remove; an account with threads cannot be removed
priority: medium
type: functional
requirements: [F-CLIENT-08]
module: clients
run: agent
platform: any
tags: [clients, organisations, cli]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-009: Organisations that are not clients are kept with echoctl org add/set/remove; an account with threads cannot be removed

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- No organisation `contoso-partners` exists

## Test Data

| Field | Value |
|-------|-------|
| Organisation | Contoso Partners, domain contoso.example |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl org add "Contoso Partners" --domains contoso.example` | JSON with `"account": false`, `"id": "contoso-partners"`, `"domains": ["contoso.example"]` |
| 2 | Run `echoctl org list` | `contoso-partners` is listed with `"account": false`; Acme is listed without an `account` field (an account is the default and is left out) |
| 3 | Navigate to `{{base_url}}/#projects` | No group "CONTOSO PARTNERS" (it is not a client) |
| 4 | Navigate to `{{base_url}}/#project/acme-rollout/profile`, click "+ Organisation" | The select lists "Contoso Partners · contoso.example" and "Globex · globex.example" |
| 5 | Run `echoctl org remove acme` | Error "acme still has projects: move or delete them first" |
| 6 | Run `echoctl org remove contoso-partners` | Prints "contoso-partners removed" |

## Expected Final State
The organisation is removed; Acme is untouched.

## Teardown
- Step 6 removes it; press Escape in the open select

## Changes
- Creates and removes `contoso-partners`.

---
id: TC-008
title: The user's company shows as "<name> · your company" and its people are colleagues
priority: medium
type: functional
requirements: [F-CLIENT-07]
module: clients
run: agent
platform: any
tags: [clients, internal]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-008: The user's company shows as "<name> · your company" and its people are colleagues

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Client | Initech (`initech`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#projects` | The group header reads "INITECH · YOUR COMPANY" (the others read "ACME", "GLOBEX") |
| 2 | Click that header (`[data-pj="client"][data-id="initech"]`) | The URL ends with `#client/initech/overview`; the tag reads "your company"; facts "0 threads · initech.example" |
| 3 | Click the tab "Overview" | "No threads for this client yet — add one on Profile." |
| 4 | Click the tab "Profile" | Kind: a switch named "Your own company" that is on, with the text "Your own company (internal work)"; the hint "Its people are your colleagues: they never count as a client's people." |

## Expected Final State
Initech is shown as the user's own company.

## Teardown
- None needed

## Changes
- Nothing: read only.

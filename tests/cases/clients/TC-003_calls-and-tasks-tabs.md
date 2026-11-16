---
id: TC-003
title: Client Calls lists the calls of its threads; Tasks shows the piles with Others folded
priority: medium
type: functional
requirements: [F-CLIENT-03, F-CLIENT-04]
module: clients
run: agent
platform: any
tags: [clients, calls, tasks]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-003: Client Calls lists the calls of its threads; Tasks shows the piles with Others folded

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
| 1 | Navigate to `{{base_url}}/#client/acme/calls` | One row: "Acme weekly sync" with today's time, no "no minutes yet", and the chip "Acme rollout"; no row "Initech planning" |
| 2 | Click the tab "Tasks" | Piles "I owe" 1 (card "Send Acme the SSO checklist"), "Owed to me" 0, "Waiting for others" 0, "Nobody named" 0; a folded line "Others · 1" |
| 3 | Click "Others · 1" (`[data-tk="more"][data-pile="client-others"]`: on a client's Tasks tab the fold is `client-others`, on `#tasks` it is `others`) | The card "Answer the security questionnaire" with "Omar Haddad" |
| 4 | Click the card "Send Acme the SSO checklist" | The right panel shows Details of T001 of Acme rollout |

## Expected Final State
The Tasks tab shows the piles of Acme; Details of T001 is open.

## Teardown
- Click "Others · 1" (`[data-pile="client-others"]`) again to fold it

## Changes
- Nothing: read only.

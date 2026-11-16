---
id: TC-004
title: Client Profile adds and removes another name and a mail domain
priority: high
type: functional
requirements: [F-CLIENT-05]
module: clients
run: agent
platform: any
tags: [clients, profile]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-004: Client Profile adds and removes another name and a mail domain

**Module:** Clients | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Client | Globex |
| Other name | GBX |
| Domain | globex.net |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#client/globex/profile` | Rows Name "Globex", Other names ("+ Add"), Mail domains "globex.example" with ×, Kind "A client, partner or vendor", Logo, Threads ("Globex pilot", "+ New thread"); the hint "A call is this client's when half of its outside people come from these domains." |
| 2 | Click "+ Add" in Other names, type `GBX`, press Enter. Wait up to 3 s after an action for the window to redraw before reading. | A token "GBX" with a button "Remove" |
| 3 | Click "+ Add" in Mail domains, type `globex.net`, press Enter. Wait up to 3 s after an action for the window to redraw before reading. | Tokens "globex.example" and "globex.net"; `echoctl client list` shows both domains for `globex` |
| 4 | Click "Remove" on "GBX" and on "globex.net". Wait up to 3 s after an action for the window to redraw before reading. | Both tokens are gone; Mail domains shows only "globex.example" |

## Expected Final State
Globex has only its original domain and no other names.

## Teardown
- Step 4 removes both; if not reached, remove them

## Changes
- Adds then removes an alias and a domain of Globex.

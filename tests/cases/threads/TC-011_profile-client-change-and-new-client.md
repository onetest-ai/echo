---
id: TC-011
title: Profile → Client changes the client, makes a new one inline, or clears it
priority: high
type: functional
requirements: [F-THR-13]
module: threads
run: agent
platform: any
tags: [threads, profile, clients]
---

# TC-011: Profile → Client changes the client, makes a new one inline, or clears it

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- No client `northwind` exists

## Test Data

| Field | Value |
|-------|-------|
| Thread | Globex pilot |
| New client | Northwind, domain `northwind.example` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/globex-pilot/profile` | The Client row shows a link "Globex", the text "its domains: globex.example" and a select labelled "Client" |
| 2 | In the "Client" select choose `+ New client…` | Two inputs ("Client name", "Mail domains") with buttons "Add" and "Cancel" appear |
| 3 | Type `Northwind` and `northwind.example`, click "Add". Wait up to 3 s after an action for the window to redraw before reading. | The Client row shows "Northwind" and "its domains: northwind.example"; the header tag reads "Northwind"; the left list has a new group "NORTHWIND" with the row "Globex pilot" |
| 4 | In the "Client" select choose `No client` | The Client row reads "No client — internal work, or not set"; the header has no client tag; the row sits under "No client" |
| 5 | In the "Client" select choose `Globex` | The Client row shows "Globex" again |

## Expected Final State
Globex pilot is back with its client Globex; a client Northwind exists.

## Teardown
- `echoctl project set globex-pilot --client globex`
- `echoctl client remove northwind`

## Changes
- Creates client `northwind`; changes the client of `globex-pilot` three times.

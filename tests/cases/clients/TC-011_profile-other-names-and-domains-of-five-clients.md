---
id: TC-011
title: A client's Profile lists its other names and mail domains: none, one, several, and the user's company
priority: medium
type: functional
requirements: [F-CLIENT-05, F-CLIENT-07]
module: clients
run: agent
platform: any
tags: [clients, profile, aliases, domains, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-011: A client's Profile lists its other names and mail domains: none, one, several, and the user's company

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No client has been edited since the data was built

## Test Data

| Client | Other names (tokens) | Mail domains (tokens) | Kind line |
|---|---|---|---|
| Acme | ACME Corp, Acme GmbH | acme.example | A client, partner or vendor |
| Globex | none | globex.example, globex-labs.example | A client, partner or vendor |
| Umbrella | none | umbrella.example | A client, partner or vendor |
| Tyrell | none | tyrell.example | A client, partner or vendor |
| Initech | none | initech.example | Your own company (internal work) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | For each client of the table navigate to `{{base_url}}/#client/<id>/profile` (ids acme, globex, umbrella, tyrell, initech) | The Profile shows the rows Name, Other names, Mail domains, Kind, Logo and Threads |
| 2 | Read "Other names" and "Mail domains" | The tokens of the row, each with a "×", then "+ Add"; a client with no other name has only "+ Add" |
| 3 | Read the line under "Mail domains" | For an outside client: "A call is this client's when half of its outside people come from these domains."; for Initech: "Its people are your colleagues: they never count as a client's people." |
| 4 | Read the row "Kind" | The switch "Your own company" is off with the text of the Kind line, and on only for Initech |
| 5 | Run `echoctl client list` | `aliases` and `domains` of each client are those of the table |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Read only; `TC-004` adds and removes a name and a domain. Covers org.domains (none, one, several; aliases none, some) and org.internal.

---
id: TC-027
title: A thread has no counterpart, one or several, and a counterpart that is not a client has no group in the list
priority: medium
type: functional
requirements: [F-THR-13, F-CLIENT-08]
module: threads
run: agent
platform: any
tags: [threads, profile, org]
---

# TC-027: A thread has no counterpart, one or several, and a counterpart that is not a client has no group in the list

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No organisation has been linked or unlinked since the data was built

## Test Data

| Thread | Counterparts (Organisations row) | Domains in the tooltip |
|---|---|---|
| Acme rollout | Vandelay, Northwind Analysts | vandelay.example, northwind.example |
| Globex pilot | Vandelay | vandelay.example |
| Acme renewal | none | - |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/profile` and read the row "Organisations" | Two tokens, "Vandelay" and "Northwind Analysts", each with "×", then "+ Organisation"; hovering a token gives its domain |
| 2 | Navigate to `{{base_url}}/#project/globex-pilot/profile` and read the row "Organisations" | One token, "Vandelay", then "+ Organisation" |
| 3 | Navigate to `{{base_url}}/#project/acme-renewal/profile` and read the row "Organisations" | No token, only "+ Organisation" |
| 4 | Navigate to `{{base_url}}/#projects` and look for groups named Vandelay or Northwind | None: an organisation that is not a client has no group in the list |
| 5 | Run `echoctl org list` and read `vandelay` and `northwind-analysts` | Both have `"account": false` |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Extends `TC-012` (linking one organisation) to the three cases of thread.orgs.

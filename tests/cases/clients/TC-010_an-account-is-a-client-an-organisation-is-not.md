---
id: TC-010
title: An account is a client with a page and a group; an organisation that is not a client has neither
priority: medium
type: functional
requirements: [F-CLIENT-08, F-CLIENT-01, F-THR-13]
module: clients
run: agent
platform: any
tags: [clients, org, account, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-010: An account is a client with a page and a group; an organisation that is not a client has neither

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No client or organisation has been added or removed since the data was built

## Test Data

| Name | Id | Kind | Domains | In the thread list | Page `#client/<id>` |
|---|---|---|---|---|---|
| Acme | acme | client (account) | acme.example | group "Acme" | opens |
| Globex | globex | client (account) | globex.example, globex-labs.example | group "Globex" | opens |
| Vandelay | vandelay | organisation, not a client (counterpart of Globex pilot) | vandelay.example | no group | "This client is gone" |
| Northwind Analysts | northwind-analysts | organisation, not a client (counterpart of Acme rollout) | northwind.example | no group | "This client is gone" |
| Stark Advisory | stark-advisory | organisation, not a client, no domain, no thread | - | no group | "This client is gone" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `echoctl client list` | Six entries: initech, acme, globex, umbrella, tyrell, duff; none of vandelay, northwind-analysts, stark-advisory |
| 2 | Run `echoctl org list` | Nine entries: the six above and vandelay, northwind-analysts and stark-advisory, the last three with `"account": false`; stark-advisory has `"domains": []` |
| 3 | Navigate to `{{base_url}}/#projects` and read the groups | Groups Acme, Globex, Umbrella, Tyrell, Duff, "Initech · your company", "No client"; no group named Vandelay, Northwind Analysts or Stark Advisory |
| 4 | Navigate to `{{base_url}}/#client/acme/overview` | The heading "Acme" with the tag "client" and the line "3 threads · acme.example" |
| 5 | Navigate to `{{base_url}}/#client/vandelay/overview` | The page says "This client is gone" with the link "Back to the threads"; the same for `#client/stark-advisory/overview` |
| 6 | Navigate to `{{base_url}}/#project/globex-pilot/profile` and read the row "Organisations" | The token "Vandelay" (a counterpart) is there although it has no client page |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- Covers org.account (client, organisation that is not a client) by org.domains (none, one, several).
- An organisation that exists but is not a client gets the message "This client is gone" (the page only knows accounts); the word "gone" is wrong for it. Verify at run time.
- Extends `TC-009`, which makes an organisation with the CLI.

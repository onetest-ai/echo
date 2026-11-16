---
id: TC-015
title: client and org commands list, add, change and remove clients and organisations as the window shows them
priority: medium
type: functional
requirements: [F-CLI-10]
module: cli
run: agent
platform: any
data: standard
tags: [cli, clients, organisations]
auto: [tests/auto/specs/S-28.spec.mjs]
---

# TC-015: client and org commands list, add, change and remove clients and organisations as the window shows them

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Reload the page fully (`{{base_url}}/`) after a CLI change before reading the window

## Test Data

| Field | Value |
|---|---|
| Clients | initech (internal), acme, globex (globex.example, globex-labs.example), umbrella, tyrell, duff |
| Organisations that aren't clients | vandelay, northwind-analysts, stark-advisory (`account: false`) |
| New client | Hooli (`hooli`), hooli.example — made-up |
| New organisation | Pied Piper (`pied-piper`), piedpiper.example, counterpart of Umbrella proposal — made-up |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl client list` | A JSON array of 8: `initech` (`internal` true), `acme`, `globex` (`domains` ["globex.example","globex-labs.example"]), `umbrella`, `tyrell`, `duff`, `wonka`, `cyberdyne` |
| 2 | Run `target/debug/echoctl org list` | The 8 clients (no `account` key) and `vandelay`, `northwind-analysts`, `stark-advisory`, each with `account: false` |
| 3 | Run `target/debug/echoctl client add Hooli --domains hooli.example` | `{"aliases":[],"domains":["hooli.example"],"id":"hooli","internal":false,"name":"Hooli"}` (pretty-printed) |
| 4 | Run `target/debug/echoctl client set hooli --name "Hooli XYZ" --domains hooli.example,hooli-xyz.example` | `name` "Hooli XYZ", `domains` ["hooli.example","hooli-xyz.example"], `id` still "hooli" |
| 5 | Reload, navigate to `{{base_url}}/#client/hooli/overview` | The thread list has a group "HOOLI XYZ" (between CYBERDYNE and INITECH · YOUR COMPANY); the page reads "Hooli XYZ", "client", "0 threads · hooli.example, hooli-xyz.example" and "No threads for this client yet — add one on Profile." |
| 6 | Run `target/debug/echoctl org add "Pied Piper" --domains piedpiper.example --thread umbrella-proposal` | `id` "pied-piper", `account` false, `domains` ["piedpiper.example"] |
| 7 | Reload, navigate to `{{base_url}}/#project/umbrella-proposal/profile` | Under "Organisations" the chip "Pied Piper" with "×" |
| 8 | Run `target/debug/echoctl org set pied-piper --unlink umbrella-proposal`, reload the Profile | "Pied Piper" is no longer under "Organisations" |
| 9 | Run `target/debug/echoctl client remove acme` | Exit code 1; "Error: acme still has projects: move or delete them first" |
| 10 | Run `target/debug/echoctl org remove pied-piper` and `target/debug/echoctl client remove hooli` | "pied-piper removed" and "hooli removed" |
| 11 | Run `target/debug/echoctl client list` and reload `{{base_url}}/#projects` | The 8 clients of step 1 again; no "HOOLI XYZ" group in the thread list |

## Expected Final State
The clients and organisations are as the data built them.

## Teardown
- Step 10 removes what the case added

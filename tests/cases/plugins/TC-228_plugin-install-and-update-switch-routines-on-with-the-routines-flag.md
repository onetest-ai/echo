---
id: TC-228
title: echoctl plugin install / update --routines switch on the routines named (or all the sheet offers); a routine the sheet doesn't offer is refused before anything is installed
priority: high
type: functional
requirements: [F-PLUG-136, F-PLUG-76]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, cli, install, update]
---

# TC-228: `plugin install` and `plugin update` switch routines on with `--routines`

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served (any set; the case uses only the CLI): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A copy of the test marketplace (its routines-demo gets updated): `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `$E plugin marketplace add "$C" --name test-plugins`

**Changes:** the installed plugins, `<config dir>/jobs.json`, `$C/plugins/routines-demo/`

## Test Data

| Plugin | Routines demo (`routines-demo`) 0.1.0: routines `mail-knowledge` "Mail → Knowledge" and `vault-health` "Vault health"; 0.2.0 (`$C/next/routines-demo`): the same two, unchanged, and a third, `weekly-digest` "Weekly digest" |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install routines-demo --from test-plugins --routines nope --yes` | The sheet, ending "Its routines stay off unless you switch them on: --routines mail-knowledge,vault-health (or all)"; then `Error: Routines demo 0.1.0 has no routine “nope” to switch on here — its routines on this sheet: mail-knowledge, vault-health`, exit 1; `$E plugin list`: no `routines-demo` under `installed` |
| 2 | `$E plugin install routines-demo --from test-plugins --routines vault-health --yes` | `Installed Routines demo 0.1.0`; `$E jobs routines`: `routines-demo.vault-health` `enabled` true, `routines-demo.mail-knowledge` `enabled` false |
| 3 | `cp -R "$C/next/routines-demo/." "$C/plugins/routines-demo/"`; `$E plugin marketplace update test-plugins`; `$E plugin update routines-demo --routines mail-knowledge --yes` | The sheet marks only “Weekly digest”'s line `(new)` and ends "Its routines stay off unless you switch them on: --routines weekly-digest (or all)"; `Error: Routines demo 0.2.0 has no routine “mail-knowledge” to switch on here — its routines on this sheet: weekly-digest`, exit 1; `$E plugin list`: routines-demo still 0.1.0 |
| 4 | `$E plugin update --routines all --yes` | `Error: --routines needs the plugin's id`, exit 1 |
| 5 | `$E plugin update routines-demo --routines all --yes` | `Updated Routines demo to 0.2.0`; `$E jobs routines`: `weekly-digest` and `vault-health` `enabled` true, `mail-knowledge` still `enabled` false — `all` is every routine the update's sheet offers, a routine it keeps stays as it was |

## Expected Final State
Routines demo 0.2.0 installed: Vault health and Weekly digest on, Mail → Knowledge off.

## Teardown
- `$E plugin remove routines-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cp`, `mktemp` and `rm`.
- Without `--routines` the routines stay off (TC-110, TC-111). The flag, as every change to plugins, is refused inside a run Echo started (`cli::plugin::refuse_in_run`).

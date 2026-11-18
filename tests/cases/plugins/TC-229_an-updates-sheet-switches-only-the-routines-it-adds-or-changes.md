---
id: TC-229
title: An update's sheet shows a switch only for the routine it adds; the routines it keeps unchanged have none and stay as the user had them
priority: high
type: functional
requirements: [F-PLUG-135]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, sheet, update]
---

# TC-229: An update's sheet switches only the routines it adds or changes

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A copy of the test marketplace: `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `$E plugin marketplace add "$C" --name test-plugins`; `$E plugin install routines-demo --from test-plugins --routines vault-health --yes`
- Its update waiting: `cp -R "$C/next/routines-demo/." "$C/plugins/routines-demo/"`; `$E plugin marketplace update test-plugins`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** the installed plugins, `<config dir>/jobs.json`, `$C/plugins/routines-demo/`

## Test Data

| Plugin | Routines demo 0.1.0 installed, Vault health on, Mail → Knowledge off; 0.2.0 waiting: the same two unchanged and a third, `weekly-digest` "Weekly digest" (every 1440 min, paused at night, "Sums up the week's new notes in one page") |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins`; click the tab "Installed"; click the row "Routines demo"; click "Update to 0.2.0" | The sheet "Routines demo 0.2.0 asks for more" has no "Has a routine “Weekly digest” …" line in "It will be able to"; under "Routines — off unless you switch them on" one switch, "Weekly digest on", with "Every day · paused at night", "Sums up the week's new notes in one page" and under it the small muted line "acts on its own as you set routines · may change your Echo data and send as you · prompt <hash>", not checked — no switch for Mail → Knowledge or Vault health |
| 2 | Click the switch "Weekly digest on"; click "Update to 0.2.0"; wait 3 s | `$E jobs routines`: `routines-demo.weekly-digest` `enabled` true, `vault-health` still true, `mail-knowledge` still false; the detail says 0.2.0 |

## Expected Final State
Routines demo 0.2.0 installed: Weekly digest and Vault health on, Mail → Knowledge off.

## Teardown
- `$E plugin remove routines-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `cp`, `mktemp` and `rm`.
- A routine an update changes is offered too, its switch as the user has it (on stays on unless switched off there): the Rust test `plugins::routines::the_sheet_switches_on_only_the_routines_it_offers_and_an_unknown_one_is_refused_before_installing`; that it keeps on when taken without a switch: TC-110.

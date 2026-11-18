---
id: TC-227
title: A plugin's install sheet has a switch per routine, each off as it opens; the one the user switches on is on right after the install, the other stays off
priority: high
type: functional
requirements: [F-PLUG-135, F-PLUG-76]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, sheet, install]
---

# TC-227: A plugin's routines are switched on from its install sheet

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The test marketplace: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** the installed plugins, `<config dir>/jobs.json`

## Test Data

| Plugin | Routines demo (`routines-demo`) 0.1.0: `uses.claude`, a window half, two routines — `mail-knowledge` "Mail → Knowledge" (every 60 min, paused at night, "Files trusted mail into your vault") and `vault-health` "Vault health" (every 1440 min, not paused at night, "Checks links and stale pages") |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins`; click the tab "Discover"; on the card "Routines demo" click "Install" | The sheet "Install Routines demo 0.1.0?" lists under "It will be able to" a line per routine, each "Has a routine “<name>”, off unless you switch it on here or later in Settings → Plugins: then every <N> min Echo runs Claude Code on your default AI profile with the prompt it ships, …". Under them the heading "Routines — off unless you switch them on" and two switches: "Mail → Knowledge on" with "Every 60 min · paused at night" and "Files trusted mail into your vault", and "Vault health on" with "Every 1440 min" and "Checks links and stale pages" — both not checked |
| 2 | Click the switch "Vault health on" | It is checked; "Mail → Knowledge on" still not checked; nothing is installed yet (`$E plugin list`: no `routines-demo` under `installed`) |
| 3 | Click "Install"; wait 3 s | The sheet closes; `$E jobs routines`: `routines-demo.vault-health` `enabled` true (its `on` true), `routines-demo.mail-knowledge` `enabled` false (its `on` false) |
| 4 | Click the tab "Installed"; click the row "Routines demo" | Its detail's "Its routines": "Vault health" with "Every 1440 min" and its switch checked; "Mail → Knowledge" with "Every 60 min · paused at night · off" and its switch not checked |

## Expected Final State
Routines demo 0.1.0 installed and on: Vault health on, Mail → Knowledge off.

## Teardown
- `$E plugin remove routines-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A click a script makes on a sheet's switch moves nothing, and only the switches the user moved are sent with Install: the node test `plugin-routines.test.mjs`. That an install switches on only the routines its sheet offered: the Rust test `plugins::routines::the_sheet_switches_on_only_the_routines_it_offers_and_an_unknown_one_is_refused_before_installing`.

---
id: TC-230
title: A plugin's own setup opens Echo's sheet of its routines on the user's click (ctx.echo.routines.offer); the user switches them there and it resolves to their states; an id it hasn't is refused
priority: high
type: functional
requirements: [F-PLUG-137, F-PLUG-76]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, routines, client, sheet]
---

# TC-230: A plugin's setup offers its routines on Echo's sheet

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built right before the run, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install routines-demo --routines vault-health --yes`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** `<config dir>/jobs.json`

## Test Data

| Plugin | Routines demo (`routines-demo`): its Settings section "Routines demo" has buttons "All routines", "Only Vault health" and "A routine it hasn't", each calling `ctx.echo.routines.offer` on the click, and a line showing what it resolved to ("Nothing chosen yet" at first). Its routines: Mail → Knowledge (off), Vault health (on) |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/routines-demo:main`; click "All routines" | A dialog "Routines demo’s routines": "Each runs Claude Code on its own while it is on, as its sheet in Settings → Plugins says; you can switch it off there any time.", the heading "Routines — off unless you switch them on", the switches "Mail → Knowledge on" (not checked, "Every hour · paused at night", "Files trusted mail into your vault") and "Vault health on" (checked, "Every day"), and "Done" |
| 2 | Click the switch "Mail → Knowledge on"; wait 1 s | It is checked; `$E jobs routines`: `routines-demo.mail-knowledge` `enabled` true |
| 3 | Click "Done"; wait 1 s | The dialog closes; the plugin's line reads "mail-knowledge on, vault-health on" |
| 4 | Click "Only Vault health" | The dialog shows one switch, "Vault health on", checked |
| 5 | Press Escape; wait 1 s | The dialog closes; the line reads "vault-health on" |
| 6 | Click "A routine it hasn't" | No dialog opens; the line reads "Refused: the plugin has no routine “nope”" |

## Expected Final State
Both of Routines demo's routines on.

## Teardown
- `$E plugin remove routines-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Without the user's click `offer` rejects before anything (`copyAllowed`), and a click a script makes on the sheet's switch moves nothing: the node tests `plugin-routines.test.mjs` and `copy-guard.test.mjs`. The guard is the page's transient activation: a client half loaded a moment after a click (an install, a tab) still has it — a guard against accidents, not a boundary; the switches stay the user's.
- `offer` needs no capability: the routines are on the plugin's sheet already.

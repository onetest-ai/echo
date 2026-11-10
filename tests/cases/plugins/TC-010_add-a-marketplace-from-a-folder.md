---
id: TC-010
title: Add a marketplace from a folder lists it with its plugin count
priority: medium
type: functional
requirements: [F-PLUG-07]
module: plugins
run: agent
platform: any
tags: [plugins, marketplaces]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-010: Add a marketplace from a folder lists it with its plugin count

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the marketplace folder `$M` below. In S-12: written once by the prelude (run only the `M=…` line here). On its own: the lines below.
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`

**Changes:** `<config dir>/plugins/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins`, click the tab "Marketplaces", type the path of `$M` (print it with `echo "$M"`) in "Marketplace", type `acme-local` in "Name" and click "Add" | After a moment the tab reads "Marketplaces" with the count `2`; a row "acme-local" with the path of the folder (`$(cd "$M" && pwd -P)`; `/var` and `/private/var` are the same folder on macOS) and "1 plugin · read today" |
| 2 | Run `echoctl plugin list` | `markets` holds `onetest` and `acme-local` (`"count": 1`, `"built_in": false`) |
| 3 | Click "Read again" on the row "acme-local" | The row still reads "1 plugin · read today" |

## Expected Final State
The local marketplace is added and read.

## Teardown
- `echoctl plugin marketplace remove acme-local`

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

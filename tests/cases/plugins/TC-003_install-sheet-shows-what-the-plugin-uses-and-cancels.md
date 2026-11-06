---
id: TC-003
title: Install opens a sheet with what the plugin may do, and Cancel installs nothing
priority: critical
type: functional
requirements: [F-PLUG-03]
module: plugins
run: agent
platform: any
tags: [plugins, install]
---

# TC-003: Install opens a sheet with what the plugin may do, and Cancel installs nothing

**Module:** Plugins | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`

**Changes:** `<config dir>/plugins/` (the marketplace)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Add the marketplace: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) | Prints `Added acme-local: 1 plugins` |
| 2 | Navigate to `{{base_url}}/#settings/plugins`, click the tab "Discover", type `acme` in "Search plugins" and click "Install" on the card "Acme notes" | A dialog opens, titled "Install Acme notes 0.1.0?", with "from acme-local · commit local-" |
| 3 | Read the dialog | A warning "Listed by acme-local — Echo hasn’t reviewed it. Install it only if you trust who made it."; the heading "It will be able to" with "Uses nothing of Echo's."; the buttons "Cancel" and "Install" |
| 4 | Click "Cancel" | The dialog closes; the card still shows "Install" |
| 5 | Run `echoctl plugin list` | `"installed": []` |

## Expected Final State
The sheet was read and cancelled; nothing was installed.

## Teardown
- `echoctl plugin marketplace remove acme-local`

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

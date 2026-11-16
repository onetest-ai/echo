---
id: TC-006
title: The plugin's switch turns it off and on
priority: high
type: functional
requirements: [F-PLUG-05]
module: plugins
run: agent
platform: any
tags: [plugins, enable]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-006: The plugin's switch turns it off and on

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** Acme notes installed from `acme-local`. In S-12: the prelude writes the folder, stage 22 (plugins/TC-010) adds the marketplace and stage 26 (plugins/TC-004) installs the plugin through the sheet, so the lines below are skipped there. On its own: the lines below.
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`
- The plugin is installed: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) then `echoctl plugin install acme-notes --from acme-local --yes` (prints `Installed Acme notes 0.1.0`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/plugins` (tab "Installed") | The row "Acme notes" is on; the detail's switch "Acme notes on" is on |
| 2 | Click the switch "Acme notes on" in the detail | It is off; the row looks dimmed |
| 3 | Run `echoctl plugin list` | The entry `acme-notes` has `"enabled": false` |
| 4 | Click the switch again | It is on; the row is not dimmed |
| 5 | Run `echoctl plugin list` | The entry has `"enabled": true` |

## Expected Final State
The plugin is enabled, as it began.

## Teardown
- `echoctl plugin remove acme-notes --delete-data`, then `echoctl plugin marketplace remove acme-local`.

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.
- What the window part adds or takes away (buttons, panels, ⌘K entries) needs a plugin with code and stays manual (F-PLUG-04).

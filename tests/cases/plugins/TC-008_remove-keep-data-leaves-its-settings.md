---
id: TC-008
title: Remove, keep data removes the plugin and leaves its settings
priority: high
type: functional
requirements: [F-PLUG-05]
module: plugins
run: agent
platform: any
tags: [plugins, remove]
---

# TC-008: Remove, keep data removes the plugin and leaves its settings

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`
- The plugin is installed: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) then `echoctl plugin install acme-notes --from acme-local --yes` (prints `Installed Acme notes 0.1.0`)

**Changes:** `<data>/.echo/plugins/acme-notes/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Save a setting so there is data: `echoctl plugin set acme-notes greeting "good morning"` | Prints the setting with `"value": "good morning"` |
| 2 | Navigate to `{{base_url}}/#settings/plugins` (tab "Installed"), click "Remove…", then "Remove, keep data" | The tab "Installed" has no count and says "No plugins yet — Discover lists what your marketplaces offer." |
| 3 | Run `echoctl plugin list` | `"installed": []` |
| 4 | Run `cat "$ECHOCTL_HOME/.echo/plugins/acme-notes/settings.json"` | The file still holds `good morning` |

## Expected Final State
The plugin is gone; its settings stayed.

## Teardown
- `echoctl plugin marketplace remove acme-local`; `rm -r "$ECHOCTL_HOME/.echo/plugins/acme-notes"` (the settings left on purpose).

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

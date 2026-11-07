---
id: TC-005
title: A plugin's own settings are saved when changed
priority: high
type: functional
requirements: [F-PLUG-05]
module: plugins
run: agent
platform: any
tags: [plugins, settings]
---

# TC-005: A plugin's own settings are saved when changed

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

**Changes:** `<data>/.echo/plugins/acme-notes/settings.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Read its settings: `echoctl plugin set acme-notes greeting` and `echoctl plugin set acme-notes loud` | `value` is `hello` for greeting and `false` for loud |
| 2 | Navigate to `{{base_url}}/#settings/plugins` (tab "Installed"); read "Its settings" | "Greeting" holds `hello`; the switch "Loud" is off |
| 3 | Replace "Greeting" with `good morning` and press Tab | The box holds `good morning` |
| 4 | Click the switch "Loud" | It is on |
| 5 | Wait 1 s, run `echoctl plugin set acme-notes greeting` and `echoctl plugin set acme-notes loud` | `value` is `good morning` and `true` |

## Expected Final State
Both settings were saved by the plugin's own store.

## Teardown
- `echoctl plugin remove acme-notes --delete-data`, then `echoctl plugin marketplace remove acme-local`.

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

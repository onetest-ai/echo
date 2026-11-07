---
id: TC-012
title: ⋯ Remove takes an added marketplace away
priority: medium
type: functional
requirements: [F-PLUG-07]
module: plugins
run: agent
platform: any
tags: [plugins, marketplaces]
---

# TC-012: ⋯ Remove takes an added marketplace away

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Add the marketplace: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) | Prints `Added acme-local: 1 plugins` |
| 2 | Navigate to `{{base_url}}/#settings/plugins`, click the tab "Marketplaces" and click "⋯" on the row "acme-local" | The menu has "Hide its plugins from Discover" and "Remove — installed plugins stay, without updates" |
| 3 | Click "Remove — installed plugins stay, without updates" | The row "acme-local" is gone; the tab reads "Marketplaces" with the count `1` |
| 4 | Run `echoctl plugin list` | `markets` holds only `onetest` |

## Expected Final State
Only the built-in marketplace remains.

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

---
id: TC-011
title: ⋯ hides a marketplace's plugins from Discover and shows them again
priority: medium
type: functional
requirements: [F-PLUG-07, F-PLUG-02]
module: plugins
run: agent
platform: any
tags: [plugins, marketplaces]
---

# TC-011: ⋯ hides a marketplace's plugins from Discover and shows them again

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the marketplace `acme-local` from the folder `$M` below. In S-12: the prelude writes the folder and stage 22 (plugins/TC-010) adds it in the window, so the lines below and step 1 are skipped there. On its own: the lines below and step 1.
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`

**Changes:** `<config dir>/plugins/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Add the marketplace: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) | Prints `Added acme-local: 1 plugins` |
| 2 | Navigate to `{{base_url}}/#settings/plugins`, click the tab "Marketplaces", click "⋯" on the row "acme-local" and click "Hide its plugins from Discover" | The row "acme-local" gets the tag "hidden" |
| 3 | Click the tab "Discover" and type `acme` in "Search plugins" | No card "Acme notes" |
| 4 | Click the tab "Marketplaces", click "⋯" on "acme-local" and click "Show its plugins in Discover" | The tag "hidden" is gone |
| 5 | Click the tab "Discover" | With `acme` in the search the card "Acme notes" shows |

## Expected Final State
The marketplace is shown again.

## Teardown
- `echoctl plugin marketplace remove acme-local`

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

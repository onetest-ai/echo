---
id: TC-002
title: Discover filters by category chip and search, each card naming its marketplace
priority: medium
type: functional
requirements: [F-PLUG-02]
module: plugins
run: agent
platform: any
tags: [plugins, discover]
---

# TC-002: Discover filters by category chip and search, each card naming its marketplace

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

**Changes:** `<config dir>/plugins/` (the marketplaces)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Add the made-up marketplace: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) | Prints `Added acme-local: 1 plugins` |
| 2 | Navigate to `{{base_url}}/#settings/plugins` and click the tab "Discover" | A row of chips starting with "All" and including "Automation"; a box "Search plugins" |
| 3 | Type `acme` in "Search plugins" | One card "Acme notes": category "Automation", the tag "acme-local", the text "A made-up plugin for tests", the version `0.1.0` and a button "Install" (not pressed) |
| 4 | Clear the box and click the chip "Automation" | The card "Acme notes" shows |
| 5 | Click the chip "All", type `zzzz` in "Search plugins" | The text "Nothing matches." shows |

## Expected Final State
Discover filtered the local plugin by search and by chip.

## Teardown
- `echoctl plugin marketplace remove acme-local`

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.
- The built-in marketplace's own plugins come from the network; this case asserts only the local card.

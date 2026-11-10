---
id: TC-004
title: Install through the sheet adds the plugin to Installed
priority: high
type: functional
requirements: [F-PLUG-03, F-PLUG-05]
module: plugins
run: agent
platform: any
tags: [plugins, install]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-004: Install through the sheet adds the plugin to Installed

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)
- **Shared asset:** the marketplace `acme-local` from the folder `$M` below. In S-12: the prelude writes the folder and stage 22 (plugins/TC-010) adds it in the window, so the lines below and step 1 are skipped there. On its own: the lines below and step 1.
- A made-up local marketplace and plugin exist (the only files this case writes, in the run's own scratch folder): run in Bash after `. <dir>/env`
- `M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-notes"`
- `echo '{"name":"acme-local","featured":["acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes","categories":["automation"]}]}' > "$M/echo-marketplace.json"`
- `echo '{"id":"acme-notes","name":"Acme notes","version":"0.1.0","summary":"A made-up plugin for tests","settings":[{"key":"greeting","label":"Greeting","type":"string","default":"hello"},{"key":"loud","label":"Loud","type":"bool","default":false}]}' > "$M/plugins/acme-notes/echo-plugin.json"`

**Changes:** `<data>/.echo/` plugin state, `<config dir>/plugins/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Add the marketplace: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`) | Prints `Added acme-local: 1 plugins` |
| 2 | Navigate to `{{base_url}}/#settings/plugins`, click the tab "Discover", type `acme` in "Search plugins", click "Install" on "Acme notes", then click "Install" in the dialog | The dialog closes; after a moment the tab reads "Installed" with the count `1` and the card shows "Installed ✓" |
| 3 | Click the tab "Installed" | One row "Acme notes 0.1.0" with the text "A made-up plugin for tests" and a switch "Acme notes on" that is on |
| 4 | Read the detail on the right | "0.1.0 · from acme-local · commit local-"; the heading "Its settings" with "Greeting" and "Loud"; "What it can do" with "Uses nothing of Echo's."; a button "Remove…" |
| 5 | Run `echoctl plugin list` | `installed` holds one entry: id `acme-notes`, `"enabled": true`, version `0.1.0` |

## Expected Final State
Acme notes is installed and enabled.

## Teardown
- `echoctl plugin remove acme-notes --delete-data`, then `echoctl plugin marketplace remove acme-local`.

## Notes
- The plugin has no code (no `main`, no `client`): it needs nothing from the network and runs nothing; its manifest is the only thing Echo reads.

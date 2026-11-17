---
id: TC-175
title: Files dropped while any of a plugin's Knowledge tabs is shown go to that plugin; elsewhere it is told nothing
priority: medium
type: functional
requirements: [F-KNOW-11]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, knowledge, files, drop]
---

# TC-175: Drops on any of a plugin's Knowledge tabs

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-172 (Vault demo with its vault, Recall off), the page loaded afresh (`location.reload()`): Inbox empty
- A drop is handed over as Echo's window does after a drag (`window.echo.pluginDrop(id, handles)`, the app's answer to `plugin_drops`); the drag itself from Finder is TC-062 (person, native window)

**Changes:** none (dropped names live in the page)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#brain/vault-demo:pages`; run `window.echo.pluginDrop('vault-demo', [{file: 'h1', name: 'Kickoff.md', size: 12}])` | "Inbox" shows the badge "1" |
| 2 | Click "Vaults"; run the same line with `name: 'Budget.xlsx'` | "Inbox" shows "2" |
| 3 | Navigate to `{{base_url}}/#calls`; run the same line with `name: 'Elsewhere.md'`; navigate back to `{{base_url}}/#brain/vault-demo:inbox` | "Inbox" lists "Kickoff.md · vault-demo:pages" and "Budget.xlsx · vault-demo:vaults" — not "Elsewhere.md" |

## Expected Final State
Unchanged data.

## Teardown
- In S-41: none. On its own: as TC-170.

## Notes
- Which plugin a drop goes to by the address (`#brain/<plugin>:…`) — `node crates/echoctl/ui/plugin-file-drop.test.mjs`.

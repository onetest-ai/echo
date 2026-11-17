---
id: TC-173
title: A plugin's header strip shows over all its tabs; moving between its tabs keeps each tab's scroll, the text typed in the strip and the plugin's state, nothing unloaded
priority: medium
type: functional
requirements: [F-KNOW-09]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, knowledge, scroll]
---

# TC-173: The header strip and each tab's place

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-172 (Vault demo with its vault, Recall off)

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#brain/vault-demo:pages` | Under the tab row, a strip reads "Acme vault" with a box "Ask the vault"; below it the Pages list fills the height to the window's bottom edge with no inset around the pane, its own scroll bar (60 pages); the tab row stays in place when the list scrolls |
| 2 | Scroll the Pages list down (about 600 px: `document.querySelector('.kn-pane:not([hidden])').scrollTop = 600`); type `kickoff` in "Ask the vault" (don't press Enter) | — |
| 3 | Click the tab "Inbox" | The strip is still there, "Ask the vault" still holds `kickoff` |
| 4 | Click the tab "Vaults", then "Pages" | The list is where it was scrolled to (`scrollTop` 600); "Ask the vault" still holds `kickoff`; `window.vaultDemoUnloads` is undefined (the plugin never unloaded) |
| 5 | Press Enter in "Ask the vault" | The strip shows "Asked: kickoff" |

## Expected Final State
Unchanged data.

## Teardown
- In S-41: none. On its own: as TC-170.

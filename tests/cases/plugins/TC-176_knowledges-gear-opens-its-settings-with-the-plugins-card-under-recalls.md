---
id: TC-176
title: The gear at the end of Knowledge's tab row opens Settings → Knowledge, the plugin's card under Recall's, headed by its name
priority: low
type: functional
requirements: [F-KNOW-12, F-KNOW-06]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, knowledge, settings]
---

# TC-176: Knowledge's settings from its tab row

**Module:** Plugins | **Priority:** Low | **Type:** Functional

## Preconditions
- As TC-172 (Vault demo with its vault, Recall off)

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#brain/vault-demo:pages`; click the gear "Knowledge settings" at the end of the tab row | The address is `#settings/brain`; "Knowledge" is picked in Settings' list |
| 2 | Read the section | The card "Recall" ("Use Recall" off) comes first; under it a card "Vault demo" with the row "Vault" — "Acme vault" — and a button "Open its tabs" |
| 3 | Click "Open its tabs" | Knowledge opens on Vault demo's "Vaults" |

## Expected Final State
Unchanged data.

## Teardown
- In S-41: none. On its own: as TC-170.

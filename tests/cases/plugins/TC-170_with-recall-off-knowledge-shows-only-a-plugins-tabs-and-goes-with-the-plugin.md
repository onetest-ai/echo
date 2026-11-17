---
id: TC-170
title: With Recall off, Knowledge is in the activity bar for a plugin's tabs alone, opens on its first shown tab, and goes when the plugin is switched off
priority: high
type: functional
requirements: [F-KNOW-01, F-KNOW-06]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, slots, knowledge]
---

# TC-170: Recall off — Knowledge for a plugin's tabs alone

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- Recall is off on this data (Settings → Knowledge → "Use Recall" off)
- Vault demo installed: in S-41, stage 1. On its own: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install vault-demo --yes`; then reload `{{base_url}}/`

**Changes:** Vault demo switched off and on again

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Look at the activity bar | "Knowledge" is in it, between Tasks and Settings |
| 2 | Click "Knowledge" | The address becomes `#brain/vault-demo:inbox`; the tab row holds a group headed "VAULT DEMO" with the tabs "Inbox" (selected) and "Vaults" — no "Sources", "Dictionary" or "People"; the page says "Drop files on any of Vault demo’s tabs." |
| 3 | Navigate to `{{base_url}}/#brain/sources` | Knowledge stays open on Vault demo's "Inbox" (Recall is off: none of its tabs), the address `#brain/vault-demo:inbox` |
| 4 | Navigate to `{{base_url}}/#settings/plugins`, switch Vault demo off (its row's switch); look at the activity bar; navigate to `{{base_url}}/#brain` | "Knowledge" is gone from the activity bar; `#brain` opens Calls |
| 5 | Switch Vault demo on again (Settings → Plugins); navigate to `{{base_url}}/#brain` | "Knowledge" is back in the activity bar and opens on Vault demo's first shown tab |

## Expected Final State
Vault demo is installed and on; Recall is off.

## Teardown
- In S-41: none. On its own: `echoctl plugin remove vault-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Which tab opens when the address names none, a Recall tab with Recall off, or a hidden tab — `node crates/echoctl/ui/knowledge-tabs.test.mjs` (`knowledgeTab`).

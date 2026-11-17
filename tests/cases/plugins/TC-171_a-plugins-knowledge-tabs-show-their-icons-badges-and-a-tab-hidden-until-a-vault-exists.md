---
id: TC-171
title: A plugin's Knowledge tabs come after Recall's as a group under its name, with icons and badges; a tab it hides shows once it says so, and the activity bar's badge sums the tabs' badges
priority: high
type: functional
requirements: [F-KNOW-06, F-KNOW-08]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, slots, knowledge, badges]
---

# TC-171: A plugin's tab group, its hidden tab and its badges

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- As TC-170 (standard data, Vault demo installed and on, Recall off); Vault demo has no vault yet
- Files are handed to the plugin as Echo does after a drop: `window.echo.pluginDrop('vault-demo', [{file: 'h1', name: 'Kickoff.md', size: 12}])` run in the page (a real drag from Finder needs the native window: TC-062)

**Changes:** Vault demo gets a vault (its store's `vault.json`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#brain/vault-demo:vaults` | Tabs "Inbox" and "Vaults", each with an icon before its name; no "Pages" (no vault yet); "Vaults" shows a "!" badge; the activity bar's "Knowledge" shows a "!" badge |
| 2 | Click "Add a vault" | "Pages" appears first in the group (before "Inbox"); "Vaults" has no badge; the header strip reads "Acme vault"; the activity bar's "Knowledge" has no badge |
| 3 | Run the drop line from the preconditions twice in the page (Kickoff.md, then with `name: 'Budget.xlsx'`) | "Inbox" shows the badge "2"; the activity bar's "Knowledge" shows "2" |
| 4 | Settings → Knowledge → switch "Use Recall" on; click "Knowledge" in the activity bar, then the tab "Sources" | The tab row: "Sources", "Dictionary", "People", a divider, then "VAULT DEMO" with "Pages", "Inbox 2", "Vaults"; "Sources" is shown with Recall's own page |
| 5 | Switch "Use Recall" off again (Settings → Knowledge) | — |

## Expected Final State
Vault demo has its vault and two files in its Inbox (in the page); Recall is off.

## Teardown
- In S-41: none. On its own: as TC-170.

## Notes
- Grouping, `order`, `visible()`, badges ('' | n | '!'), a throwing `visible`/`badge` and the bar's sum — `node crates/echoctl/ui/knowledge-tabs.test.mjs`.

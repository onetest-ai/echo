---
id: TC-172
title: "A link #brain/<plugin>:<tab>/<route> opens that tab with its route as written, through back, forward and a reload; a link to a tab it hides opens its first shown tab"
priority: high
type: functional
requirements: [F-KNOW-07]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, client-half, knowledge, routes]
---

# TC-172: A deep link into a plugin's Knowledge tab

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- As TC-171 after its step 2: Vault demo installed and on with its vault ("Acme vault"), Recall off

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#brain/vault-demo:pages/Page%2007%20Q3%20plan%2Fdraft` | "Pages" is selected; the page's title reads "Page 07 Q3 plan/draft" and its line "Route: Page%2007%20Q3%20plan%2Fdraft" (the route handed over as written) |
| 2 | Reload the page (`location.reload()`), wait for it | The same tab and page are shown again, the address unchanged |
| 3 | Click the page link "Page 03" in its list; then go back (browser Back) | "Page 03" shows, then "Page 07 Q3 plan/draft" again; Forward shows "Page 03" again |
| 4 | Click the tab "Inbox", then "Pages" | "Pages" opens on the page it last showed (the address ends `/Page%2003`) |
| 5 | Click "Calls" in the activity bar, then "Knowledge" | Knowledge opens on "Pages" at "Page 03" |
| 6 | Click "Vaults" → "Remove the vault"; navigate to `{{base_url}}/#brain/vault-demo:pages/Page%2003` | "Pages" is gone; Vault demo's first shown tab ("Inbox") is selected; the address keeps the link |
| 7 | Click "Vaults" → "Add a vault" | "Pages" is back |

## Expected Final State
Vault demo has its vault again.

## Teardown
- In S-41: none. On its own: as TC-170.

## Notes
- The address parsing (rest as written, a broken %-sequence kept) — `node crates/echoctl/ui/knowledge-tabs.test.mjs` (`knowledgeHash`). `ctx.where().route` still gives the rest decoded (an old single tab reads it).

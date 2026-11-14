---
id: TC-013
title: "\"Show me\" on a What's new item closes the dialog and opens that item's place in Echo"
priority: medium
type: functional
requirements: [F-LIFE-11]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, whats-new]
---

# TC-013: Show me opens the item's place

**Module:** Lifecycle | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built as an update from a very old version: `tests/fixtures/serve.sh up <dir> <port> --whats-new-seen 0.0.1`; the window is served at `{{base_url}}`
- An item's place is its `open`: a window route (`settings/plugins` → the URL ends `#settings/plugins`) or an `echo://` link

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Load `{{base_url}}/`; in `browser_evaluate` run `async () => (await (await fetch('api/whats-new')).json()).entries.flatMap((e) => e.items).filter((i) => i.open).map((i) => [i.title, i.open])` | The items with a place; pick the first whose `open` starts with `settings` (else the first): its TITLE and OPEN |
| 2 | In the dialog, click "Show me" on the item TITLE (`button[aria-label="Show me: TITLE"]`) | The dialog closes; the URL ends `#OPEN`; that place shows (for `settings/plugins`: Settings with the Plugins section) |
| 3 | Reload `{{base_url}}/`, wait 3 s | No dialog |
| 4 | Press `ControlOrMeta+k`, type `What's new`, press Enter; click "Show me" on an item whose `open` is another place (e.g. `today`, `projects`) | The dialog closes; that place shows |

## Expected Final State
Each Show me took the user to the place it names and closed What's new.

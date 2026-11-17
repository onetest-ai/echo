---
id: TC-181
title: A plugin's items in a call's ⋯ menu are named from the call ("Open in Acme notes", "Open in Inbox notes"), a fixed label still works, and each plugin's items are a group after a separator, above Delete
priority: medium
type: functional
requirements: [F-PLUG-102]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, calls, client-half, slots]
---

# TC-181: A plugin's items in a call's menu, named from the call and grouped per plugin

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-180's: the standard set served, `{{base_url}}/` loaded first, CLI steps after `. <dir>/env`
- The test marketplace added and "call-demo" and "menu-demo" installed and on (TC-180 step 1). Call demo adds "Open in <vault>" (the vault from the call's client: Acme's calls "Acme notes", the rest "Inbox notes") and "About Call demo"; Menu demo adds "Say hello to the call"

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the call Acme weekly sync (`[data-call$="-acme-weekly-sync"]`, the first), click "Call actions" | The menu lists, in order: Echo's own items down to "Ask Claude about this call", a separator, "Open in Acme notes", "About Call demo", a separator, "Say hello to the call", a separator, "Delete…" |
| 2 | Click "About Call demo" | A notice reads "Call demo files your calls in your notes." |
| 3 | Open the call Globex standup (`[data-call$="-globex-standup"]`), click "Call actions" | Call demo's first item reads "Open in Inbox notes" |
| 4 | Click "Say hello to the call" | A notice reads "Hello, Globex standup" (a fixed label runs on its call) |
| 5 | Switch "Call demo on" off in `{{base_url}}/#settings/plugins` (the first such switch); open Acme weekly sync, click "Call actions" | After "Ask Claude about this call": a separator, "Say hello to the call", a separator, "Delete…" — no Call demo items, no empty group |
| 6 | Switch "Call demo on" on again | — |

## Expected Final State
Both plugins installed and on.

## Teardown
- Escape closes the menu; the stage's plugins stay for the next stage, else as TC-180's teardown

## Notes
- A label function that throws or gives nothing leaves its item out and the others keep their place; `calendar.event` and `task.actions` take a label function too, without separators — `node crates/echoctl/ui/call-slots.test.mjs`.

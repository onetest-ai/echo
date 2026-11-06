---
id: TC-006
title: Notifications come from Echo and open its window
priority: high
type: functional
requirements: [F-LIFE-06]
module: lifecycle
run: person
platform: macos
tags: [lifecycle, notifications]
external_id: TC-LIFE-006
---

# TC-006: Notifications come from Echo and open its window

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- A fresh Mac account

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open Echo for the first time; Allow notifications | — |
| 2 | Trigger one (finish a recording, or Menu → Claude Code → Connect Claude Code) | The banner has Echo's icon (purple rounded square with the octopus), not a blank one |
| 3 | Click it | The Echo window opens |
| 4 | Turn Echo's notifications off in System Settings; trigger one | Nothing shows |

## Expected Final State
Notifications are Echo's own and lead to the window.

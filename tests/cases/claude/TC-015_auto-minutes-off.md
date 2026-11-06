---
id: TC-015
title: Automatic minutes off: nothing is written until asked
priority: medium
type: functional
module: claude
run: manual
platform: macos
tags: [minutes, auto]
external_id: TC-CLA-016
---

# TC-015: Automatic minutes off: nothing is written until asked

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- Menu → Claude Code → "Write minutes automatically" off

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a 4-minute call; wait 20 minutes after its transcript | The notification says to click Write minutes; no minutes written, by Echo or any session |
| 2 | Open it → Minutes → "Write minutes" | Written as usual |
| 3 | Turn the switch on again | Calls that ended while off still get none automatically; the next call does |

## Expected Final State
With the switch off Echo writes minutes only when asked.

---
id: TC-009
title: Notes taken during a call shape its minutes
priority: high
type: functional
module: claude
run: manual
platform: macos
tags: [notes, minutes]
external_id: TC-CLA-008
---

# TC-009: Notes taken during a call shape its minutes

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu bar → "Add a note…": `ask legal about the contract` | — |
| 2 | In the window, the call's My notes: add another point | Both notes are listed |
| 3 | After the call, let the minutes be written | They keep both points and use them in their structure |

## Expected Final State
The user's own notes are part of the minutes.

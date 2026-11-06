---
id: TC-006
title: Minutes run in the chosen folder with its settings
priority: low
type: functional
module: claude
run: manual
platform: macos
tags: [minutes, settings]
external_id: TC-CLA-005
---

# TC-006: Minutes run in the chosen folder with its settings

**Module:** Claude Code | **Priority:** Low | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- The call uses the built-in AI profile
- A folder A with `.claude/settings.json` holding an `env` (e.g. a gateway URL)
- A session open in an unrelated project

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → Claude Code → "Echo runs Claude Code for minutes in" → "Choose a folder…" → folder A | Folder A is ticked |
| 2 | Record a call and let its minutes be written | "Show log" names folder A; its `env` applied; the coding session is untouched |
| 3 | Menu → same item → "The data folder"; write minutes for another call | The log names the data folder |

## Expected Final State
Echo runs Claude Code in the folder chosen, with that folder's settings, never in a user's session.

---
id: TC-010
title: A note added after the call moves the minutes' time
priority: medium
type: functional
requirements: [F-MIN-14, F-NOTES-06]
module: claude
run: manual
platform: macos
tags: [notes, minutes, auto]
external_id: TC-CLA-015
---

# TC-010: A note added after the call moves the minutes' time

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- A 4-minute call just transcribed; its Minutes tab shows "Minutes at HH:MM"

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Ten minutes after the call ended, add a note in My notes | Saved; the call's `my-notes.md` has it |
| 2 | Read the Minutes tab | The time moved to 15 minutes after the last edit |
| 3 | Wait | The minutes are written then, not before, and keep the note |

## Expected Final State
Editing notes postpones automatic minutes so they include the notes.

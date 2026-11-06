---
id: TC-003
title: Minutes are written automatically 15 minutes after a call
priority: critical
type: functional
requirements: [F-MIN-14, F-MIN-07]
module: claude
run: manual
platform: macos
tags: [minutes, auto]
external_id: TC-CLA-003
---

# TC-003: Minutes are written automatically 15 minutes after a call

**Module:** Claude Code | **Priority:** Critical | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- Menu → Claude Code → "Write minutes automatically" on (default); no Claude Code session open
- A recorded test call of at least 4 minutes with a decision and a task with an owner and a date (made-up content)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Wait for the transcript; open the call → Minutes | "Minutes at HH:MM" (15 minutes after the call ended) with "Write minutes now"; the Transcript ready notification names the same time |
| 2 | Wait until that time without touching the call | `notes.md` and `actions.json` appear; a "Minutes ready" notification |
| 3 | Read the minutes | Summary, decisions and next steps with time chips; the task with its owner and date |
| 4 | Click "Show log" | The run says Echo started Claude Code for the minutes automatically; it sent nothing and offered no follow-ups |

## Expected Final State
With no one asking, the minutes are written once, at the time Echo named, from the call alone.

---
id: TC-004
title: Write minutes from the window, and for a call too short for automatic ones
priority: high
type: functional
requirements: [F-MIN-13, F-MIN-08]
module: claude
run: person
platform: macos
tags: [minutes]
external_id: TC-CLA-009
---

# TC-004: Write minutes from the window, and for a call too short for automatic ones

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- No Claude Code session open
- A recorded test call of at least 4 minutes with a decision and a task with an owner and a date (made-up content)
- A second recorded call of about 1 minute

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the 4-minute call → Minutes → "Write minutes now" (before its time) | "Writing minutes…" naming the model; within about two minutes the minutes; a "Minutes ready" notification |
| 2 | Click "Show log" | `claude.log`: the commands Claude ran, the time taken and the cost |
| 3 | Open the 1-minute call → Minutes | "No minutes written automatically" with the reason, and "Write minutes" |
| 4 | Click "Write minutes" | Minutes are written anyway |

## Expected Final State
Minutes can be written at once from the window, also for a call the gate skipped.

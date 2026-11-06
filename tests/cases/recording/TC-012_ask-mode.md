---
id: TC-012
title: Ask when calls start: Echo's own dialog decides per call
priority: high
type: functional
requirements: [F-REC-03, F-SET-06]
module: recording
run: manual
platform: macos
tags: [recording, ask]
external_id: TC-REC-014
---

# TC-012: Ask when calls start: Echo's own dialog decides per call

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → Settings → Calls not on the calendar → "Ask when they start" | — |
| 2 | Start a Teams call; answer Echo's "Record?" with no | Nothing recorded |
| 3 | Start another and answer yes | Only that call is recorded |
| 4 | Set it back to "Only when I click Record" | No more questions |

## Expected Final State
Ask mode needs no Claude Code and records only on yes.

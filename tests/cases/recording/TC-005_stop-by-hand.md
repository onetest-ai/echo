---
id: TC-005
title: Stopping by hand ends the recording, not the call
priority: critical
type: functional
requirements: [F-REC-05]
module: recording
run: person
platform: macos
tags: [recording]
external_id: TC-REC-005
---

# TC-005: Stopping by hand ends the recording, not the call

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Stop recording" | Recording stops at once; Teams stays connected |
| 2 | Stay one more minute, then hang up | Nothing more is recorded; Echo does not re-arm |
| 3 | Read the transcript | It ends where you stopped |

## Expected Final State
Stop means stop, for the rest of that call.

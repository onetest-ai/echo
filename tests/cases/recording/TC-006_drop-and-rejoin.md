---
id: TC-006
title: A short drop and rejoin stays one recording
priority: high
type: functional
requirements: [F-REC-08]
module: recording
run: manual
platform: macos
tags: [recording]
external_id: TC-REC-006
---

# TC-006: A short drop and rejoin stays one recording

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Leave the call and rejoin within 5 seconds | — |
| 2 | After the call, open Calls | One call, not two; the gap is silence in its timeline |

## Expected Final State
A glitch doesn't split the call.

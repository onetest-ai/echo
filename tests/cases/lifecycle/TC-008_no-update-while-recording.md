---
id: TC-008
title: No update during a recording
priority: low
type: functional
module: lifecycle
run: manual
platform: macos
tags: [update, recording]
external_id: TC-UPD-002
---

# TC-008: No update during a recording

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- A newer release; a call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Click Update | "Update failed: a call is being recorded — update after it ends"; the recording continues |

## Expected Final State
An update never interrupts a recording.

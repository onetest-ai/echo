---
id: TC-008
title: No update during a recording
priority: low
type: functional
requirements: [F-LIFE-07]
module: lifecycle
run: person
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

## Notes
- Stays person-run: it needs a newer release offered and an Update click while recording; the offer lives only in the app process (tray, notification) and a mistaken Update would replace the installed Echo.

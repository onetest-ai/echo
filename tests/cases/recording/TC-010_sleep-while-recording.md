---
id: TC-010
title: The Mac sleeping closes the recording
priority: low
type: functional
module: recording
run: manual
platform: macos
tags: [recording, lifecycle]
external_id: TC-REC-011
---

# TC-010: The Mac sleeping closes the recording

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Close the lid for a minute, then open it | The recording closed at sleep and is transcribed; nothing recorded after wake unless armed again |

## Expected Final State
Sleep ends a recording cleanly.

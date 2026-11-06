---
id: TC-007
title: Switching audio devices mid-call keeps recording in sync
priority: high
type: functional
module: recording
run: manual
platform: macos
tags: [recording, devices]
external_id: TC-REC-007
---

# TC-007: Switching audio devices mid-call keeps recording in sync

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- A call being recorded on the built-in speakers and mic

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | After a minute connect AirPods and let Teams switch | Recording continues |
| 2 | After a minute disconnect them | Recording continues |
| 3 | Read the transcript | Both sides before, during and after; no drift between your lines and theirs |

## Expected Final State
Device switches don't break or desync the recording.

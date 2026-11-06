---
id: TC-011
title: A crash while recording is repaired on restart
priority: low
type: functional
requirements: [F-REC-11]
module: recording
run: manual
platform: macos
tags: [recording, lifecycle]
external_id: TC-REC-012
---

# TC-011: A crash while recording is repaired on restart

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Activity Monitor → echoctl → Force Quit | — |
| 2 | Open Echo again | The unfinished recording is repaired and transcribed up to the crash |
| 3 | If its transcription fails | The call shows the error instead of retrying forever |

## Expected Final State
A crash loses at most the time after it.

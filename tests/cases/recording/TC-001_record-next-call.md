---
id: TC-001
title: Record the next call from the menu
priority: critical
type: functional
requirements: [F-REC-02, F-REC-08, F-REC-13]
module: recording
run: manual
platform: macos
tags: [recording, arm]
external_id: TC-REC-001
---

# TC-001: Record the next call from the menu

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- Not in a call

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Record next call" | The icon turns armed; status "Armed — recording starts with the next call"; the item reads "Cancel" |
| 2 | Start a Teams test call within 15 minutes; talk 2 minutes, both sides | When it connects the icon turns recording; the menu offers "Stop recording" |
| 3 | Hang up | About 10 seconds later the recording closes; the icon goes idle |
| 4 | Open the window → Calls | The call is listed, transcribing, then with its transcript |

## Expected Final State
An armed next call is recorded from connect to hang-up and transcribed.

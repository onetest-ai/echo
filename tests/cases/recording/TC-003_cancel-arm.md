---
id: TC-003
title: Cancelling an arm records nothing
priority: high
type: functional
requirements: [F-REC-02, F-REC-03]
module: recording
run: manual
platform: macos
tags: [recording, arm]
external_id: TC-REC-003
---

# TC-003: Cancelling an arm records nothing

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Record next call", then "Cancel" | The icon is idle |
| 2 | Start a Teams call | It is not recorded |

## Expected Final State
Cancel really disarms.

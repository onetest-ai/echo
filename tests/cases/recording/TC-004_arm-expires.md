---
id: TC-004
title: An arm expires after 15 minutes
priority: low
type: functional
module: recording
run: manual
platform: macos
tags: [recording, arm]
external_id: TC-REC-004
---

# TC-004: An arm expires after 15 minutes

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Record next call"; start no call for 16 minutes | After 15 minutes the icon is idle again |
| 2 | Then start a Teams call | It is not recorded |

## Expected Final State
An unused arm doesn't linger.

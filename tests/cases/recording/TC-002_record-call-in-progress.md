---
id: TC-002
title: Record a call already in progress
priority: critical
type: functional
requirements: [F-REC-01]
module: recording
run: manual
platform: macos
tags: [recording]
external_id: TC-REC-002
---

# TC-002: Record a call already in progress

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- In a Teams test call, not armed

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the menu | It offers "Record this call" (not "next") |
| 2 | Click it | Recording starts within a couple of seconds |
| 3 | After the call read the transcript | It covers only what was said after the click |

## Expected Final State
A call can be recorded from the middle.

---
id: TC-011
title: Recording works without Microsoft 365
priority: medium
type: functional
requirements: [F-REC-16, F-CALLS-07]
module: calendar
run: manual
platform: macos
tags: [recording, m365]
external_id: TC-CHAT-003
---

# TC-011: Recording works without Microsoft 365

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo signed out of Microsoft 365

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a call | Recording and transcription work as usual |
| 2 | Open it | No Chat tab, no calendar title |

## Expected Final State
Microsoft 365 is optional for recording.

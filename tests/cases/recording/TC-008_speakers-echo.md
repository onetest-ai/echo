---
id: TC-008
title: On speakers, the other side isn't duplicated as Me
priority: high
type: functional
module: recording
run: manual
platform: macos
tags: [recording, echo]
external_id: TC-REC-008
---

# TC-008: On speakers, the other side isn't duplicated as Me

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a call on the laptop speakers, the other person talking a lot | — |
| 2 | Read the transcript | Their words are not repeated as Me lines |

## Expected Final State
Echo cancellation keeps the speakers' sound out of your track.

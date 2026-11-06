---
id: TC-009
title: What you say while muted in Teams isn't recorded
priority: high
type: functional
module: recording
run: manual
platform: macos
tags: [recording, mute, accessibility]
external_id: TC-REC-009
---

# TC-009: What you say while muted in Teams isn't recorded

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running with permissions granted; a Teams test call means a second person (or account) speaking, at least 1 minute
- Accessibility granted
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Mute in Teams; say "this should not be recorded" | — |
| 2 | Unmute and keep talking | — |
| 3 | Read the transcript | The muted sentence is absent; speech after unmuting is there |

## Expected Final State
Teams' mute is honoured.

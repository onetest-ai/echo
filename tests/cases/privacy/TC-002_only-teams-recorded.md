---
id: TC-002
title: Only the call app's audio is recorded as Remote
priority: critical
type: functional
requirements: [F-PRIV-02]
module: privacy
run: person
platform: macos
tags: [privacy, capture]
external_id: TC-PRIV-002
---

# TC-002: Only the call app's audio is recorded as Remote

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- A call being recorded

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Play a video with speech in a browser, on headphones | None of it appears as Remote lines |
| 2 | Play it on the speakers | At most faint pickup by your mic as Me; nothing as Remote |

## Expected Final State
Echo records the call app, not the rest of the computer.

---
id: TC-005
title: Silence produces no made-up text
priority: medium
type: functional
requirements: [F-STT-03]
module: transcription
run: manual
platform: macos
tags: [transcription]
external_id: TC-TRN-006
---

# TC-005: Silence produces no made-up text

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a call with 2 minutes of silence in the middle (typing allowed) | No text in that stretch |

## Expected Final State
No hallucinated lines during silence.

---
id: TC-006
title: A long call transcribes and the Mac stays usable
priority: medium
type: functional
requirements: [F-STT-06, F-STT-01]
module: transcription
run: person
platform: macos
tags: [transcription, performance]
external_id: TC-TRN-007
---

# TC-006: A long call transcribes and the Mac stays usable

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a call of 60 minutes or more | — |
| 2 | Time the transcription | A few minutes per hour on Apple Silicon; the Mac stays usable |
| 3 | Wait for it | The transcript appears with a "Transcript ready" notification |

## Expected Final State
Long calls finish in reasonable time without blocking the Mac.

---
id: TC-004
title: Transcription works offline
priority: critical
type: functional
requirements: [F-PRIV-04]
module: privacy
run: person
platform: macos
tags: [privacy, transcription]
external_id: TC-PRIV-004
---

# TC-004: Transcription works offline

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- The built-in speech model downloaded; no transcription server connected

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Turn Wi-Fi off and record a call (a second device speaking to the mic will do) | — |
| 2 | Let it transcribe | The transcript is produced |
| 3 | (Optional) watch Echo's connections with `nettop` during transcription | None |

## Expected Final State
Transcription needs no network.

## Notes
- Stays person-run: Wi-Fi is switched off by a person. TestCall can serve the call on `127.0.0.1`, so the call itself still works with the network off.

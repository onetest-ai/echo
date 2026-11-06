---
id: TC-003
title: No audio is kept once a call is transcribed
priority: critical
type: functional
requirements: [F-PRIV-03]
module: privacy
run: person
platform: any
tags: [privacy, audio]
external_id: TC-PRIV-003, TC-TRN-002
---

# TC-003: No audio is kept once a call is transcribed

**Module:** Privacy | **Priority:** Critical | **Type:** Functional

## Preconditions
- Several recorded calls, all transcribed

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `find <data folder> <config folder> -name '*.wav'` | Nothing (audio exists only while a call records or transcribes) |

## Expected Final State
Only text stays.

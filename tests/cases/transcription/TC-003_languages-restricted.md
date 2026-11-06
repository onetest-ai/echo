---
id: TC-003
title: With one language set, nothing comes out in another
priority: high
type: functional
requirements: [F-STT-02, F-SET-07]
module: transcription
run: person
platform: macos
tags: [transcription, languages]
external_id: TC-TRN-004
---

# TC-003: With one language set, nothing comes out in another

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- Settings → Languages: English only

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record an English call with a strong accent | The transcript is English throughout |

## Expected Final State
The language setting holds.

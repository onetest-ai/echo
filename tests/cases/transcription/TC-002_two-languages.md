---
id: TC-002
title: Each sentence stays in the language it was spoken in
priority: high
type: functional
requirements: [F-STT-02]
module: transcription
run: person
platform: macos
tags: [transcription, languages]
external_id: TC-TRN-003
---

# TC-002: Each sentence stays in the language it was spoken in

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- Settings → Languages: English and Ukrainian (or a pair you speak)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record a call switching between the two languages | — |
| 2 | Read the transcript | Each sentence in its own language; nothing translated |

## Expected Final State
Mixed-language calls are transcribed as spoken.

---
id: TC-004
title: A language pinned for one call applies to that call only
priority: medium
type: functional
requirements: [F-STT-02]
module: transcription
run: person
platform: macos
tags: [transcription, languages]
external_id: TC-TRN-005
---

# TC-004: A language pinned for one call applies to that call only

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Settings → Languages has two or more

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Record menu → "Only Ukrainian" before a call; record it in Ukrainian | Its transcript is Ukrainian only |
| 2 | Check Settings → Languages | Unchanged; the next call uses them |

## Expected Final State
A per-call pin doesn't change the workspace setting.

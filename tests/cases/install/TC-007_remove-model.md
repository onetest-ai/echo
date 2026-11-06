---
id: TC-007
title: Remove a model; the one in use is protected
priority: low
type: functional
requirements: [F-SET-09, F-LIFE-01]
module: install
run: manual
platform: any
tags: [install, model]
external_id: TC-INST-007
---

# TC-007: Remove a model; the one in use is protected

**Module:** Install | **Priority:** Low | **Type:** Functional

## Preconditions
- Two models downloaded, one in use

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → Speech models → the unused one → "Remove (… MB)…" → "Remove" | It is deleted and leaves the list |
| 2 | Try the same on the model in use (or the language-ID helper) | Refused, or a clear question first; Echo keeps a working model |

## Expected Final State
Models can be removed without breaking transcription.

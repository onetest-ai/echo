---
id: TC-007
title: Remove a model; the one in use is protected
priority: low
type: functional
requirements: [F-SET-09, F-LIFE-01]
module: install
run: agent
platform: any
tags: [install, model]
external_id: TC-INST-007
---

# TC-007: Remove a model; the one in use is protected

**Module:** Install | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo installed and running; two models downloaded, one in use (in S-16: `install/TC-005`'s, in use, and `install/TC-006`'s; on its own: download a second one in Settings → Transcription first); `E=/Applications/Echo.app/Contents/MacOS/echoctl`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | **The user does:** Echo's own window → Settings → Transcription → the unused model → "Remove (… MB)…" → "Remove" | It is deleted (its file is gone from `~/Library/Application Support/echo/models/`) and leaves the list |
| 2 | Try the same on the model in use (or, while large-v3 or small is in use, the language-ID helper `large-v3-turbo-q5_0`); also `$E model remove <the one in use>` | Refused, or a clear question first; `$E model list` still has a model in use |

## Expected Final State
Models can be removed without breaking transcription.

---
id: TC-009
title: Shared-screen frames on Windows
priority: low
type: functional
requirements: [F-WIN-06, F-REC-12]
module: windows
run: person
platform: windows
tags: [windows, frames]
external_id: TC-WINOS-009
---

# TC-009: Shared-screen frames on Windows

**Module:** Windows | **Priority:** Low | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl frame-grab %TEMP%\echo-frame.jpg` with Teams open behind other windows | The Teams content, not black |
| 2 | Settings → Save snapshots of shared screens; record a call where someone shares slides | `frames/` one JPEG per slide; `frames.jsonl` indexes them |

## Expected Final State
Frames work on Windows.

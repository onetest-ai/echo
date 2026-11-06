---
id: TC-007
title: Mute sync and Teams UI text on Windows
priority: high
type: functional
requirements: [F-WIN-06, F-REC-10]
module: windows
run: person
platform: windows
tags: [windows, mute]
external_id: TC-WINOS-007
---

# TC-007: Mute sync and Teams UI text on Windows

**Module:** Windows | **Priority:** High | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In a Teams call: `echoctl ax-mic --seconds 30`; mute and unmute | The button state flips live / MUTED |
| 2 | Record a call, muted ~20 s in the middle while talking | Events `mic_muted` / `mic_unmuted`; nothing said while muted in the transcript; `ui.jsonl` and `ui-tree-10s.json` in the folder |

## Expected Final State
Teams' mute is honoured on Windows.

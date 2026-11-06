---
id: TC-005
title: Echo connects only where the user expects
priority: high
type: functional
requirements: [F-PRIV-05, F-PRIV-07]
module: privacy
run: person
platform: macos
tags: [privacy, network]
external_id: TC-PRIV-005
---

# TC-005: Echo connects only where the user expects

**Module:** Privacy | **Priority:** High | **Type:** Functional

## Preconditions
- A connection monitor (Little Snitch, LuLu or `nettop`)
- Microsoft 365 connected; no transcription server

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Use Echo for a day: record, let minutes be written, open the calendar | Echo's own connections go only to GitHub (updates), Hugging Face (model download), Microsoft (sign-in and Graph) |
| 2 | Check what carries call content | Only the Claude Code process of the call's AI profile (minutes, live notes when on); Echo itself sends no transcript anywhere |

## Expected Final State
No unexpected destination; call content leaves only through the AI profile the user chose. (What a minutes run may read and do: `.ai-run/guides/design/ai-and-minutes.md`.)

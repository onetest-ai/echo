---
id: TC-008
title: The live pane follows a recorded call in a session
priority: medium
type: functional
module: claude
run: manual
platform: macos
tags: [live]
external_id: TC-CLA-007
---

# TC-008: The live pane follows a recorded call in a session

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- Live notes on (menu → AI for calls → "Live notes during calls")

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In a session, start recording a call (`/echo:live` if the pane doesn't open) | A band above the prompt shows the recording with `[ Live ]` |
| 2 | Talk for 5 minutes | The pane shows the transcript as it comes and, after a few minutes, a rolling summary, highlights and actions |

## Expected Final State
A session shows the call live while it records.

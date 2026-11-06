---
id: TC-005
title: A session can't write a call's minutes while Echo writes them, and the other way round
priority: medium
type: functional
module: claude
run: manual
platform: macos
tags: [minutes, claim]
external_id: TC-CLA-004, TC-CLA-013
---

# TC-005: A session can't write a call's minutes while Echo writes them, and the other way round

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- A transcribed call without minutes

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Window → its Minutes → "Write minutes"; while "Writing minutes…" shows, run `/echo:process-call <call>` in a session | The session says Echo is writing the minutes for this call right now, and writes nothing |
| 2 | After Echo's minutes are written, run `/echo:process-call <call>` in the session | It writes them again and offers follow-ups |
| 3 | For another call without minutes: start `/echo:process-call <call>` in a session, then click "Write minutes" in the window | "Couldn't write minutes": a session is writing them; Echo starts nothing and no automatic run starts for it meanwhile |

## Expected Final State
Only one writer at a time per call, whichever started first.

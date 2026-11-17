---
id: TC-182
title: A plugin's line shows in a call's minutes box only while the minutes are coming — gone while the user holds them ("Until I say"), back when they are automatic again, and never on a call with minutes
priority: medium
type: functional
requirements: [F-PLUG-103]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, calls, minutes, client-half, slots]
---

# TC-182: A plugin's line in the minutes box, only while the minutes are coming

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-180's: the standard set served, `{{base_url}}/` loaded first
- "call-demo" installed and on (TC-180 step 1); its line reads "Call demo files these minutes in <vault> once they are written."
- The standard set's Globex standup: transcribed, no minutes, automatic minutes due in about 15 minutes ("Minutes in N min (HH:MM)", moved by its notes)

**Changes:** Globex standup's automatic minutes held, then automatic again (as TC-028)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the call Globex standup (`[data-call$="-globex-standup"]`), tab "Minutes" | The box "Minutes in N min (HH:MM)" holds, under "Claude Code writes them with …", the line "Call demo files these minutes in Inbox notes once they are written.", above the Model choice and "Write now" |
| 2 | Click "Not yet ▾", then "Until I say — I'll click Write minutes" | The heading reads "Minutes when you say"; no Call demo line (`[data-call-minutes]` count 0) |
| 3 | Click "Write them automatically again" | The heading reads "Minutes in N min (HH:MM)" again, with the Call demo line |
| 4 | Open the call Acme weekly sync (it has minutes) | Its minutes page shows; no Call demo line |

## Expected Final State
Globex standup's minutes are automatic again; nothing was written.

## Teardown
- None beyond the scenario's (no Claude run was started: "Write now" is never clicked)

## Notes
- "Coming" is a run queued or going, or Echo writing them by itself (due now or at a time, the call not over or not transcribed yet, a Claude Code session holding it); not when off, held, too thin, can't start, failed, or written (`minutesComing`) — `node crates/echoctl/ui/call-slots.test.mjs`.

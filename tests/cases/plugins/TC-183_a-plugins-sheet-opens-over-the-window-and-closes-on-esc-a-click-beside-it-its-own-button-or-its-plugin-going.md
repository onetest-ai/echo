---
id: TC-183
title: A plugin's sheet opens over the window with the keyboard in it and Tab going round inside; Esc, a click beside it or its own button close it (its onClose called once, the keyboard back on Call actions); a click inside doesn't; switched off, its plugin's sheet closes without onClose; it knows Recall is off
priority: medium
type: functional
requirements: [F-PLUG-104]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, calls, client-half, dialog, keyboard]
---

# TC-183: A plugin's sheet over the window

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- As TC-180's: the standard set served (Recall off), `{{base_url}}/` loaded first, CLI steps after `. <dir>/env`
- "call-demo" installed and on (TC-180 step 1). Its "Open in <vault>" opens a sheet "File this call": a heading, "Into <vault>. Recall is on|off.", a box "A note for the vault", "Cancel" and "File it"; its `onClose` counts in its store (`echoctl plugin call call-demo closes`)
- Before step 1: `echoctl plugin call call-demo closes` — note the number as N

**Changes:** `<data>/.echo/plugins/call-demo/` (its count of closes, the calls it filed)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the call Acme weekly sync (`[data-call$="-acme-weekly-sync"]`, the first), "Call actions" → "Open in Acme notes" | A dialog "File this call" over the window: heading "File “Acme weekly sync”", "Into Acme notes. Recall is off.", the box "A note for the vault" focused |
| 2 | Press Tab three times | The focus goes Cancel, File it, and back to the box (it never leaves the sheet) |
| 3 | Click the sheet's text "Into Acme notes…" | The sheet stays open |
| 4 | Press Escape | The sheet is gone; the focus is on "Call actions"; `echoctl plugin call call-demo closes` is N+1 |
| 5 | "Call actions" → "Open in Acme notes" again; click the dimmed backdrop beside the sheet (e.g. 30 px from the window's left edge, mid-height) | The sheet is gone; `closes` is N+2 |
| 6 | Open the call Globex standup, "Call actions" → "Open in Inbox notes", click "File it" | The sheet is gone; the header's chip reads "▤ Filed"; `closes` is N+3 |
| 7 | "Call actions" → "Open in Inbox notes"; while it is open, `echoctl plugin disable call-demo` | Within a few seconds the sheet is gone, and with it the header chip and the "Filed calls" activity |
| 8 | Switch "Call demo on" on again in `{{base_url}}/#settings/plugins` (the first such switch); `echoctl plugin call call-demo closes` | Still N+3: a sheet closed by its plugin going calls no `onClose` |

## Expected Final State
Call demo on; Globex standup filed.

## Teardown
- None beyond the scenario's

## Notes
- One sheet at a time (a second closes the first, its `onClose` called), `onClose` once, Tab's order, and the sheet closed quietly by its plugin's cleanup — `node crates/echoctl/ui/call-slots.test.mjs`.
- The sheet is Echo's dialog frame (`td-modal`, `td-dialog`, `role="dialog"`, `aria-modal`) around the plugin's HTML, in its own holder after `#r-over` (Echo's redraws never replace it); the palette and Echo's own dialogs open over it.

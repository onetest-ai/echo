---
id: TC-012
title: Open minutes in browser and Show in Finder use the call's files
priority: low
type: functional
requirements: [F-CALLS-17]
module: window
run: person
platform: macos
tags: [minutes, native]
external_id: TC-WIN-003
---

# TC-012: Open minutes in browser and Show in Finder use the call's files

**Module:** Window | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running; a call with minutes

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open the call → "…" → "Open minutes in browser" | The default browser opens `<title>-minutes.html` from the call's folder |
| 2 | "…" → "Show minutes in Finder" | Finder opens with that file selected |
| 3 | "…" → "Show folder" | Finder opens the call's folder |

## Expected Final State
Each menu item opens the call's own file or folder in the system app.

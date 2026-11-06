---
id: TC-019
title: The tray icon and menu follow arm, record and stop
priority: medium
type: functional
requirements: [F-REC-13]
module: recording
run: person
platform: macos
tags: [recording, tray]
---

# TC-019: The tray icon and menu follow arm, record and stop

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo running as an app with its tray icon, permissions granted; the dev build with `ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall` and TestCall on this Mac, or a Teams test call
- Not in a call, not armed

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Record next call" | The icon turns armed; the menu shows the armed status and an item reading "Cancel" |
| 2 | Start the test call (or a Teams call) | When it connects the icon turns recording; the menu offers "Stop recording" |
| 3 | Menu → "Stop recording" | The icon goes idle |
| 4 | Menu → "Record next call", then "Cancel" | The icon goes armed, then idle |

## Expected Final State
The tray icon shows idle, armed and recording at the right moments.

## Notes
- Split from `recording/TC-001`, whose recording path an agent now runs; only the icon and menu wording need a person.

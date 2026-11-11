---
id: TC-019
title: The tray icon and menu follow arm, record and stop
priority: medium
type: functional
requirements: [F-REC-13]
module: recording
run: agent
platform: macos
tags: [recording, tray]
---

# TC-019: The tray icon and menu follow arm, record and stop

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo running as an app with its tray icon: the dev build on file-fed test data (`make_data.py <dir> --ai fake`, `. <dir>/env`, `echoctl app run` without `--no-tray`; calls from `tests/fixtures/call.py`: `app_profile.md` → "Recording with file-fed calls"), or an installed Echo and a Teams test call
- Not in a call, not armed
- **No other Echo runs:** `pgrep -fl echoctl` lists only this run's app — checked before the case and again right before every click; any other match (the user's Echo, a launcher, another `echoctl` call) makes the case BLOCKED, never filtered away — so no click can reach another Echo

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Record next call" → "In English" | The icon turns armed; the menu shows the armed status and an item reading "Cancel" |
| 2 | Start a call (`tests/fixtures/call.py start acme-sync --hold`, or a Teams call) | When it connects the icon turns recording; the menu offers "Stop recording" |
| 3 | Menu → "Stop recording" | The icon goes idle |
| 4 | End the call (`tests/fixtures/call.py end`); then Menu → "Record next call" → "In English", then "Cancel" | The icon goes armed, then idle |

## Expected Final State
The tray icon shows idle, armed and recording at the right moments.

## Notes
- Agent-run where no other Echo runs (run 2026-11-11 on a Mac without one: 4/4). With the user's own Echo running, System Events can't keep the two `echoctl` menu-bar items apart (tried 2026-11-07: the dev app's pid returned the installed Echo's menu), hence the `pgrep` precondition.
- Driving it: System Events' `click` (AXPress) on the menu-bar item doesn't open the menu; a real mouse click at its position does (a CGEvent click from `osascript -l JavaScript`); then `click menu item …` of the process by its pid. Evidence: the item's tooltip (`AXHelp`: "Armed …", "● Recording · since …", "Not recording"), `echoctl status`, and a small `screencapture -x -R` of the icon (armed green, recording orange, idle white).
- A person still judges the icon's look (readable in light and dark menu bars, not behind the notch).
- Split from `recording/TC-001`, whose recording path an agent now runs; only the icon and menu wording need a person.

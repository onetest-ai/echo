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
- Echo running as an app with its tray icon: the dev build on file-fed test data (`make_data.py <dir> --ai fake`, `. <dir>/env`, `echoctl app run` without `--no-tray`; calls from `tests/fixtures/call.py`: `app_profile.md` → "Recording with file-fed calls"), or an installed Echo and a Teams test call
- Not in a call, not armed

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → "Record next call" | The icon turns armed; the menu shows the armed status and an item reading "Cancel" |
| 2 | Start a call (`tests/fixtures/call.py start acme-sync`, or a Teams call) | When it connects the icon turns recording; the menu offers "Stop recording" |
| 3 | Menu → "Stop recording" | The icon goes idle |
| 4 | Menu → "Record next call", then "Cancel" | The icon goes armed, then idle |

## Expected Final State
The tray icon shows idle, armed and recording at the right moments.

## Notes
- Stays person-run (tried 2026-11-07): with the user's own Echo running, System Events can't keep the two `echoctl` menu-bar items apart — opening the dev app's item by its pid returned the installed Echo's menu (its real recent calls). A click there could quit or arm the user's Echo, so no agent drives a tray on a Mac where Echo runs. On a Mac without Echo installed it could become an agent case.
- Split from `recording/TC-001`, whose recording path an agent now runs; only the icon and menu wording need a person.

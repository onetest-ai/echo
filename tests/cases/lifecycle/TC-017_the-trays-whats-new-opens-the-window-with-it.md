---
id: TC-017
title: The tray menu's "What's new…" opens Echo's window on Today with What's new over it, whether the window was closed or open
priority: low
type: functional
requirements: [F-LIFE-12]
module: lifecycle
run: person
platform: any
tags: [lifecycle, whats-new, tray]
---

# TC-017: The tray's What's new opens the window with it

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running with its tray (the menu-bar octopus on macOS, the notification area on Windows): an installed Echo, or the dev build's `echoctl app run` on test data where the user's own Echo isn't running
- The Echo window closed

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Open the tray menu | "What's new…" stands after "Check for updates…" (or "Update to …") |
| 2 | Click "What's new…" | Echo's window opens on Today with the dialog "What's new in Echo" over it: every version's items, newest first |
| 3 | Click "Got it"; go to Calls; open the tray menu and click "What's new…" again | The window comes to the front on Today with the dialog |
| 4 | Press Escape | The dialog closes; Today stays |

## Expected Final State
The tray's What's new opened the window at it, closed or open; nothing else changed.

## Notes
- A person's case: the tray can't be clicked by an agent. The window's side (`#whats-new` opens Today with the dialog) is `TC-014` steps 8–9; the menu's place is the window's own route (`tray::tests::the_menus_whats_new_opens_a_place_the_window_routes`).

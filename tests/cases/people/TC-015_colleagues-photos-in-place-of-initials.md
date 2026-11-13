---
id: TC-015
title: Allow… colleagues' photos: the switch goes on and colleagues of the user's domain show their photos in place of initials; others keep initials; off brings initials back
priority: medium
type: functional
requirements: [F-PPL-08, F-SET-22]
module: people
run: person
platform: macos
tags: [people, photos, microsoft-365]
---

# TC-015: Colleagues' photos in place of initials

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- **Why a person:** it needs a real Microsoft 365 account and its sign-in in the browser; the photos are real people's, so nothing of them goes into a report (say "a photo shows", never whose)
- Echo's dev build beside Echo (`.ai-run/guides/development/dev-workflow.md`) with its own config folder (`ECHOCTL_CONFIG_DIR`), signed in to Microsoft 365 (`echoctl calendar login`); the data folder has at least one recorded call with a colleague from the user's own mail domain who is in the people directory with that address, and one with someone from another domain
- Settings → About me has the user's own address (or the sign-in gives it)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Settings → Calendar & chat, read the row "Colleagues' photos" | Before any photo sign-in: it says Echo asks Microsoft 365 for one more permission and the organisation may refuse it, with "Allow…" |
| 2 | Click "Allow…", sign in in the browser and accept the permission (read users' basic profiles) | A notice "Colleagues' photos on"; the row now has a switch, on |
| 3 | Open Calls and wait 10 s | The faces of colleagues from the user's own domain show photos (those who have one); faces of people from another domain keep their initials |
| 4 | Open a thread's People → Map with a colleague placed in a stance | Their photo, with a ring in the stance's colour |
| 5 | Run `ls "$ECHOCTL_CONFIG_DIR/photos"` | Files named by colleagues' addresses only, in the user's domain (`.jpg`, or `.none` for one without a photo); none for another domain |
| 6 | Settings → Calendar & chat, turn "Colleagues' photos" off, open Calls again (reload the window) | Every face shows initials again |
| 7 | If the organisation refuses the permission at step 2 | A notice "Photos not allowed"; the row keeps "Allow…"; the faces keep initials |

## Expected Final State
The switch as left; the photos kept in the config folder's `photos/` (delete it to clean up).

## Notes
- Allowed once, the permission stays with the account for Echo's app registration: the main sign-in's token carries it from its next refresh, so the switch is what keeps photos off.

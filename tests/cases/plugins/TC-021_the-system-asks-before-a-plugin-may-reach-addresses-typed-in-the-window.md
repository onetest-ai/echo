---
id: TC-021
title: Addresses typed in a plugin's settings in the window bring up the system's own dialog — Allow saves them, Don't allow (or no answer) keeps what was there
priority: high
type: functional
requirements: [F-PLUG-14]
module: plugins
run: person
platform: macos
tags: [plugins, network, settings, permissions, dialog]
---

# TC-021: The system asks before a plugin may reach the addresses you type

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Echo from a release (the dmg), running, its window open
- A plugin whose sheet says "Talks to the addresses you list in its setting …" installed (made-up: Acme sneak from plugins/TC-020, without its `client.js` — or any plugin with `network.hosts_from`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Settings → Plugins → the plugin; in its address setting type `http://192.168.68.*:8808, http://10.0.0.5:*` and leave the field | A dialog of the system's (not part of Echo's page), titled Echo: "Let <plugin> reach these addresses?", the two addresses on their own lines, "Allow only if you entered them yourself.", buttons "Don't allow" (the default) and "Allow"; the detail says "Waiting for you to allow the addresses…" |
| 2 | Click "Don't allow" | The detail says "Not saved: you didn't allow <plugin> to reach http://192.168.68.*:8808, http://10.0.0.5:*"; the field, after a reload, is as before |
| 2a | At once, type them again and leave the field | No dialog: "Not saved: you didn't allow these addresses a moment ago" (a no holds for a minute) |
| 3 | After a minute, type the same addresses again, leave the field; click "Allow" | The dialog closes; the field keeps the addresses after a reload; `echoctl plugin set <id> <key>` shows them |
| 4 | Leave the field unchanged (click in and out) | No dialog: nothing changed, nothing is sent |
| 5 | Empty the field and leave it | No dialog (it only narrows); the setting is empty |

## Expected Final State
The setting holds only what the user allowed in the system's dialog.

## Teardown
- Remove the made-up plugin and its marketplace, if one was added

## Notes
- Written from the feature's text (F-PLUG-14), not from the code.
- On Windows the dialog is the system's Yes/No popup, Yes standing for Allow and No focused.

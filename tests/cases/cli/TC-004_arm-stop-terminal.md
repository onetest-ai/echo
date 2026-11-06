---
id: TC-004
title: Arming and stopping from the terminal drives the recording like the menu
priority: medium
type: functional
module: cli
run: manual
platform: macos
tags: [cli, recording]
external_id: TC-CLI-002
---

# TC-004: Arming and stopping from the terminal drives the recording like the menu

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo running; Teams signed in; a call you can join

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl arm` | The menu-bar icon shows armed |
| 2 | Start the Teams call | The icon turns to recording |
| 3 | After a minute run `echoctl stop` | The recording stops; the icon goes idle; the call appears in Calls |

## Expected Final State
The terminal arms and stops a recording exactly as the menu does.

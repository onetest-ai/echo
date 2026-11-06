---
id: TC-005
title: echoctl doctor reports permissions and the model from the running app
priority: low
type: functional
requirements: [F-CLI-11, F-SET-17]
module: cli
run: person
platform: macos
tags: [cli, doctor]
external_id: TC-CLI-003
---

# TC-005: echoctl doctor reports permissions and the model from the running app

**Module:** CLI | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo running

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `echoctl doctor` | Each permission (Microphone, System Audio Recording, Accessibility, Screen Recording) and the speech model, as the running app sees them |
| 2 | Compare with the menu's Health | The same states |

## Expected Final State
doctor answers from the app, matching its Health menu.

---
id: TC-002
title: /echo:doctor reports a healthy setup and explains a missing permission
priority: high
type: functional
requirements: [F-CC-08]
module: claude
run: person
platform: macos
tags: [plugin, doctor]
external_id: TC-CLA-002
---

# TC-002: /echo:doctor reports a healthy setup and explains a missing permission

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- A machine where Echo works

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `/echo:doctor` | One line that Echo is fine; nothing asked or downloaded |
| 2 | Turn off Echo's Microphone permission in System Settings; run `/echo:doctor` again | Claude names the missing permission, opens its settings page and says what to click; it never changes the setting itself |

## Expected Final State
doctor is quiet when all is well and guides, without acting, when a permission is missing.

## Notes
- Stays person-run: it needs a signed-in Claude Code session with the plugin and a permission switched off in System Settings. `echoctl doctor` from the app is `cli/TC-005` (agent).

---
id: TC-001
title: Permission prompts come at first launch and name Echo
priority: critical
type: functional
module: permissions
run: manual
platform: macos
tags: [permissions]
external_id: TC-PERM-001
---

# TC-001: Permission prompts come at first launch and name Echo

**Module:** Permissions | **Priority:** Critical | **Type:** Functional

## Preconditions
- A fresh Mac account (tests/README.md → Test machines): no Echo settings, data folder or permissions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Finish the folder and model dialogs | The Microphone and System Audio Recording prompts appear now, not in the first call |
| 2 | Read each prompt; click Allow | Each names Echo (not Terminal, not echoctl) |
| 3 | System Settings → Privacy & Security | Echo under Microphone and under Screen & System Audio Recording (System Audio Recording Only) |

## Expected Final State
Echo asks for what it needs up front, under its own name.

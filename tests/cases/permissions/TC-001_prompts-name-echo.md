---
id: TC-001
title: Permission prompts come at first launch and name Echo
priority: critical
type: functional
requirements: [F-REC-14]
module: permissions
run: agent
platform: macos
tags: [permissions]
external_id: TC-PERM-001
user_steps:
  - 'the user reads each permission prompt and clicks Allow when the run asks (Microphone, System Audio Recording)'
---

# TC-001: Permission prompts come at first launch and name Echo

**Module:** Permissions | **Priority:** Critical | **Type:** Functional

## Preconditions
- A Mac where Echo has never been (app_profile.md → Clean-install runs): `tests/tools/clean-mac.sh` (dry run) finds nothing of Echo's; otherwise the run stops and asks the user to reset it
- The folder and model questions answered (install/TC-002, TC-004)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | After the folder and model dialogs, wait for the permission prompts | The Microphone and System Audio Recording prompts appear now, not in the first call |
| 2 | **The user does:** read each prompt and click Allow when the run asks; the user tells the run what each prompt names | Each names Echo (not Terminal, not echoctl) |
| 3 | `$E doctor --json`; System Settings → Privacy & Security, opened read-only with `open "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"` (and `…Privacy_ScreenCapture`): the user confirms what it lists | doctor has microphone and system audio granted; Echo is under Microphone and under Screen & System Audio Recording (System Audio Recording Only) |

## Expected Final State
Echo asks for what it needs up front, under its own name.

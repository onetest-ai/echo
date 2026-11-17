---
id: TC-036
title: "Claude's permissions: each of the three settings is saved as shown, from the window or from `echoctl config`, and the window shows what the CLI set"
priority: high
type: functional
requirements: [F-SET-24]
module: settings
run: agent
platform: any
data: standard
tags: [settings, claude, permissions, cli]
---

# TC-036: Claude's permissions are saved as shown, from the window and the CLI

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data (`tests/fixtures/serve.sh up <dir> <port> --set standard`), window served at `{{base_url}}`, `{{base_url}}/` loaded first (the cookie); the data has Claude in the window on "Ask each time" and routines and Yolo unset
- CLI steps in Bash after `. <dir>/env`; `E=target/debug/echoctl`, built right before the run; `C="$ECHOCTL_HOME/.echo/config.json"`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- After a reload, read the buttons once Settings has its values (wait for "Claude's permissions" and a moment more: the page first shows what it had, then what `config.json` holds)
- The groups of buttons are named "<row>: permissions" ("Claude in the window: permissions", "Routines acting on their own: permissions", "Yolo acting on its own: permissions")

**Changes:** `config.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; in the card "Claude's permissions" click "Auto" of "Claude in the window"; then `grep -E '"(folder\|routine\|yolo)_permissions"' "$C"` | "Auto" is pressed, "Ask each time" no longer; the file holds only `"folder_permissions": "auto",` |
| 2 | Click "Skip all permission checks" of "Yolo acting on its own"; `grep -E '"(routine\|yolo)_permissions"' "$C"` | It is pressed and its row shows "Skips every check."; the routines' row still has "Auto" pressed; the file holds `"yolo_permissions": "skip"` and no `routine_permissions` |
| 3 | `$E config --routine-permissions skip \| grep -E '"(folder\|routine\|yolo)_permissions"'`; reload `{{base_url}}/#settings/ai` | It prints `"folder_permissions": "auto"`, `"routine_permissions": "skip"`, `"yolo_permissions": "skip"`; in the window "Skip all permission checks" is pressed for routines too |

## Expected Final State
Claude in the window on Auto, routines and Yolo on Skip all permission checks, all three written in `config.json`.

## Teardown
- `$E config --folder-permissions ask > /dev/null` (as the data had it); on its own: `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `grep`.
- The three rows, their defaults and flags: `settings/TC-014`. That each setting reaches the command Claude Code starts with is `tasks/TC-030` (Yolo) and `plugins/TC-038` (a routine); the Rust tests `folder_args::each_session_starts_with_its_own_permission_setting_auto_by_default` and `claudejob::a_routine_and_yolo_acting_on_their_own_each_run_in_their_own_permission_mode_auto_by_default`.

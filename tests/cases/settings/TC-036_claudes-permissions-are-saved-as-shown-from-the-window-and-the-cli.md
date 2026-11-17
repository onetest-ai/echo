---
id: TC-095
title: "Claude's permissions: each of the three settings is saved as shown, from the window or `echoctl config`, never from inside a run; the old switch (`folder_skip_permissions`) on becomes Skip all permission checks, off becomes Auto; what Auto gets on each profile is shown"
priority: high
type: functional
requirements: [F-SET-24]
module: settings
run: agent
platform: any
data: standard
tags: [settings, claude, permissions, cli]
---

# TC-095: Claude's permissions are saved as shown, by the window and the CLI, and the old switch moves

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
| 4 | `$E config --routine-permissions ask; echo "exit $?"`; `ECHOCTL_HEADLESS=1 $E config --yolo-permissions auto; echo "exit $?"`; `grep -E '"(routine\|yolo)_permissions"' "$C"` | The first is refused by its own options (`invalid value 'ask'` … `[possible values: auto, skip]`, exit 2): nobody answers a routine's prompt; the second (as inside a run Echo started) says `refused inside a run Echo started: how Claude handles permissions is the user's to choose`, exit 1; both still `"skip"` |
| 5 | The old switch, on: `python3 -c "import json,sys; p=sys.argv[1]; c=json.load(open(p)); [c.pop(k, None) for k in ('folder_permissions','routine_permissions','yolo_permissions')]; c['folder_skip_permissions']=True; json.dump(c, open(p,'w'), indent=2)" "$C"`; reload `{{base_url}}/#settings/ai` | Claude in the window: "Skip all permission checks" pressed, its row shows "Skips every check."; routines and Yolo: "Auto" pressed (their default) |
| 6 | `$E config --onedrive none > /dev/null` (any save); `grep -E '"folder_(skip_)?permissions"' "$C"` | Only `"folder_permissions": "skip",`: the user's old choice is kept, the old key is gone |
| 7 | The old switch, off: the same `python3 -c …` as step 5 with `c['folder_skip_permissions']=False`; reload `{{base_url}}/#settings/ai` | Claude in the window: "Auto" pressed (the default after the update); routines and Yolo: "Auto" pressed |
| 8 | Read the card's last line; `$E config --yolo-permissions auto 2>&1 >/dev/null` | The line starts "Auto on your profiles:" and names each profile with what Auto gets there: "Claude Code (as configured)", "Claude · work", "Claude · other" and "Claude · out" — "auto mode: Claude Code decides each action"; the local model "Qwen · this computer" — "edits accepted: this profile's model has no auto mode" (auto mode needs a recent Claude model); the CLI prints the same, one `Auto on <profile>: …` line each |

## Expected Final State
Claude in the window on Auto (unset), routines and Yolo on Auto (unset), the old key `folder_skip_permissions: false` still in the file until the next save.

## Teardown
- `$E config --folder-permissions ask > /dev/null` (as the data had it); `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `grep`, `echo` and `python3 -c` (writing the old switch into the run's own `config.json`, as an Echo before this setting left it).
- That each setting reaches the command Claude Code starts with is `tasks/TC-030` (Yolo) and `plugins/TC-038` (a routine); the Rust tests `folder_args::each_session_starts_with_its_own_permission_setting_auto_by_default` and `claudejob::a_routine_and_yolo_acting_on_their_own_each_run_in_their_own_permission_mode_auto_by_default`.

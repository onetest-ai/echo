---
id: TC-038
title: "Claude's permissions after the update: the old switch (`folder_skip_permissions`) on becomes Skip all permission checks for the window, off becomes Auto, routines and Yolo start on Auto, and the old key is dropped at the next save"
priority: high
type: functional
requirements: [F-SET-24]
module: settings
run: agent
platform: any
data: standard
tags: [settings, claude, permissions, update]
---

# TC-038: The old switch on becomes Skip all permission checks, off becomes Auto, and its key goes

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data (`tests/fixtures/serve.sh up <dir> <port> --set standard`), window served at `{{base_url}}`, `{{base_url}}/` loaded first (the cookie)
- CLI steps in Bash after `. <dir>/env`; `E=target/debug/echoctl`, built right before the run; `C="$ECHOCTL_HOME/.echo/config.json"`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- After a reload, read the buttons once Settings has its values (wait for "Claude's permissions" and a moment more: the page first shows what it had, then what `config.json` holds)
- `OLD` is the Python line that writes `config.json` as an Echo before this setting left it — the three new keys gone, the old switch set to `V`: `python3 -c "import json,sys; p=sys.argv[1]; c=json.load(open(p)); [c.pop(k, None) for k in ('folder_permissions','routine_permissions','yolo_permissions')]; c['folder_skip_permissions']=sys.argv[2]=='on'; json.dump(c, open(p,'w'), indent=2)" "$C" V`

**Changes:** `config.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | The old switch on: `OLD` with `V` = `on`; reload `{{base_url}}/#settings/ai` | Claude in the window: "Skip all permission checks" pressed, its row shows "Skips every check."; routines and Yolo: "Auto" pressed (their default) |
| 2 | `$E config --onedrive none > /dev/null` (any save); `grep -E '"folder_(skip_)?permissions"' "$C"` | Only `"folder_permissions": "skip",`: the user's old choice is kept, the old key is gone |
| 3 | The old switch off: `OLD` with `V` = `off`; reload `{{base_url}}/#settings/ai` | Claude in the window: "Auto" pressed (the default after the update); routines and Yolo: "Auto" pressed |
| 4 | `$E config --onedrive none > /dev/null`; `grep -c -E '"(folder_skip\|folder\|routine\|yolo)_permissions"' "$C"` | `0`: the old key is gone and nothing is written for the three: each on its default, Auto |

## Expected Final State
All three settings on Auto (unset), no `folder_skip_permissions` in `config.json`.

## Teardown
- `$E config --folder-permissions ask > /dev/null` (as the data had it); on its own: `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `grep` and `python3 -c` (writing the old switch into the run's own `config.json`).
- The Rust test `config::permissions_default_to_auto_and_the_old_switch_on_stays_skip`.

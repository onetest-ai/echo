---
id: TC-030
title: Echo knows the folder OneDrive keeps on this computer; a folder OneDrive has online but doesn't sync here is never made here by Claude in Echo, and `m365 drive where` says where a path is
priority: high
type: functional
requirements: [F-SET-21, F-M365-09]
module: settings
run: agent
platform: macos
data: standard
tags: [settings, onedrive, microsoft-365, hooks, data-safety]
---

# TC-030: OneDrive on this computer

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `ECHOCTL_TEST_ONEDRIVE_ONLINE="$(printf 'Projects\nProjects/Acme\nShared')" tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (a development build answers "is it online?" from that list instead of Microsoft 365: here `Projects`, `Projects/Acme` and `Shared` are online); the window at `{{base_url}}`, `{{base_url}}/` loaded first
- Every Bash call starts with `. <dir>/env; export ECHOCTL_TEST_ONEDRIVE_ONLINE="$(printf 'Projects\nProjects/Acme\nShared')"; OD="$(dirname "$ECHOCTL_HOME")/OneDrive-Acme"`; `echoctl` is `target/debug/echoctl`
- A made-up OneDrive folder: `mkdir -p "$OD/Shared" "$OD/Notes"; printf x > "$OD/Notes/a.md"` — `Shared` is here and online; `Notes` only here; `Projects` only online (not synced to this computer)

**Changes:** `<config>/config.json` (`onedrive`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl config --onedrive "$OD"`; open `{{base_url}}/#settings/calendar` | Exit 0; Microsoft 365 has a row "OneDrive on this computer" with the folder's path, the line "Claude in Echo doesn't make a folder there that OneDrive keeps online but doesn't sync to this computer.", buttons "Choose…", "Find it" and "Off" |
| 2 | `echoctl m365 drive where "$OD/Projects/Acme/Rollout"`; `echoctl m365 drive where "$OD/Shared"`; `echoctl m365 drive where "$OD/Notes"`; `echoctl m365 drive where /tmp/x` | 1st: `in_onedrive` true, `online_path` "Projects/Acme/Rollout", `here` false, `online` true, `not_synced_here.online_path` "Projects" (the first of its folders missing here is online), `advice` "It is in OneDrive online but not on this computer… Don't create it here…"; 2nd: `here` true, `online` true, `advice` "…work with it here, on this computer (faster than online)."; 3rd: `here` true, `online` false, `advice` "…isn't uploaded…"; 4th: `in_onedrive` false |
| 3 | `printf '{"tool_name":"Bash","tool_input":{"command":"mkdir -p %s/Projects/Acme/Rollout"},"cwd":"/tmp"}' "$OD" \| echoctl onedrive-hook; echo "exit=$?"`; then the same with `"$OD/Notes/New"` and with a Write `{"tool_name":"Write","tool_input":{"file_path":"<OD>/Shared/plan.md","content":"x"}}` | 1st: exit 2 and the message "Not here: “…/OneDrive-Acme/Projects” is in the user's OneDrive online but OneDrive doesn't sync it to this computer…Choose folders…"; 2nd and 3rd: exit 0 (a new folder under one that is here; a write into a synced folder) |
| 4 | `ls "$OD"` | `Notes`, `Shared` only: no `Projects` was made |
| 5 | In the window click "Off"; `echoctl m365 drive where "$OD/Shared"`; then click "Find it" | The row says "Off: Echo treats no folder here as OneDrive."; `in_onedrive` false with "no OneDrive folder on this computer is known"; after "Find it" the row shows the folder Echo finds itself (the test Mac's own `~/Library/CloudStorage/OneDrive-…` if it has exactly one, else "Not found on this computer.") |
| 6 | `echoctl config --onedrive "$OD"`; `ECHOCTL_HEADLESS=1 echoctl config --onedrive none` | The second is refused: "where OneDrive is is the user's to say" |

## Expected Final State
OneDrive's folder is set to `$OD`; nothing named `Projects` was made in it.

## Teardown
- `echoctl config --onedrive auto`; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the features' text (F-SET-21, F-M365-09), not from the code.
- With Microsoft 365 signed in (a person's run), the same without `ECHOCTL_TEST_ONEDRIVE_ONLINE`: a folder of their OneDrive not chosen in OneDrive → Choose folders is refused the same way.

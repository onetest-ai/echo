---
id: TC-029
title: A Claude Code policy file Echo can't read asks instead of only refusing — a profile's own file offers Claude Code (as configured), in the window and the terminal; a managed file only the admin may read offers to write anyway; a broken managed file says Claude Code won't start with it
priority: high
type: functional
requirements: [F-AI-06, F-CC-02]
module: claude
run: agent
platform: any
data: standard
tags: [claude, minutes, policy, profiles]
---

# TC-029: A policy file Echo can't read asks

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; every `echoctl` command is `target/debug/echoctl …` after `. <dir>/env`
- `F="$ECHOCTL_CONFIG_DIR/claude/work/remote-settings.json"` (server-managed settings cached in Claude · work's folder); `mkdir -p "$(dirname "$F")"`; `echo 'not json' > "$F"`
- `<call>` is `$(ls -d $ECHOCTL_HOME/calls/*-initrode-intro)` ("Initrode intro": transcribed, no minutes, profile Claude · work)

**Changes:** `$F`; the call's `meta.json` (its profile), `notes-run.json`, `claude.log`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl notes <call> --wait; echo "exit $?"` | Refused: the policy sentence ending `(<$F> can't be read)`, then `To write them on Claude Code (as configured) instead: echoctl notes '<call name>' --profile system --wait`; `exit 1`; the call is still on Claude · work (`echoctl call` / `meta.json` `ai_profile` = `work`): nothing switched by itself |
| 2 | Open `{{base_url}}/#calls`, find "Initrode intro" (search) and open its Minutes | The failure is shown with a **Write on Claude Code (as configured)** button beside Try again |
| 3 | Click **Write on Claude Code (as configured)** | The minutes start on Claude Code (as configured): the call's profile is now `system`, the run's profile `system` (on test data the fake Claude Code then stops it: "test data: Claude Code is a fake here") |
| 4 | Put the call back on Claude · work (`echoctl notes <call> --profile work`, or the Model menu), then `target/debug/echoctl notes <call> --profile system --wait` | The same switch from the terminal: the run starts on `system` (the fake stops it) |

## Expected Final State
An unreadable policy file never switches the call's profile on its own; the user's click or `--profile` does.

## Teardown
- `rm -f "$F"`; `tests/fixtures/serve.sh down <dir>` and remove `<dir>`.

## Notes
- Managed files (`/Library/Application Support/ClaudeCode/managed-settings.json`, its `managed-settings.d`) need root, so they are person-run, on test data only, with `sudo`, removed right after:
  - a file only root may read (`sudo chmod 600`): the run is refused with "Claude Code can't read that file either, so it starts without it. To write them anyway: echoctl notes '<call>' --without-policy '<file>' --wait", the window shows **Write anyway**; after it, the run starts on the call's own profile, and later runs of that call don't ask again;
  - a file everyone can read that isn't JSON: refused with "Claude Code won't start while the policy file … is as it is … ask your admin to fix it", and no button (Claude Code refuses to start with it: code.claude.com/docs/en/managed-settings).
- The decision is the user's (#404).

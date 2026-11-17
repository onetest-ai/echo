---
id: TC-145
title: "Switched off, a plugin's open Claude session runs on as the user's, but once ended it doesn't start again"
priority: high
type: functional
requirements: [F-PLUG-93]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, terminal]
---

# TC-145: A plugin's Claude session when the plugin is switched off

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)

**Changes:** `<config dir>/plugins/` (Acme ask switched off and on), `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `open acme-ask '"path":"","mode":"read","profile":""'`; `$E plugin disable acme-ask`; `curl -s "$B/api/ptys"` | `\"ok\":true`; after the switch-off the session is still listed (`"key":"claude:plugin:acme-ask"`, `"exited":false`): it is the user's |
| 2 | `close; open acme-ask '"path":"","mode":"read","profile":""'`; `$E plugin enable acme-ask` | Starting it again is refused: `\"ok\":false`, `acme-ask isn't installed and on` |

## Expected Final State
Acme ask installed and on, with `$D/vault` given to it; no session open.

## Teardown
- `close`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `curl` and `python3` (the helpers).

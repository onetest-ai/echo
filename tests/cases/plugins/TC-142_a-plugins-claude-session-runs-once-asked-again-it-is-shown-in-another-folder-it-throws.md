---
id: TC-142
title: "A plugin has one Claude session at a time: asked again while it runs it is shown, not started twice; asked for another folder it throws"
priority: high
type: functional
requirements: [F-PLUG-92]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders, terminal]
---

# TC-142: A plugin's Claude session: once per plugin

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)

**Changes:** `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-ask:main`; click "Ask the vault"; read `#acme-ask` and the bottom panel | `opened`; the bottom panel's tab "Claude · Acme ask" shows the session |
| 2 | `rm "$D/run-args"`; click "Ask the vault" again; read `#acme-ask`; `ls "$D/run-args"` | `opened`; the tab "Claude · Acme ask" is shown; `ls` fails: the running session was shown, not started twice |
| 3 | Click "Ask Notes"; read `#acme-ask`; `ls "$D/run-args"` | `error: Claude is open for this plugin in another folder: close it in its panel first`; the tab "Claude · Acme ask" is shown, on the running session in `$D/vault`; `ls` fails |

## Expected Final State
Acme ask's one session open in `$D/vault`.

## Teardown
- `close`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `ls`, `rm`, `curl` and `python3` (the helpers).

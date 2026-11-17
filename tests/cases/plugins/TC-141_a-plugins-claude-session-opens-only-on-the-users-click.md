---
id: TC-141
title: "A plugin's Claude session opens only on the user's click: asked from a timer, past the click, it throws and nothing starts; asked on the click, it starts"
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

# TC-141: A plugin's Claude session: the user's click only

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`); `rm -f "$D/run-args"`

**Changes:** `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-ask:main`; click "Ask later" and wait 7 s; read `#acme-ask`; `ls "$D/run-args"`; `curl -s "$B/api/ptys"` | `error: Claude opens only on your click` (the plugin asked from a timer, past the click's activation); `ls` fails and the list has no `claude:plugin:acme-ask`: nothing was sent |
| 2 | Click "Ask the vault"; read `#acme-ask`; `ls "$D/run-args"`; `curl -s "$B/api/ptys"` | `opened`; `$D/run-args` is there; the list has `"key":"claude:plugin:acme-ask"`, `"exited":false` |

## Expected Final State
Acme ask's session open in `$D/vault`.

## Teardown
- `close`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `ls`, `rm`, `curl` and `python3` (the helpers).
- The guard itself (`pluginSessionAsk` over `copyAllowed`, an engine that can't tell refusing) and that the plugin's `ctx` has no way to type into or read the session (no `pty_input`, `pty_paste` or `api/pty/` in `pluginCtx`): the node test `plugin-session.test.mjs`.

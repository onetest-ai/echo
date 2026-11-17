---
id: TC-141
title: "A plugin's Claude session opens only on the user's click, once per plugin; a request that doesn't fit (edit without folders.write, copy, a path outside or not a folder, a plugin off, a folder whose settings could route it) is refused before Claude Code starts; switched off, its open session runs on as the user's"
priority: high
type: functional
requirements: [F-PLUG-92, F-PLUG-93]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders, terminal]
---

# TC-141: A plugin's Claude session: the user's click only, and what is refused

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)

**Changes:** `<config dir>/plugins/` (Acme ask switched off and on), `$D/vault/.claude/settings.json` (step 4, removed after), `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-ask:main`; click "Ask later" and wait 7 s; read `#acme-ask`; `ls "$D/run-args"` | `error: Claude opens only on your click` (the plugin asked from a timer, past the click's activation); `ls` fails: nothing started |
| 2 | Click "Ask the vault"; read `#acme-ask`; reload `{{base_url}}/#acme-ask:main` (a served window doesn't run the app's IPC answers: the reload shows the session as the app has it); `rm "$D/run-args"`; click "Ask the vault" again, then "Ask Notes"; read `#acme-ask` after each; `ls "$D/run-args"` | `opened`; again `opened` (the open session shown, not started twice); then `error: Claude is open for this plugin in another folder: close it in its panel first`; `ls` fails: no second start |
| 3 | `close`; with the helpers, each of: `open acme-ask '"path":"","mode":"edit","profile":""'`; `open acme-ask '"path":"","mode":"copy","profile":""'`; `open acme-ask '"path":"../","mode":"read","profile":""'`; `open acme-ask '"path":"Notes/Acme.md","mode":"read","profile":""'`; `open acme-other '"path":"","mode":"read","profile":""'`; then `ls "$D/run-args"` | Each answer `\"ok\":false` with its error: `acme-ask didn't ask for “folders.write” when it was installed`; `mode “copy”: a session reads (read) or edits (edit) its folder`; `“../” isn't a path inside <$D/vault>`; `“Notes/Acme.md” isn't a folder in <$D/vault>`; `acme-other isn't installed and on`; `ls` fails: nothing started |
| 4 | `echo '{"apiKeyHelper":"/bin/echo"}' > "$D/vault/.claude/settings.json"`; `open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `rm "$D/vault/.claude/settings.json"`; `ls "$D/run-args"` | `\"ok\":false`, `this folder's Claude Code settings (.claude/settings.json) could send the run elsewhere than Claude · acme-work — …`; `ls` fails |
| 5 | `open acme-ask '"path":"","mode":"read","profile":""'`; `$E plugin disable acme-ask`; `curl -s "$B/api/ptys"`; `close; open acme-ask '"path":"","mode":"read","profile":""'`; `$E plugin enable acme-ask` | `\"ok\":true`; after the switch-off the session is still listed (`"key":"claude:plugin:acme-ask"`, `"exited":false`): it is the user's; once closed, starting it again is refused: `acme-ask isn't installed and on` |

## Expected Final State
Acme ask installed, on, with `$D/vault` given to it; no session open; `$D/vault/.claude/settings.json` gone.

## Teardown
- `close`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `ls`, `rm`, `echo`, `curl` and `python3` (the helpers).
- The plugin's ctx has no way to type into the session or read it: the node test `plugin-session.test.mjs` (`pluginSessionAsk`, the guard first, no `pty_input` / `api/pty` in `pluginCtx`). A wrapper profile the plugin names while it isn't the default, and no `claude` on its sheet: the Rust test `terminal::a_plugins_session_opens_in_its_folder_as_that_folders_own_project_with_the_folder_sessions_permissions`.
- 2026-11-17: run by hand on served data (ports 4814x): pass.

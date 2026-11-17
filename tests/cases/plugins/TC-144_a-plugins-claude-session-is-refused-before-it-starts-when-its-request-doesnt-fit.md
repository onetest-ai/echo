---
id: TC-144
title: "A plugin's Claude session that doesn't fit is refused before Claude Code starts: edit without folders.write, copy, a path outside or not a folder, a plugin not installed, a folder whose settings could route it"
priority: high
type: functional
requirements: [F-PLUG-93]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders, terminal]
---

# TC-144: A plugin's Claude session: what is refused

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)

**Changes:** `$D/vault/.claude/settings.json` (step 2, removed after), `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | With the helpers, each of: `open acme-ask '"path":"","mode":"edit","profile":""'`; `open acme-ask '"path":"","mode":"copy","profile":""'`; `open acme-ask '"path":"../","mode":"read","profile":""'`; `open acme-ask '"path":"Notes/Acme.md","mode":"read","profile":""'`; `open acme-other '"path":"","mode":"read","profile":""'`; then `ls "$D/run-args"` | Each answer `"ok":false` with its error: `acme-ask didn't ask for “folders.write” when it was installed`; `mode “copy”: a session reads (read) or edits (edit) its folder`; `“../” isn't a path inside <$D/vault>`; `“Notes/Acme.md” isn't a folder in <$D/vault>`; `acme-other isn't installed and on`; `ls` fails: nothing started |
| 2 | `echo '{"apiKeyHelper":"/bin/echo"}' > "$D/vault/.claude/settings.json"`; `open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `rm "$D/vault/.claude/settings.json"`; `ls "$D/run-args"` | `"ok":false`, `this folder's Claude Code settings (.claude/settings.json) could send the run elsewhere than Claude · acme-work — …`; `ls` fails |

## Expected Final State
Acme ask installed with `$D/vault` given to it; no session open; `$D/vault/.claude/settings.json` gone.

## Teardown
- `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `ls`, `rm`, `echo`, `curl` and `python3` (the helpers).
- No `claude` or `folders` on its sheet, a folder not given, and a wrapper profile the plugin names while it isn't the default: the Rust test `terminal::a_plugins_session_opens_in_its_folder_as_that_folders_own_project_with_the_folder_sessions_permissions`.

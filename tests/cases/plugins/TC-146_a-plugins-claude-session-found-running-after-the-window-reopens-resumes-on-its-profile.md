---
id: TC-146
title: "A plugin's Claude session found running when the window opens gets its tab back, and its Resume keeps the profile it ran on: the same conversation, not the default profile's"
priority: medium
type: functional
requirements: [F-PLUG-94]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, terminal, ai-profiles]
---

# TC-146: A plugin's Claude session found running resumes on its profile

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work` — not the default, the fake `claude` writing `$D/run-args`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)

**Changes:** `<home>/.echo/sessions.json`, `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `grep -E -A1 '^--(resume\|session-id)$' "$D/run-args"`; note the id | `"ok":true` with `"profile":"acme-work"` in its `about`; `--session-id` or `--resume` with a `<uuid>` (`--resume` after TC-140) |
| 2 | Reload `{{base_url}}/` (the window reopened: the session found running); open the bottom panel with the status bar's "Claude" if it is closed; click the tab "Claude · Acme ask"; read the panel | `Acme ask · Claude Code in <$D/vault>, as that folder's own project — it reads only; asks each time · <acme-work's name>` above its terminal (`acme fake session in <$D/vault>`) |
| 3 | `close`; wait until the panel says `The session ended. Resume picks up the conversation where it stopped.`; `rm "$D/run-args"`; click "Resume"; wait 2 s; `grep -E -A1 '^--(resume\|session-id)$' "$D/run-args"` | `--resume` with step 1's uuid and no `--session-id`: started again on `acme-work`, the same conversation (on the default profile it would be another one) |

## Expected Final State
Acme ask's session open on `acme-work`, its conversation of step 1.

## Teardown
- `close`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `grep`, `rm`, `curl` and `python3` (the helpers).
- Echo checks the profile again at the start (a wrapper profile that isn't the default is refused): the Rust test `terminal::a_plugins_session_opens_in_its_folder_as_that_folders_own_project_with_the_folder_sessions_permissions`; what Resume asks for from what the app says of the session (`aboutExtra`): the node test `plugin-session.test.mjs`.

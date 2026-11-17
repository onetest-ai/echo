---
id: TC-147
title: "⌘K starts a new conversation in a plugin's Claude session, asking first while it runs; the new one is resumed after that"
priority: medium
type: functional
requirements: [F-PLUG-94]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, terminal, palette]
---

# TC-147: A new conversation in a plugin's Claude session

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude` writing `$D/run-args`, the folder `$D/vault`, the plugin Acme ask, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)

**Changes:** `<home>/.echo/sessions.json`, `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `grep -E -A1 '^--(resume\|session-id)$' "$D/run-args"`; note the id | `"ok":true`; `--session-id` or `--resume` with a `<uuid>` |
| 2 | Reload `{{base_url}}/`; open Actions (`[data-pal-open]`), type `Claude · Acme ask`; read the list | Under CLAUDE: `Restart Claude · Acme ask` and `New conversation in Claude · Acme ask` |
| 3 | Run `New conversation in Claude · Acme ask` (Enter on it); read the bottom panel | The tab "Claude · Acme ask" shown, asking `Start a new conversation? This session stops, and whatever it is doing now is lost.` with "Keep it running" and "Start new" |
| 4 | `rm "$D/run-args"`; click "Start new"; wait 3 s; `grep -E -A1 '^--(resume\|session-id)$' "$D/run-args"`; `curl -s "$B/api/ptys"` | `--session-id` with a new `<uuid>`, not step 1's, and no `--resume`; the list has `"key":"claude:plugin:acme-ask"`, `"exited":false` (one session, the old one stopped) |
| 5 | `close; open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `grep -A1 '^--resume$' "$D/run-args"` | `--resume` with step 4's uuid: the new conversation is the one kept |

## Expected Final State
Acme ask's session open on `acme-work`, in the conversation step 4 started.

## Teardown
- `close`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `grep`, `rm`, `curl` and `python3` (the helpers).
- A new conversation's id and the resume after it: also the Rust test `terminal::a_plugins_session_opens_in_its_folder_as_that_folders_own_project_with_the_folder_sessions_permissions`.

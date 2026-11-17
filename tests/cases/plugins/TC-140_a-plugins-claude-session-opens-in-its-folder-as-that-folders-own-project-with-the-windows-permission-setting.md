---
id: TC-140
title: "A plugin's window half opens a Claude Code session in Echo's terminal, on the user's click, in a folder the user gave it — as that folder's own project, read-only tools, no MCP server, its conversation resumed, and the permissions the user set for Claude in the window (Ask each time, Auto, Skip all permission checks)"
priority: high
type: functional
requirements: [F-PLUG-90, F-PLUG-91]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, folders, permissions, terminal]
---

# TC-140: A plugin's Claude session in its folder

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Test data served: `tests/fixtures/serve.sh up <dir> <port>` (in S-37, the scenario's own), `{{base_url}}/` loaded first (its cookie); CLI steps in Bash, each starting with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; E=target/debug/echoctl` (inside the repository, built right before the run; a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- A Claude AI profile of Echo's own on the data's fake (never a real account): `$E ai add-claude acme-work --email sam@initech.example`
- **A fake `claude` that writes down how Echo starts it, then waits** (a session that stays open), in front of the data's fake: `mv "$D/bin/claude" "$D/bin/claude.fake"`, then write `$D/bin/claude` (`chmod +x`):
  ```sh
  #!/bin/sh
  # --version and auth go to the data's fake; a session writes its arguments one a line and its folder, then waits.
  d="$(dirname "$0")/.."
  case "$1" in --version|auth) exec "$d/bin/claude.fake" "$@";; esac
  printf '%s\n' "$@" > "$d/run-args"; pwd > "$d/run-cwd"
  echo "acme fake session in $(pwd)"
  exec sleep 600
  ```
- A made-up folder and the made-up plugin **Acme ask** (Claude, a folder it reads; a window half with buttons that open its session) in a local marketplace `acme-ask`:

```bash
mkdir -p "$D/vault/Notes" "$D/vault/.claude"
printf '# Acme vault\n' > "$D/vault/CLAUDE.md"; printf '# Acme\n' > "$D/vault/Notes/Acme.md"
M="$D/market-ask"; mkdir -p "$M/plugins/acme-ask"
echo '{"name":"acme-ask","plugins":[{"id":"acme-ask","source":"./plugins/acme-ask"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-ask","name":"Acme ask","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js","uses":{"claude":{},"folders":{"what":"notes"}}}' > "$M/plugins/acme-ask/echo-plugin.json"
cat > "$M/plugins/acme-ask/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('your notes'))
  ctx.command('folder', () => ctx.fs.folders()[0])
}
JS
cat > "$M/plugins/acme-ask/client.js" <<'JS'
export function apply(ctx) {
  let said = '—', root = ''
  ctx.call('folder').then((f) => { root = f || ''; ctx.render() })
  const ask = (o) => { try { ctx.claude.session({ folder: root, ...o }); said = 'opened' } catch (e) { said = `error: ${e.message}` } ctx.render() }
  ctx.listen('click', (ev) => {
    const b = ev.target.closest('[data-acme-ask]')
    if (!b) return
    const o = JSON.parse(b.dataset.acmeAsk)
    if (o.later) setTimeout(() => ask({}), 6000); else ask(o)
  })
  const btn = (label, o) => `<button class="btn" data-acme-ask='${ctx.ui.esc(JSON.stringify(o))}'>${ctx.ui.esc(label)}</button>`
  ctx.activities.add({ id: 'main', label: 'Acme ask', icon: '', render: () => `<h2>Acme ask</h2>${btn('Ask the vault', {})}${btn('Ask Notes', { path: 'Notes' })}${btn('Edit the vault', { mode: 'edit' })}${btn('Ask outside', { path: '../' })}${btn('Ask later', { later: true })}<p id="acme-ask">${ctx.ui.esc(said)}</p>` })
}
JS
$E plugin marketplace add "$M" --name acme-ask
```

- The IPC helpers for the CLI steps (the served window's own IPC, as the page sends it; `B` is `{{base_url}}`):
  ```bash
  curl -s -c "$D/jar" "$B/" > /dev/null
  ipc() { curl -s -b "$D/jar" -H "Origin: $B" -X POST --data "$1" "$B/ipc"; echo; }
  close() { for id in $(curl -s "$B/api/ptys" | python3 -c "import json,sys;print(' '.join(str(t['id']) for t in json.load(sys.stdin) if 'id' in t))"); do ipc "{\"cmd\":\"pty_close\",\"id\":$id}" > /dev/null; done; sleep 1.5; }
  open() { rm -f "$D/run-args"; ipc "{\"cmd\":\"pty_open\",\"kind\":\"claude-plugin\",\"key\":\"claude:plugin:$1\",\"plugin\":\"$1\",\"folder\":\"$D/vault\",$2,\"cols\":80,\"rows\":20}"; sleep 1; }
  ```

**Changes:** `<config dir>/ai.json` (the profile), `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `config.json` (`folder_permissions`), `<home>/.echo/sessions.json`, `$D/run-*`, `$D/jar`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install acme-ask --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" $E plugin call acme-ask add`; `$E config --folder-permissions ask > /dev/null` | The sheet's Claude line ends `on your click in its screens it opens Claude Code there in Echo's Claude panel for you to ask, as you set Claude in the window`; `Installed Acme ask 0.1.0`; the folder's path |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-ask:main`; click "Ask the vault"; then reload `{{base_url}}/#acme-ask:main` (a served window doesn't run the app's IPC answers: the reload shows the session as the app has it) and read the bottom panel | `#acme-ask` read `opened` before the reload; the bottom panel has the tabs "Claude" and "Claude · Acme ask", on the latter: `Acme ask · Claude Code in <$D/vault>, as that folder's own project — it reads only; asks each time · Claude Code (as configured)` above a terminal whose first line is `acme fake session in <$D/vault>` |
| 3 | `cat "$D/run-cwd"`; `grep -E -A1 '^--(tools\|permission-mode\|setting-sources\|session-id)$' "$D/run-args"`; `grep -c -E '^--(dangerously-skip-permissions\|settings\|plugin-dir\|mcp-config\|allowedTools\|add-dir\|no-session-persistence)$' "$D/run-args"`; `grep -c '^--strict-mcp-config$' "$D/run-args"` | `$D/vault` (its real path); `--session-id` `<uuid>`, `--tools` `Read,Grep,Glob,Skill`, no `--permission-mode` (Ask each time: Claude Code's own prompts), no `--setting-sources` (the user's own Claude Code loads the folder's project anyway); `0` (nothing of Echo's session, prompts not skipped, the conversation kept); `1` |
| 4 | With the helpers: `close; open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `grep -E -A1 '^--(session-id\|setting-sources)$' "$D/run-args"`; note the id; `close; open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `grep -A1 '^--resume$' "$D/run-args"` | Each answer holds `\"ok\":true` and the plugin's `about` (`\"name\":\"Acme ask\"`, `\"permissions\":\"asks each time\"`); `--session-id` `<uuid>` and `--setting-sources` `user,project` as the last two lines (the folder's project on a profile of Echo's own); the second start `--resume` with that same uuid |
| 5 | `$E config --folder-permissions auto > /dev/null`; `close; open acme-ask '"path":"Notes","mode":"read","profile":"acme-work"'`; `cat "$D/run-cwd"`; `grep -E -A1 '^--(permission-mode\|session-id)$' "$D/run-args"` | `$D/vault/Notes`; `--permission-mode` `auto`; a `--session-id` (a subfolder is a conversation of its own) |
| 6 | `$E config --folder-permissions skip > /dev/null`; `close; open acme-ask '"path":"","mode":"read","profile":"acme-work"'`; `grep -c '^--dangerously-skip-permissions$' "$D/run-args"`; `grep -c '^--permission-mode$' "$D/run-args"`; then `$E config --folder-permissions ask > /dev/null; close` | `1` and `0`: the user's setting for Claude in the window, and only it |

## Expected Final State
Acme ask installed with `$D/vault` given to it; no session open; the setting back on Ask each time.

## Teardown
- `$E plugin remove acme-ask --delete-data`; `$E plugin marketplace remove acme-ask`; `$E ai remove acme-work`; `mv "$D/bin/claude.fake" "$D/bin/claude"`; `rm -r "$D/vault" "$M" "$D/jar" "$D"/run-*`

## Notes
- Besides `echoctl`, this case runs `cat`, `grep`, `printf`, `mkdir`, `rm`, `mv`, `curl` (the served window's own IPC, as the page sends it) and `python3` (the open terminals' ids).
- The fake stands for Claude Code: what a real one loads from the folder and how it asks are Claude Code's own; this case checks how Echo starts it. Edit mode with `folders.write`, a model server's `acceptEdits` / `dontAsk`, and a wrapper profile are the Rust tests `terminal::a_plugins_session_opens_in_its_folder_as_that_folders_own_project_with_the_folder_sessions_permissions` and `claudejob::a_plugins_folder_session_is_that_folders_own_project_kept_for_resuming_with_the_permissions_it_is_given`.
- 2026-11-17: run by hand on served data (ports 4814x): pass.

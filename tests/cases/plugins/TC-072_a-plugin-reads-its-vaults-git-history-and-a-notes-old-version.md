---
id: TC-072
title: A plugin reads its vault's git history — where the repository is, a note's commits with the plugin named on its own, the note's text at an older version — and gets nothing for a version that isn't one
priority: high
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git]
---

# TC-072: A plugin reads its vault's history and an old version

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- git 2.31 or newer where the system installs it (`/usr/bin/git` with developer tools, or Homebrew's)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault and a made-up plugin **Acme history** (a folder it may change, a host half) in a local marketplace `acme-git` — the same block in TC-071 to TC-074:

```bash
mkdir -p "$D/history"
M="$D/market-git"; mkdir -p "$M/plugins/acme-history"
echo '{"name":"acme-git","plugins":[{"id":"acme-history","source":"./plugins/acme-history"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-history","name":"Acme history","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"folders":{"what":"vaults","write":true}}}' > "$M/plugins/acme-history/echo-plugin.json"
cat > "$M/plugins/acme-history/main.js" <<'JS'
export function apply(ctx) {
  const r = () => ctx.fs.folders()[0]
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('put', (a) => ctx.fs.write(r(), a.path, a.text))
  ctx.command('info', async () => await ctx.git.info(r()))
  ctx.command('init', async () => await ctx.git.init(r()))
  ctx.command('status', async () => await ctx.git.status(r()))
  ctx.command('commit', async (a) => await ctx.git.commit(r(), a))
  ctx.command('log', async (a) => await ctx.git.log(r(), a))
  ctx.command('show', async (a) => await ctx.git.show(r(), a.rev, a.path))
}
JS
echoctl plugin marketplace add "$M" --name acme-git
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/history/` (a repository), the plugin's store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-history --yes`; `ECHOCTL_PICK_FOLDER="$D/history" echoctl plugin call acme-history add`; `echoctl plugin call acme-history init`; `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot planned.\n"}'`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: first notes"}'` | `Installed Acme history 0.1.0`; the vault's path; its `info`; `null`; `{"rev": "<40 hex>"}` |
| 2 | `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\nPilot budget agreed.\n"}'`; `echoctl plugin call acme-history status`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: budget"}'` | `null`; the list holds `{"path": "Notes/Acme.md", "code": " M"}`; `{"rev": "<40 hex>"}` |
| 3 | `echoctl plugin call acme-history info` | `"repo": true`, `"branch": "main"`, `"head"` the 40 hex of step 2's commit, `"detached": false`, `"busy": null`, `"subfolder": false` |
| 4 | `echoctl plugin call acme-history log '{"path":"Notes/Acme.md"}'` | Two commits, newest first: subjects `Acme: budget`, `Acme: first notes`, each with `"echo": "acme-history"`, `"path": "Notes/Acme.md"`, an ISO `date` |
| 5 | `echoctl plugin call acme-history show '{"rev":"HEAD~1","path":"Notes/Acme.md"}'`; `echoctl plugin call acme-history show '{"rev":"HEAD","path":"Notes/None.md"}'` | `{"text": "# Acme\nPilot planned.\n", "size": 22, "truncated": false}` — the text as committed then; `null`: no such note at that version |
| 6 | `echoctl plugin call acme-history show '{"rev":"--output=x","path":"Notes/Acme.md"}'`; `echoctl plugin call acme-history show '{"rev":"main","path":"Notes/Acme.md"}'` | Both fail: `“--output=x” isn't a version: HEAD, HEAD~N or a commit's id`, then the same for `“main”` |

## Expected Final State
`$D/history/Notes/Acme.md` holds "Pilot budget agreed." and is committed; nothing else changed by the reads.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`

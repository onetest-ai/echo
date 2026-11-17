---
id: TC-086
title: A plugin gets no git in a repository it could lay out itself — a bare repository's files written in its own folder; once they are gone, a real .git works
priority: critical
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git, security]
---

# TC-086: No git in a repository a plugin could lay out

**Module:** Plugins | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- git 2.31 or newer where the system installs it (`/usr/bin/git` with developer tools, or Homebrew's) or on your `PATH`
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up empty vault and the made-up plugin **Acme history** (a folder it may change, a host half) in a local marketplace `acme-git` — the block of TC-071 to TC-074, its folder `$D/layout`:

```bash
mkdir -p "$D/layout"
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

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/layout/` (a repository), the plugin's store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-history --yes`; `ECHOCTL_PICK_FOLDER="$D/layout" echoctl plugin call acme-history add`; `echoctl plugin call acme-history info` | `Installed Acme history 0.1.0`; the vault's path; `{"repo": false}` |
| 2 | The plugin lays out a bare repository's files in its folder (none of them hidden): `echoctl plugin call acme-history put '{"path":"HEAD","text":"ref: refs/heads/main\n"}'`; `echoctl plugin call acme-history put "{\"path\":\"config\",\"text\":\"[core]\\n\\tbare = false\\n\\tworktree = $D/layout\\n\"}"`; `echoctl plugin call acme-history put '{"path":"objects/keep","text":""}'`; `echoctl plugin call acme-history put '{"path":"refs/heads/keep","text":""}'` | `null` four times: the plugin may write those files |
| 3 | `echoctl plugin call acme-history info`; `echoctl plugin call acme-history init`; `echoctl plugin call acme-history put '{"path":"Notes/Acme.md","text":"# Acme\n"}'`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"x"}'` | `info`, `init` and `commit` fail: `this folder is in a bare repository (its HEAD, objects and refs not in a .git): Echo doesn't use it`; `put` gives `null` |
| 4 | The user clears those files and the plugin starts a repository: `rm -r "$D/layout/HEAD" "$D/layout/config" "$D/layout/objects" "$D/layout/refs"`; `echoctl plugin call acme-history init`; `echoctl plugin call acme-history commit '{"paths":["Notes/Acme.md"],"message":"Acme: first notes"}'` | Its `info` (`"repo": true`, `"branch": "main"`); `{"rev": "<40 hex>"}` |

## Expected Final State
`$D/layout` holds a repository whose one commit is `Acme: first notes` by `Echo (acme-history)`.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`

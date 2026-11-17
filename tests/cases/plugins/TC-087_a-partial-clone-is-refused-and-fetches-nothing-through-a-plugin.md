---
id: TC-087
title: A vault's repository that is a partial clone — whose remote would fetch a missing object through a program — is refused when a plugin reads it, and nothing is fetched
priority: critical
type: functional
requirements: [F-PLUG-57]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, git, security]
---

# TC-087: A partial clone is refused and fetches nothing

**Module:** Plugins | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- git 2.31 or newer where the system installs it (`/usr/bin/git` with developer tools, or Homebrew's)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault and a made-up plugin **Acme history** (a folder it may change, a host half) in a local marketplace `acme-git` — the block of TC-071 to TC-074:

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
| 2 | The repository becomes a partial clone whose remote is a shell command: `printf '[extensions]\n\tpartialClone = origin\n[remote "origin"]\n\turl = %s\n\tpromisor = true\n[protocol "ext"]\n\tallow = always\n' "ext::sh -c touch% $D/fetched" >> "$D/history/.git/config"` | Nothing printed |
| 3 | `echoctl plugin call acme-history info`; `echoctl plugin call acme-history show '{"rev":"HEAD","path":"Notes/Acme.md"}'` | Both fail: `this repository is a partial clone, which fetches what it lacks from elsewhere: Echo doesn't use it` |
| 4 | `ls "$D/fetched"` | `No such file or directory`: nothing was fetched |

## Expected Final State
`$D/history` is refused as a partial clone; no `$D/fetched`.

## Teardown
- `echoctl plugin remove acme-history --delete-data`; `echoctl plugin marketplace remove acme-git`

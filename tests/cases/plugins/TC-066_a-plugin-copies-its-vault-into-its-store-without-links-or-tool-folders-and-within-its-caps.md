---
id: TC-066
title: A plugin copies its vault into its store's cache to work on — never through a link, never a .git folder at any depth (hidden files asked too), at most 100 excluded paths, 3 snapshots kept — and removes a copy with ctx.store.remove
priority: high
type: functional
requirements: [F-PLUG-49]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, store]
---

# TC-066: A plugin copies its vault into its store

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault, a folder beside it and a made-up plugin **Acme vault** (folders it may change, a host half) in a local marketplace `acme-local` — the same block in TC-064 to TC-067:

```bash
mkdir -p "$D/vault/.git" "$D/vault/Archive/.git" "$D/outside"
printf '[remote] pilot budget token\n' > "$D/vault/.git/config"
printf 'ref: pilot budget\n' > "$D/vault/Archive/.git/HEAD"
printf 'pilot budget of someone else\n' > "$D/outside/leak.md"
ln -s "$D/outside" "$D/vault/Out"
M="$D/market"; mkdir -p "$M/plugins/acme-vault"
echo '{"name":"acme-local","plugins":[{"id":"acme-vault","source":"./plugins/acme-vault"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-vault","name":"Acme vault","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"folders":{"what":"vaults","write":true}}}' > "$M/plugins/acme-vault/echo-plugin.json"
cat > "$M/plugins/acme-vault/main.js" <<'JS'
export function apply(ctx) {
  const r = () => ctx.fs.folders()[0]
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('new', (a) => ctx.fs.write(r(), a.path, a.text, { createNew: true }))
  ctx.command('put', (a) => ctx.fs.write(r(), a.path, a.text, { ifSha: a.ifSha }))
  ctx.command('hash', (a) => ctx.crypto.sha256(a.text))
  ctx.command('stat', (a) => ctx.fs.stat(r(), a.path, { sha: true }))
  ctx.command('log', (a) => ctx.fs.append(r(), a.path, a.text))
  ctx.command('mkdir', (a) => ctx.fs.mkdir(r(), a.path))
  ctx.command('find', async (a) => await ctx.fs.search(r(), a.q))
  ctx.command('snap', async (a) => { a = a || {}; return await ctx.fs.snapshot(r(), { hidden: !!a.hidden, exclude: Array.from({ length: a.n || 0 }, (_, i) => `x${i}`) }) })
  ctx.command('drop', (a) => ctx.store.remove(a.path))
  ctx.command('bump', () => ctx.store.update('count.txt', (t) => String(Number(t || 0) + 1)))
  ctx.command('count', () => ctx.store.read('count.txt'))
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/vault/`, the plugin's store `$ECHOCTL_HOME/.echo/plugins/acme-vault/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-vault --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-vault add` | `Installed Acme vault 0.1.0`; the vault's path |
| 2 | `echoctl plugin call acme-vault snap`; `find "$ECHOCTL_HOME/.echo/plugins/acme-vault/<its path>" -type f \| sort`; `find "$D/vault" -type f -not -path '*/.*' \| sort` | `{"path": "cache/snapshots/<16 hex>", "files": N, "bytes": …, "skipped": 0, "why": {"large": 0, "unreadable": 0}}` — no path of `$D` in it; the copy holds the same files (by their path under the vault) as the vault's own listing, `N` of them: no `.git`, nothing of `Out` |
| 3 | `echoctl plugin call acme-vault snap '{"hidden":true}'`; `find "$ECHOCTL_HOME/.echo/plugins/acme-vault/<its path>" -path '*.git*'` | A second snapshot; no output — neither `.git/` nor `Archive/.git/`, hidden files asked or not |
| 4 | `echoctl plugin call acme-vault snap '{"n":101}'` | It fails: `exclude names 100 paths at most` |
| 5 | `echoctl plugin call acme-vault snap` (a third); then `snap` again (a fourth) | A third snapshot; the fourth fails: `this plugin keeps 3 snapshots already: remove one (ctx.store.remove) first` |
| 6 | `echoctl plugin call acme-vault drop '{"path":"<each path>"}'` for the three; `ls -A "$ECHOCTL_HOME/.echo/plugins/acme-vault/cache/snapshots"` | `null` each; no output: the snapshots folder is empty |

## Expected Final State
The plugin's store holds no snapshot; the vault is unchanged.

## Teardown
- `echoctl plugin remove acme-vault --delete-data`; `echoctl plugin marketplace remove acme-local`

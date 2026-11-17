---
id: TC-067
title: A plugin's store updates from two processes at once (the app and a terminal's plugin call) both count — none lost
priority: high
type: functional
requirements: [F-PLUG-50]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, store]
---

# TC-067: Two processes' store updates both count

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
| 2 | Two processes at once: `(for i in $(seq 15); do echoctl plugin call acme-vault bump >/dev/null; done) & (for i in $(seq 15); do echoctl plugin call acme-vault bump >/dev/null; done); wait; echoctl plugin call acme-vault count` | `"30"` — no update lost |

## Expected Final State
The plugin's store holds `count.txt` reading `30`.

## Teardown
- `echoctl plugin remove acme-vault --delete-data`; `echoctl plugin marketplace remove acme-local`

---
id: TC-065
title: A plugin asks what a path of its vault is — a link reported as a link, its place relative to the vault, never a path outside — hashes in Rust and searches the vault's text, not hidden files or through a link; reads work while Claude prepares and asks
priority: high
type: functional
requirements: [F-PLUG-48]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, store]
---

# TC-065: A plugin asks what a path is and searches

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
printf '# Initech wiki\nMade-up notes for tests.\n' > "$D/vault/Wiki.md"
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
| 2 | `echoctl plugin call acme-vault new '{"path":"Research/Globex.md","text":"# Globex pilot\nPilot budget agreed.\n"}'`; `echoctl plugin call acme-vault stat '{"path":"Research/Globex.md"}'`; `echoctl plugin call acme-vault hash '{"text":"# Globex pilot\nPilot budget agreed.\n"}'` | `null`; `{"kind": "file", "size": 36, "real": "Research/Globex.md", "mtime": …, "sha": "<64 hex>"}`; the hash is that same `sha` |
| 3 | `echoctl plugin call acme-vault stat '{"path":"Research"}'`; `echoctl plugin call acme-vault stat '{"path":"Research/None.md"}'` | `{"kind": "dir", "real": "Research", …}` (no `sha`); `null` |
| 4 | `echoctl plugin call acme-vault stat '{"path":"Out"}'`; `echoctl plugin call acme-vault stat '{"path":"Out/leak.md"}'` | `{"kind": "link", "real": null, …}` — no `sha`, no path of `$D/outside` anywhere in it; the second fails: `“Out” leads outside …` (the folder it is in is a link out) |
| 5 | `echoctl plugin call acme-vault find '{"q":"PILOT BUDGET"}'` | Exactly `[{"path": "Research/Globex.md", "line": 2, "text": "Pilot budget agreed."}]` — any case, not `.git/config` or `Archive/.git/HEAD` (hidden), not `Out/leak.md` (through a link) |
| 6 | `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-vault find '{"q":"pilot budget"}'`; `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-vault stat '{"path":"Research/Globex.md"}'` | The same one line; the same stat: reads work while Claude prepares and asks |

## Expected Final State
`$D/vault/Research/Globex.md` is there; nothing else changed.

## Teardown
- `echoctl plugin remove acme-vault --delete-data`; `echoctl plugin marketplace remove acme-local`

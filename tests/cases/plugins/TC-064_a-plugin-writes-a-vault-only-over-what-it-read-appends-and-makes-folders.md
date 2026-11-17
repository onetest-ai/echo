---
id: TC-064
title: A plugin writes a vault's file only over what it read or where nothing is (an explicit null sha too), adds lines to a log and makes folders — never through a link out; the read-only marker of before limits nothing
priority: high
type: functional
requirements: [F-PLUG-47]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, store]
---

# TC-064: A plugin writes only over what it read

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
  ctx.command('read', (a) => ctx.fs.read(r(), a.path, a.opts))
  ctx.command('readmany', async (a) => await ctx.fs.readMany(r(), a.paths, a.opts))
  ctx.command('peek', (a) => ctx.store.read(a.path, a.opts))
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/vault/`, the plugin's store `$ECHOCTL_HOME/.echo/plugins/acme-vault/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-vault --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-vault add` | `Installed Acme vault 0.1.0`; the vault's path |
| 2 | `echoctl plugin call acme-vault new '{"path":"Notes/Acme.md","text":"# Acme kickoff\nSSO first.\n"}'`; the same again | `null`; the second fails: `“Notes/Acme.md” is there already` |
| 3 | `echoctl plugin call acme-vault hash '{"text":"# Acme kickoff\nSSO first.\n"}'`; `echoctl plugin call acme-vault put '{"path":"Notes/Acme.md","text":"# Acme kickoff\nSSO first, then billing.\n","ifSha":"<that hash>"}'`; the same `put` again; `cat "$D/vault/Notes/Acme.md"` | A 64-hex hash; `null`; the second fails: `“Notes/Acme.md” changed since you read it: read it again`; the file reads `# Acme kickoff` / `SSO first, then billing.` |
| 4 | `echoctl plugin call acme-vault put '{"path":"Notes/Acme.md","text":"lost","ifSha":null}'`; `echoctl plugin call acme-vault put '{"path":"Notes/Globex.md","text":"# Globex\n","ifSha":null}'` | The first fails: `“Notes/Acme.md” is there already` (an explicit null is "nothing there", never a write over it); the second `null` — `Notes/Globex.md` is made |
| 5 | `echoctl plugin call acme-vault log '{"path":"log.md","text":"- added · Notes/Acme.md"}'` twice; `cat "$D/vault/log.md"`; `echoctl plugin call acme-vault mkdir '{"path":"Projects/Acme/2026"}'`; `ls "$D/vault/Projects/Acme"` | `null` twice; `log.md` holds the line twice, each on its own line; `null`; `2026` |
| 6 | `echoctl plugin call acme-vault log '{"path":"Out/x.md","text":"x"}'`; `echoctl plugin call acme-vault mkdir '{"path":"Out/new"}'`; `ls "$D/outside"` | Both fail: `“Out/…” leads outside …`; `$D/outside` holds `leak.md` alone |
| 7 | `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-vault log '{"path":"log.md","text":"x"}'`; `wc -l < "$D/vault/log.md"` | `null`; `3` — the read-only marker of before (`ECHOCTL_RUN_MODE=routine`) limits nothing: no run prepares and asks any more |

## Expected Final State
`$D/vault/` holds `Wiki.md`, `Notes/Acme.md` (two lines), `Notes/Globex.md`, `log.md` (three lines), `Projects/Acme/2026/`, the two `.git` folders and the link `Out`; `$D/outside` holds `leak.md` alone.

## Teardown
- `echoctl plugin remove acme-vault --delete-data`; `echoctl plugin marketplace remove acme-local`

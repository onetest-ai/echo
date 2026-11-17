---
id: TC-064
title: A plugin keeps a vault — writes only over what it read or where nothing is, appends, makes folders, asks what a path is (a link a link), searches, copies the folder into its store, and two processes' store updates both count; while Claude prepares and asks it only reads
priority: high
type: functional
requirements: [F-PLUG-47, F-PLUG-48, F-PLUG-49, F-PLUG-50]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, folders, store]
---

# TC-064: A plugin keeps a vault with guarded writes

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A made-up vault, a folder beside it and a made-up plugin **Acme vault** (folders it may change, a host half) in a local marketplace `acme-local`:

```bash
mkdir -p "$D/vault/.git" "$D/outside"
printf '[remote] sso token\n' > "$D/vault/.git/config"
printf 'SSO notes of someone else\n' > "$D/outside/leak.md"
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
  ctx.command('snap', async () => await ctx.fs.snapshot(r()))
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
| 2 | `echoctl plugin call acme-vault new '{"path":"Notes/Acme.md","text":"# Acme kickoff\nSSO first.\n"}'`; the same again; `echoctl plugin call acme-vault stat '{"path":"Notes/Acme.md"}'`; `echoctl plugin call acme-vault hash '{"text":"# Acme kickoff\nSSO first.\n"}'` | `null`; the second fails: `“Notes/Acme.md” is there already`; the stat is `{"kind": "file", "size": 26, "real": "Notes/Acme.md", "mtime": …, "sha": "<64 hex>"}`; the hash is that same `sha` |
| 3 | `echoctl plugin call acme-vault put '{"path":"Notes/Acme.md","text":"# Acme kickoff\nSSO first, then billing.\n","ifSha":"<the sha of step 2>"}'`; the same command again; `cat "$D/vault/Notes/Acme.md"` | `null`; the second fails: `“Notes/Acme.md” changed since you read it: read it again`; the file reads `# Acme kickoff` / `SSO first, then billing.` |
| 4 | `echoctl plugin call acme-vault log '{"path":"log.md","text":"- added · Notes/Acme.md"}'` twice; `cat "$D/vault/log.md"`; `echoctl plugin call acme-vault mkdir '{"path":"Projects/Acme/2026"}'`; `echoctl plugin call acme-vault stat '{"path":"Projects/Acme"}'`; `echoctl plugin call acme-vault log '{"path":"Out/x.md","text":"x"}'` | `null` twice; `log.md` holds the line twice, each on its own line; `null`; `{"kind": "dir", "real": "Projects/Acme", …}`; the last fails: `“Out/x.md” leads outside …` — nothing new in `$D/outside` |
| 5 | `echoctl plugin call acme-vault stat '{"path":"Out"}'` | `{"kind": "link", "real": null, …}` — no `sha`, and no path of `$D/outside` anywhere in the answer |
| 6 | `echoctl plugin call acme-vault find '{"q":"sso"}'` | Exactly `[{"path": "Notes/Acme.md", "line": 2, "text": "SSO first, then billing."}]` — not `.git/config` (hidden), not `Out/leak.md` (through a link) |
| 7 | `echoctl plugin call acme-vault snap`; `find "$ECHOCTL_HOME/.echo/plugins/acme-vault/<its path>" -type f \| sort`; `echoctl plugin call acme-vault drop '{"path":"<its path>"}'`; `ls "$ECHOCTL_HOME/.echo/plugins/acme-vault/cache/snapshots"` | `{"path": "cache/snapshots/<16 hex>", "files": 2, "bytes": 90, "skipped": 0}` — no path of `$D` in it; the copy holds `Notes/Acme.md` and `log.md` only (no `.git`, nothing of `Out`); `null`; the snapshots folder is empty |
| 8 | Two processes at once: `(for i in $(seq 15); do echoctl plugin call acme-vault bump >/dev/null; done) & (for i in $(seq 15); do echoctl plugin call acme-vault bump >/dev/null; done); wait; echoctl plugin call acme-vault count` | `"30"` — no update lost |
| 9 | `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-vault log '{"path":"log.md","text":"x"}'`; `ECHOCTL_RUN_MODE=routine echoctl plugin call acme-vault find '{"q":"billing"}'` | The first fails: `changing files or settings isn't allowed while Claude prepares and asks` — `log.md` still holds two lines; the second answers the one line of `Notes/Acme.md` |

## Expected Final State
Acme vault is installed with the vault given to it; `$D/vault/` holds `Notes/Acme.md` (two lines), `log.md` (two lines), `Projects/Acme/2026/`, `.git/config` and the link `Out`, nothing else; `$D/outside` holds `leak.md` alone; the plugin's store holds `count.txt` (`30`) and no snapshot.

## Teardown
- `echoctl plugin remove acme-vault --delete-data`; `echoctl plugin marketplace remove acme-local`

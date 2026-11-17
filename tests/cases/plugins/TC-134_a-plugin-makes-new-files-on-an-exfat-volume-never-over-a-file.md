---
id: TC-134
title: A plugin makes new files (a createNew write, a copyIn) in a vault on an exFAT volume, which has no hard links — still never over a file there, no hidden part left
priority: medium
type: functional
requirements: [F-PLUG-95]
module: plugins
run: person
platform: macos
data: standard
tags: [plugins, host-half, files, folders, exfat]
---

# TC-134: A plugin makes new files on an exFAT volume

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

`run: person`: it makes and mounts a disk image (`hdiutil`), which a runner's Bash may not (`tests/app_profile.md`
rule 4). No password or prompt: a person runs these commands in a terminal.

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (CLI steps only)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the folder picker, no dialog opens)
- A 20 MB exFAT disk image mounted inside the run's folder (exFAT has no hard links: `ln` there fails with `Operation not supported`), a made-up file to hand over and a made-up plugin **Acme exfat** (folders it may change, a host half) in a local marketplace `acme-local`:

```bash
hdiutil create -size 20m -fs ExFAT -volname ECHOTEST "$D/exfat.dmg"
mkdir -p "$D/mnt" "$D/in"
hdiutil attach "$D/exfat.dmg" -mountpoint "$D/mnt" -nobrowse
mkdir -p "$D/mnt/vault"
printf 'Globex deck v3 — made-up bytes\n' > "$D/in/Deck.txt"
M="$D/market"; mkdir -p "$M/plugins/acme-exfat"
echo '{"name":"acme-local","plugins":[{"id":"acme-exfat","source":"./plugins/acme-exfat"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-exfat","name":"Acme exfat","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"folders":{"what":"vaults","write":true}}}' > "$M/plugins/acme-exfat/echo-plugin.json"
cat > "$M/plugins/acme-exfat/main.js" <<'JS'
export function apply(ctx) {
  const r = () => ctx.fs.folders()[0]
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('new', (a) => ctx.fs.write(r(), a.path, a.text, { createNew: true }))
  ctx.command('take', async (a, { files }) => await ctx.fs.copyIn(files[0], r(), a.to))
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/exfat.dmg` and its volume at `$D/mnt`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-exfat --yes`; `ECHOCTL_PICK_FOLDER="$D/mnt/vault" echoctl plugin call acme-exfat add` | `Installed Acme exfat 0.1.0`; the vault's path on the volume |
| 2 | `echoctl plugin call acme-exfat new '{"path":"Notes/Acme.md","text":"# Acme kickoff\nSSO first.\n"}'`; `cat "$D/mnt/vault/Notes/Acme.md"` | `null` (no `Operation not supported`); the file reads `# Acme kickoff` / `SSO first.` |
| 3 | The same `new` with `"text":"lost"`; `cat "$D/mnt/vault/Notes/Acme.md"` | It fails: `“Notes/Acme.md” is there already`; the file still reads `# Acme kickoff` / `SSO first.` |
| 4 | `echoctl plugin call acme-exfat take '{"to":"sources/Deck.txt"}' --file "$D/in/Deck.txt"`; `cat "$D/mnt/vault/sources/Deck.txt"` | `{"path": "sources/Deck.txt", "size": 33}`; the copy reads `Globex deck v3 — made-up bytes` |
| 5 | `printf 'other\n' > "$D/in/Other.txt"`; `echoctl plugin call acme-exfat take '{"to":"sources/Deck.txt"}' --file "$D/in/Other.txt"`; `cat "$D/mnt/vault/sources/Deck.txt"` | It fails: `“sources/Deck.txt” is there already` (copyIn's own check before it copies; step 3 is the volume's refusal); the copy still reads `Globex deck v3 — made-up bytes` |
| 6 | `find "$D/mnt/vault" -name '.*' ! -name '._*'` | Nothing: no hidden `.part` file left (`._*` files are macOS's own on exFAT) |

## Expected Final State
`$D/mnt/vault/` holds `Notes/Acme.md` (two lines) and `sources/Deck.txt` (one line), no hidden part.

## Teardown
- `echoctl plugin remove acme-exfat --delete-data`; `echoctl plugin marketplace remove acme-local`
- Always, also after a failed step: `hdiutil detach "$D/mnt"`; `rm -f "$D/exfat.dmg"`

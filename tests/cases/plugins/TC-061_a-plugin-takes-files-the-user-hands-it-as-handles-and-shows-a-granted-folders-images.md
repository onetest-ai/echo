---
id: TC-061
title: A plugin takes files the user hands it — picked (the debug stand-in) or named with plugin call --file — as handles, copies them byte for byte into its folder never over a file, is refused --file inside a run Echo started, and its page shows the folder's images but no SVG and nothing outside
priority: high
type: functional
requirements: [F-PLUG-44, F-PLUG-45]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, host-half, files, folders, images]
---

# TC-061: A plugin takes the files you hand it and shows its folder's images

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; the browser 1201 × 868
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` / `ECHOCTL_PICK_FILES` answer the pickers, no dialog opens)
- Made-up files and a made-up plugin **Acme files** (folders it may change, a host half and a window half) in a local marketplace `acme-local`:

```bash
mkdir -p "$D/vault/assets" "$D/in" "$D/outside"
printf '# Acme kickoff brief\nScope: Aurora phase 2.\n' > "$D/in/Brief.md"
printf 'Globex deck v3 — made-up bytes\n' > "$D/in/Deck.txt"
echo 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==' | base64 -d > "$D/vault/assets/logo.png"
cp "$D/vault/assets/logo.png" "$D/outside/secret.png"; printf '<svg xmlns="http://www.w3.org/2000/svg"/>' > "$D/vault/assets/x.svg"
M="$D/market"; mkdir -p "$M/plugins/acme-files"
echo '{"name":"acme-local","plugins":[{"id":"acme-files","source":"./plugins/acme-files"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-files","name":"Acme files","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js","uses":{"folders":{"what":"vaults","write":true}}}' > "$M/plugins/acme-files/echo-plugin.json"
cat > "$M/plugins/acme-files/main.js" <<'JS'
export function apply(ctx) {
  const r = () => ctx.fs.folders()[0]
  const into = async (files) => { const out = []; for (const f of files) out.push({ name: f.name, ...(await ctx.fs.copyIn(f, r(), `sources/${f.name}`)) }); return out }
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('pick', async () => await into(await ctx.system.pickFiles('add sources', { multiple: true, ext: ['md', 'txt'] })))
  ctx.command('take', async (a, { files }) => await into(files))
  ctx.command('peek', (a, { files }) => files.map((f) => ctx.files.read(f)))
  ctx.command('root', () => r())
}
JS
cat > "$M/plugins/acme-files/client.js" <<'JS'
export function apply(ctx) {
  let root = ''
  ctx.call('root').then((x) => { root = x; ctx.render() })
  const src = (p) => `plugin/acme-files/folder/${encodeURIComponent(root)}/${p}`
  ctx.activities.add({ id: 'main', label: 'Acme files', icon: '', render: () => root ? `<h2>Acme files</h2><img id="acme-logo" alt="Acme logo" src="${src('assets/logo.png')}"><img id="acme-svg" alt="Acme svg" src="${src('assets/x.svg')}">` : '<p>Loading</p>' })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant), `$D/vault/sources/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-files --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-files add` | `Installed Acme files 0.1.0`; the vault's path (the folder is given to Acme files). The install sheet named no line about files: only the folders it changes |
| 2 | `ECHOCTL_PICK_FILES="$D/in/Brief.md" echoctl plugin call acme-files pick`; `cat "$D/vault/sources/Brief.md"` | `[{"name": "Brief.md", "path": "sources/Brief.md", "size": 44}]` — no path of `$D/in` in the answer; the copy reads exactly the two lines of `$D/in/Brief.md` |
| 3 | The same `pick` again | It fails: `“sources/Brief.md” is there already` — the file in the vault is unchanged |
| 4 | `echoctl plugin call acme-files peek '{}' --file "$D/in/Deck.txt"`; then `echoctl plugin call acme-files take '{}' --file "$D/in/Deck.txt"`; `cat "$D/vault/sources/Deck.txt"` | `["Globex deck v3 — made-up bytes\n"]`; `[{"name": "Deck.txt", "path": "sources/Deck.txt", "size": 33}]`; the copy reads `Globex deck v3 — made-up bytes` |
| 5 | `ECHOCTL_HEADLESS=1 echoctl plugin call acme-files take '{}' --file "$D/in/Brief.md"` | It fails: `--file is refused inside a run Echo started` — nothing new in `$D/vault/sources/` |
| 6 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-files:main`; read `document.getElementById('acme-logo').naturalWidth` and `document.getElementById('acme-svg').naturalWidth` (after a moment) | The page shows "Acme files"; the logo loaded (`1`); the SVG didn't (`0`) |
| 7 | In the page: `fetch` of `plugin/acme-files/folder/<the vault's path, encoded>/assets/x.svg`, `…/../outside/secret.png` and `plugin/acme-files/folder/<$D/outside, encoded>/secret.png`; their `status` | `404` each: no SVG, nothing outside the folder it was given |

## Expected Final State
Acme files is installed with the vault given to it; `$D/vault/sources/` holds `Brief.md` and `Deck.txt`, nothing else.

## Teardown
- `echoctl plugin remove acme-files --delete-data`; `echoctl plugin marketplace remove acme-local`

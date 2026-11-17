---
id: TC-063
title: A plugin's page shows images of a folder it was given, but no SVG, nothing outside the folder and nothing of a folder it wasn't given
priority: medium
type: functional
requirements: [F-PLUG-45]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, client-half, folders, images]
---

# TC-063: A plugin's page shows its folder's images, and only those

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie); the browser 1201 × 868
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_PICK_FOLDER` answers the picker)
- Made-up images and a made-up plugin **Acme pictures** (folders it may read, a host half and a window half) in a local marketplace `acme-local`:

```bash
mkdir -p "$D/vault/assets" "$D/outside"
echo 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==' | base64 -d > "$D/vault/assets/logo.png"
cp "$D/vault/assets/logo.png" "$D/outside/secret.png"; printf '<svg xmlns="http://www.w3.org/2000/svg"/>' > "$D/vault/assets/x.svg"
M="$D/market"; mkdir -p "$M/plugins/acme-pics"
echo '{"name":"acme-local","plugins":[{"id":"acme-pics","source":"./plugins/acme-pics"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-pics","name":"Acme pictures","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","client":"client.js","uses":{"folders":{"what":"vaults"}}}' > "$M/plugins/acme-pics/echo-plugin.json"
cat > "$M/plugins/acme-pics/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('add', async () => await ctx.system.pickFolder('a vault'))
  ctx.command('root', () => ctx.fs.folders()[0] || '')
}
JS
cat > "$M/plugins/acme-pics/client.js" <<'JS'
export function apply(ctx) {
  let root = ''
  ctx.call('root').then((x) => { root = x; ctx.render() })
  const src = (p) => `plugin/acme-pics/folder/${encodeURIComponent(root)}/${p}`
  ctx.activities.add({ id: 'main', label: 'Acme pictures', icon: '', render: () => root ? `<h2>Acme pictures</h2><img id="acme-logo" alt="Acme logo" src="${src('assets/logo.png')}"><img id="acme-svg" alt="Acme svg" src="${src('assets/x.svg')}">` : '<p>Loading</p>' })
}
JS
echoctl plugin marketplace add "$M" --name acme-local
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugin, its folder grant)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-pics --yes`; `ECHOCTL_PICK_FOLDER="$D/vault" echoctl plugin call acme-pics add` | `Installed Acme pictures 0.1.0`; the vault's path |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-pics:main`; after a moment read `document.getElementById('acme-logo').naturalWidth` and `document.getElementById('acme-svg').naturalWidth` | The page shows "Acme pictures"; the logo loaded (`1`); the SVG didn't (`0`) |
| 3 | In the page, with `V = encodeURIComponent(<the vault's path from step 1>)`: the `status` of `fetch` for `plugin/acme-pics/folder/${V}/assets/x.svg`, `plugin/acme-pics/folder/${V}/%2E%2E%2Foutside%2Fsecret.png` and `plugin/acme-pics/folder/${encodeURIComponent(<$D/outside>)}/secret.png` | `404` each: no SVG, nothing outside the folder (a `../` sent as is), nothing of a folder it wasn't given |

## Expected Final State
Acme pictures is installed with the vault given to it; nothing changed in the vault.

## Teardown
- `echoctl plugin remove acme-pics --delete-data`; `echoctl plugin marketplace remove acme-local`
